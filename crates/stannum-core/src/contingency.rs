//! N-1 contingency analysis: take each branch or generator out on its own and
//! see what overloads or voltage problems follow.
//!
//! The default run screens every outage with linear (DC) outage factors from
//! one factorization of the network, then checks the worst N with a full AC
//! solve that starts from the base solution. Every outage can also be solved
//! in AC. Runs use every core and can be cancelled between outages.
//!
//! Instead of every branch and generator, a run can take the NERC TPL-001
//! planning events [`crate::planning`] generates: each outage is then an
//! event of a category, and can take out shunts and DC lines too.

use crate::error::Error;
use crate::planning::{self, PlanningSettings};
use crate::power::{self, PowerCase, SolveSettings};
use rs_power::{BusType, Case, DcFactors, OutageFlows, PowerFlowResult, SolveStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

/// How each outage is checked.
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Linear screen of every outage, then an AC solve of the worst ones.
    #[default]
    Screen,
    /// An AC solve of every outage.
    Ac,
}

/// Which of a branch's three ratings a loading is measured against.
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    /// Rate A (RAW RATE1): normal, long-term.
    #[default]
    A,
    /// Rate B (RAW RATE2): short-term, often the post-contingency rating.
    B,
    /// Rate C (RAW RATE3): emergency.
    C,
}

impl Rating {
    /// The branch's rating in MVA; zero means unrated.
    pub fn of(self, b: &rs_power::Branch) -> f64 {
        match self {
            Rating::A => b.rate_a,
            Rating::B => b.rate_b,
            Rating::C => b.rate_c,
        }
    }
}

fn yes() -> bool {
    true
}
fn default_ac_count() -> usize {
    50
}
fn default_limit() -> f64 {
    100.0
}
fn default_zero_impedance() -> f64 {
    0.0001
}

/// What to run.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ContingencySettings {
    /// Ids to take out, one at a time. A bus id (bus:101) takes out the
    /// bus, and ids joined by "&" (line:1:2:1&line:2:3:1) go out together.
    /// When absent, every in-service branch, generator and/or bus, as
    /// chosen below.
    #[serde(default)]
    pub elements: Option<Vec<String>>,
    #[serde(default = "yes")]
    pub branches: bool,
    #[serde(default = "yes")]
    pub generators: bool,
    /// Take out each bus: every branch and DC line connected to it, which
    /// cuts off its loads, generators and shunts.
    #[serde(default)]
    pub buses: bool,
    /// N-2: take the chosen branches and generators out two at a time,
    /// every pair once, instead of one at a time. Buses stay one at a time.
    #[serde(default)]
    pub pairs: bool,
    /// Leave out zero-impedance lines: lines with no resistance and a
    /// reactance at or below `zero_impedance_pu`, as PSS/E treats them
    /// (THRSHZ). Applies to N-1 and N-2, also to listed single elements;
    /// a bus outage still opens them.
    #[serde(default)]
    pub skip_zero_impedance: bool,
    /// The zero-impedance threshold in per unit; 0.0001 by default, as
    /// PSS/E's THRSHZ.
    #[serde(default = "default_zero_impedance")]
    pub zero_impedance_pu: f64,
    #[serde(default)]
    pub method: Method,
    /// How many of the worst screened outages get an AC solve.
    #[serde(default = "default_ac_count")]
    pub ac_count: usize,
    /// Loading, as a percent of the rating, above which a branch is
    /// overloaded.
    #[serde(default = "default_limit")]
    pub limit_pct: f64,
    /// The rating loadings are measured against after an outage.
    #[serde(default)]
    pub rating: Rating,
    /// The rating for the base case: its loadings (the "before" figures)
    /// and the P0 event.
    #[serde(default)]
    pub base_rating: Rating,
    #[serde(default)]
    pub solve: SolveSettings,
    /// Run these NERC TPL-001 planning events instead of every branch and
    /// generator; `elements`, `branches` and `generators` are then unused.
    #[serde(default)]
    pub planning: Option<PlanningSettings>,
}

impl Default for ContingencySettings {
    fn default() -> Self {
        Self {
            elements: None,
            branches: true,
            generators: true,
            buses: false,
            pairs: false,
            skip_zero_impedance: false,
            zero_impedance_pu: default_zero_impedance(),
            method: Method::default(),
            ac_count: default_ac_count(),
            limit_pct: default_limit(),
            rating: Rating::default(),
            base_rating: Rating::default(),
            solve: SolveSettings::default(),
            planning: None,
        }
    }
}

/// A branch over its limit after an outage.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Overload {
    pub element: String,
    pub from: u32,
    pub to: u32,
    /// MVA as a percent of the run's rating after the outage, and of its
    /// base rating before it.
    pub pct: f64,
    pub base_pct: f64,
    /// Active flow at the from end, MW, after and before.
    pub mw: f64,
    pub base_mw: f64,
}

/// A bus outside its voltage limits after an outage.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct VoltageProblem {
    pub bus: u32,
    pub name: String,
    pub vm: f64,
    pub base_vm: f64,
    pub vmin: f64,
    pub vmax: f64,
    /// Distance outside the limits, p.u.
    pub outside: f64,
}

/// One outage's outcome, as a row of the ranked results.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ContingencyRow {
    /// Position in the ranked list, from 1.
    pub rank: usize,
    /// The element taken out; empty for the base case (P0).
    pub element: String,
    /// "branch", "generator", "shunt", "dc_line", "bus", "pair" for two
    /// elements out together, or "base" for P0.
    #[schemars(extend("enum" = ["branch", "generator", "shunt", "dc_line", "bus", "pair", "base"]))]
    pub kind: &'static str,
    /// The planning event category, such as "P1.2"; none in an N-1 run.
    pub category: Option<String>,
    /// What the event is, in words, when the element alone doesn't say.
    pub description: Option<String>,
    /// "ac" when solved in AC, "linear" when only screened.
    #[schemars(extend("enum" = ["ac", "linear"]))]
    pub checked: &'static str,
    /// AC only: whether the solve converged, and in how many iterations.
    pub converged: Option<bool>,
    pub iterations: Option<usize>,
    /// The most loaded branch that is over its limit and new or worse than
    /// before the outage; otherwise the most loaded branch.
    pub worst: Option<Overload>,
    /// Branches over the limit that are new or more than a point worse.
    pub overloads: usize,
    /// The `worst` branch's loading from the linear screen.
    pub screened_pct: Option<f64>,
    /// AC only: why a solve failed, in a sentence.
    pub note: Option<String>,
    /// AC only: the worst bus outside its limits that is new or worse.
    pub worst_voltage: Option<VoltageProblem>,
    pub voltage_violations: usize,
    /// Buses split off from the rest of the network, and what they carried.
    pub cut_off_buses: usize,
    pub lost_load_mw: f64,
    pub lost_generation_mw: f64,
    /// Sort key, higher is worse: the worst new or worse overload, or a bus
    /// voltage problem scored on the same scale; infinite when the AC solve
    /// failed.
    pub severity: f64,
}

