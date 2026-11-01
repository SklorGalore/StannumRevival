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
    /// An AC solve of every outage when there are at most
    /// [`AUTO_AC_OUTAGES`], as PSS/E's ACCC does; otherwise the linear
    /// screen, then an AC solve of the worst ones.
    #[default]
    Auto,
    /// Linear screen of every outage, then an AC solve of the worst ones.
    Screen,
    /// An AC solve of every outage.
    Ac,
}

/// Outages up to which [`Method::Auto`] solves every one in AC.
pub const AUTO_AC_OUTAGES: usize = 2_000;

impl Method {
    /// What `Auto` means for a run of `outages`.
    pub fn resolve(self, outages: usize) -> Method {
        match self {
            Method::Auto if outages <= AUTO_AC_OUTAGES => Method::Ac,
            Method::Auto => Method::Screen,
            m => m,
        }
    }
}

/// Which generators make up the MW an outage loses: tripped units and the
/// generation (net of load) in a part of the network it cuts off.
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Pickup {
    /// The reference (swing) bus alone.
    #[default]
    Reference,
    /// Every online generator left, in proportion to its Pmax.
    Pmax,
    /// Every online generator left, in proportion to its room to Pmax.
    Headroom,
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
fn default_worse_by_pct() -> f64 {
    1.0
}
fn default_worse_by_pu() -> f64 {
    0.001
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
    /// Which generators make up the MW an outage loses: the reference bus
    /// (the default), or every online unit left by Pmax or by headroom.
    #[serde(default)]
    pub pickup: Pickup,
    /// How many of the worst screened outages get an AC solve.
    #[serde(default = "default_ac_count")]
    pub ac_count: usize,
    /// Loading, as a percent of the rating, above which a branch is
    /// overloaded.
    #[serde(default = "default_limit")]
    pub limit_pct: f64,
    /// How many percentage points of loading a branch or interface already
    /// over its limit before the outage must gain to be reported. 1 by
    /// default.
    #[serde(default = "default_worse_by_pct")]
    pub worse_by_pct: f64,
    /// How many per unit a bus already outside its voltage limits before
    /// the outage must move further out to be reported. 0.001 by default.
    #[serde(default = "default_worse_by_pu")]
    pub worse_by_pu: f64,
    /// Report every branch, interface and bus outside its limits after the
    /// outage, also those that already were before it and got no worse.
    #[serde(default)]
    pub report_existing: bool,
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
    /// Run the contingencies of this lists file (`.stnl`): its
    /// `contingency` records and `contingency_set` rules. Equipment the
    /// case lacks is reported in the summary's notes and skipped. Takes
    /// the place of `elements`, `branches`, `generators`, `buses` and
    /// `pairs`; `planning` is unused.
    #[serde(default)]
    pub lists: Option<String>,
    /// Check only what the `monitor` records of this lists file (`.stnl`)
    /// name: branch flows and bus voltages with their own limits, and
    /// interfaces. Every rated branch and every bus when absent.
    #[serde(default)]
    pub monitors: Option<String>,
    /// Lists text (`.stnl`) to run in place of a `lists` file, as the
    /// setup builds it from its subsystems. Wins over `lists`.
    #[serde(default)]
    pub lists_text: Option<String>,
    /// Lists text (`.stnl`) whose monitor records choose what is checked,
    /// in place of a `monitors` file. Wins over `monitors`.
    #[serde(default)]
    pub monitors_text: Option<String>,
}

/// Lists read from inline `text` when given, else from the file at `path`.
fn lists_from(
    text: &Option<String>,
    path: &Option<String>,
    power: &PowerCase,
) -> Option<Result<crate::lists::Lists, String>> {
    match (text, path) {
        (Some(t), _) => Some(crate::lists::parse(t, power)),
        (None, Some(p)) => Some(crate::lists::load(p, power)),
        (None, None) => None,
    }
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
            pickup: Pickup::default(),
            ac_count: default_ac_count(),
            limit_pct: default_limit(),
            worse_by_pct: default_worse_by_pct(),
            worse_by_pu: default_worse_by_pu(),
            report_existing: false,
            rating: Rating::default(),
            base_rating: Rating::default(),
            solve: SolveSettings::default(),
            planning: None,
            lists: None,
            monitors: None,
            lists_text: None,
            monitors_text: None,
        }
    }
}

/// A branch over its limit after an outage.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Overload {
    pub element: String,
    /// The branch named PSS/E style: bus names and numbers, then the circuit.
    pub name: String,
    pub from: u32,
    pub to: u32,
    /// MVA as a percent of the run's rating after the outage, and of its
    /// base rating before it.
    pub pct: f64,
    pub base_pct: f64,
    /// Active flow at the from end, MW, after and before.
    pub mw: f64,
    pub base_mw: f64,
    /// The loading, in percent, above which this branch counts as over:
    /// the run's limit, or its monitor's.
    pub limit_pct: f64,
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

/// An interface's summed flow after an outage, from a lists file monitor.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct InterfaceFlow {
    pub name: String,
    /// Weighted sum of its branch flows, MW, after and before.
    pub mw: f64,
    pub base_mw: f64,
    /// Its limit; none when the interface has no limit.
    pub limit_mw: Option<f64>,
    /// |MW| as a percent of the limit, after and before.
    pub pct: Option<f64>,
    pub base_pct: Option<f64>,
    /// Percent of the limit that counts as a violation.
    pub limit_pct: f64,
}

/// One outage's outcome, as a row of the ranked results.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ContingencyRow {
    /// Position in the ranked list, from 1.
    pub rank: usize,
    /// The element taken out; empty for the base case (P0).
    pub element: String,
    /// "branch", "generator", "shunt", "dc_line", "bus", "pair" for two
    /// elements out together, "group" for more, or "base" for P0.
    #[schemars(extend("enum" = ["branch", "generator", "shunt", "dc_line", "bus", "pair", "group", "base"]))]
    pub kind: &'static str,
    /// The planning event category, such as "P1.2"; none in an N-1 run.
    pub category: Option<String>,
    /// The contingency's name: the one its lists file gives it, or else
    /// built PSS/E style from what goes out, bus names and numbers then the
    /// circuit or unit id, such as "ALPHA 101 BETA 102 1"; "BASE CASE" for P0.
    pub name: String,
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
    /// Monitored interfaces over their limit that are new or worse, and
    /// the worst of them.
    #[serde(default)]
    pub interface_violations: usize,
    pub worst_interface: Option<InterfaceFlow>,
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
    /// Outages that take a monitored interface over its limit.
    #[serde(default)]
    pub with_interface_problems: usize,
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
    /// Lists file runs: what was skipped and why, in sentences.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Whether a lists file's monitors chose what was checked.
    #[serde(default)]
    pub monitored: bool,
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
    /// Events with any overload, voltage problem or failed solve, each
    /// counted once.
    #[serde(default)]
    pub with_problems: usize,
}

