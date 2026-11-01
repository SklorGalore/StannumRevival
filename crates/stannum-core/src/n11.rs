//! N-1-1: one element out, the system adjusted, then each contingency on
//! top (NERC TPL-001 P6 and the like). Each first outage is solved in AC
//! with every control of the last solve; the adjustment between the two,
//! when chosen, is the redispatch of [`crate::redispatch`], moving
//! generators as little as possible to bring branches back within their
//! limit; then every second outage runs as an ordinary contingency run
//! from the adjusted case, so problems are judged new or worse against it.

use crate::contingency::{
    ContingencyJob, ContingencySettings, Overload, Progress, Rating, RunHandle, StepChecker,
    VoltageProblem,
};
use crate::power::{PowerCase, SolveSettings};
use crate::redispatch::{self, RedispatchSettings};
use rs_power::PowerFlowResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// What happens between the first outage and the second.
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Adjustment {
    /// Nothing: the second outage comes straight after the first.
    None,
    /// Redispatch generators to bring branches back within their limit.
    #[default]
    Redispatch,
}

fn default_keep() -> usize {
    2000
}

/// What an N-1-1 run takes out, and how it adjusts in between.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct N11Settings {
    /// The second outages, what is checked and against which limits, as
    /// for an N-1 run. `pairs` and `planning` are not used.
    #[serde(default)]
    pub contingencies: ContingencySettings,
    /// The first outages (element ids, as `contingencies.elements`). The
    /// same outages as the second ones when absent.
    #[serde(default)]
    pub first: Option<Vec<String>>,
    #[serde(default)]
    pub adjustment: Adjustment,
    /// How the redispatch adjusts: which generators may move, the limit it
    /// brings branches within. Its AC check is always on.
    #[serde(default)]
    pub redispatch: RedispatchSettings,
    /// Pairs kept in the results, worst first. 2000 by default.
    #[serde(default = "default_keep")]
    pub keep_pairs: usize,
}

impl Default for N11Settings {
    fn default() -> Self {
        Self {
            contingencies: ContingencySettings::default(),
            first: None,
            adjustment: Adjustment::default(),
            redispatch: RedispatchSettings::default(),
            keep_pairs: default_keep(),
        }
    }
}

/// The adjustment after one first outage.
#[derive(Clone, Debug, Default, Serialize, PartialEq, schemars::JsonSchema)]
pub struct AdjustmentSummary {
    /// As the redispatch's: `relieved`, `partly`, `nothing_to_do` or
    /// `failed`.
    pub status: String,
    pub moved_mw: f64,
    pub units_moved: usize,
    /// Branches over the redispatch limit before and after.
    pub over_before: usize,
    pub over_after: usize,
    /// The most loaded branch after, percent of its rating.
    pub worst_after_pct: Option<f64>,
    pub worst_after_name: Option<String>,
}

/// One first outage and what followed it.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct FirstOutage {
    pub element: String,
    pub name: String,
    pub kind: &'static str,
    /// Whether the first outage's AC solve converged.
    pub converged: bool,
    pub note: Option<String>,
    /// What the first outage alone leaves over its limits, against the
    /// base case.
    pub overloads: usize,
    pub voltage_violations: usize,
    pub worst: Option<Overload>,
    /// The adjustment, when one ran.
    pub adjustment: Option<AdjustmentSummary>,
    /// Second outages checked from the adjusted case, how many of them
    /// found a problem, and how many didn't converge.
    pub seconds: usize,
    pub seconds_with_problems: usize,
    pub seconds_not_converged: usize,
    /// The worst second outage, by name.
    pub worst_second: Option<String>,
    /// Higher is worse: the worst pair it starts that solved. Null
    /// (infinitely bad) when the first outage's AC solve failed.
    #[schemars(with = "Option<f64>")]
    pub severity: f64,
}

