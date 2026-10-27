//! Stannum DC grid files: meshed and multi-terminal VSC DC grids that sit
//! beside an AC case, and their results.
//!
//! A DC grid file is JSON (`*.dcgrid.json`). It names AC buses of the open
//! case by number, so one file can serve every variant of a case. See the
//! "Stannum DC grid file" page of the documentation for every field.
//!
//! ```json
//! {
//!   "format": "stannum-dc-grids",
//!   "version": 1,
//!   "grids": [{
//!     "name": "North Sea", "base_kv": 320, "poles": 2,
//!     "buses": [{ "number": 1, "name": "Hub" }, { "number": 2 }],
//!     "lines": [{ "from": 1, "to": 2, "r_ohm": 4.2 }],
//!     "converters": [
//!       { "name": "Shore", "ac_bus": 5, "dc_bus": 1,
//!         "dc_control": { "mode": "slack", "v_dc_pu": 1.0 },
//!         "ac_control": { "mode": "reactive", "q_mvar": 0 },
//!         "reactor": { "r_pu": 0.0001, "x_pu": 0.16 } },
//!       { "name": "Wind", "ac_bus": 9, "dc_bus": 2,
//!         "dc_control": { "mode": "power", "p_mw": -400 },
//!         "ac_control": { "mode": "voltage", "v_pu": 1.0 },
//!         "reactor": { "r_pu": 0.0001, "x_pu": 0.16 } }
//!     ]
//!   }]
//! }
//! ```

