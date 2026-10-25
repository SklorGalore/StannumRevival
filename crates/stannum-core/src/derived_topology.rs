//! A topology case built from the power flow model, for files nodal can't
//! read itself (MATPOWER, and RAW revisions other than 35).
//!
//! It holds what rs-power keeps: buses, branches (a three-winding transformer
//! becomes one piece of equipment again), generators and per-bus load. It has
//! no switching devices or original RAW records, and loads are summed per bus.

use crate::diagram::{self, CaseNetwork};
use crate::power::PowerCase;
use nodal::model::{Bus, Case, Diagnostic, Equipment, EquipmentKind, Injection, SourceRecord};
use std::collections::BTreeMap;

/// RAW's default zero-impedance threshold (THRSHZ), in per unit.
const ZERO_IMPEDANCE_THRESHOLD: f64 = 0.0001;

/// Shown in Topology's diagnostics so the user knows where the data came from.
pub const DERIVED_NOTE: &str = "Built from the power flow model: switching devices and original RAW records aren't available, and loads are summed per bus.";

fn record(section: &str, fields: &[(&str, String)]) -> SourceRecord {
    SourceRecord {
        line: 0,
        section: section.to_string(),
        fields: fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
        raw_lines: vec![],
    }
}

fn bus_type(kind: &str) -> i32 {
    match kind {
        "PV" => 2,
        "Ref" => 3,
        "Isolated" => 4,
        _ => 1,
    }
}

pub fn from_power(power: &PowerCase) -> Case {
    let net: CaseNetwork = diagram::network(power);
    let case = &power.case;
    let star: std::collections::HashSet<u32> =
        net.buses.iter().filter(|b| b.star).map(|b| b.id).collect();
    let vm: BTreeMap<u32, (f64, f64)> = case.buses.iter().map(|b| (b.id, (b.vm, b.va))).collect();

    let buses = net
        .buses
        .iter()
        .filter(|b| !b.star)
        .map(|b| {
            let (v, a) = vm.get(&b.id).copied().unwrap_or((1.0, 0.0));
            Bus {
                number: b.id as i32,
                name: b.name.clone(),
                base_kv: b.base_kv,
                bus_type: bus_type(b.kind),
                area: Some(b.area as i32),
                zone: None,
                voltage_pu: Some(v),
                source: record(
                    "BUS",
                    &[
                        ("NAME", b.name.clone()),
                        ("BASKV", b.base_kv.to_string()),
                        ("IDE", bus_type(b.kind).to_string()),
                        ("AREA", b.area.to_string()),
                        ("VM", v.to_string()),
                        ("VA", a.to_string()),
                    ],
                ),
            }
        })
        .collect();

    // One piece of equipment per element id: the legs of a three-winding
    // transformer share an id and meet at an internal star bus.
    let mut order: Vec<String> = vec![];
    let mut legs: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, br) in net.branches.iter().enumerate() {
        let id = if br.element.is_empty() {
            format!("branch:{}", br.index)
        } else {
            br.element.clone()
        };
        if !legs.contains_key(&id) {
            order.push(id.clone());
        }
        legs.entry(id).or_default().push(i);
    }
    let equipment = order
        .into_iter()
        .map(|id| {
            let rows = &legs[&id];
            let first = &net.branches[rows[0]];
            let data = &case.branches[first.index];
            let mut buses: Vec<i32> = vec![];
            let mut active: Vec<bool> = vec![];
            for &i in rows {
                let br = &net.branches[i];
                for end in [br.from, br.to] {
                    if star.contains(&end) || buses.contains(&(end as i32)) {
                        continue;
                    }
                    buses.push(end as i32);
                    active.push(br.online);
                }
            }
            let online = rows.iter().any(|&i| net.branches[i].online);
            let transformer = id.starts_with("transformer:") || first.transformer;
            Equipment {
                kind: if transformer {
                    EquipmentKind::Transformer
                } else {
                    EquipmentKind::Line
                },
                name: String::new(),
                circuit: first.circuit.clone(),
                buses,
                status: online as i32,
                active_terminals: active,
                source: record(
                    if transformer { "TRANSFORMER" } else { "BRANCH" },
                    &[
                        ("R", data.r.to_string()),
                        ("X", data.x.to_string()),
                        ("B", data.b.to_string()),
                        ("RATEA", data.rate_a.to_string()),
                        ("TAP", first.tap.to_string()),
                        ("SHIFT", first.shift.to_string()),
                    ],
                ),
                id,
            }
        })
        .collect();

    let loads = case
        .buses
        .iter()
        .filter(|b| !star.contains(&b.id) && (b.pd != 0.0 || b.qd != 0.0))
        .map(|b| Injection {
            id: format!("load:{}:1", b.id),
            bus: b.id as i32,
            status: 1,
            p_mw: b.pd,
            q_mvar: b.qd,
            source: record(
                "LOAD",
                &[("PL", b.pd.to_string()), ("QL", b.qd.to_string())],
            ),
        })
        .collect();

    let generators = case
        .generators
        .iter()
        .zip(&net.generators)
        .map(|(g, n)| Injection {
            id: n.element.clone(),
            bus: g.bus as i32,
            status: g.online as i32,
            p_mw: g.pg,
            q_mvar: g.qg,
            source: record(
                "GENERATOR",
                &[
                    ("PG", g.pg.to_string()),
                    ("QG", g.qg.to_string()),
                    ("PMAX", g.pmax.to_string()),
                ],
            ),
        })
        .collect();

    Case {
        revision: 0,
        base_mva: case.base_mva,
        frequency_hz: 60.0,
        zero_impedance_threshold: ZERO_IMPEDANCE_THRESHOLD,
        titles: vec![],
        encoding: "UTF-8".into(),
        buses,
        equipment,
        loads,
        generators,
        diagnostics: vec![Diagnostic {
            severity: "info".into(),
            section: "CASE".into(),
            line: 0,
            message: DERIVED_NOTE.into(),
        }],
        skipped_sections: BTreeMap::new(),
    }
}
