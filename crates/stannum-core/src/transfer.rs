//! Transfer limits (DFAX and FCITC): how much power can move from a source
//! subsystem to a sink subsystem before a monitored branch or interface
//! reaches its limit, in the base case or after a contingency.
//!
//! The transfer is linear (DC) on top of the solved AC base case, as PSS/E's
//! DFAX and TLTG do it. The source raises its output and the sink lowers its
//! own, shared out by the scaling method. Each MW of transfer moves every
//! branch by its transfer distribution factor (PTDF); after an outage, by
//! the outage transfer distribution factor (OTDF). For a monitored element
//! with flow `F` (before the transfer), limit `L` and factor `d`, the
//! transfer that loads it to its limit is `(L - F) / d` when `d > 0` and
//! `(-L - F) / d` when `d < 0`: the first change of total transfer capability
//! (FCITC) is the smallest of these over every element and contingency
//! whose factor is at least the cutoff.

use crate::contingency::{self, ContingencySettings, Outage, Rating, RunHandle};
use crate::power::PowerCase;
use rs_power::{Case, DcFactors, PowerFlowResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// A set of buses, as a `.stnl` subsystem: listed areas, zones, owners and
/// buses combine as a union (every bus when all are empty), then the kV
/// filters narrow it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Subsystem {
    #[serde(default)]
    pub areas: Vec<u32>,
    #[serde(default)]
    pub zones: Vec<u32>,
    #[serde(default)]
    pub owners: Vec<u32>,
    #[serde(default)]
    pub buses: Vec<u32>,
    /// Only buses at or above this base voltage, kV.
    #[serde(default)]
    pub kv_min: Option<f64>,
    /// Only buses at or below this base voltage, kV.
    #[serde(default)]
    pub kv_max: Option<f64>,
}

impl Subsystem {
    fn is_everything(&self) -> bool {
        self.areas.is_empty()
            && self.zones.is_empty()
            && self.owners.is_empty()
            && self.buses.is_empty()
    }

    /// Whether case bus row `row` is a member.
    fn contains(&self, power: &PowerCase, row: usize) -> bool {
        let bus = &power.case.buses[row];
        let listed = self.is_everything()
            || self.areas.contains(&bus.area)
            || self.zones.contains(&bus.zone)
            || self.buses.contains(&bus.id)
            || power
                .bus_owners
                .get(&bus.id)
                .is_some_and(|o| self.owners.contains(o));
        // A bus with no base voltage passes the kV filters.
        let kv = bus.base_kv;
        listed
            && (kv <= 0.0 || self.kv_min.is_none_or(|min| kv >= min))
            && (kv <= 0.0 || self.kv_max.is_none_or(|max| kv <= max))
    }

    fn bus_ids(&self, power: &PowerCase) -> HashSet<u32> {
        (0..power.case.buses.len())
            .filter(|&r| self.contains(power, r))
            .map(|r| power.case.buses[r].id)
            .collect()
    }
}

/// How the source and sink share out the transfer.
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Scaling {
    /// Online generators, in proportion to their output now.
    #[default]
    Generation,
    /// Online generators, in proportion to the room they have: up to Pmax
    /// in the source, down to Pmin in the sink. Every unit reaches its
    /// limit at the same transfer.
    Reserve,
    /// Loads, in proportion to their MW: the source's loads drop and the
    /// sink's grow.
    Load,
}

/// Which branches and interfaces to watch.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MonitorSettings {
    /// Every rated branch in service (lines and transformers), narrowed by
    /// `subsystem` and `kv_min`.
    #[serde(default = "yes")]
    pub branches: bool,
    /// Only branches with a terminal bus in this subsystem.
    #[serde(default)]
    pub subsystem: Option<Subsystem>,
    /// Only branches with a terminal bus at or above this voltage, kV.
    #[serde(default)]
    pub kv_min: f64,
    /// Branches watched as well as (or, with `branches` off, instead of)
    /// the ones above, by id.
    #[serde(default)]
    pub elements: Vec<String>,
    /// Flowgates: weighted sums of branch flows with their own limit.
    #[serde(default)]
    pub interfaces: Vec<Interface>,
}

impl Default for MonitorSettings {
    fn default() -> Self {
        Self {
            branches: true,
            subsystem: None,
            kv_min: 0.0,
            elements: Vec::new(),
            interfaces: Vec::new(),
        }
    }
}

/// A flowgate: branch flows summed with weights, as a `.stnl` interface.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Interface {
    pub name: String,
    /// Branches as `[element, weight]`; flow measured at the from end of
    /// the branch row.
    pub elements: Vec<(String, f64)>,
    /// The interface's limit in MW, the same in both directions.
    pub limit_mw: f64,
}

/// Which outages to study the transfer under. The base case is always
/// studied too.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TransferContingencies {
    /// Ids to take out, one at a time, as for contingency analysis (ids
    /// joined by "&" go out together). When absent, every in-service
    /// branch and/or generator, as chosen below.
    #[serde(default)]
    pub elements: Option<Vec<String>>,
    #[serde(default = "yes")]
    pub branches: bool,
    #[serde(default)]
    pub generators: bool,
    /// Only branch outages with a terminal bus at or above this voltage, kV.
    #[serde(default)]
    pub kv_min: f64,
    /// Leave out zero-impedance lines (PSS/E THRSHZ, 0.0001 p.u.).
    #[serde(default)]
    pub skip_zero_impedance: bool,
}

impl Default for TransferContingencies {
    fn default() -> Self {
        Self {
            elements: None,
            branches: true,
            generators: false,
            kv_min: 0.0,
            skip_zero_impedance: false,
        }
    }
}

fn yes() -> bool {
    true
}
fn default_cutoff() -> f64 {
    0.05
}
fn default_limit() -> f64 {
    100.0
}
fn default_rows() -> usize {
    200
}