/// A branch's flow after one outage, for drawing the change on a diagram.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct BranchChange {
    pub element: String,
    /// The branch named PSS/E style: bus names and numbers, then the circuit.
    pub name: String,
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
    /// Every monitored interface's flow.
    #[serde(default)]
    pub interfaces: Vec<InterfaceFlow>,
}

/// Progress of the run in hand, for a progress bar.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Progress {
    /// "idle", "screening", "ac" or "contingencies" (a secure transfer
    /// step solving its contingencies).
    #[schemars(extend("enum" = ["idle", "screening", "ac", "contingencies"]))]
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
                3 => "contingencies",
                _ => "idle",
            },
            done: self.0.done.load(Ordering::SeqCst),
            total: self.0.total.load(Ordering::SeqCst),
        }
    }

    pub(crate) fn set_phase(&self, phase: usize, total: usize) {
        self.0.phase.store(phase, Ordering::SeqCst);
        self.0.done.store(0, Ordering::SeqCst);
        self.0.total.store(total, Ordering::SeqCst);
    }

    /// Count one more piece of work done.
    pub(crate) fn step(&self) {
        self.0.done.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::SeqCst)
    }

    /// The flag that stops a solve, shared with the run.
    pub(crate) fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.0.cancel.clone()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Outage {
    pub(crate) element: String,
    pub(crate) kind: &'static str,
    pub(crate) branch_rows: Vec<usize>,
    pub(crate) generator_rows: Vec<usize>,
    /// Rows of `PowerCase::shunts`.
    pub(crate) shunt_rows: Vec<usize>,
    /// `case.dcline` rows with the share of the schedule that stays.
    pub(crate) dc_rows: Vec<(usize, f64)>,
    category: Option<String>,
    description: Option<String>,
    /// The name a lists file gives it.
    pub(crate) name: Option<String>,
}

impl Outage {
    /// P0: judged against the limits, not against the base case.
    fn is_base(&self) -> bool {
        self.kind == "base"
    }

    /// P0: nothing out, checked against the base rating.
    pub(crate) fn base() -> Self {
        Self {
            kind: "base",
            ..Default::default()
        }
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
            category: Some(e.category.to_string()),
            description: e.description,
            name: None,
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
    notes: Vec<String>,
    watch: Watch,
    handle: RunHandle,
}

/// Pick the outages `settings` asks for from `power`.
pub(crate) fn outages(
    power: &PowerCase,
    settings: &ContingencySettings,
) -> Result<(Vec<Outage>, usize, Vec<String>), String> {
    if let Some(lists) = lists_from(&settings.lists_text, &settings.lists, power) {
        return from_lists(power, settings, lists?);
    }
    if let Some(p) = &settings.planning {
        let list: Vec<Outage> = planning::events(power, p)?
            .into_iter()
            .map(Outage::from)
            .filter(|o| !zero_impedance_line(&power.case, settings, o))
            .collect();
        return Ok((list, 0, vec![]));
    }
    let case = &power.case;
    let wanted: Option<Vec<&str>> = settings
        .elements
        .as_ref()
        .map(|e| e.iter().map(String::as_str).collect());
    let wants_buses = settings.buses
        || wanted
            .iter()
            .flatten()
            .any(|w| w.split('&').any(|p| p.starts_with("bus:")));
    let Singles {
        list: singles,
        index,
    } = Singles::new(power, wants_buses);
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
    Ok((list, already_out, vec![]))
}

/// The outages a lists file asks for: its named contingencies, then what
/// each contingency set generates.
fn from_lists(
    power: &PowerCase,
    settings: &ContingencySettings,
    lists: crate::lists::Lists,
) -> Result<(Vec<Outage>, usize, Vec<String>), String> {
    use crate::lists::SetKind;
    let case = &power.case;
    let mut notes = lists.notes;
    let with_buses = lists
        .named
        .iter()
        .any(|n| n.elements.iter().any(|e| e.starts_with("bus:")))
        || lists.sets.iter().any(|s| s.kind == SetKind::Buses);
    let singles = Singles::new(power, with_buses);
    let mut list = Vec::new();
    'named: for n in &lists.named {
        let mut parts = Vec::new();
        for id in &n.elements {
            match singles.get(id) {
                Some(o) => parts.push(o),
                None => {
                    notes.push(format!(
                        "Contingency \"{}\": {id} isn't in this case; skipped.",
                        n.name
                    ));
                    continue 'named;
                }
            }
        }
        let mut o = combine(&parts);
        if parts.len() > 2 {
            o.kind = "group";
        }
        o.description = Some(n.name.clone());
        o.name = Some(n.name.clone());
        o.category = n.category.clone();
        list.push(o);
    }
    let kv: HashMap<u32, f64> = case.buses.iter().map(|b| (b.id, b.base_kv)).collect();
    for set in &lists.sets {
        let fits = |o: &Outage| {
            let buses = Singles::buses(case, o);
            let inside = set
                .buses
                .as_ref()
                .is_none_or(|s| buses.iter().any(|b| s.contains(b)));
            let high = set
                .kv_min
                .is_none_or(|k| buses.iter().any(|b| kv.get(b).copied().unwrap_or(0.0) >= k));
            inside && high
        };
        let want = match set.kind {
            SetKind::Branches | SetKind::BranchPairs => "branch",
            SetKind::Generators => "generator",
            SetKind::Buses => "bus",
        };
        let chosen: Vec<&Outage> = singles
            .list
            .iter()
            .filter(|o| o.kind == want)
            .filter(|o| !o.element.starts_with("switch:"))
            .filter(|o| !zero_impedance_line(case, settings, o))
            .filter(|o| fits(o))
            .collect();
        let category = Some(set.name.clone());
        if set.kind == SetKind::BranchPairs {
            let n = chosen.len();
            let pairs = n * n.saturating_sub(1) / 2;
            if list.len() + pairs > MAX_PAIRS {
                return Err(format!(
                    "Contingency set \"{}\" makes {pairs} pairs from {n} branches; narrow its subsystem to {MAX_PAIRS} outages or fewer in all.",
                    set.name
                ));
            }
            for a in 0..n {
                for b in a + 1..n {
                    let mut o = combine(&[chosen[a], chosen[b]]);
                    o.category = category.clone();
                    list.push(o);
                }
            }
        } else {
            for o in chosen {
                let mut o = o.clone();
                o.category = category.clone();
                list.push(o);
            }
        }
    }
    let before = list.len();
    list.retain(|o| {
        o.branch_rows.iter().any(|&r| case.branches[r].online)
            || o.generator_rows.iter().any(|&r| case.generators[r].online)
            || o.shunt_rows.iter().any(|&r| power.shunts[r].online)
            || o.dc_rows.iter().any(|&(r, _)| {
                case.dcline
                    .as_ref()
                    .and_then(|t| t.get(r))
                    .is_some_and(|l| l[2] != 0.0)
            })
    });
    let already_out = before - list.len();
    Ok((list, already_out, notes))
}

