//! Parse verified RAW layouts (revisions 33 and 35), keeping unsupported
//! content visible as diagnostics instead of guessing at it.
//!
//! Ported from case-diff's Python `parser.py`; behaviour is intended to match
//! it record for record.

use super::tokenizer::tokenize;
use super::value::{Number, Value};
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

pub const CASE_DETAILS: &str = "CASE DETAILS";

pub(crate) fn schema() -> &'static Json {
    static SCHEMA: OnceLock<Json> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(include_str!("../../schemas/raw.schema.json"))
            .expect("bundled RAW schema is valid JSON")
    })
}

pub type Key = Vec<Value>;

#[derive(Clone, Debug)]
pub struct Record {
    pub section: String,
    pub key: Key,
    /// Field values in layout order.
    pub fields: Vec<(String, Value)>,
    pub raw_lines: Vec<String>,
    pub line_number: usize,
}

impl Record {
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

#[derive(Clone, Debug, serde::Serialize, schemars::JsonSchema)]
pub struct Diagnostic {
    pub section: String,
    pub line_number: usize,
    pub message: String,
    pub raw_lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DiffCase {
    pub name: String,
    pub revision: i64,
    /// Sections in file order (`CASE DETAILS` first).
    pub sections: Vec<(String, Vec<Record>)>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, thiserror::Error, serde::Serialize, schemars::JsonSchema)]
#[error("{name}: {section}, line {line}: {reason}")]
pub struct RawParseError {
    pub name: String,
    pub section: String,
    pub line: usize,
    pub reason: String,
}

/// Strict Windows-1252: the five undefined bytes are errors, as in Python.
fn decode_cp1252(bytes: &[u8]) -> std::result::Result<String, String> {
    if let Some(pos) = bytes
        .iter()
        .position(|b| matches!(b, 0x81 | 0x8d | 0x8f | 0x90 | 0x9d))
    {
        return Err(format!(
            "'charmap' codec can't decode byte 0x{:02x} in position {pos}: character maps to <undefined>",
            bytes[pos]
        ));
    }
    Ok(encoding_rs::WINDOWS_1252
        .decode_without_bom_handling(bytes)
        .0
        .into_owned())
}

struct Parser<'a> {
    name: &'a str,
    lines: &'a [String],
    sections: Vec<(String, Vec<Record>)>,
    section_index: BTreeMap<String, usize>,
    keys: BTreeMap<String, HashSet<Key>>,
    diagnostics: Vec<Diagnostic>,
    system_keys: HashSet<String>,
    /// For viewing: keep records with a repeated key, and accept a file that
    /// ends (Q) before its last sections, noting both as diagnostics instead
    /// of failing. Comparison needs the strict reading.
    lenient: bool,
}

type Result<T> = std::result::Result<T, RawParseError>;

fn kind_of(definition: &Json) -> &str {
    definition["type"].as_str().unwrap_or("string")
}

impl<'a> Parser<'a> {
    fn fail<T>(&self, section: &str, index: usize, reason: impl Into<String>) -> Result<T> {
        Err(RawParseError {
            name: self.name.to_string(),
            section: section.to_string(),
            line: index + 1,
            reason: reason.into(),
        })
    }

    fn tokens(&self, index: usize, section: &str) -> Result<Vec<String>> {
        tokenize(&self.lines[index]).or_else(|e| self.fail(section, index, e))
    }

    fn value(&self, token: &str, kind: &str, section: &str, index: usize) -> Result<Value> {
        let parsed = match kind {
            "string" => return Ok(Value::Text(token.to_string())),
            "integer" => Number::parse_integer(token),
            _ => Number::parse_number(token),
        };
        match parsed {
            Some(n) => Ok(Value::Number(n)),
            None => self.fail(
                section,
                index,
                format!("Invalid {kind} token {}", super::value::python_repr(token)),
            ),
        }
    }

