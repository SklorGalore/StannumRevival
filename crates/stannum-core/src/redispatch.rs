//! Redispatch generation to relieve branch overloads, as a DC optimal power
//! flow with enforced limits.
//!
//! The study starts from the last AC solve. A linear program moves online
//! generators up and down, keeping each island's total the same and every
//! unit within Pmin and Pmax, so that every monitored branch's flow is
//! within its limit, while moving as few megawatts as it can (no cost data
//! is needed: RAW cases carry none). Flows respond to the moves through DC
//! distribution factors (PTDFs), and only the branches that are, or would
//! become, over their limit are constraints, added a few at a time.
//!
//! Each pass then solves the case in AC with the new outputs. Losses,
//! reactive flow and controls make AC loadings differ from the DC estimate,
//! so a branch still over its limit has its megawatt cap lowered by the
//! difference and the next pass moves units again from where the last one
//! left them. A simplex solve gives a corner of the feasible region, so few
//! units move.
//!
//! The answer lists the moves and the branches before and after, and the
//! record edits that write the new outputs into the case (`redispatch`),
//! so History logs one line and the change saves with the case like any
//! Records edit.

use crate::contingency::{Rating, element_name};
use crate::power::{self, PowerCase, SolveSettings};
use crate::record_edit::RecordEdit;
use crate::records::{RecordSource, Records};
use crate::transfer::Subsystem;
use microlp::{ComparisonOp, OptimizationDirection, Problem};
use rs_power::{DcFactors, PowerFlowResult, SolveStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

fn default_limit() -> f64 {
    100.0
}
fn default_passes() -> usize {
    4
}
fn yes() -> bool {
    true
}

/// What to relieve and what may move.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct RedispatchSettings {
    /// The rating limits are taken from.
    #[serde(default)]
    pub rating: Rating,
    /// Loading, in percent of the rating, that no monitored branch may end
    /// above.
    #[serde(default = "default_limit")]
    pub limit_pct: f64,
    /// Generators at these buses may move (every bus when empty).
    #[serde(default)]
    pub generators: Subsystem,
    /// Branches with an end at these buses are monitored (every bus when
    /// empty).
    #[serde(default)]
    pub monitor: Subsystem,
    /// Relieve only these branches (element ids such as `line:1:2:1`). Other
    /// monitored branches may not go over their limit, nor further over it
    /// if they already are.
    #[serde(default)]
    pub branches: Option<Vec<String>>,
    /// Check each pass with an AC solve, moving units again while a branch
    /// is still over in AC. Without it the answer is the DC estimate.
    #[serde(default = "yes")]
    pub ac_check: bool,
    /// The most AC passes.
    #[serde(default = "default_passes")]
    pub max_passes: usize,
}

impl Default for RedispatchSettings {
    fn default() -> Self {
        Self {
            rating: Rating::A,
            limit_pct: default_limit(),
            generators: Subsystem::default(),
            monitor: Subsystem::default(),
            branches: None,
            ac_check: true,
            max_passes: default_passes(),
        }
    }
}

/// One generator the redispatch moves.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct MovedUnit {
    pub element: String,
    pub name: String,
    pub bus: u32,
    pub area: u32,
    pub mw_before: f64,
    pub mw_after: f64,
    pub pmin: f64,
    pub pmax: f64,
}

/// One branch's loading before and after.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct BranchLoading {
    pub element: String,
    pub name: String,
    pub from: u32,
    pub to: u32,
    /// The rating, MVA.
    pub rating_mva: f64,
    /// The loading this branch must end at or below, percent: the limit,
    /// or its loading before when it was already over and not chosen.
    pub limit_pct: f64,
    pub pct_before: f64,
    pub pct_after: f64,
    /// Active flow at the from end, MW.
    pub mw_before: f64,
    pub mw_after: f64,
    /// One of the branches to relieve.
    pub target: bool,
}