/// Notes kept from a lists file; the rest are counted.
const MAX_NOTES: usize = 50;

fn cap_notes(notes: &mut Vec<String>) {
    if notes.len() > MAX_NOTES {
        let more = notes.len() - MAX_NOTES;
        notes.truncate(MAX_NOTES);
        notes.push(format!("And {more} more."));
    }
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

/// Every single outage a case offers, by element id: branches (a
/// three-winding transformer is one element over its windings),
/// generators, shunts, DC lines and, when asked, buses.
struct Singles {
    list: Vec<Outage>,
    index: HashMap<String, usize>,
}

impl Singles {
    fn new(power: &PowerCase, with_buses: bool) -> Self {
        let case = &power.case;
        let mut s = Singles {
            list: Vec::new(),
            index: HashMap::new(),
        };
        for (row, e) in power.elements.branches.iter().enumerate() {
            let i = s.add(&e.element, "branch");
            s.list[i].branch_rows.push(row);
        }
        for (row, e) in power.elements.generators.iter().enumerate() {
            let i = s.add(&e.element, "generator");
            s.list[i].generator_rows.push(row);
        }
        for (row, sh) in power.shunts.iter().enumerate() {
            let id = if sh.kind == rs_power::ShuntKind::Switched {
                crate::elements::switched_shunt(sh.bus, &sh.id)
            } else {
                crate::elements::shunt(sh.bus, &sh.id)
            };
            let i = s.add(&id, "shunt");
            s.list[i].shunt_rows.push(row);
        }
        for (row, id) in power.elements.dc_lines.iter().enumerate() {
            if !id.is_empty() {
                let i = s.add(id, "dc_line");
                s.list[i].dc_rows.push((row, 0.0));
            }
        }
        if with_buses {
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
                let i = s.add(&crate::elements::bus(bus.id), "bus");
                let found = at.remove(&bus.id).unwrap_or_default();
                s.list[i].branch_rows = found.branch_rows;
                s.list[i].dc_rows = found.dc_rows;
            }
        }
        s
    }

    fn add(&mut self, element: &str, kind: &'static str) -> usize {
        *self.index.entry(element.to_string()).or_insert_with(|| {
            self.list.push(Outage {
                element: element.to_string(),
                kind,
                ..Default::default()
            });
            self.list.len() - 1
        })
    }

    /// The single outage an id names. Branch ids are also found with
    /// their bus numbers in another order (line:2:1:1 for line:1:2:1).
    fn get(&self, id: &str) -> Option<&Outage> {
        if let Some(&i) = self.index.get(id) {
            return Some(&self.list[i]);
        }
        let mut parts: Vec<&str> = id.split(':').collect();
        let kind = *parts.first()?;
        let buses = match kind {
            "line" | "switch" => 2,
            "transformer" if parts.len() >= 5 => 3,
            "transformer" => 2,
            _ => return None,
        };
        if parts.len() < 1 + buses + 1 {
            return None;
        }
        let nums = &mut parts[1..1 + buses];
        if nums.iter().any(|n| n.parse::<u32>().is_err()) {
            return None;
        }
        nums.sort_by_key(|n| n.parse::<u32>().unwrap_or(0));
        self.index.get(&parts.join(":")).map(|&i| &self.list[i])
    }

    /// The buses an outage touches.
    fn buses(case: &Case, o: &Outage) -> Vec<u32> {
        let mut out = Vec::new();
        for &r in &o.branch_rows {
            out.push(case.branches[r].from);
            out.push(case.branches[r].to);
        }
        for &r in &o.generator_rows {
            out.push(case.generators[r].bus);
        }
        for &(r, _) in &o.dc_rows {
            if let Some(l) = case.dcline.as_ref().and_then(|t| t.get(r)) {
                out.push(l[0] as u32);
                out.push(l[1] as u32);
            }
        }
        if o.kind == "bus"
            && let Some(n) = o.element.strip_prefix("bus:").and_then(|n| n.parse().ok())
        {
            out.push(n);
        }
        out
    }
}

/// N-2 runs at most this many pairs: each is a row of the results.
pub const MAX_PAIRS: usize = 250_000;

/// A watched branch's limit (percent) and the rating a monitor sets.
type BranchWatch = Option<(f64, Option<Rating>)>;
/// A watched bus's limits, in place of its own.
type BusWatch = Option<(Option<f64>, Option<f64>)>;

/// What a run checks after each outage: every rated branch and every bus
/// by default, or only what a lists file's monitors name.
#[derive(Clone, Debug, Default)]
pub(crate) struct Watch {
    /// By branch row: the limit (percent) and the rating a monitor sets
    /// (the run's when `None`); unwatched rows are `None`. Every branch at
    /// the run's limit when absent.
    branches: Option<Vec<BranchWatch>>,
    /// By bus row: limits that replace the bus's own; unwatched rows are
    /// `None`. Every bus at its own limits when absent.
    buses: Option<Vec<BusWatch>>,
    interfaces: Vec<WatchedInterface>,
    /// Chosen by a lists file's monitors.
    from_file: bool,
}

#[derive(Clone, Debug)]
struct WatchedInterface {
    name: String,
    /// Branch rows and weights, signed so the from-end flow counts in the
    /// direction the interface's ids are written.
    rows: Vec<(usize, f64)>,
    limit_mw: Option<f64>,
    limit_pct: f64,
}