    fn decode(
        &self,
        items: &[String],
        layout: &Json,
        section: &str,
        index: usize,
    ) -> Result<Vec<(String, Value)>> {
        // Borrowed: cloning the field list copied every field's definition
        // for every record, nearly half the time of reading a large case.
        let definitions: &[Json] = layout["fields"].as_array().map_or(&[], Vec::as_slice);
        let valid = match layout.get("trailing_group") {
            Some(group) if !group.is_null() => {
                let start = group["start_index"].as_u64().unwrap_or(0) as i64;
                let width = group["width"].as_u64().unwrap_or(1) as i64;
                let min = group["minimum_groups"].as_i64().unwrap_or(0);
                let max = group["maximum_groups"].as_i64().unwrap_or(i64::MAX);
                let remaining = items.len() as i64 - start;
                remaining.rem_euclid(width) == 0
                    && remaining.div_euclid(width) >= min
                    && remaining.div_euclid(width) <= max
            }
            _ => items.len() == definitions.len(),
        };
        if !valid {
            return self.fail(
                section,
                index,
                format!("Invalid field/group count: got {}", items.len()),
            );
        }
        items
            .iter()
            .zip(definitions)
            .map(|(token, definition)| {
                let name = definition["name"].as_str().unwrap_or_default().to_string();
                Ok((
                    name,
                    self.value(token, kind_of(definition), section, index)?,
                ))
            })
            .collect()
    }

    fn section_mut(&mut self, section: &str) -> &mut Vec<Record> {
        let next = self.sections.len();
        let at = *self
            .section_index
            .entry(section.to_string())
            .or_insert(next);
        if at == next {
            self.sections.push((section.to_string(), Vec::new()));
        }
        &mut self.sections[at].1
    }

    fn add(
        &mut self,
        section: &str,
        key: Key,
        fields: Vec<(String, Value)>,
        raw: Vec<String>,
        index: usize,
    ) -> Result<()> {
        let normalized: Key = key.iter().map(Value::normalized).collect();
        let seen = self.keys.entry(section.to_string()).or_default();
        if !seen.insert(normalized.clone()) {
            if self.lenient {
                self.diagnostics.push(Diagnostic {
                    section: section.to_string(),
                    line_number: index + 1,
                    message: "Duplicate record key".into(),
                    raw_lines: raw.clone(),
                });
                self.section_mut(section).push(Record {
                    section: section.to_string(),
                    key: normalized,
                    fields,
                    raw_lines: raw,
                    line_number: index + 1,
                });
                return Ok(());
            }
            // A repeated key (SyntheticUSA names all nine of its DC lines
            // 'DC Line 1') is numbered in file order, "#2" on, and matched
            // with the other file's record of the same number.
            let numbered = (2..)
                .map(|n| {
                    let mut k = normalized.clone();
                    k.push(Value::Text(format!("#{n}")));
                    k
                })
                .find(|k| !seen.contains(k))
                .expect("a free number");
            seen.insert(numbered.clone());
            self.diagnostics.push(Diagnostic {
                section: section.to_string(),
                line_number: index + 1,
                message: REPEATED_KEY.into(),
                raw_lines: Vec::new(),
            });
            self.section_mut(section).push(Record {
                section: section.to_string(),
                key: numbered,
                fields,
                raw_lines: raw,
                line_number: index + 1,
            });
            return Ok(());
        }
        self.section_mut(section).push(Record {
            section: section.to_string(),
            key: normalized,
            fields,
            raw_lines: raw,
            line_number: index + 1,
        });
        Ok(())
    }

    fn skip_headers(&self, mut index: usize) -> usize {
        while index < self.lines.len() {
            let line = &self.lines[index];
            let left = line.trim_start();
            if line.trim().is_empty() || left.starts_with("@!") || left.starts_with('/') {
                index += 1;
            } else {
                break;
            }
        }
        index
    }

    /// Whether a line's content before any comment is `what` (any case).
    fn structural_is(&self, index: usize, what: &str) -> bool {
        self.lines[index]
            .split('/')
            .next()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case(what)
    }

