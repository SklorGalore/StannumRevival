use crate::{BusType, Case, Error};
use std::{fmt::Write, path::Path};

/// Serialize a case as literal MATPOWER version-2 assignments.
///
/// Preserves row order, extra columns, costs and supported labels. Generator
/// statuses are written as 0/1, matching the boolean representation in `Case`.
/// Rejects ragged matrices, NaN and multiline labels. This checks format only;
/// it does not validate network topology or physical inputs.
pub fn format_matpower(case: &Case) -> Result<String, Error> {
    let mut out = String::from("mpc.version = '2';\nmpc.baseMVA = ");
    number(&mut out, case.base_mva)?;
    out.push_str(";\n");
    matrix(
        &mut out,
        "bus",
        case.buses.iter().map(|b| {
            let kind = match b.kind {
                BusType::Pq => 1.0,
                BusType::Pv => 2.0,
                BusType::Reference => 3.0,
                BusType::Isolated => 4.0,
            };
            let mut row = vec![
                b.id as f64,
                kind,
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
            row.extend_from_slice(&b.extra);
            row
        }),
    )?;
    matrix(
        &mut out,
        "gen",
        case.generators.iter().map(|g| {
            let mut row = vec![
                g.bus as f64,
                g.pg,
                g.qg,
                g.qmax,
                g.qmin,
                g.vg,
                g.mbase,
                u8::from(g.online) as f64,
                g.pmax,
                g.pmin,
            ];
            row.extend_from_slice(&g.extra);
            row
        }),
    )?;
    matrix(
        &mut out,
        "branch",
        case.branches.iter().map(|b| {
            let mut row = vec![
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
                u8::from(b.online) as f64,
                b.angmin,
                b.angmax,
            ];
            row.extend_from_slice(&b.extra);
            row
        }),
    )?;
    if let Some(lines) = &case.dcline {
        if lines.iter().any(|r| r.len() < 17) {
            return Err(Error::InvalidCase(
                "dcline requires at least 17 columns".into(),
            ));
        }
        matrix(&mut out, "dcline", lines.iter().cloned())?;
    }
    if let Some(costs) = &case.gencost {
        matrix(&mut out, "gencost", costs.iter().cloned())?;
    }
    for (name, labels) in &case.labels {
        if !matches!(name.as_str(), "bus_name" | "gentype" | "genfuel") {
            return Err(Error::InvalidCase(format!(
                "unsupported label field {name}"
            )));
        }
        writeln!(out, "mpc.{name} = {{").unwrap();
        for label in labels {
            if label.contains(['\n', '\r']) {
                return Err(Error::InvalidCase(
                    "labels must be single-line strings".into(),
                ));
            }
            writeln!(out, "    '{}';", label.replace('\'', "''")).unwrap();
        }
        out.push_str("};\n");
    }
    Ok(out)
}

fn number(out: &mut String, value: f64) -> Result<(), Error> {
    if value.is_nan() {
        return Err(Error::InvalidCase(
            "cannot serialize NaN in a MATPOWER case".into(),
        ));
    }
    if value == f64::INFINITY {
        out.push_str("Inf");
    } else if value == f64::NEG_INFINITY {
        out.push_str("-Inf");
    } else {
        write!(out, "{value}").unwrap();
    }
    Ok(())
}

fn matrix(out: &mut String, name: &str, rows: impl Iterator<Item = Vec<f64>>) -> Result<(), Error> {
    writeln!(out, "mpc.{name} = [").unwrap();
    let mut width = None;
    for row in rows {
        if row.is_empty() || width.is_some_and(|n| n != row.len()) {
            return Err(Error::InvalidCase(format!(
                "{name} rows must have equal, nonzero lengths"
            )));
        }
        width = Some(row.len());
        out.push_str("    ");
        for (i, value) in row.into_iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            number(out, value)?;
        }
        out.push_str(";\n");
    }
    out.push_str("];\n");
    Ok(())
}

/// Save a MATPOWER `.m` function using the destination file stem as its name.
/// Existing files are overwritten, after serialization succeeds.
///
/// # Errors
///
/// Returns [`Error::InvalidCase`] for an invalid MATLAB function name or data
/// rejected by [`format_matpower`], and [`Error::Io`] if writing fails. The stem
/// must start with an ASCII letter, contain only ASCII letters, digits and
/// underscores, have at most 63 characters, and not be a MATLAB keyword.
pub fn save_matpower(path: impl AsRef<Path>, case: &Case) -> Result<(), Error> {
    let path = path.as_ref();
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if !name.starts_with(|c: char| c.is_ascii_alphabetic())
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || name.len() > 63
        || matches!(
            name,
            "break"
                | "case"
                | "catch"
                | "classdef"
                | "continue"
                | "else"
                | "elseif"
                | "end"
                | "for"
                | "function"
                | "global"
                | "if"
                | "otherwise"
                | "parfor"
                | "persistent"
                | "return"
                | "spmd"
                | "switch"
                | "try"
                | "while"
        )
    {
        return Err(Error::InvalidCase(
            "file stem must be a valid MATLAB function name (at most 63 ASCII characters)".into(),
        ));
    }
    let body = format_matpower(case)?;
    std::fs::write(path, format!("function mpc = {name}\n{body}end\n"))?;
    Ok(())
}