impl Watch {
    /// The monitors of `lists`, resolved to `power`'s
    /// rows, and notes on what was skipped.
    fn from_lists(
        power: &PowerCase,
        lists: crate::lists::Lists,
        settings: &ContingencySettings,
    ) -> Result<(Self, Vec<String>), String> {
        let mut notes = lists.notes;
        if lists.monitors.is_empty() {
            notes.push(
                "The monitors file has no monitor records; every branch and bus was checked."
                    .into(),
            );
            return Ok((Self::default(), notes));
        }
        let case = &power.case;
        let singles = Singles::new(power, false);
        let branch_rows = |id: &str| -> Option<Vec<usize>> {
            singles
                .get(id)
                .map(|o| o.branch_rows.clone())
                .filter(|r| !r.is_empty())
        };
        let bus_row: HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let mut branches: Vec<BranchWatch> = vec![None; case.branches.len()];
        let mut buses: Vec<BusWatch> = vec![None; case.buses.len()];
        let mut interfaces = Vec::new();
        // A branch under more than one monitor keeps the lowest limit in MVA.
        let mut watch_branch = |r: usize, limit: f64, rating: Option<Rating>| {
            let b = &case.branches[r];
            let mva = |l: f64, rt: Option<Rating>| l * rt.unwrap_or(settings.rating).of(b);
            if branches[r].is_none_or(|(l, rt)| mva(limit, rating) < mva(l, rt)) {
                branches[r] = Some((limit, rating));
            }
        };
        for m in &lists.monitors {
            let skip = |why: String| format!("Monitor \"{}\": {why}; skipped.", m.name);
            match &m.watched {
                crate::lists::Watched::Flow {
                    element,
                    buses: within,
                    rating,
                } => {
                    let rating = rating.map(|n| match n {
                        2 => Rating::B,
                        3 => Rating::C,
                        _ => Rating::A,
                    });
                    match element {
                        Some(id) => match branch_rows(id) {
                            Some(rows) => rows
                                .into_iter()
                                .for_each(|r| watch_branch(r, m.limit_pct, rating)),
                            None => notes.push(skip(format!("{id} isn't a branch in this case"))),
                        },
                        None => {
                            for (r, b) in case.branches.iter().enumerate() {
                                if within
                                    .as_ref()
                                    .is_none_or(|ids| ids.contains(&b.from) || ids.contains(&b.to))
                                {
                                    watch_branch(r, m.limit_pct, rating);
                                }
                            }
                        }
                    }
                }
                crate::lists::Watched::Voltage {
                    element,
                    buses: within,
                    vmin,
                    vmax,
                } => {
                    let limits = Some((*vmin, *vmax));
                    match element {
                        Some(id) => match id
                            .strip_prefix("bus:")
                            .and_then(|n| n.parse::<u32>().ok())
                            .and_then(|n| bus_row.get(&n))
                        {
                            Some(&i) => buses[i] = limits,
                            None => notes.push(skip(format!("{id} isn't a bus in this case"))),
                        },
                        None => {
                            for (i, b) in case.buses.iter().enumerate() {
                                if within.as_ref().is_none_or(|ids| ids.contains(&b.id)) {
                                    buses[i] = limits;
                                }
                            }
                        }
                    }
                }
                crate::lists::Watched::Interface(i) => {
                    let mut rows = Vec::new();
                    let mut missing = None;
                    for (id, w) in &i.elements {
                        // A three-winding transformer counts at its first
                        // winding.
                        let Some(&r) = branch_rows(id).as_ref().and_then(|r| r.first()) else {
                            missing = Some(id.clone());
                            break;
                        };
                        let first: Option<u32> = id.split(':').nth(1).and_then(|n| n.parse().ok());
                        let sign = if first.is_some_and(|f| f != case.branches[r].from) {
                            -1.0
                        } else {
                            1.0
                        };
                        rows.push((r, w * sign));
                    }
                    match missing {
                        Some(id) => notes.push(skip(format!(
                            "{id} in interface {} isn't a branch in this case",
                            i.name
                        ))),
                        None => interfaces.push(WatchedInterface {
                            name: i.name.clone(),
                            rows,
                            limit_mw: i.limit_mw.filter(|l| *l > 0.0),
                            limit_pct: m.limit_pct,
                        }),
                    }
                }
            }
        }
        Ok((
            Self {
                branches: Some(branches),
                buses: Some(buses),
                interfaces,
                from_file: true,
            },
            notes,
        ))
    }
}

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
        let (outages, already_out, mut notes) = outages(power, &settings)?;
        let watch = match lists_from(&settings.monitors_text, &settings.monitors, power) {
            Some(lists) => {
                let (watch, more) = Watch::from_lists(power, lists?, &settings)?;
                notes.extend(more);
                watch
            }
            None => Watch::default(),
        };
        cap_notes(&mut notes);
        let mut settings = settings;
        settings.method = settings.method.resolve(outages.len());
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
            notes,
            watch,
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
        let context = Context::new(&self.power, &self.base, &self.settings, &self.watch)?;
        let n = self.outages.len();

        handle.set_phase(1, n);
        let mut rows: Vec<Option<(ContingencyRow, Violations)>> = parallel(handle, n, |i| {
            let o = context.screen(&self.outages[i]);
            Some((o.row.clone(), o.violations()))
        });
        let screen_ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut was_cancelled = rows.iter().any(Option::is_none);

        let ac_start = Instant::now();
        let mut ac_checked = 0;
        if !was_cancelled {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| {
                severity(rows[b].as_ref().map(|r| &r.0))
                    .total_cmp(&severity(rows[a].as_ref().map(|r| &r.0)))
            });
            let count = match self.settings.method {
                Method::Ac | Method::Auto => n,
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
                    .map(|d| {
                        let v = d.violations();
                        (d.row, v)
                    })
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
        let (mut ranked, mut found): (Vec<(usize, ContingencyRow)>, Vec<Violations>) = rows
            .into_iter()
            .enumerate()
            .filter_map(|(i, r)| r.map(|(r, v)| ((i, r), v)))
            .unzip();
        // Keep the violations in step with the ranking below.
        let mut by_outage: HashMap<usize, Violations> = ranked
            .iter()
            .map(|(i, _)| *i)
            .zip(found.drain(..))
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
            notes: self.notes.clone(),
            ac_checked,
            with_overloads: ranked.iter().filter(|(_, r)| r.overloads > 0).count(),
            with_voltage_problems: ranked
                .iter()
                .filter(|(_, r)| r.voltage_violations > 0)
                .count(),
            with_interface_problems: ranked
                .iter()
                .filter(|(_, r)| r.interface_violations > 0)
                .count(),
            monitored: self.watch.from_file,
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
        let violations = order
            .iter()
            .map(|i| by_outage.remove(i).unwrap_or_default())
            .collect();
        Ok(ContingencyResults {
            summary,
            rows,
            violations,
            outages,
            power: self.power,
            base: self.base,
            settings: self.settings,
            watch: self.watch,
            id: next_run_id(),
            last_detail: None,
            last_solution: None,
            last_report: None,
        })
    }
}