/// A transfer limit study.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TransferSettings {
    /// Where the power comes from.
    #[serde(default)]
    pub source: Subsystem,
    /// Where it goes.
    #[serde(default)]
    pub sink: Subsystem,
    #[serde(default)]
    pub scaling: Scaling,
    #[serde(default)]
    pub monitor: MonitorSettings,
    /// Outages to study; `None` studies the base case only.
    #[serde(default)]
    pub contingencies: Option<TransferContingencies>,
    /// Elements whose distribution factor is below this (as a fraction,
    /// 0.05 = 5%) in size don't limit the transfer.
    #[serde(default = "default_cutoff")]
    pub cutoff: f64,
    /// Rating after an outage, and in the base case (`base_rating`).
    #[serde(default)]
    pub rating: Rating,
    #[serde(default)]
    pub base_rating: Rating,
    /// Percent of the rating that counts as the limit.
    #[serde(default = "default_limit")]
    pub limit_pct: f64,
    /// The most rows kept in each of the results' lists.
    #[serde(default = "default_rows")]
    pub max_rows: usize,
    /// Let elements already past their limit before any transfer set the
    /// FCITC (negative). Off by default: they are listed apart in
    /// `past_limits`, and the FCITC is the transfer that first takes an
    /// element within its limit past it.
    #[serde(default)]
    pub include_past_limits: bool,
    /// A lists file (`.stnl`). Its contingencies and contingency sets,
    /// when it has any, are the outages studied instead of
    /// `contingencies`; its flow and interface monitors, when it has any,
    /// are what is watched instead of `monitor`.
    #[serde(default)]
    pub lists: Option<String>,
    /// The lists file's `transfer` record to run: its source, sink,
    /// scaling, cutoff, ratings and limit replace the ones above. The
    /// file's only transfer when absent.
    #[serde(default)]
    pub transfer: Option<String>,
}

/// One limit the transfer runs into: a monitored element, in the base case
/// or after a contingency.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct TransferLimit {
    /// The branch id, or `interface:<name>`.
    pub monitored: String,
    /// End buses of a branch; 0 for an interface.
    pub from: u32,
    pub to: u32,
    /// 1-3 for a leg of a three-winding transformer.
    pub winding: u8,
    /// The monitored element named PSS/E style (bus names and numbers,
    /// then the circuit), or the interface's name.
    pub monitored_name: String,
    /// What is out, "" for the base case.
    pub contingency: String,
    /// The contingency's name: a lists file's name, or PSS/E style;
    /// "BASE CASE" for the base case.
    pub contingency_name: String,
    /// The transfer that brings the element to its limit, MW. Negative
    /// when it is already past it in the direction the transfer pushes.
    pub fcitc_mw: f64,
    /// Flow before the transfer (after the contingency), MW, from end.
    pub flow_mw: f64,
    pub limit_mw: f64,
    /// Distribution factor: PTDF in the base case, OTDF after an outage.
    pub factor: f64,
    /// Base case PTDF.
    pub ptdf: f64,
}

/// Base case PTDF of one monitored element.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct TransferFactor {
    pub monitored: String,
    pub monitored_name: String,
    pub from: u32,
    pub to: u32,
    pub winding: u8,
    pub ptdf: f64,
    pub flow_mw: f64,
    /// Base rating limit, MW; 0 when unrated.
    pub limit_mw: f64,
}

/// How far the source and sink can move before a generator or load runs
/// out of room.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Participation {
    /// Buses taking part.
    pub buses: usize,
    /// Generators (or loads) taking part.
    pub units: usize,
    /// MW the side can move in total: room to Pmax (source) or to Pmin
    /// (sink) for generator scaling, load to drop for a load source; `None`
    /// when unbounded (a sink of loads).
    pub available_mw: Option<f64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct TransferSummary {
    /// The first change of total transfer capability from network limits,
    /// MW; `None` when no monitored element reaches the cutoff.
    pub fcitc_mw: Option<f64>,
    pub limiting: Option<TransferLimit>,
    /// The most the source and sink can move: the smaller of their
    /// available MW.
    pub generation_limit_mw: Option<f64>,
    /// The transfer limit: the smaller of the two above.
    pub transfer_limit_mw: Option<f64>,
    /// "network", "generation" or "none".
    #[schemars(extend("enum" = ["network", "generation", "none"]))]
    pub limited_by: &'static str,
    pub source: Participation,
    pub sink: Participation,
    pub scaling: Scaling,
    pub cutoff: f64,
    pub rating: Rating,
    pub base_rating: Rating,
    pub limit_pct: f64,
    /// Contingencies studied (the base case not counted), and those left
    /// out because they split the network.
    pub contingencies: usize,
    pub islanding: usize,
    /// Monitored branch rows and interfaces.
    pub monitored: usize,
    pub elapsed_ms: f64,
    pub cancelled: bool,
    /// Things worth knowing about the setup, in words.
    pub notes: Vec<String>,
}

/// The worst limit of one contingency.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ContingencyLimit {
    pub contingency: String,
    /// As in [`TransferLimit::contingency_name`].
    pub contingency_name: String,
    pub fcitc_mw: f64,
    pub monitored: String,
    pub monitored_name: String,
    pub factor: f64,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct TransferResults {
    pub summary: TransferSummary,
    /// Each monitored element's lowest limit over the base case and every
    /// contingency, lowest first.
    pub limits: Vec<TransferLimit>,
    /// Elements already past their limit before any transfer (where it
    /// pushes them further), each at its worst, lowest FCITC first. Empty
    /// when `include_past_limits` puts them in `limits`.
    pub past_limits: Vec<TransferLimit>,
    /// Each contingency's lowest limit, lowest first; the base case is "".
    pub contingencies: Vec<ContingencyLimit>,
    /// Base case PTDFs at or above the cutoff, largest first.
    pub factors: Vec<TransferFactor>,
}

/// What one monitored item sums: branch rows with weights.
struct Monitored {
    name: String,
    from: u32,
    to: u32,
    winding: u8,
    rows: Vec<(usize, f64)>,
    /// Limit in the base case and after an outage, MW (0 = unrated).
    base_limit: f64,
    limit: f64,
}

impl Monitored {
    fn of(&self, values: &[f64]) -> f64 {
        self.rows.iter().map(|&(r, w)| w * values[r]).sum()
    }
}

