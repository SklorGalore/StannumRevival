//! Power flow and sensitivities (rs-power), shaped for the frontend.

use crate::elements::Elements;
use crate::table::{self, Table};
use rs_power::{
    BusType, Case, Initialization, InjectionVariable, Model, PowerFlowOptions, PowerFlowResult,
    PowerFlowSensitivity, SolveStatus, StateVariable,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// Full data of the two-terminal (LCC) DC lines for the detailed model.
    /// Empty for MATPOWER cases, whose `dcline` rows carry no converter data.
    pub lcc_lines: Vec<rs_power::LccLine>,
    /// Full data of the VSC DC lines for the detailed model.
    pub vsc_lines: Vec<rs_power::VscLine>,
    /// Switched shunts and transformer taps in voltage control (RAW only).
    pub voltage_controls: rs_power::VoltageControls,
    /// Full data of the multi-terminal DC systems for the detailed model.
    pub mtdc: Vec<rs_power::MtdcSystem>,
    /// Substations with their places and buses, from RAW revision 34-35
    /// substation data; empty otherwise.
    pub substations: Vec<rs_power::RawSubstation>,
    /// RAW revision 34-35 system switching devices, each also a branch of
    /// `case`; empty otherwise.
    pub switching_devices: Vec<rs_power::SwitchingDevice>,
    /// The RAW revision the file was read as; None for MATPOWER.
    pub revision: Option<u32>,
}

fn dc_kind_label(kind: rs_power::DcLineKind) -> &'static str {
    match kind {
        rs_power::DcLineKind::TwoTerminal => "two_terminal",
        rs_power::DcLineKind::VoltageSourceConverter => "vsc",
        rs_power::DcLineKind::MultiTerminal => "multi_terminal",
    }
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Warning {
    pub line: usize,
    pub message: String,
    /// The branch or generator whose record the warning is about, as a
    /// shared element id, when the line is one of theirs.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub element: Option<String>,
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
            lcc_lines: Vec::new(),
            vsc_lines: Vec::new(),
            voltage_controls: rs_power::VoltageControls::default(),
            mtdc: Vec::new(),
            substations: Vec::new(),
            switching_devices: Vec::new(),
            revision: None,
        })
    } else {
        // VSC and multi-terminal DC lines are imported too, beyond MATPOWER.
        // Line-commutated converters absorb reactive power, unlike MATPOWER's
        // positive estimates. Generators hold the remote buses they regulate.
        // Loads keep their constant-current and constant-impedance parts.
        let options = rs_power::RawOptions {
            all_dc_lines: true,
            lcc_absorbs_reactive: true,
            remote_regulation: true,
            zip_loads: true,
            ..Default::default()
        };
        let import = rs_power::parse_raw_with_options(text, &options).map_err(|e| e.to_string())?;
        let elements = Elements::from_raw(
            &import.case,
            &import.branch_sources,
            &import.generator_sources,
            &import.dc_line_names,
        );
        // Which element each record's first line belongs to, so a warning
        // can name what it is about.
        let mut by_line: HashMap<usize, String> = HashMap::new();
        for (s, e) in import.branch_sources.iter().zip(&elements.branches) {
            by_line.entry(s.line).or_insert_with(|| e.element.clone());
        }
        for (s, e) in import.generator_sources.iter().zip(&elements.generators) {
            by_line.entry(s.line).or_insert_with(|| e.element.clone());
        }
        Ok(PowerCase {
            elements,
            dc_line_kinds: import
                .dc_line_kinds
                .iter()
                .copied()
                .map(dc_kind_label)
                .collect(),
            lcc_lines: import.lcc_lines,
            vsc_lines: import.vsc_lines,
            voltage_controls: import.voltage_controls,
            mtdc: import.mtdc,
            substations: import.substations,
            switching_devices: import.switching_devices,
            revision: Some(import.revision),
            case: import.case,
            warnings: import
                .warnings
                .into_iter()
                .map(|w| Warning {
                    element: by_line.get(&w.line).cloned(),
                    line: w.line,
                    message: w.message,
                })
                .collect(),
        })
    }
}