/// How the redispatch went.
#[derive(Clone, Debug, Default, Serialize, PartialEq, schemars::JsonSchema)]
pub struct RedispatchSummary {
    /// `relieved` (every monitored branch within its limit), `partly` (some
    /// still over: no room left to move), `nothing_to_do`, or `failed`
    /// (the AC solve didn't converge; the moves are the last good pass's).
    pub status: String,
    /// Megawatts raised in all (as much is lowered).
    pub moved_mw: f64,
    pub units_moved: usize,
    /// Monitored branches over their limit, before and after.
    pub over_before: usize,
    pub over_after: usize,
    pub worst_before: Option<BranchLoading>,
    pub worst_after: Option<BranchLoading>,
    /// AC passes run (zero without the AC check).
    pub passes: usize,
    /// The loadings after come from an AC solve (else the DC estimate).
    pub ac_checked: bool,
    /// Linear programs solved.
    pub programs: usize,
    pub monitored: usize,
    pub movable: usize,
    pub elapsed_ms: f64,
}

/// What redispatch would do, or did.
#[derive(Clone, Debug, Default, Serialize, schemars::JsonSchema)]
pub struct RedispatchReport {
    pub summary: RedispatchSummary,
    /// Largest moves first.
    pub units: Vec<MovedUnit>,
    /// Branches to relieve, then any other branch over its limit before or
    /// after, or held at its limit by the program; most loaded after first.
    pub branches: Vec<BranchLoading>,
    pub notes: Vec<String>,
    /// The cell edits that write the new outputs.
    #[serde(skip)]
    pub edits: Vec<RecordEdit>,
}

/// What `redispatch` answers: what changed, and the case after.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Redispatched {
    pub report: RedispatchReport,
    pub case: crate::CaseSummary,
}

/// Overload slack cost per MW, against 1 per MW moved: the program takes a
/// branch over its limit only when no move can bring it back.
const OVER_COST: f64 = 1000.0;
/// Moves smaller than this aren't made, MW.
const TINY_MW: f64 = 1e-4;
/// Loading that counts as at the limit, percent points.
const SLACK_PCT: f64 = 0.05;
/// Constraint rounds per linear program.
const ROUNDS: usize = 20;
/// MW caps aim this far inside the limit, so AC loadings land just under.
const AIM: f64 = 0.998;

struct Unit {
    gen_row: usize,
    bus_row: usize,
    bus_id: u32,
    island: Option<usize>,
    pmin: f64,
    pmax: f64,
}

struct Monitored {
    row: usize,
    rate: f64,
    /// The loading allowed, percent.
    limit: f64,
    target: bool,
    /// Cap scale learned from AC passes.
    tighten: f64,
}

/// Loadings and from-end MW of every branch row in `result`.
fn loadings(power: &PowerCase, result: &PowerFlowResult, rows: &[Monitored]) -> Vec<(f64, f64)> {
    rows.iter()
        .map(|m| {
            let f = result.branch_flows[m.row];
            let winding = power.elements.branches.get(m.row).map_or(0, |e| e.winding);
            (
                power::loading_pct(f[0], f[1], f[2], f[3], m.rate, winding),
                f[0],
            )
        })
        .collect()
}

/// Work out the redispatch from the AC solve `base` of `power`.
pub fn plan(
    records: Option<&Records>,
    power: &PowerCase,
    base: &PowerFlowResult,
    solve: &SolveSettings,
    settings: &RedispatchSettings,
) -> Result<RedispatchReport, String> {
    plan_solved(records, power, base, solve, settings).map(|(report, _)| report)
}

