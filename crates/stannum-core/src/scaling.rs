//! Scale load and generation in a set of buses, as PSS/E's SCAL does.
//!
//! The buses come from a [`Subsystem`] (areas, zones, owners, buses and a kV
//! range). Loads and generators at those buses are scaled to a target total
//! or by a change (MW or percent), and the new values are written into the
//! case's records, so the change is part of the case like any Records edit:
//! saved with it, compared by Case diff, and logged in History as one
//! `scale_load_generation` command.
//!
//! Load totals are the nominal load (at 1 p.u. voltage): constant power plus
//! constant current plus constant admittance. Only in-service loads at the
//! chosen buses count, and negative loads (often small generators modelled
//! as load) are left alone unless asked for.

use crate::power::PowerCase;
use crate::record_edit::RecordEdit;
use crate::records::{RecordSource, RecordTable, Records};
use crate::transfer::Subsystem;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};

/// How much to scale by.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Amount {
    /// The total the scaled loads or generators should come to.
    Target(f64),
    /// Add this much to the total (negative lowers it).
    Change(f64),
    /// Change the total by this percent (10 raises it by a tenth).
    Percent(f64),
}

impl Amount {
    /// The new total for a present total of `now`.
    fn total(self, now: f64) -> f64 {
        match self {
            Amount::Target(v) => v,
            Amount::Change(v) => now + v,
            Amount::Percent(p) => now * (1.0 + p / 100.0),
        }
    }
}

/// Which parts of a load scale.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LoadParts {
    /// Constant power, current and admittance parts alike.
    #[default]
    All,
    /// Only the constant power part (PL/QL).
    ConstantPower,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct LoadScaling {
    /// Real power: left out, MW stays as it is.
    #[serde(default)]
    pub mw: Option<Amount>,
    /// Reactive power: left out, Mvar stays as it is, unless
    /// `constant_power_factor` moves it with the MW.
    #[serde(default)]
    pub mvar: Option<Amount>,
    /// Scale each load's Mvar by the same factor as its MW, so its power
    /// factor stays the same. `mvar` is then ignored.
    #[serde(default)]
    pub constant_power_factor: bool,
    #[serde(default)]
    pub parts: LoadParts,
    /// Also scale loads that draw negative MW.
    #[serde(default)]
    pub include_negative: bool,
}

/// How a generation change is shared between units.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Participation {
    /// In proportion to each unit's output: every unit scales by the same
    /// factor.
    #[default]
    Output,
    /// In proportion to each unit's maximum output.
    Pmax,
    /// In proportion to each unit's room to move: Pmax - PG going up,
    /// PG - Pmin going down.
    Headroom,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct GenerationScaling {
    pub mw: Amount,
    /// Keep every unit between Pmin and Pmax. What the others can't take
    /// is reported as a shortfall.
    #[serde(default = "yes")]
    pub respect_limits: bool,
    #[serde(default)]
    pub participation: Participation,
}

fn yes() -> bool {
    true
}

/// What to scale and where.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ScaleSettings {
    /// The buses whose loads and generators scale. Empty lists mean every bus.
    #[serde(default)]
    pub buses: Subsystem,
    #[serde(default)]
    pub load: Option<LoadScaling>,
    #[serde(default)]
    pub generation: Option<GenerationScaling>,
}

/// Totals before and after.
#[derive(Clone, Debug, Default, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Totals {
    /// How many loads or in-service generators are at the chosen buses.
    pub count: usize,
    pub mw_before: f64,
    pub mw_after: f64,
    pub mvar_before: f64,
    pub mvar_after: f64,
}

/// One scaled load or generator.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ScaledRow {
    pub element: String,
    pub bus: u32,
    pub area: u32,
    pub mw_before: f64,
    pub mw_after: f64,
    pub mvar_before: f64,
    pub mvar_after: f64,
    /// A generator stopped at Pmin or Pmax.
    pub at_limit: bool,
}

/// What scaling did, or would do.
#[derive(Clone, Debug, Default, Serialize, schemars::JsonSchema)]
pub struct ScaleReport {
    /// How many buses the filters chose.
    pub buses: usize,
    pub load: Option<Totals>,
    pub generation: Option<Totals>,
    /// Generation MW the units couldn't take within their limits.
    pub shortfall_mw: f64,
    /// How many generators ended at Pmin or Pmax.
    pub at_limit: usize,
    /// The biggest changes first, at most [`ROWS_KEPT`] of them.
    pub rows: Vec<ScaledRow>,
    /// How many loads and generators changed in all.
    pub changed: usize,
    /// Anything worth knowing, such as loads left alone.
    pub notes: Vec<String>,
    /// The cell edits that make the change.
    #[serde(skip)]
    pub edits: Vec<RecordEdit>,
}