#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ModelChoice {
    #[default]
    Ac,
    Dc,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum StartChoice {
    /// The voltages in the case.
    Case,
    /// 1 p.u. at every bus.
    Flat,
    /// Where the last converged AC solve ended, with its controls where they
    /// ended, also after study edits; the case voltages when there is none.
    #[default]
    Last,
}

/// How two-terminal (LCC), VSC and multi-terminal DC lines are solved when
/// the case has their data.
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum DcModelChoice {
    /// MATPOWER's lossless generator pair.
    Simplified,
    /// Converter equations with losses, firing angles and taps.
    #[default]
    Detailed,
}

/// How each AC pass finds the voltages. Every method stops at the same
/// mismatch tolerance, so they reach the same answer at different speeds.
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MethodChoice {
    /// Decoupled iterations first, finishing with Newton when they slow
    /// down; Newton alone on cases under 2,000 buses.
    #[default]
    Auto,
    /// Full Newton–Raphson.
    Newton,
    /// Fixed-slope decoupled Newton (FDNS).
    FixedSlope,
    /// Fast decoupled load flow, XB version.
    FastDecoupledXb,
    /// Fast decoupled load flow, BX version.
    FastDecoupledBx,
}

impl From<MethodChoice> for rs_power::AcMethod {
    fn from(m: MethodChoice) -> Self {
        match m {
            MethodChoice::Auto => Self::Auto,
            MethodChoice::Newton => Self::Newton,
            MethodChoice::FixedSlope => Self::FixedSlope,
            MethodChoice::FastDecoupledXb => Self::FastDecoupledXb,
            MethodChoice::FastDecoupledBx => Self::FastDecoupledBx,
        }
    }
}

fn yes() -> bool {
    true
}

fn decoupled_iterations() -> usize {
    PowerFlowOptions::default().decoupled_iterations
}

/// How to solve. Fields left out take their defaults: AC with the automatic
/// method from the last solution, to 2 MVA, without reactive limits, with
/// every control on.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(default)]
pub struct SolveSettings {
    pub model: ModelChoice,
    /// AC solution method.
    #[serde(default)]
    pub method: MethodChoice,
    pub start: StartChoice,
    /// Largest power mismatch allowed at any bus, in MVA. It converts to per
    /// unit on each case's own base at solve time, so the same setting means
    /// the same MW and Mvar on every case.
    pub tolerance_mva: f64,
    /// Newton iterations per pass.
    pub max_iterations: usize,
    /// Decoupled iterations per pass (fixed-slope and fast decoupled, and
    /// the decoupled stage of the automatic method).
    #[serde(default = "decoupled_iterations")]
    pub decoupled_iterations: usize,
    pub enforce_q_limits: bool,
    #[serde(default)]
    pub dc_model: DcModelChoice,
    /// Step DC converter taps (detailed model only).
    #[serde(default = "yes")]
    pub adjust_dc_taps: bool,
    /// Switch shunts in voltage control (AC only). When false they are
    /// locked: they stay where they are, which is where the last solve left
    /// them when starting from it, and at their case values otherwise.
    #[serde(default = "yes")]
    pub switch_shunts: bool,
    /// Step transformer taps in voltage control (AC only). When false they
    /// are locked, like shunts.
    #[serde(default = "yes")]
    pub adjust_transformer_taps: bool,
}

/// Default mismatch tolerance: 2 MVA on any system base. Looser than the
/// solver library's own default, which suits reference checks rather than
/// day-to-day studies.
pub const DEFAULT_TOLERANCE_MVA: f64 = 2.0;