/// [`plan`], with the last converged AC solve of the moved case too (with
/// the AC check; `base` itself without it, or when nothing moved).
pub(crate) fn plan_solved(
    records: Option<&Records>,
    power: &PowerCase,
    base: &PowerFlowResult,
    solve: &SolveSettings,
    settings: &RedispatchSettings,
) -> Result<(RedispatchReport, PowerFlowResult), String> {
    let start = Instant::now();
    if !(settings.limit_pct.is_finite() && settings.limit_pct > 0.0) {
        return Err("The limit must be more than 0%.".into());
    }
    let case = &power.case;
    let names = power::bus_names(case);
    let bus_row: HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let name_of = |id: &str| element_name(id, &names, &bus_row);
    let mut notes = Vec::new();
    let targets: Option<HashSet<&str>> = settings
        .branches
        .as_ref()
        .map(|l| l.iter().map(String::as_str).collect());
    if let Some(t) = &targets {
        let known: HashSet<&str> = power
            .elements
            .branches
            .iter()
            .map(|e| e.element.as_str())
            .collect();
        let missing: Vec<&str> = t.iter().copied().filter(|e| !known.contains(e)).collect();
        if missing.len() == t.len() {
            return Err(format!("The case has no branch {}.", missing.join(", ")));
        }
        if !missing.is_empty() {
            notes.push(format!("Not in the case: {}.", missing.join(", ")));
        }
    }

    // Monitored branches and their loadings now.
    let mut monitored: Vec<Monitored> = Vec::new();
    for (row, b) in case.branches.iter().enumerate() {
        let rate = settings.rating.of(b);
        if !b.online || rate <= 0.0 {
            continue;
        }
        let in_monitor = settings.monitor.is_everything()
            && settings.monitor.kv_min.is_none()
            && settings.monitor.kv_max.is_none()
            || [b.from, b.to].iter().any(|id| {
                bus_row
                    .get(id)
                    .is_some_and(|&r| settings.monitor.contains(power, r))
            });
        let element = power.elements.branches.get(row).map(|e| e.element.as_str());
        let target = targets
            .as_ref()
            .is_some_and(|t| element.is_some_and(|e| t.contains(e)));
        if in_monitor || target {
            monitored.push(Monitored {
                row,
                rate,
                limit: settings.limit_pct,
                target,
                tighten: 1.0,
            });
        }
    }
    let before = loadings(power, base, &monitored);
    // Over-limit branches that aren't to be relieved may stay where they are.
    for (m, &(pct, _)) in monitored.iter_mut().zip(&before) {
        let relieve = targets.is_none() || m.target;
        if !relieve && pct > m.limit {
            m.limit = pct;
        }
        if targets.is_none() {
            m.target = pct > settings.limit_pct + SLACK_PCT;
        }
    }

    // Units that may move.
    let factors = DcFactors::new(case).map_err(|e| e.to_string())?;
    let units: Vec<Unit> = case
        .generators
        .iter()
        .enumerate()
        .filter(|(_, g)| g.online && g.pmax > g.pmin)
        .filter_map(|(i, g)| {
            let bus = *bus_row.get(&g.bus)?;
            settings.generators.contains(power, bus).then(|| Unit {
                gen_row: i,
                bus_row: bus,
                bus_id: g.bus,
                island: factors.reference_of(bus),
                pmin: g.pmin,
                pmax: g.pmax,
            })
        })
        .filter(|u| u.island.is_some())
        .collect();
    let base_pg: Vec<f64> = units
        .iter()
        .map(|u| base.case.generators[u.gen_row].pg)
        .collect();

    let mut summary = RedispatchSummary {
        monitored: monitored.len(),
        movable: units.len(),
        ac_checked: settings.ac_check,
        ..RedispatchSummary::default()
    };
    let over = |pcts: &[(f64, f64)], monitored: &[Monitored]| -> Vec<usize> {
        (0..monitored.len())
            .filter(|&i| pcts[i].0 > monitored[i].limit + SLACK_PCT)
            .collect()
    };
    let needs = over(&before, &monitored);
    if needs.is_empty() {
        summary.status = "nothing_to_do".into();
        notes.push(match &targets {
            Some(_) => "The chosen branches are within their limit already.".into(),
            None => "No monitored branch is over its limit.".into(),
        });
    } else if units.is_empty() {
        notes.push("No generator may move: check the generator buses.".into());
    }

    // Passes: DC program from the current state, then AC check.
    let mut pg = base_pg.clone();
    let mut state = base.clone();
    let mut after = before.clone();
    let mut held: HashSet<usize> = HashSet::new();
    let mut failed: Option<String> = None;
    let passes = if settings.ac_check {
        settings.max_passes.max(1)
    } else {
        1
    };
    if !needs.is_empty() && !units.is_empty() {
        for pass in 0..passes {
            let flows: Vec<[f64; 4]> = monitored
                .iter()
                .map(|m| state.branch_flows[m.row])
                .collect();
            // MW caps from the MVA limits: at the end that loads the branch
            // most, the active flow that fits beside its reactive flow,
            // moved to the from end by the branch's losses.
            let caps: Vec<f64> = monitored
                .iter()
                .zip(&flows)
                .map(|(m, f)| {
                    let mva = m.rate * m.limit / 100.0 * AIM * m.tighten;
                    let s_from = f[0].hypot(f[1]);
                    let s_to = f[2].hypot(f[3]);
                    let (q, losses) = if s_to > s_from {
                        (f[3], f[0].abs() - f[2].abs())
                    } else {
                        (f[1], 0.0)
                    };
                    let p = (mva * mva - q * q).max(0.0).sqrt().max(0.05 * mva);
                    (p + losses).max(0.0)
                })
                .collect();
            let f0: Vec<f64> = flows.iter().map(|f| f[0]).collect();
            let room: Vec<(f64, f64)> = units
                .iter()
                .zip(&pg)
                .map(|(u, &p)| ((u.pmax - p).max(0.0), (p - u.pmin).max(0.0)))
                .collect();
            let moves = program(
                &factors,
                &units,
                &room,
                &monitored,
                &caps,
                &f0,
                &mut held,
                &mut summary.programs,
            )?;
            for (p, d) in pg.iter_mut().zip(&moves) {
                *p += d;
            }
            if !settings.ac_check {
                // The DC estimate, scaled onto each branch's loading.
                let delta: Vec<(u32, f64)> = units
                    .iter()
                    .zip(&moves)
                    .filter(|(_, d)| d.abs() > TINY_MW)
                    .map(|(u, &d)| (u.bus_id, d))
                    .collect();
                let change = factors.injection_flows(&delta);
                for (i, m) in monitored.iter().enumerate() {
                    let mw = f0[i] + change[m.row];
                    let pct = if f0[i].abs() > 1e-9 {
                        before[i].0 * (mw / f0[i]).abs()
                    } else {
                        before[i].0
                    };
                    after[i] = (pct, mw);
                }
                break;
            }
            let mut moved = case.clone();
            for (u, &p) in units.iter().zip(&pg) {
                moved.generators[u.gen_row].pg = p;
            }
            let mut options = solve.options_for(power);
            let warm = power::warm_start(&moved, &state, &mut options);
            summary.passes = pass + 1;
            match rs_power::solve(&warm, &options) {
                Ok(r) if r.status == SolveStatus::Converged => {
                    state = r;
                }
                Ok(r) => {
                    failed = Some(format!(
                        "The AC solve after pass {} didn't converge ({:?}).",
                        pass + 1,
                        r.status
                    ));
                    for (p, d) in pg.iter_mut().zip(&moves) {
                        *p -= d;
                    }
                    break;
                }
                Err(e) => {
                    failed = Some(format!("The AC solve after pass {} failed: {e}", pass + 1));
                    for (p, d) in pg.iter_mut().zip(&moves) {
                        *p -= d;
                    }
                    break;
                }
            }
            after = loadings(power, &state, &monitored);
            let still = over(&after, &monitored);
            if still.is_empty() {
                break;
            }
            // Lower each still-over branch's cap by how far AC is over.
            for &i in &still {
                monitored[i].tighten *= (monitored[i].limit / after[i].0).min(1.0);
                held.insert(i);
            }
            let room_left = units
                .iter()
                .zip(&pg)
                .any(|(u, &p)| p < u.pmax - TINY_MW || p > u.pmin + TINY_MW);
            if !room_left {
                break;
            }
        }
    }

    // The report.
    let branch = |i: usize, (pct_b, mw_b): (f64, f64), (pct_a, mw_a): (f64, f64)| {
        let m = &monitored[i];
        let b = &case.branches[m.row];
        let element = power
            .elements
            .branches
            .get(m.row)
            .map(|e| e.element.clone())
            .unwrap_or_default();
        BranchLoading {
            name: name_of(&element),
            element,
            from: b.from,
            to: b.to,
            rating_mva: m.rate,
            limit_pct: m.limit,
            pct_before: pct_b,
            pct_after: pct_a,
            mw_before: mw_b,
            mw_after: mw_a,
            target: m.target,
        }
    };
    let over_after = over(&after, &monitored);
    summary.over_before = needs.len();
    summary.over_after = over_after.len();
    let worst = |pcts: &[(f64, f64)]| {
        (0..monitored.len())
            .max_by(|&a, &b| {
                (pcts[a].0 / monitored[a].limit).total_cmp(&(pcts[b].0 / monitored[b].limit))
            })
            .map(|i| branch(i, before[i], after[i]))
    };
    summary.worst_before = worst(&before);
    summary.worst_after = worst(&after);
    let mut listed: Vec<usize> = (0..monitored.len())
        .filter(|&i| {
            monitored[i].target
                || needs.contains(&i)
                || over_after.contains(&i)
                || held.contains(&i)
        })
        .collect();
    listed.sort_by(|&a, &b| after[b].0.total_cmp(&after[a].0));
    let branches: Vec<BranchLoading> = listed
        .into_iter()
        .map(|i| branch(i, before[i], after[i]))
        .collect();

    let mut moved_units: Vec<MovedUnit> = units
        .iter()
        .zip(&base_pg)
        .zip(&pg)
        .filter(|((_, b), a)| (*a - *b).abs() > TINY_MW)
        .map(|((u, &b), &a)| {
            let g = &case.generators[u.gen_row];
            let element = power
                .elements
                .generators
                .get(u.gen_row)
                .map(|e| e.element.clone())
                .unwrap_or_default();
            MovedUnit {
                name: name_of(&element),
                element,
                bus: g.bus,
                area: case.buses[u.bus_row].area,
                mw_before: b,
                mw_after: a,
                pmin: u.pmin,
                pmax: u.pmax,
            }
        })
        .collect();
    moved_units.sort_by(|a, b| {
        (b.mw_after - b.mw_before)
            .abs()
            .total_cmp(&(a.mw_after - a.mw_before).abs())
    });
    summary.units_moved = moved_units.len();
    summary.moved_mw = moved_units
        .iter()
        .map(|u| (u.mw_after - u.mw_before).max(0.0))
        .sum();
    if summary.status.is_empty() {
        summary.status = if failed.is_some() {
            "failed"
        } else if over_after.is_empty() {
            "relieved"
        } else {
            "partly"
        }
        .into();
    }
    if let Some(f) = failed {
        notes.push(f);
    }
    if summary.status == "partly" {
        notes.push(format!(
            "{} branch{} still over: the generators that could relieve {} are at their limits, or don't move flow on {}.",
            over_after.len(),
            if over_after.len() == 1 { " is" } else { "es are" },
            if over_after.len() == 1 { "it" } else { "them" },
            if over_after.len() == 1 { "it" } else { "them" },
        ));
    }
    let reactive: Vec<String> = over_after
        .iter()
        .filter(|&&i| {
            let f = state.branch_flows[monitored[i].row];
            settings.ac_check && f[0].abs() < 0.5 * f[1].abs()
        })
        .map(|&i| {
            let element = power
                .elements
                .branches
                .get(monitored[i].row)
                .map(|e| e.element.clone())
                .unwrap_or_default();
            name_of(&element)
        })
        .collect();
    if !reactive.is_empty() {
        notes.push(format!(
            "Mostly reactive flow keeps {} over: moving generation can't relieve that; voltage control or switched shunts can.",
            reactive.join(", ")
        ));
    }
    if summary.passes > 1 && summary.status == "relieved" {
        notes.push(format!(
            "The DC estimate needed {} AC passes to land within the limit.",
            summary.passes
        ));
    }
    if !settings.ac_check && summary.units_moved > 0 {
        notes.push("Loadings after are DC estimates: solve in AC to confirm.".into());
    }
    if solve.area_interchange != power::InterchangeChoice::Off && summary.units_moved > 0 {
        notes.push("Area interchange control is on, so area slack generators may undo moves between areas.".into());
    }

    let edits = match records {
        Some(r) if summary.units_moved > 0 => edits_for(r, power, &units, &base_pg, &pg)?,
        _ => Vec::new(),
    };
    summary.elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
    Ok((
        RedispatchReport {
            summary,
            units: moved_units,
            branches,
            notes,
            edits,
        },
        state,
    ))
}

