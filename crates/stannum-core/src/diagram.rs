//! The open case as a network of buses, injections and branches, shaped for
//! drawing a one-line diagram. Positions are not computed here: the frontend
//! lays the network out, so this only says what exists and how it connects.

use crate::power::{PowerCase, bus_names, kind_label};
use serde::Serialize;
use std::collections::HashMap;

/// rs-power names the internal star bus of a three-winding transformer this way.
const STAR_PREFIX: &str = "STR_PT_XF_";

#[derive(Clone, Debug, Serialize)]
pub struct NetworkBus {
    pub id: u32,
    pub name: String,
    pub base_kv: f64,
    pub area: u32,
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

#[derive(Clone, Debug, Serialize)]
pub struct NetworkGenerator {
    /// Index in the case's generator table (matches the power flow report).
    pub index: usize,
    pub bus: u32,
    pub online: bool,
    pub pmax: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkBranch {
    /// Index in the case's branch table (matches the power flow report).
    pub index: usize,
    pub from: u32,
    pub to: u32,
    pub online: bool,
    pub transformer: bool,
    pub rate_a: f64,
    /// Off-nominal turns ratio (1.0 when unity) and phase shift in degrees.
    pub tap: f64,
    pub shift: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseNetwork {
    pub base_mva: f64,
    pub buses: Vec<NetworkBus>,
    pub generators: Vec<NetworkGenerator>,
    pub branches: Vec<NetworkBranch>,
}

pub fn network(power: &PowerCase) -> CaseNetwork {
    let case = &power.case;
    let names = bus_names(case);
    let kv: HashMap<u32, f64> = case.buses.iter().map(|b| (b.id, b.base_kv)).collect();
    let base_kv = |id: u32| kv.get(&id).copied().unwrap_or(0.0);
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
            pd: b.pd,
            qd: b.qd,
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
            let transformer = (kv_from - kv_to).abs() > 1e-6 || tap != 1.0 || br.shift != 0.0;
            NetworkBranch {
                index,
                from: br.from,
                to: br.to,
                online: br.online,
                transformer,
                rate_a: br.rate_a,
                tap,
                shift: br.shift,
            }
        })
        .collect();
    CaseNetwork {
        base_mva: case.base_mva,
        buses,
        generators,
        branches,
    }
}
