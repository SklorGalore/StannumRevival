//! The open case's records as tables, one per record type, for the Records
//! module.
//!
//! RAW revisions 33 and 35 are read record by record with case-diff's parser,
//! so every field keeps its RAW name and its exact value. Other files (MATPOWER,
//! and RAW revisions that parser doesn't read) are shown as the power flow
//! model's tables, in MATPOWER's column order.

use crate::diff::parser::{DiffCase, Record};
use crate::diff::value::Value;
use crate::error::Error;
use crate::power::PowerCase;
use serde::Serialize;
use serde_json::Value as Json;

/// Where the tables come from.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordSource {
    /// The RAW file's own records.
    Raw,
    /// The power flow model (MATPOWER columns).
    Model,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    Integer,
    Number,
    Text,
}

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct RecordColumn {
    pub name: String,
    pub kind: ColumnKind,
    /// Whether its cells can be edited (key fields and case details can't).
    pub editable: bool,
}

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct RecordRow {
    /// Line in the file where the record starts (0 when not from the file).
    pub line: usize,
    /// Shared element id (see `crate::elements`), when the record is one.
    pub element: Option<String>,
    /// One value per column: a number, text, or null when absent.
    #[schemars(extend("items" = { "type": ["string", "number", "null"] }))]
    pub cells: Vec<Json>,
    /// Which of the table's layouts the record has (RAW only).
    #[serde(skip)]
    pub layout: usize,
}

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct RecordTable {
    /// Stable id, such as "BUS" for RAW or "bus" for the model.
    pub id: String,
    pub label: String,
    pub columns: Vec<RecordColumn>,
    /// Sent as columns, like every big table.
    pub rows: crate::table::Table<RecordRow>,
    /// Records in the file that couldn't be read into this table.
    pub skipped: usize,
    /// Why some records are missing, or how to read the table.
    pub note: Option<String>,
    /// For each layout, the column of each of its fields in record order
    /// (RAW only), so an edit can find its token in the file.
    #[serde(skip)]
    pub layouts: Vec<Vec<usize>>,
}

/// A page of one record type: its columns, and the rows asked for.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct RecordPage {
    pub id: String,
    pub label: String,
    pub columns: Vec<RecordColumn>,
    pub skipped: usize,
    pub note: Option<String>,
    pub page: crate::paging::Page<RecordRow>,
}

/// One record type in the list, without its rows.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct SectionInfo {
    pub id: String,
    pub label: String,
    pub count: usize,
    pub skipped: usize,
}

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct RecordBook {
    pub source: RecordSource,
    /// RAW revision, when read from the file.
    pub revision: Option<i64>,
    /// Shown above every table, such as why the model is shown instead.
    pub note: Option<String>,
    /// Whether cells can be edited at all (RAW 33/35 and MATPOWER files).
    pub editable: bool,
    pub sections: Vec<SectionInfo>,
}

#[derive(Clone)]
pub struct Records {
    pub source: RecordSource,
    pub revision: Option<i64>,
    pub note: Option<String>,
    pub editable: bool,
    pub tables: Vec<RecordTable>,
}

impl Records {
    pub fn book(&self) -> RecordBook {
        RecordBook {
            source: self.source,
            revision: self.revision,
            note: self.note.clone(),
            editable: self.editable,
            sections: self
                .tables
                .iter()
                .map(|t| SectionInfo {
                    id: t.id.clone(),
                    label: t.label.clone(),
                    count: t.rows.len(),
                    skipped: t.skipped,
                })
                .collect(),
        }
    }

    pub fn table(&self, id: &str) -> Result<&RecordTable, Error> {
        self.tables
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| Error::not_found(format!("The case has no {id} records.")))
    }

    pub fn table_mut(&mut self, id: &str) -> Result<&mut RecordTable, Error> {
        self.tables
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| Error::not_found(format!("The case has no {id} records.")))
    }
}