/// What one outage found at a step of another study (secure transfer
/// limits), with its solution when the AC solve converged.
pub(crate) struct StepCheck<'a> {
    pub(crate) row: &'a ContingencyRow,
    pub(crate) violations: Violations,
    /// Every watched interface's flow.
    pub(crate) interfaces: &'a [InterfaceFlow],
    pub(crate) solution: Option<&'a PowerFlowResult>,
}

/// Contingency checks for a study that moves the case and checks every
/// outage again at each step: the outages and monitors chosen once, from
/// the case before it moves.
pub(crate) struct StepChecker {
    settings: ContingencySettings,
    watch: Watch,
    pub(crate) outages: Vec<Outage>,
    pub(crate) already_out: usize,
    pub(crate) notes: Vec<String>,
}

impl StepChecker {
    pub(crate) fn new(power: &PowerCase, settings: ContingencySettings) -> Result<Self, String> {
        let (outages, already_out, mut notes) = outages(power, &settings)?;
        let watch = match lists_from(&settings.monitors_text, &settings.monitors, power) {
            Some(lists) => {
                let (watch, more) = Watch::from_lists(power, lists?, &settings)?;
                notes.extend(more);
                watch
            }
            None => Watch::default(),
        };
        cap_notes(&mut notes);
        Ok(Self {
            settings,
            watch,
            outages,
            already_out,
            notes,
        })
    }

    pub(crate) fn settings(&self) -> &ContingencySettings {
        &self.settings
    }

    /// Bus rows a voltage monitor names; `None` when every bus is watched.
    pub(crate) fn watched_buses(&self) -> Option<Vec<usize>> {
        self.watch.buses.as_ref().map(|rows| {
            rows.iter()
                .enumerate()
                .filter(|(_, w)| w.is_some())
                .map(|(r, _)| r)
                .collect()
        })
    }

    /// Names of the watched interfaces, in order.
    pub(crate) fn interface_names(&self) -> Vec<String> {
        self.watch
            .interfaces
            .iter()
            .map(|i| i.name.clone())
            .collect()
    }