use num_complex::Complex64 as C;
use rs_power::{
    AcControl, AcDcResult, AcState, ConverterLimits, ConverterLosses, DcControl, DcGrid, DcGridBus,
    DcGridLine, GridConverter,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The `format` value every DC grid file carries.
pub const FORMAT: &str = "stannum-dc-grids";
/// The file version this build reads and writes.
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DcGridFile {
    pub format: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub grids: Vec<GridData>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GridData {
    pub name: String,
    /// Pole-to-ground DC voltage base in kV.
    pub base_kv: f64,
    /// 1 (asymmetric monopole) or 2 (symmetric monopole or bipole).
    #[serde(default = "two")]
    pub poles: u8,
    pub buses: Vec<BusData>,
    #[serde(default)]
    pub lines: Vec<LineData>,
    pub converters: Vec<ConverterData>,
}

fn two() -> u8 {
    2
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BusData {
    pub number: u32,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineData {
    pub from: u32,
    pub to: u32,
    #[serde(default)]
    pub name: String,
    /// Resistance of one pole conductor in ohms.
    pub r_ohm: f64,
    #[serde(default = "yes")]
    pub in_service: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Impedance {
    #[serde(default)]
    pub r_pu: f64,
    #[serde(default)]
    pub x_pu: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum DcControlData {
    Power {
        p_mw: f64,
    },
    Slack {
        v_dc_pu: f64,
    },
    Droop {
        p_mw: f64,
        v_dc_pu: f64,
        mw_per_kv: f64,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum AcControlData {
    Reactive { q_mvar: f64 },
    Voltage { v_pu: f64 },
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LossData {
    #[serde(default)]
    pub a_mw: f64,
    #[serde(default)]
    pub b_kv: f64,
    #[serde(default)]
    pub c_rectifier_ohm: f64,
    #[serde(default)]
    pub c_inverter_ohm: f64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LimitData {
    #[serde(default)]
    pub i_max_pu: f64,
    #[serde(default)]
    pub uc_max_pu: f64,
    #[serde(default)]
    pub uc_min_pu: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConverterData {
    #[serde(default)]
    pub name: String,
    pub ac_bus: u32,
    pub dc_bus: u32,
    #[serde(default = "yes")]
    pub in_service: bool,
    pub dc_control: DcControlData,
    pub ac_control: AcControlData,
    #[serde(default)]
    pub transformer: Impedance,
    #[serde(default)]
    pub filter_b_pu: f64,
    #[serde(default)]
    pub reactor: Impedance,
    #[serde(default)]
    pub losses: LossData,
    #[serde(default)]
    pub limits: LimitData,
}

impl From<&GridData> for DcGrid {
    fn from(g: &GridData) -> Self {
        DcGrid {
            name: g.name.clone(),
            base_kv: g.base_kv,
            poles: g.poles,
            buses: g
                .buses
                .iter()
                .map(|b| DcGridBus {
                    number: b.number,
                    name: b.name.clone(),
                })
                .collect(),
            lines: g
                .lines
                .iter()
                .map(|l| DcGridLine {
                    from: l.from,
                    to: l.to,
                    name: l.name.clone(),
                    resistance_ohm: l.r_ohm,
                    in_service: l.in_service,
                })
                .collect(),
            converters: g
                .converters
                .iter()
                .map(|c| GridConverter {
                    name: c.name.clone(),
                    ac_bus: c.ac_bus,
                    dc_bus: c.dc_bus,
                    in_service: c.in_service,
                    dc_control: match c.dc_control {
                        DcControlData::Power { p_mw } => DcControl::Power { p_mw },
                        DcControlData::Slack { v_dc_pu } => DcControl::Slack { v_dc_pu },
                        DcControlData::Droop {
                            p_mw,
                            v_dc_pu,
                            mw_per_kv,
                        } => DcControl::Droop {
                            p_mw,
                            v_dc_pu,
                            mw_per_kv,
                        },
                    },
                    ac_control: match c.ac_control {
                        AcControlData::Reactive { q_mvar } => AcControl::Reactive { q_mvar },
                        AcControlData::Voltage { v_pu } => AcControl::Voltage { v_pu },
                    },
                    transformer: C::new(c.transformer.r_pu, c.transformer.x_pu),
                    filter_b: c.filter_b_pu,
                    reactor: C::new(c.reactor.r_pu, c.reactor.x_pu),
                    losses: ConverterLosses {
                        a_mw: c.losses.a_mw,
                        b_kv: c.losses.b_kv,
                        c_rectifier_ohm: c.losses.c_rectifier_ohm,
                        c_inverter_ohm: c.losses.c_inverter_ohm,
                    },
                    limits: ConverterLimits {
                        current_max_pu: c.limits.i_max_pu,
                        uc_max_pu: c.limits.uc_max_pu,
                        uc_min_pu: c.limits.uc_min_pu,
                    },
                })
                .collect(),
        }
    }
}

/// Read a DC grid file's text.
pub fn parse(text: &str) -> Result<Vec<DcGrid>, String> {
    let file: DcGridFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if file.format != FORMAT {
        return Err(format!(
            "not a Stannum DC grid file: \"format\" is \"{}\", expected \"{FORMAT}\"",
            file.format
        ));
    }
    if file.version != VERSION {
        return Err(format!(
            "DC grid file version {} is not supported (this build reads version {VERSION})",
            file.version
        ));
    }
    if file.grids.is_empty() {
        return Err("the file has no DC grids".into());
    }
    Ok(file.grids.iter().map(DcGrid::from).collect())
}

/// DC grids attached to the open case.
#[derive(Clone, Debug)]
pub struct LoadedDcGrids {
    pub path: PathBuf,
    pub grids: Vec<DcGrid>,
}

/// What the app shows about the attached DC grid file.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct DcGridsSummary {
    pub path: String,
    pub name: String,
    pub grids: Vec<DcGridCounts>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct DcGridCounts {
    pub name: String,
    pub base_kv: f64,
    pub poles: u8,
    pub buses: usize,
    pub lines: usize,
    pub converters: usize,
}

impl LoadedDcGrids {
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let grids = parse(&crate::workspace::decode_text(&bytes))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            grids,
        })
    }

    pub fn summary(&self) -> DcGridsSummary {
        DcGridsSummary {
            path: self.path.display().to_string(),
            name: self
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            grids: self
                .grids
                .iter()
                .map(|g| DcGridCounts {
                    name: g.name.clone(),
                    base_kv: g.base_kv,
                    poles: g.poles,
                    buses: g.buses.len(),
                    lines: g.lines.len(),
                    converters: g.converters.len(),
                })
                .collect(),
        }
    }
}

/// The sequential AC/DC solve, for the VSC grids tab.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct VscGridsReport {
    pub file: String,
    pub converged: bool,
    /// AC/DC iterations, and AC Newton steps over all of them.
    pub iterations: usize,
    pub ac_iterations: usize,
    /// Largest converter change in the last iteration, MVA.
    pub last_change_mva: f64,
    pub grids: Vec<VscGridReport>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct VscGridReport {
    pub name: String,
    pub base_kv: f64,
    pub poles: u8,
    pub line_loss_mw: f64,
    pub converter_loss_mw: f64,
    pub converters: Vec<VscGridConverterRow>,
    pub buses: Vec<VscGridBusRow>,
    pub lines: Vec<VscGridLineRow>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct VscGridConverterRow {
    pub name: String,
    pub ac_bus: u32,
    pub ac_bus_name: String,
    pub dc_bus: u32,
    pub in_service: bool,
    /// "slack", "power" or "droop".
    #[schemars(extend("enum" = ["slack", "power", "droop"]))]
    pub dc_control: &'static str,
    /// "reactive", "voltage" or "reactive_limit".
    #[schemars(extend("enum" = ["reactive", "voltage", "reactive_limit"]))]
    pub ac_control: &'static str,
    pub ps_mw: f64,
    pub qs_mvar: f64,
    pub us_pu: f64,
    pub us_deg: f64,
    pub pc_mw: f64,
    pub qc_mvar: f64,
    pub uc_pu: f64,
    pub uc_deg: f64,
    pub current_pu: f64,
    pub current_ka: f64,
    pub loss_mw: f64,
    pub pdc_mw: f64,
    pub udc_pu: f64,
    pub udc_kv: f64,
    /// Reactive range at the operating point; None without limits.
    pub q_min_mvar: Option<f64>,
    pub q_max_mvar: Option<f64>,
    pub p_limited: bool,
    pub outside_limits: bool,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct VscGridBusRow {
    pub number: u32,
    pub name: String,
    pub v_pu: f64,
    pub kv: f64,
    pub injection_mw: f64,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct VscGridLineRow {
    pub from: u32,
    pub to: u32,
    pub name: String,
    pub in_service: bool,
    pub current_ka: f64,
    pub from_mw: f64,
    pub to_mw: f64,
    pub loss_mw: f64,
}

fn finite(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

/// The VSC grids tab's tables from a solve.
pub fn report(
    file: &LoadedDcGrids,
    result: &AcDcResult,
    bus_name: impl Fn(u32) -> String,
) -> VscGridsReport {
    VscGridsReport {
        file: file.summary().name,
        converged: result.status == rs_power::SolveStatus::Converged,
        iterations: result.iterations,
        ac_iterations: result.ac_iterations,
        last_change_mva: result.last_change_mva,
        grids: file
            .grids
            .iter()
            .zip(&result.grids)
            .map(|(g, s)| VscGridReport {
                name: g.name.clone(),
                base_kv: g.base_kv,
                poles: g.poles,
                line_loss_mw: s.line_loss_mw,
                converter_loss_mw: s.converter_loss_mw,
                converters: s
                    .converters
                    .iter()
                    .map(|c| VscGridConverterRow {
                        name: c.name.clone(),
                        ac_bus: c.ac_bus,
                        ac_bus_name: bus_name(c.ac_bus),
                        dc_bus: c.dc_bus,
                        in_service: c.in_service,
                        dc_control: match c.dc_control {
                            DcControl::Power { .. } => "power",
                            DcControl::Slack { .. } => "slack",
                            DcControl::Droop { .. } => "droop",
                        },
                        ac_control: match c.ac_state {
                            AcState::Reactive => "reactive",
                            AcState::Voltage => "voltage",
                            AcState::ReactiveLimit => "reactive_limit",
                        },
                        ps_mw: c.ps_mw,
                        qs_mvar: c.qs_mvar,
                        us_pu: c.us_pu,
                        us_deg: c.us_deg,
                        pc_mw: c.pc_mw,
                        qc_mvar: c.qc_mvar,
                        uc_pu: c.uc_pu,
                        uc_deg: c.uc_deg,
                        current_pu: c.current_pu,
                        current_ka: c.current_ka,
                        loss_mw: c.loss_mw,
                        pdc_mw: c.pdc_mw,
                        udc_pu: c.udc_pu,
                        udc_kv: c.udc_kv,
                        q_min_mvar: finite(c.q_min_mvar),
                        q_max_mvar: finite(c.q_max_mvar),
                        p_limited: c.p_limited,
                        outside_limits: c.outside_limits,
                    })
                    .collect(),
                buses: s
                    .buses
                    .iter()
                    .map(|b| VscGridBusRow {
                        number: b.number,
                        name: b.name.clone(),
                        v_pu: b.v_pu,
                        kv: b.kv,
                        injection_mw: b.injection_mw,
                    })
                    .collect(),
                lines: s
                    .lines
                    .iter()
                    .map(|l| VscGridLineRow {
                        from: l.from,
                        to: l.to,
                        name: l.name.clone(),
                        in_service: l.in_service,
                        current_ka: l.current_ka,
                        from_mw: l.from_mw,
                        to_mw: l.to_mw,
                        loss_mw: l.loss_mw,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_other_files_and_typos() {
        assert!(
            parse(r#"{"format":"x","version":1,"grids":[]}"#)
                .unwrap_err()
                .contains("not a Stannum DC grid file")
        );
        assert!(
            parse(r#"{"format":"stannum-dc-grids","version":2,"grids":[]}"#)
                .unwrap_err()
                .contains("version 2")
        );
        let typo = r#"{"format":"stannum-dc-grids","version":1,"grids":[{"name":"a","base_kv":1,
            "buses":[{"number":1}],"converters":[{"ac_bus":1,"dc_bus":1,
            "dc_control":{"mode":"slack","v_dc_pu":1},"ac_control":{"mode":"reactive","q_mvr":0}}]}]}"#;
        assert!(parse(typo).unwrap_err().contains("q_mvr"));
    }

    #[test]
    fn defaults_fill_optional_fields() {
        let text = r#"{"format":"stannum-dc-grids","version":1,"grids":[{"name":"a","base_kv":150,
            "buses":[{"number":1},{"number":2}],"lines":[{"from":1,"to":2,"r_ohm":3}],
            "converters":[{"ac_bus":4,"dc_bus":1,"dc_control":{"mode":"droop","p_mw":10,"v_dc_pu":1,"mw_per_kv":2},
            "ac_control":{"mode":"voltage","v_pu":1.02},"reactor":{"x_pu":0.15}}]}]}"#;
        let [g] = &parse(text).unwrap()[..] else {
            panic!()
        };
        assert_eq!(g.poles, 2);
        assert!(g.lines[0].in_service);
        let c = &g.converters[0];
        assert!(c.in_service);
        assert_eq!(c.reactor, C::new(0.0, 0.15));
        assert_eq!(c.transformer, C::new(0.0, 0.0));
        assert_eq!(c.limits, ConverterLimits::default());
        assert_eq!(
            c.dc_control,
            DcControl::Droop {
                p_mw: 10.0,
                v_dc_pu: 1.0,
                mw_per_kv: 2.0
            }
        );
    }
}