fn raw_label(section: &str) -> String {
    match section {
        "CASE DETAILS" => "Case details",
        "BUS" => "Buses",
        "LOAD" => "Loads",
        "FIXED SHUNT" => "Fixed shunts",
        "GENERATOR" => "Generators",
        "BRANCH" => "Branches",
        "SYSTEM SWITCHING DEVICE" => "Switching devices",
        "TRANSFORMER" => "Transformers",
        "AREA" => "Areas",
        "TWO-TERMINAL DC" => "Two-terminal DC",
        "VSC DC LINE" => "VSC DC lines",
        "IMPEDANCE CORRECTION" => "Impedance corrections",
        "MULTI-TERMINAL DC" => "Multi-terminal DC",
        "MULTI-SECTION LINE" => "Multi-section lines",
        "ZONE" => "Zones",
        "INTER-AREA TRANSFER" => "Inter-area transfers",
        "OWNER" => "Owners",
        "FACTS DEVICE" => "FACTS devices",
        "SWITCHED SHUNT" => "Switched shunts",
        "GEN DEVICE" => "GNE devices",
        "INDUCTION MACHINE" => "Induction machines",
        other => return other.to_string(),
    }
    .to_string()
}

fn number_cell(text: &str) -> Json {
    if let Ok(i) = text.parse::<i64>() {
        return Json::from(i);
    }
    text.parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(Json::Null, Json::Number)
}

/// Field names of a record, with repeats told apart ("IBUS", "IBUS 2").
fn field_names(record: &Record) -> Vec<String> {
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();
    record
        .fields
        .iter()
        .map(|(name, _)| {
            let n = seen.entry(name.as_str()).or_default();
            *n += 1;
            if *n == 1 {
                name.clone()
            } else {
                format!("{name} {n}")
            }
        })
        .collect()
}

/// The element a RAW record is (see `crate::elements::KINDS`).
fn raw_element(section: &str, record: &Record) -> Option<String> {
    crate::diff::record_element(section, &record.key, Some(record))
}

/// "2,400 records".
fn plural(n: usize, one: &str, many: &str) -> String {
    let digits = n.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{grouped} {}", if n == 1 { one } else { many })
}

/// Names and types of each physical line of a layout.
type Layout = Vec<Vec<(String, String)>>;