/// What `scale_load_generation` answers: what changed, and the case after.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Scaled {
    pub report: ScaleReport,
    pub case: crate::CaseSummary,
}

/// How many rows a report lists.
pub const ROWS_KEPT: usize = 2000;

/// Changes smaller than this (MW or Mvar) aren't written.
const TINY: f64 = 1e-9;

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

/// A value as record text: up to six decimals, no trailing zeros.
fn text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// One load: the record row and its parts' columns and values.
struct Load {
    element: String,
    bus: u32,
    row: usize,
    /// (column, value) of each real power part that scales.
    p: Vec<(usize, f64)>,
    q: Vec<(usize, f64)>,
    /// Every part's total, scaling or not, for the report.
    p_all: f64,
    q_all: f64,
}

fn loads<'a>(
    records: &'a Records,
    chosen: &HashSet<u32>,
    parts: LoadParts,
) -> Result<(&'a RecordTable, Vec<Load>), String> {
    match records.source {
        RecordSource::Raw => {
            let table = records.table("LOAD").map_err(|e| e.to_string())?;
            let status = column(table, "STAT")?;
            let fields = |names: [&str; 3]| -> Result<Vec<usize>, String> {
                names.iter().map(|n| column(table, n)).collect()
            };
            let pc = fields(["PL", "IP", "YP"])?;
            let qc = fields(["QL", "IQ", "YQ"])?;
            let used = match parts {
                LoadParts::All => 3,
                LoadParts::ConstantPower => 1,
            };
            let mut out = Vec::new();
            for (row, r) in table.rows.iter().enumerate() {
                let Some(element) = r.element.as_deref() else {
                    continue;
                };
                let Some(bus) = element
                    .strip_prefix("load:")
                    .and_then(|rest| rest.split(':').next())
                    .and_then(|b| b.parse::<u32>().ok())
                else {
                    continue;
                };
                if !chosen.contains(&bus) || number(r.cells.get(status)) == 0.0 {
                    continue;
                }
                let value = |c: &usize| (*c, number(r.cells.get(*c)));
                let p: Vec<_> = pc.iter().map(value).collect();
                let q: Vec<_> = qc.iter().map(value).collect();
                out.push(Load {
                    element: element.to_string(),
                    bus,
                    row,
                    p_all: p.iter().map(|x| x.1).sum(),
                    q_all: q.iter().map(|x| x.1).sum(),
                    p: p[..used].to_vec(),
                    q: q[..used].to_vec(),
                });
            }
            Ok((table, out))
        }
        RecordSource::Model => {
            // MATPOWER loads are the buses' PD and QD.
            let table = records.table("bus").map_err(|e| e.to_string())?;
            let (id, pd, qd) = (
                column(table, "BUS_I")?,
                column(table, "PD")?,
                column(table, "QD")?,
            );
            let mut out = Vec::new();
            for (row, r) in table.rows.iter().enumerate() {
                let bus = number(r.cells.get(id)) as u32;
                let (p, q) = (number(r.cells.get(pd)), number(r.cells.get(qd)));
                if !chosen.contains(&bus) || (p == 0.0 && q == 0.0) {
                    continue;
                }
                out.push(Load {
                    element: format!("load:{bus}:1"),
                    bus,
                    row,
                    p: vec![(pd, p)],
                    q: vec![(qd, q)],
                    p_all: p,
                    q_all: q,
                });
            }
            Ok((table, out))
        }
    }
}

/// The factor that takes `now` to `amount`'s total.
fn factor(amount: Amount, now: f64, what: &str) -> Result<f64, String> {
    let then = amount.total(now);
    if now.abs() < TINY {
        if then.abs() < TINY {
            return Ok(1.0);
        }
        return Err(format!(
            "The chosen loads draw no {what}, so there's nothing to scale to {}.",
            text((then * 1000.0).round() / 1000.0)
        ));
    }
    Ok(then / now)
}

/// One in-service generator at a chosen bus.
struct Unit {
    element: String,
    bus: u32,
    row: usize,
    pg: f64,
    qg: f64,
    pmin: f64,
    pmax: f64,
}