#[derive(Clone, Debug, Serialize, Default, schemars::JsonSchema)]
pub struct ContingencySummary {
    pub method: Option<Method>,
    pub limit_pct: f64,
    /// The ratings loadings were measured against, after outages and in
    /// the base case.
    pub rating: Rating,
    pub base_rating: Rating,
    /// Outages run, and in-service checks skipped because the element was
    /// already out.
    pub total: usize,
    pub already_out: usize,
    pub ac_checked: usize,
    pub with_overloads: usize,
    pub with_voltage_problems: usize,
    pub not_converged: usize,
    pub islanding: usize,
    pub screen_ms: f64,
    pub ac_ms: f64,
    pub elapsed_ms: f64,
    /// Stopped early: outages after the stop keep their screened result.
    pub cancelled: bool,
    pub worst: Option<ContingencyRow>,
    /// Planning event runs: outages and problems in each category, in
    /// category order. Empty in an N-1 run.
    pub categories: Vec<CategorySummary>,
}

/// One planning event category's share of a run.
#[derive(Clone, Debug, Serialize, Default, PartialEq, schemars::JsonSchema)]
pub struct CategorySummary {
    pub category: String,
    pub total: usize,
    pub with_overloads: usize,
    pub with_voltage_problems: usize,
    pub not_converged: usize,
    pub islanding: usize,
}

/// A branch's flow after one outage, for drawing the change on a diagram.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct BranchChange {
    pub element: String,
    pub from: u32,
    pub to: u32,
    pub mw: f64,
    pub base_mw: f64,
    /// Percent of the run's rating after, and of its base rating before;
    /// `None` when unrated.
    pub pct: Option<f64>,
    pub base_pct: Option<f64>,
    pub tripped: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct BusChange {
    pub bus: u32,
    pub vm: f64,
    pub base_vm: f64,
    pub vmin: f64,
    pub vmax: f64,
}

/// One outage in full: every overload and voltage problem, and the flow and
/// voltage changes worth drawing.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ContingencyDetail {
    pub row: ContingencyRow,
    pub overloads: Vec<Overload>,
    pub voltage_problems: Vec<VoltageProblem>,
    /// Branches whose flow moved by at least half a MW, or that are over the
    /// limit or tripped.
    pub branches: Vec<BranchChange>,
    /// AC only: buses whose voltage moved by at least 0.001 p.u., or that
    /// are outside their limits.
    pub buses: Vec<BusChange>,
    /// Buses split off by the outage.
    pub cut_off: Vec<u32>,
}

/// Progress of the run in hand, for a progress bar.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Progress {
    /// "idle", "screening" or "ac".
    #[schemars(extend("enum" = ["idle", "screening", "ac"]))]
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
}

impl Default for Progress {
    /// No run in hand.
    fn default() -> Self {
        Self {
            phase: "idle",
            done: 0,
            total: 0,
        }
    }
}

/// Cancels a run and reports its progress from another thread.
#[derive(Clone, Default)]
pub struct RunHandle(Arc<RunState>);

#[derive(Default)]
struct RunState {
    /// Shared with each AC solve, so cancelling stops one mid-solve too.
    cancel: Arc<AtomicBool>,
    phase: AtomicUsize,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl RunHandle {
    /// Ask the run to stop after the outages it is on.
    pub fn cancel(&self) {
        self.0.cancel.store(true, Ordering::SeqCst);
    }

    pub fn progress(&self) -> Progress {
        Progress {
            phase: match self.0.phase.load(Ordering::SeqCst) {
                1 => "screening",
                2 => "ac",
                _ => "idle",
            },
            done: self.0.done.load(Ordering::SeqCst),
            total: self.0.total.load(Ordering::SeqCst),
        }
    }

    fn set_phase(&self, phase: usize, total: usize) {
        self.0.phase.store(phase, Ordering::SeqCst);
        self.0.done.store(0, Ordering::SeqCst);
        self.0.total.store(total, Ordering::SeqCst);
    }

    fn cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug, Default)]
struct Outage {
    element: String,
    kind: &'static str,
    branch_rows: Vec<usize>,
    generator_rows: Vec<usize>,
    /// Rows of `PowerCase::shunts`.
    shunt_rows: Vec<usize>,
    /// `case.dcline` rows with the share of the schedule that stays.
    dc_rows: Vec<(usize, f64)>,
    category: Option<&'static str>,
    description: Option<String>,
}

impl Outage {
    /// P0: judged against the limits, not against the base case.
    fn is_base(&self) -> bool {
        self.kind == "base"
    }
}

/// The MVA at 1 p.u. a shunt outage takes off, for choosing which shunts to
/// solve in AC.
fn shunt_mvar(power: &PowerCase, outage: &Outage) -> f64 {
    outage
        .shunt_rows
        .iter()
        .map(|&r| power.shunts[r].g.hypot(power.shunts[r].b))
        .sum()
}

impl From<planning::PlanningEvent> for Outage {
    fn from(e: planning::PlanningEvent) -> Self {
        Self {
            element: e.element,
            kind: e.kind,
            branch_rows: e.branch_rows,
            generator_rows: e.generator_rows,
            shunt_rows: e.shunt_rows,
            dc_rows: e.dc_rows,
            category: Some(e.category),
            description: e.description,
        }
    }
}

/// Everything a run needs, copied out of the workspace so the run doesn't
/// hold it.
pub struct ContingencyJob {
    power: PowerCase,
    base: PowerFlowResult,
    settings: ContingencySettings,
    outages: Vec<Outage>,
    already_out: usize,
    handle: RunHandle,
}

