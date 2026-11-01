//! Secure transfer limits: a transfer grown step by step, with every
//! contingency solved in AC at each step, until a contingency (or the base
//! case) shows a new violation or stops solving.
//!
//! The transfer moves the source and sink as PV curves and transfer limits
//! do. It grows by the major step; when a step finds a new problem the run
//! goes back to the last clean step and walks forward by the minor step to
//! pin down where it starts. Thermal and voltage violations each stop the
//! run or not, as chosen, and a failed solve always does. With
//! `stop_at_violation` off, the run carries on to the nose of the base
//! case (or the largest transfer asked for) and logs every problem it
//! meets on the way.
//!
//! Every step's values of the chosen channels (bus voltages, branch flows,
//! generator outputs, interface flows), for the base case and each
//! contingency, go to a results file (`.stnr`, an Arrow IPC stream) as the
//! run goes, so a large study never holds them all in memory. The answer
//! itself (the limit, what set it, the problems found and each
//! contingency's own limit) comes back in memory, and any channel's curves
//! are read back from the file with [`read_channel`].

use crate::contingency::{ContingencySettings, Outage, RunHandle, StepCheck, StepChecker};
use crate::power::{self, PowerCase, SolveSettings};
use crate::pv::{MIN_ITERATIONS, Side, solve_step};
use crate::transfer::{Participation, Scaling, Subsystem};
use arrow_array::builder::{BooleanBuilder, Float32Builder, Float64Builder, UInt32Builder};
use arrow_array::types::Int32Type;
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_ipc::CompressionType;
use arrow_ipc::writer::{IpcWriteOptions, StreamWriter};
use arrow_schema::{DataType, Field, Schema};
use rs_power::{BusType, Case, PowerFlowResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// The results file's format and version, as `stannum.format`.
pub const SECURE_FORMAT: &str = "stannum-secure-transfer 0.1";

/// Rows per record batch.
const BATCH_ROWS: usize = 16_384;

fn yes() -> bool {
    true
}
fn load_scaling() -> Scaling {
    Scaling::Load
}
fn default_max_points() -> usize {
    200
}

/// A quantity recorded at every step, for the base case and each
/// contingency.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Quantity {
    /// Bus voltage magnitude, p.u.
    Vm,
    /// Bus voltage angle, degrees.
    Va,
    /// Branch active flow at the from end, MW.
    Mw,
    /// Branch reactive flow at the from end, Mvar.
    Mvar,
    /// Branch loading, percent of the contingency rating.
    Pct,
    /// Generator active output, MW.
    Pg,
    /// Generator reactive output, Mvar.
    Qg,
    /// A monitored interface's flow, MW (`element` is its name).
    Interface,
}

/// One channel: a quantity of one element.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Channel {
    pub quantity: Quantity,
    /// `bus:101` for vm and va, a branch id (`line:1:4:1`) for mw, mvar
    /// and pct, a generator id (`generator:3:1`) for pg and qg, or an
    /// interface's name from the monitors.
    pub element: String,
}

/// A secure transfer limit study.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SecureTransferSettings {
    /// Where the power comes from. Every bus when left empty.
    #[serde(default)]
    pub source: Subsystem,
    /// How the source shares out its part: generation by default.
    #[serde(default)]
    pub source_scaling: Scaling,
    /// Where the power goes.
    #[serde(default)]
    pub sink: Subsystem,
    /// How the sink shares out its part: its loads grow by default.
    #[serde(default = "load_scaling")]
    pub sink_scaling: Scaling,
    /// The step the transfer grows by while nothing new is found, MW. 5%
    /// of what the sink starts with when absent.
    #[serde(default)]
    pub major_step_mw: Option<f64>,
    /// The step that pins down where a problem starts, MW. A tenth of the
    /// major step when absent.
    #[serde(default)]
    pub minor_step_mw: Option<f64>,
    /// Stop at this transfer, MW.
    #[serde(default)]
    pub max_transfer_mw: Option<f64>,
    /// Stop when the source or sink runs out of room, as transfer limits
    /// count it. On by default.
    #[serde(default = "yes")]
    pub respect_generation_limits: bool,
    /// New branch overloads and interfaces over their limit count as
    /// problems that stop the run (and set the limit).
    #[serde(default = "yes")]
    pub stop_on_thermal: bool,
    /// New bus voltages outside their limits count as problems that stop
    /// the run (and set the limit).
    #[serde(default = "yes")]
    pub stop_on_voltage: bool,
    /// Stop at the first problem. Off: carry on to the nose of the base
    /// case or the largest transfer, logging each problem met.
    #[serde(default = "yes")]
    pub stop_at_violation: bool,
    /// The contingencies, monitors and limits, as for a contingency run.
    /// Every contingency is solved in AC at every step; `method` and
    /// `ac_count` are unused, and problems already there before the
    /// transfer starts don't count unless they get worse by the
    /// `worse_by` margins.
    #[serde(default)]
    pub contingencies: ContingencySettings,
    /// Buses whose voltage is recorded at every step. The voltage
    /// monitors' buses when the monitors name some, else the sink's
    /// buses.
    #[serde(default)]
    pub voltage_buses: Option<Subsystem>,
    /// More quantities recorded at every step.
    #[serde(default)]
    pub channels: Vec<Channel>,
    /// Where the results file goes. A new file in the temporary folder
    /// when absent.
    #[serde(default)]
    pub output: Option<String>,
    /// The most transfer steps solved.
    #[serde(default = "default_max_points")]
    pub max_points: usize,
}

impl Default for SecureTransferSettings {
    fn default() -> Self {
        Self {
            source: Subsystem::default(),
            source_scaling: Scaling::Generation,
            sink: Subsystem::default(),
            sink_scaling: Scaling::Load,
            major_step_mw: None,
            minor_step_mw: None,
            max_transfer_mw: None,
            respect_generation_limits: true,
            stop_on_thermal: true,
            stop_on_voltage: true,
            stop_at_violation: true,
            contingencies: ContingencySettings::default(),
            voltage_buses: None,
            channels: Vec::new(),
            output: None,
            max_points: default_max_points(),
        }
    }
}

/// One channel as recorded.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ChannelInfo {
    pub quantity: Quantity,
    pub element: String,
    /// The element named PSS/E style.
    pub name: String,
    /// "pu", "deg", "MW", "Mvar" or "%".
    pub unit: &'static str,
    /// For a bus voltage: its low and high limits, p.u.
    pub low: Option<f64>,
    pub high: Option<f64>,
    /// Its column in the results file.
    pub column: String,
}

/// One transfer step solved, the base case and every contingency.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct SecurePoint {
    pub transfer_mw: f64,
    /// Taken with the minor step, pinning down a problem.
    pub minor: bool,
    /// The lowest recorded bus voltage in the base case, p.u.
    pub base_min_vm_pu: Option<f64>,
    /// Contingencies solved here, and how many didn't converge.
    pub solved: usize,
    pub not_converged: usize,
    /// Contingencies (the base case counted as one) with a branch,
    /// voltage or interface problem here, new or worse since the start.
    pub with_thermal: usize,
    pub with_voltage: usize,
    /// Problems that count towards stopping, first seen at this step.
    pub new_problems: usize,
}