    fn structural(&self, index: usize) -> String {
        self.lines[index]
            .split('/')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn system_record(&mut self, index: usize) -> Result<()> {
        let schema = schema();
        let items = self.tokens(index, "SYSTEM-WIDE")?;
        let Some(tag) = items.first().cloned() else {
            return self.fail("SYSTEM-WIDE", index, "Empty setting record");
        };
        if tag != "RATING" && !self.system_keys.insert(tag.clone()) {
            return self.fail("SYSTEM-WIDE", index, format!("Duplicate setting tag {tag}"));
        }
        let definition = &schema["system_wide"]["records"][&tag];
        if definition.is_null() {
            self.diagnostics.push(Diagnostic {
                section: "SYSTEM-WIDE".into(),
                line_number: index + 1,
                message: format!("Unsupported setting tag {tag}"),
                raw_lines: vec![self.lines[index].clone()],
            });
            return Ok(());
        }
        let mut fields = vec![("TAG".to_string(), Value::Text(tag.clone()))];
        let key: Key = if definition.get("fields").is_some() {
            fields.extend(self.decode(&items[1..], definition, "SYSTEM-WIDE", index)?);
            let mut k = vec![Value::Text(tag.clone())];
            for name in definition["key_fields"].as_array().into_iter().flatten() {
                let name = name.as_str().unwrap_or_default();
                if let Some((_, v)) = fields.iter().find(|(n, _)| n == name) {
                    k.push(v.clone());
                }
            }
            k
        } else {
            let positional: Vec<&str> = definition["positional_fields"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_str)
                .collect();
            if items.len() - 1 < positional.len() {
                return self.fail("SYSTEM-WIDE", index, "Missing positional setting fields");
            }
            for (name, token) in positional.iter().zip(&items[1..]) {
                fields.push((name.to_string(), Value::Text(token.clone())));
            }
            let mut unknown = false;
            for item in &items[1 + positional.len()..] {
                let Some((name, token)) = item.split_once('=') else {
                    return self.fail(
                        "SYSTEM-WIDE",
                        index,
                        format!(
                            "Malformed named parameter {}",
                            super::value::python_repr(item)
                        ),
                    );
                };
                let (name, token) = (name.trim(), token.trim());
                if fields.iter().any(|(n, _)| n == name) {
                    return self.fail(
                        "SYSTEM-WIDE",
                        index,
                        format!("Duplicate named parameter {name}"),
                    );
                }
                match definition["parameters"][name].as_str() {
                    None => {
                        unknown = true;
                        fields.push((name.to_string(), Value::Text(token.to_string())));
                    }
                    Some(kind) => {
                        let v = self.value(token, kind, "SYSTEM-WIDE", index)?;
                        fields.push((name.to_string(), v));
                    }
                }
            }
            if unknown {
                self.diagnostics.push(Diagnostic {
                    section: "SYSTEM-WIDE".into(),
                    line_number: index + 1,
                    message: "Unsupported setting parameter".into(),
                    raw_lines: vec![self.lines[index].clone()],
                });
                return Ok(());
            }
            vec![Value::Text(tag)]
        };
        let raw = vec![self.lines[index].clone()];
        self.add(CASE_DETAILS, key, fields, raw, index)
    }

    /// The case identification and title lines: where the sections start,
    /// the revision, and the sections in file order.
    fn header(&mut self) -> Result<(usize, i64, String, Vec<String>)> {
        let schema = schema();
        self.section_mut(CASE_DETAILS);
        let mut position = self.skip_headers(0);
        if position == self.lines.len() {
            return self.fail(CASE_DETAILS, position, "Missing case identification");
        }
        let id_layout = &schema["case_identification"]["lines"][0];
        let id_tokens = self.tokens(position, CASE_DETAILS)?;
        let identification = self.decode(&id_tokens, id_layout, CASE_DETAILS, position)?;
        let revision = identification
            .iter()
            .find(|(n, _)| n == "REV")
            .and_then(|(_, v)| match v {
                Value::Number(n) => n.to_string().parse::<i64>().ok(),
                Value::Text(_) => None,
            })
            .unwrap_or(-1);
        let supported = schema["supported_revisions"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|r| r.as_i64() == Some(revision));
        if !supported {
            return self.fail(
                CASE_DETAILS,
                position,
                format!("Unsupported revision {revision}"),
            );
        }
        let line = self.lines[position].clone();
        self.add(
            CASE_DETAILS,
            vec![Value::Text("CASE IDENTIFICATION".into())],
            identification,
            vec![line],
            position,
        )?;
        position += 1;
        for title in 1..=2 {
            if position >= self.lines.len() {
                return self.fail(CASE_DETAILS, position, "Missing physical title line");
            }
            let line = self.lines[position].clone();
            self.add(
                CASE_DETAILS,
                vec![Value::Text(format!("TITLE {title}"))],
                vec![("TITLE".into(), Value::Text(line.clone()))],
                vec![line],
                position,
            )?;
            position += 1;
        }
        let rev = revision.to_string();
        let order: Vec<String> = schema["section_order"][&rev]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect();
        Ok((position, revision, rev, order))
    }

    fn run(mut self) -> Result<DiffCase> {
        let (mut position, revision, rev, order) = self.header()?;
        let mut ended = false;
        for section in &order {
            if ended {
                break;
            }
            self.section_mut(section);
            (position, ended) = self.read_section(section, &rev, position, None)?;
        }
        self.finish(position, revision)
    }

    /// Check that the file ends with Q after `position`, and hand over what
    /// was read.
    fn finish(self, position: usize, revision: i64) -> Result<DiffCase> {
        let position = self.skip_headers(position);
        let final_q = position < self.lines.len() && {
            let t = self.tokens(position, "FILE")?;
            t.len() == 1 && (t[0] == "Q" || t[0] == "q")
        };
        if !final_q {
            return self.fail("FILE", position, "Missing final Q terminator");
        }
        let position = self.skip_headers(position + 1);
        if position < self.lines.len() {
            return self.fail("FILE", position, "Unexpected content after final Q");
        }
        Ok(DiffCase {
            name: self.name.to_string(),
            revision,
            sections: self.sections,
            diagnostics: self.diagnostics,
        })
    }

    /// Read one section's records from `position`: to its terminator, or
    /// only up to line `stop` when given (a part of a section read on its
    /// own, see [`parse_in_parts`]). Returns where reading stopped, and
    /// whether a Q ended the file there.
    fn read_section(
        &mut self,
        section: &str,
        rev: &str,
        mut position: usize,
        stop: Option<usize>,
    ) -> Result<(usize, bool)> {
        let schema = schema();
        let layout = &schema["sections"][section]["layouts"][rev];
        let evidence = layout["evidence"].as_str().unwrap_or_default();
        let verified = matches!(
            evidence,
            "records_verified" | "two_winding_records_verified" | "manual_layout"
        );
        let keys = effective_keys(section, rev);
        let mut opened = None;
        loop {
            position = self.skip_headers(position);
            if stop.is_some_and(|stop| position >= stop) {
                return Ok((position, false));
            }
            // Q where a section would start ends the file: PSS/E reads the
            // sections after it as empty (SyntheticUSA stops before
            // INDUCTION MACHINE).
            let at_start = *opened.get_or_insert(position) == position;
            if position >= self.lines.len() {
                return self.fail(
                    section,
                    position,
                    "Unexpected EOF before section terminator",
                );
            }
            let items = if section == "SYSTEM-WIDE" || verified {
                Some(self.tokens(position, section)?)
            } else {
                None
            };
            let structural = self.structural(position);
            if structural == "0" {
                return Ok((position + 1, false));
            }
            if structural.eq_ignore_ascii_case("q") {
                if at_start && !self.lenient {
                    return Ok((position, true));
                }
                if self.lenient {
                    self.diagnostics.push(Diagnostic {
                        section: section.to_string(),
                        line_number: position + 1,
                        message: "File ends before this section".into(),
                        raw_lines: vec![],
                    });
                    return Ok((position, true));
                }
                return self.fail(section, position, "Early Q before section terminator");
            }
            if section == "SYSTEM-WIDE" {
                self.system_record(position)?;
                position += 1;
                continue;
            }
            let start = position;
            if !verified {
                while position < self.lines.len() {
                    let s = self.structural(position);
                    if s == "0" || s.eq_ignore_ascii_case("q") {
                        break;
                    }
                    position += 1;
                }
                self.diagnostics.push(Diagnostic {
                    section: section.to_string(),
                    line_number: start + 1,
                    message: "Unsupported populated section layout".into(),
                    raw_lines: self.lines[start..position].to_vec(),
                });
                continue;
            }
            let mut items = items.unwrap_or_default();
            let mut record_layout = layout;
            if section == "TRANSFORMER" {
                if items.len() < 3 {
                    return self.fail(section, start, "Missing transformer variant identifier K");
                }
                let k = self.value(&items[2], "integer", section, start)?;
                record_layout = &layout["variants"][0];
                let zero = Value::Number(Number::parse_integer("0").unwrap());
                if k != zero {
                    // The three-winding layout is provisional: skip its
                    // physical envelope and never decode its fields.
                    let first_width = record_layout["lines"][0]["fields"]
                        .as_array()
                        .map_or(0, Vec::len);
                    position += 1;
                    for _ in 0..4 {
                        position = self.skip_headers(position);
                        if position >= self.lines.len() {
                            break;
                        }
                        let candidate = self.structural(position);
                        if candidate == "0" || candidate.eq_ignore_ascii_case("q") {
                            break;
                        }
                        let width = tokenize(&self.lines[position]).map_or(0, |t| t.len());
                        if width == first_width {
                            break;
                        }
                        position += 1;
                    }
                    self.diagnostics.push(Diagnostic {
                        section: section.to_string(),
                        line_number: start + 1,
                        message: "Unsupported three-winding transformer".into(),
                        raw_lines: self.lines[start..position.min(self.lines.len())].to_vec(),
                    });
                    continue;
                }
            }
            let mut fields = Vec::new();
            let mut raw = Vec::new();
            let line_layouts: &[Json] =
                record_layout["lines"].as_array().map_or(&[], Vec::as_slice);
            for (offset, line_layout) in line_layouts.iter().enumerate() {
                if offset > 0 {
                    position = self.skip_headers(position);
                }
                if position >= self.lines.len() {
                    return self.fail(section, position, "Missing continuation line: EOF");
                }
                // The first line's tokens were read above.
                let continuation = if offset == 0 {
                    std::mem::take(&mut items)
                } else {
                    self.tokens(position, section)?
                };
                if continuation.len() == 1 && matches!(continuation[0].as_str(), "0" | "Q" | "q") {
                    return self.fail(
                        section,
                        position,
                        "Missing continuation line before terminator",
                    );
                }
                fields.extend(self.decode(&continuation, line_layout, section, position)?);
                raw.push(self.lines[position].clone());
                position += 1;
            }
            let key = keys
                .iter()
                .map(|name| {
                    fields
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, v)| v.clone())
                        .unwrap_or(Value::Text(String::new()))
                })
                .collect();
            self.add(section, key, fields, raw, start)?;
        }
    }
}