fn layout_lines(lines: &Json) -> Option<Layout> {
    let lines = lines.as_array()?;
    Some(
        lines
            .iter()
            .map(|line| {
                line["fields"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|f| {
                                (
                                    f["name"].as_str().unwrap_or_default().to_string(),
                                    f["type"].as_str().unwrap_or("string").to_string(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect(),
    )
    .filter(|l: &Layout| !l.is_empty())
}

/// One record from its physical lines, named by `layout` line by line.
/// Tokens beyond the layout are numbered ("FIELD 22").
fn decode(
    section: &str,
    lines: &[(usize, &String)],
    layout: &[Vec<(String, String)>],
    key_names: &[String],
) -> Option<Record> {
    let mut fields: Vec<(String, Value)> = Vec::new();
    for (i, (_, line)) in lines.iter().enumerate() {
        let names = layout.get(i).map(Vec::as_slice).unwrap_or(&[]);
        for (j, token) in crate::diff::tokenizer::tokenize(line)
            .ok()?
            .iter()
            .enumerate()
        {
            let (name, kind) = names
                .get(j)
                .cloned()
                .unwrap_or_else(|| (format!("FIELD {}", fields.len() + 1), "string".into()));
            let number = match kind.as_str() {
                "integer" => crate::diff::value::Number::parse_integer(token.trim()),
                "number" => crate::diff::value::Number::parse_number(token.trim()),
                _ => None,
            };
            fields.push((
                name,
                number.map_or_else(|| Value::Text(token.clone()), Value::Number),
            ));
        }
    }
    let key = key_names
        .iter()
        .map(|k| {
            fields
                .iter()
                .find(|(n, _)| n == k)
                .map_or(Value::Text(String::new()), |(_, v)| v.normalized())
        })
        .collect();
    Some(Record {
        section: section.to_string(),
        key,
        fields,
        raw_lines: lines.iter().map(|(_, l)| (*l).clone()).collect(),
        line_number: lines.first()?.0,
    })
}

/// The records the strict parser set aside (layouts case-diff hasn't
/// verified, three-winding transformers), read for viewing with the layout
/// the schema has: this revision's, else revision 35's. Also returns notes on
/// how the section was read, and how many records stayed unread.
fn recover(case: &DiffCase, section: &str) -> (Vec<Record>, Vec<String>, usize) {
    let schema = crate::diff::parser::schema();
    let definition = &schema["sections"][section];
    let rev = case.revision.to_string();
    let key_names: Vec<String> = definition["key_fields"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut records = Vec::new();
    let mut notes = Vec::new();
    let mut unread = 0;
    let mut borrowed = None;
    let mut numbered = false;
    let mut three_winding = 0;
    for d in case.diagnostics.iter().filter(|d| d.section == section) {
        let lines: Vec<(usize, &String)> = d
            .raw_lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with("@!"))
            .map(|(i, l)| (d.line_number + i, l))
            .collect();
        match d.message.as_str() {
            "Unsupported three-winding transformer" => {
                let layout = layout_lines(&definition["layouts"][&rev]["variants"][1]["lines"]);
                match layout.and_then(|l| decode(section, &lines, &l, &key_names)) {
                    Some(r) => {
                        three_winding += 1;
                        records.push(r);
                    }
                    None => unread += 1,
                }
            }
            "Unsupported populated section layout" => {
                // One record per line, when a one-line layout is known.
                let own =
                    layout_lines(&definition["layouts"][&rev]["lines"]).filter(|l| l.len() == 1);
                let layout = own.clone().or_else(|| {
                    layout_lines(&definition["layouts"]["35"]["lines"]).filter(|l| l.len() == 1)
                });
                match &layout {
                    Some(_) if own.is_none() => borrowed = Some("35"),
                    Some(_) => borrowed = borrowed.or(Some("")),
                    None => numbered = true,
                }
                let layout = layout.unwrap_or_default();
                for line in &lines {
                    match decode(section, std::slice::from_ref(line), &layout, &key_names) {
                        Some(r) => records.push(r),
                        None => unread += 1,
                    }
                }
            }
            _ => {}
        }
    }
    if three_winding > 0 {
        notes.push(format!(
            "{} {} read with a layout case-diff hasn't verified against example records.",
            plural(
                three_winding,
                "three-winding transformer",
                "three-winding transformers"
            ),
            if three_winding == 1 { "is" } else { "are" }
        ));
    }
    match borrowed {
        Some("35") => notes.push(format!(
            "Field names come from the RAW revision 35 layout; check them against the revision {rev} manual."
        )),
        Some(_) => notes.push(format!(
            "Field names come from the revision {rev} layout header; case-diff hasn't verified them against example records."
        )),
        None => {}
    }
    if numbered {
        notes.push(format!(
            "No layout is known for this section in RAW revision {rev}, so each line is shown with numbered fields."
        ));
    }
    if unread > 0 {
        notes.push(format!(
            "{} couldn't be split into fields and {} left out.",
            plural(unread, "record", "records"),
            if unread == 1 { "is" } else { "are" }
        ));
    }
    let mut duplicates = 0;
    let mut ended = false;
    for d in case.diagnostics.iter().filter(|d| d.section == section) {
        match d.message.as_str() {
            "Duplicate record key" => duplicates += 1,
            "File ends before this section" => ended = true,
            _ => {}
        }
    }
    if duplicates > 0 {
        notes.push(format!(
            "{} repeat the key of an earlier record.",
            plural(duplicates, "record", "records")
        ));
    }
    if ended {
        notes.push("The file ends (Q) before this section.".into());
    }
    (records, notes, unread)
}

fn raw_table(case: &DiffCase, section: &str, parsed: &[Record]) -> RecordTable {
    let (recovered, notes, skipped) = recover(case, section);
    let mut records: Vec<&Record> = parsed.iter().chain(&recovered).collect();
    records.sort_by_key(|r| r.line_number);
    let note = (!notes.is_empty()).then(|| notes.join(" "));
    let mut columns: Vec<RecordColumn> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = Default::default();
    let mut rows = Vec::with_capacity(records.len());
    // Records of a section mostly share one layout: the column of each field
    // is worked out once per layout, not once per record.
    let mut last: Option<(&Record, usize)> = None;
    let mut layouts: Vec<Vec<usize>> = Vec::new();
    let keys: Vec<String> = key_fields(section);
    let editable_section = section != crate::diff::parser::CASE_DETAILS;
    for record in records {
        let same = last.as_ref().is_some_and(|(prev, _)| {
            prev.fields.len() == record.fields.len()
                && prev
                    .fields
                    .iter()
                    .zip(&record.fields)
                    .all(|(a, b)| a.0 == b.0)
        });
        if !same {
            let at: Vec<usize> = field_names(record)
                .into_iter()
                .map(|name| match index.get(&name) {
                    Some(&i) => i,
                    None => {
                        index.insert(name.clone(), columns.len());
                        let editable = editable_section && !keys.contains(&name);
                        columns.push(RecordColumn {
                            name,
                            kind: ColumnKind::Integer,
                            editable,
                        });
                        columns.len() - 1
                    }
                })
                .collect();
            let id = layouts.iter().position(|l| *l == at).unwrap_or_else(|| {
                layouts.push(at);
                layouts.len() - 1
            });
            last = Some((record, id));
        }
        let layout = last.as_ref().expect("set above").1;
        let at = &layouts[layout];
        let mut cells = vec![Json::Null; columns.len()];
        for (&col, (_, value)) in at.iter().zip(&record.fields) {
            cells[col] = match value {
                Value::Text(t) => {
                    columns[col].kind = ColumnKind::Text;
                    Json::String(t.trim().to_string())
                }
                Value::Number(n) => {
                    let text = n.to_string();
                    // A decimal anywhere makes an integer column a number column.
                    if columns[col].kind == ColumnKind::Integer && text.contains(['.', 'E', 'e']) {
                        columns[col].kind = ColumnKind::Number;
                    }
                    number_cell(&text)
                }
            };
        }
        rows.push(RecordRow {
            line: record.line_number,
            element: raw_element(section, record),
            cells,
            layout,
        });
    }
    for row in &mut rows {
        row.cells.resize(columns.len(), Json::Null);
    }
    RecordTable {
        id: section.to_string(),
        label: raw_label(section),
        columns,
        rows: rows.into(),
        skipped,
        note,
        layouts,
    }
}

/// A section's key fields, which identify its records and can't be edited.
fn key_fields(section: &str) -> Vec<String> {
    let definition = &crate::diff::parser::schema()["sections"][section];
    let mut keys: Vec<String> = definition["key_fields"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if let Some(by_rev) = definition["key_fields_by_revision"].as_object() {
        for list in by_rev.values().filter_map(Json::as_array) {
            keys.extend(list.iter().filter_map(|k| k.as_str().map(str::to_string)));
        }
    }
    keys
}

/// Every section of a RAW file read by case-diff's parser, in file order,
/// with case details last.
pub fn from_raw(case: &DiffCase) -> Records {
    // SYSTEM-WIDE settings are listed with the case details.
    let sections: Vec<_> = case
        .sections
        .iter()
        .filter(|(name, _)| name != crate::diff::parser::CASE_DETAILS && name != "SYSTEM-WIDE")
        .collect();
    // Tables are built on threads of their own: on an 80,000-bus case this
    // takes the time of the largest table rather than of all of them.
    let mut tables: Vec<RecordTable> = std::thread::scope(|scope| {
        let handles: Vec<_> = sections
            .iter()
            .map(|(name, records)| scope.spawn(move || raw_table(case, name, records)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("building a record table"))
            .collect()
    });
    if let Some((name, records)) = case
        .sections
        .iter()
        .find(|(name, _)| name == crate::diff::parser::CASE_DETAILS)
    {
        tables.push(raw_table(case, name, records));
    }
    Records {
        source: RecordSource::Raw,
        revision: Some(case.revision),
        note: None,
        editable: true,
        tables,
    }
}

/// A model table: numeric columns by MATPOWER name, then any extra columns,
/// with an optional NAME column after the first.
fn model_table(
    id: &str,
    label: &str,
    names: &[&str],
    integer: &[&str],
    extra: &str,
    rows: Vec<(Vec<f64>, Option<String>)>,
    row_names: Option<&Vec<String>>,
) -> RecordTable {
    let width = rows
        .iter()
        .map(|(v, _)| v.len())
        .max()
        .unwrap_or(0)
        .max(names.len());
    let mut columns: Vec<RecordColumn> = (0..width)
        .map(|i| {
            let name = names.get(i).map_or_else(
                || format!("{extra} {}", i + 1 - names.len()),
                |n| n.to_string(),
            );
            RecordColumn {
                kind: if integer.contains(&name.as_str()) {
                    ColumnKind::Integer
                } else {
                    ColumnKind::Number
                },
                // Bus numbers tie the tables together, so they stay as read.
                editable: !MODEL_KEYS.contains(&name.as_str()),
                name,
            }
        })
        .collect();
    if row_names.is_some() {
        columns.insert(
            1,
            RecordColumn {
                name: "NAME".into(),
                kind: ColumnKind::Text,
                editable: true,
            },
        );
    }
    let rows = rows
        .into_iter()
        .enumerate()
        .map(|(i, (values, element))| {
            let mut cells: Vec<Json> = (0..width)
                .map(|c| {
                    values
                        .get(c)
                        .and_then(|v| serde_json::Number::from_f64(*v))
                        .map_or(Json::Null, Json::Number)
                })
                .collect();
            if let Some(n) = row_names {
                let name = n.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
                cells.insert(1, Json::String(name));
            }
            RecordRow {
                line: 0,
                element,
                cells,
                layout: 0,
            }
        })
        .collect();
    RecordTable {
        id: id.to_string(),
        label: label.to_string(),
        columns,
        rows,
        skipped: 0,
        note: None,
        layouts: Vec::new(),
    }
}

/// Model columns holding bus numbers, which stay as read.
const MODEL_KEYS: &[&str] = &["BUS_I", "GEN_BUS", "F_BUS", "T_BUS"];

pub(crate) const BUS: &[&str] = &[
    "BUS_I", "TYPE", "PD", "QD", "GS", "BS", "BUS_AREA", "VM", "VA", "BASE_KV", "ZONE", "VMAX",
    "VMIN",
];
const GEN: &[&str] = &[
    "GEN_BUS",
    "PG",
    "QG",
    "QMAX",
    "QMIN",
    "VG",
    "MBASE",
    "GEN_STATUS",
    "PMAX",
    "PMIN",
    "PC1",
    "PC2",
    "QC1MIN",
    "QC1MAX",
    "QC2MIN",
    "QC2MAX",
    "RAMP_AGC",
    "RAMP_10",
    "RAMP_30",
    "RAMP_Q",
    "APF",
];
const BRANCH: &[&str] = &[
    "F_BUS",
    "T_BUS",
    "BR_R",
    "BR_X",
    "BR_B",
    "RATE_A",
    "RATE_B",
    "RATE_C",
    "TAP",
    "SHIFT",
    "BR_STATUS",
    "ANGMIN",
    "ANGMAX",
    "PF",
    "QF",
    "PT",
    "QT",
];
const DCLINE: &[&str] = &[
    "F_BUS",
    "T_BUS",
    "BR_STATUS",
    "PF",
    "PT",
    "QF",
    "QT",
    "VF",
    "VT",
    "PMIN",
    "PMAX",
    "QMINF",
    "QMAXF",
    "QMINT",
    "QMAXT",
    "LOSS0",
    "LOSS1",
];

/// The power flow model's tables, in MATPOWER's column order.
pub fn from_model(power: &PowerCase, from_raw: bool) -> Records {
    let case = &power.case;
    let bus_type = |k: rs_power::BusType| -> f64 {
        match k {
            rs_power::BusType::Pq => 1.0,
            rs_power::BusType::Pv => 2.0,
            rs_power::BusType::Reference => 3.0,
            rs_power::BusType::Isolated => 4.0,
        }
    };
    let buses = case
        .buses
        .iter()
        .map(|b| {
            let mut v = vec![
                b.id as f64,
                bus_type(b.kind),
                b.pd,
                b.qd,
                b.gs,
                b.bs,
                b.area as f64,
                b.vm,
                b.va,
                b.base_kv,
                b.zone as f64,
                b.vmax,
                b.vmin,
            ];
            v.extend(&b.extra);
            (v, Some(crate::elements::bus(b.id)))
        })
        .collect();
    let mut tables = vec![model_table(
        "bus",
        "Buses",
        BUS,
        &["BUS_I", "TYPE", "BUS_AREA", "ZONE"],
        "Column",
        buses,
        case.labels.get("bus_name"),
    )];
    let generators = case
        .generators
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let mut v = vec![
                g.bus as f64,
                g.pg,
                g.qg,
                g.qmax,
                g.qmin,
                g.vg,
                g.mbase,
                if g.online { 1.0 } else { 0.0 },
                g.pmax,
                g.pmin,
            ];
            v.extend(&g.extra);
            let element = power.elements.generators.get(i).map(|e| e.element.clone());
            (v, element)
        })
        .collect();
    tables.push(model_table(
        "gen",
        "Generators",
        GEN,
        &["GEN_BUS", "GEN_STATUS"],
        "Column",
        generators,
        None,
    ));
    let branches = case
        .branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut v = vec![
                b.from as f64,
                b.to as f64,
                b.r,
                b.x,
                b.b,
                b.rate_a,
                b.rate_b,
                b.rate_c,
                b.tap,
                b.shift,
                if b.online { 1.0 } else { 0.0 },
                b.angmin,
                b.angmax,
            ];
            v.extend(&b.extra);
            let element = power.elements.branches.get(i).map(|e| e.element.clone());
            (v, element)
        })
        .collect();
    tables.push(model_table(
        "branch",
        "Branches",
        BRANCH,
        &["F_BUS", "T_BUS", "BR_STATUS"],
        "Column",
        branches,
        None,
    ));
    if let Some(rows) = case.dcline.as_ref().filter(|r| !r.is_empty()) {
        let rows = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.clone(), power.elements.dc_lines.get(i).cloned()))
            .collect();
        tables.push(model_table(
            "dcline",
            "DC lines",
            DCLINE,
            &["F_BUS", "T_BUS", "BR_STATUS"],
            "Column",
            rows,
            None,
        ));
    }
    if let Some(rows) = case.gencost.as_ref().filter(|r| !r.is_empty()) {
        let rows = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let element = power.elements.generators.get(i).map(|e| e.element.clone());
                (r.clone(), element)
            })
            .collect();
        tables.push(model_table(
            "gencost",
            "Generator costs",
            &["MODEL", "STARTUP", "SHUTDOWN", "NCOST"],
            &["MODEL", "NCOST"],
            "COST",
            rows,
            None,
        ));
    }
    Records {
        source: RecordSource::Model,
        revision: None,
        // A RAW file shown as the model can't be written back as RAW.
        editable: !from_raw,
        note: from_raw.then(|| {
            "Stannum reads RAW revisions 33 and 35 record by record. This file is shown as the power flow model instead, in MATPOWER's columns: loads and fixed shunts are summed into each bus.".to_string()
        }),
        tables,
    }
}