impl Default for SolveSettings {
    fn default() -> Self {
        let d = PowerFlowOptions::default();
        Self {
            model: ModelChoice::Ac,
            method: MethodChoice::Auto,
            start: StartChoice::Last,
            tolerance_mva: DEFAULT_TOLERANCE_MVA,
            max_iterations: d.max_iterations,
            decoupled_iterations: d.decoupled_iterations,
            enforce_q_limits: false,
            dc_model: DcModelChoice::Detailed,
            adjust_dc_taps: true,
            switch_shunts: true,
            adjust_transformer_taps: true,
        }
    }
}

impl SolveSettings {
    /// Solver options for `power`, using its LCC data when the detailed DC
    /// model is chosen and its voltage controls when they are switched on.
    pub fn options_for(&self, power: &PowerCase) -> PowerFlowOptions {
        let mut options = self.options(power.case.base_mva);
        if self.dc_model == DcModelChoice::Detailed
            && (!power.lcc_lines.is_empty()
                || !power.vsc_lines.is_empty()
                || !power.mtdc.is_empty())
        {
            options.dc_model = rs_power::DcModel::Detailed {
                lines: power.lcc_lines.clone(),
                vsc_lines: power.vsc_lines.clone(),
                mtdc: power.mtdc.clone(),
                adjust_taps: self.adjust_dc_taps,
            };
        }
        if self.model == ModelChoice::Ac {
            // Locked controls stay listed, so that a solve from the last
            // solution starts them where it left them and holds them there.
            options.voltage_controls = power.voltage_controls.clone();
            options.lock_switched_shunts = !self.switch_shunts;
            options.lock_taps = !self.adjust_transformer_taps;
        }
        options
    }

    /// Solver options without detailed DC line data, for a case on
    /// `base_mva`.
    pub fn options(&self, base_mva: f64) -> PowerFlowOptions {
        PowerFlowOptions {
            model: match self.model {
                ModelChoice::Ac => Model::Ac,
                ModelChoice::Dc => Model::Dc,
            },
            initialization: match self.start {
                // Without an earlier solution to start from.
                StartChoice::Case | StartChoice::Last => Initialization::Case,
                StartChoice::Flat => Initialization::Flat,
            },
            tolerance: tolerance_pu(self.tolerance_mva, base_mva),
            max_iterations: self.max_iterations,
            method: self.method.into(),
            decoupled_iterations: self.decoupled_iterations,
            // Q limits only apply to AC; the solver rejects them for DC.
            enforce_q_limits: self.enforce_q_limits && self.model == ModelChoice::Ac,
            ..PowerFlowOptions::default()
        }
    }
}

/// `mva` in per unit on `base_mva`, taking 100 MVA when the base is missing.
pub fn tolerance_pu(mva: f64, base_mva: f64) -> f64 {
    mva / if base_mva > 0.0 { base_mva } else { 100.0 }
}

