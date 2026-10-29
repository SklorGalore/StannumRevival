//! Stannum's open case format (`.stn`, preview): cases read from and written
//! to it.
//!
//! The text format itself lives in the `stannum-format` crate; this module
//! maps it to and from PSS/E RAW revision 35 records, which the rest of
//! Stannum already reads and writes:
//!
//! - RAW 33 or 35 → `.stn`: the file's records, moved to revision 35 where
//!   needed, become records of the matching tables, field by field.
//!   MATPOWER cases (and RAW revisions read for power flow only) go through
//!   the power flow model first, as RAW export does.
//! - `.stn` → RAW 35 text, which opens as any RAW file does.
//!
//! Units and codes become the format's own on the way in: transformer
//! impedances on the transformer's base, taps in per unit of the winding's
//! nominal voltage, admittance load in MVAr consumed, names for codes. When
//! a transformer's numbers don't convert exactly, the originals are kept
//! in an `x.psse.orig` field, so writing RAW again gives them back unless
//! the transformer was changed. RAW data Stannum doesn't map (GNE devices,
//! multi-terminal DC, solution settings, substation data) is kept line for
//! line in `x.psse.lines` records.

mod maps;
mod transformer;

use crate::diff::tokenizer::tokenize;
use crate::diff::value::{Number, Value as RawValue};
use crate::raw_export::{self, Doc, Fields, Item, Rec, int, layout, num, value_f64};
pub(crate) use crate::raw_export::{int as raw_int, num as raw_num, text as raw_text};
use stannum_format::{Document, Record, Value};
use std::collections::HashMap;

pub use stannum_format::ReadError;

/// What converting a case to the Stannum format kept or changed.
#[derive(Debug, Clone, Default, serde::Serialize, schemars::JsonSchema)]
pub struct StnExport {
    /// Records written, in every table.
    pub records: usize,
    /// Anything worth knowing: conversions, data kept as RAW lines.
    pub notes: Vec<String>,
}

/// Whether bytes look like a Stannum case file (its header line).
pub fn is_stn(bytes: &[u8]) -> bool {
    let start = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    start.starts_with(b"stannum-case ")
}

/// A RAW 33 or 35 file as a Stannum case.
pub fn from_raw(name: &str, bytes: &[u8]) -> Result<(Document, StnExport), String> {
    let (doc, notes) = raw_export::read_doc35(name, bytes)?;
    Ok(from_doc(&doc, notes))
}

/// The power flow model as a Stannum case, for cases whose records can't
/// be read one by one (MATPOWER, older RAW revisions).
pub fn from_model(
    name: &str,
    power: &crate::power::PowerCase,
    why: String,
) -> (Document, StnExport) {
    let (doc, mut notes) = raw_export::model_doc35(name, power);
    notes.insert(0, why);
    from_doc(&doc, notes)
}

/// Write a Stannum case as text.
pub fn write(doc: &Document) -> Result<String, String> {
    stannum_format::write(doc)
}

/// Read a Stannum case's text.
pub fn read(text: &str) -> Result<Document, ReadError> {
    stannum_format::read_case(text)
}

/// A Stannum case as RAW revision 35 text.
pub fn to_raw(doc: &Document, source_name: &str) -> Result<String, String> {
    let raw = to_doc(doc)?;
    raw_export::write_doc(&raw, source_name)
}

/// Read a `.stn` file's bytes and write them as RAW 35 text, the form the
/// rest of Stannum opens.
pub fn raw_text_of(name: &str, bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{name} isn't UTF-8 text"))?;
    let doc = read(text).map_err(|e| format!("{name}: {e}"))?;
    to_raw(&doc, name)
}

