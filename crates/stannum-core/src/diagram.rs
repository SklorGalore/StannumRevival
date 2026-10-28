//! The open case as a network of buses, injections and branches, shaped for
//! drawing a one-line diagram. Positions are not computed here: the frontend
//! lays the network out, so this only says what exists and how it connects.

use crate::power::{PowerCase, bus_names, kind_label};
use rs_power::SwitchType;
use serde::Serialize;
use std::collections::HashMap;

/// rs-power names the internal star bus of a three-winding transformer this way.
const STAR_PREFIX: &str = "STR_PT_XF_";

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct NetworkBus {
    pub id: u32,
    pub name: String,
    pub base_kv: f64,
    pub area: u32,
    #[schemars(extend("enum" = ["PQ", "PV", "Ref", "Isolated"]))]
    pub kind: &'static str,
    /// Load at the bus in MW / MVAr (rs-power aggregates loads per bus).
    pub pd: f64,
    pub qd: f64,
    /// Shunt MW / MVAr at 1 p.u.
    pub gs: f64,
    pub bs: f64,
    /// The star point of a three-winding transformer, not a real bus.
    pub star: bool,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct NetworkGenerator {
    /// Index in the case's generator table (matches the power flow report).
    pub index: usize,
    /// Shared id, such as `generator:101:1`.
    pub element: String,
    pub bus: u32,
    pub online: bool,
    pub pmax: f64,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct NetworkBranch {
    /// Index in the case's branch table (matches the power flow report).
    pub index: usize,
    /// Shared id, such as `line:101:102:1`.
    pub element: String,
    pub circuit: String,
    /// 1, 2 or 3 for a leg of a three-winding transformer; 0 otherwise.
    pub winding: u8,
    pub from: u32,
    pub to: u32,
    pub online: bool,
    pub transformer: bool,
    pub rate_a: f64,
    /// Off-nominal turns ratio (1.0 when unity) and phase shift in degrees.
    pub tap: f64,
    pub shift: f64,
    /// Set when the branch is a RAW system switching device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switching: Option<NetworkSwitch>,
}

/// A system switching device drawn on a one-line.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct NetworkSwitch {
    /// "breaker", "disconnector" or "connector" (PSS/E's generic type).
    #[schemars(extend("enum" = ["breaker", "disconnector", "connector"]))]
    pub kind: &'static str,
    /// The device's name; may be empty.
    pub name: String,
    /// Its normal status is open.
    pub normally_open: bool,
}

/// A DC line, from its sending (rectifier) bus to its receiving (inverter)
/// bus. A multi-terminal system gives one per scheduled converter, each
/// running to the converter that holds the system's DC voltage.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct NetworkDcLine {
    /// Row in the case's DC line table.
    pub index: usize,
    /// Shared id, such as `dcline:101:205:DC1`.
    pub element: String,
    /// The RAW name, or a number for MATPOWER cases.
    pub name: String,
    /// "two_terminal", "vsc" or "multi_terminal".
    #[schemars(extend("enum" = ["two_terminal", "vsc", "multi_terminal"]))]
    pub kind: &'static str,
    pub from: u32,
    pub to: u32,
    pub online: bool,
    /// Scheduled MW sent from the rectifier. The power flow holds it fixed.
    pub mw: f64,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct CaseNetwork {
    pub base_mva: f64,
    pub buses: Vec<NetworkBus>,
    pub generators: Vec<NetworkGenerator>,
    pub branches: Vec<NetworkBranch>,
    pub dc_lines: Vec<NetworkDcLine>,
}

pub fn network(power: &PowerCase) -> CaseNetwork {
    let case = &power.case;
    let names = bus_names(case);
    let kv: HashMap<u32, f64> = case.buses.iter().map(|b| (b.id, b.base_kv)).collect();
    let base_kv = |id: u32| kv.get(&id).copied().unwrap_or(0.0);
    let switches: HashMap<usize, &rs_power::SwitchingDevice> = power
        .switching_devices
        .iter()
        .map(|d| (d.branch, d))
        .collect();
    let buses = case
        .buses
        .iter()
        .zip(names)
        .map(|(b, name)| NetworkBus {
            id: b.id,
            star: name.starts_with(STAR_PREFIX),
            name,
            base_kv: b.base_kv,
            area: b.area,
            kind: kind_label(b.kind),
            // What the load draws at the file's voltage.
            pd: b.load().0,
            qd: b.load().1,
            gs: b.gs,
            bs: b.bs,
        })
        .collect();
    let generators = case
        .generators
        .iter()
        .enumerate()
        .map(|(index, g)| NetworkGenerator {
            index,
            element: power
                .elements
                .generators
                .get(index)
                .map(|e| e.element.clone())
                .unwrap_or_default(),
            bus: g.bus,
            online: g.online,
            pmax: g.pmax,
        })
        .collect();
    let branches = case
        .branches
        .iter()
        .enumerate()
        .map(|(index, br)| {
            let tap = if br.tap == 0.0 { 1.0 } else { br.tap };
            let (kv_from, kv_to) = (base_kv(br.from), base_kv(br.to));
            // A branch is drawn as a transformer when it changes voltage level,
            // has an off-nominal ratio, or shifts phase.
            let switching = switches.get(&index).map(|d| NetworkSwitch {
                kind: match d.kind {
                    SwitchType::Breaker => "breaker",
                    SwitchType::Disconnector => "disconnector",
                    SwitchType::Generic => "connector",
                },
                name: d.name.clone(),
                normally_open: d.normally_open,
            });
            let transformer = switching.is_none()
                && ((kv_from - kv_to).abs() > 1e-6 || tap != 1.0 || br.shift != 0.0);
            let e = power.elements.branches.get(index);
            NetworkBranch {
                index,
                element: e.map(|e| e.element.clone()).unwrap_or_default(),
                circuit: e.map(|e| e.circuit.clone()).unwrap_or_default(),
                winding: e.map_or(0, |e| e.winding),
                from: br.from,
                to: br.to,
                online: br.online,
                transformer,
                rate_a: br.rate_a,
                tap,
                shift: br.shift,
                switching,
            }
        })
        .collect();
    let dc_lines = case
        .dcline
        .iter()
        .flatten()
        .enumerate()
        .map(|(index, row)| {
            let col = |i: usize| row.get(i).copied().unwrap_or(0.0);
            let element = power
                .elements
                .dc_lines
                .get(index)
                .cloned()
                .unwrap_or_default();
            NetworkDcLine {
                index,
                name: element.rsplit(':').next().unwrap_or_default().to_string(),
                kind: power
                    .dc_line_kinds
                    .get(index)
                    .copied()
                    .unwrap_or("two_terminal"),
                element,
                from: col(0) as u32,
                to: col(1) as u32,
                online: col(2) != 0.0,
                mw: col(3),
            }
        })
        .collect();
    CaseNetwork {
        base_mva: case.base_mva,
        buses,
        generators,
        branches,
        dc_lines,
    }
}