fn effective_keys(section: &str, revision: &str) -> Vec<String> {
    let definition = &schema()["sections"][section];
    let pick = |v: &Json| -> Option<Vec<String>> {
        v.as_array().map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        })
    };
    pick(&definition["layouts"][revision]["key_fields"])
        .or_else(|| pick(&definition["key_fields_by_revision"][revision]))
        .or_else(|| pick(&definition["key_fields"]))
        .unwrap_or_default()
}

/// Diagnostic for a record whose key repeats an earlier one in a comparison.
pub const REPEATED_KEY: &str =
    "Repeats an earlier record's key; numbered in file order and matched by that number";

/// Parse a RAW file's bytes. `name` is used only in error messages.
pub fn parse_bytes(name: &str, bytes: &[u8]) -> Result<DiffCase> {
    parse(name, bytes, false)
}

/// Like [`parse_bytes`], but records whose key repeats an earlier one are
/// kept (with a diagnostic) rather than failing the whole file.
pub fn parse_bytes_for_viewing(name: &str, bytes: &[u8]) -> Result<DiffCase> {
    parse(name, bytes, true)
}

fn parse(name: &str, bytes: &[u8], lenient: bool) -> Result<DiffCase> {
    let text = decode_cp1252(bytes).map_err(|reason| RawParseError {
        name: name.to_string(),
        section: CASE_DETAILS.into(),
        line: 1,
        reason,
    })?;
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    let fresh = || Parser {
        name,
        lines: &lines,
        sections: Vec::new(),
        section_index: BTreeMap::new(),
        keys: BTreeMap::new(),
        diagnostics: Vec::new(),
        system_keys: HashSet::new(),
        lenient,
    };
    if lines.len() >= PARALLEL_FROM_LINES {
        let start = std::time::Instant::now();
        if let Some(case) = parse_in_parts(&fresh, PART_RECORDS) {
            log::debug!(
                "Read {name} in parts ({:.0} ms)",
                start.elapsed().as_secs_f64() * 1e3
            );
            return Ok(case);
        }
        log::debug!("{name} can't be read in parts; reading it in one go");
    }
    fresh().run()
}