/// Everything a transfer study needs, copied out of the workspace.
pub struct TransferJob {
    power: PowerCase,
    base: PowerFlowResult,
    settings: TransferSettings,
    outages: Vec<Outage>,
    /// What the lists file left out or changed, in words.
    notes: Vec<String>,
    handle: RunHandle,
}

/// The transfer at which `flow` reaches `limit` with factor `d`.
fn fcitc(flow: f64, limit: f64, d: f64) -> f64 {
    if d > 0.0 {
        (limit - flow) / d
    } else {
        (-limit - flow) / d
    }
}

fn kv_at_least(case: &Case, bus_row: &HashMap<u32, usize>, bus: u32, kv_min: f64) -> bool {
    kv_min <= 0.0
        || bus_row.get(&bus).is_none_or(|&r| {
            let kv = case.buses[r].base_kv;
            kv <= 0.0 || kv >= kv_min
        })
}

impl TransferJob {
    pub fn new(
        power: &PowerCase,
        base: &PowerFlowResult,
        settings: TransferSettings,
    ) -> Result<Self, String> {
        let mut settings = settings;
        let mut notes = Vec::new();
        let mut listed = None;
        match settings.lists.clone() {
            Some(path) => {
                let lists = crate::lists::load(&path, power)?;
                apply_lists(power, &mut settings, &lists, &mut notes)?;
                if lists.named.is_empty() && lists.sets.is_empty() {
                    notes.extend(lists.notes);
                } else {
                    // The lists file's own notes come back with its outages.
                    let picker = ContingencySettings {
                        lists: Some(path),
                        ..Default::default()
                    };
                    let (list, _, more) = contingency::outages(power, &picker)?;
                    notes.extend(more);
                    listed = Some(list);
                }
            }
            None if settings.transfer.is_some() => {
                return Err("A transfer named from a lists file needs the lists file.".into());
            }
            None => {}
        }
        if settings.source.is_everything() && settings.sink.is_everything() {
            return Err("Pick a source and a sink for the transfer.".into());
        }
        if !(settings.cutoff.is_finite() && settings.cutoff >= 0.0) {
            return Err("The distribution factor cutoff must be zero or more.".into());
        }
        let outages = match &settings.contingencies {
            _ if listed.is_some() => {
                let mut list = listed.unwrap_or_default();
                list.retain(|o| !o.branch_rows.is_empty() || !o.generator_rows.is_empty());
                list
            }
            None => Vec::new(),
            Some(c) => {
                let picker = ContingencySettings {
                    elements: c.elements.clone(),
                    branches: c.branches,
                    generators: c.generators,
                    skip_zero_impedance: c.skip_zero_impedance,
                    ..Default::default()
                };
                let (mut list, _, _) = contingency::outages(power, &picker)?;
                if c.kv_min > 0.0 {
                    let case = &power.case;
                    let bus_row = bus_rows(case);
                    list.retain(|o| {
                        o.branch_rows.is_empty()
                            || o.branch_rows.iter().any(|&r| {
                                let b = &case.branches[r];
                                kv_at_least(case, &bus_row, b.from, c.kv_min)
                                    || kv_at_least(case, &bus_row, b.to, c.kv_min)
                            })
                    });
                }
                // Shunt and DC line outages don't move a transfer's factors;
                // the base flows they move are left to contingency analysis.
                list.retain(|o| !o.branch_rows.is_empty() || !o.generator_rows.is_empty());
                list
            }
        };
        Ok(Self {
            power: power.clone(),
            base: base.clone(),
            settings,
            outages,
            notes,
            handle: RunHandle::default(),
        })
    }

    pub fn handle(&self) -> RunHandle {
        self.handle.clone()
    }

    /// Contingencies the run studies, the base case not counted.
    pub fn len(&self) -> usize {
        self.outages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outages.is_empty()
    }