/// A copy of `case` that starts where `last`, an earlier converged AC solve,
/// ended, with `options` changed to match.
///
/// Bus voltages and generator reactive outputs come from `last`. Switched
/// shunts, transformer taps and DC converter taps that `options` lists
/// start at their final settings, also when `options` locks them there. Everything else, such as dispatch, loads
/// and statuses, stays as in `case`. Generators `last` held at a reactive
/// limit start there, also after study edits: on SyntheticUSA an outage then
/// solves in 6 iterations and lands within 0.02 MW of losses of a solve from
/// the file, while checking every limit again took 43 iterations and landed
/// further off.
pub fn warm_start(case: &Case, last: &PowerFlowResult, options: &mut PowerFlowOptions) -> Case {
    let mut warm = case.clone();
    let solved = &last.case;
    for (b, s) in warm.buses.iter_mut().zip(&solved.buses) {
        if b.id == s.id {
            b.vm = s.vm;
            b.va = s.va;
        }
    }
    for (g, s) in warm.generators.iter_mut().zip(&solved.generators) {
        if g.bus == s.bus && g.online && s.qg.is_finite() {
            g.qg = s.qg;
        }
    }
    let controls = &mut options.voltage_controls;
    if controls.switched_shunts.len() == last.switched_shunts.len() {
        let index: std::collections::HashMap<u32, usize> = warm
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        for (shunt, &value) in controls
            .switched_shunts
            .iter_mut()
            .zip(&last.switched_shunts)
        {
            if let (Some(&i), true) = (index.get(&shunt.bus), value.is_finite()) {
                warm.buses[i].bs += value - shunt.initial;
                shunt.initial = value;
            }
        }
    }
    if controls.taps.len() == last.tap_ratios.len() {
        for (tap, &ratio) in controls.taps.iter_mut().zip(&last.tap_ratios) {
            if ratio.is_finite() && tap.branch < warm.branches.len() {
                tap.ratio = ratio;
                warm.branches[tap.branch].tap = tap.tap_per_ratio * ratio;
            }
        }
    }
    if let rs_power::DcModel::Detailed { lines, .. } = &mut options.dc_model {
        for line in lines {
            if let Some(s) = last.lcc.iter().find(|s| s.row == line.row) {
                line.rectifier.tap = s.rectifier_tap;
                line.inverter.tap = s.inverter_tap;
            }
        }
    }
    options.initialization = Initialization::Solution;
    warm
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
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

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct BusRow {
    pub id: u32,
    pub name: String,
    #[schemars(extend("enum" = ["PQ", "PV", "Ref", "Isolated"]))]
    pub kind: &'static str,
    #[serde(serialize_with = "table::mw")]
    pub base_kv: f64,
    #[serde(serialize_with = "table::pu")]
    pub vm: f64,
    #[serde(serialize_with = "table::mw")]
    pub va: f64,
    #[serde(serialize_with = "table::pu")]
    pub vmin: f64,
    #[serde(serialize_with = "table::pu")]
    pub vmax: f64,
    /// Load drawn at the solved voltage, all parts together.
    #[serde(serialize_with = "table::mw")]
    pub pd: f64,
    #[serde(serialize_with = "table::mw")]
    pub qd: f64,
    /// The load split by how it varies with voltage, when any of it does.
    pub load_parts: Option<LoadParts>,
    pub area: u32,
    /// Distance outside [vmin, vmax] in p.u.; zero when within limits.
    #[serde(serialize_with = "table::pu")]
    pub violation: f64,
}

/// A bus's load at the solved voltage by part, in MW and Mvar:
/// `[constant power, constant current, constant impedance]`.
#[derive(Clone, Debug, Serialize, PartialEq, schemars::JsonSchema)]
pub struct LoadParts {
    pub p: [f64; 3],
    pub q: [f64; 3],
}

impl LoadParts {
    /// The parts `bus` draws at voltage `vm`, or `None` when its load is all
    /// constant power.
    pub fn of(bus: &rs_power::Bus, vm: f64) -> Option<Self> {
        bus.has_voltage_dependent_load().then_some(Self {
            p: [bus.pd, bus.pd_i * vm, bus.pd_z * vm * vm],
            q: [bus.qd, bus.qd_i * vm, bus.qd_z * vm * vm],
        })
    }
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct GeneratorRow {
    pub index: usize,
    /// Shared id, such as `generator:101:1`.
    pub element: String,
    pub machine_id: String,
    pub bus: u32,
    pub bus_name: String,
    pub online: bool,
    #[serde(serialize_with = "table::mw")]
    pub pg: f64,
    #[serde(serialize_with = "table::mw")]
    pub qg: f64,
    #[serde(serialize_with = "table::mw")]
    pub qmin: f64,
    #[serde(serialize_with = "table::mw")]
    pub qmax: f64,
    #[serde(serialize_with = "table::pu")]
    pub vg: f64,
    #[serde(serialize_with = "table::mw")]
    pub pmax: f64,
    /// MVAr outside [qmin, qmax]; zero when within limits or offline.
    #[serde(serialize_with = "table::mw")]
    pub q_violation: f64,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
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
    #[serde(serialize_with = "table::mw")]
    pub pf: f64,
    #[serde(serialize_with = "table::mw")]
    pub qf: f64,
    #[serde(serialize_with = "table::mw")]
    pub pt: f64,
    #[serde(serialize_with = "table::mw")]
    pub qt: f64,
    #[serde(serialize_with = "table::mw")]
    pub loss_mw: f64,
    #[serde(serialize_with = "table::mw")]
    pub rate_a: f64,
    /// Highest end MVA as a percent of rate A; `None` when unrated or offline.
    #[serde(serialize_with = "table::pct")]
    pub loading_pct: Option<f64>,
}

/// A solved DC line row, from its sending (rectifier) bus to its receiving
/// (inverter) bus.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct DcLineRow {
    pub index: usize,
    /// Shared id, such as `dcline:101:205:DC1`.
    pub element: String,
    /// "two_terminal", "vsc" or "multi_terminal".
    #[schemars(extend("enum" = ["two_terminal", "vsc", "multi_terminal"]))]
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
    /// Largest MVAr outside either end's range; zero when within, offline,
    /// or solved with the detailed model (which has no Q range).
    pub q_violation: f64,
    /// Converter operating point from the detailed LCC model.
    pub detail: Option<LccDetail>,
    /// Converter operating point from the detailed VSC model.
    pub vsc_detail: Option<VscDetail>,
    /// This row's converter from the detailed multi-terminal model.
    pub mtdc_detail: Option<MtdcDetail>,
}

/// One converter of a multi-terminal DC system under the detailed model, as
/// shown on the row that links it to the voltage-controlling converter.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct MtdcDetail {
    /// The system's name.
    pub system: String,
    /// "voltage", "order", "angle_limited" or "blocked".
    #[schemars(extend("enum" = ["voltage", "order", "angle_limited", "blocked"]))]
    pub control: &'static str,
    /// Low DC voltage switched the power orders to current orders.
    pub reduced_voltage: bool,
    /// Whether the converter is the row's sending end.
    pub at_from: bool,
    pub rectifier: bool,
    pub current_ka: f64,
    /// DC voltage across the converter, kV.
    pub dc_kv: f64,
    /// Firing (rectifier) or extinction (inverter) angle, degrees.
    pub angle: f64,
    pub tap: f64,
}