/// Pick the outages `settings` asks for from `power`.
fn outages(
    power: &PowerCase,
    settings: &ContingencySettings,
) -> Result<(Vec<Outage>, usize), String> {
    if let Some(p) = &settings.planning {
        let list: Vec<Outage> = planning::events(power, p)?
            .into_iter()
            .map(Outage::from)
            .filter(|o| !zero_impedance_line(&power.case, settings, o))
            .collect();
        return Ok((list, 0));
    }
    let case = &power.case;
    // Every single outage the case offers, by id: branches (a three-winding
    // transformer is one element over its windings), generators and buses.
    let mut singles: Vec<Outage> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    fn add(
        singles: &mut Vec<Outage>,
        index: &mut HashMap<String, usize>,
        element: &str,
        kind: &'static str,
    ) -> usize {
        *index.entry(element.to_string()).or_insert_with(|| {
            singles.push(Outage {
                element: element.to_string(),
                kind,
                ..Default::default()
            });
            singles.len() - 1
        })
    }
    for (row, e) in power.elements.branches.iter().enumerate() {
        let i = add(&mut singles, &mut index, &e.element, "branch");
        singles[i].branch_rows.push(row);
    }
    for (row, e) in power.elements.generators.iter().enumerate() {
        let i = add(&mut singles, &mut index, &e.element, "generator");
        singles[i].generator_rows.push(row);
    }
    let wanted: Option<Vec<&str>> = settings
        .elements
        .as_ref()
        .map(|e| e.iter().map(String::as_str).collect());
    let wants_buses = settings.buses
        || wanted
            .iter()
            .flatten()
            .any(|w| w.split('&').any(|p| p.starts_with("bus:")));
    if wants_buses {
        let mut at: HashMap<u32, Outage> = HashMap::new();
        for (r, b) in case.branches.iter().enumerate() {
            at.entry(b.from).or_default().branch_rows.push(r);
            if b.to != b.from {
                at.entry(b.to).or_default().branch_rows.push(r);
            }
        }
        for (r, l) in case.dcline.iter().flatten().enumerate() {
            let (from, to) = (l[0] as u32, l[1] as u32);
            at.entry(from).or_default().dc_rows.push((r, 0.0));
            if to != from {
                at.entry(to).or_default().dc_rows.push((r, 0.0));
            }
        }
        for bus in &case.buses {
            let i = add(
                &mut singles,
                &mut index,
                &crate::elements::bus(bus.id),
                "bus",
            );
            let found = at.remove(&bus.id).unwrap_or_default();
            singles[i].branch_rows = found.branch_rows;
            singles[i].dc_rows = found.dc_rows;
        }
    }
    let in_service = |o: &Outage| {
        o.branch_rows.iter().any(|&r| case.branches[r].online)
            || o.generator_rows.iter().any(|&r| case.generators[r].online)
            || o.dc_rows.iter().any(|&(r, _)| {
                case.dcline
                    .as_ref()
                    .and_then(|t| t.get(r))
                    .is_some_and(|l| l[2] != 0.0)
            })
    };

    let zero_impedance = |o: &Outage| zero_impedance_line(case, settings, o);

    // The outages asked for, one element (or one "&" group) each.
    let mut list: Vec<Outage> = Vec::new();
    match &wanted {
        Some(w) => {
            for id in w {
                if !id.contains('&')
                    && let Some(&i) = index.get(*id)
                    && zero_impedance(&singles[i])
                {
                    continue;
                }
                let mut parts = Vec::new();
                for part in id.split('&') {
                    let i = *index.get(part).ok_or_else(|| {
                        format!("{part} isn't a branch, generator or bus in this case.")
                    })?;
                    parts.push(&singles[i]);
                }
                list.push(combine(&parts));
            }
        }
        None => {
            for o in &singles {
                // Switching devices run only when listed: opening one is a
                // switching study, not a branch outage.
                let chosen = match o.kind {
                    "branch" => {
                        settings.branches && !o.element.starts_with("switch:") && !zero_impedance(o)
                    }
                    "generator" => settings.generators,
                    _ => false,
                };
                if chosen {
                    list.push(o.clone());
                }
            }
        }
    }
    if settings.pairs {
        let (single_ones, buses): (Vec<Outage>, Vec<Outage>) =
            list.into_iter().partition(|o| o.kind != "bus");
        let usable: Vec<Outage> = single_ones.into_iter().filter(&in_service).collect();
        let n = usable.len();
        let pairs = n * n.saturating_sub(1) / 2;
        if pairs > MAX_PAIRS {
            return Err(format!(
                "N-2 of {n} elements is {pairs} pairs. Narrow the outages with the bus filters to {MAX_PAIRS} pairs or fewer (about {} elements).",
                max_pair_elements()
            ));
        }
        list = Vec::with_capacity(pairs + buses.len());
        for a in 0..n {
            for b in a + 1..n {
                list.push(combine(&[&usable[a], &usable[b]]));
            }
        }
        list.extend(buses);
    } else if settings.buses && wanted.is_none() {
        list.extend(singles.iter().filter(|o| o.kind == "bus").cloned());
    }
    let before = list.len();
    list.retain(in_service);
    let already_out = before - list.len();
    Ok((list, already_out))
}

/// A zero-impedance line, when the settings leave them out: no resistance
/// and a reactance at or below the threshold (PSS/E's THRSHZ).
/// Transformers never count.
fn zero_impedance_line(case: &Case, settings: &ContingencySettings, o: &Outage) -> bool {
    settings.skip_zero_impedance
        && o.kind == "branch"
        && o.element.starts_with("line:")
        && !o.branch_rows.is_empty()
        && o.branch_rows.iter().all(|&r| {
            let b = &case.branches[r];
            b.r == 0.0 && b.x.abs() <= settings.zero_impedance_pu
        })
}

/// N-2 runs at most this many pairs: each is a row of the results.
pub const MAX_PAIRS: usize = 250_000;

/// The most elements whose pairs fit in [`MAX_PAIRS`].
fn max_pair_elements() -> usize {
    let mut n = 1;
    while (n + 1) * n / 2 <= MAX_PAIRS {
        n += 1;
    }
    n
}

/// Several outages taken together, as one.
fn combine(parts: &[&Outage]) -> Outage {
    if let [one] = parts {
        return (*one).clone();
    }
    let mut out = Outage {
        element: parts
            .iter()
            .map(|p| p.element.as_str())
            .collect::<Vec<_>>()
            .join("&"),
        kind: "pair",
        ..Default::default()
    };
    for p in parts {
        for &r in &p.branch_rows {
            if !out.branch_rows.contains(&r) {
                out.branch_rows.push(r);
            }
        }
        for &r in &p.generator_rows {
            if !out.generator_rows.contains(&r) {
                out.generator_rows.push(r);
            }
        }
        out.shunt_rows.extend(&p.shunt_rows);
        out.dc_rows.extend(&p.dc_rows);
    }
    out
}

impl ContingencyJob {
    /// A run over `power` (the case with the study's edits) from its
    /// converged AC solve `base`.
    pub fn new(
        power: &PowerCase,
        base: &PowerFlowResult,
        settings: ContingencySettings,
    ) -> Result<Self, String> {
        let (outages, already_out) = outages(power, &settings)?;
        if outages.is_empty() {
            return Err(if settings.planning.is_some() {
                "Nothing to take out: no in-service element fits the planning event filters.".into()
            } else {
                "Nothing to take out: the list has no in-service branches or generators.".into()
            });
        }
        Ok(Self {
            power: power.clone(),
            base: base.clone(),
            settings,
            outages,
            already_out,
            handle: RunHandle::default(),
        })
    }

