//! Edits made in the Records module, written into the case.
//!
//! A RAW edit replaces one field's text in the file, keeping the rest of the
//! line (other fields, spacing, comments) as it was. A MATPOWER edit changes
//! the power flow model, which is then written out as MATPOWER. Either way the
//! edited text is opened again, so every module sees the new values.

use crate::records::{ColumnKind, RecordSource, RecordTable, Records};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// One cell's new value, as typed.
#[derive(Deserialize, Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct RecordEdit {
    /// Table id, such as "BUS" or "gen".
    pub section: String,
    /// Row position in the table.
    pub row: usize,
    /// Column position in the table.
    pub column: usize,
    pub value: String,
}

/// The value as it will be stored: checked against the column's kind.
fn checked(table: &RecordTable, edit: &RecordEdit) -> Result<Json, String> {
    let column = table
        .columns
        .get(edit.column)
        .ok_or_else(|| format!("{} has no column {}.", table.label, edit.column))?;
    if !column.editable {
        return Err(format!(
            "{} in {} can't be edited.",
            column.name, table.label
        ));
    }
    let row = edit.row + 1;
    let text = edit.value.trim();
    match column.kind {
        ColumnKind::Text => {
            if edit.value.contains(['\n', '\r']) {
                return Err(format!(
                    "{} (row {row}): text can't span lines.",
                    column.name
                ));
            }
            Ok(Json::String(text.to_string()))
        }
        ColumnKind::Integer => text.parse::<i64>().map(Json::from).map_err(|_| {
            format!(
                "{} (row {row}): \"{text}\" isn't a whole number.",
                column.name
            )
        }),
        ColumnKind::Number => text
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .and_then(|v| {
                // Keep whole numbers whole, as the tables show them.
                if let Ok(i) = text.parse::<i64>() {
                    Some(Json::from(i))
                } else {
                    serde_json::Number::from_f64(v).map(Json::Number)
                }
            })
            .ok_or_else(|| format!("{} (row {row}): \"{text}\" isn't a number.", column.name)),
    }
}

/// Byte ranges of each comma-separated field of a RAW line: the whole field,
/// and its content (inside the quotes for a quoted field, trimmed otherwise).
/// Mirrors `diff::tokenizer::tokenize`.
fn field_spans(line: &str) -> Vec<(std::ops::Range<usize>, Option<char>)> {
    let mut spans = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut quoted: Option<(usize, usize, char)> = None;
    let mut end = line.len();
    let mut chars = line.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if let Some(q) = quote {
            if ch == q {
                if chars.peek().map(|(_, c)| *c) == Some(q) {
                    chars.next();
                } else {
                    quote = None;
                    if let Some((open, _, q)) = quoted {
                        quoted = Some((open, i, q));
                    }
                }
            }
            continue;
        }
        match ch {
            '/' => {
                end = i;
                break;
            }
            ',' => {
                spans.push(content(line, start..i, quoted.take()));
                start = i + 1;
            }
            '\'' | '"' if line[start..i].trim().is_empty() => {
                quote = Some(ch);
                quoted = Some((i + 1, i + 1, ch));
            }
            _ => {}
        }
    }
    let last = content(line, start..end, quoted);
    if !spans.is_empty() || last.1.is_some() || !line[start..end].trim().is_empty() {
        spans.push(last);
    }
    spans
}

fn content(
    line: &str,
    field: std::ops::Range<usize>,
    quoted: Option<(usize, usize, char)>,
) -> (std::ops::Range<usize>, Option<char>) {
    if let Some((open, close, q)) = quoted {
        return (open..close, Some(q));
    }
    let text = &line[field.clone()];
    let lead = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    let start = field.start + lead;
    (start..start + trimmed.len(), None)
}

/// The new text for a field, padded to the old width so columns stay aligned.
fn field_text(value: &Json, old_width: usize, quote: Option<char>, kind: ColumnKind) -> String {
    match (value, quote) {
        (Json::String(s), Some(q)) => {
            let escaped = s.replace(q, &format!("{q}{q}"));
            format!("{escaped:<old_width$}")
        }
        (Json::String(s), None) if kind == ColumnKind::Text => {
            let escaped = s.replace('\'', "''");
            format!("'{escaped}'")
        }
        (Json::String(s), None) => s.clone(),
        (v, _) => format!("{:>old_width$}", v.to_string()),
    }
}