/// One linear program, adding the branches it would push over until none
/// is. Returns each unit's move, MW.
#[allow(clippy::too_many_arguments)]
fn program(
    factors: &DcFactors,
    units: &[Unit],
    room: &[(f64, f64)],
    monitored: &[Monitored],
    caps: &[f64],
    f0: &[f64],
    held: &mut HashSet<usize>,
    count: &mut usize,
) -> Result<Vec<f64>, String> {
    let all_rows: Vec<usize> = monitored.iter().map(|m| m.row).collect();
    let mut active: Vec<usize> = (0..monitored.len())
        .filter(|&i| f0[i].abs() > caps[i] || held.contains(&i))
        .collect();
    let mut sens: HashMap<usize, Vec<f64>> = HashMap::new();
    let mut moves = vec![0.0; units.len()];
    for _ in 0..ROUNDS {
        let missing: Vec<usize> = active
            .iter()
            .copied()
            .filter(|i| !sens.contains_key(i))
            .collect();
        let rows: Vec<usize> = missing.iter().map(|&i| all_rows[i]).collect();
        for (i, s) in missing.into_iter().zip(factors.branch_sensitivities(&rows)) {
            sens.insert(i, s);
        }
        let mut p = Problem::new(OptimizationDirection::Minimize);
        let vars: Vec<(microlp::Variable, microlp::Variable)> = room
            .iter()
            .map(|&(up, down)| (p.add_var(1.0, (0.0, up)), p.add_var(1.0, (0.0, down))))
            .collect();
        // Each island keeps its total.
        let mut islands: HashMap<usize, Vec<usize>> = HashMap::new();
        for (k, u) in units.iter().enumerate() {
            if let Some(i) = u.island {
                islands.entry(i).or_default().push(k);
            }
        }
        for members in islands.values() {
            let terms: Vec<(microlp::Variable, f64)> = members
                .iter()
                .flat_map(|&k| [(vars[k].0, 1.0), (vars[k].1, -1.0)])
                .collect();
            p.add_constraint(terms, ComparisonOp::Eq, 0.0);
        }
        for &i in &active {
            let s = &sens[&i];
            let over = p.add_var(OVER_COST, (0.0, f64::INFINITY));
            let terms: Vec<(microlp::Variable, f64)> = units
                .iter()
                .enumerate()
                .filter(|(_, u)| s[u.bus_row].abs() > 1e-9)
                .flat_map(|(k, u)| [(vars[k].0, s[u.bus_row]), (vars[k].1, -s[u.bus_row])])
                .collect();
            let mut upper = terms.clone();
            upper.push((over, -1.0));
            p.add_constraint(upper, ComparisonOp::Le, caps[i] - f0[i]);
            let mut lower: Vec<(microlp::Variable, f64)> =
                terms.iter().map(|&(v, c)| (v, -c)).collect();
            lower.push((over, -1.0));
            p.add_constraint(lower, ComparisonOp::Le, caps[i] + f0[i]);
        }
        *count += 1;
        let solution = p
            .solve()
            .map_err(|e| format!("The redispatch program failed: {e}"))?
            .into_solution()
            .map_err(|_| "The redispatch program stopped before an answer.".to_string())?;
        moves = vars
            .iter()
            .map(|&(up, down)| {
                let d = solution.var_value(up) - solution.var_value(down);
                if d.abs() > TINY_MW { d } else { 0.0 }
            })
            .collect();
        // Flows on every monitored branch after the moves.
        let injections: Vec<(u32, f64)> = units
            .iter()
            .zip(&moves)
            .filter(|(_, d)| **d != 0.0)
            .map(|(u, &d)| (u.bus_id, d))
            .collect();
        let change = factors.injection_flows(&injections);
        let newly: Vec<usize> = (0..monitored.len())
            .filter(|i| !active.contains(i))
            .filter(|&i| (f0[i] + change[all_rows[i]]).abs() > caps[i] + 1e-6)
            .collect();
        if newly.is_empty() {
            break;
        }
        active.extend(newly);
    }
    for &i in &active {
        let s = &sens[&i];
        let flow: f64 = f0[i]
            + units
                .iter()
                .zip(&moves)
                .map(|(u, d)| s[u.bus_row] * d)
                .sum::<f64>();
        if flow.abs() >= caps[i] - 1e-3 {
            held.insert(i);
        }
    }
    Ok(moves)
}