/// Share `delta` MW between `units` by `weights`, keeping them within
/// limits when asked. Returns the new outputs and what couldn't be placed.
fn share(units: &[Unit], delta: f64, g: &GenerationScaling) -> (Vec<f64>, Vec<bool>, f64) {
    let up = delta > 0.0;
    let mut pg: Vec<f64> = units.iter().map(|u| u.pg).collect();
    let mut stuck = vec![false; units.len()];
    let weight = |u: &Unit, now: f64| -> f64 {
        match g.participation {
            Participation::Output => now.abs(),
            Participation::Pmax => u.pmax.max(0.0),
            Participation::Headroom if up => (u.pmax - now).max(0.0),
            Participation::Headroom => (now - u.pmin).max(0.0),
        }
    };
    // Room left before a unit's limit in the direction of travel. Units
    // already past a limit don't move further past it.
    let room = |u: &Unit, now: f64| -> f64 {
        if up {
            (u.pmax - now).max(0.0)
        } else {
            (now - u.pmin).max(0.0)
        }
    };
    let mut left = delta;
    // Each pass either places everything or pins at least one unit.
    for _ in 0..=units.len() {
        if left.abs() < TINY {
            break;
        }
        let total: f64 = units
            .iter()
            .enumerate()
            .filter(|(i, _)| !stuck[*i])
            .map(|(i, u)| weight(u, pg[i]))
            .sum();
        if total < TINY {
            break;
        }
        let weights: Vec<f64> = units
            .iter()
            .enumerate()
            .map(|(i, u)| if stuck[i] { 0.0 } else { weight(u, pg[i]) })
            .collect();
        let mut placed = 0.0;
        for (i, u) in units.iter().enumerate() {
            if weights[i] == 0.0 {
                continue;
            }
            let want = left * weights[i] / total;
            let step = if g.respect_limits {
                let r = room(u, pg[i]);
                if want.abs() >= r - TINY {
                    stuck[i] = true;
                    want.signum() * r
                } else {
                    want
                }
            } else {
                want
            };
            pg[i] += step;
            placed += step;
        }
        left -= placed;
        if !g.respect_limits {
            break;
        }
    }
    let at_limit = units
        .iter()
        .enumerate()
        .map(|(i, u)| {
            g.respect_limits
                && pg[i] != u.pg
                && ((pg[i] - u.pmax).abs() < 1e-6 || (pg[i] - u.pmin).abs() < 1e-6)
        })
        .collect();
    (pg, at_limit, left)
}

