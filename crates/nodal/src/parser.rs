use crate::model::*;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

fn diagnostic(section: &str, line: usize, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        severity: "error".into(),
        section: section.into(),
        line,
        message: message.into(),
    }
}

/// RAW quoting and comments are recognized only outside strings. Quoted padding
/// is retained for display; comparison keys are normalized separately.
fn tokenize(text: &str, section: &str, line: usize) -> Result<Vec<String>, Diagnostic> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut quote = None;
    let mut quoted = false;
    let mut closed = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                if chars.peek() == Some(&q) {
                    field.push(c);
                    chars.next();
                } else {
                    quote = None;
                    closed = true;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '/' => break,
                ',' => {
                    out.push(if quoted {
                        field.clone()
                    } else {
                        field.trim().to_owned()
                    });
                    field.clear();
                    quoted = false;
                    closed = false;
                }
                '\'' | '"' if field.trim().is_empty() && !closed => {
                    field.clear();
                    quote = Some(c);
                    quoted = true;
                }
                c if closed && c.is_whitespace() => {}
                _ if closed => {
                    return Err(diagnostic(
                        section,
                        line,
                        "Unexpected text after quoted field",
                    ));
                }
                _ => field.push(c),
            }
        }
    }
    if quote.is_some() {
        return Err(diagnostic(section, line, "Unterminated quoted field"));
    }
    out.push(if quoted {
        field
    } else {
        field.trim().to_owned()
    });
    Ok(out)
}

struct Row {
    line: usize,
    text: String,
    fields: Vec<String>,
}

fn source(rows: &[Row], layout: &Value, section: &str) -> Result<SourceRecord, Diagnostic> {
    let mut fields = BTreeMap::new();
    for (row, spec) in rows.iter().zip(layout["lines"].as_array().unwrap()) {
        for (i, f) in spec["fields"].as_array().unwrap().iter().enumerate() {
            let name = f["name"].as_str().unwrap();
            if let Some(value) = row.fields.get(i) {
                if !value.trim().is_empty() {
                    let valid = match f["type"].as_str().unwrap() {
                        "integer" => value.trim().parse::<i32>().is_ok(),
                        "number" => value.trim().parse::<f64>().is_ok_and(f64::is_finite),
                        _ => true,
                    };
                    if !valid {
                        return Err(diagnostic(
                            section,
                            row.line,
                            format!("Invalid {name}: {value}"),
                        ));
                    }
                }
                fields.insert(name.into(), value.clone());
            }
        }
        // Extensions remain inspectable without using annotations as a schema.
        let count = spec["fields"].as_array().unwrap().len();
        for (i, value) in row.fields.iter().enumerate().skip(count) {
            fields.insert(format!("EXTRA_{}_{}", row.line, i + 1), value.clone());
        }
    }
    Ok(SourceRecord {
        line: rows[0].line,
        section: section.into(),
        fields,
        raw_lines: rows.iter().map(|r| r.text.clone()).collect(),
    })
}

fn field<'a>(s: &'a SourceRecord, key: &str) -> Option<&'a str> {
    s.fields
        .get(key)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

fn integer(s: &SourceRecord, key: &str, default: Option<i32>) -> Result<i32, Diagnostic> {
    field(s, key)
        .and_then(|v| v.parse().ok())
        .or(default)
        .ok_or_else(|| diagnostic(&s.section, s.line, format!("Missing or invalid {key}")))
}

fn number(s: &SourceRecord, key: &str, default: Option<f64>) -> Result<f64, Diagnostic> {
    field(s, key)
        .and_then(|v| v.parse().ok())
        .or(default)
        .filter(|v: &f64| v.is_finite())
        .ok_or_else(|| diagnostic(&s.section, s.line, format!("Missing or invalid {key}")))
}

fn status(s: &SourceRecord, maximum: i32) -> Result<i32, Diagnostic> {
    let value = integer(s, if s.section == "BUS" { "IDE" } else { "STAT" }, Some(1))?;
    let minimum = if s.section == "BUS" { 1 } else { 0 };
    if value < minimum || value > maximum {
        return Err(diagnostic(
            &s.section,
            s.line,
            format!("Invalid status {value}"),
        ));
    }
    Ok(value)
}

