//! Power flow and sensitivities (rs-power), shaped for the frontend.

use crate::elements::Elements;
use rs_power::{
    BusType, Case, Initialization, InjectionVariable, Model, PowerFlowOptions, PowerFlowResult,
    PowerFlowSensitivity, SolveStatus, StateVariable,
};
use serde::{Deserialize, Serialize};

/// A case loaded for power flow, with any RAW conversion warnings.
#[derive(Clone, Debug)]
pub struct PowerCase {
    pub case: Case,
    pub warnings: Vec<Warning>,
    /// Shared ids for the case's branch and generator rows.
    pub elements: Elements,
    /// What each `case.dcline` row is: "two_terminal", "vsc" or
    /// "multi_terminal". MATPOWER cases only have two-terminal rows.
    pub dc_line_kinds: Vec<&'static str>,
}

fn dc_kind_label(kind: rs_power::DcLineKind) -> &'static str {
    match kind {
        rs_power::DcLineKind::TwoTerminal => "two_terminal",
        rs_power::DcLineKind::VoltageSourceConverter => "vsc",
        rs_power::DcLineKind::MultiTerminal => "multi_terminal",
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Warning {
    pub line: usize,
    pub message: String,
}

/// Parse MATPOWER (`.m`) or PSS/E RAW text.
pub fn load(text: &str, is_matpower: bool) -> Result<PowerCase, String> {
    if is_matpower {
        let case = rs_power::parse_matpower(text).map_err(|e| e.to_string())?;
        Ok(PowerCase {
            elements: Elements::from_matpower(&case),
            dc_line_kinds: vec!["two_terminal"; case.dcline.as_ref().map_or(0, Vec::len)],
            case,
            warnings: Vec::new(),
        })
    } else {
        // VSC and multi-terminal DC lines are imported too, beyond MATPOWER.
        // Line-commutated converters absorb reactive power, unlike MATPOWER's
        // positive estimates.
        let options = rs_power::RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            ..Default::default()
        };
        let import = rs_power::parse_raw_with_options(text, &options).map_err(|e| e.to_string())?;
        Ok(PowerCase {
            elements: Elements::from_raw(
                &import.case,
                &import.branch_sources,
                &import.generator_sources,
                &import.dc_line_names,
            ),
            dc_line_kinds: import
                .dc_line_kinds
                .iter()
                .copied()
                .map(dc_kind_label)
                .collect(),
            case: import.case,
            warnings: import
                .warnings
                .into_iter()
                .map(|w| Warning {
                    line: w.line,
                    message: w.message,
                })
                .collect(),
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelChoice {
    #[default]
    Ac,
    Dc,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum StartChoice {
    #[default]
    Case,
    Flat,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolveSettings {
    pub model: ModelChoice,
    pub start: StartChoice,
    pub tolerance: f64,
    pub max_iterations: usize,
    pub enforce_q_limits: bool,
}

impl Default for SolveSettings {
    fn default() -> Self {
        let d = PowerFlowOptions::default();
        Self {
            model: ModelChoice::Ac,
            start: StartChoice::Case,
            tolerance: d.tolerance,
            max_iterations: d.max_iterations,
            enforce_q_limits: false,
        }
    }
}

impl SolveSettings {
    pub fn options(&self) -> PowerFlowOptions {
        PowerFlowOptions {
            model: match self.model {
                ModelChoice::Ac => Model::Ac,
                ModelChoice::Dc => Model::Dc,
            },
            initialization: match self.start {
                StartChoice::Case => Initialization::Case,
                StartChoice::Flat => Initialization::Flat,
            },
            tolerance: self.tolerance,
            max_iterations: self.max_iterations,
            // Q limits only apply to AC; the solver rejects them for DC.
            enforce_q_limits: self.enforce_q_limits && self.model == ModelChoice::Ac,
            ..PowerFlowOptions::default()
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseCounts {
    pub base_mva: f64,
    pub buses: usize,
    pub generators: usize,
    pub branches: usize,
    pub dc_lines: usize,
}

pub fn counts(case: &Case) -> CaseCounts {
    CaseCounts {
        base_mva: case.base_mva,
        buses: case.buses.len(),
        generators: case.generators.len(),
        branches: case.branches.len(),
        dc_lines: case.dcline.as_ref().map_or(0, Vec::len),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BusRow {
    pub id: u32,
    pub name: String,
    pub kind: &'static str,
    pub base_kv: f64,
    pub vm: f64,
    pub va: f64,
    pub vmin: f64,
    pub vmax: f64,
    pub pd: f64,
    pub qd: f64,
    pub area: u32,
    /// Distance outside [vmin, vmax] in p.u.; zero when within limits.
    pub violation: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GeneratorRow {
    pub index: usize,
    /// Shared id, such as `generator:101:1`.
    pub element: String,
    pub machine_id: String,
    pub bus: u32,
    pub bus_name: String,
    pub online: bool,
    pub pg: f64,
    pub qg: f64,
    pub qmin: f64,
    pub qmax: f64,
    pub vg: f64,
    pub pmax: f64,
    /// MVAr outside [qmin, qmax]; zero when within limits or offline.
    pub q_violation: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BranchRow {
    pub index: usize,
    /// Shared id, such as `line:101:102:1`.
    pub element: String,
    pub circuit: String,
    /// 1, 2 or 3 for a leg of a three-winding transformer; 0 otherwise.
    pub winding: u8,
    pub from: u32,
    pub to: u32,
    pub online: bool,
    pub is_transformer: bool,
    pub pf: f64,
    pub qf: f64,
    pub pt: f64,
    pub qt: f64,
    pub loss_mw: f64,
    pub rate_a: f64,
    /// Highest end MVA as a percent of rate A; `None` when unrated or offline.
    pub loading_pct: Option<f64>,
}

/// A solved DC line row, from its sending (rectifier) bus to its receiving
/// (inverter) bus.
#[derive(Clone, Debug, Serialize)]
pub struct DcLineRow {
    pub index: usize,
    /// Shared id, such as `dcline:101:205:DC1`.
    pub element: String,
    /// "two_terminal", "vsc" or "multi_terminal".
    pub kind: &'static str,
    pub from: u32,
    pub to: u32,
    pub from_name: String,
    pub to_name: String,
    pub online: bool,
    /// MW taken from the AC system at the sending end.
    pub pf: f64,
    /// MW delivered to the AC system at the receiving end.
    pub pt: f64,
    /// MVAr injected into the AC system at each end (negative = absorbed).
    pub qf: f64,
    pub qt: f64,
    pub vf: f64,
    pub vt: f64,
    pub loss_mw: f64,
    pub qminf: f64,
    pub qmaxf: f64,
    pub qmint: f64,
    pub qmaxt: f64,
    /// Largest MVAr outside either end's range; zero when within or offline.
    pub q_violation: f64,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct LimitSummary {
    pub voltage_violations: usize,
    pub worst_voltage: Option<(u32, f64)>,
    pub reactive_violations: usize,
    pub worst_reactive: Option<(u32, f64)>,
    pub branch_overloads: usize,
    /// (from, to, loading %) of the most heavily loaded rated branch.
    pub worst_branch: Option<(u32, u32, f64)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SolveReport {
    pub status: &'static str,
    pub converged: bool,
    pub model: ModelChoice,
    pub iterations: usize,
    pub q_limit_passes: usize,
    pub residual_pu: f64,
    pub residual_mva: f64,
    pub elapsed_ms: f64,
    pub total_generation_mw: f64,
    pub total_load_mw: f64,
    pub total_losses_mw: f64,
    pub limits: LimitSummary,
    pub buses: Vec<BusRow>,
    pub generators: Vec<GeneratorRow>,
    pub branches: Vec<BranchRow>,
    pub dc_lines: Vec<DcLineRow>,
}

pub fn bus_names(case: &Case) -> Vec<String> {
    let labels = case.labels.get("bus_name");
    (0..case.buses.len())
        .map(|i| {
            labels
                .and_then(|l| l.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        })
        .collect()
}

pub(crate) fn kind_label(kind: BusType) -> &'static str {
    match kind {
        BusType::Pq => "PQ",
        BusType::Pv => "PV",
        BusType::Reference => "Ref",
        BusType::Isolated => "Isolated",
    }
}

fn outside(value: f64, low: f64, high: f64) -> f64 {
    if value < low {
        low - value
    } else if value > high {
        value - high
    } else {
        0.0
    }
}

/// Build the frontend report from a solve, including operating-limit checks.
pub fn report(
    result: &PowerFlowResult,
    elements: &Elements,
    dc_kinds: &[&'static str],
    settings: &SolveSettings,
    elapsed_ms: f64,
) -> SolveReport {
    let case = &result.case;
    let names = bus_names(case);
    let name_of = |id: u32| {
        case.buses
            .iter()
            .position(|b| b.id == id)
            .map(|i| names[i].clone())
            .unwrap_or_default()
    };
    let mut limits = LimitSummary::default();

    let buses: Vec<BusRow> = case
        .buses
        .iter()
        .zip(&names)
        .map(|(b, name)| {
            let violation = if b.kind == BusType::Isolated {
                0.0
            } else {
                outside(b.vm, b.vmin, b.vmax)
            };
            if violation > 0.0 {
                limits.voltage_violations += 1;
                if limits.worst_voltage.is_none_or(|(_, w)| violation > w) {
                    limits.worst_voltage = Some((b.id, violation));
                }
            }
            BusRow {
                id: b.id,
                name: name.clone(),
                kind: kind_label(b.kind),
                base_kv: b.base_kv,
                vm: b.vm,
                va: b.va,
                vmin: b.vmin,
                vmax: b.vmax,
                pd: b.pd,
                qd: b.qd,
                area: b.area,
                violation,
            }
        })
        .collect();

    let generators: Vec<GeneratorRow> = case
        .generators
        .iter()
        .enumerate()
        .map(|(index, g)| {
            let q_violation = if g.online && settings.model == ModelChoice::Ac {
                outside(g.qg, g.qmin, g.qmax)
            } else {
                0.0
            };
            if q_violation > 0.0 {
                limits.reactive_violations += 1;
                if limits.worst_reactive.is_none_or(|(_, w)| q_violation > w) {
                    limits.worst_reactive = Some((g.bus, q_violation));
                }
            }
            let e = elements.generators.get(index);
            GeneratorRow {
                index,
                element: e.map(|e| e.element.clone()).unwrap_or_default(),
                machine_id: e.map(|e| e.machine_id.clone()).unwrap_or_default(),
                bus: g.bus,
                bus_name: name_of(g.bus),
                online: g.online,
                pg: g.pg,
                qg: g.qg,
                qmin: g.qmin,
                qmax: g.qmax,
                vg: g.vg,
                pmax: g.pmax,
                q_violation,
            }
        })
        .collect();

    let branches: Vec<BranchRow> = case
        .branches
        .iter()
        .zip(&result.branch_flows)
        .enumerate()
        .map(|(index, (br, f))| {
            let loading_pct = (br.online && br.rate_a > 0.0)
                .then(|| f[0].hypot(f[1]).max(f[2].hypot(f[3])) / br.rate_a * 100.0);
            if let Some(loading) = loading_pct {
                if loading > 100.0 {
                    limits.branch_overloads += 1;
                }
                if limits.worst_branch.is_none_or(|(_, _, w)| loading > w) {
                    limits.worst_branch = Some((br.from, br.to, loading));
                }
            }
            let e = elements.branches.get(index);
            BranchRow {
                index,
                element: e.map(|e| e.element.clone()).unwrap_or_default(),
                circuit: e.map(|e| e.circuit.clone()).unwrap_or_default(),
                winding: e.map_or(0, |e| e.winding),
                from: br.from,
                to: br.to,
                online: br.online,
                is_transformer: (br.tap != 0.0 && br.tap != 1.0) || br.shift != 0.0,
                pf: f[0],
                qf: f[1],
                pt: f[2],
                qt: f[3],
                loss_mw: f[0] + f[2],
                rate_a: br.rate_a,
                loading_pct,
            }
        })
        .collect();

    let dc_lines: Vec<DcLineRow> = case
        .dcline
        .iter()
        .flatten()
        .enumerate()
        .map(|(index, d)| {
            let (from, to) = (d[0] as u32, d[1] as u32);
            let online = d[2] != 0.0;
            let q_violation = if online && settings.model == ModelChoice::Ac {
                outside(d[5], d[11], d[12]).max(outside(d[6], d[13], d[14]))
            } else {
                0.0
            };
            DcLineRow {
                index,
                element: elements.dc_lines.get(index).cloned().unwrap_or_default(),
                kind: dc_kinds.get(index).copied().unwrap_or("two_terminal"),
                from,
                to,
                from_name: name_of(from),
                to_name: name_of(to),
                online,
                pf: d[3],
                pt: d[4],
                qf: d[5],
                qt: d[6],
                vf: d[7],
                vt: d[8],
                loss_mw: if online { d[3] - d[4] } else { 0.0 },
                qminf: d[11],
                qmaxf: d[12],
                qmint: d[13],
                qmaxt: d[14],
                q_violation,
            }
        })
        .collect();

    let total_generation_mw = case
        .generators
        .iter()
        .filter(|g| g.online)
        .map(|g| g.pg)
        .sum();
    let total_load_mw = case
        .buses
        .iter()
        .filter(|b| b.kind != BusType::Isolated)
        .map(|b| b.pd)
        .sum();
    let total_losses_mw = branches.iter().map(|b| b.loss_mw).sum::<f64>()
        + dc_lines.iter().map(|d| d.loss_mw).sum::<f64>();

    let (status, converged) = match result.status {
        SolveStatus::Converged => ("Converged", true),
        SolveStatus::IterationLimit => ("Iteration limit", false),
        SolveStatus::ReactiveLimitsInfeasible => ("Q limits infeasible", false),
    };
    SolveReport {
        status,
        converged,
        model: settings.model,
        iterations: result.iterations,
        q_limit_passes: result.q_limit_passes,
        residual_pu: result.residual,
        residual_mva: result.residual * case.base_mva,
        elapsed_ms,
        total_generation_mw,
        total_load_mw,
        total_losses_mw,
        limits,
        buses,
        generators,
        branches,
        dc_lines,
    }
}

// ---------------------------------------------------------------------------
// Sensitivities
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct InjectionColumn {
    pub column: usize,
    pub bus: u32,
    pub bus_name: String,
    /// "p" (active) or "q" (reactive).
    pub kind: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct QuantityInfo {
    pub key: String,
    pub title: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct SensitivitySummary {
    pub model: ModelChoice,
    pub status: &'static str,
    pub elapsed_ms: f64,
    pub states: usize,
    pub injections: Vec<InjectionColumn>,
    pub branches: usize,
    pub quantities: Vec<QuantityInfo>,
    pub explanation: &'static str,
    pub column_explanation: &'static str,
}

pub const EXPLANATION: &str = "Positive injection means added generation or reduced demand. Active injection is balanced at the reference bus in the same island. Branch-end flows are positive into the branch. Responses are first-order derivatives at the solved operating point.";
pub const COLUMN_EXPLANATION: &str = "Reference-bus and isolated-bus injection columns are omitted. Q columns are available only for PQ buses in AC.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    Angle,
    Magnitude,
    Flow(usize),
}

impl Quantity {
    pub fn key(self) -> String {
        match self {
            Self::Angle => "angle".into(),
            Self::Magnitude => "magnitude".into(),
            Self::Flow(i) => format!("flow{i}"),
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        match key {
            "angle" => Some(Self::Angle),
            "magnitude" => Some(Self::Magnitude),
            "flow0" => Some(Self::Flow(0)),
            "flow1" => Some(Self::Flow(1)),
            "flow2" => Some(Self::Flow(2)),
            "flow3" => Some(Self::Flow(3)),
            _ => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Angle => "Voltage angle",
            Self::Magnitude => "Voltage magnitude",
            Self::Flow(0) => "From-end active flow",
            Self::Flow(1) => "From-end reactive flow",
            Self::Flow(2) => "To-end active flow",
            _ => "To-end reactive flow",
        }
    }

    pub fn units(self, engineering: bool, injection: InjectionVariable) -> String {
        if !engineering {
            return match self {
                Self::Angle => "rad/p.u.",
                _ => "p.u./p.u.",
            }
            .into();
        }
        let denominator = match injection {
            InjectionVariable::ActivePower(_) => "MW",
            InjectionVariable::ReactivePower(_) => "MVAr",
        };
        let numerator = match self {
            Self::Angle => "degrees",
            Self::Magnitude => "p.u.",
            Self::Flow(0 | 2) => "MW",
            Self::Flow(_) => "MVAr",
        };
        format!("{numerator}/{denominator}")
    }

    fn convert(self, value: f64, base_mva: f64, engineering: bool) -> f64 {
        match (engineering, self) {
            (true, Self::Angle) => value.to_degrees() / base_mva,
            (true, Self::Magnitude) => value / base_mva,
            _ => value,
        }
    }
}

pub fn quantities(model: ModelChoice) -> Vec<Quantity> {
    match model {
        ModelChoice::Ac => vec![
            Quantity::Angle,
            Quantity::Magnitude,
            Quantity::Flow(0),
            Quantity::Flow(1),
            Quantity::Flow(2),
            Quantity::Flow(3),
        ],
        ModelChoice::Dc => vec![Quantity::Angle, Quantity::Flow(0), Quantity::Flow(2)],
    }
}

pub fn sensitivity_summary(
    s: &PowerFlowSensitivity,
    model: ModelChoice,
    elapsed_ms: f64,
) -> SensitivitySummary {
    let case = &s.power_flow.case;
    let names = bus_names(case);
    let name_of = |id: u32| {
        case.buses
            .iter()
            .position(|b| b.id == id)
            .map(|i| names[i].clone())
            .unwrap_or_default()
    };
    SensitivitySummary {
        model,
        status: match s.power_flow.status {
            SolveStatus::Converged => "Converged",
            SolveStatus::IterationLimit => "Iteration limit",
            SolveStatus::ReactiveLimitsInfeasible => "Q limits infeasible",
        },
        elapsed_ms,
        states: s.state_variables.len(),
        injections: s
            .injection_variables
            .iter()
            .enumerate()
            .map(|(column, v)| {
                let (bus, kind) = match *v {
                    InjectionVariable::ActivePower(b) => (b, "p"),
                    InjectionVariable::ReactivePower(b) => (b, "q"),
                };
                InjectionColumn {
                    column,
                    bus,
                    bus_name: name_of(bus),
                    kind,
                }
            })
            .collect(),
        branches: case.branches.len(),
        quantities: quantities(model)
            .into_iter()
            .map(|q| QuantityInfo {
                key: q.key(),
                title: q.title(),
            })
            .collect(),
        explanation: EXPLANATION,
        column_explanation: COLUMN_EXPLANATION,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Response {
    /// Original row (state row or branch index), for stable "input order" sorting.
    pub index: usize,
    pub label: String,
    pub value: f64,
    pub offline: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResponseTable {
    pub units: String,
    pub title: &'static str,
    pub rows: Vec<Response>,
}

fn bus_label(names: &[String], case: &Case, id: u32) -> String {
    let name = case
        .buses
        .iter()
        .position(|b| b.id == id)
        .map(|i| names[i].as_str())
        .filter(|n| !n.is_empty());
    match name {
        Some(n) => format!("Bus {id} ({n})"),
        None => format!("Bus {id}"),
    }
}

/// All responses to one injection column, in input order.
pub fn responses(
    s: &PowerFlowSensitivity,
    column: usize,
    quantity: Quantity,
    engineering: bool,
) -> Result<ResponseTable, String> {
    let injection = *s
        .injection_variables
        .get(column)
        .ok_or_else(|| format!("No injection column {column}"))?;
    let case = &s.power_flow.case;
    let names = bus_names(case);
    let rows = match quantity {
        Quantity::Angle | Quantity::Magnitude => s
            .state_variables
            .iter()
            .enumerate()
            .filter_map(|(index, state)| {
                let (bus, q) = match *state {
                    StateVariable::VoltageAngle(b) => (b, Quantity::Angle),
                    StateVariable::VoltageMagnitude(b) => (b, Quantity::Magnitude),
                };
                (q == quantity).then(|| Response {
                    index,
                    label: bus_label(&names, case, bus),
                    value: q.convert(
                        s.state_sensitivities[index][column],
                        case.base_mva,
                        engineering,
                    ),
                    offline: false,
                })
            })
            .collect(),
        Quantity::Flow(component) => case
            .branches
            .iter()
            .enumerate()
            .map(|(index, br)| Response {
                index,
                label: format!(
                    "Branch {}: {} → {}",
                    index + 1,
                    bus_label(&names, case, br.from),
                    bus_label(&names, case, br.to)
                ),
                value: s.branch_flow_sensitivities[index][column][component],
                offline: !br.online,
            })
            .collect(),
    };
    Ok(ResponseTable {
        units: quantity.units(engineering, injection),
        title: quantity.title(),
        rows,
    })
}