/// Files with at least this many lines are read in parts on several
/// threads (about 5,000 buses).
const PARALLEL_FROM_LINES: usize = 20_000;

/// Records per part when a section is split.
const PART_RECORDS: usize = 5_000;

/// Read a large file's sections, and parts of its large sections, on
/// several threads. The parts are found first by their terminator lines,
/// then each is read by the same code as a whole-file read, and the
/// results are put together in file order. Anything unusual (an error, a
/// repeated key, a Q inside a section, a part ending somewhere else than
/// expected) gives None, and the file is read again in one go so that the
/// result, errors and diagnostics included, is exactly the one-thread one.
fn parse_in_parts<'a>(
    fresh: &(dyn Fn() -> Parser<'a> + Sync),
    part_records: usize,
) -> Option<DiffCase> {
    let mut main = fresh();
    let (mut position, revision, rev, order) = main.header().ok()?;
    let lines = main.lines;
    let schema = schema();

    // Where each section starts and ends; one-line records may be split.
    struct Part<'s> {
        section: &'s str,
        start: usize,
        /// Where the next part of the same section starts.
        stop: Option<usize>,
        /// Where reading must end: the next part's start, or the line
        /// after the section's terminator.
        end: usize,
    }
    let mut parts: Vec<Part> = Vec::new();
    let mut sections: Vec<&str> = Vec::new();
    let mut ended_at = None;
    for section in &order {
        sections.push(section);
        let start = main.skip_headers(position);
        if start >= lines.len() {
            return None;
        }
        if main.structural_is(start, "q") {
            ended_at = Some((section.as_str(), start));
            break;
        }
        let layout = &schema["sections"][section.as_str()]["layouts"][&rev];
        let one_line = section != "SYSTEM-WIDE"
            && section != "TRANSFORMER"
            && layout["lines"].as_array().is_some_and(|l| l.len() == 1)
            && matches!(
                layout["evidence"].as_str(),
                Some("records_verified" | "two_winding_records_verified" | "manual_layout")
            );
        // Every line up to the terminator.
        let mut records = Vec::new();
        let mut p = start;
        let terminator = loop {
            p = main.skip_headers(p);
            if p >= lines.len() || main.structural_is(p, "q") {
                return None;
            }
            if main.structural_is(p, "0") {
                break p;
            }
            if one_line {
                records.push(p);
            }
            p += 1;
        };
        let cuts: Vec<usize> = records
            .iter()
            .step_by(part_records)
            .skip(1)
            .copied()
            .collect();
        let starts: Vec<usize> = std::iter::once(start).chain(cuts.iter().copied()).collect();
        for (k, &from) in starts.iter().enumerate() {
            let stop = starts.get(k + 1).copied();
            parts.push(Part {
                section,
                start: from,
                stop,
                end: stop.unwrap_or(terminator + 1),
            });
        }
        position = terminator + 1;
    }

    // Read the parts, each on its own parser, a few threads at a time.
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let read: Vec<Vec<(usize, Option<Parser>)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.min(parts.len()).max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(part) = parts.get(k) else { break };
                        let mut parser = fresh();
                        parser.section_mut(part.section);
                        let ok =
                            match parser.read_section(part.section, &rev, part.start, part.stop) {
                                Ok((at, false)) => at == part.end,
                                _ => false,
                            };
                        out.push((k, ok.then_some(parser)));
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    });
    let mut done: Vec<Option<Parser>> = (0..parts.len()).map(|_| None).collect();
    for (k, parser) in read.into_iter().flatten() {
        done[k] = Some(parser?);
    }
    if done.iter().any(Option::is_none) {
        return None;
    }

    // Put them together in file order, as a one-thread read would have.
    let mut done = done.into_iter().flatten();
    for (k, section) in sections.iter().enumerate() {
        main.section_mut(section);
        if ended_at.is_some() && k == sections.len() - 1 {
            break;
        }
        // Keys only repeat within a section. A section read whole was read
        // exactly as in one go, repeated keys and all; a split one must not
        // repeat a key across its parts.
        let count = parts.iter().filter(|p| p.section == *section).count();
        for _ in 0..count {
            let part = done.next()?;
            if count > 1
                && part
                    .diagnostics
                    .iter()
                    .any(|d| d.message == "Duplicate record key" || d.message == REPEATED_KEY)
            {
                return None;
            }
            for (name, records) in part.sections {
                for record in records {
                    if count > 1
                        && !main
                            .keys
                            .entry(name.clone())
                            .or_default()
                            .insert(record.key.clone())
                    {
                        return None;
                    }
                    main.section_mut(&name).push(record);
                }
            }
            main.diagnostics.extend(part.diagnostics);
            // Settings are unique across the whole file too.
            for tag in part.system_keys {
                if !main.system_keys.insert(tag) {
                    return None;
                }
            }
        }
    }
    if let Some((section, at)) = ended_at {
        if main.lenient {
            main.diagnostics.push(Diagnostic {
                section: section.to_string(),
                line_number: at + 1,
                message: "File ends before this section".into(),
                raw_lines: vec![],
            });
        }
        position = at;
    }
    main.finish(position, revision).ok()
}