fn bus_ref(s: &SourceRecord, key: &str) -> Result<i32, Diagnostic> {
    integer(s, key, None)?
        .checked_abs()
        .filter(|v| *v > 0)
        .ok_or_else(|| diagnostic(&s.section, s.line, format!("Invalid bus number {key}")))
}

pub fn parse_raw(bytes: &[u8]) -> Result<Case, Diagnostic> {
    let (text, encoding) = match std::str::from_utf8(bytes) {
        Ok(text) => (std::borrow::Cow::Borrowed(text), "UTF-8"),
        Err(_) => (
            encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(bytes)
                .0,
            "Windows-1252",
        ),
    };
    let lines: Vec<&str> = text.lines().collect();
    let header_index = lines
        .iter()
        .position(|line| {
            let line = line.trim().trim_start_matches('\u{feff}');
            !line.is_empty() && !line.starts_with('@') && !line.starts_with('/')
        })
        .ok_or_else(|| diagnostic("HEADER", 1, "Empty RAW file"))?;
    let header = tokenize(lines[header_index], "HEADER", header_index + 1)?;
    let rev = header.get(2).and_then(|v| v.parse::<i32>().ok());
    if rev != Some(35) {
        return Err(diagnostic(
            "HEADER",
            header_index + 1,
            "Only PSS/E RAW revision 35 is supported",
        ));
    }
    let base_mva = header
        .get(1)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| diagnostic("HEADER", header_index + 1, "Invalid SBASE"))?;
    let frequency_hz = match header.get(5).filter(|v| !v.is_empty()) {
        Some(v) => v
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| diagnostic("HEADER", header_index + 1, "Invalid BASFRQ"))?,
        None => 60.0,
    };
    if lines.len() < header_index + 3 {
        return Err(diagnostic(
            "HEADER",
            header_index + 1,
            "Missing two title lines",
        ));
    }
    let mut case = Case {
        revision: 35,
        base_mva,
        frequency_hz,
        zero_impedance_threshold: 0.0001,
        titles: lines[header_index + 1..header_index + 3]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        encoding: encoding.into(),
        buses: vec![],
        equipment: vec![],
        loads: vec![],
        generators: vec![],
        diagnostics: vec![],
        skipped_sections: BTreeMap::new(),
    };
    if encoding != "UTF-8" {
        case.diagnostics.push(Diagnostic {
            severity: "info".into(),
            section: "HEADER".into(),
            line: header_index + 1,
            message: "Decoded file using Windows-1252.".into(),
        });
    }
    let schema: Value =
        serde_json::from_str(include_str!("../raw.schema.json")).expect("Bundled RAW schema");
    let order = schema["section_order"]["35"].as_array().unwrap();
    let mut section_index = 0;
    let mut pos = header_index + 3;
    let mut finished = false;
    let mut seen_buses = HashSet::new();
    let mut seen_equipment = HashSet::new();
    let mut seen_injections = HashSet::new();
    let mut skip_lines = BTreeMap::new();
    while pos < lines.len() {
        let section = order
            .get(section_index)
            .and_then(Value::as_str)
            .unwrap_or("UNSUPPORTED CONNECTIVITY");
        let text = lines[pos];
        let line = pos + 1;
        pos += 1;
        if text.trim().is_empty() || text.trim_start().starts_with('@') {
            continue;
        }
        let tokens = tokenize(text, section, line)?;
        if tokens.len() == 1 && tokens[0].is_empty() {
            continue;
        }
        if tokens.len() == 1 && tokens[0].eq_ignore_ascii_case("Q") {
            if section_index < order.len() {
                return Err(diagnostic(
                    section,
                    line,
                    "File ended before all revision 35 sections were complete",
                ));
            }
            if lines[pos..].iter().any(|s| {
                !s.trim().is_empty()
                    && !s.trim_start().starts_with('@')
                    && !s.trim_start().starts_with('/')
            }) {
                return Err(diagnostic(
                    section,
                    line,
                    "Unexpected records after Q terminator",
                ));
            }
            finished = true;
            break;
        }
        if tokens.len() == 1 && tokens[0] == "0" {
            section_index += 1;
            continue;
        }
        if section == "SYSTEM-WIDE"
            && let Some(value) = system_wide_threshold(&tokens, line)?
        {
            case.zero_impedance_threshold = value;
        }
        if !matches!(
            section,
            "BUS" | "LOAD" | "GENERATOR" | "BRANCH" | "SYSTEM SWITCHING DEVICE" | "TRANSFORMER"
        ) {
            *case
                .skipped_sections
                .entry(section.to_string())
                .or_insert(0) += 1;
            skip_lines.entry(section.to_string()).or_insert(line);
            continue;
        }
        let layout = &schema["sections"][section]["layouts"]["35"];
        let mut rows = vec![Row {
            line,
            text: text.into(),
            fields: tokens,
        }];
        let variant;
        let layout = if section == "TRANSFORMER" {
            let k = rows[0]
                .fields
                .get(2)
                .and_then(|v| v.parse::<i32>().ok())
                .ok_or_else(|| diagnostic(section, line, "Missing or invalid K"))?;
            variant = if k == 0 { 0 } else { 1 };
            &layout["variants"][variant]
        } else {
            layout
        };
        let record_lines = layout["lines"].as_array().unwrap().len();
        while rows.len() < record_lines {
            if pos >= lines.len() {
                return Err(diagnostic(section, line, "Truncated transformer record"));
            }
            let text = lines[pos];
            let next_line = pos + 1;
            pos += 1;
            if text.trim().is_empty() || text.trim_start().starts_with('@') {
                continue;
            }
            let tokens = tokenize(text, section, next_line)?;
            if tokens.len() == 1 && tokens[0].is_empty() {
                continue;
            }
            let maximum_fields = layout["lines"][rows.len()]["fields"]
                .as_array()
                .unwrap()
                .len();
            if tokens.len() > maximum_fields {
                return Err(diagnostic(
                    section,
                    next_line,
                    format!(
                        "Truncated or malformed transformer continuation: expected at most {maximum_fields} fields"
                    ),
                ));
            }
            if (tokens.len() == 1 && (tokens[0] == "0" || tokens[0] == "Q"))
                || tokens
                    .get(3)
                    .is_some_and(|v| v.parse::<f64>().is_err() && !v.is_empty())
            {
                return Err(diagnostic(section, line, "Truncated transformer record"));
            }
            rows.push(Row {
                line: next_line,
                text: text.into(),
                fields: tokens,
            });
        }
        let s = source(&rows, layout, section)?;
        match section {
            "BUS" => {
                let id = integer(&s, "I", None)?;
                if id <= 0 {
                    return Err(diagnostic(section, line, "Bus number must be positive"));
                }
                if !seen_buses.insert(id) {
                    return Err(diagnostic(section, line, format!("Duplicate bus {id}")));
                }
                let kv = number(&s, "BASKV", None)?;
                if kv < 0.0 {
                    return Err(diagnostic(section, line, "BASKV must be nonnegative"));
                }
                case.buses.push(Bus {
                    number: id,
                    name: s.fields.get("NAME").cloned().unwrap_or_default(),
                    base_kv: kv,
                    bus_type: status(&s, 4)?,
                    area: field(&s, "AREA").and_then(|v| v.parse().ok()),
                    zone: field(&s, "ZONE").and_then(|v| v.parse().ok()),
                    voltage_pu: field(&s, "VM").and_then(|v| v.parse().ok()),
                    source: s,
                });
            }
            "LOAD" | "GENERATOR" => {
                let bus = bus_ref(&s, "I")?;
                let key = field(&s, "ID").unwrap_or("1");
                let id = format!("{}:{bus}:{key}", section.to_lowercase());
                if !seen_injections.insert(id.clone()) {
                    return Err(diagnostic(
                        section,
                        line,
                        format!("Duplicate equipment {id}"),
                    ));
                }
                let injection = Injection {
                    id,
                    bus,
                    status: status(&s, 1)?,
                    p_mw: number(&s, if section == "LOAD" { "PL" } else { "PG" }, Some(0.0))?,
                    q_mvar: number(&s, if section == "LOAD" { "QL" } else { "QG" }, Some(0.0))?,
                    source: s,
                };
                if section == "LOAD" {
                    case.loads.push(injection);
                } else {
                    case.generators.push(injection);
                }
            }
            _ => {
                let mut buses = vec![bus_ref(&s, "I")?, bus_ref(&s, "J")?];
                if section == "TRANSFORMER" && integer(&s, "K", None)? != 0 {
                    buses.push(bus_ref(&s, "K")?);
                }
                let kind = match section {
                    "BRANCH" => EquipmentKind::Line,
                    "TRANSFORMER" => EquipmentKind::Transformer,
                    _ => EquipmentKind::Switch,
                };
                let circuit = field(&s, "CKT").unwrap_or("1").to_owned();
                let mut normalized = buses.clone();
                normalized.sort_unstable();
                if normalized.windows(2).any(|b| b[0] == b[1]) {
                    return Err(diagnostic(
                        section,
                        line,
                        "Equipment terminals must reference distinct buses",
                    ));
                }
                let id = format!(
                    "{}:{}:{circuit}",
                    match kind {
                        EquipmentKind::Line => "line",
                        EquipmentKind::Transformer => "transformer",
                        EquipmentKind::Switch => "switch",
                    },
                    normalized
                        .iter()
                        .map(i32::to_string)
                        .collect::<Vec<_>>()
                        .join(":")
                );
                if !seen_equipment.insert(id.clone()) {
                    return Err(diagnostic(
                        section,
                        line,
                        format!("Duplicate equipment {id}"),
                    ));
                }
                let stat = status(&s, if buses.len() == 3 { 4 } else { 1 })?;
                let active_terminals = match stat {
                    0 => vec![false; buses.len()],
                    2 => vec![true, false, true],
                    3 => vec![true, true, false],
                    4 => vec![false, true, true],
                    _ => vec![true; buses.len()],
                };
                if section == "BRANCH" {
                    number(&s, "R", Some(0.0))?;
                    number(&s, "X", None)?;
                }
                if section == "TRANSFORMER" {
                    number(&s, "R1-2", Some(0.0))?;
                    number(&s, "X1-2", None)?;
                    number(&s, "WINDV1", None)?;
                    number(&s, "WINDV2", None)?;
                    if buses.len() == 3 {
                        number(&s, "X2-3", None)?;
                        number(&s, "X3-1", None)?;
                        number(&s, "WINDV3", None)?;
                    }
                }
                case.equipment.push(Equipment {
                    id,
                    kind,
                    name: s.fields.get("NAME").cloned().unwrap_or_default(),
                    circuit,
                    buses,
                    status: stat,
                    active_terminals,
                    source: s,
                });
            }
        }
    }
    if !finished {
        return Err(diagnostic(
            "FILE",
            lines.len().max(1),
            "Missing Q file terminator",
        ));
    }
    for (section, count) in &case.skipped_sections {
        let connectivity =
            section.contains("DC") || section.contains("FACTS") || section.contains("CONNECTIVITY");
        case.diagnostics.push(Diagnostic {
            severity: if connectivity { "warning" } else { "info" }.into(),
            section: section.clone(),
            line: skip_lines[section],
            message: format!(
                "{count} nonempty record lines {}.",
                if connectivity {
                    "excluded from AC topology; connectivity results may be incomplete"
                } else {
                    "retained only in the original file; section not used for topology"
                }
            ),
        });
    }
    for (s, buses) in case
        .equipment
        .iter()
        .map(|e| (&e.source, e.buses.clone()))
        .chain(
            case.loads
                .iter()
                .chain(case.generators.iter())
                .map(|i| (&i.source, vec![i.bus])),
        )
    {
        for bus in buses {
            if !seen_buses.contains(&bus) {
                return Err(diagnostic(
                    &s.section,
                    s.line,
                    format!("Reference to missing bus {bus}"),
                ));
            }
        }
    }
    Ok(case)
}

fn system_wide_threshold(fields: &[String], line: usize) -> Result<Option<f64>, Diagnostic> {
    if !fields
        .first()
        .is_some_and(|field| field.trim().eq_ignore_ascii_case("GENERAL"))
    {
        return Ok(None);
    }
    for field in fields.iter().skip(1) {
        let trimmed = field.trim();
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("THRSHZ") {
            return parse_threshold(value.trim(), line).map(Some);
        }
    }
    Ok(None)
}

fn parse_threshold(value: &str, line: usize) -> Result<f64, Diagnostic> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(|| {
            diagnostic(
                "SYSTEM-WIDE",
                line,
                "Invalid THRSHZ; expected a finite nonnegative number",
            )
        })
}