/// Work out the scaling: the report and the cell edits that make it.
pub fn plan(
    records: &Records,
    power: &PowerCase,
    settings: &ScaleSettings,
) -> Result<ScaleReport, String> {
    if settings.load.is_none() && settings.generation.is_none() {
        return Err("Choose load, generation or both to scale.".into());
    }
    let chosen = settings.buses.bus_ids(power);
    if chosen.is_empty() {
        return Err("No bus passes the filters.".into());
    }
    let area_of: HashMap<u32, u32> = power.case.buses.iter().map(|b| (b.id, b.area)).collect();
    let mut report = ScaleReport {
        buses: chosen.len(),
        ..ScaleReport::default()
    };
    let mut rows: Vec<ScaledRow> = Vec::new();
    let mut edits: Vec<RecordEdit> = Vec::new();

    if let Some(l) = &settings.load {
        let (table, all) = loads(records, &chosen, l.parts)?;
        let negative = all.iter().filter(|x| x.p_all < 0.0).count();
        let (scaled, kept): (Vec<_>, Vec<_>) = all
            .into_iter()
            .partition(|x| l.include_negative || x.p_all >= 0.0);
        if negative > 0 && !l.include_negative {
            report.notes.push(format!(
                "{negative} negative load{} left as {} {}.",
                if negative == 1 { "" } else { "s" },
                if negative == 1 { "it" } else { "they" },
                if negative == 1 { "is" } else { "are" },
            ));
        }
        // Only the scaling parts move, so the factor is set on their sum
        // and the other parts' sum stays fixed.
        let sum = |f: &dyn Fn(&Load) -> f64| scaled.iter().map(f).sum::<f64>();
        let p_fixed = sum(&|x| x.p_all - x.p.iter().map(|c| c.1).sum::<f64>());
        let q_fixed = sum(&|x| x.q_all - x.q.iter().map(|c| c.1).sum::<f64>());
        let p_moving = sum(&|x| x.p.iter().map(|c| c.1).sum());
        let q_moving = sum(&|x| x.q.iter().map(|c| c.1).sum());
        let p_now = p_fixed + p_moving + kept.iter().map(|x| x.p_all).sum::<f64>();
        let q_now = q_fixed + q_moving + kept.iter().map(|x| x.q_all).sum::<f64>();
        // Targets are for every load at the chosen buses, negative ones too.
        let to = |amount: Amount, now: f64, moving: f64, what: &str| -> Result<f64, String> {
            let then = amount.total(now);
            factor(Amount::Target(moving + then - now), moving, what)
        };
        let kp = match l.mw {
            Some(a) => to(a, p_now, p_moving, "MW")?,
            None => 1.0,
        };
        let kq = if l.constant_power_factor {
            kp
        } else {
            match l.mvar {
                Some(a) => to(a, q_now, q_moving, "Mvar")?,
                None => 1.0,
            }
        };
        let mut totals = Totals {
            count: scaled.len() + kept.len(),
            mw_before: p_now,
            mvar_before: q_now,
            ..Totals::default()
        };
        totals.mw_after = kept.iter().map(|x| x.p_all).sum();
        totals.mvar_after = kept.iter().map(|x| x.q_all).sum();
        for x in &scaled {
            let mut p_after = x.p_all;
            let mut q_after = x.q_all;
            for (parts, k, after) in [(&x.p, kp, &mut p_after), (&x.q, kq, &mut q_after)] {
                for &(col, v) in parts {
                    let now = v * k;
                    if (now - v).abs() > TINY {
                        edits.push(RecordEdit {
                            section: table.id.clone(),
                            row: x.row,
                            column: col,
                            value: text(now),
                        });
                        *after += now - v;
                    }
                }
            }
            totals.mw_after += p_after;
            totals.mvar_after += q_after;
            if (p_after - x.p_all).abs() > TINY || (q_after - x.q_all).abs() > TINY {
                rows.push(ScaledRow {
                    element: x.element.clone(),
                    bus: x.bus,
                    area: area_of.get(&x.bus).copied().unwrap_or(0),
                    mw_before: x.p_all,
                    mw_after: p_after,
                    mvar_before: x.q_all,
                    mvar_after: q_after,
                    at_limit: false,
                });
            }
        }
        if totals.count == 0 {
            report
                .notes
                .push("No load in service at the chosen buses.".into());
        }
        report.load = Some(totals);
    }

    if let Some(g) = &settings.generation {
        let table = match records.source {
            RecordSource::Raw => records.table("GENERATOR"),
            RecordSource::Model => records.table("gen"),
        }
        .map_err(|e| e.to_string())?;
        let pg_col = column(table, "PG")?;
        let row_of: HashMap<&str, usize> = table
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.element.as_deref().map(|e| (e, i)))
            .collect();
        let units: Vec<Unit> = power
            .case
            .generators
            .iter()
            .zip(&power.elements.generators)
            .filter(|(u, _)| u.online && chosen.contains(&u.bus))
            .filter_map(|(u, e)| {
                Some(Unit {
                    element: e.element.clone(),
                    bus: u.bus,
                    row: *row_of.get(e.element.as_str())?,
                    pg: u.pg,
                    qg: u.qg,
                    pmin: u.pmin,
                    pmax: u.pmax,
                })
            })
            .collect();
        let before: f64 = units.iter().map(|u| u.pg).sum();
        let target = g.mw.total(before);
        let (after, at_limit, left) = share(&units, target - before, g);
        if !units.is_empty()
            && (target - before).abs() > TINY
            && (left - (target - before)).abs() < TINY
        {
            report.notes.push(match g.participation {
                Participation::Output => {
                    "The chosen generators produce nothing, so scaling by output can't move them: share by Pmax or headroom instead.".into()
                }
                _ => "The chosen generators have no room to move.".into(),
            });
        }
        report.shortfall_mw = left;
        report.at_limit = at_limit.iter().filter(|&&a| a).count();
        let mut totals = Totals {
            count: units.len(),
            mw_before: before,
            mw_after: after.iter().sum(),
            mvar_before: units.iter().map(|u| u.qg).sum(),
            ..Totals::default()
        };
        totals.mvar_after = totals.mvar_before;
        if units.is_empty() {
            report
                .notes
                .push("No generator in service at the chosen buses.".into());
        }
        for (i, u) in units.iter().enumerate() {
            if (after[i] - u.pg).abs() <= TINY {
                continue;
            }
            edits.push(RecordEdit {
                section: table.id.clone(),
                row: u.row,
                column: pg_col,
                value: text(after[i]),
            });
            rows.push(ScaledRow {
                element: u.element.clone(),
                bus: u.bus,
                area: area_of.get(&u.bus).copied().unwrap_or(0),
                mw_before: u.pg,
                mw_after: after[i],
                mvar_before: u.qg,
                mvar_after: u.qg,
                at_limit: at_limit[i],
            });
        }
        report.generation = Some(totals);
    }

    report.changed = rows.len();
    rows.sort_by(|a, b| {
        let d =
            |r: &ScaledRow| (r.mw_after - r.mw_before).abs() + (r.mvar_after - r.mvar_before).abs();
        d(b).total_cmp(&d(a))
    });
    rows.truncate(ROWS_KEPT);
    report.rows = rows;
    report.edits = edits;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(pg: f64, pmin: f64, pmax: f64) -> Unit {
        Unit {
            element: String::new(),
            bus: 1,
            row: 0,
            pg,
            qg: 0.0,
            pmin,
            pmax,
        }
    }

    fn generation(
        mw: Amount,
        respect_limits: bool,
        participation: Participation,
    ) -> GenerationScaling {
        GenerationScaling {
            mw,
            respect_limits,
            participation,
        }
    }

    #[test]
    fn amounts() {
        assert_eq!(Amount::Target(50.0).total(80.0), 50.0);
        assert_eq!(Amount::Change(-5.0).total(80.0), 75.0);
        assert!((Amount::Percent(10.0).total(80.0) - 88.0).abs() < 1e-12);
    }

    #[test]
    fn output_scaling_is_one_factor() {
        let units = [unit(50.0, 0.0, 200.0), unit(100.0, 0.0, 200.0)];
        let g = generation(Amount::Change(30.0), true, Participation::Output);
        let (pg, _, left) = share(&units, 30.0, &g);
        assert!((pg[0] - 60.0).abs() < 1e-9 && (pg[1] - 120.0).abs() < 1e-9);
        assert!(left.abs() < 1e-9);
    }

    #[test]
    fn limits_pass_the_rest_on() {
        // The first unit can only take 10 MW; the second takes the rest.
        let units = [unit(90.0, 0.0, 100.0), unit(90.0, 0.0, 300.0)];
        let g = generation(Amount::Change(60.0), true, Participation::Output);
        let (pg, at, left) = share(&units, 60.0, &g);
        assert!((pg[0] - 100.0).abs() < 1e-9, "{pg:?}");
        assert!((pg[1] - 140.0).abs() < 1e-9, "{pg:?}");
        assert_eq!(at, vec![true, false]);
        assert!(left.abs() < 1e-9);
    }

    #[test]
    fn shortfall_when_everyone_is_full() {
        let units = [unit(90.0, 0.0, 100.0), unit(50.0, 0.0, 60.0)];
        let g = generation(Amount::Change(50.0), true, Participation::Headroom);
        let (pg, _, left) = share(&units, 50.0, &g);
        assert_eq!(pg, vec![100.0, 60.0]);
        assert!((left - 30.0).abs() < 1e-9);
    }

    #[test]
    fn ignoring_limits_goes_past_pmax() {
        let units = [unit(90.0, 0.0, 100.0)];
        let g = generation(Amount::Change(50.0), false, Participation::Output);
        let (pg, at, left) = share(&units, 50.0, &g);
        assert_eq!(pg, vec![140.0]);
        assert_eq!(at, vec![false]);
        assert_eq!(left, 0.0);
    }

    #[test]
    fn going_down_stops_at_pmin() {
        let units = [unit(50.0, 40.0, 100.0), unit(50.0, 0.0, 100.0)];
        let g = generation(Amount::Change(-40.0), true, Participation::Output);
        let (pg, at, left) = share(&units, -40.0, &g);
        assert!((pg[0] - 40.0).abs() < 1e-9, "{pg:?}");
        assert!((pg[1] - 20.0).abs() < 1e-9, "{pg:?}");
        assert_eq!(at, vec![true, false]);
        assert!(left.abs() < 1e-9);
    }

    #[test]
    fn record_text() {
        assert_eq!(text(12.5), "12.5");
        assert_eq!(text(-0.0000001), "0");
        assert_eq!(text(100.0), "100");
    }
}