/// Read and parse a RAW file from disk.
pub fn parse_path(path: &std::path::Path) -> Result<DiffCase> {
    let name = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|e| RawParseError {
        name: name.clone(),
        section: CASE_DETAILS.into(),
        line: 1,
        reason: e.to_string(),
    })?;
    // A Stannum case file compares as the RAW 35 text it converts to.
    let bytes = crate::stn::raw_bytes_of(&name, bytes).map_err(|reason| RawParseError {
        name: name.clone(),
        section: CASE_DETAILS.into(),
        line: 1,
        reason,
    })?;
    parse_bytes(&name, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read `bytes` in one go and in parts of `part_records` records; the
    /// parts must give exactly the same case, or nothing.
    fn same_in_parts(name: &str, bytes: &[u8], lenient: bool, part_records: usize) -> bool {
        let text = decode_cp1252(bytes).unwrap();
        let mut lines: Vec<String> = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
            .collect();
        if text.ends_with('\n') {
            lines.pop();
        }
        let fresh = || Parser {
            name,
            lines: &lines,
            sections: Vec::new(),
            section_index: BTreeMap::new(),
            keys: BTreeMap::new(),
            diagnostics: Vec::new(),
            system_keys: HashSet::new(),
            lenient,
        };
        let whole = fresh().run();
        match parse_in_parts(&fresh, part_records) {
            Some(parts) => {
                assert_eq!(
                    format!("{parts:?}"),
                    format!(
                        "{:?}",
                        whole.expect("parts read a file the whole read refused")
                    ),
                    "{name}"
                );
                true
            }
            None => false,
        }
    }

    fn fixtures() -> Vec<std::path::PathBuf> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        for dir in ["tests/fixtures", "../rs-power/tests/fixtures/raw"] {
            for entry in std::fs::read_dir(root.join(dir)).unwrap().flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("raw"))
                {
                    files.push(path);
                }
            }
        }
        files.sort();
        files
    }

    #[test]
    fn reading_in_parts_matches_reading_in_one_go() {
        let mut split = 0;
        for path in fixtures() {
            let bytes = std::fs::read(&path).unwrap();
            let name = path.display().to_string();
            for lenient in [false, true] {
                for part_records in [1, 2, 7] {
                    if same_in_parts(&name, &bytes, lenient, part_records) {
                        split += 1;
                    }
                }
            }
        }
        // Most fixtures are read in parts; the rest fall back.
        assert!(split > 20, "{split}");
    }

    #[test]
    fn a_repeated_key_falls_back_to_one_go() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/five_bus.raw");
        let text = std::fs::read_to_string(path).unwrap();
        // Repeat the first bus line.
        let mut lines: Vec<&str> = text.lines().collect();
        let first_bus = 3 + lines[3..]
            .iter()
            .position(|l| !l.trim_start().starts_with('@') && !l.trim().is_empty())
            .unwrap();
        lines.insert(first_bus + 1, lines[first_bus]);
        let bytes = lines.join("\n");
        assert!(!same_in_parts("dup.raw", bytes.as_bytes(), true, 1));
        assert!(!same_in_parts("dup.raw", bytes.as_bytes(), false, 1));
    }

    /// `STANNUM_LARGE_CASES=<folder> cargo test --release -p stannum-core
    /// parts_match_on_large -- --ignored`
    #[test]
    #[ignore = "needs the large RAW cases"]
    fn parts_match_on_large_cases() {
        let dir = std::path::PathBuf::from(std::env::var("STANNUM_LARGE_CASES").unwrap());
        // (The East-West case is revision 34, which this reader doesn't
        // take.)
        for file in ["SyntheticUSA.RAW"] {
            let bytes = std::fs::read(dir.join(file)).unwrap();
            for lenient in [false, true] {
                assert!(same_in_parts(file, &bytes, lenient, PART_RECORDS), "{file}");
            }
        }
    }
}