/// A checked cell value, to copy into the cached tables once the edited
/// case has been opened.
pub type CellUpdate = (String, usize, usize, Json);

/// Apply edits to a RAW file's text. Returns the new text and the cells to
/// update in the cached tables ([`patch`]). Fails when any edit is invalid.
pub fn apply_raw(
    text: &str,
    records: &Records,
    edits: &[RecordEdit],
) -> Result<(String, Vec<CellUpdate>), String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut cell_updates = Vec::new();
    for edit in edits {
        let table = records.table(&edit.section)?;
        let value = checked(table, edit)?;
        let row = table
            .rows
            .get(edit.row)
            .ok_or_else(|| format!("{} has no row {}.", table.label, edit.row + 1))?;
        let layout = table
            .layouts
            .get(row.layout)
            .ok_or("The record's layout is unknown.")?;
        let ordinal = layout
            .iter()
            .position(|&c| c == edit.column)
            .ok_or_else(|| {
                format!(
                    "{} (row {}) has no {} field in the file to change.",
                    table.label,
                    edit.row + 1,
                    table.columns[edit.column].name
                )
            })?;
        // Walk the record's physical lines (skipping header comments, as the
        // parser does) to the line holding this field.
        let mut index = row
            .line
            .checked_sub(1)
            .ok_or("The record has no line in the file.")?;
        let mut skip = ordinal;
        loop {
            let line = lines
                .get(index)
                .ok_or("The record runs past the end of the file.")?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("@!") {
                index += 1;
                continue;
            }
            let spans = field_spans(line.strip_suffix('\r').unwrap_or(line));
            if skip < spans.len() {
                let (range, quote) = spans[skip].clone();
                let kind = table.columns[edit.column].kind;
                let new = field_text(&value, range.len(), quote, kind);
                lines[index].replace_range(range, &new);
                break;
            }
            skip -= spans.len();
            index += 1;
        }
        cell_updates.push((edit.section.clone(), edit.row, edit.column, value));
    }
    Ok((lines.join("\n"), cell_updates))
}

/// Copy edited values into the cached tables.
pub fn patch(records: &mut Records, updates: Vec<CellUpdate>) -> Result<(), String> {
    for (section, row, column, value) in updates {
        records.table_mut(&section)?.rows[row].cells[column] = value;
    }
    Ok(())
}

fn number(value: &Json) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