    /// For cancelling the run, or following it, from another thread.
    pub fn handle(&self) -> RunHandle {
        self.handle.clone()
    }

    pub fn len(&self) -> usize {
        self.outages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outages.is_empty()
    }

    /// Screen, then check in AC, using every core. Stops early (keeping what
    /// is done) when [`cancel`] is called.
    pub fn run(self) -> Result<ContingencyResults, String> {
        let handle = &self.handle;
        let start = Instant::now();
        let context = Context::new(&self.power, &self.base, &self.settings)?;
        let n = self.outages.len();

        handle.set_phase(1, n);
        let mut rows: Vec<Option<ContingencyRow>> =
            parallel(handle, n, |i| Some(context.screen(&self.outages[i]).row));
        let screen_ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut was_cancelled = rows.iter().any(Option::is_none);

        let ac_start = Instant::now();
        let mut ac_checked = 0;
        if !was_cancelled {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| severity(&rows[b]).total_cmp(&severity(&rows[a])));
            let count = match self.settings.method {
                Method::Ac => n,
                Method::Screen => self.settings.ac_count.min(n),
            };
            // The screen can't judge a shunt outage, which moves voltages
            // rather than MW: the largest shunts get their own AC budget of
            // the same size. The base case (P0) is always solved.
            let (shunts, screened): (Vec<usize>, Vec<usize>) = order
                .iter()
                .copied()
                .filter(|&i| !self.outages[i].is_base())
                .partition(|&i| !self.outages[i].shunt_rows.is_empty());
            let mut chosen: Vec<usize> = (0..n).filter(|&i| self.outages[i].is_base()).collect();
            let mut shunts = shunts;
            shunts.sort_by(|&a, &b| {
                shunt_mvar(&self.power, &self.outages[b])
                    .total_cmp(&shunt_mvar(&self.power, &self.outages[a]))
            });
            chosen.extend(screened.iter().take(count));
            chosen.extend(shunts.iter().take(count));
            let count = chosen.len();
            handle.set_phase(2, count);
            let solved = parallel(handle, count, |k| {
                context
                    .ac(&self.outages[chosen[k]], &self.settings.solve, handle)
                    .map(|d| d.row)
            });
            for (k, row) in solved.into_iter().enumerate() {
                match row {
                    Some(row) => {
                        rows[chosen[k]] = Some(row);
                        ac_checked += 1;
                    }
                    None => was_cancelled = true,
                }
            }
        }
        handle.set_phase(0, 0);
        let ac_ms = ac_start.elapsed().as_secs_f64() * 1000.0;

        // A cancelled screen leaves gaps; those outages aren't reported.
        let mut ranked: Vec<(usize, ContingencyRow)> = rows
            .into_iter()
            .enumerate()
            .filter_map(|(i, r)| r.map(|r| (i, r)))
            .collect();
        ranked.sort_by(|a, b| {
            // AC results first, then by severity.
            (b.1.checked == "ac")
                .cmp(&(a.1.checked == "ac"))
                .then(b.1.severity.total_cmp(&a.1.severity))
        });
        for (rank, (_, row)) in ranked.iter_mut().enumerate() {
            row.rank = rank + 1;
        }
        let summary = ContingencySummary {
            method: Some(self.settings.method),
            limit_pct: self.settings.limit_pct,
            rating: self.settings.rating,
            base_rating: self.settings.base_rating,
            total: ranked.len(),
            already_out: self.already_out,
            ac_checked,
            with_overloads: ranked.iter().filter(|(_, r)| r.overloads > 0).count(),
            with_voltage_problems: ranked
                .iter()
                .filter(|(_, r)| r.voltage_violations > 0)
                .count(),
            not_converged: ranked
                .iter()
                .filter(|(_, r)| r.converged == Some(false))
                .count(),
            islanding: ranked.iter().filter(|(_, r)| r.cut_off_buses > 0).count(),
            screen_ms,
            ac_ms,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            cancelled: was_cancelled,
            worst: ranked.first().map(|(_, r)| r.clone()),
            categories: category_summaries(&ranked),
        };
        log_summary(&summary);
        let (order, rows): (Vec<usize>, Vec<ContingencyRow>) = ranked.into_iter().unzip();
        let outages = order.iter().map(|&i| self.outages[i].clone()).collect();
        Ok(ContingencyResults {
            summary,
            rows,
            outages,
            power: self.power,
            base: self.base,
            settings: self.settings,
            id: next_run_id(),
            last_detail: None,
            last_solution: None,
            last_report: None,
        })
    }
}

/// Each category's outages and problems, in the order planning lists them.
fn category_summaries(ranked: &[(usize, ContingencyRow)]) -> Vec<CategorySummary> {
    let mut out: Vec<CategorySummary> = planning::CATEGORIES
        .iter()
        .map(|(c, _)| CategorySummary {
            category: c.to_string(),
            ..Default::default()
        })
        .collect();
    for (_, r) in ranked {
        let Some(c) = &r.category else { continue };
        let Some(s) = out.iter_mut().find(|s| &s.category == c) else {
            continue;
        };
        s.total += 1;
        s.with_overloads += usize::from(r.overloads > 0);
        s.with_voltage_problems += usize::from(r.voltage_violations > 0);
        s.not_converged += usize::from(r.converged == Some(false));
        s.islanding += usize::from(r.cut_off_buses > 0);
    }
    out.retain(|s| s.total > 0);
    out
}

/// Under one point of severity, so it only orders outages with the same
/// worst loading (such as every outage of a case without ratings): the MW
/// they cut off, then the largest flow change they cause.
fn tie_break(lost_mw: f64, branches: &[BranchChange]) -> f64 {
    let moved = branches
        .iter()
        .filter(|b| !b.tripped)
        .map(|b| (b.mw - b.base_mw).abs())
        .fold(0.0, f64::max);
    ((lost_mw.abs() + moved) / 10_000.0).min(0.999)
}

fn severity(row: &Option<ContingencyRow>) -> f64 {
    row.as_ref().map_or(f64::NEG_INFINITY, |r| r.severity)
}

fn log_summary(s: &ContingencySummary) {
    let mut line = format!(
        "Contingencies: {} outages, {} solved in AC; {} with overloads, {} with voltage problems",
        s.total, s.ac_checked, s.with_overloads, s.with_voltage_problems
    );
    if s.not_converged > 0 {
        line += &format!(", {} not converged", s.not_converged);
    }
    if s.islanding > 0 {
        line += &format!(", {} split the network", s.islanding);
    }
    line += &format!(" (screen {:.0} ms, AC {:.0} ms)", s.screen_ms, s.ac_ms);
    if s.cancelled {
        log::warn!("{line}; cancelled before the end");
    } else if s.with_overloads + s.with_voltage_problems + s.not_converged > 0 {
        log::warn!("{line}");
    } else {
        log::info!("{line}");
    }
}