    /// Solve each of `outages` in AC from `base`, the converged solve of
    /// `power` (the moved case), on every core, mapping each with `f`.
    /// `None` for an outage the cancel stopped.
    pub(crate) fn check<T: Send>(
        &self,
        power: &PowerCase,
        base: &PowerFlowResult,
        outages: &[&Outage],
        handle: &RunHandle,
        f: impl Fn(usize, StepCheck<'_>) -> T + Sync,
    ) -> Result<Vec<Option<T>>, String> {
        let context = Context::new(power, base, &self.settings, &self.watch)?;
        Ok(parallel(handle, outages.len(), |i| {
            let (o, solution) = context.ac_solved(outages[i], &self.settings.solve, handle)?;
            let violations = o.violations();
            Some(f(
                i,
                StepCheck {
                    row: &o.row,
                    violations,
                    interfaces: &o.interfaces,
                    solution: solution.as_ref(),
                },
            ))
        }))
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
        // Lists file categories follow the planning ones, as first seen.
        if !out.iter().any(|s| &s.category == c) {
            out.push(CategorySummary {
                category: c.clone(),
                ..Default::default()
            });
        }
        let Some(s) = out.iter_mut().find(|s| &s.category == c) else {
            continue;
        };
        s.total += 1;
        s.with_overloads += usize::from(r.overloads > 0);
        s.with_voltage_problems += usize::from(r.voltage_violations > 0);
        s.not_converged += usize::from(r.converged == Some(false));
        s.islanding += usize::from(r.cut_off_buses > 0);
        s.with_problems += usize::from(
            r.overloads > 0
                || r.voltage_violations > 0
                || r.interface_violations > 0
                || r.converged == Some(false),
        );
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

fn severity(row: Option<&ContingencyRow>) -> f64 {
    row.map_or(f64::NEG_INFINITY, |r| r.severity)
}

fn log_summary(s: &ContingencySummary) {
    let mut line = format!(
        "Contingencies: {} outages, {} solved in AC; {} with overloads, {} with voltage problems",
        s.total, s.ac_checked, s.with_overloads, s.with_voltage_problems
    );
    if s.ac_checked < s.total {
        line += &format!(
            " (voltage checked only in the {} AC solves; {} outages had the linear screen only)",
            s.ac_checked,
            s.total - s.ac_checked
        );
    }
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
pub(crate) fn parallel<T: Send>(
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
    worse_by_pct: f64,
    worse_by_pu: f64,
    report_existing: bool,
    rating: Rating,
    base_rating: Rating,
    pickup: Pickup,
    /// Base flows at the from and to ends.
    pf: Vec<f64>,
    qf: Vec<f64>,
    pt: Vec<f64>,
    qt: Vec<f64>,
    base_pct: Vec<Option<f64>>,
    names: Vec<String>,
    bus_row: HashMap<u32, usize>,
    watch: &'a Watch,
}

struct Outcome {
    row: ContingencyRow,
    overloads: Vec<Overload>,
    voltage_problems: Vec<VoltageProblem>,
    branches: Vec<BranchChange>,
    buses: Vec<BusChange>,
    cut_off: Vec<u32>,
    interfaces: Vec<InterfaceFlow>,
    /// The interfaces over their limit, as reported, worst first.
    interfaces_over: Vec<InterfaceFlow>,
}

impl Outcome {
    /// What the outage found outside its limits, to keep with the run.
    fn violations(&self) -> Violations {
        Violations {
            overloads: self.overloads.clone(),
            voltage_problems: self.voltage_problems.clone(),
            interfaces: self.interfaces_over.clone(),
        }
    }
}

/// Everything one outage found outside its limits, as reported (new or
/// worse unless the run reports existing problems too), worst first.
#[derive(Clone, Debug, Default)]
pub struct Violations {
    pub overloads: Vec<Overload>,
    pub voltage_problems: Vec<VoltageProblem>,
    pub interfaces: Vec<InterfaceFlow>,
}

/// Loading of branch row `r`, measured as in the solve report.
fn loading(power: &PowerCase, r: usize, pf: f64, qf: f64, pt: f64, qt: f64, rate: f64) -> f64 {
    let winding = power.elements.branches.get(r).map_or(0, |e| e.winding);
    power::loading_pct(pf, qf, pt, qt, rate, winding)
}

/// A branch is new or worse when it is over the limit now and was under it,
/// or has grown by more than `worse_by` points.
fn new_or_worse(pct: f64, base: f64, limit: f64, worse_by: f64) -> bool {
    pct > limit && (base <= limit || pct > base + worse_by)
}

impl<'a> Context<'a> {
    fn new(
        power: &'a PowerCase,
        base: &'a PowerFlowResult,
        settings: &ContingencySettings,
        watch: &'a Watch,
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
            worse_by_pct: settings.worse_by_pct.max(0.0),
            worse_by_pu: settings.worse_by_pu.max(0.0),
            report_existing: settings.report_existing,
            rating: settings.rating,
            base_rating,
            pickup: settings.pickup,
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
            watch,
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

    /// Branch row `r`'s limit (percent) and rating, or `None` when it
    /// isn't watched.
    fn branch_limit(&self, r: usize, base_case: bool) -> Option<(f64, Rating)> {
        match &self.watch.branches {
            None => Some((self.limit_pct, self.rating_for(base_case))),
            Some(rows) => rows[r].map(|(limit, rating)| {
                let rating = if base_case {
                    self.base_rating
                } else {
                    rating.unwrap_or(self.rating)
                };
                (limit, rating)
            }),
        }
    }

    /// Bus row `i`'s voltage limits, or `None` when it isn't watched.
    fn bus_limits(&self, i: usize, b: &rs_power::Bus) -> Option<(f64, f64)> {
        match &self.watch.buses {
            None => Some((b.vmin, b.vmax)),
            Some(rows) => rows[i].map(|(lo, hi)| (lo.unwrap_or(b.vmin), hi.unwrap_or(b.vmax))),
        }
    }

    /// Whether a loading of `pct` (`base` before the outage) over `limit` is
    /// reported: new or worse, unless the run reports existing ones too.
    fn reported(&self, pct: f64, base: f64, limit: f64, base_case: bool) -> bool {
        let before = if base_case || self.report_existing {
            0.0
        } else {
            base
        };
        new_or_worse(pct, before, limit, self.worse_by_pct)
    }

    /// Each watched interface's flow, from every branch row's from-end MW
    /// after the outage; and those over their limit, new or worse, worst
    /// first.
    fn interfaces(
        &self,
        mw: impl Fn(usize) -> f64,
        base_case: bool,
    ) -> (Vec<InterfaceFlow>, Vec<InterfaceFlow>) {
        let mut all = Vec::new();
        let mut over = Vec::new();
        for i in &self.watch.interfaces {
            let now: f64 = i.rows.iter().map(|&(r, w)| w * mw(r)).sum();
            let before: f64 = i.rows.iter().map(|&(r, w)| w * self.pf[r]).sum();
            let pct = |v: f64| i.limit_mw.map(|l| v.abs() / l * 100.0);
            let flow = InterfaceFlow {
                name: i.name.clone(),
                mw: now,
                base_mw: before,
                limit_mw: i.limit_mw,
                pct: pct(now),
                base_pct: pct(before),
                limit_pct: i.limit_pct,
            };
            if let (Some(p), Some(b)) = (flow.pct, flow.base_pct)
                && self.reported(p, b, i.limit_pct, base_case)
            {
                over.push(flow.clone());
            }
            all.push(flow);
        }
        over.sort_by(|a, b| b.pct.unwrap_or(0.0).total_cmp(&a.pct.unwrap_or(0.0)));
        (all, over)
    }

    /// An outage's name, PSS/E style: each element's bus names and numbers,
    /// then its circuit or unit id; elements out together joined by " & ".
    fn auto_name(&self, outage: &Outage) -> String {
        if outage.is_base() {
            return "BASE CASE".into();
        }
        self.name_of(&outage.element)
    }

    /// Element ids named PSS/E style (see [`element_name`]).
    fn name_of(&self, ids: &str) -> String {
        element_name(ids, &self.names, &self.bus_row)
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
        // Generators taken out: their output moves to the reference bus, or
        // is shared among the units left.
        let mut lost: Vec<(u32, f64)> = outage
            .generator_rows
            .iter()
            .filter(|&&r| case.generators[r].online)
            .map(|&r| (case.generators[r].bus, -self.base.case.generators[r].pg))
            .collect();
        let tripped: f64 = lost.iter().map(|(_, mw)| -mw).sum();
        if tripped != 0.0 {
            let none = std::collections::HashSet::new();
            for (g, share) in self.pickup_shares(&outage.generator_rows, &none) {
                lost.push((case.generators[g].bus, tripped * share));
            }
        }
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
                let total: f64 = injections.iter().map(|(_, mw)| mw).sum();
                let shares = self.pickup_shares(&outage.generator_rows, &ids);
                if !shares.is_empty() {
                    for (g, share) in shares {
                        injections.push((case.generators[g].bus, -total * share));
                    }
                } else if let Some(pickup) = pickup {
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

    /// The generator rows that make up lost MW and each one's share (summing
    /// to 1): every online unit not tripped (`tripped` rows) or cut off
    /// (`cut` bus ids), by Pmax or headroom. Empty for the reference bus,
    /// or when no unit has any.
    fn pickup_shares(
        &self,
        tripped: &[usize],
        cut: &std::collections::HashSet<u32>,
    ) -> Vec<(usize, f64)> {
        let solved = &self.base.case.generators;
        let weight = |r: usize, g: &rs_power::Generator| match self.pickup {
            Pickup::Reference => 0.0,
            Pickup::Pmax => g.pmax.max(0.0),
            Pickup::Headroom => (g.pmax - solved.get(r).map_or(g.pg, |s| s.pg)).max(0.0),
        };
        let weights: Vec<(usize, f64)> = self
            .case()
            .generators
            .iter()
            .enumerate()
            .filter(|(r, g)| g.online && !tripped.contains(r) && !cut.contains(&g.bus))
            .map(|(r, g)| (r, weight(r, g)))
            .filter(|&(_, w)| w > 0.0 && w.is_finite())
            .collect();
        let total: f64 = weights.iter().map(|(_, w)| w).sum();
        if total <= 1e-9 {
            return vec![];
        }
        weights.into_iter().map(|(r, w)| (r, w / total)).collect()
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
        let mut overloads = Vec::new();
        let mut branches = Vec::new();
        let mut worst: Option<Overload> = None;
        for (r, b) in case.branches.iter().enumerate() {
            let watched = self.branch_limit(r, absolute);
            let (limit, rating) = watched.unwrap_or((self.limit_pct, self.rating_for(absolute)));
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
            if let (Some(pct), Some(base), true) = (pct, base_pct, watched.is_some()) {
                let o = Overload {
                    element: self.element(r),
                    name: self.name_of(&self.element(r)),
                    from: b.from,
                    to: b.to,
                    pct,
                    base_pct: base,
                    mw: flows[r],
                    base_mw: self.pf[r],
                    limit_pct: limit,
                };
                if worst.as_ref().is_none_or(|w| pct > w.pct) {
                    worst = Some(o.clone());
                }
                if self.reported(pct, base, limit, absolute) {
                    overloads.push(o);
                }
            }
            if b.online
                && ((flows[r] - self.pf[r]).abs() >= 0.5
                    || tripped
                    || pct.is_some_and(|p| p > limit))
            {
                branches.push(BranchChange {
                    element: self.element(r),
                    name: self.name_of(&self.element(r)),
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
        let (interfaces, over) = self.interfaces(
            |r| {
                if outage.branch_rows.contains(&r) {
                    0.0
                } else {
                    flows[r]
                }
            },
            absolute,
        );
        let worst_interface_pct = over.first().and_then(|i| i.pct).unwrap_or(0.0);
        let row = ContingencyRow {
            rank: 0,
            element: outage.element.clone(),
            kind: outage.kind,
            category: outage.category.clone(),
            name: outage
                .name
                .clone()
                .unwrap_or_else(|| self.auto_name(outage)),
            description: outage.description.clone(),
            checked: "linear",
            converged: None,
            iterations: None,
            severity: overloads
                .first()
                .map_or(0.0, |o| o.pct)
                .max(worst_interface_pct)
                + tie_break(lost_load + lost_gen, &branches),
            worst: shown,
            overloads: overloads.len(),
            screened_pct: screened,
            note: None,
            worst_voltage: None,
            voltage_violations: 0,
            interface_violations: over.len(),
            worst_interface: over.first().cloned(),
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
            interfaces,
            interfaces_over: over,
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
        // Share what the outage loses (tripped units, and the generation
        // net of load it cuts off) among the units left, before the solve,
        // so the reference bus only takes up the change in losses. A unit
        // stops at Pmax (or Pmin, when load is cut off); what it can't take
        // stays with the reference.
        let shares = self.pickup_shares(&outage.generator_rows, &ids);
        if !shares.is_empty() {
            let solved = &self.base.case.generators;
            let pg = |r: usize| solved.get(r).map_or(self.case().generators[r].pg, |g| g.pg);
            let tripped: f64 = outage
                .generator_rows
                .iter()
                .filter(|&&r| self.case().generators[r].online)
                .map(|&r| pg(r))
                .sum();
            let (load, generation) = self.lost(cut, outage);
            let net = tripped + generation - load;
            for (r, share) in shares {
                let g = &mut case.generators[r];
                let start = pg(r);
                g.pg = (start + net * share).clamp(g.pmin.min(start), g.pmax.max(start));
            }
        }
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
        let mut overloads = Vec::new();
        let mut branches = Vec::new();
        let mut worst: Option<Overload> = None;
        for (r, (b, f)) in solved.branches.iter().zip(&result.branch_flows).enumerate() {
            let tripped = outage.branch_rows.contains(&r);
            let base_pct = self.base_pct[r];
            let watched = self.branch_limit(r, absolute);
            let (limit, rating) = watched.unwrap_or((self.limit_pct, self.rating_for(absolute)));
            let rate = rating.of(b);
            let pct = (b.online && rate > 0.0)
                .then(|| loading(self.power, r, f[0], f[1], f[2], f[3], rate));
            if let (Some(pct), Some(base), true) = (pct, base_pct, watched.is_some()) {
                let o = Overload {
                    element: self.element(r),
                    name: self.name_of(&self.element(r)),
                    from: b.from,
                    to: b.to,
                    pct,
                    base_pct: base,
                    mw: f[0],
                    base_mw: self.pf[r],
                    limit_pct: limit,
                };
                if worst.as_ref().is_none_or(|w| pct > w.pct) {
                    worst = Some(o.clone());
                }
                if self.reported(pct, base, limit, absolute) {
                    overloads.push(o);
                }
            }
            let was_online = self.case().branches[r].online;
            if was_online
                && ((f[0] - self.pf[r]).abs() >= 0.5
                    || tripped
                    || !b.online
                    || pct.is_some_and(|p| p > limit))
            {
                branches.push(BranchChange {
                    element: self.element(r),
                    name: self.name_of(&self.element(r)),
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
            let watched = self.bus_limits(i, b);
            let (vmin, vmax) = watched.unwrap_or((b.vmin, b.vmax));
            let outside = |vm: f64| {
                if vm < vmin {
                    vmin - vm
                } else if vm > vmax {
                    vm - vmax
                } else {
                    0.0
                }
            };
            let (now, was) = (
                outside(b.vm),
                if absolute || self.report_existing {
                    0.0
                } else {
                    outside(before.vm)
                },
            );
            if watched.is_some() && now > 0.0 && now > was + self.worse_by_pu {
                voltage_problems.push(VoltageProblem {
                    bus: b.id,
                    name: self.names.get(i).cloned().unwrap_or_default(),
                    vm: b.vm,
                    base_vm: before.vm,
                    vmin,
                    vmax,
                    outside: now,
                });
            }
            if (b.vm - before.vm).abs() >= 0.001 || now > 0.0 {
                buses.push(BusChange {
                    bus: b.id,
                    vm: b.vm,
                    base_vm: before.vm,
                    vmin,
                    vmax,
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
        let (interfaces, over) = self.interfaces(
            |r| {
                let b = &solved.branches[r];
                if b.online {
                    result.branch_flows[r][0]
                } else {
                    0.0
                }
            },
            absolute,
        );
        let worst_interface_pct = over.first().and_then(|i| i.pct).unwrap_or(0.0);
        row.severity = worst_pct.max(voltage_score).max(worst_interface_pct);
        row.interface_violations = over.len();
        row.worst_interface = over.first().cloned();
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
                interfaces,
                interfaces_over: over,
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
    /// Each row's violations, in the same order.
    pub violations: Vec<Violations>,
    outages: Vec<Outage>,
    power: PowerCase,
    base: PowerFlowResult,
    settings: ContingencySettings,
    watch: Watch,
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

/// What [`ContingencyResults::write_violations_csv`] wrote.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct ViolationsExport {
    /// Lines written, one per violation (and one per outage whose AC solve
    /// failed), not counting the header.
    pub rows: usize,
    /// Outages with at least one line.
    pub outages: usize,
}

/// One CSV field: quoted when it holds a comma, quote or line break.
fn csv_field(v: &str) -> std::borrow::Cow<'_, str> {
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\"")).into()
    } else {
        v.into()
    }
}

fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{}", (v * 10_000.0).round() / 10_000.0)
    } else {
        String::new()
    }
}

/// Element ids named PSS/E style: each element's bus names and numbers,
/// then its circuit or unit id; elements joined by " & ". `names` is by
/// bus row and `bus_row` maps bus numbers to rows.
pub(crate) fn element_name(ids: &str, names: &[String], bus_row: &HashMap<u32, usize>) -> String {
    let bus = |n: &str| -> String {
        let name = n
            .parse::<u32>()
            .ok()
            .and_then(|id| bus_row.get(&id))
            .and_then(|&i| names.get(i))
            .map(|s| s.trim())
            .unwrap_or("");
        if name.is_empty() {
            n.to_string()
        } else {
            format!("{name} {n}")
        }
    };
    ids.split('&')
        .map(|id| {
            if let Some(name) = id.strip_prefix("interface:") {
                return name.to_string();
            }
            let parts: Vec<&str> = id.split(':').collect();
            match parts.as_slice() {
                ["bus", n] => bus(n),
                [_, rest @ .., last] if !rest.is_empty() => {
                    let mut out: Vec<String> = rest.iter().map(|b| bus(b)).collect();
                    out.push((*last).to_string());
                    out.join(" ")
                }
                _ => id.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" & ")
}

/// A bus named PSS/E style: its name and number, or the number alone.
fn bus_name(name: &str, number: u32) -> String {
    let name = name.trim();
    if name.is_empty() {
        number.to_string()
    } else {
        format!("{name} {number}")
    }
}

fn opt(v: Option<f64>) -> String {
    v.map(num).unwrap_or_default()
}

/// The header of the violations CSV.
pub const VIOLATIONS_CSV_HEADER: &str = "rank,category,contingency,outage,description,solved,type,element,element_id,from_bus,to_bus,bus,value_pct,before_pct,limit_pct,mw,before_mw,vm_pu,before_vm_pu,vmin_pu,vmax_pu,note,circuit,from_kv,to_kv,bus_kv";

impl ContingencyResults {
    /// Write every violation of the run as CSV, one line each, in rank
    /// order: branch overloads, bus voltages and interfaces as reported
    /// (new or worse by the run's thresholds, or every one when it reports
    /// existing problems), and a line for each outage whose AC solve
    /// failed. Streams, so a large run never sits in memory as text.
    pub fn write_violations_csv(
        &self,
        out: &mut impl std::io::Write,
    ) -> std::io::Result<ViolationsExport> {
        writeln!(out, "{VIOLATIONS_CSV_HEADER}")?;
        let kv: HashMap<u32, f64> = self
            .power
            .case
            .buses
            .iter()
            .map(|b| (b.id, b.base_kv))
            .collect();
        let kv_of = |bus: u32| kv.get(&bus).map(|&v| num(v)).unwrap_or_default();
        // The circuit is the last part of a branch id (line:1:2:1).
        let circuit = |element: &str| {
            element
                .rsplit_once(':')
                .map(|(_, c)| c.to_string())
                .unwrap_or_default()
        };
        let mut written = ViolationsExport {
            rows: 0,
            outages: 0,
        };
        for (row, found) in self.rows.iter().zip(&self.violations) {
            // The name, then the ids of what goes out (none for P0).
            let outage = if row.kind == "base" {
                ""
            } else {
                row.element.as_str()
            };
            let solved = match (row.checked, row.converged) {
                ("ac", Some(false)) => "ac_not_converged",
                ("ac", _) => "ac",
                _ => "linear",
            };
            let lead = format!(
                "{},{},{},{},{},{}",
                row.rank,
                csv_field(row.category.as_deref().unwrap_or("")),
                csv_field(&row.name),
                csv_field(outage),
                csv_field(row.description.as_deref().unwrap_or("")),
                solved,
            );
            let before = written.rows;
            if row.converged == Some(false) {
                writeln!(
                    out,
                    "{lead},not_converged,,,,,,,,,,,,,,,{},,,,",
                    csv_field(row.note.as_deref().unwrap_or(""))
                )?;
                written.rows += 1;
            } else {
                for o in &found.overloads {
                    writeln!(
                        out,
                        "{lead},branch,{},{},{},{},,{},{},{},{},{},,,,,,{},{},{},",
                        csv_field(&o.name),
                        csv_field(&o.element),
                        o.from,
                        o.to,
                        num(o.pct),
                        num(o.base_pct),
                        num(o.limit_pct),
                        num(o.mw),
                        num(o.base_mw),
                        csv_field(&circuit(&o.element)),
                        kv_of(o.from),
                        kv_of(o.to),
                    )?;
                }
                for v in &found.voltage_problems {
                    writeln!(
                        out,
                        "{lead},voltage,{},bus:{},,,{},,,,,,{},{},{},{},,,,,{}",
                        csv_field(&bus_name(&v.name, v.bus)),
                        v.bus,
                        v.bus,
                        num(v.vm),
                        num(v.base_vm),
                        num(v.vmin),
                        num(v.vmax),
                        kv_of(v.bus),
                    )?;
                }
                for i in &found.interfaces {
                    writeln!(
                        out,
                        "{lead},interface,{},,,,,{},{},{},{},{},,,,,,,,,",
                        csv_field(&i.name),
                        opt(i.pct),
                        opt(i.base_pct),
                        num(i.limit_pct),
                        num(i.mw),
                        num(i.base_mw),
                    )?;
                }
                written.rows +=
                    found.overloads.len() + found.voltage_problems.len() + found.interfaces.len();
            }
            if written.rows > before {
                written.outages += 1;
            }
        }
        out.flush()?;
        Ok(written)
    }

    /// What the run was asked to do.
    pub fn settings(&self) -> &ContingencySettings {
        &self.settings
    }

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
        let context = Context::new(&self.power, &self.base, &self.settings, &self.watch)?;
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
            interfaces: outcome.interfaces,
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
            let context = Context::new(&self.power, &self.base, &self.settings, &self.watch)?;
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