/// The record edits that set each moved unit's output.
fn edits_for(
    records: &Records,
    power: &PowerCase,
    units: &[Unit],
    before: &[f64],
    after: &[f64],
) -> Result<Vec<RecordEdit>, String> {
    let table = match records.source {
        RecordSource::Raw => records.table("GENERATOR"),
        RecordSource::Model => records.table("gen"),
    }
    .map_err(|e| e.to_string())?;
    let pg_col = table
        .columns
        .iter()
        .position(|c| c.name == "PG")
        .ok_or_else(|| format!("{} has no PG field.", table.label))?;
    let row_of: HashMap<&str, usize> = table
        .rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.element.as_deref().map(|e| (e, i)))
        .collect();
    let mut edits = Vec::new();
    for ((u, &b), &a) in units.iter().zip(before).zip(after) {
        if (a - b).abs() <= TINY_MW {
            continue;
        }
        let element = &power.elements.generators[u.gen_row].element;
        let row = *row_of
            .get(element.as_str())
            .ok_or_else(|| format!("{element} has no record to edit."))?;
        edits.push(RecordEdit {
            section: table.id.clone(),
            row,
            column: pg_col,
            value: text(a),
        });
    }
    Ok(edits)
}

fn text(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// Apply the moves to a copy of `case`, for checks.
#[cfg(test)]
fn with_moves(
    case: &rs_power::Case,
    power: &PowerCase,
    report: &RedispatchReport,
) -> rs_power::Case {
    let mut out = case.clone();
    for u in &report.units {
        let row = power
            .elements
            .generators
            .iter()
            .position(|e| e.element == u.element)
            .unwrap();
        out.generators[row].pg = u.mw_after;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case9() -> (PowerCase, PowerFlowResult, SolveSettings) {
        let text = include_str!("../../rs-power/tests/fixtures/case9.m");
        let case = rs_power::parse_matpower(text).unwrap();
        let power = crate::power::from_case(case);
        let solve = SolveSettings::default();
        let solved = rs_power::solve(&power.case, &solve.options_for(&power)).unwrap();
        (power, solved, solve)
    }

    /// case9 with branch 8-9 (86 MW, 29 Mvar) rated so it is overloaded.
    fn tight(rate: f64) -> (PowerCase, PowerFlowResult, SolveSettings, usize) {
        let (mut power, _, solve) = case9();
        let row = power
            .case
            .branches
            .iter()
            .position(|b| (b.from, b.to) == (8, 9) || (b.from, b.to) == (9, 8))
            .unwrap();
        power.case.branches[row].rate_a = rate;
        let solved = rs_power::solve(&power.case, &solve.options_for(&power)).unwrap();
        (power, solved, solve, row)
    }

    #[test]
    fn nothing_to_do_when_within_limits() {
        let (power, solved, solve) = case9();
        let r = plan(
            None,
            &power,
            &solved,
            &solve,
            &RedispatchSettings::default(),
        )
        .unwrap();
        assert_eq!(r.summary.status, "nothing_to_do");
        assert!(r.units.is_empty());
    }

    #[test]
    fn relieves_an_overload_in_ac() {
        let (power, solved, solve, row) = tight(80.0);
        let r = plan(
            None,
            &power,
            &solved,
            &solve,
            &RedispatchSettings::default(),
        )
        .unwrap();
        let s = &r.summary;
        assert_eq!(s.status, "relieved", "{r:#?}");
        assert_eq!(s.over_before, 1);
        assert_eq!(s.over_after, 0);
        let b = r.branches.iter().find(|b| b.target).unwrap();
        assert!(
            b.pct_before > 100.0 && b.pct_after <= 100.0 + SLACK_PCT,
            "{b:?}"
        );
        assert!(b.pct_after > 95.0, "moves no more than needed: {b:?}");
        // Units stay within limits, and the moves balance.
        let up: f64 = r
            .units
            .iter()
            .map(|u| (u.mw_after - u.mw_before).max(0.0))
            .sum();
        let down: f64 = r
            .units
            .iter()
            .map(|u| (u.mw_before - u.mw_after).max(0.0))
            .sum();
        assert!((up - down).abs() < 1e-6 + 5.0, "{up} {down}");
        for u in &r.units {
            assert!(
                u.mw_after >= u.pmin - 1e-6 && u.mw_after <= u.pmax + 1e-6,
                "{u:?}"
            );
        }
        // An independent AC solve with the moves agrees.
        let moved = with_moves(&power.case, &power, &r);
        let check = rs_power::solve(&moved, &solve.options_for(&power)).unwrap();
        let f = check.branch_flows[row];
        let pct = power::loading_pct(f[0], f[1], f[2], f[3], 80.0, 0);
        assert!(pct <= 100.0 + SLACK_PCT, "{pct}");
    }

    #[test]
    fn dc_only_gives_an_estimate() {
        let (power, solved, solve, _) = tight(80.0);
        let settings = RedispatchSettings {
            ac_check: false,
            ..Default::default()
        };
        let r = plan(None, &power, &solved, &solve, &settings).unwrap();
        assert_eq!(r.summary.passes, 0);
        assert!(r.summary.units_moved > 0);
        assert!(r.notes.iter().any(|n| n.contains("DC estimates")));
    }

    #[test]
    fn reports_what_cannot_be_relieved() {
        // Its reactive flow alone is over a 20 MVA rating.
        let (power, solved, solve, _) = tight(20.0);
        let r = plan(
            None,
            &power,
            &solved,
            &solve,
            &RedispatchSettings::default(),
        )
        .unwrap();
        assert_eq!(r.summary.status, "partly", "{r:#?}");
        assert_eq!(r.summary.over_after, 1);
        assert!(
            r.notes.iter().any(|n| n.contains("reactive")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn a_fixed_generator_does_not_move() {
        let (power, solved, solve, _) = tight(80.0);
        let settings = RedispatchSettings {
            generators: Subsystem {
                buses: vec![1, 2],
                ..Default::default()
            },
            ..Default::default()
        };
        let r = plan(None, &power, &solved, &solve, &settings).unwrap();
        assert!(r.units.iter().all(|u| u.bus == 1 || u.bus == 2), "{r:#?}");
    }

    /// With one constraint, the least movement is the best pair of units:
    /// one up, one down, the pair whose distribution factors differ most
    /// for the room they have. Checked against every pair.
    #[test]
    fn the_program_finds_the_best_pair() {
        let (power, solved, _, row) = tight(80.0);
        let case = &power.case;
        let factors = DcFactors::new(case).unwrap();
        let bus_row: HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let units: Vec<Unit> = case
            .generators
            .iter()
            .enumerate()
            .map(|(i, g)| Unit {
                gen_row: i,
                bus_row: bus_row[&g.bus],
                bus_id: g.bus,
                island: factors.reference_of(bus_row[&g.bus]),
                pmin: g.pmin,
                pmax: g.pmax,
            })
            .collect();
        let pg: Vec<f64> = solved.case.generators.iter().map(|g| g.pg).collect();
        let room: Vec<(f64, f64)> = units
            .iter()
            .zip(&pg)
            .map(|(u, &p)| (u.pmax - p, p - u.pmin))
            .collect();
        let monitored = vec![Monitored {
            row,
            rate: 80.0,
            limit: 100.0,
            target: true,
            tighten: 1.0,
        }];
        let f0 = vec![solved.branch_flows[row][0]];
        for cap in [80.0, 60.0, 40.0] {
            let mut held = HashSet::new();
            let mut n = 0;
            let moves = program(
                &factors,
                &units,
                &room,
                &monitored,
                &[cap],
                &f0,
                &mut held,
                &mut n,
            )
            .unwrap();
            let raised: f64 = moves.iter().map(|d| d.max(0.0)).sum();
            let s = &factors.branch_sensitivities(&[row])[0];
            let excess = f0[0].abs() - cap;
            let mut best = f64::INFINITY;
            for (i, ui) in units.iter().enumerate() {
                for (j, uj) in units.iter().enumerate() {
                    // Raise i, lower j by d: the flow changes by d (s_i - s_j).
                    let k = (s[ui.bus_row] - s[uj.bus_row]) * -f0[0].signum();
                    if i == j || k <= 1e-12 {
                        continue;
                    }
                    let d = excess / k;
                    if d <= room[i].0 + 1e-9 && d <= room[j].1 + 1e-9 {
                        best = best.min(d);
                    }
                }
            }
            assert!(
                (raised - best).abs() < 1e-6,
                "cap {cap}: program {raised}, best pair {best}"
            );
            let units_moved = moves.iter().filter(|d| **d != 0.0).count();
            assert_eq!(units_moved, 2, "{moves:?}");
        }
    }
}