/// Read a `.stn` file to open it: the file, its RAW 35 text, and the line
/// of each record by typed reference (`generator:101:1`), to point
/// warnings at.
#[allow(clippy::type_complexity)]
pub fn open(
    name: &str,
    bytes: &[u8],
) -> Result<(Document, String, HashMap<String, usize>), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{name} isn't UTF-8 text"))?;
    let doc = read(text).map_err(|e| format!("{name}: {e}"))?;
    let mut lines = HashMap::new();
    for table in &doc.tables {
        let Some(def) = doc.schema.table(&table.name) else {
            continue;
        };
        for r in &table.records {
            lines.insert(format!("{}:{}", def.kind, r.id), r.line);
        }
    }
    let raw = to_raw(&doc, name)?;
    Ok((doc, raw, lines))
}

/// A case file's bytes as RAW text: a Stannum case converted, anything
/// else as it is.
pub fn raw_bytes_of(name: &str, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if is_stn(&bytes) {
        raw_text_of(name, &bytes).map(String::into_bytes)
    } else {
        Ok(bytes)
    }
}

// ---------------------------------------------------------------------------
// RAW values.

pub(crate) fn raw_get<'a>(f: &'a Fields, name: &str) -> Option<&'a RawValue> {
    f.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

pub(crate) fn raw_f64(f: &Fields, name: &str, default: f64) -> f64 {
    raw_get(f, name).map_or(default, |v| match v {
        RawValue::Text(t) if t.trim().is_empty() => default,
        v => value_f64(v),
    })
}

pub(crate) fn raw_i64(f: &Fields, name: &str, default: i64) -> i64 {
    raw_get(f, name).map_or(default, |v| match v {
        RawValue::Text(t) if t.trim().is_empty() => default,
        v => value_f64(v).round() as i64,
    })
}

pub(crate) fn raw_str(f: &Fields, name: &str, default: &str) -> String {
    match raw_get(f, name) {
        Some(RawValue::Text(t)) => t.trim().to_string(),
        Some(RawValue::Number(n)) => n.to_string(),
        None => default.to_string(),
    }
}

// ---------------------------------------------------------------------------
// RAW 35 → Stannum.

/// Facts about buses that other records need.
pub(crate) struct Buses {
    /// Bus number → (area, zone, owner, base kV).
    pub info: HashMap<i64, (i64, i64, i64, f64)>,
}

impl Buses {
    pub fn id(n: i64) -> Value {
        Value::Str(n.to_string())
    }
    pub fn kv(&self, n: i64) -> f64 {
        self.info.get(&n).map_or(0.0, |b| b.3)
    }
}

pub(crate) struct Ctx {
    pub buses: Buses,
    pub sbase: f64,
}

/// The fields of a record, decoding lines the reader kept undecoded.
fn record_fields(section: &str, item: &Item) -> Option<Fields> {
    match item {
        Item::Rec(r) if !r.fields.is_empty() => Some(r.fields.clone()),
        Item::Rec(r) => decode(section, r.lines.as_deref().unwrap_or_default()),
        Item::Lines(lines) => decode(section, lines),
    }
}

fn data_lines(lines: &[String]) -> Vec<&String> {
    lines
        .iter()
        .filter(|l| {
            let t = l.trim_start();
            !(t.is_empty() || t.starts_with("@!") || t.starts_with('/'))
        })
        .collect()
}

fn raw_value(token: &str, kind: &str) -> Option<RawValue> {
    Some(match kind {
        "string" => RawValue::Text(token.to_string()),
        _ if token.trim().is_empty() => return None,
        "integer" => {
            RawValue::Number(Number::parse_integer(token).or_else(|| Number::parse_number(token))?)
        }
        _ => RawValue::Number(Number::parse_number(token)?),
    })
}