/// One pair: the first outage, the adjustment, then the second.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct PairRow {
    /// Position in the ranked list, from 1.
    pub rank: usize,
    pub first: String,
    pub first_name: String,
    pub second: String,
    pub second_name: String,
    /// "ac" when solved in AC, "linear" when only screened.
    #[schemars(extend("enum" = ["ac", "linear"]))]
    pub checked: &'static str,
    pub converged: Option<bool>,
    pub note: Option<String>,
    /// Overloads new or worse than after the first outage and adjustment.
    pub overloads: usize,
    pub worst: Option<Overload>,
    pub voltage_violations: usize,
    pub worst_voltage: Option<VoltageProblem>,
    pub cut_off_buses: usize,
    pub lost_load_mw: f64,
    /// Higher is worse, as for a contingency row. Null (infinitely bad)
    /// when the AC solve failed.
    #[schemars(with = "Option<f64>")]
    pub severity: f64,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, schemars::JsonSchema)]
pub struct N11Summary {
    pub adjustment: Adjustment,
    pub limit_pct: f64,
    pub rating: Rating,
    pub firsts: usize,
    /// First outages whose AC solve didn't converge (no pairs follow).
    pub firsts_not_converged: usize,
    /// First outages after which the redispatch moved generators.
    pub firsts_adjusted: usize,
    /// First outages the redispatch couldn't fully relieve.
    pub firsts_still_over: usize,
    pub pairs: usize,
    pub pairs_ac: usize,
    pub pairs_with_overloads: usize,
    pub pairs_with_voltage_problems: usize,
    pub pairs_not_converged: usize,
    /// Pairs in `pairs`, the worst ones when there were more.
    pub kept_pairs: usize,
    pub worst: Option<PairRow>,
    pub notes: Vec<String>,
    pub cancelled: bool,
    pub elapsed_ms: f64,
}

#[derive(Clone, Debug, Default, Serialize, schemars::JsonSchema)]
pub struct N11Results {
    pub summary: N11Summary,
    /// Those that don't solve first, then by how many second outages
    /// found a problem.
    pub firsts: Vec<FirstOutage>,
    /// Worst first, at most `keep_pairs`.
    pub pairs: Vec<PairRow>,
}

/// How far an N-1-1 run has got: first outages done, and the second
/// outage run of the one it is on.
#[derive(Clone, Debug, Default, Serialize, schemars::JsonSchema)]
pub struct N11Progress {
    pub firsts_done: usize,
    pub firsts_total: usize,
    pub seconds: Progress,
}

/// Cancels an N-1-1 run and follows it from another thread.
#[derive(Clone, Default)]
pub struct N11Handle {
    run: RunHandle,
    done: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
}

impl N11Handle {
    pub fn cancel(&self) {
        self.run.cancel();
    }

    pub fn progress(&self) -> N11Progress {
        N11Progress {
            firsts_done: self.done.load(Ordering::SeqCst),
            firsts_total: self.total.load(Ordering::SeqCst),
            seconds: self.run.progress(),
        }
    }
}

/// Everything an N-1-1 run needs, copied out of the workspace.
pub struct N11Job {
    power: PowerCase,
    base: PowerFlowResult,
    solve: SolveSettings,
    settings: N11Settings,
    firsts: StepChecker,
    notes: Vec<String>,
    handle: N11Handle,
}

/// The settings for the second outages: one at a time, from the case after
/// the first outage, with the solve the run uses.
fn second_settings(settings: &N11Settings, solve: &SolveSettings) -> ContingencySettings {
    let mut c = settings.contingencies.clone();
    c.pairs = false;
    c.planning = None;
    c.solve = solve.clone();
    c
}

fn severity_of(row: &PairRow) -> f64 {
    row.severity
}

impl N11Job {
    pub fn new(
        power: &PowerCase,
        base: &PowerFlowResult,
        solve: &SolveSettings,
        settings: N11Settings,
    ) -> Result<Self, String> {
        let mut notes = Vec::new();
        if settings.contingencies.pairs {
            notes.push("N-2 pairs aren't used: each second outage goes out on its own.".into());
        }
        if settings.contingencies.planning.is_some() {
            notes.push(
                "Planning events aren't used: the outages come from the element choices.".into(),
            );
        }
        let mut first = second_settings(&settings, solve);
        if let Some(list) = &settings.first {
            if list.is_empty() {
                return Err(
                    "List the first outages, or leave the list out to use the second ones.".into(),
                );
            }
            first.elements = Some(list.clone());
            first.lists = None;
            first.lists_text = None;
        }
        // Every problem after the first outage is reported against the base.
        let firsts = StepChecker::new(power, first)?;
        if firsts.outages.is_empty() {
            return Err("Nothing to take out first: no in-service element fits.".into());
        }
        notes.extend(firsts.notes.iter().cloned());
        let handle = N11Handle::default();
        handle.total.store(firsts.outages.len(), Ordering::SeqCst);
        Ok(Self {
            power: power.clone(),
            base: base.clone(),
            solve: solve.clone(),
            settings,
            firsts,
            notes,
            handle,
        })
    }