/// Apply edits to a MATPOWER case's model. Fails without changing anything
/// when any edit is invalid.
pub fn apply_model(
    case: &rs_power::Case,
    records: &Records,
    edits: &[RecordEdit],
) -> Result<rs_power::Case, String> {
    if records.source != RecordSource::Model {
        return Err("These records aren't the power flow model's.".into());
    }
    let mut case = case.clone();
    for edit in edits {
        let table = records.table(&edit.section)?;
        let value = checked(table, edit)?;
        let name = table.columns[edit.column].name.as_str();
        let v = number(&value);
        let row = edit.row;
        let missing = || format!("{} has no row {}.", table.label, row + 1);
        let extra = |names: &[&str], extra: &mut Vec<f64>| -> Result<(), String> {
            let at = name
                .rsplit(' ')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|_| !names.contains(&name))
                .ok_or_else(|| format!("{name} can't be edited."))?;
            if extra.len() < at {
                extra.resize(at, 0.0);
            }
            extra[at - 1] = v;
            Ok(())
        };
        match edit.section.as_str() {
            "bus" => {
                let b = case.buses.get_mut(row).ok_or_else(missing)?;
                match name {
                    "NAME" => {
                        let names = case.labels.entry("bus_name".into()).or_default();
                        if names.len() <= row {
                            names.resize(row + 1, String::new());
                        }
                        names[row] = value.as_str().unwrap_or_default().to_string();
                    }
                    "TYPE" => {
                        b.kind = match v as i64 {
                            1 => rs_power::BusType::Pq,
                            2 => rs_power::BusType::Pv,
                            3 => rs_power::BusType::Reference,
                            4 => rs_power::BusType::Isolated,
                            _ => {
                                return Err(format!(
                                    "TYPE (row {}) must be 1, 2, 3 or 4.",
                                    row + 1
                                ));
                            }
                        }
                    }
                    "PD" => b.pd = v,
                    "QD" => b.qd = v,
                    "GS" => b.gs = v,
                    "BS" => b.bs = v,
                    "BUS_AREA" => b.area = v as u32,
                    "VM" => b.vm = v,
                    "VA" => b.va = v,
                    "BASE_KV" => b.base_kv = v,
                    "ZONE" => b.zone = v as u32,
                    "VMAX" => b.vmax = v,
                    "VMIN" => b.vmin = v,
                    _ => extra(crate::records::BUS, &mut b.extra)?,
                }
            }
            "gen" => {
                let g = case.generators.get_mut(row).ok_or_else(missing)?;
                match name {
                    "PG" => g.pg = v,
                    "QG" => g.qg = v,
                    "QMAX" => g.qmax = v,
                    "QMIN" => g.qmin = v,
                    "VG" => g.vg = v,
                    "MBASE" => g.mbase = v,
                    "GEN_STATUS" => g.online = v > 0.0,
                    "PMAX" => g.pmax = v,
                    "PMIN" => g.pmin = v,
                    _ => {
                        // Columns after PMIN are stored in order in `extra`.
                        let at = table
                            .columns
                            .iter()
                            .position(|c| c.name == name)
                            .unwrap_or(0);
                        let at = at
                            .checked_sub(10)
                            .ok_or_else(|| format!("{name} can't be edited."))?;
                        if g.extra.len() <= at {
                            g.extra.resize(at + 1, 0.0);
                        }
                        g.extra[at] = v;
                    }
                }
            }
            "branch" => {
                let b = case.branches.get_mut(row).ok_or_else(missing)?;
                match name {
                    "BR_R" => b.set_impedance(v, b.x),
                    "BR_X" => b.set_impedance(b.r, v),
                    "BR_B" => b.b = v,
                    "RATE_A" => b.rate_a = v,
                    "RATE_B" => b.rate_b = v,
                    "RATE_C" => b.rate_c = v,
                    "TAP" => {
                        b.tap = v;
                        b.correct_impedance();
                    }
                    "SHIFT" => {
                        b.shift = v;
                        b.correct_impedance();
                    }
                    "BR_STATUS" => b.online = v > 0.0,
                    "ANGMIN" => b.angmin = v,
                    "ANGMAX" => b.angmax = v,
                    _ => {
                        let at = table
                            .columns
                            .iter()
                            .position(|c| c.name == name)
                            .unwrap_or(0);
                        let at = at
                            .checked_sub(13)
                            .ok_or_else(|| format!("{name} can't be edited."))?;
                        if b.extra.len() <= at {
                            b.extra.resize(at + 1, 0.0);
                        }
                        b.extra[at] = v;
                    }
                }
            }
            "dcline" | "gencost" => {
                let rows = if edit.section == "dcline" {
                    case.dcline.as_mut()
                } else {
                    case.gencost.as_mut()
                };
                let r = rows.and_then(|r| r.get_mut(row)).ok_or_else(missing)?;
                if r.len() <= edit.column {
                    r.resize(edit.column + 1, 0.0);
                }
                r[edit.column] = v;
            }
            other => return Err(format!("The case has no {other} records.")),
        }
    }
    Ok(case)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_fields_and_their_contents() {
        let line = "   101,'NORTH   ', 138.0000,1 / comment, 'x'";
        let spans = field_spans(line);
        let texts: Vec<&str> = spans.iter().map(|(r, _)| &line[r.clone()]).collect();
        assert_eq!(texts, ["101", "NORTH   ", "138.0000", "1"]);
        assert_eq!(spans[1].1, Some('\''));
        assert_eq!(field_spans(" 1, ,0").len(), 3);
        assert_eq!(
            field_spans("'O''B', 2")
                .iter()
                .map(|(r, _)| r.clone())
                .collect::<Vec<_>>(),
            [1..5, 8..9]
        );
    }

    #[test]
    fn new_text_keeps_the_width() {
        assert_eq!(
            field_text(&Json::from(69), 8, None, ColumnKind::Number),
            "      69"
        );
        assert_eq!(
            field_text(
                &Json::String("SOUTH".into()),
                8,
                Some('\''),
                ColumnKind::Text
            ),
            "SOUTH   "
        );
        assert_eq!(
            field_text(&Json::String("A'B".into()), 0, None, ColumnKind::Text),
            "'A''B'"
        );
    }
}