/// Decode a record's lines by the RAW 35 layout, allowing trailing fields
/// to be left out (they take their defaults). None when the lines don't
/// fit the layout.
fn decode(section: &str, lines: &[String]) -> Option<Fields> {
    let data = data_lines(lines);
    let three = section == "TRANSFORMER" && {
        let first = tokenize(data.first()?).ok()?;
        first
            .get(2)
            .is_some_and(|k| k.trim().parse::<f64>().is_ok_and(|k| k != 0.0))
    };
    let lay = layout(section, 35, three)?;
    let line_layouts: Vec<&Vec<(String, String)>> = if section == "VSC DC LINE" {
        // A VSC line is its own line, then one line per converter.
        vec![lay.lines.first()?, lay.lines.get(1)?, lay.lines.get(1)?]
    } else {
        lay.lines.iter().collect()
    };
    if data.len() != line_layouts.len() {
        return None;
    }
    let mut out = Fields::new();
    for (k, (line, names)) in data.iter().zip(line_layouts).enumerate() {
        let tokens = tokenize(line).ok()?;
        if tokens.len() > names.len() {
            return None;
        }
        for (t, (name, kind)) in tokens.iter().zip(names) {
            // The second VSC converter's fields get a 2 suffix.
            let name = if section == "VSC DC LINE" && k == 2 {
                format!("{name}#2")
            } else {
                name.clone()
            };
            if let Some(v) = raw_value(t, kind) {
                out.push((name, v));
            } else if !t.trim().is_empty() {
                return None;
            }
        }
    }
    Some(out)
}

fn slug(section: &str) -> String {
    section.to_lowercase().replace([' ', '-'], "_")
}

fn psse_lines(
    doc: &mut Document,
    section: &str,
    lines: &[String],
    counter: &mut HashMap<String, usize>,
) {
    let s = slug(section);
    let n = counter.entry(s.clone()).or_default();
    *n += 1;
    let mut r = Record::new(format!("{s}:{n}"));
    r.set("section", Value::Str(section.to_string()));
    r.set(
        "text",
        Value::List(lines.iter().map(|l| Value::Str(l.clone())).collect()),
    );
    doc.push("x.psse.lines", r);
}