/// Run `f` for 0..n on every core, in any order, returning results in
/// order. A result is `None` when the run was cancelled before it.
fn parallel<T: Send>(
    handle: &RunHandle,
    n: usize,
    f: impl Fn(usize) -> Option<T> + Sync,
) -> Vec<Option<T>> {
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(1, |p| p.get())
        .min(n.max(1));
    let mut out: Vec<Option<T>> = (0..n).map(|_| None).collect();
    let results: Vec<Vec<(usize, Option<T>)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        if handle.cancelled() {
                            break;
                        }
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        if i >= n {
                            break;
                        }
                        mine.push((i, f(i)));
                        handle.0.done.fetch_add(1, Ordering::SeqCst);
                    }
                    mine
                })
            })
            .collect();
        // A worker that panicked takes the run down with it, so the command
        // fails as an internal error instead of reporting a partial run.
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    });
    for (i, value) in results.into_iter().flatten() {
        out[i] = value;
    }
    out
}

/// The base case, its solution and the linear factors, shared by every
/// outage of a run.
struct Context<'a> {
    power: &'a PowerCase,
    base: &'a PowerFlowResult,
    factors: DcFactors,
    limit_pct: f64,
    rating: Rating,
    base_rating: Rating,
    /// Base flows at the from and to ends.
    pf: Vec<f64>,
    qf: Vec<f64>,
    pt: Vec<f64>,
    qt: Vec<f64>,
    base_pct: Vec<Option<f64>>,
    names: Vec<String>,
    bus_row: HashMap<u32, usize>,
}

struct Outcome {
    row: ContingencyRow,
    overloads: Vec<Overload>,
    voltage_problems: Vec<VoltageProblem>,
    branches: Vec<BranchChange>,
    buses: Vec<BusChange>,
    cut_off: Vec<u32>,
}

/// Loading of branch row `r`, measured as in the solve report.
fn loading(power: &PowerCase, r: usize, pf: f64, qf: f64, pt: f64, qt: f64, rate: f64) -> f64 {
    let winding = power.elements.branches.get(r).map_or(0, |e| e.winding);
    power::loading_pct(pf, qf, pt, qt, rate, winding)
}

/// A branch is new or worse when it is over the limit now and was under it,
/// or has grown by more than a point.
fn new_or_worse(pct: f64, base: f64, limit: f64) -> bool {
    pct > limit && (base <= limit || pct > base + 1.0)
}

impl<'a> Context<'a> {
    fn new(
        power: &'a PowerCase,
        base: &'a PowerFlowResult,
        settings: &ContingencySettings,
    ) -> Result<Self, String> {
        let base_rating = settings.base_rating;
        let factors = DcFactors::new(&power.case).map_err(|e| e.to_string())?;
        let flows = &base.branch_flows;
        let case = &base.case;
        Ok(Self {
            power,
            base,
            factors,
            limit_pct: settings.limit_pct,
            rating: settings.rating,
            base_rating,
            pf: flows.iter().map(|f| f[0]).collect(),
            qf: flows.iter().map(|f| f[1]).collect(),
            pt: flows.iter().map(|f| f[2]).collect(),
            qt: flows.iter().map(|f| f[3]).collect(),
            base_pct: case
                .branches
                .iter()
                .zip(flows)
                .enumerate()
                .map(|(r, (b, f))| {
                    let rate = base_rating.of(b);
                    (b.online && rate > 0.0)
                        .then(|| loading(power, r, f[0], f[1], f[2], f[3], rate))
                })
                .collect(),
            names: power::bus_names(case),
            bus_row: case
                .buses
                .iter()
                .enumerate()
                .map(|(i, b)| (b.id, i))
                .collect(),
        })
    }

    fn case(&self) -> &Case {
        &self.power.case
    }

    /// The base case (P0) is held to the base rating; outages to the run's.
    fn rating_for(&self, base_case: bool) -> Rating {
        if base_case {
            self.base_rating
        } else {
            self.rating
        }
    }

    fn element(&self, row: usize) -> String {
        self.power
            .elements
            .branches
            .get(row)
            .map(|e| e.element.clone())
            .unwrap_or_default()
    }

    /// What the cut-off buses carried: load, and online generation not
    /// already taken out.
    fn lost(&self, cut: &[usize], outage: &Outage) -> (f64, f64) {
        let case = self.case();
        let load = cut.iter().map(|&b| case.buses[b].load().0).sum();
        let ids: std::collections::HashSet<u32> = cut.iter().map(|&b| case.buses[b].id).collect();
        let generation = case
            .generators
            .iter()
            .enumerate()
            .filter(|(i, g)| g.online && ids.contains(&g.bus) && !outage.generator_rows.contains(i))
            .map(|(_, g)| g.pg)
            .sum();
        (load, generation)
    }

    /// Linear estimate of every branch's from-end MW after the outage, and
    /// the bus rows it cuts off.
    fn linear_flows(&self, outage: &Outage) -> (Vec<f64>, Vec<usize>) {
        let case = self.case();
        let mut flows = self.pf.clone();
        // Generators taken out: their output moves to the reference bus.
        let mut lost: Vec<(u32, f64)> = outage
            .generator_rows
            .iter()
            .filter(|&&r| case.generators[r].online)
            .map(|&r| (case.generators[r].bus, -self.base.case.generators[r].pg))
            .collect();
        lost.extend(self.other_injections(outage));
        if !lost.is_empty() {
            for (f, d) in flows.iter_mut().zip(self.factors.injection_flows(&lost)) {
                *f += d;
            }
        }
        let rows: Vec<usize> = outage
            .branch_rows
            .iter()
            .copied()
            .filter(|&r| self.factors.in_service(r))
            .collect();
        if rows.is_empty() {
            return (flows, vec![]);
        }
        match self.factors.trip(&rows, &flows) {
            OutageFlows::Flows(after) => (after, vec![]),
            OutageFlows::Islands(cut) => {
                let ids: std::collections::HashSet<u32> =
                    cut.iter().map(|&b| case.buses[b].id).collect();
                let in_cut = |id: u32| ids.contains(&id);
                // What each boundary branch delivered to the part that stays
                // is lost there, and the bus taking up the slack makes it up.
                let pickup = self.slack_bus(&ids);
                let mut injections = Vec::new();
                let mut inside = Vec::new();
                for &r in &rows {
                    let b = &case.branches[r];
                    match (in_cut(b.from), in_cut(b.to)) {
                        (true, false) => injections.push((b.to, -flows[r])),
                        (false, true) => injections.push((b.from, flows[r])),
                        (false, false) => inside.push(r),
                        (true, true) => {}
                    }
                }
                if let Some(pickup) = pickup {
                    let total: f64 = injections.iter().map(|(_, mw)| mw).sum();
                    injections.push((pickup, -total));
                }
                for (f, d) in flows
                    .iter_mut()
                    .zip(self.factors.injection_flows(&injections))
                {
                    *f += d;
                }
                for (r, b) in case.branches.iter().enumerate() {
                    if in_cut(b.from) || in_cut(b.to) {
                        flows[r] = 0.0;
                    }
                }
                if !inside.is_empty()
                    && let OutageFlows::Flows(after) = self.factors.trip(&inside, &flows)
                {
                    flows = after;
                }
                for &r in &rows {
                    flows[r] = 0.0;
                }
                (flows, cut)
            }
        }
    }