fn mtdc_control(mode: rs_power::MtdcMode) -> &'static str {
    match mode {
        rs_power::MtdcMode::VoltageControl => "voltage",
        rs_power::MtdcMode::OrderControl => "order",
        rs_power::MtdcMode::AngleLimited => "angle_limited",
        rs_power::MtdcMode::Blocked => "blocked",
    }
}

/// A multi-terminal DC system's DC network under the detailed model.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct DcSystemReport {
    pub name: String,
    pub reduced_voltage: bool,
    pub line_loss_mw: f64,
    pub converter_loss_mw: f64,
    pub converters: Vec<DcConverterRow>,
    pub buses: Vec<DcBusRow>,
    pub links: Vec<DcLinkRow>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct DcConverterRow {
    /// AC bus number and name.
    pub bus: u32,
    pub bus_name: String,
    /// DC bus number.
    pub dc_bus: u32,
    #[schemars(extend("enum" = ["voltage", "order", "angle_limited", "blocked"]))]
    pub control: &'static str,
    pub rectifier: bool,
    pub current_ka: f64,
    pub dc_kv: f64,
    pub angle: f64,
    pub tap: f64,
    /// MW drawn from the AC bus (negative: delivered).
    pub p_mw: f64,
    /// MVAr absorbed at the AC bus.
    pub q_mvar: f64,
    pub loss_mw: f64,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct DcBusRow {
    pub number: u32,
    pub name: String,
    pub kv: f64,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct DcLinkRow {
    pub from: u32,
    pub to: u32,
    pub circuit: String,
    pub current_ka: f64,
    pub from_mw: f64,
    pub to_mw: f64,
    pub loss_mw: f64,
}

/// Operating point of a VSC DC line under the detailed model.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct VscDetail {
    /// DC current from the sending to the receiving end, kA.
    pub current_ka: f64,
    /// DC voltage at the sending and receiving converters, kV.
    pub from_kv: f64,
    pub to_kv: f64,
    /// Converter losses at each end, MW.
    pub from_loss_mw: f64,
    pub to_loss_mw: f64,
    pub line_loss_mw: f64,
    /// Fraction of the power order kept after MVA or current limits.
    pub order_scale: f64,
    /// Whether each end's MVA or current limit reduced its output.
    pub from_limited: bool,
    pub to_limited: bool,
}

fn vsc_detail(s: &rs_power::VscSolution) -> VscDetail {
    VscDetail {
        current_ka: s.current_ka,
        from_kv: s.from_kv,
        to_kv: s.to_kv,
        from_loss_mw: s.from_loss_mw,
        to_loss_mw: s.to_loss_mw,
        line_loss_mw: s.line_loss_mw,
        order_scale: s.order_scale,
        from_limited: s.from_limited,
        to_limited: s.to_limited,
    }
}

/// Operating point of a two-terminal DC line under the detailed model.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct LccDetail {
    /// "rectifier", "inverter" (current margin), "angle_limited" or "blocked":
    /// which converter holds the DC current.
    #[schemars(extend("enum" = ["rectifier", "inverter", "angle_limited", "blocked"]))]
    pub control: &'static str,
    /// Low DC voltage switched a power order to current control.
    pub reduced_voltage: bool,
    pub current_ka: f64,
    pub rectifier_kv: f64,
    pub inverter_kv: f64,
    pub alpha: f64,
    pub gamma: f64,
    pub rectifier_tap: f64,
    pub inverter_tap: f64,
    pub line_loss_mw: f64,
    pub converter_loss_mw: f64,
}