    pub fn handle(&self) -> N11Handle {
        self.handle.clone()
    }

    /// First outages in the run.
    pub fn len(&self) -> usize {
        self.firsts.outages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.firsts.outages.is_empty()
    }

    pub fn run(self) -> Result<N11Results, String> {
        let start = Instant::now();
        let handle = &self.handle;
        let seconds = second_settings(&self.settings, &self.solve);
        let mut adjust = self.settings.redispatch.clone();
        adjust.ac_check = true;
        let mut firsts: Vec<FirstOutage> = Vec::new();
        let mut pairs: Vec<PairRow> = Vec::new();
        let mut summary = N11Summary {
            adjustment: self.settings.adjustment,
            limit_pct: seconds.limit_pct,
            rating: seconds.rating,
            firsts: self.firsts.outages.len(),
            ..N11Summary::default()
        };
        let mut notes = self.notes.clone();
        let mut empty_seconds = 0usize;
        let mut adjust_errors: Vec<String> = Vec::new();
        self.firsts
            .each_after(&self.power, &self.base, &handle.run, |_, after| {
                let row = after.row;
                let mut first = FirstOutage {
                    element: row.element.clone(),
                    name: row.name.clone(),
                    kind: row.kind,
                    converged: row.converged == Some(true),
                    note: row.note.clone(),
                    overloads: row.overloads,
                    voltage_violations: row.voltage_violations,
                    worst: after
                        .violations
                        .overloads
                        .first()
                        .cloned()
                        .or(row.worst.clone()),
                    adjustment: None,
                    seconds: 0,
                    seconds_with_problems: 0,
                    seconds_not_converged: 0,
                    worst_second: None,
                    severity: 0.0,
                };
                let Some((power, result)) = after.solved else {
                    summary.firsts_not_converged += 1;
                    first.severity = f64::INFINITY;
                    firsts.push(first);
                    handle.done.fetch_add(1, Ordering::SeqCst);
                    return Ok(!handle.run.cancelled());
                };
                // The system adjustment.
                let (power, result) = match self.settings.adjustment {
                    Adjustment::None => (power, result),
                    Adjustment::Redispatch => {
                        match redispatch::plan_solved(None, &power, &result, &self.solve, &adjust) {
                            Ok((report, moved)) => {
                                let s = &report.summary;
                                if s.units_moved > 0 {
                                    summary.firsts_adjusted += 1;
                                }
                                if s.over_after > 0 {
                                    summary.firsts_still_over += 1;
                                }
                                first.adjustment = Some(AdjustmentSummary {
                                    status: s.status.clone(),
                                    moved_mw: s.moved_mw,
                                    units_moved: s.units_moved,
                                    over_before: s.over_before,
                                    over_after: s.over_after,
                                    worst_after_pct: s.worst_after.as_ref().map(|w| w.pct_after),
                                    worst_after_name: s
                                        .worst_after
                                        .as_ref()
                                        .map(|w| w.name.clone()),
                                });
                                let mut power = power;
                                if s.units_moved > 0 {
                                    power.case = moved.case.clone();
                                }
                                (power, moved)
                            }
                            Err(e) => {
                                adjust_errors.push(format!("{}: {e}", first.name));
                                (power, result)
                            }
                        }
                    }
                };
                if handle.run.cancelled() {
                    return Ok(false);
                }
                // Every second outage, from the adjusted case.
                let job = match ContingencyJob::new(&power, &result, seconds.clone()) {
                    Ok(job) => job.with_handle(handle.run.clone()),
                    Err(_) => {
                        empty_seconds += 1;
                        firsts.push(first);
                        handle.done.fetch_add(1, Ordering::SeqCst);
                        return Ok(true);
                    }
                };
                let results = job.run()?;
                let cancelled = results.summary.cancelled;
                for r in &results.rows {
                    if r.kind == "base" {
                        continue;
                    }
                    first.seconds += 1;
                    if r.overloads > 0 || r.voltage_violations > 0 || r.converged == Some(false) {
                        first.seconds_with_problems += 1;
                    }
                    if r.converged == Some(false) {
                        first.seconds_not_converged += 1;
                    }
                    summary.pairs += 1;
                    if r.checked == "ac" {
                        summary.pairs_ac += 1;
                    }
                    if r.overloads > 0 {
                        summary.pairs_with_overloads += 1;
                    }
                    if r.voltage_violations > 0 {
                        summary.pairs_with_voltage_problems += 1;
                    }
                    if r.converged == Some(false) {
                        summary.pairs_not_converged += 1;
                    }
                    // The rows come ranked, so the first one is the worst.
                    if first.worst_second.is_none() {
                        first.worst_second = Some(r.name.clone());
                    }
                    if r.severity.is_finite() {
                        first.severity = first.severity.max(r.severity);
                    }
                    pairs.push(PairRow {
                        rank: 0,
                        first: first.element.clone(),
                        first_name: first.name.clone(),
                        second: r.element.clone(),
                        second_name: r.name.clone(),
                        checked: r.checked,
                        converged: r.converged,
                        note: r.note.clone(),
                        overloads: r.overloads,
                        worst: r.worst.clone(),
                        voltage_violations: r.voltage_violations,
                        worst_voltage: r.worst_voltage.clone(),
                        cut_off_buses: r.cut_off_buses,
                        lost_load_mw: r.lost_load_mw,
                        severity: r.severity,
                    });
                }
                // Keep memory flat on big runs: only the worst pairs stay.
                let keep = self.settings.keep_pairs;
                if pairs.len() > keep.saturating_mul(2).max(4096) {
                    rank(&mut pairs);
                    pairs.truncate(keep);
                }
                firsts.push(first);
                handle.done.fetch_add(1, Ordering::SeqCst);
                Ok(!cancelled && !handle.run.cancelled())
            })?;

        summary.cancelled = handle.run.cancelled();
        rank(&mut pairs);
        pairs.truncate(self.settings.keep_pairs);
        for (i, p) in pairs.iter_mut().enumerate() {
            p.rank = i + 1;
        }
        // First outages that don't solve, then those that leave the most
        // second outages with a problem.
        firsts.sort_by(|a, b| {
            a.converged
                .cmp(&b.converged)
                .then(b.seconds_with_problems.cmp(&a.seconds_with_problems))
                .then(b.severity.total_cmp(&a.severity))
        });
        summary.kept_pairs = pairs.len();
        summary.worst = pairs.first().cloned();
        if empty_seconds > 0 {
            notes.push(format!(
                "{empty_seconds} first outage{} left no second outage to take.",
                if empty_seconds == 1 { "" } else { "s" }
            ));
        }
        if !adjust_errors.is_empty() {
            let n = adjust_errors.len();
            let mut note = format!(
                "The redispatch failed after {n} first outage{}, which went on unadjusted: ",
                if n == 1 { "" } else { "s" }
            );
            note.push_str(
                &adjust_errors
                    .into_iter()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("; "),
            );
            notes.push(note);
        }
        if summary.firsts_still_over > 0 {
            notes.push(format!(
                "After {} first outage{} the redispatch couldn't bring every branch within its limit; a branch still over is reported only when the second outage makes it worse.",
                summary.firsts_still_over,
                if summary.firsts_still_over == 1 { "" } else { "s" }
            ));
        }
        if summary.pairs > summary.kept_pairs {
            notes.push(format!(
                "Kept the worst {} of {} pairs.",
                summary.kept_pairs, summary.pairs
            ));
        }
        summary.notes = notes;
        summary.elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
        log::info!(
            "N-1-1: {} first outages, {} pairs, {} with overloads in {:.1} s",
            summary.firsts,
            summary.pairs,
            summary.pairs_with_overloads,
            summary.elapsed_ms / 1e3
        );
        Ok(N11Results {
            summary,
            firsts,
            pairs,
        })
    }
}

/// Worst first: AC results ahead of screened ones, then by severity.
fn rank(pairs: &mut [PairRow]) {
    pairs.sort_by(|a, b| {
        (b.checked == "ac")
            .cmp(&(a.checked == "ac"))
            .then(severity_of(b).total_cmp(&severity_of(a)))
    });
}