    pub fn run(self) -> Result<TransferResults, String> {
        let start = Instant::now();
        let s = &self.settings;
        let case = &self.power.case;
        let solved = &self.base.case;
        let mut notes = self.notes.clone();

        let source_buses = s.source.bus_ids(&self.power);
        let sink_buses = s.sink.bus_ids(&self.power);
        let overlap = source_buses.intersection(&sink_buses).count();
        if overlap > 0 {
            notes.push(format!(
                "The source and sink share {overlap} bus{}; what they shift there cancels out.",
                if overlap == 1 { "" } else { "es" }
            ));
        }
        let (source_w, source) = participation(solved, &source_buses, s.scaling, true);
        let (sink_w, sink) = participation(solved, &sink_buses, s.scaling, false);
        let total = |w: &HashMap<u32, f64>| w.values().sum::<f64>();
        let (source_total, sink_total) = (total(&source_w), total(&sink_w));
        let what = match s.scaling {
            Scaling::Load => "load",
            Scaling::Reserve => "generation with room to move",
            Scaling::Generation => "online generation",
        };
        if source_total <= 0.0 {
            return Err(format!("The source has no {what} to shift."));
        }
        if sink_total <= 0.0 {
            return Err(format!("The sink has no {what} to shift."));
        }
        // MW injected at each bus per MW of transfer.
        let mut injection: HashMap<u32, f64> = HashMap::new();
        for (bus, w) in &source_w {
            *injection.entry(*bus).or_default() += w / source_total;
        }
        for (bus, w) in &sink_w {
            *injection.entry(*bus).or_default() -= w / sink_total;
        }
        let injection: Vec<(u32, f64)> = injection.into_iter().collect();

        let factors = DcFactors::new(case).map_err(|e| e.to_string())?;
        let ptdf = factors.injection_flows(&injection);
        let flows: Vec<f64> = self.base.branch_flows.iter().map(|f| f[0]).collect();
        let monitored = self.monitored()?;
        let names = crate::power::bus_names(case);
        let bus_row = bus_rows(case);
        let monitored_names: Vec<String> = monitored
            .iter()
            .map(|m| contingency::element_name(&m.name, &names, &bus_row))
            .collect();
        if monitored.is_empty() {
            return Err(
                "Nothing to monitor: no rated branch or interface fits the settings.".into(),
            );
        }

        // The base case, then each outage.
        let n = self.outages.len();
        self.handle.set_phase(1, n + 1);
        let next = AtomicUsize::new(0);
        let workers = std::thread::available_parallelism()
            .map_or(1, |p| p.get())
            .min(n.max(1));
        let study = |i: usize| -> Option<(Vec<f64>, Vec<f64>)> {
            if i == 0 {
                return Some((flows.clone(), ptdf.clone()));
            }
            let o = &self.outages[i - 1];
            let mut before = flows.clone();
            let lost: Vec<(u32, f64)> = o
                .generator_rows
                .iter()
                .filter(|&&r| case.generators[r].online)
                .map(|&r| (case.generators[r].bus, -solved.generators[r].pg))
                .collect();
            if !lost.is_empty() {
                for (f, d) in before.iter_mut().zip(factors.injection_flows(&lost)) {
                    *f += d;
                }
            }
            let rows: Vec<usize> = o
                .branch_rows
                .iter()
                .copied()
                .filter(|&r| factors.in_service(r))
                .collect();
            let mut after = factors.trip_each(&rows, &[&before, &ptdf]).ok()?;
            let otdf = after.pop()?;
            Some((after.pop()?, otdf))
        };
        let limit = |m: &Monitored, base_case: bool| {
            if base_case { m.base_limit } else { m.limit }
        };
        let cutoff = s.cutoff;
        let include_past = s.include_past_limits;
        // Each limit with the index of what was out (0 for the base case),
        // so that equal limits keep the earlier one whatever thread found it.
        type Best = Vec<Option<(usize, TransferLimit)>>;
        type Local = (Best, Best, Vec<(usize, ContingencyLimit)>, usize);
        let parts: Vec<Local> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut best: Best = (0..monitored.len()).map(|_| None).collect();
                        let mut past: Best = (0..monitored.len()).map(|_| None).collect();
                        let mut worst = Vec::new();
                        let mut islanding = 0;
                        loop {
                            if self.handle.cancelled() {
                                break;
                            }
                            let i = next.fetch_add(1, Ordering::SeqCst);
                            if i > n {
                                break;
                            }
                            let Some((after, otdf)) = study(i) else {
                                islanding += 1;
                                self.handle.step();
                                continue;
                            };
                            let (contingency, contingency_name) = if i == 0 {
                                (String::new(), "BASE CASE".to_string())
                            } else {
                                let o = &self.outages[i - 1];
                                let name = o.name.clone().unwrap_or_else(|| {
                                    contingency::element_name(&o.element, &names, &bus_row)
                                });
                                (o.element.clone(), name)
                            };
                            let mut lowest: Option<ContingencyLimit> = None;
                            for (k, m) in monitored.iter().enumerate() {
                                let lim = limit(m, i == 0);
                                let d = m.of(&otdf);
                                if lim <= 0.0 || d.abs() < cutoff || d == 0.0 {
                                    continue;
                                }
                                let f = m.of(&after);
                                let t = fcitc(f, lim, d);
                                // Already past the limit before any transfer:
                                // kept apart unless asked for.
                                let slot = if t < 0.0 && !include_past {
                                    &mut past[k]
                                } else {
                                    &mut best[k]
                                };
                                if slot.as_ref().is_none_or(|(_, b)| t < b.fcitc_mw) {
                                    *slot = Some((
                                        i,
                                        TransferLimit {
                                            monitored: m.name.clone(),
                                            monitored_name: monitored_names[k].clone(),
                                            from: m.from,
                                            to: m.to,
                                            winding: m.winding,
                                            contingency: contingency.clone(),
                                            contingency_name: contingency_name.clone(),
                                            fcitc_mw: t,
                                            flow_mw: f,
                                            limit_mw: lim,
                                            factor: d,
                                            ptdf: m.of(&ptdf),
                                        },
                                    ));
                                }
                                if (t >= 0.0 || include_past)
                                    && lowest.as_ref().is_none_or(|l| t < l.fcitc_mw)
                                {
                                    lowest = Some(ContingencyLimit {
                                        contingency: contingency.clone(),
                                        contingency_name: contingency_name.clone(),
                                        fcitc_mw: t,
                                        monitored: m.name.clone(),
                                        monitored_name: monitored_names[k].clone(),
                                        factor: d,
                                    });
                                }
                            }
                            worst.extend(lowest.map(|l| (i, l)));
                            self.handle.step();
                        }
                        (best, past, worst, islanding)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
                .collect()
        });
        let cancelled = self.handle.cancelled();

        let merge = |into: &mut Best, part: Best| {
            for (slot, candidate) in into.iter_mut().zip(part) {
                if let Some((ci, c)) = candidate
                    && slot.as_ref().is_none_or(|(bi, b)| {
                        c.fcitc_mw < b.fcitc_mw || (c.fcitc_mw == b.fcitc_mw && ci < *bi)
                    })
                {
                    *slot = Some((ci, c));
                }
            }
        };
        let mut best: Best = (0..monitored.len()).map(|_| None).collect();
        let mut past: Best = (0..monitored.len()).map(|_| None).collect();
        let mut by_contingency = Vec::new();
        let mut islanding = 0;
        for (part, part_past, worst, cut) in parts {
            merge(&mut best, part);
            merge(&mut past, part_past);
            by_contingency.extend(worst);
            islanding += cut;
        }
        let ascending = |list: Best| {
            let mut out: Vec<(usize, TransferLimit)> = list.into_iter().flatten().collect();
            out.sort_by(|(i, a), (j, b)| a.fcitc_mw.total_cmp(&b.fcitc_mw).then(i.cmp(j)));
            out.into_iter().map(|(_, l)| l).collect::<Vec<_>>()
        };
        let mut limits = ascending(best);
        let mut past_limits = ascending(past);
        by_contingency.sort_by(|(i, a), (j, b)| a.fcitc_mw.total_cmp(&b.fcitc_mw).then(i.cmp(j)));
        let mut by_contingency: Vec<ContingencyLimit> =
            by_contingency.into_iter().map(|(_, c)| c).collect();
        let mut table: Vec<TransferFactor> = monitored
            .iter()
            .zip(&monitored_names)
            .map(|(m, name)| TransferFactor {
                monitored: m.name.clone(),
                monitored_name: name.clone(),
                from: m.from,
                to: m.to,
                winding: m.winding,
                ptdf: m.of(&ptdf),
                flow_mw: m.of(&flows),
                limit_mw: m.base_limit,
            })
            .filter(|f| f.ptdf.abs() >= cutoff && f.ptdf != 0.0)
            .collect();
        table.sort_by(|a, b| b.ptdf.abs().total_cmp(&a.ptdf.abs()));

        let fcitc_mw = limits.first().map(|l| l.fcitc_mw);
        let generation_limit_mw = match (source.available_mw, sink.available_mw) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let (transfer_limit_mw, limited_by) = match (fcitc_mw, generation_limit_mw) {
            (Some(f), Some(g)) if g < f => (Some(g), "generation"),
            (Some(f), _) => (Some(f), "network"),
            (None, Some(g)) => (Some(g), "generation"),
            (None, None) => (None, "none"),
        };
        if fcitc_mw.is_none() {
            notes.push(format!(
                "No monitored element has a distribution factor of {:.0}% or more.",
                cutoff * 100.0
            ));
        }
        let past = if include_past {
            limits.iter().filter(|l| l.fcitc_mw < 0.0).count()
        } else {
            past_limits.len()
        };
        if past > 0 {
            notes.push(format!(
                "{past} monitored element{} already past {} limit before any transfer, in the base case or after a contingency{}.",
                if past == 1 { " is" } else { "s are" },
                if past == 1 { "its" } else { "their" },
                if include_past {
                    " (negative FCITC)"
                } else {
                    "; they are listed apart and don't set the FCITC"
                }
            ));
        }
        if islanding > 0 {
            notes.push(format!(
                "{islanding} contingenc{} split the network and {} left out.",
                if islanding == 1 { "y" } else { "ies" },
                if islanding == 1 { "was" } else { "were" }
            ));
        }
        let max = s.max_rows;
        limits.truncate(max);
        past_limits.truncate(max);
        by_contingency.truncate(max);
        table.truncate(max);
        let summary = TransferSummary {
            fcitc_mw,
            limiting: limits.first().cloned(),
            generation_limit_mw,
            transfer_limit_mw,
            limited_by,
            source,
            sink,
            scaling: s.scaling,
            cutoff,
            rating: s.rating,
            base_rating: s.base_rating,
            limit_pct: s.limit_pct,
            contingencies: n,
            islanding,
            monitored: monitored.len(),
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            cancelled,
            notes,
        };
        log::info!(
            "Transfer limit: FCITC {} over {} contingencies and {} monitored elements in {:.0} ms",
            summary
                .fcitc_mw
                .map_or("not reached".to_string(), |v| format!("{v:.1} MW")),
            n,
            summary.monitored,
            summary.elapsed_ms
        );
        Ok(TransferResults {
            summary,
            limits,
            past_limits,
            contingencies: by_contingency,
            factors: table,
        })
    }

    /// The monitored branch rows and interfaces, with their MW limits.
    fn monitored(&self) -> Result<Vec<Monitored>, String> {
        let s = &self.settings;
        let m = &s.monitor;
        let case = &self.power.case;
        let elements = &self.power.elements.branches;
        let bus_row = bus_rows(case);
        let pct = s.limit_pct / 100.0;
        let mut by_element: HashMap<&str, Vec<usize>> = HashMap::new();
        for (r, e) in elements.iter().enumerate() {
            by_element.entry(e.element.as_str()).or_default().push(r);
        }
        let in_subsystem: Option<HashSet<u32>> =
            m.subsystem.as_ref().map(|sub| sub.bus_ids(&self.power));
        let row = |r: usize| {
            let b = &case.branches[r];
            let e = elements.get(r);
            Monitored {
                name: e.map(|e| e.element.clone()).unwrap_or_default(),
                from: b.from,
                to: b.to,
                winding: e.map_or(0, |e| e.winding),
                rows: vec![(r, 1.0)],
                base_limit: s.base_rating.of(b) * pct,
                limit: s.rating.of(b) * pct,
            }
        };
        let mut out = Vec::new();
        let mut taken = HashSet::new();
        if m.branches {
            for (r, b) in case.branches.iter().enumerate() {
                let ends = [b.from, b.to];
                let wanted = b.online
                    && (s.base_rating.of(b) > 0.0 || s.rating.of(b) > 0.0)
                    && !elements
                        .get(r)
                        .is_some_and(|e| e.element.starts_with("switch:"))
                    && ends
                        .iter()
                        .any(|&bus| kv_at_least(case, &bus_row, bus, m.kv_min))
                    && in_subsystem
                        .as_ref()
                        .is_none_or(|ids| ends.iter().any(|bus| ids.contains(bus)));
                if wanted && taken.insert(r) {
                    out.push(row(r));
                }
            }
        }
        for id in &m.elements {
            let rows = by_element
                .get(id.as_str())
                .ok_or_else(|| format!("{id} isn't a branch in this case."))?;
            for &r in rows {
                if case.branches[r].online && taken.insert(r) {
                    out.push(row(r));
                }
            }
        }
        for i in &m.interfaces {
            let mut rows = Vec::new();
            for (id, w) in &i.elements {
                let found = by_element.get(id.as_str()).ok_or_else(|| {
                    format!("{id} in interface {} isn't a branch in this case.", i.name)
                })?;
                // A three-winding transformer counts at its first winding.
                rows.push((found[0], *w));
            }
            out.push(Monitored {
                name: format!("interface:{}", i.name),
                from: 0,
                to: 0,
                winding: 0,
                rows,
                base_limit: i.limit_mw,
                limit: i.limit_mw,
            });
        }
        Ok(out)
    }
}