    /// How a shunt or DC line outage moves each bus's MW injection, for the
    /// linear screen. A shunt only moves its conductance; a DC line's
    /// terminals move by the scheduled flow given up, net of losses at the
    /// receiving end.
    fn other_injections(&self, outage: &Outage) -> Vec<(u32, f64)> {
        let mut out = Vec::new();
        let vm: HashMap<u32, f64> = self.base.case.buses.iter().map(|b| (b.id, b.vm)).collect();
        for &r in &outage.shunt_rows {
            let s = &self.power.shunts[r];
            if s.g != 0.0 {
                let v = vm.get(&s.bus).copied().unwrap_or(1.0);
                out.push((s.bus, s.g * v * v));
            }
        }
        let solved = self
            .base
            .case
            .dcline
            .as_ref()
            .or(self.case().dcline.as_ref());
        for &(r, keep) in &outage.dc_rows {
            let Some(line) = solved.and_then(|t| t.get(r)) else {
                continue;
            };
            if line[2] == 0.0 {
                continue;
            }
            let pf = line[3];
            let delivered = |p: f64| p - line[15] - line[16] * p;
            let to_change = if keep == 0.0 {
                -delivered(pf)
            } else {
                delivered(keep * pf) - delivered(pf)
            };
            out.push((line[0] as u32, (1.0 - keep) * pf));
            out.push((line[1] as u32, to_change));
        }
        out
    }

    /// The bus that takes up the slack in the part of the network that stays:
    /// the reference bus, or if it was cut off, the bus of the largest online
    /// generator left in its island.
    fn slack_bus(&self, cut: &std::collections::HashSet<u32>) -> Option<u32> {
        let case = self.case();
        let reference = case
            .buses
            .iter()
            .find(|b| b.kind == BusType::Reference && !cut.contains(&b.id));
        if let Some(r) = reference {
            return Some(r.id);
        }
        case.generators
            .iter()
            .filter(|g| g.online && !cut.contains(&g.bus))
            .max_by(|a, b| a.pmax.total_cmp(&b.pmax))
            .map(|g| g.bus)
    }

    fn screen(&self, outage: &Outage) -> Outcome {
        let case = self.case();
        let (flows, cut) = self.linear_flows(outage);
        let absolute = outage.is_base();
        let rating = self.rating_for(absolute);
        let mut overloads = Vec::new();
        let mut branches = Vec::new();
        let mut worst: Option<Overload> = None;
        for (r, b) in case.branches.iter().enumerate() {
            let rate = rating.of(b);
            let tripped = outage.branch_rows.contains(&r);
            let base_pct = self.base_pct[r];
            // The linear model only moves active power: both ends move by
            // the same MW, and Q and losses stay as they were.
            let delta = flows[r] - self.pf[r];
            let pct = (!tripped && b.online && rate > 0.0).then(|| {
                loading(
                    self.power,
                    r,
                    self.pf[r] + delta,
                    self.qf[r],
                    self.pt[r] - delta,
                    self.qt[r],
                    rate,
                )
            });
            if let (Some(pct), Some(base)) = (pct, base_pct) {
                let o = Overload {
                    element: self.element(r),
                    from: b.from,
                    to: b.to,
                    pct,
                    base_pct: base,
                    mw: flows[r],
                    base_mw: self.pf[r],
                };
                if worst.as_ref().is_none_or(|w| pct > w.pct) {
                    worst = Some(o.clone());
                }
                if new_or_worse(pct, if absolute { 0.0 } else { base }, self.limit_pct) {
                    overloads.push(o);
                }
            }
            if b.online
                && ((flows[r] - self.pf[r]).abs() >= 0.5
                    || tripped
                    || pct.is_some_and(|p| p > self.limit_pct))
            {
                branches.push(BranchChange {
                    element: self.element(r),
                    from: b.from,
                    to: b.to,
                    mw: flows[r],
                    base_mw: self.pf[r],
                    pct,
                    base_pct,
                    tripped,
                });
            }
        }
        overloads.sort_by(|a, b| b.pct.total_cmp(&a.pct));
        let shown = overloads.first().cloned().or(worst);
        let screened = shown.as_ref().map(|w| w.pct);
        let (lost_load, lost_gen) = self.lost(&cut, outage);
        let row = ContingencyRow {
            rank: 0,
            element: outage.element.clone(),
            kind: outage.kind,
            category: outage.category.map(str::to_string),
            description: outage.description.clone(),
            checked: "linear",
            converged: None,
            iterations: None,
            severity: overloads.first().map_or(0.0, |o| o.pct)
                + tie_break(lost_load + lost_gen, &branches),
            worst: shown,
            overloads: overloads.len(),
            screened_pct: screened,
            note: None,
            worst_voltage: None,
            voltage_violations: 0,
            cut_off_buses: cut.len(),
            lost_load_mw: lost_load,
            lost_generation_mw: lost_gen,
        };
        Outcome {
            row,
            overloads,
            voltage_problems: vec![],
            branches,
            buses: vec![],
            cut_off: cut.iter().map(|&b| case.buses[b].id).collect(),
        }
    }

