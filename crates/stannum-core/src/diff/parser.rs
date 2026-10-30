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
    lines: Vec<String>,
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

    fn run(mut self) -> Result<DiffCase> {
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
        let mut ended = false;
        for section in &order {
            if ended {
                break;
            }
            self.section_mut(section);
            let layout = &schema["sections"][section]["layouts"][&rev];
            let evidence = layout["evidence"].as_str().unwrap_or_default();
            let verified = matches!(
                evidence,
                "records_verified" | "two_winding_records_verified" | "manual_layout"
            );
            let mut opened = None;
            loop {
                position = self.skip_headers(position);
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
                    position += 1;
                    break;
                }
                if structural.eq_ignore_ascii_case("q") {
                    if at_start && !self.lenient {
                        ended = true;
                        break;
                    }
                    if self.lenient {
                        self.diagnostics.push(Diagnostic {
                            section: section.clone(),
                            line_number: position + 1,
                            message: "File ends before this section".into(),
                            raw_lines: vec![],
                        });
                        ended = true;
                        break;
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
                        section: section.clone(),
                        line_number: start + 1,
                        message: "Unsupported populated section layout".into(),
                        raw_lines: self.lines[start..position].to_vec(),
                    });
                    continue;
                }
                let items = items.unwrap_or_default();
                let mut record_layout = layout;
                if section == "TRANSFORMER" {
                    if items.len() < 3 {
                        return self.fail(
                            section,
                            start,
                            "Missing transformer variant identifier K",
                        );
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
                            section: section.clone(),
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
                    let continuation = self.tokens(position, section)?;
                    if continuation.len() == 1
                        && matches!(continuation[0].as_str(), "0" | "Q" | "q")
                    {
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
                let key = effective_keys(section, &rev)
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
        position = self.skip_headers(position);
        let final_q = position < self.lines.len() && {
            let t = self.tokens(position, "FILE")?;
            t.len() == 1 && (t[0] == "Q" || t[0] == "q")
        };
        if !final_q {
            return self.fail("FILE", position, "Missing final Q terminator");
        }
        position = self.skip_headers(position + 1);
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
    Parser {
        name,
        lines,
        sections: Vec::new(),
        section_index: BTreeMap::new(),
        keys: BTreeMap::new(),
        diagnostics: Vec::new(),
        system_keys: HashSet::new(),
        lenient,
    }
    .run()
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