/// Take the transfer and what it watches from a lists file: its `transfer`
/// record (the one named, or its only one) and its flow and interface
/// monitors. The record's ratings and limit apply to the monitors.
fn apply_lists(
    power: &PowerCase,
    s: &mut TransferSettings,
    lists: &crate::lists::Lists,
    notes: &mut Vec<String>,
) -> Result<(), String> {
    use crate::lists::Watched;
    let chosen = match &s.transfer {
        Some(name) => Some(
            lists
                .transfers
                .iter()
                .find(|t| &t.name == name)
                .ok_or_else(|| format!("The lists file has no transfer {name}."))?,
        ),
        None if lists.transfers.len() == 1 => lists.transfers.first(),
        None => None,
    };
    if chosen.is_none() && lists.transfers.len() > 1 {
        let names: Vec<&str> = lists.transfers.iter().map(|t| t.name.as_str()).collect();
        return Err(format!(
            "The lists file has {} transfers; name the one to run ({}).",
            names.len(),
            names.join(", ")
        ));
    }
    if let Some(t) = chosen {
        let side = |buses: &HashSet<u32>, what: &str| {
            if buses.is_empty() {
                return Err(format!(
                    "Transfer \"{}\": its {what} has no buses in this case.",
                    t.name
                ));
            }
            let mut buses: Vec<u32> = buses.iter().copied().collect();
            buses.sort_unstable();
            Ok(Subsystem {
                buses,
                ..Default::default()
            })
        };
        s.source = side(&t.source, "source")?;
        s.sink = side(&t.sink, "sink")?;
        s.scaling = match t.scaling.as_str() {
            "reserve" => Scaling::Reserve,
            "load" => Scaling::Load,
            _ => Scaling::Generation,
        };
        s.cutoff = t.cutoff_pct / 100.0;
        let rating = |slot: u8| match slot {
            2 => Rating::B,
            3 => Rating::C,
            _ => Rating::A,
        };
        s.base_rating = rating(t.base_rating);
        s.rating = rating(t.rating);
        s.limit_pct = t.limit_pct;
        notes.push(format!(
            "Transfer \"{}\" from the lists file: {} source and {} sink bus{}.",
            t.name,
            t.source.len(),
            t.sink.len(),
            if t.sink.len() == 1 { "" } else { "es" }
        ));
    }
    // The monitors, when the file has flow or interface monitors.
    let branches: HashSet<&str> = power
        .elements
        .branches
        .iter()
        .map(|e| e.element.as_str())
        .collect();
    let mut m = MonitorSettings {
        branches: false,
        subsystem: None,
        kv_min: 0.0,
        elements: Vec::new(),
        interfaces: Vec::new(),
    };
    let (mut any, mut whole, mut voltage) = (false, false, 0);
    let mut buses: HashSet<u32> = HashSet::new();
    for mon in &lists.monitors {
        match &mon.watched {
            Watched::Flow {
                element: Some(e), ..
            } => {
                if branches.contains(e.as_str()) {
                    m.elements.push(e.clone());
                    any = true;
                } else {
                    notes.push(format!(
                        "Monitor \"{}\": {e} isn't a branch in this case; skipped.",
                        mon.name
                    ));
                }
            }
            Watched::Flow { buses: Some(b), .. } => {
                buses.extend(b);
                any = true;
            }
            Watched::Flow { .. } => {
                whole = true;
                any = true;
            }
            Watched::Interface(i) => {
                let Some(limit_mw) = i.limit_mw else {
                    notes.push(format!(
                        "Monitor \"{}\": interface {} has no limit; skipped.",
                        mon.name, i.name
                    ));
                    continue;
                };
                if let Some((id, _)) = i
                    .elements
                    .iter()
                    .find(|(id, _)| !branches.contains(id.as_str()))
                {
                    notes.push(format!(
                        "Monitor \"{}\": {id} in interface {} isn't a branch in this case; skipped.",
                        mon.name, i.name
                    ));
                    continue;
                }
                m.interfaces.push(Interface {
                    name: i.name.clone(),
                    elements: i.elements.clone(),
                    limit_mw,
                });
                any = true;
            }
            Watched::Voltage { .. } => voltage += 1,
        }
    }
    if voltage > 0 {
        notes.push(format!(
            "{voltage} voltage monitor{} in the lists file left out: a transfer's limit is in MW.",
            if voltage == 1 { "" } else { "s" },
        ));
    }
    if any {
        if whole {
            m.branches = true;
        } else if !buses.is_empty() {
            let mut buses: Vec<u32> = buses.into_iter().collect();
            buses.sort_unstable();
            m.branches = true;
            m.subsystem = Some(Subsystem {
                buses,
                ..Default::default()
            });
        }
        s.monitor = m;
    }
    Ok(())
}