    /// The case with the outage applied, ready for an AC solve: split-off
    /// buses isolated, and a new reference bus if the old one lost its
    /// generators or was split off.
    fn outage_case(&self, outage: &Outage, cut: &[usize]) -> Case {
        let mut case = self.case().clone();
        for &r in &outage.branch_rows {
            case.branches[r].online = false;
        }
        for &r in &outage.generator_rows {
            case.generators[r].online = false;
        }
        // Shunts in voltage control come off after the warm start, which
        // moves them to their solved step first.
        for &r in &outage.shunt_rows {
            let s = &self.power.shunts[r];
            if self.controlled_shunt(r).is_some() {
                continue;
            }
            if let Some(&row) = self.bus_row.get(&s.bus) {
                case.buses[row].gs -= s.g;
                case.buses[row].bs -= s.b;
            }
        }
        if let Some(table) = &mut case.dcline {
            for &(r, keep) in &outage.dc_rows {
                if let Some(line) = table.get_mut(r) {
                    if keep == 0.0 {
                        line[2] = 0.0;
                    } else {
                        line[3] *= keep;
                    }
                }
            }
        }
        let ids: std::collections::HashSet<u32> = cut.iter().map(|&b| case.buses[b].id).collect();
        for &b in cut {
            case.buses[b].kind = BusType::Isolated;
        }
        for g in &mut case.generators {
            if ids.contains(&g.bus) {
                g.online = false;
            }
        }
        let has_generator =
            |case: &Case, bus: u32| case.generators.iter().any(|g| g.online && g.bus == bus);
        let reference = case.buses.iter().position(|b| b.kind == BusType::Reference);
        let needs_new = match reference {
            Some(r) => !has_generator(&case, case.buses[r].id),
            None => true,
        };
        if needs_new
            && let Some(bus) = case
                .generators
                .iter()
                .filter(|g| g.online)
                .max_by(|a, b| a.pmax.total_cmp(&b.pmax))
                .map(|g| g.bus)
        {
            if let Some(r) = reference {
                case.buses[r].kind = BusType::Pq;
            }
            if let Some(&row) = self.bus_row.get(&bus) {
                case.buses[row].kind = BusType::Reference;
            }
        }
        case
    }

    /// The voltage control entry of shunt row `r`, if it is in one.
    fn controlled_shunt(&self, r: usize) -> Option<usize> {
        let s = &self.power.shunts[r];
        if s.kind != rs_power::ShuntKind::Switched || s.line == 0 {
            return None;
        }
        self.power
            .voltage_controls
            .switched_shunts
            .iter()
            .position(|c| c.line == s.line && c.bus == s.bus)
    }

    /// Solve the outage in AC from the base solution. `None` only when the
    /// run was cancelled first.
    fn ac(&self, outage: &Outage, settings: &SolveSettings, handle: &RunHandle) -> Option<Outcome> {
        self.ac_solved(outage, settings, handle).map(|(o, _)| o)
    }

    /// [`Self::ac`], keeping the solution too when the solve converged.
    fn ac_solved(
        &self,
        outage: &Outage,
        settings: &SolveSettings,
        handle: &RunHandle,
    ) -> Option<(Outcome, Option<PowerFlowResult>)> {
        if handle.cancelled() {
            return None;
        }
        let screened = self.screen(outage);
        let cut: Vec<usize> = screened
            .cut_off
            .iter()
            .filter_map(|id| self.bus_row.get(id).copied())
            .collect();
        let case = self.outage_case(outage, &cut);
        let mut options = settings.options_for(self.power);
        options.cancel = Some(handle.0.cancel.clone());
        let mut warm = power::warm_start(&case, self.base, &mut options);
        for &r in &outage.shunt_rows {
            if let Some(k) = self.controlled_shunt(r) {
                // Off at whatever step it was on, and nothing left to switch.
                let control = &mut options.voltage_controls.switched_shunts[k];
                if let Some(&row) = self.bus_row.get(&control.bus) {
                    warm.buses[row].bs -= control.initial;
                }
                control.initial = 0.0;
                control.blocks.clear();
            }
        }
        if let rs_power::DcModel::Detailed { lines, .. } = &mut options.dc_model {
            for &(r, keep) in &outage.dc_rows {
                if keep > 0.0
                    && let Some(line) = lines.iter_mut().find(|l| l.row == r)
                {
                    line.setpoint *= keep;
                }
            }
        }
        let mut row = screened.row.clone();
        row.checked = "ac";
        let result = match rs_power::solve(&warm, &options) {
            Ok(r) => r,
            Err(rs_power::Error::Cancelled) => return None,
            Err(e) => {
                log::debug!("{}: AC solve failed: {e}", outage.element);
                row.converged = Some(false);
                row.severity = f64::INFINITY;
                row.note = Some(format!("The AC solve failed: {e}"));
                return Some((Outcome { row, ..screened }, None));
            }
        };
        let converged = result.status == SolveStatus::Converged;
        row.converged = Some(converged);
        row.iterations = Some(result.iterations);
        if !converged {
            row.severity = f64::INFINITY;
            // Say whether generator Q limits are what stops it.
            row.note = Some(if options.enforce_q_limits {
                options.enforce_q_limits = false;
                match rs_power::solve(&warm, &options) {
                    Ok(r) if r.status == SolveStatus::Converged => "Converges only without generator Q limits: too little reactive support is left.".into(),
                    _ => "Doesn't converge, even without generator Q limits.".into(),
                }
            } else {
                "Doesn't converge.".into()
            });
            return Some((Outcome { row, ..screened }, None));
        }
        let solved = &result.case;
        let absolute = outage.is_base();
        let rating = self.rating_for(absolute);
        let mut overloads = Vec::new();
        let mut branches = Vec::new();
        let mut worst: Option<Overload> = None;
        for (r, (b, f)) in solved.branches.iter().zip(&result.branch_flows).enumerate() {
            let tripped = outage.branch_rows.contains(&r);
            let base_pct = self.base_pct[r];
            let rate = rating.of(b);
            let pct = (b.online && rate > 0.0)
                .then(|| loading(self.power, r, f[0], f[1], f[2], f[3], rate));
            if let (Some(pct), Some(base)) = (pct, base_pct) {
                let o = Overload {
                    element: self.element(r),
                    from: b.from,
                    to: b.to,
                    pct,
                    base_pct: base,
                    mw: f[0],
                    base_mw: self.pf[r],
                };
                if worst.as_ref().is_none_or(|w| pct > w.pct) {
                    worst = Some(o.clone());
                }
                if new_or_worse(pct, if absolute { 0.0 } else { base }, self.limit_pct) {
                    overloads.push(o);
                }
            }
            let was_online = self.case().branches[r].online;
            if was_online
                && ((f[0] - self.pf[r]).abs() >= 0.5
                    || tripped
                    || !b.online
                    || pct.is_some_and(|p| p > self.limit_pct))
            {
                branches.push(BranchChange {
                    element: self.element(r),
                    from: b.from,
                    to: b.to,
                    mw: if b.online { f[0] } else { 0.0 },
                    base_mw: self.pf[r],
                    pct,
                    base_pct,
                    tripped,
                });
            }
        }
        overloads.sort_by(|a, b| b.pct.total_cmp(&a.pct));
        let mut voltage_problems = Vec::new();
        let mut buses = Vec::new();
        for (i, (b, before)) in solved.buses.iter().zip(&self.base.case.buses).enumerate() {
            if b.kind == BusType::Isolated {
                continue;
            }
            let outside = |vm: f64| {
                if vm < b.vmin {
                    b.vmin - vm
                } else if vm > b.vmax {
                    vm - b.vmax
                } else {
                    0.0
                }
            };
            let (now, was) = (
                outside(b.vm),
                if absolute { 0.0 } else { outside(before.vm) },
            );
            if now > 0.0 && now > was + 0.001 {
                voltage_problems.push(VoltageProblem {
                    bus: b.id,
                    name: self.names.get(i).cloned().unwrap_or_default(),
                    vm: b.vm,
                    base_vm: before.vm,
                    vmin: b.vmin,
                    vmax: b.vmax,
                    outside: now,
                });
            }
            if (b.vm - before.vm).abs() >= 0.001 || now > 0.0 {
                buses.push(BusChange {
                    bus: b.id,
                    vm: b.vm,
                    base_vm: before.vm,
                    vmin: b.vmin,
                    vmax: b.vmax,
                });
            }
        }
        voltage_problems.sort_by(|a, b| b.outside.total_cmp(&a.outside));
        // Overloads that were there before the outage, and didn't get
        // worse, don't make it severe.
        let worst_pct = overloads.first().map_or(0.0, |o| o.pct);
        let voltage_score = voltage_problems
            .first()
            .map_or(0.0, |v| self.limit_pct + 2000.0 * v.outside);
        row.severity = worst_pct.max(voltage_score);
        row.worst = overloads.first().cloned().or(worst);
        row.overloads = overloads.len();
        row.worst_voltage = voltage_problems.first().cloned();
        row.voltage_violations = voltage_problems.len();
        Some((
            Outcome {
                row,
                overloads,
                voltage_problems,
                branches,
                buses,
                cut_off: screened.cut_off,
            },
            Some(result),
        ))
    }
}