fn lcc_detail(s: &rs_power::LccSolution) -> LccDetail {
    LccDetail {
        control: match s.mode {
            rs_power::LccMode::RectifierControl => "rectifier",
            rs_power::LccMode::InverterControl => "inverter",
            rs_power::LccMode::AngleLimited => "angle_limited",
            rs_power::LccMode::Blocked => "blocked",
        },
        reduced_voltage: s.reduced_voltage,
        current_ka: s.current_ka,
        rectifier_kv: s.rectifier_kv,
        inverter_kv: s.inverter_kv,
        alpha: s.alpha,
        gamma: s.gamma,
        rectifier_tap: s.rectifier_tap,
        inverter_tap: s.inverter_tap,
        line_loss_mw: s.line_loss_mw,
        converter_loss_mw: s.converter_loss_mw,
    }
}

#[derive(Clone, Debug, Serialize, Default, schemars::JsonSchema)]
pub struct LimitSummary {
    pub voltage_violations: usize,
    pub worst_voltage: Option<(u32, f64)>,
    pub reactive_violations: usize,
    pub worst_reactive: Option<(u32, f64)>,
    pub branch_overloads: usize,
    /// (from, to, loading %) of the most heavily loaded rated branch.
    pub worst_branch: Option<(u32, u32, f64)>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct SolveReport {
    pub status: &'static str,
    pub converged: bool,
    pub model: ModelChoice,
    /// The AC method asked for.
    pub method: MethodChoice,
    /// Iterations in all passes, Newton and decoupled together.
    pub iterations: usize,
    /// How many of `iterations` were decoupled iterations.
    pub decoupled_iterations: usize,
    /// Passes in which the automatic method finished with Newton.
    pub newton_fallbacks: usize,
    pub q_limit_passes: usize,
    /// Passes that stepped DC converter taps (detailed model).
    pub dc_tap_passes: usize,
    /// Passes that reduced VSC output to converter MVA or current limits.
    pub vsc_limit_passes: usize,
    /// Passes that switched shunts or stepped transformer taps.
    pub voltage_control_passes: usize,
    /// Switched shunts that ended away from their starting susceptance.
    pub shunts_switched: usize,
    /// Transformer taps that ended away from their starting ratio.
    pub taps_moved: usize,
    /// Whether the solve locked the switched shunts, and the transformer
    /// taps, where they were instead of moving them (AC only).
    pub shunts_locked: bool,
    pub taps_locked: bool,
    pub residual_pu: f64,
    pub residual_mva: f64,
    pub elapsed_ms: f64,
    pub total_generation_mw: f64,
    pub total_load_mw: f64,
    pub total_losses_mw: f64,
    pub limits: LimitSummary,
    /// Sent as columns (see [`Table`]); the frontend turns them back into rows.
    pub buses: Table<BusRow>,
    pub generators: Table<GeneratorRow>,
    pub branches: Table<BranchRow>,
    pub dc_lines: Vec<DcLineRow>,
    /// DC networks of the multi-terminal systems (detailed model).
    pub dc_systems: Vec<DcSystemReport>,
    /// VSC DC grids from an attached DC grid file, solved in sequence with
    /// the AC network.
    pub vsc_grids: Option<crate::dc_grids::VscGridsReport>,
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
/// How many devices ended more than a rounding error from where they began.
fn moved<T>(values: &[f64], devices: &[T], start: impl Fn(&T) -> f64) -> usize {
    values
        .iter()
        .zip(devices)
        .filter(|(v, d)| (*v - start(d)).abs() > 1e-6)
        .count()
}

pub fn report(
    result: &PowerFlowResult,
    elements: &Elements,
    dc_kinds: &[&'static str],
    controls: &rs_power::VoltageControls,
    settings: &SolveSettings,
    elapsed_ms: f64,
) -> SolveReport {
    let case = &result.case;
    let names = bus_names(case);
    // Looked up once per generator and branch end, so index the buses.
    let index: std::collections::HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let name_of = |id: u32| {
        index
            .get(&id)
            .map(|&i| names[i].clone())
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
            let (pd, qd) = b.load();
            BusRow {
                id: b.id,
                name: name.clone(),
                kind: kind_label(b.kind),
                base_kv: b.base_kv,
                vm: b.vm,
                va: b.va,
                vmin: b.vmin,
                vmax: b.vmax,
                pd,
                qd,
                load_parts: LoadParts::of(b, b.vm),
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
            let detail = result.lcc.iter().find(|s| s.row == index).map(lcc_detail);
            let vsc_detail = result.vsc.iter().find(|s| s.row == index).map(vsc_detail);
            let mtdc_detail = result.mtdc.iter().find_map(|s| {
                let c = s.converters.iter().find(|c| c.row == Some(index))?;
                Some(MtdcDetail {
                    system: s.name.clone(),
                    control: mtdc_control(c.mode),
                    reduced_voltage: s.reduced_voltage,
                    at_from: c.bus == from,
                    rectifier: c.rectifier,
                    current_ka: c.current_ka,
                    dc_kv: c.dc_kv,
                    angle: c.angle,
                    tap: c.tap,
                })
            });
            let q_violation = if online
                && settings.model == ModelChoice::Ac
                && detail.is_none()
                && mtdc_detail.is_none()
            {
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
                detail,
                vsc_detail,
                mtdc_detail,
            }
        })
        .collect();
    let dc_systems = result
        .mtdc
        .iter()
        .map(|s| DcSystemReport {
            name: s.name.clone(),
            reduced_voltage: s.reduced_voltage,
            line_loss_mw: s.line_loss_mw,
            converter_loss_mw: s.converter_loss_mw,
            converters: s
                .converters
                .iter()
                .map(|c| DcConverterRow {
                    bus: c.bus,
                    bus_name: name_of(c.bus),
                    dc_bus: c.dc_bus,
                    control: mtdc_control(c.mode),
                    rectifier: c.rectifier,
                    current_ka: c.current_ka,
                    dc_kv: c.dc_kv,
                    angle: c.angle,
                    tap: c.tap,
                    p_mw: c.ac_mw,
                    q_mvar: c.ac_mvar,
                    loss_mw: c.loss_mw,
                })
                .collect(),
            buses: s
                .buses
                .iter()
                .map(|b| DcBusRow {
                    number: b.number,
                    name: b.name.clone(),
                    kv: b.kv,
                })
                .collect(),
            links: s
                .links
                .iter()
                .map(|l| DcLinkRow {
                    from: l.from,
                    to: l.to,
                    circuit: l.circuit.clone(),
                    current_ka: l.current_ka,
                    from_mw: l.from_mw,
                    to_mw: l.to_mw,
                    loss_mw: l.loss_mw,
                })
                .collect(),
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
        .map(|b| b.load().0)
        .sum();
    let total_losses_mw = branches.iter().map(|b| b.loss_mw).sum::<f64>()
        + dc_lines.iter().map(|d| d.loss_mw).sum::<f64>();

    let ac = settings.model == ModelChoice::Ac;
    let (status, converged) = match result.status {
        SolveStatus::Converged => ("Converged", true),
        SolveStatus::IterationLimit => ("Iteration limit", false),
        SolveStatus::ReactiveLimitsInfeasible => ("Q limits infeasible", false),
    };
    SolveReport {
        status,
        converged,
        model: settings.model,
        method: settings.method,
        iterations: result.iterations,
        decoupled_iterations: result.decoupled_iterations,
        newton_fallbacks: result.newton_fallbacks,
        q_limit_passes: result.q_limit_passes,
        dc_tap_passes: result.dc_tap_passes,
        vsc_limit_passes: result.vsc_limit_passes,
        voltage_control_passes: result.voltage_control_passes,
        shunts_switched: moved(&result.switched_shunts, &controls.switched_shunts, |s| {
            s.initial
        }),
        taps_moved: moved(&result.tap_ratios, &controls.taps, |t| t.ratio),
        shunts_locked: ac && !settings.switch_shunts,
        taps_locked: ac && !settings.adjust_transformer_taps,
        residual_pu: result.residual,
        residual_mva: result.residual * case.base_mva,
        elapsed_ms,
        total_generation_mw,
        total_load_mw,
        total_losses_mw,
        limits,
        buses: buses.into(),
        generators: generators.into(),
        branches: branches.into(),
        dc_lines,
        dc_systems,
        vsc_grids: None,
    }
}

// ---------------------------------------------------------------------------
// Sensitivities
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct InjectionColumn {
    pub column: usize,
    pub bus: u32,
    pub bus_name: String,
    /// "p" (active) or "q" (reactive).
    #[schemars(extend("enum" = ["p", "q"]))]
    pub kind: &'static str,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct QuantityInfo {
    pub key: String,
    pub title: &'static str,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
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
    // Looked up once per generator and branch end, so index the buses.
    let index: std::collections::HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let name_of = |id: u32| {
        index
            .get(&id)
            .map(|&i| names[i].clone())
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

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Response {
    /// Original row (state row or branch index), for stable "input order" sorting.
    pub index: usize,
    pub label: String,
    /// Shared element id of the bus or branch, for drawing on the diagram.
    pub element: String,
    pub value: f64,
    pub offline: bool,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
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
    elements: &crate::elements::Elements,
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
                    element: format!("bus:{bus}"),
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
                element: elements
                    .branches
                    .get(index)
                    .map(|e| e.element.clone())
                    .unwrap_or_default(),
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