fn from_doc(raw: &Doc, mut notes: Vec<String>) -> (Document, StnExport) {
    let mut doc = Document::new(stannum_format::case_schema());
    let ident = &raw.ident;
    let sbase = raw_f64(ident, "SBASE", 100.0);
    let mut case = Record::new("main");
    case.set("name", Value::Str(raw.titles[0].trim().to_string()));
    case.set("description", Value::Str(raw.titles[1].trim().to_string()));
    case.set("base_mva", Value::Float(sbase));
    let freq = raw_f64(ident, "BASFRQ", 0.0);
    if freq > 0.0 {
        case.set("frequency_hz", Value::Float(freq));
    }
    for (name, field) in [
        ("IC", "x.psse.ic"),
        ("XFRRAT", "x.psse.xfrrat"),
        ("NXFRAT", "x.psse.nxfrat"),
    ] {
        let v = raw_f64(ident, name, 0.0);
        if v != 0.0 {
            case.set(field, Value::Raw(stannum_format::Value::Float(v).to_text()));
        }
    }
    doc.push("case", case);

    let mut info = HashMap::new();
    for item in raw.items("BUS") {
        if let Some(f) = record_fields("BUS", item) {
            info.insert(
                raw_i64(&f, "I", 0),
                (
                    raw_i64(&f, "AREA", 1),
                    raw_i64(&f, "ZONE", 1),
                    raw_i64(&f, "OWNER", 1),
                    raw_f64(&f, "BASKV", 0.0),
                ),
            );
        }
    }
    let ctx = Ctx {
        buses: Buses { info },
        sbase,
    };
    let mut counter = HashMap::new();
    let mut kept = 0usize;
    let mut inexact = 0usize;
    let order: Vec<String> = crate::diff::parser::schema()["section_order"]["35"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect();
    for section in &order {
        for item in raw.items(section) {
            let fields = record_fields(section, item);
            let mapped = match (section.as_str(), fields) {
                ("IMPEDANCE CORRECTION", _) => impedance_tables(item).map(|recs| {
                    for r in recs {
                        doc.push("impedance_table", r);
                    }
                }),
                ("TRANSFORMER", Some(f)) => transformer::import(&ctx, &f, &mut inexact)
                    .and_then(|(t, r)| push_new(&mut doc, t, r)),
                (_, Some(f)) => {
                    maps::import(&ctx, section, &f).and_then(|(t, r)| push_new(&mut doc, t, r))
                }
                _ => None,
            };
            if mapped.is_none() {
                let lines: Vec<String> = match item {
                    Item::Rec(r) => match &r.lines {
                        Some(l) => l.clone(),
                        None => continue,
                    },
                    Item::Lines(l) => l.clone(),
                };
                kept += 1;
                psse_lines(&mut doc, section, &lines, &mut counter);
            }
        }
    }
    if !raw.tail.is_empty() {
        kept += 1;
        psse_lines(&mut doc, "SUBSTATION", &raw.tail, &mut counter);
    }
    if kept > 0 {
        notes.push(format!(
            "{kept} RAW records Stannum doesn't map yet (such as GNE devices, multi-terminal DC, solution settings or substation data) kept as RAW lines in x.psse.lines"
        ));
    }
    if inexact > 0 {
        notes.push(format!(
            "{inexact} transformers whose impedance, tap or magnetizing data don't convert exactly keep their RAW values in x.psse.orig too"
        ));
    }
    let records = doc.tables.iter().map(|t| t.records.len()).sum();
    (doc, StnExport { records, notes })
}

/// Add a record made from RAW, its id made from its fields.
fn push_new(doc: &mut Document, table: &str, mut rec: Record) -> Option<()> {
    rec.id = doc.default_id(table, &rec)?;
    doc.push(table, rec);
    Some(())
}

/// Impedance correction tables from revision 35 lines: the table number,
/// then (T, Re F, Im F) points, ended by 0, 0, 0.
fn impedance_tables(item: &Item) -> Option<Vec<Record>> {
    let lines = match item {
        Item::Lines(l) => l,
        Item::Rec(r) => r.lines.as_ref()?,
    };
    let mut tokens = Vec::new();
    for l in data_lines(lines) {
        tokens.extend(tokenize(l).ok()?);
    }
    let num = |s: &String| s.trim().parse::<f64>().ok();
    let mut out = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        let table: i64 = tokens[at].trim().parse().ok()?;
        at += 1;
        let mut points = Vec::new();
        loop {
            let p = tokens.get(at..at + 3)?;
            at += 3;
            let (t, re, im) = (num(&p[0])?, num(&p[1])?, num(&p[2])?);
            if t == 0.0 && re == 0.0 && im == 0.0 {
                break;
            }
            let mut point = vec![Value::Float(t), Value::Float(re)];
            if im != 0.0 {
                point.push(Value::Float(im));
            }
            points.push(Value::List(point));
        }
        out.push(
            Record::new(table.to_string())
                .with("number", Value::Int(table))
                .with("points", Value::List(points)),
        );
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Stannum → RAW 35.

pub(crate) struct ExCtx<'a> {
    pub doc: &'a Document,
    /// Bus id → number.
    pub numbers: HashMap<&'a str, i64>,
    /// Bus number → (area, zone, owner, base kV).
    pub buses: HashMap<i64, (i64, i64, i64, f64)>,
    pub sbase: f64,
}

impl ExCtx<'_> {
    /// The number of the bus a reference names.
    pub fn bus(&self, v: Option<&Value>) -> Result<i64, String> {
        let id = v
            .and_then(Value::as_str)
            .ok_or("a bus reference is missing")?;
        self.numbers
            .get(id)
            .copied()
            .ok_or_else(|| format!("no bus has the id {id}"))
    }

    pub fn f(&self, table: &str, r: &Record, field: &str) -> f64 {
        self.doc
            .value(table, r, field)
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }

    pub fn s<'b>(&'b self, table: &str, r: &'b Record, field: &str) -> &'b str {
        self.doc
            .value(table, r, field)
            .and_then(Value::as_str)
            .unwrap_or("")
    }
}