/// A finished run, kept by the open case so the UI can page through it and
/// open one outage at a time.
pub struct ContingencyResults {
    pub summary: ContingencySummary,
    /// Ranked, worst first.
    pub rows: Vec<ContingencyRow>,
    outages: Vec<Outage>,
    power: PowerCase,
    base: PowerFlowResult,
    settings: ContingencySettings,
    /// Different for every run, so a sorted order kept for one run's rows
    /// is never used for another's.
    pub id: u64,
    last_detail: Option<(usize, ContingencyDetail)>,
    /// The converged AC solution of the last outage solved in full, by rank.
    last_solution: Option<(usize, PowerFlowResult)>,
    /// The last outage's report, kept while its tables are paged through.
    last_report: Option<(usize, std::sync::Arc<power::SolveReport>)>,
}

fn next_run_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl ContingencyResults {
    /// Ranked rows `offset..offset + limit`.
    pub fn page(&self, offset: usize, limit: usize) -> crate::table::Table<ContingencyRow> {
        self.rows.iter().skip(offset).take(limit).cloned().collect()
    }

    fn index(&self, rank: usize) -> Result<usize, Error> {
        rank.checked_sub(1)
            .filter(|&i| i < self.rows.len())
            .ok_or_else(|| Error::not_found(format!("No contingency ranked {rank}.")))
    }

    /// One outage in full, by rank (from 1). Solved again on demand, in AC
    /// when the run checked it in AC.
    pub fn detail(&mut self, rank: usize) -> Result<ContingencyDetail, Error> {
        if let Some((r, d)) = &self.last_detail
            && *r == rank
        {
            return Ok(d.clone());
        }
        let i = self.index(rank)?;
        let context = Context::new(&self.power, &self.base, &self.settings)?;
        let row = &self.rows[i];
        let (outcome, solution) = if row.checked == "ac" {
            context
                .ac_solved(
                    &self.outages[i],
                    &self.settings.solve,
                    &RunHandle::default(),
                )
                .ok_or("Cancelled")?
        } else {
            (context.screen(&self.outages[i]), None)
        };
        let detail = ContingencyDetail {
            row: ContingencyRow {
                rank,
                ..outcome.row
            },
            overloads: outcome.overloads,
            voltage_problems: outcome.voltage_problems,
            branches: outcome.branches,
            buses: outcome.buses,
            cut_off: outcome.cut_off,
        };
        self.last_detail = Some((rank, detail.clone()));
        if let Some(solution) = solution {
            self.last_solution = Some((rank, solution));
        }
        Ok(detail)
    }

    /// One outage's whole AC solution as a solve report, by rank, so every
    /// view can show it like the base solve. Solved in AC even when the run
    /// only screened the outage.
    pub fn report(&mut self, rank: usize) -> Result<power::SolveReport, Error> {
        self.shared_report(rank).map(|r| (*r).clone())
    }

    /// [`Self::report`], kept for the next call with the same rank.
    pub fn shared_report(
        &mut self,
        rank: usize,
    ) -> Result<std::sync::Arc<power::SolveReport>, Error> {
        if let Some((r, report)) = &self.last_report
            && *r == rank
        {
            return Ok(report.clone());
        }
        let report = std::sync::Arc::new(self.build_report(rank)?);
        self.last_report = Some((rank, report.clone()));
        Ok(report)
    }

    fn build_report(&mut self, rank: usize) -> Result<power::SolveReport, Error> {
        let i = self.index(rank)?;
        let start = Instant::now();
        let cached = matches!(&self.last_solution, Some((r, _)) if *r == rank);
        if !cached {
            let context = Context::new(&self.power, &self.base, &self.settings)?;
            let (outcome, solution) = context
                .ac_solved(
                    &self.outages[i],
                    &self.settings.solve,
                    &RunHandle::default(),
                )
                .ok_or("Cancelled")?;
            let solution = solution.ok_or_else(|| {
                format!(
                    "{}: {}",
                    self.rows[i].element,
                    outcome
                        .row
                        .note
                        .unwrap_or_else(|| "the AC solve did not converge.".into())
                )
            })?;
            self.last_solution = Some((rank, solution));
        }
        let (_, solution) = self.last_solution.as_ref().expect("solved above");
        let options = self.settings.solve.options_for(&self.power);
        Ok(power::report(
            solution,
            &self.power.elements,
            &self.power.dc_line_kinds,
            &options.voltage_controls,
            &options.flow_controls,
            &self.settings.solve,
            start.elapsed().as_secs_f64() * 1000.0,
        ))
    }

    /// The element each rank takes out.
    pub fn element(&self, rank: usize) -> Option<&str> {
        self.index(rank).ok().map(|i| self.rows[i].element.as_str())
    }
}
