//! Write the open case as a complete PSS/E RAW file, revision 33 or 35.
//!
//! A RAW 33 or 35 case is read record by record with case-diff's parser
//! (with its Records edits and study edits already written in, see
//! [`crate::working_case`]) and written back out in the revision asked for.
//! Records that need no change keep their lines as they were; records whose
//! layout differs between the revisions are rewritten field by field.
//! Anything else (MATPOWER, other RAW revisions, files that parser can't
//! read) is written from the power flow model instead, which keeps less.
//!
//! Every loss or approximation is listed in [`RawExport::notes`].

mod convert;
mod model;

use crate::diff::parser::{self, CASE_DETAILS, DiffCase};
use crate::diff::value::{Number, Value};
use crate::power::PowerCase;
use serde::Serialize;
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashMap};

/// What an export wrote, and what it couldn't keep.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct RawExport {
    /// The RAW revision written: 33 or 35.
    pub revision: u32,
    /// "records" when the file's own records were written, "model" when the
    /// power flow model was (MATPOWER cases, other RAW revisions).
    #[schemars(extend("enum" = ["records", "model"]))]
    pub source: String,
    /// Records written, in every section.
    pub records: usize,
    /// What changed on the way, such as fields the revision has no place
    /// for. Empty when the file holds everything the case had.
    pub notes: Vec<String>,
}

pub(crate) type Fields = Vec<(Name, Value)>;

/// A field's name: mostly fixed, so not allocated.
pub(crate) type Name = std::borrow::Cow<'static, str>;

fn named(fields: Vec<(String, Value)>) -> Fields {
    fields.into_iter().map(|(n, v)| (n.into(), v)).collect()
}

/// One record: its fields, and its lines when they can be written as read.
#[derive(Clone, Debug)]
pub(crate) struct Rec {
    pub fields: Fields,
    /// The record's lines from the file. Cleared by any change, so the
    /// record is then written field by field.
    pub lines: Option<Vec<String>>,
}