fn to_doc(doc: &Document) -> Result<Doc, String> {
    let case = doc.records("case").first();
    let get = |field: &str| {
        case.and_then(|c| doc.value("case", c, field)).or_else(|| {
            doc.schema
                .table("case")
                .and_then(|t| t.field(field))
                .and_then(|f| f.default.as_ref())
        })
    };
    let sbase = get("base_mva").and_then(Value::as_f64).unwrap_or(100.0);
    let freq = get("frequency_hz").and_then(Value::as_f64).unwrap_or(60.0);
    let x = |field: &str| {
        case.and_then(|c| c.get(field))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    let ident: Fields = vec![
        ("IC".into(), int(x("x.psse.ic") as i64)),
        ("SBASE".into(), num(sbase)),
        ("REV".into(), int(35)),
        ("XFRRAT".into(), int(x("x.psse.xfrrat") as i64)),
        ("NXFRAT".into(), int(x("x.psse.nxfrat") as i64)),
        ("BASFRQ".into(), num(freq)),
    ];
    let title = |field: &str| get(field).and_then(Value::as_str).unwrap_or("").to_string();
    let mut numbers = HashMap::new();
    let mut buses = HashMap::new();
    for r in doc.records("bus") {
        let n = r
            .int("number")
            .ok_or_else(|| format!("bus {} has no number", r.id))?;
        numbers.insert(r.id.as_str(), n);
        let soft = |field: &str| {
            doc.value("bus", r, field)
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(1)
        };
        buses.insert(
            n,
            (
                soft("area"),
                soft("zone"),
                soft("owner"),
                r.float("kv").unwrap_or(0.0),
            ),
        );
    }
    let ctx = ExCtx {
        doc,
        numbers,
        buses,
        sbase,
    };
    let mut raw = Doc {
        revision: 35,
        ident,
        titles: [title("name"), title("description")],
        sections: HashMap::new(),
        tail: Vec::new(),
    };
    for table in &doc.tables {
        for r in &table.records {
            let (section, item) = match table.name.as_str() {
                "transformer" | "transformer3" => {
                    ("TRANSFORMER", transformer::export(&ctx, &table.name, r)?)
                }
                "impedance_table" => ("IMPEDANCE CORRECTION", impedance_lines(r)),
                "x.psse.lines" => {
                    let section = r.str("section").unwrap_or_default().to_string();
                    let lines: Vec<String> = r
                        .get("text")
                        .and_then(Value::as_list)
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect();
                    if section == "SUBSTATION" {
                        raw.tail = lines;
                    } else {
                        raw.items_mut(&section).push(Item::Lines(lines));
                    }
                    continue;
                }
                name => match maps::section_of(name) {
                    Some(section) => (section, maps::export(&ctx, name, r)?),
                    None => continue,
                },
            };
            raw.items_mut(section).push(item);
        }
    }
    Ok(raw)
}

fn impedance_lines(r: &Record) -> Item {
    let n = r.int("number").unwrap_or(0);
    let mut triples: Vec<String> = r
        .get("points")
        .and_then(Value::as_list)
        .unwrap_or_default()
        .iter()
        .map(|p| {
            let p = p.as_list().unwrap_or_default();
            let g = |i: usize| {
                raw_export::token(
                    &num(p.get(i).and_then(Value::as_f64).unwrap_or(0.0)),
                    "number",
                )
            };
            format!("{},{},{}", g(0), g(1), g(2))
        })
        .collect();
    triples.push("0.0,0.0,0.0".into());
    let lines = triples
        .chunks(6)
        .enumerate()
        .map(|(i, chunk)| {
            let body = chunk.join(",");
            if i == 0 { format!("{n},{body}") } else { body }
        })
        .collect();
    Item::Lines(lines)
}

/// A record written as lines from fields, for layouts the RAW writer
/// doesn't have (VSC DC lines, multi-section lines with a few sections).
pub(crate) fn lines_item(lines: Vec<Vec<(RawValue, &str)>>) -> Item {
    let lines = lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|(v, kind)| raw_export::token(v, kind))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    Item::Rec(Rec {
        fields: Fields::new(),
        lines: Some(lines),
    })
}

pub(crate) fn rec_item(fields: Fields) -> Item {
    Item::Rec(Rec::new(fields))
}
