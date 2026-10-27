//! The working case as RAW text: the file as opened, with the Records edits
//! already in it and the study's edits written into the records they
//! change. Case diff compares this with the file on disk.

use crate::power::PowerCase;
use crate::record_edit::RecordEdit;
use crate::records::{RecordTable, Records};
use crate::study::{self, Edit};
use serde_json::Value as Json;

/// The status field of each table a study's outage can change.
const STATUS_TABLES: [&str; 4] = [
    "BRANCH",
    "TRANSFORMER",
    "SYSTEM SWITCHING DEVICE",
    "GENERATOR",
];

fn column(table: &RecordTable, name: &str) -> Result<usize, String> {
    table
        .columns
        .iter()
        .position(|c| c.name == name)
        .ok_or_else(|| format!("{} has no {name} field.", table.label))
}

fn number(cell: Option<&Json>) -> f64 {
    match cell {
        Some(Json::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Json::String(s)) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// A value as RAW text: whole numbers stay whole, others keep up to six
/// decimals without trailing zeros.
fn text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The Records edits that write `edits` into a RAW case's records.
///
/// Outages set STAT, generator output sets PG, and a bus's new load scales
/// the constant power, current and impedance parts (PL/QL, IP/IQ, YP/YQ) of
/// its in-service load records by the same factors the solve uses, so the
/// file read back gives the same load.
pub fn study_record_edits(
    records: &Records,
    base: &PowerCase,
    edits: &[Edit],
) -> Result<Vec<RecordEdit>, String> {
    let mut out: Vec<RecordEdit> = Vec::new();
    let mut set =
        |table: &RecordTable, row: usize, name: &str, value: String| -> Result<(), String> {
            let column = column(table, name)?;
            // A later edit to the same field wins, as it does in the solve.
            out.retain(|e| !(e.section == table.id && e.row == row && e.column == column));
            out.push(RecordEdit {
                section: table.id.clone(),
                row,
                column,
                value,
            });
            Ok(())
        };
    let find = |element: &str, tables: &[&str]| -> Vec<(&RecordTable, usize)> {
        records
            .tables
            .iter()
            .filter(|t| tables.contains(&t.id.as_str()))
            .flat_map(|t| {
                t.rows
                    .iter()
                    .enumerate()
                    .filter(move |(_, r)| r.element.as_deref() == Some(element))
                    .map(move |(i, _)| (t, i))
            })
            .collect()
    };

    let mut load_buses: Vec<u32> = Vec::new();
    for edit in edits {
        match edit {
            Edit::Status { element, online } => {
                let rows = find(element, &STATUS_TABLES);
                if rows.is_empty() {
                    return Err(format!("{element} has no record in the file to change."));
                }
                for (table, row) in rows {
                    set(table, row, "STAT", if *online { "1" } else { "0" }.into())?;
                }
            }
            Edit::Generation { element, mw } => {
                let (table, row) = *find(element, &["GENERATOR"])
                    .first()
                    .ok_or_else(|| format!("{element} has no generator record in the file."))?;
                set(table, row, "PG", text(*mw))?;
            }
            Edit::Load { bus, .. } => {
                if !load_buses.contains(bus) {
                    load_buses.push(*bus);
                }
            }
        }
    }
    if load_buses.is_empty() {
        return Ok(out);
    }

    // Loads: compare each bus before and after the study, part by part.
    let edited = study::apply(base, edits)?;
    let loads = records.table("LOAD")?;
    let status = column(loads, "STAT")?;
    let prefix_of = |bus: u32| format!("load:{bus}:");
    for bus in load_buses {
        let before = base.case.buses.iter().find(|b| b.id == bus);
        let after = edited.case.buses.iter().find(|b| b.id == bus);
        let (Some(before), Some(after)) = (before, after) else {
            return Err(format!("Bus {bus} isn't in this case."));
        };
        let prefix = prefix_of(bus);
        let rows: Vec<usize> = loads
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.element.as_deref().is_some_and(|e| e.starts_with(&prefix))
                    && number(r.cells.get(status)) != 0.0
            })
            .map(|(i, _)| i)
            .collect();
        if rows.is_empty() {
            return Err(format!(
                "Bus {bus} has no load record in service to write its new load into."
            ));
        }
        for (field, was, now) in [
            ("PL", before.pd, after.pd),
            ("QL", before.qd, after.qd),
            ("IP", before.pd_i, after.pd_i),
            ("IQ", before.qd_i, after.qd_i),
            ("YP", before.pd_z, after.pd_z),
            ("YQ", before.qd_z, after.qd_z),
        ] {
            if (now - was).abs() < 1e-9 {
                continue;
            }
            let col = column(loads, field)?;
            if was != 0.0 {
                let k = now / was;
                for &row in &rows {
                    let v = number(loads.rows[row].cells.get(col));
                    if v != 0.0 {
                        set(loads, row, field, text(v * k))?;
                    }
                }
            } else {
                // Nothing drawn before: the first load takes it all.
                set(loads, rows[0], field, text(now))?;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::text;

    #[test]
    fn numbers_read_as_raw_text() {
        assert_eq!(text(120.0), "120");
        assert_eq!(text(-3.5), "-3.5");
        assert_eq!(text(1.0 / 3.0), "0.333333");
        assert_eq!(text(0.0), "0");
    }
}