fn bus_rows(case: &Case) -> HashMap<u32, usize> {
    case.buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect()
}

/// Each bus's weight in a side of the transfer, and what the side can move.
/// `source` raises output (or drops load); the sink does the opposite.
fn participation(
    solved: &Case,
    buses: &HashSet<u32>,
    scaling: Scaling,
    source: bool,
) -> (HashMap<u32, f64>, Participation) {
    let mut weights: HashMap<u32, f64> = HashMap::new();
    let mut units = 0;
    let mut available = 0.0;
    match scaling {
        Scaling::Load => {
            for b in solved.buses.iter().filter(|b| buses.contains(&b.id)) {
                let p = b.load().0;
                if p > 0.0 {
                    *weights.entry(b.id).or_default() += p;
                    units += 1;
                    available += p;
                }
            }
        }
        Scaling::Generation | Scaling::Reserve => {
            for g in solved
                .generators
                .iter()
                .filter(|g| g.online && buses.contains(&g.bus))
            {
                let room = if source { g.pmax - g.pg } else { g.pg - g.pmin }.max(0.0);
                let w = match scaling {
                    Scaling::Reserve => room,
                    _ => g.pg.max(0.0),
                };
                if w > 0.0 {
                    *weights.entry(g.bus).or_default() += w;
                    units += 1;
                    available += room;
                }
            }
        }
    }
    let side = Participation {
        buses: weights.len(),
        units,
        available_mw: (scaling != Scaling::Load || source).then_some(available),
    };
    (weights, side)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case9() -> (PowerCase, PowerFlowResult) {
        let text = include_str!("../../rs-power/tests/fixtures/case9.m");
        let case = rs_power::parse_matpower(text).unwrap();
        let power = crate::power::from_case(case);
        let solved = rs_power::solve(&power.case, &rs_power::PowerFlowOptions::default()).unwrap();
        (power, solved)
    }

    fn settings(source: u32, sink: u32) -> TransferSettings {
        TransferSettings {
            source: Subsystem {
                buses: vec![source],
                ..Default::default()
            },
            sink: Subsystem {
                buses: vec![sink],
                ..Default::default()
            },
            scaling: Scaling::Generation,
            monitor: MonitorSettings::default(),
            contingencies: Some(TransferContingencies::default()),
            cutoff: 0.05,
            rating: Rating::A,
            base_rating: Rating::A,
            limit_pct: 100.0,
            max_rows: 200,
            include_past_limits: false,
            lists: None,
            transfer: None,
        }
    }

    /// Settings that take everything from the lists file `text`.
    fn from_lists(text: &str, tag: &str) -> (TransferSettings, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "stannum-transfer-{tag}-{}.stnl",
            std::process::id()
        ));
        std::fs::write(&path, text).unwrap();
        let s = TransferSettings {
            source: Subsystem::default(),
            sink: Subsystem::default(),
            contingencies: None,
            lists: Some(path.to_string_lossy().into_owned()),
            ..settings(1, 1)
        };
        (s, path)
    }

    #[test]
    fn a_lists_file_transfer_runs_as_written() {
        let (power, base) = case9();
        let text = "stannum-lists 0.1\n\
            subsystem a buses=[2]\n\
            subsystem b buses=[3]\n\
            contingency_set n1 kind=n_1_branches\n\
            monitor v kind=voltage element=bus:5\n\
            transfer t source=a sink=b scaling=reserve rating=2 cutoff_pct=1\n";
        let (s, path) = from_lists(text, "as-written");
        let listed = TransferJob::new(&power, &base, s).unwrap().run();
        std::fs::remove_file(&path).ok();
        let listed = listed.unwrap();
        let mut direct = settings(2, 3);
        direct.scaling = Scaling::Reserve;
        direct.rating = Rating::B;
        direct.cutoff = 0.01;
        let direct = TransferJob::new(&power, &base, direct)
            .unwrap()
            .run()
            .unwrap();
        let (a, b) = (&listed.summary, &direct.summary);
        assert_eq!(a.fcitc_mw, b.fcitc_mw);
        assert_eq!(a.contingencies, b.contingencies);
        assert_eq!(a.monitored, b.monitored);
        assert_eq!(a.rating, Rating::B);
        assert_eq!(listed.limits, direct.limits);
        assert!(
            a.notes.iter().any(|n| n.contains("1 voltage monitor")),
            "{:?}",
            a.notes
        );
    }

    #[test]
    fn limits_are_named_by_buses() {
        let (power, base) = case9();
        let r = TransferJob::new(&power, &base, settings(2, 3))
            .unwrap()
            .run()
            .unwrap();
        assert!(!r.limits.is_empty());
        for l in r.limits.iter().chain(&r.past_limits) {
            assert!(!l.monitored_name.contains(':'), "{}", l.monitored_name);
            if l.contingency.is_empty() {
                assert_eq!(l.contingency_name, "BASE CASE");
            } else {
                assert!(!l.contingency_name.contains(':'), "{}", l.contingency_name);
            }
        }
        // case9 has no bus names: a branch is its bus numbers then circuit.
        let f = &r.factors[0];
        let parts: Vec<&str> = f.monitored.split(':').skip(1).collect();
        assert_eq!(f.monitored_name, parts.join(" "));
        assert!(
            r.contingencies
                .iter()
                .all(|c| !c.contingency_name.is_empty())
        );
    }

    #[test]
    fn a_lists_file_chooses_what_is_watched() {
        let (power, base) = case9();
        let line = power.elements.branches[3].element.clone();
        let other = power.elements.branches[4].element.clone();
        let text = format!(
            "stannum-lists 0.1\n\
            subsystem a buses=[2]\n\
            subsystem b buses=[3]\n\
            monitor one element={line}\n\
            monitor gone element=line:98:99:1\n\
            interface gate elements=[[{line}, 1], [{other}, -1]] limit_mw=150\n\
            monitor gate kind=interface interface=gate\n\
            transfer t source=a sink=b\n\
            transfer u source=b sink=a\n"
        );
        let (mut s, path) = from_lists(&text, "watched");
        let unnamed = TransferJob::new(&power, &base, s.clone()).err();
        s.transfer = Some("u".into());
        let r = TransferJob::new(&power, &base, s.clone()).map(|j| j.run());
        s.transfer = Some("nope".into());
        let missing = TransferJob::new(&power, &base, s).err();
        std::fs::remove_file(&path).ok();
        assert!(unnamed.unwrap().contains("2 transfers; name the one"));
        assert!(missing.unwrap().contains("no transfer nope"));
        let r = r.unwrap().unwrap();
        // The branch and the interface; no outages in the file, none asked.
        assert_eq!((r.summary.monitored, r.summary.contingencies), (2, 0));
        assert_eq!(r.summary.source.buses, 1);
        assert!(r.factors.iter().any(|f| f.monitored == "interface:gate"));
        assert!(
            r.summary
                .notes
                .iter()
                .any(|n| n.contains("line:98:99:1 isn't a branch")),
            "{:?}",
            r.summary.notes
        );
    }

    #[test]
    fn a_transfer_needs_its_sides_and_its_file() {
        let (power, base) = case9();
        let mut s = settings(2, 3);
        s.transfer = Some("t".into());
        assert!(TransferJob::new(&power, &base, s).is_err());
        let mut s = settings(2, 3);
        s.source = Subsystem::default();
        s.sink = Subsystem::default();
        assert!(
            TransferJob::new(&power, &base, s)
                .err()
                .unwrap()
                .contains("Pick a source and a sink")
        );
    }

    #[test]
    fn base_case_factors_match_the_ptdf_matrix() {
        let (power, base) = case9();
        let ptdf = rs_power::make_ptdf(&power.case, None).unwrap();
        let mut s = settings(2, 3);
        s.contingencies = None;
        s.cutoff = 0.0;
        let r = TransferJob::new(&power, &base, s).unwrap().run().unwrap();
        // Buses 2 and 3 are rows 1 and 2 of case9.
        for f in &r.factors {
            let row = power
                .case
                .branches
                .iter()
                .position(|b| (b.from, b.to) == (f.from, f.to))
                .unwrap();
            let expected = ptdf[row][1] - ptdf[row][2];
            assert!(
                (f.ptdf - expected).abs() < 1e-9,
                "{}: {} vs {expected}",
                f.monitored,
                f.ptdf
            );
        }
        assert_eq!(r.summary.contingencies, 0);
        assert!(r.limits.iter().all(|l| l.contingency.is_empty()));
    }

    #[test]
    fn fcitc_brings_the_limiting_branch_to_its_rating() {
        let (power, base) = case9();
        let r = TransferJob::new(&power, &base, settings(2, 3))
            .unwrap()
            .run()
            .unwrap();
        let l = r.summary.limiting.clone().expect("a limit");
        assert_eq!(r.summary.fcitc_mw, Some(l.fcitc_mw));
        // Moving the FCITC puts the element exactly at its limit.
        let at = l.flow_mw + l.factor * l.fcitc_mw;
        assert!(
            (at.abs() - l.limit_mw).abs() < 1e-6,
            "{at} vs {}",
            l.limit_mw
        );
        // Every other limit is higher.
        assert!(r.limits.iter().all(|x| x.fcitc_mw >= l.fcitc_mw));
        assert!(r.contingencies.iter().all(|x| x.fcitc_mw >= l.fcitc_mw));
        // case9's generator step-ups split the network when they trip.
        assert!(r.summary.islanding > 0);
        assert_eq!(r.summary.contingencies, power.case.branches.len());
    }

    #[test]
    fn contingency_factors_match_lodf_otdf() {
        let (power, base) = case9();
        let ptdf = rs_power::make_ptdf(&power.case, None).unwrap();
        let lodf = rs_power::make_lodf(&power.case, &ptdf).unwrap();
        let mut s = settings(2, 3);
        s.cutoff = 0.0;
        s.contingencies = Some(TransferContingencies {
            elements: Some(vec![power.elements.branches[5].element.clone()]),
            ..Default::default()
        });
        let r = TransferJob::new(&power, &base, s).unwrap().run().unwrap();
        let k = 5;
        let pk = ptdf[k][1] - ptdf[k][2];
        for l in r.limits.iter().filter(|l| !l.contingency.is_empty()) {
            let m = power
                .elements
                .branches
                .iter()
                .position(|e| e.element == l.monitored)
                .unwrap();
            let expected = (ptdf[m][1] - ptdf[m][2]) + lodf[m][k] * pk;
            assert!(
                (l.factor - expected).abs() < 1e-9,
                "{}: {} vs {expected}",
                l.monitored,
                l.factor
            );
        }
    }

    #[test]
    fn generation_limit_is_the_smaller_side() {
        let (power, base) = case9();
        let mut s = settings(2, 3);
        s.scaling = Scaling::Reserve;
        let r = TransferJob::new(&power, &base, s).unwrap().run().unwrap();
        let g2 = base.case.generators.iter().find(|g| g.bus == 2).unwrap();
        let g3 = base.case.generators.iter().find(|g| g.bus == 3).unwrap();
        assert_eq!(r.summary.source.available_mw, Some(g2.pmax - g2.pg));
        assert_eq!(r.summary.sink.available_mw, Some(g3.pg - g3.pmin));
        let g = (g2.pmax - g2.pg).min(g3.pg - g3.pmin);
        assert_eq!(r.summary.generation_limit_mw, Some(g));
    }

    #[test]
    fn interface_sums_its_branches() {
        let (power, base) = case9();
        let mut s = settings(2, 3);
        s.contingencies = None;
        s.cutoff = 0.0;
        let a = power.elements.branches[3].element.clone();
        let b = power.elements.branches[4].element.clone();
        s.monitor.interfaces = vec![Interface {
            name: "cut".into(),
            elements: vec![(a.clone(), 1.0), (b.clone(), -1.0)],
            limit_mw: 50.0,
        }];
        let r = TransferJob::new(&power, &base, s).unwrap().run().unwrap();
        let get = |name: &str| {
            r.factors
                .iter()
                .find(|f| f.monitored == name)
                .map(|f| f.ptdf)
        };
        let i = r
            .limits
            .iter()
            .find(|l| l.monitored == "interface:cut")
            .unwrap();
        assert!((i.ptdf - (get(&a).unwrap_or(0.0) - get(&b).unwrap_or(0.0))).abs() < 1e-9);
        assert_eq!(i.limit_mw, 50.0);
    }

    #[test]
    fn elements_already_past_their_limit_are_kept_apart() {
        // A 1 MW rating on the limiting line puts it past its limit.
        let (mut power, base) = case9();
        let r = TransferJob::new(&power, &base, settings(2, 3))
            .unwrap()
            .run()
            .unwrap();
        let row = power
            .elements
            .branches
            .iter()
            .position(|e| e.element == r.summary.limiting.as_ref().unwrap().monitored)
            .unwrap();
        power.case.branches[row].rate_a = 1.0;
        let apart = TransferJob::new(&power, &base, settings(2, 3))
            .unwrap()
            .run()
            .unwrap();
        assert!(apart.limits.iter().all(|l| l.fcitc_mw >= 0.0));
        assert!(apart.past_limits.iter().any(
            |l| l.from == power.case.branches[row].from && l.to == power.case.branches[row].to
        ));
        assert!(apart.summary.fcitc_mw.unwrap() >= 0.0);
        let mut s = settings(2, 3);
        s.include_past_limits = true;
        let together = TransferJob::new(&power, &base, s).unwrap().run().unwrap();
        assert!(together.past_limits.is_empty());
        assert!(together.summary.fcitc_mw.unwrap() < 0.0);
    }

    #[test]
    fn a_side_with_nothing_to_shift_is_an_error() {
        let (power, base) = case9();
        // Bus 5 has load but no generator.
        let err = TransferJob::new(&power, &base, settings(5, 3))
            .unwrap()
            .run()
            .unwrap_err();
        assert!(err.contains("source"), "{err}");
    }
}