/// A problem, the first time it was seen.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct SecureViolation {
    /// "branch", "voltage", "interface", or "not_converged" for a
    /// contingency (or the base case) whose solve failed.
    #[schemars(extend("enum" = ["branch", "voltage", "interface", "not_converged"]))]
    pub kind: &'static str,
    /// The contingency's name; "BASE CASE" for the case with nothing out.
    pub contingency: String,
    /// Its index: 0 for the base case, then the contingencies in order.
    pub contingency_index: usize,
    /// The element over its limit, and its id; empty for not_converged.
    pub element: String,
    pub element_id: String,
    /// The first transfer at which it was seen, MW, and the last clean
    /// one before it.
    pub transfer_mw: f64,
    pub clean_mw: f64,
    /// Loading (percent) or voltage (p.u.) when first seen, and its limit.
    pub value: Option<f64>,
    pub limit: Option<f64>,
    /// Counts towards stopping the run, as the settings choose.
    pub stops: bool,
}

/// What each contingency allows.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct SecureContingency {
    /// 0 for the base case, then the contingencies in order.
    pub index: usize,
    pub name: String,
    /// What goes out; empty for the base case.
    pub element: String,
    /// The last clean transfer before its first problem that stops the
    /// run, MW; `None` when it had none.
    pub limit_mw: Option<f64>,
    /// That problem's kind (as [`SecureViolation::kind`]) and element.
    pub limited_by: Option<&'static str>,
    pub limiting_element: Option<String>,
    /// Already failing to solve with no transfer: left out of the study.
    pub failed_at_start: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct SecureSummary {
    /// The secure transfer limit: the last clean transfer before the first
    /// problem that stops the run, MW; or, when none was found, the
    /// largest transfer studied.
    pub limit_mw: f64,
    /// What set the limit.
    #[schemars(extend("enum" = ["thermal", "voltage", "not_converged", "base_collapse", "generation", "max_transfer", "max_points", "cancelled"]))]
    pub limited_by: &'static str,
    /// The contingency and element behind the limit, when one set it.
    pub limiting_contingency: Option<String>,
    pub limiting_element: Option<String>,
    /// The limit lies between `limit_mw` and `limit_mw` plus this, MW.
    pub resolution_mw: f64,
    /// The largest transfer studied, MW, and why the run stopped there.
    pub last_transfer_mw: f64,
    #[schemars(extend("enum" = ["violation", "base_collapse", "generation", "max_transfer", "max_points", "cancelled"]))]
    pub stopped_by: &'static str,
    /// The base case's nose, MW, when the run reached it.
    pub nose_mw: Option<f64>,
    pub source: Participation,
    pub sink: Participation,
    pub source_scaling: Scaling,
    pub sink_scaling: Scaling,
    pub major_step_mw: f64,
    pub minor_step_mw: f64,
    /// Contingencies studied (the base case not counted), elements already
    /// out of service, and contingencies failing with no transfer.
    pub contingencies: usize,
    pub already_out: usize,
    pub failed_at_start: usize,
    /// Problems already there with no transfer, left out unless they got
    /// worse.
    pub existing_problems: usize,
    /// Base case solves, and contingency solves.
    pub base_solves: usize,
    pub contingency_solves: usize,
    pub channels: usize,
    /// The results file, its size and rows.
    pub results_file: String,
    pub results_bytes: u64,
    pub results_rows: usize,
    pub elapsed_ms: f64,
    pub cancelled: bool,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct SecureResults {
    pub summary: SecureSummary,
    /// Every step solved, in order.
    pub points: Vec<SecurePoint>,
    /// Every problem, the first time it was seen, in transfer order.
    pub violations: Vec<SecureViolation>,
    /// The base case and each contingency, the most limiting first.
    pub contingencies: Vec<SecureContingency>,
    pub channels: Vec<ChannelInfo>,
}

/// Progress of a secure transfer study.
#[derive(Clone, Debug, Default, Serialize, schemars::JsonSchema)]
pub struct SecureProgress {
    /// "idle", or "contingencies" while a step's contingencies solve.
    pub phase: &'static str,
    /// Contingencies solved at this step, of how many.
    pub done: usize,
    pub total: usize,
    /// Steps solved so far, and the transfer of the step in hand, MW.
    pub steps: usize,
    pub transfer_mw: f64,
}

/// Cancels a secure transfer study and follows it from another thread.
#[derive(Clone, Default)]
pub struct SecureHandle {
    run: RunHandle,
    steps: Arc<AtomicU64>,
    transfer: Arc<AtomicU64>,
}

impl SecureHandle {
    pub fn cancel(&self) {
        self.run.cancel();
    }

    pub fn progress(&self) -> SecureProgress {
        let p = self.run.progress();
        SecureProgress {
            phase: if p.phase == "idle" {
                "idle"
            } else {
                "contingencies"
            },
            done: p.done,
            total: p.total,
            steps: self.steps.load(Ordering::SeqCst) as usize,
            transfer_mw: f64::from_bits(self.transfer.load(Ordering::SeqCst)),
        }
    }
}

/// A channel resolved to rows of the case.
#[derive(Clone, Copy)]
enum Resolved {
    Bus(usize),
    Branch(usize),
    Generator(usize),
    Interface(usize),
}

/// Everything a secure transfer study needs, copied out of the workspace.
pub struct SecureTransferJob {
    power: PowerCase,
    base: PowerFlowResult,
    solve: SolveSettings,
    settings: SecureTransferSettings,
    checker: StepChecker,
    channels: Vec<(ChannelInfo, Quantity, Resolved)>,
    notes: Vec<String>,
    handle: SecureHandle,
}

/// A problem to log: kind, element name, element id, how far past the
/// limit, value, limit.
type Logged = (&'static str, String, String, f64, Option<f64>, Option<f64>);

/// A problem's identity: contingency, kind, element.
type Key = (usize, &'static str, String);

/// One contingency's outcome at one step, as kept.
struct Found {
    converged: bool,
    /// Problems: kind, element name, element id, how far past the limit
    /// (percentage points, or p.u. outside), value, limit.
    problems: Vec<(&'static str, String, String, f64, f64, f64)>,
    values: Vec<f32>,
    overloads: u32,
    voltage: u32,
    interfaces: u32,
}

/// A step's outcome, before it is accepted.
struct Step {
    transfer: f64,
    found: Vec<Option<Found>>,
}

fn next_file_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl SecureTransferJob {
    pub fn new(
        power: &PowerCase,
        base: &PowerFlowResult,
        solve: &SolveSettings,
        settings: SecureTransferSettings,
    ) -> Result<Self, String> {
        if settings.sink.is_everything() && settings.source.is_everything() {
            return Err("Pick a sink for the transfer (and a source, or every bus).".into());
        }
        for (name, value) in [
            ("major step", settings.major_step_mw),
            ("minor step", settings.minor_step_mw),
            ("largest transfer", settings.max_transfer_mw),
        ] {
            if value.is_some_and(|v| !(v.is_finite() && v > 0.0)) {
                return Err(format!("The {name} must be more than 0 MW."));
            }
        }
        if let (Some(major), Some(minor)) = (settings.major_step_mw, settings.minor_step_mw)
            && minor > major
        {
            return Err("The minor step can't be larger than the major step.".into());
        }
        let mut contingencies = settings.contingencies.clone();
        // Every problem over its limit is looked at; the ones there before
        // the transfer are told apart from the start of the run instead.
        contingencies.report_existing = true;
        let checker = StepChecker::new(power, contingencies)?;
        let mut notes = checker.notes.clone();
        let case = &power.case;

        // Channels: recorded bus voltages, then the ones asked for.
        let names = power::bus_names(case);
        let row_of: HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let energized =
            |r: usize| case.buses[r].kind != BusType::Isolated && base.case.buses[r].vm > 0.0;
        let voltage_rows: Vec<usize> = match (&settings.voltage_buses, checker.watched_buses()) {
            (Some(sub), _) => (0..case.buses.len())
                .filter(|&r| sub.contains(power, r) && energized(r))
                .collect(),
            (None, Some(rows)) if !rows.is_empty() => {
                rows.into_iter().filter(|&r| energized(r)).collect()
            }
            _ => {
                let sink = settings.sink.bus_ids(power);
                (0..case.buses.len())
                    .filter(|&r| sink.contains(&case.buses[r].id) && energized(r))
                    .collect()
            }
        };
        let mut channels: Vec<(ChannelInfo, Quantity, Resolved)> = Vec::new();
        let mut seen: HashSet<(Quantity, String)> = HashSet::new();
        let bus_info = |r: usize, quantity: Quantity| {
            let b = &case.buses[r];
            let (low, high) = match quantity {
                Quantity::Vm => (
                    Some(if b.vmin > 0.0 && b.vmin < 1.0 {
                        b.vmin
                    } else {
                        0.9
                    }),
                    Some(if b.vmax > 1.0 { b.vmax } else { 1.1 }),
                ),
                _ => (None, None),
            };
            ChannelInfo {
                quantity,
                element: crate::elements::bus(b.id),
                name: if names[r].is_empty() {
                    b.id.to_string()
                } else {
                    format!("{} {}", names[r], b.id)
                },
                unit: if quantity == Quantity::Vm {
                    "pu"
                } else {
                    "deg"
                },
                low,
                high,
                column: String::new(),
            }
        };
        for &r in &voltage_rows {
            let info = bus_info(r, Quantity::Vm);
            seen.insert((Quantity::Vm, info.element.clone()));
            channels.push((info, Quantity::Vm, Resolved::Bus(r)));
        }
        let interfaces = checker.interface_names();
        let element_name = |id: &str| crate::contingency::element_name(id, &names, &row_of);
        for c in &settings.channels {
            if !seen.insert((c.quantity, c.element.clone())) {
                continue;
            }
            let unknown = || {
                format!(
                    "Channel {:?} of {}: no such element in this case.",
                    c.quantity, c.element
                )
            };
            let (info, resolved) = match c.quantity {
                Quantity::Vm | Quantity::Va => {
                    let r = c
                        .element
                        .strip_prefix("bus:")
                        .and_then(|n| n.parse::<u32>().ok())
                        .and_then(|n| row_of.get(&n).copied())
                        .ok_or_else(unknown)?;
                    (bus_info(r, c.quantity), Resolved::Bus(r))
                }
                Quantity::Mw | Quantity::Mvar | Quantity::Pct => {
                    let r = power
                        .elements
                        .branches
                        .iter()
                        .position(|b| b.element == c.element)
                        .ok_or_else(unknown)?;
                    let unit = match c.quantity {
                        Quantity::Mw => "MW",
                        Quantity::Mvar => "Mvar",
                        _ => "%",
                    };
                    let limit =
                        (c.quantity == Quantity::Pct).then_some(checker.settings().limit_pct);
                    (
                        ChannelInfo {
                            quantity: c.quantity,
                            element: c.element.clone(),
                            name: element_name(&c.element),
                            unit,
                            low: None,
                            high: limit,
                            column: String::new(),
                        },
                        Resolved::Branch(r),
                    )
                }
                Quantity::Pg | Quantity::Qg => {
                    let r = power
                        .elements
                        .generators
                        .iter()
                        .position(|g| g.element == c.element)
                        .ok_or_else(unknown)?;
                    (
                        ChannelInfo {
                            quantity: c.quantity,
                            element: c.element.clone(),
                            name: element_name(&c.element),
                            unit: if c.quantity == Quantity::Pg {
                                "MW"
                            } else {
                                "Mvar"
                            },
                            low: None,
                            high: None,
                            column: String::new(),
                        },
                        Resolved::Generator(r),
                    )
                }
                Quantity::Interface => {
                    let k = interfaces.iter().position(|n| *n == c.element).ok_or_else(|| {
                        format!("Channel interface {}: the monitors have no interface by that name.", c.element)
                    })?;
                    (
                        ChannelInfo {
                            quantity: c.quantity,
                            element: c.element.clone(),
                            name: c.element.clone(),
                            unit: "MW",
                            low: None,
                            high: None,
                            column: String::new(),
                        },
                        Resolved::Interface(k),
                    )
                }
            };
            channels.push((info, c.quantity, resolved));
        }
        for (k, (info, quantity, _)) in channels.iter_mut().enumerate() {
            let q = serde_json::to_value(quantity)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            info.column = format!("c{k} {q} {}", info.element);
        }
        if channels.is_empty() {
            notes.push(
                "No channels are recorded: the sink has no energized bus and none were asked for."
                    .into(),
            );
        }
        Ok(Self {
            power: power.clone(),
            base: base.clone(),
            solve: solve.clone(),
            settings,
            checker,
            channels,
            notes,
            handle: SecureHandle::default(),
        })
    }

    pub fn handle(&self) -> SecureHandle {
        self.handle.clone()
    }

    /// Contingencies studied, the base case not counted.
    pub fn len(&self) -> usize {
        self.checker.outages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checker.outages.is_empty()
    }

    /// The channel values of a solved case.
    fn values(
        &self,
        r: &PowerFlowResult,
        interfaces: &[crate::contingency::InterfaceFlow],
    ) -> Vec<f32> {
        let c = &r.case;
        let rating = self.checker.settings().rating;
        self.channels
            .iter()
            .map(|(_, q, at)| {
                let v = match *at {
                    Resolved::Bus(row) => {
                        let b = &c.buses[row];
                        if b.kind == BusType::Isolated || b.vm <= 0.0 {
                            f64::NAN
                        } else if *q == Quantity::Vm {
                            b.vm
                        } else {
                            b.va
                        }
                    }
                    Resolved::Branch(row) => {
                        let b = &c.branches[row];
                        let f = r.branch_flows[row];
                        if !b.online {
                            0.0
                        } else {
                            match q {
                                Quantity::Mw => f[0],
                                Quantity::Mvar => f[1],
                                _ => {
                                    let rate = rating.of(b);
                                    if rate > 0.0 {
                                        let winding = self
                                            .power
                                            .elements
                                            .branches
                                            .get(row)
                                            .map_or(0, |e| e.winding);
                                        power::loading_pct(f[0], f[1], f[2], f[3], rate, winding)
                                    } else {
                                        f64::NAN
                                    }
                                }
                            }
                        }
                    }
                    Resolved::Generator(row) => {
                        let g = &c.generators[row];
                        if !g.online {
                            0.0
                        } else if *q == Quantity::Pg {
                            g.pg
                        } else {
                            g.qg
                        }
                    }
                    Resolved::Interface(k) => interfaces.get(k).map_or(f64::NAN, |i| i.mw),
                };
                v as f32
            })
            .collect()
    }

    /// What one contingency check found.
    fn found(&self, check: StepCheck<'_>) -> Found {
        let converged = check.row.converged != Some(false) && check.solution.is_some();
        let mut problems = Vec::new();
        if converged {
            for o in &check.violations.overloads {
                problems.push((
                    "branch",
                    o.name.clone(),
                    o.element.clone(),
                    o.pct - o.limit_pct,
                    o.pct,
                    o.limit_pct,
                ));
            }
            for v in &check.violations.voltage_problems {
                let (value, limit) = (v.vm, if v.vm < v.vmin { v.vmin } else { v.vmax });
                let name = if v.name.trim().is_empty() {
                    v.bus.to_string()
                } else {
                    format!("{} {}", v.name.trim(), v.bus)
                };
                problems.push((
                    "voltage",
                    name,
                    format!("bus:{}", v.bus),
                    v.outside,
                    value,
                    limit,
                ));
            }
            for i in &check.violations.interfaces {
                let pct = i.pct.unwrap_or(0.0);
                problems.push((
                    "interface",
                    i.name.clone(),
                    format!("interface:{}", i.name),
                    pct - i.limit_pct,
                    pct,
                    i.limit_pct,
                ));
            }
        }
        Found {
            converged,
            values: match check.solution {
                Some(r) if converged => self.values(r, check.interfaces),
                _ => vec![f32::NAN; self.channels.len()],
            },
            overloads: check.violations.overloads.len() as u32,
            voltage: check.violations.voltage_problems.len() as u32,
            interfaces: check.violations.interfaces.len() as u32,
            problems,
        }
    }

    /// The case moved by `mw` of transfer, not yet solved.
    fn moved(&self, source: &Side, sink: &Side, mw: f64) -> Case {
        let case = &self.power.case;
        let mut moved = case.clone();
        source.apply(&mut moved, case, mw);
        sink.apply(&mut moved, case, mw);
        moved
    }

    /// Solve the base case at `mw` from `last`; `Err(None)` when cancelled.
    fn solve_base(
        &self,
        source: &Side,
        sink: &Side,
        mw: f64,
        last: &PowerFlowResult,
        tolerance: f64,
    ) -> Result<PowerFlowResult, Option<String>> {
        let moved = self.moved(source, sink, mw);
        let mut options = self.solve.options_for(&self.power);
        options.cancel = Some(self.handle.run.cancel_flag());
        options.tolerance = tolerance;
        let warm = power::warm_start(&moved, last, &mut options);
        options.method = rs_power::AcMethod::Newton;
        options.max_iterations = options.max_iterations.max(MIN_ITERATIONS);
        options.initialization = rs_power::Initialization::Case;
        solve_step(&warm, &options)
    }

    pub fn run(self) -> Result<SecureResults, String> {
        let start = Instant::now();
        let s = &self.settings;
        let case = &self.power.case;
        let solved = &self.base.case;
        let mut notes = self.notes.clone();

        let sink_buses = s.sink.bus_ids(&self.power);
        let source_buses = s.source.bus_ids(&self.power);
        let (source, source_part, source_total) =
            Side::new(case, solved, &source_buses, s.source_scaling, true);
        let (sink, sink_part, sink_total) =
            Side::new(case, solved, &sink_buses, s.sink_scaling, false);
        if source_total <= 0.0 {
            return Err("The source has nothing to shift.".into());
        }
        if sink_total <= 0.0 {
            return Err("The sink has nothing to shift.".into());
        }
        let major = s
            .major_step_mw
            .unwrap_or_else(|| (sink_total * 0.05).max(1.0));
        let minor = s.minor_step_mw.unwrap_or(major / 10.0).min(major);
        let generation_cap = if s.respect_generation_limits {
            match (source_part.available_mw, sink_part.available_mw) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        } else {
            None
        };
        let cap = match (generation_cap, s.max_transfer_mw) {
            (Some(g), Some(m)) => Some(g.min(m)),
            (g, m) => g.or(m),
        };
        if cap.is_some_and(|c| c <= 0.0) {
            return Err(
                "The source or sink has no room to move: its generators are at their limits."
                    .into(),
            );
        }
        let tolerance = self
            .solve
            .options(case.base_mva)
            .tolerance
            .min(power::tolerance_pu(
                minor * source.largest_share(case).max(sink.largest_share(case)) / 20.0,
                case.base_mva,
            ));

        // The base case, then each contingency.
        let base_outage = Outage::base();
        let mut studied: Vec<&Outage> = vec![&base_outage];
        studied.extend(self.checker.outages.iter());
        let n = studied.len();
        let names: Vec<String> = {
            let bus_names = power::bus_names(case);
            let row_of: HashMap<u32, usize> = case
                .buses
                .iter()
                .enumerate()
                .map(|(i, b)| (b.id, i))
                .collect();
            studied
                .iter()
                .map(|o| {
                    if o.kind == "base" {
                        "BASE CASE".to_string()
                    } else {
                        o.name.clone().unwrap_or_else(|| {
                            crate::contingency::element_name(&o.element, &bus_names, &row_of)
                        })
                    }
                })
                .collect()
        };

        let path = match &s.output {
            Some(p) => std::path::PathBuf::from(p),
            None => std::env::temp_dir().join(format!(
                "stannum-secure-{}-{}.stnr",
                std::process::id(),
                next_file_id()
            )),
        };
        let file = std::fs::File::create(&path)
            .map_err(|e| format!("Can't write the results file {}: {e}", path.display()))?;
        let mut writer = Writer::new(file, &self.channels, &names, s, &path)?;

        let mut dead = vec![false; n];
        let mut points: Vec<SecurePoint> = Vec::new();
        let mut violations: Vec<SecureViolation> = Vec::new();
        let mut first: Vec<Option<usize>> = vec![None; n];
        let mut existing: HashMap<Key, f64> = HashMap::new();
        let mut seen: HashSet<Key> = HashSet::new();
        let mut base_solves = 0;
        let mut contingency_solves = 0;
        let worse_pct = self.checker.settings().worse_by_pct.max(0.0);
        let worse_pu = self.checker.settings().worse_by_pu.max(0.0);
        let stops = |kind: &str| match kind {
            "branch" | "interface" => s.stop_on_thermal,
            "voltage" => s.stop_on_voltage,
            _ => true,
        };

        // Solve every live contingency at a step of `base`.
        let check = |transfer: f64,
                     base: &PowerFlowResult,
                     dead: &[bool]|
         -> Result<Option<Vec<Option<Found>>>, String> {
            let live: Vec<usize> = (0..n).filter(|&i| !dead[i]).collect();
            let list: Vec<&Outage> = live.iter().map(|&i| studied[i]).collect();
            // The case as set up, moved by the transfer; the step's
            // solution gives each contingency its starting point.
            let mut moved = self.power.clone();
            moved.case = self.moved(&source, &sink, transfer);
            self.handle
                .transfer
                .store(transfer.to_bits(), Ordering::SeqCst);
            self.handle.run.set_phase(3, list.len());
            let got = self
                .checker
                .check(&moved, base, &list, &self.handle.run, |_, c| self.found(c))?;
            self.handle.run.set_phase(0, 0);
            if got.iter().any(Option::is_none) {
                return Ok(None);
            }
            let mut out: Vec<Option<Found>> = (0..n).map(|_| None).collect();
            for (k, f) in live.into_iter().zip(got) {
                out[k] = f;
            }
            Ok(Some(out))
        };

        // Accept a step: record it, log its problems, and say whether it
        // has a new one that stops the run.
        let accept = |step: Step,
                      minor_step: bool,
                      clean: f64,
                      dead: &mut Vec<bool>,
                      points: &mut Vec<SecurePoint>,
                      violations: &mut Vec<SecureViolation>,
                      first: &mut Vec<Option<usize>>,
                      existing: &mut HashMap<Key, f64>,
                      seen: &mut HashSet<Key>,
                      writer: &mut Writer|
         -> Result<usize, String> {
            let at_start = points.is_empty();
            let mut point = SecurePoint {
                transfer_mw: step.transfer,
                minor: minor_step,
                base_min_vm_pu: None,
                solved: 0,
                not_converged: 0,
                with_thermal: 0,
                with_voltage: 0,
                new_problems: 0,
            };
            let mut base_min = f64::INFINITY;
            for (k, (info, _, _)) in self.channels.iter().enumerate() {
                if info.quantity == Quantity::Vm
                    && let Some(f) = step.found[0].as_ref()
                {
                    let v = f.values[k] as f64;
                    if v.is_finite() {
                        base_min = base_min.min(v);
                    }
                }
            }
            point.base_min_vm_pu = base_min.is_finite().then_some(base_min);
            for (i, f) in step.found.iter().enumerate() {
                let Some(f) = f else { continue };
                point.solved += 1;
                writer.row(points.len(), step.transfer, i, f)?;
                let mut thermal = false;
                let mut voltage = false;
                let mut found_here: Vec<Logged> = Vec::new();
                if !f.converged {
                    point.not_converged += 1;
                    if at_start {
                        dead[i] = true;
                        continue;
                    }
                    if !s.stop_at_violation || i != 0 {
                        dead[i] = !s.stop_at_violation;
                    }
                    found_here.push((
                        "not_converged",
                        String::new(),
                        String::new(),
                        f64::INFINITY,
                        None,
                        None,
                    ));
                }
                for (kind, name, id, past, value, limit) in &f.problems {
                    let key: Key = (i, kind, id.clone());
                    if at_start {
                        existing.insert(key, *past);
                        continue;
                    }
                    let margin = if *kind == "voltage" {
                        worse_pu
                    } else {
                        worse_pct
                    };
                    if existing.get(&key).is_some_and(|&was| *past <= was + margin) {
                        continue;
                    }
                    if *kind == "voltage" {
                        voltage = true;
                    } else {
                        thermal = true;
                    }
                    found_here.push((
                        kind,
                        name.clone(),
                        id.clone(),
                        *past,
                        Some(*value),
                        Some(*limit),
                    ));
                }
                point.with_thermal += thermal as usize;
                point.with_voltage += voltage as usize;
                for (kind, name, id, _, value, limit) in found_here {
                    let key: Key = (i, kind, id.clone());
                    if !seen.insert(key) {
                        continue;
                    }
                    let stop = stops(kind);
                    if stop {
                        point.new_problems += 1;
                        if first[i].is_none() {
                            first[i] = Some(violations.len());
                        }
                    }
                    violations.push(SecureViolation {
                        kind,
                        contingency: names[i].clone(),
                        contingency_index: i,
                        element: name,
                        element_id: id,
                        transfer_mw: step.transfer,
                        clean_mw: clean,
                        value,
                        limit,
                        stops: stop,
                    });
                }
            }
            let new = point.new_problems;
            points.push(point);
            self.handle
                .steps
                .store(points.len() as u64, Ordering::SeqCst);
            Ok(new)
        };

        // The start: no transfer.
        base_solves += 1;
        let start_found = match check(0.0, &self.base, &dead)? {
            Some(f) => f,
            None => return Err("Cancelled before the first step.".into()),
        };
        contingency_solves += n;
        accept(
            Step {
                transfer: 0.0,
                found: start_found,
            },
            false,
            0.0,
            &mut dead,
            &mut points,
            &mut violations,
            &mut first,
            &mut existing,
            &mut seen,
            &mut writer,
        )?;
        let failed_at_start: Vec<bool> = dead.clone();
        if failed_at_start[0] {
            return Err("The base case doesn't solve again with no transfer.".into());
        }
        let n_failed_start = failed_at_start.iter().filter(|&&d| d).count();
        if n_failed_start > 0 {
            notes.push(format!(
                "{n_failed_start} contingenc{} already fail{} to solve with no transfer; left out.",
                if n_failed_start == 1 { "y" } else { "ies" },
                if n_failed_start == 1 { "s" } else { "" },
            ));
        }
        let existing_problems = existing.len();

        let mut transfer = 0.0;
        let mut last = self.base.clone();
        let mut limit: Option<(f64, f64, Option<usize>)> = None; // (clean, resolution, violation)
        let mut nose = None;
        let mut step = major;
        let mut pinning = false;
        // The major step that found the problem being pinned down: past
        // it with nothing found, the walk goes back to major steps.
        let mut pin_until = f64::INFINITY;
        let stopped_by = loop {
            if self.handle.run.cancelled() {
                break "cancelled";
            }
            if points.len() >= s.max_points {
                break "max_points";
            }
            if cap.is_some_and(|c| transfer >= c - 1e-9) {
                break if generation_cap.is_some_and(|g| transfer >= g - 1e-9) {
                    "generation"
                } else {
                    "max_transfer"
                };
            }
            let next = match cap {
                Some(c) => (transfer + step).min(c),
                None => transfer + step,
            };
            base_solves += 1;
            let base = match self.solve_base(&source, &sink, next, &last, tolerance) {
                Ok(r) => r,
                Err(None) => break "cancelled",
                Err(Some(_)) => {
                    if next - transfer > minor * (1.0 + 1e-9) {
                        // Pin the nose down with the minor step.
                        step = minor;
                        pinning = true;
                        pin_until = next;
                        continue;
                    }
                    nose = Some(transfer);
                    if limit.is_none() {
                        limit = Some((transfer, next - transfer, None));
                    }
                    break "base_collapse";
                }
            };
            let Some(found) = check(next, &base, &dead)? else {
                break "cancelled";
            };
            contingency_solves += found.iter().filter(|f| f.is_some()).count();
            let problem = found.iter().enumerate().any(|(i, f)| {
                f.as_ref().is_some_and(|f| {
                    (!f.converged)
                        || f.problems.iter().any(|(kind, _, id, past, _, _)| {
                            let key: Key = (i, kind, id.clone());
                            let margin = if *kind == "voltage" {
                                worse_pu
                            } else {
                                worse_pct
                            };
                            stops(kind)
                                && !seen.contains(&key)
                                && existing.get(&key).is_none_or(|&was| *past > was + margin)
                        })
                })
            });
            if problem && limit.is_none() && !pinning && next - transfer > minor * (1.0 + 1e-9) {
                // Walk forward from the last clean step with the minor step.
                step = minor;
                pinning = true;
                pin_until = next;
                continue;
            }
            let new = accept(
                Step {
                    transfer: next,
                    found,
                },
                pinning,
                transfer,
                &mut dead,
                &mut points,
                &mut violations,
                &mut first,
                &mut existing,
                &mut seen,
                &mut writer,
            )?;
            if new > 0 && limit.is_none() {
                let first_stop = violations
                    .iter()
                    .position(|v| v.stops && v.transfer_mw == next);
                limit = Some((transfer, next - transfer, first_stop));
            }
            transfer = next;
            last = base;
            if new > 0 && s.stop_at_violation {
                break "violation";
            }
            if pinning && (new > 0 || transfer >= pin_until - 1e-9) {
                // Pinned, or nothing found where the major step found it:
                // back to the major step.
                pinning = false;
                step = major;
                pin_until = f64::INFINITY;
            }
            if dead[0] {
                break "base_collapse";
            }
        };
        self.handle.run.set_phase(0, 0);
        let cancelled = stopped_by == "cancelled";
        let (bytes, rows) = writer.finish()?;

        let (limit_mw, resolution, limiting) = match limit {
            Some((t, r, v)) => (t, r, v),
            None => (transfer, 0.0, None),
        };
        let limited_by: &'static str = match (limit, limiting) {
            (Some(_), Some(v)) => match violations[v].kind {
                "branch" | "interface" => "thermal",
                "voltage" => "voltage",
                _ => {
                    if violations[v].contingency_index == 0 {
                        "base_collapse"
                    } else {
                        "not_converged"
                    }
                }
            },
            (Some(_), None) => "base_collapse",
            (None, _) => match stopped_by {
                "violation" => "thermal",
                other => other,
            },
        };

        let mut contingencies: Vec<SecureContingency> = (0..n)
            .map(|i| {
                let v = first[i].map(|k| &violations[k]);
                SecureContingency {
                    index: i,
                    name: names[i].clone(),
                    element: studied[i].element.clone(),
                    limit_mw: v.map(|v| v.clean_mw),
                    limited_by: v.map(|v| v.kind),
                    limiting_element: v.map(|v| v.element.clone()).filter(|e| !e.is_empty()),
                    failed_at_start: failed_at_start[i],
                }
            })
            .collect();
        contingencies.sort_by(|a, b| {
            let key = |c: &SecureContingency| c.limit_mw.unwrap_or(f64::INFINITY);
            key(a)
                .total_cmp(&key(b))
                .then(a.failed_at_start.cmp(&b.failed_at_start))
                .then(a.index.cmp(&b.index))
        });

        let summary = SecureSummary {
            limit_mw,
            limited_by,
            limiting_contingency: limiting.map(|v| violations[v].contingency.clone()),
            limiting_element: limiting
                .map(|v| violations[v].element.clone())
                .filter(|e| !e.is_empty()),
            resolution_mw: resolution,
            last_transfer_mw: transfer,
            stopped_by,
            nose_mw: nose,
            source: source_part,
            sink: sink_part,
            source_scaling: s.source_scaling,
            sink_scaling: s.sink_scaling,
            major_step_mw: major,
            minor_step_mw: minor,
            contingencies: n - 1,
            already_out: self.checker.already_out,
            failed_at_start: n_failed_start,
            existing_problems,
            base_solves,
            contingency_solves,
            channels: self.channels.len(),
            results_file: path.display().to_string(),
            results_bytes: bytes,
            results_rows: rows,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            cancelled,
            notes,
        };
        Ok(SecureResults {
            summary,
            points,
            violations,
            contingencies,
            channels: self.channels.into_iter().map(|c| c.0).collect(),
        })
    }
}

type Dict = arrow_array::builder::StringDictionaryBuilder<Int32Type>;

/// The results file being written, a batch at a time.
struct Writer {
    writer: StreamWriter<Counted<std::io::BufWriter<std::fs::File>>>,
    schema: Arc<Schema>,
    names: Vec<String>,
    point: UInt32Builder,
    transfer: Float64Builder,
    index: UInt32Builder,
    contingency: Dict,
    converged: BooleanBuilder,
    overloads: UInt32Builder,
    voltage: UInt32Builder,
    interfaces: UInt32Builder,
    values: Vec<Float32Builder>,
    len: usize,
    rows: usize,
}

/// Counts what goes through to the file.
struct Counted<W> {
    inner: W,
    bytes: u64,
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn arrow_err(e: arrow_schema::ArrowError) -> String {
    e.to_string()
}

impl Writer {
    fn new(
        file: std::fs::File,
        channels: &[(ChannelInfo, Quantity, Resolved)],
        names: &[String],
        settings: &SecureTransferSettings,
        path: &std::path::Path,
    ) -> Result<Self, String> {
        let dict = || DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8));
        let mut fields = vec![
            Field::new("point", DataType::UInt32, false),
            Field::new("transfer_mw", DataType::Float64, false),
            Field::new("contingency_index", DataType::UInt32, false),
            Field::new("contingency", dict(), false),
            Field::new("converged", DataType::Boolean, false),
            Field::new("overloads", DataType::UInt32, false),
            Field::new("voltage_problems", DataType::UInt32, false),
            Field::new("interface_problems", DataType::UInt32, false),
        ];
        for (info, _, _) in channels {
            fields.push(Field::new(&info.column, DataType::Float32, true));
        }
        let infos: Vec<&ChannelInfo> = channels.iter().map(|c| &c.0).collect();
        let metadata = HashMap::from([
            ("stannum.format".to_string(), SECURE_FORMAT.to_string()),
            ("stannum.kind".to_string(), "secure_transfer".to_string()),
            (
                "stannum.channels".to_string(),
                serde_json::to_string(&infos).unwrap_or_default(),
            ),
            (
                "stannum.settings".to_string(),
                serde_json::to_string(settings).unwrap_or_default(),
            ),
            (
                "stannum.created_by".to_string(),
                format!("stannum-core {}", env!("CARGO_PKG_VERSION")),
            ),
        ]);
        let schema = Arc::new(Schema::new_with_metadata(fields, metadata));
        let options = IpcWriteOptions::default()
            .try_with_compression(Some(CompressionType::ZSTD))
            .map_err(arrow_err)?;
        let writer = StreamWriter::try_new_with_options(
            Counted {
                inner: std::io::BufWriter::new(file),
                bytes: 0,
            },
            &schema,
            options,
        )
        .map_err(|e| format!("Can't write the results file {}: {e}", path.display()))?;
        Ok(Self {
            writer,
            schema,
            names: names.to_vec(),
            point: UInt32Builder::new(),
            transfer: Float64Builder::new(),
            index: UInt32Builder::new(),
            contingency: Dict::new(),
            converged: BooleanBuilder::new(),
            overloads: UInt32Builder::new(),
            voltage: UInt32Builder::new(),
            interfaces: UInt32Builder::new(),
            values: channels.iter().map(|_| Float32Builder::new()).collect(),
            len: 0,
            rows: 0,
        })
    }

    fn row(&mut self, point: usize, transfer: f64, index: usize, f: &Found) -> Result<(), String> {
        self.point.append_value(point as u32);
        self.transfer.append_value(transfer);
        self.index.append_value(index as u32);
        self.contingency.append_value(&self.names[index]);
        self.converged.append_value(f.converged);
        self.overloads.append_value(f.overloads);
        self.voltage.append_value(f.voltage);
        self.interfaces.append_value(f.interfaces);
        for (b, &v) in self.values.iter_mut().zip(&f.values) {
            if v.is_finite() {
                b.append_value(v);
            } else {
                b.append_null();
            }
        }
        self.len += 1;
        self.rows += 1;
        if self.len >= BATCH_ROWS {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        let mut columns: Vec<ArrayRef> = vec![
            Arc::new(self.point.finish()),
            Arc::new(self.transfer.finish()),
            Arc::new(self.index.finish()),
            Arc::new(self.contingency.finish()),
            Arc::new(self.converged.finish()),
            Arc::new(self.overloads.finish()),
            Arc::new(self.voltage.finish()),
            Arc::new(self.interfaces.finish()),
        ];
        for b in &mut self.values {
            columns.push(Arc::new(b.finish()));
        }
        let batch = RecordBatch::try_new(self.schema.clone(), columns).map_err(arrow_err)?;
        self.writer.write(&batch).map_err(arrow_err)?;
        self.len = 0;
        Ok(())
    }

    /// Bytes and rows written.
    fn finish(mut self) -> Result<(u64, usize), String> {
        if self.len > 0 || self.rows == 0 {
            self.flush()?;
        }
        self.writer.finish().map_err(arrow_err)?;
        let mut inner = self.writer.into_inner().map_err(arrow_err)?;
        inner.flush().map_err(|e| e.to_string())?;
        Ok((inner.bytes, self.rows))
    }
}

/// One channel's curves, read back from a results file.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ChannelCurves {
    pub channel: ChannelInfo,
    /// The transfer at each step, MW.
    pub transfer_mw: Vec<f64>,
    /// The base case first, then the contingencies asked for.
    pub curves: Vec<ChannelCurve>,
}

#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct ChannelCurve {
    pub index: usize,
    pub name: String,
    /// The value at each step; `None` where it didn't solve (or wasn't
    /// solved: a contingency left out after it failed).
    pub values: Vec<Option<f64>>,
}

/// Read channel `column` (its position in the results' `channels`) from a
/// results file: the base case's curve, and each of `contingencies` (by
/// index; every one when absent).
pub fn read_channel(
    path: &str,
    column: usize,
    contingencies: Option<&[usize]>,
) -> Result<ChannelCurves, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Can't open {path}: {e}"))?;
    let reader = arrow_ipc::reader::StreamReader::try_new(std::io::BufReader::new(file), None)
        .map_err(|e| format!("{path} isn't a results file: {e}"))?;
    let schema = reader.schema();
    if schema.metadata().get("stannum.format").map(String::as_str) != Some(SECURE_FORMAT) {
        return Err(format!("{path} isn't a secure transfer results file."));
    }
    let channels: Vec<ChannelInfo> = schema
        .metadata()
        .get("stannum.channels")
        .and_then(|j| serde_json::from_str::<Vec<serde_json::Value>>(j).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| {
            Some(ChannelInfo {
                quantity: serde_json::from_value(v.get("quantity")?.clone()).ok()?,
                element: v.get("element")?.as_str()?.to_string(),
                name: v.get("name")?.as_str()?.to_string(),
                unit: match v.get("unit")?.as_str()? {
                    "pu" => "pu",
                    "deg" => "deg",
                    "MW" => "MW",
                    "Mvar" => "Mvar",
                    _ => "%",
                },
                low: v.get("low").and_then(|x| x.as_f64()),
                high: v.get("high").and_then(|x| x.as_f64()),
                column: v.get("column")?.as_str()?.to_string(),
            })
        })
        .collect();
    let channel = channels.get(column).cloned().ok_or_else(|| {
        format!(
            "The results have {} channels; there is no channel {column}.",
            channels.len()
        )
    })?;
    let at = schema
        .index_of(&channel.column)
        .map_err(|_| format!("The results file has no column {}.", channel.column))?;
    let wanted: Option<HashSet<usize>> =
        contingencies.map(|c| c.iter().copied().chain([0]).collect());
    let mut transfer_mw: Vec<f64> = Vec::new();
    let mut by_index: HashMap<usize, ChannelCurve> = HashMap::new();
    let mut order: Vec<usize> = Vec::new();
    for batch in reader {
        let batch = batch.map_err(arrow_err)?;
        let col = |name: &str| {
            batch
                .column(batch.schema().index_of(name).unwrap_or(0))
                .clone()
        };
        let point = col("point");
        let point = point
            .as_any()
            .downcast_ref::<arrow_array::UInt32Array>()
            .ok_or("bad point column")?;
        let transfer = col("transfer_mw");
        let transfer = transfer
            .as_any()
            .downcast_ref::<arrow_array::Float64Array>()
            .ok_or("bad transfer column")?;
        let index = col("contingency_index");
        let index = index
            .as_any()
            .downcast_ref::<arrow_array::UInt32Array>()
            .ok_or("bad index column")?;
        let names = col("contingency");
        let names = names
            .as_any()
            .downcast_ref::<arrow_array::DictionaryArray<Int32Type>>()
            .ok_or("bad contingency column")?;
        let name_values = names
            .values()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .ok_or("bad contingency names")?;
        let values = batch.column(at);
        let values = values
            .as_any()
            .downcast_ref::<arrow_array::Float32Array>()
            .ok_or("bad channel column")?;
        for r in 0..batch.num_rows() {
            let p = point.value(r) as usize;
            while transfer_mw.len() <= p {
                transfer_mw.push(f64::NAN);
            }
            transfer_mw[p] = transfer.value(r);
            let i = index.value(r) as usize;
            if wanted.as_ref().is_some_and(|w| !w.contains(&i)) {
                continue;
            }
            let curve = by_index.entry(i).or_insert_with(|| {
                order.push(i);
                let key = names.keys().value(r) as usize;
                ChannelCurve {
                    index: i,
                    name: name_values.value(key).to_string(),
                    values: Vec::new(),
                }
            });
            while curve.values.len() < p {
                curve.values.push(None);
            }
            curve
                .values
                .push(values.is_valid(r).then(|| values.value(r) as f64));
        }
    }
    order.sort_unstable();
    let points = transfer_mw.len();
    let curves = order
        .into_iter()
        .filter_map(|i| by_index.remove(&i))
        .map(|mut c| {
            c.values.resize(points, None);
            c
        })
        .collect();
    Ok(ChannelCurves {
        channel,
        transfer_mw,
        curves,
    })
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

    fn settings(dir: &std::path::Path) -> SecureTransferSettings {
        SecureTransferSettings {
            sink: Subsystem {
                buses: vec![5],
                ..Default::default()
            },
            respect_generation_limits: false,
            major_step_mw: Some(20.0),
            minor_step_mw: Some(2.0),
            output: Some(dir.join("run.stnr").display().to_string()),
            ..Default::default()
        }
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("stannum-secure-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn stops_at_the_first_new_problem_with_the_minor_step() {
        let (power, solved, solve) = case9();
        let dir = temp("first");
        let r = SecureTransferJob::new(&power, &solved, &solve, settings(&dir))
            .unwrap()
            .run()
            .unwrap();
        let s = &r.summary;
        assert_eq!(s.stopped_by, "violation", "{s:?}");
        assert_eq!(s.contingencies, 9 + 3);
        assert!(s.limit_mw > 0.0, "{s:?}");
        assert!((s.resolution_mw - 2.0).abs() < 1e-9, "{s:?}");
        assert_eq!(s.last_transfer_mw, s.limit_mw + s.resolution_mw);
        assert!(s.limiting_contingency.is_some());
        // Major steps, then minor ones to the problem.
        let t: Vec<f64> = r.points.iter().map(|p| p.transfer_mw).collect();
        let major = t
            .iter()
            .take_while(|&&x| x <= s.limit_mw - 20.0 + 1e-9)
            .count();
        assert!(
            t[..major]
                .windows(2)
                .all(|w| (w[1] - w[0] - 20.0).abs() < 1e-9),
            "{t:?}"
        );
        assert!(r.points.last().unwrap().minor);
        assert!(r.points.last().unwrap().new_problems > 0);
        // The limiting problem was first seen at the last step.
        let v = r.violations.iter().find(|v| v.stops).unwrap();
        assert_eq!(v.transfer_mw, s.last_transfer_mw);
        assert_eq!(v.clean_mw, s.limit_mw);
        assert_eq!(r.contingencies[0].limit_mw, Some(s.limit_mw));
        // Every step's rows are in the file: base plus every contingency.
        assert_eq!(
            s.results_rows,
            r.points.len() * (s.contingencies + 1 - s.failed_at_start)
        );
        let c = read_channel(&s.results_file, 0, None).unwrap();
        assert_eq!(c.channel.element, "bus:5");
        assert_eq!(c.transfer_mw, t);
        assert_eq!(c.curves[0].name, "BASE CASE");
        assert_eq!(c.curves.len(), s.contingencies + 1 - s.failed_at_start);
        let base = &c.curves[0].values;
        assert!(
            base.windows(2)
                .all(|w| w[1].unwrap() <= w[0].unwrap() + 1e-6),
            "{base:?}"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn finishes_the_transfer_when_asked() {
        let (power, solved, solve) = case9();
        let dir = temp("finish");
        let stop = SecureTransferJob::new(&power, &solved, &solve, settings(&dir))
            .unwrap()
            .run()
            .unwrap();
        let r = SecureTransferJob::new(
            &power,
            &solved,
            &solve,
            SecureTransferSettings {
                stop_at_violation: false,
                ..settings(&dir)
            },
        )
        .unwrap()
        .run()
        .unwrap();
        let s = &r.summary;
        assert_eq!(s.stopped_by, "base_collapse", "{s:?}");
        // The same limit, and the run went on to the nose.
        assert!((s.limit_mw - stop.summary.limit_mw).abs() < 1e-9, "{s:?}");
        assert_eq!(s.nose_mw, Some(s.last_transfer_mw));
        assert!(s.last_transfer_mw > s.limit_mw + 20.0);
        assert!(r.violations.len() > stop.violations.len());
        // Back to major steps after pinning the limit.
        let t: Vec<f64> = r.points.iter().map(|p| p.transfer_mw).collect();
        let after = t
            .iter()
            .position(|&x| x > s.limit_mw + s.resolution_mw + 1e-9)
            .unwrap();
        assert!(!r.points[after].minor, "{t:?}");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn thermal_only_goes_further_than_voltage_and_thermal() {
        let (power, solved, solve) = case9();
        let dir = temp("thermal");
        let both = SecureTransferJob::new(&power, &solved, &solve, settings(&dir))
            .unwrap()
            .run()
            .unwrap();
        let none = SecureTransferJob::new(
            &power,
            &solved,
            &solve,
            SecureTransferSettings {
                stop_on_thermal: false,
                stop_on_voltage: false,
                ..settings(&dir)
            },
        )
        .unwrap()
        .run()
        .unwrap();
        assert!(
            none.summary.limit_mw >= both.summary.limit_mw,
            "{:?}",
            none.summary
        );
        assert!(
            none.violations
                .iter()
                .all(|v| !v.stops || v.kind == "not_converged")
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn records_the_channels_asked_for() {
        let (power, solved, solve) = case9();
        let dir = temp("channels");
        let line = power.elements.branches[1].element.clone();
        let generator = power.elements.generators[1].element.clone();
        let r = SecureTransferJob::new(
            &power,
            &solved,
            &solve,
            SecureTransferSettings {
                channels: vec![
                    Channel {
                        quantity: Quantity::Mw,
                        element: line.clone(),
                    },
                    Channel {
                        quantity: Quantity::Qg,
                        element: generator,
                    },
                    Channel {
                        quantity: Quantity::Vm,
                        element: "bus:5".into(),
                    },
                ],
                contingencies: ContingencySettings {
                    elements: Some(vec![line.clone()]),
                    ..Default::default()
                },
                ..settings(&dir)
            },
        )
        .unwrap()
        .run()
        .unwrap();
        // bus 5's voltage is already there, as a sink bus.
        assert_eq!(r.channels.len(), 3);
        let mw = read_channel(&r.summary.results_file, 1, None).unwrap();
        assert_eq!(mw.channel.unit, "MW");
        assert_eq!(mw.curves.len(), 2);
        // With the line out its flow is zero.
        assert!(
            mw.curves[1]
                .values
                .iter()
                .all(|v| v.is_some_and(|v| v == 0.0))
        );
        assert!(
            mw.curves[0]
                .values
                .iter()
                .all(|v| v.is_some_and(|v| v.abs() > 1.0))
        );
        let only_base = read_channel(&r.summary.results_file, 2, Some(&[])).unwrap();
        assert_eq!(only_base.curves.len(), 1);
        assert!(read_channel(&r.summary.results_file, 3, None).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn refuses_bad_settings() {
        let (power, solved, solve) = case9();
        let dir = temp("bad");
        let bad =
            |s: SecureTransferSettings| SecureTransferJob::new(&power, &solved, &solve, s).is_err();
        assert!(bad(SecureTransferSettings::default()));
        assert!(bad(SecureTransferSettings {
            major_step_mw: Some(0.0),
            ..settings(&dir)
        }));
        assert!(bad(SecureTransferSettings {
            minor_step_mw: Some(30.0),
            ..settings(&dir)
        }));
        assert!(bad(SecureTransferSettings {
            channels: vec![Channel {
                quantity: Quantity::Mw,
                element: "line:1:99:1".into()
            }],
            ..settings(&dir)
        }));
        std::fs::remove_dir_all(dir).ok();
    }

    /// A contingency run on the case moved to the limit finds nothing new
    /// there, and finds the limiting problem one resolution step past it.
    #[test]
    fn the_limit_matches_a_contingency_run_at_that_transfer() {
        let (power, solved, solve) = case9();
        let dir = temp("check");
        let s = settings(&dir);
        let r = SecureTransferJob::new(&power, &solved, &solve, s.clone())
            .unwrap()
            .run()
            .unwrap();
        let limit = r.summary.limit_mw;
        let past = limit + r.summary.resolution_mw;
        let v = r.violations.iter().find(|v| v.stops).unwrap();
        let all = s.source.bus_ids(&power);
        let (source, _, _) = Side::new(&power.case, &solved.case, &all, Scaling::Generation, true);
        let (sink, _, _) = Side::new(
            &power.case,
            &solved.case,
            &HashSet::from([5]),
            Scaling::Load,
            false,
        );
        let run_at = |mw: f64| {
            let mut moved = power.clone();
            source.apply(&mut moved.case, &power.case, mw);
            sink.apply(&mut moved.case, &power.case, mw);
            let base = rs_power::solve(&moved.case, &solve.options_for(&moved)).unwrap();
            let job = crate::contingency::ContingencyJob::new(
                &moved,
                &base,
                ContingencySettings {
                    method: crate::contingency::Method::Ac,
                    report_existing: true,
                    ..Default::default()
                },
            )
            .unwrap();
            job.run().unwrap()
        };
        let has = |res: &crate::contingency::ContingencyResults| {
            res.rows.iter().zip(&res.violations).any(|(row, found)| {
                row.name == v.contingency
                    && found.overloads.iter().any(|o| o.element == v.element_id)
            })
        };
        assert_eq!(v.kind, "branch", "{v:?}");
        assert!(!has(&run_at(limit)), "{v:?}");
        assert!(has(&run_at(past)), "{v:?}");
        std::fs::remove_dir_all(dir).ok();
    }
}