impl Rec {
    pub fn new(fields: Fields) -> Self {
        Self {
            fields,
            lines: None,
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn number(&self, name: &str) -> f64 {
        self.get(name).map_or(0.0, value_f64)
    }

    pub fn text(&self, name: &str) -> &str {
        self.get(name).and_then(Value::as_text).unwrap_or("")
    }

    pub fn set(&mut self, name: &str, value: Value) {
        self.lines = None;
        match self.fields.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = value,
            None => self.fields.push((name.to_string().into(), value)),
        }
    }

    /// Copy `from` into `to` (keeping `from`), when `from` is there.
    pub fn copy(&mut self, from: &str, to: &str) {
        if let Some(v) = self.get(from).cloned() {
            self.set(to, v);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Item {
    Rec(Rec),
    /// Lines the reader keeps but doesn't decode, written as they are.
    Lines(Vec<String>),
}

/// A case as RAW records, in one revision.
#[derive(Clone, Debug)]
pub(crate) struct Doc {
    pub revision: i64,
    /// IC, SBASE, REV, XFRRAT, NXFRAT, BASFRQ.
    pub ident: Fields,
    pub titles: [String; 2],
    /// Every section by name, SYSTEM-WIDE included, records in file order.
    pub sections: HashMap<String, Vec<Item>>,
    /// Revision 35 substation data, between the last section and Q.
    pub tail: Vec<String>,
}

impl Doc {
    pub fn items(&self, section: &str) -> &[Item] {
        self.sections.get(section).map_or(&[], Vec::as_slice)
    }

    pub fn items_mut(&mut self, section: &str) -> &mut Vec<Item> {
        self.sections.entry(section.to_string()).or_default()
    }

    pub fn recs_mut(&mut self, section: &str) -> impl Iterator<Item = &mut Rec> {
        self.items_mut(section).iter_mut().filter_map(|i| match i {
            Item::Rec(r) => Some(r),
            Item::Lines(_) => None,
        })
    }

    /// Records written. Kept lines count one record per line, or per
    /// record's worth of lines where the layout is known.
    fn count(&self) -> usize {
        self.sections
            .iter()
            .map(|(section, items)| {
                let per = layout(section, self.revision, false).map_or(1, |l| l.lines.len().max(1));
                items
                    .iter()
                    .map(|i| match i {
                        Item::Rec(_) => 1,
                        Item::Lines(lines) => {
                            let data = lines
                                .iter()
                                .filter(|l| {
                                    let t = l.trim_start();
                                    !(t.is_empty() || t.starts_with("@!") || t.starts_with('/'))
                                })
                                .count();
                            if section == "TRANSFORMER" || section == "IMPEDANCE CORRECTION" {
                                1
                            } else {
                                data.div_ceil(per).max(1)
                            }
                        }
                    })
                    .sum::<usize>()
            })
            .sum()
    }
}

/// Notes gathered while converting, each counted once per record.
#[derive(Default)]
pub(crate) struct Notes(BTreeMap<String, usize>);

impl Notes {
    pub fn add(&mut self, note: impl Into<String>) {
        *self.0.entry(note.into()).or_default() += 1;
    }

    fn into_lines(self, first: Vec<String>) -> Vec<String> {
        let mut out = first;
        out.extend(self.0.into_iter().map(|(note, n)| {
            if n == 1 {
                note
            } else {
                format!("{note} ({n} records)")
            }
        }));
        out
    }
}

pub(crate) fn int(v: i64) -> Value {
    Value::Number(Number::from_i64(v))
}

/// A number as RAW text: the shortest form that reads back the same.
/// Rust prints -0 as "-0"; RAW readers don't need the sign.
pub(crate) fn num(v: f64) -> Value {
    Value::Number(Number::from_f64(v))
}

pub(crate) fn text(s: &str) -> Value {
    Value::Text(s.to_string())
}

pub(crate) fn value_f64(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.to_f64(),
        Value::Text(t) => t.trim().parse().unwrap_or(0.0),
    }
}

fn is_zero(v: Option<&Value>) -> bool {
    v.is_none_or(|v| match v {
        Value::Number(_) => value_f64(v) == 0.0,
        Value::Text(t) => t.trim().is_empty(),
    })
}

// ---------------------------------------------------------------------------
// Reading.

/// Split revision 35 substation data off the end of a file, which
/// case-diff's parser doesn't read: everything after the INDUCTION MACHINE
/// terminator up to the final Q.
fn split_tail(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.lines().collect();
    let structural = |l: &str| l.split('/').next().unwrap_or_default().trim().to_string();
    let skip = |l: &str| {
        let t = l.trim_start();
        t.is_empty() || t.starts_with("@!") || t.starts_with('/')
    };
    let Some(first) = lines.iter().position(|l| !skip(l)) else {
        return (text.to_string(), vec![]);
    };
    let sections = parser::schema()["section_order"]["35"]
        .as_array()
        .map_or(0, Vec::len);
    let mut seen = 0;
    // Case identification, then two title lines.
    for (i, line) in lines.iter().enumerate().skip(first + 3) {
        if skip(line) {
            continue;
        }
        if structural(line) == "0" {
            seen += 1;
            if seen == sections {
                let end = lines
                    .iter()
                    .rposition(|l| structural(l).eq_ignore_ascii_case("q"))
                    .unwrap_or(lines.len());
                if end <= i + 1 {
                    return (text.to_string(), vec![]);
                }
                let tail = lines[i + 1..end].iter().map(|s| s.to_string()).collect();
                let mut main: Vec<&str> = lines[..=i].to_vec();
                main.extend_from_slice(&lines[end..]);
                return (main.join("\n") + "\n", tail);
            }
        }
    }
    (text.to_string(), vec![])
}

/// What the reader couldn't decode but kept, to write back as it was.
fn kept(message: &str) -> bool {
    matches!(
        message,
        "Unsupported populated section layout"
            | "Unsupported three-winding transformer"
            | "Unsupported setting parameter"
    ) || message.starts_with("Unsupported setting tag")
}

fn from_diff(case: DiffCase, tail: Vec<String>) -> Doc {
    let mut ident = Vec::new();
    let mut titles = [String::new(), String::new()];
    let mut placed: HashMap<String, Vec<(usize, Item)>> = HashMap::new();
    for (section, records) in case.sections {
        for r in records {
            if section == CASE_DETAILS {
                match r.key.first().and_then(Value::as_text) {
                    Some("CASE IDENTIFICATION") => ident = named(r.fields),
                    Some("TITLE 1") => titles[0] = r.raw_lines.concat(),
                    Some("TITLE 2") => titles[1] = r.raw_lines.concat(),
                    _ => placed.entry("SYSTEM-WIDE".into()).or_default().push((
                        r.line_number,
                        Item::Rec(Rec {
                            fields: named(r.fields),
                            lines: Some(r.raw_lines),
                        }),
                    )),
                }
                continue;
            }
            placed.entry(section.clone()).or_default().push((
                r.line_number,
                Item::Rec(Rec {
                    fields: named(r.fields),
                    lines: Some(r.raw_lines),
                }),
            ));
        }
    }
    for d in case.diagnostics {
        if !kept(&d.message) || d.raw_lines.is_empty() {
            continue;
        }
        placed
            .entry(d.section.clone())
            .or_default()
            .push((d.line_number, Item::Lines(d.raw_lines)));
    }
    let sections = placed
        .into_iter()
        .map(|(name, mut items)| {
            items.sort_by_key(|(line, _)| *line);
            (name, items.into_iter().map(|(_, i)| i).collect())
        })
        .collect();
    Doc {
        revision: case.revision,
        ident,
        titles,
        sections,
        tail,
    }
}

// ---------------------------------------------------------------------------
// Writing.

/// One line of a record layout: field names and types.
pub(crate) type LineLayout = Vec<(String, String)>;

pub(crate) struct Layout {
    pub lines: Vec<LineLayout>,
    /// (start index, width) of the repeating block group at the end.
    group: Option<(usize, usize)>,
}

/// Sections whose records read the same in revisions 33 and 35, so one
/// revision's layout serves the other where the schema has only one.
pub(crate) const SAME_LAYOUT: &[&str] = &[
    "BUS",
    "FIXED SHUNT",
    "AREA",
    "VSC DC LINE",
    "MULTI-TERMINAL DC",
    "MULTI-SECTION LINE",
    "ZONE",
    "INTER-AREA TRANSFER",
    "OWNER",
    "FACTS DEVICE",
    "GEN DEVICE",
    "INDUCTION MACHINE",
];

fn line_layout(line: &Json) -> LineLayout {
    line["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            (
                f["name"].as_str().unwrap_or_default().to_string(),
                f["type"].as_str().unwrap_or("string").to_string(),
            )
        })
        .collect()
}

pub(crate) fn layout(section: &str, revision: i64, three_winding: bool) -> Option<Layout> {
    let layouts = &parser::schema()["sections"][section]["layouts"];
    let mut layout = &layouts[revision.to_string()];
    if layout["lines"].is_null() && layout["variants"].is_null() && SAME_LAYOUT.contains(&section) {
        let other = if revision == 33 { "35" } else { "33" };
        layout = &layouts[other];
    }
    if let Some(variants) = layout["variants"].as_array() {
        layout = &variants[usize::from(three_winding)];
    }
    let lines: Vec<LineLayout> = layout["lines"]
        .as_array()?
        .iter()
        .map(line_layout)
        .collect();
    let group = layout["lines"][0]["trailing_group"].as_object().map(|g| {
        (
            g["start_index"].as_u64().unwrap_or(0) as usize,
            g["width"].as_u64().unwrap_or(1) as usize,
        )
    });
    Some(Layout { lines, group })
}

/// The value of a field a record doesn't have.
fn default(name: &str, kind: &str) -> Value {
    match name {
        "ID" => text("1"),
        "S1" | "S2" | "S3" | "S4" | "S5" | "S6" | "S7" | "S8" => int(1),
        _ => match kind {
            "integer" => int(0),
            "number" => num(0.0),
            _ => text(""),
        },
    }
}

pub(crate) fn quote(s: &str) -> String {
    if s.contains('\'') && !s.contains('"') {
        format!("\"{s}\"")
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}

/// [`token`], written onto `out`. Plain numbers, most of a file, are
/// written without a string of their own.
fn push_token(out: &mut String, value: &Value, kind: &str) {
    match (value, kind) {
        (Value::Number(n), k) if k != "integer" && k != "string" => {
            use std::fmt::Write as _;
            let _ = write!(out, "{n}");
        }
        _ => out.push_str(&token(value, kind)),
    }
}

pub(crate) fn token(value: &Value, kind: &str) -> String {
    match (value, kind) {
        (Value::Text(s), "string") => quote(s),
        (Value::Text(s), _) => s.trim().to_string(),
        (Value::Number(n), "integer") => {
            let s = n.to_string();
            if s.contains(['.', 'E']) {
                format!("{}", value_f64(value).round() as i64)
            } else {
                s
            }
        }
        (Value::Number(n), "string") => quote(&n.to_string()),
        (Value::Number(n), _) => n.to_string(),
    }
}

fn write_rec(
    out: &mut Vec<String>,
    section: &str,
    revision: i64,
    rec: &Rec,
    layouts: &mut [Option<Option<Layout>>; 2],
) -> Result<(), String> {
    if let Some(lines) = &rec.lines {
        out.extend(lines.iter().cloned());
        return Ok(());
    }
    rec_lines(section, revision, rec, layouts, |fields| {
        let mut text = String::with_capacity(fields.len() * 8);
        for (j, (value, kind)) in fields.iter().enumerate() {
            if j > 0 {
                text.push(',');
            }
            push_token(&mut text, value, kind);
        }
        out.push(text);
    })
}

/// A record's fields laid out as its lines in `revision`, each line given
/// to `line` as (value, kind) pairs, with defaults for the fields the record
/// leaves out. The layouts are read from the schema once per section (and
/// per transformer kind) into `layouts`.
pub(crate) fn rec_lines(
    section: &str,
    revision: i64,
    rec: &Rec,
    layouts: &mut [Option<Option<Layout>>; 2],
    mut line: impl FnMut(&[(std::borrow::Cow<'_, Value>, &str)]),
) -> Result<(), String> {
    let three_winding = section == "TRANSFORMER" && !is_zero(rec.get("K"));
    // Read from the schema once per section (and per transformer kind).
    let layout = layouts[usize::from(three_winding)]
        .get_or_insert_with(|| layout(section, revision, three_winding))
        .as_ref()
        .ok_or_else(|| format!("RAW {revision} {section} records have no known layout."))?;
    // Blocks at the end: write as many as the record has, at least one.
    let groups = layout.group.map(|(start, width)| {
        let names = &layout.lines[0];
        let max = (names.len() - start) / width;
        let present = (0..max)
            .rev()
            .find(|g| {
                names[start + g * width..start + (g + 1) * width]
                    .iter()
                    .any(|(n, _)| rec.get(n).is_some())
            })
            .map_or(1, |g| g + 1);
        start + present * width
    });
    let mut fields = Vec::new();
    let mut hint = 0;
    for (i, names) in layout.lines.iter().enumerate() {
        let width = if i == 0 {
            groups.unwrap_or(names.len())
        } else {
            names.len()
        };
        fields.clear();
        for (name, kind) in &names[..width] {
            // Records mostly hold their fields in layout order: look from
            // after the last one found first.
            let found = (hint..rec.fields.len())
                .chain(0..hint)
                .find(|&k| rec.fields[k].0 == *name);
            if let Some(k) = found {
                hint = k + 1;
            }
            let value = match found.map(|k| &rec.fields[k].1) {
                Some(v) => std::borrow::Cow::Borrowed(v),
                None => std::borrow::Cow::Owned(default(name, kind)),
            };
            fields.push((value, kind.as_str()));
        }
        line(&fields);
    }
    Ok(())
}

fn header_lines(section: &str, revision: i64) -> Vec<String> {
    layout(section, revision, false)
        .map(|l| {
            l.lines
                .iter()
                .map(|line| {
                    let names: Vec<String> = line
                        .iter()
                        .map(|(n, k)| {
                            if k == "string" {
                                format!("'{n}'")
                            } else {
                                n.clone()
                            }
                        })
                        .collect();
                    format!("@!{}", names.join(","))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn write(doc: &Doc, source_name: &str) -> Result<String, String> {
    let revision = doc.revision;
    let order: Vec<String> = parser::schema()["section_order"][revision.to_string()]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect();
    let mut out: Vec<String> = Vec::new();
    out.push("@!IC,SBASE,REV,XFRRAT,NXFRAT,BASFRQ".into());
    let id = |name: &str, kind: &str, fallback: Value| -> String {
        let v = doc
            .ident
            .iter()
            .find(|(n, _)| n == name)
            .map_or(fallback, |(_, v)| v.clone());
        token(&v, kind)
    };
    let comment = source_name.replace(['\r', '\n'], " ");
    out.push(format!(
        "{},{},{},{},{},{}     / Written by Stannum from {comment}",
        id("IC", "integer", int(0)),
        id("SBASE", "number", num(100.0)),
        revision,
        id("XFRRAT", "integer", int(0)),
        id("NXFRAT", "integer", int(0)),
        id("BASFRQ", "number", num(60.0)),
    ));
    out.push(doc.titles[0].clone());
    out.push(doc.titles[1].clone());
    for (i, section) in order.iter().enumerate() {
        if section != "SYSTEM-WIDE" {
            out.extend(header_lines(section, revision));
        }
        let mut layouts = [None, None];
        for item in doc.items(section) {
            match item {
                Item::Rec(rec) => write_rec(&mut out, section, revision, rec, &mut layouts)?,
                Item::Lines(lines) => out.extend(lines.iter().cloned()),
            }
        }
        out.push(match order.get(i + 1) {
            Some(next) => format!("0 / END OF {section} DATA, BEGIN {next} DATA"),
            None if !doc.tail.is_empty() => {
                format!("0 / END OF {section} DATA, BEGIN SUBSTATION DATA")
            }
            None => format!("0 / END OF {section} DATA"),
        });
    }
    out.extend(doc.tail.iter().cloned());
    out.push("Q".into());
    let mut text = out.join("\n");
    text.push('\n');
    Ok(text)
}

fn encode(text: &str) -> Result<Vec<u8>, String> {
    let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(text);
    if unmappable {
        return Err(
            "The case has a character RAW files (Windows-1252) can't hold, such as in a name."
                .into(),
        );
    }
    Ok(bytes.into_owned())
}

fn check_revision(revision: u32) -> Result<(), String> {
    if matches!(revision, 33 | 35) {
        Ok(())
    } else {
        Err(format!(
            "Stannum writes RAW revisions 33 and 35, not {revision}."
        ))
    }
}

/// Write a RAW 33 or 35 file's text as `revision`, record by record.
/// Fails when case-diff's parser can't read the file.
pub fn from_raw(name: &str, bytes: &[u8], revision: u32) -> Result<(Vec<u8>, RawExport), String> {
    check_revision(revision)?;
    // Windows-1252 maps every byte to a character and back, so the text
    // written out has the file's own bytes, whatever its encoding.
    let text = encoding_rs::WINDOWS_1252
        .decode_without_bom_handling(bytes)
        .0;
    // Substation data only exists in revision 35 files, where the parser
    // stops before it.
    let (main, tail) = split_tail(&text);
    let main_bytes = encode(&main)?;
    let case = parser::parse_bytes_for_viewing(name, &main_bytes).map_err(|e| e.to_string())?;
    let from = case.revision;
    let tail = if from == 35 { tail } else { vec![] };
    let mut doc = from_diff(case, tail);
    let mut notes = Notes::default();
    convert::convert(&mut doc, i64::from(revision), &mut notes);
    let records = doc.count();
    let text = write(&doc, name)?;
    let first = if from == i64::from(revision) {
        vec![]
    } else {
        vec![format!("Converted from RAW revision {from}.")]
    };
    Ok((
        encode(&text)?,
        RawExport {
            revision,
            source: "records".into(),
            records,
            notes: notes.into_lines(first),
        },
    ))
}

/// A RAW 33 or 35 file's records in revision 35, for the Stannum case
/// format, with notes on anything the move to revision 35 changed.
pub(crate) fn read_doc35(name: &str, bytes: &[u8]) -> Result<(Doc, Vec<String>), String> {
    let text = encoding_rs::WINDOWS_1252
        .decode_without_bom_handling(bytes)
        .0;
    let (main, tail) = split_tail(&text);
    let main_bytes = encode(&main)?;
    let case = parser::parse_bytes_for_viewing(name, &main_bytes).map_err(|e| e.to_string())?;
    let from = case.revision;
    let tail = if from == 35 { tail } else { vec![] };
    let mut doc = from_diff(case, tail);
    let mut notes = Notes::default();
    convert::convert(&mut doc, 35, &mut notes);
    let first = if from == 35 {
        vec![]
    } else {
        vec![format!("Converted from RAW revision {from}.")]
    };
    Ok((doc, notes.into_lines(first)))
}

/// The power flow model as revision 35 records (MATPOWER cases, and RAW
/// revisions read for power flow only).
pub(crate) fn model_doc35(name: &str, power: &PowerCase) -> (Doc, Vec<String>) {
    let mut notes = Notes::default();
    let mut doc = model::doc(name, power, &mut notes);
    convert::convert(&mut doc, 35, &mut notes);
    (doc, notes.into_lines(vec![]))
}

/// A revision 35 document as RAW text.
pub(crate) fn write_doc(doc: &Doc, source_name: &str) -> Result<String, String> {
    write(doc, source_name)
}

/// Write the power flow model as RAW. `why` says why the model is used,
/// and becomes the first note.
pub fn from_model(
    name: &str,
    power: &PowerCase,
    revision: u32,
    why: String,
) -> Result<(Vec<u8>, RawExport), String> {
    check_revision(revision)?;
    let mut notes = Notes::default();
    let mut doc = model::doc(name, power, &mut notes);
    convert::convert(&mut doc, i64::from(revision), &mut notes);
    let records = doc.count();
    let text = write(&doc, name)?;
    Ok((
        encode(&text)?,
        RawExport {
            revision,
            source: "model".into(),
            records,
            notes: notes.into_lines(vec![why]),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_back_the_same() {
        assert_eq!(num(0.1).display(), "0.1");
        assert_eq!(num(-0.0).display(), "0.0");
        assert_eq!(num(1.0).display(), "1.0");
        assert_eq!(value_f64(&num(1.2345e-7)), 1.2345e-7);
        assert_eq!(value_f64(&num(f64::INFINITY)), 0.0);
    }

    #[test]
    fn text_is_quoted_so_it_reads_back() {
        assert_eq!(quote("Bus 1 "), "'Bus 1 '");
        assert_eq!(quote("O'Brien"), "\"O'Brien\"");
        assert_eq!(quote("a'b\"c"), "'a''b\"c'");
    }

    #[test]
    fn substation_data_splits_off_the_end() {
        let lines = ["0, 100, 35, 0, 0, 60", "t1", "t2"];
        let mut text = lines.join("\n") + "\n";
        for _ in 0..21 {
            text.push_str("0 / end\n");
        }
        text.push_str("1,'S',0,0,0.1\n0 / END OF SUBSTATION DATA\nQ\n");
        let (main, tail) = split_tail(&text);
        assert_eq!(tail, ["1,'S',0,0,0.1", "0 / END OF SUBSTATION DATA"]);
        assert!(main.ends_with("0 / end\nQ\n"));
    }
}
