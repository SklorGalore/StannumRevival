//! One element of the open case, as the element inspector shows it: what it
//! is, what the file says about it, and its limits. Results come from the
//! latest solve and study edits from the study, both of which the frontend
//! already holds, so they are not repeated here.

use crate::power::{PowerCase, bus_names, kind_label};
use serde::Serialize;

/// A number, or text for values that aren't numbers (a bus type, a missing limit).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Number(f64),
    Text(String),
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Field {
    pub label: String,
    pub value: Value,
    /// "MW", "p.u.", "kV", ... or empty.
    pub unit: &'static str,
}

/// A titled list of fields, such as one winding of a transformer.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Group {
    /// Empty when the element only has one group.
    pub title: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BusRef {
    pub number: u32,
    pub name: String,
    pub base_kv: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ElementDetail {
    pub element: String,
    /// "bus", "line", "transformer", "generator", "load" or "dcline".
    pub kind: String,
    /// The terminals: the bus itself, or the buses a branch or DC line joins.
    pub buses: Vec<BusRef>,
    /// In service in the file (before any study edits).
    pub online: bool,
    /// Parameters from the file.
    pub data: Vec<Group>,
    /// Ratings and limits from the file.
    pub limits: Vec<Field>,
    /// Elements connected at a bus: its branches, generators and DC lines.
    pub connected: Vec<String>,
    /// A caveat about how the values were read, if any.
    pub note: Option<String>,
}

fn num(label: &str, v: f64, unit: &'static str) -> Field {
    Field {
        label: label.to_string(),
        // JSON has no infinity: an unlimited value reads as text.
        value: if v.is_finite() {
            Value::Number(v)
        } else {
            Value::Text("none".to_string())
        },
        unit,
    }
}

fn text(label: &str, v: impl Into<String>) -> Field {
    Field {
        label: label.to_string(),
        value: Value::Text(v.into()),
        unit: "",
    }
}

/// A rating, where 0 means "not rated" in both MATPOWER and RAW.
fn rating(label: &str, v: f64) -> Field {
    if v > 0.0 {
        num(label, v, "MVA")
    } else {
        text(label, "not rated")
    }
}

/// Describe one element by its shared id, from the case as filed.
pub fn element(
    power: &PowerCase,
    topology: Option<&nodal::model::Case>,
    id: &str,
) -> Result<ElementDetail, String> {
    let kind = id.split(':').next().unwrap_or_default();
    let case = &power.case;
    let names = bus_names(case);
    let bus_ref = |number: u32| -> BusRef {
        let i = case.buses.iter().position(|b| b.id == number);
        BusRef {
            number,
            name: i.and_then(|i| names.get(i)).cloned().unwrap_or_default(),
            base_kv: i.map_or(0.0, |i| case.buses[i].base_kv),
        }
    };
    let missing = || format!("{id} isn't in this case.");
    let mut detail = ElementDetail {
        element: id.to_string(),
        kind: kind.to_string(),
        buses: vec![],
        online: true,
        data: vec![],
        limits: vec![],
        connected: vec![],
        note: None,
    };
    match kind {
        "bus" => {
            let number: u32 = id[4..].parse().map_err(|_| missing())?;
            let b = case
                .buses
                .iter()
                .find(|b| b.id == number)
                .ok_or_else(missing)?;
            detail.buses = vec![bus_ref(number)];
            detail.online = !matches!(b.kind, rs_power::BusType::Isolated);
            detail.data = vec![Group {
                title: String::new(),
                fields: vec![
                    text("Type", kind_label(b.kind)),
                    num("Base voltage", b.base_kv, "kV"),
                    num("Area", b.area as f64, ""),
                    num("Zone", b.zone as f64, ""),
                    num("Load", b.pd, "MW"),
                    num("Load", b.qd, "Mvar"),
                    num("Shunt conductance", b.gs, "MW"),
                    num("Shunt susceptance", b.bs, "Mvar"),
                    num("Voltage in file", b.vm, "p.u."),
                    num("Angle in file", b.va, "°"),
                ],
            }];
            detail.limits = vec![
                num("Minimum voltage", b.vmin, "p.u."),
                num("Maximum voltage", b.vmax, "p.u."),
            ];
            let mut connected: Vec<String> = Vec::new();
            for (row, br) in case.branches.iter().enumerate() {
                if br.from == number || br.to == number {
                    connected.extend(power.elements.branches.get(row).map(|e| e.element.clone()));
                }
            }
            for (row, g) in case.generators.iter().enumerate() {
                if g.bus == number {
                    connected.extend(
                        power
                            .elements
                            .generators
                            .get(row)
                            .map(|e| e.element.clone()),
                    );
                }
            }
            for (row, r) in case.dcline.iter().flatten().enumerate() {
                let ends = [r.first(), r.get(1)].map(|v| v.copied().unwrap_or(0.0) as u32);
                if ends.contains(&number) {
                    connected.extend(power.elements.dc_lines.get(row).cloned());
                }
            }
            // Loads are separate records only where nodal read the file.
            if let Some(t) = topology {
                connected.extend(
                    t.loads
                        .iter()
                        .filter(|l| l.bus == number as i32)
                        .map(|l| l.id.clone()),
                );
            }
            // Three-winding legs share an id; list each element once.
            let mut seen = std::collections::HashSet::new();
            connected.retain(|e| seen.insert(e.clone()));
            detail.connected = connected;
        }
        "line" | "transformer" => {
            let rows: Vec<usize> = power
                .elements
                .branches
                .iter()
                .enumerate()
                .filter(|(_, e)| e.element == id)
                .map(|(i, _)| i)
                .collect();
            if rows.is_empty() {
                return Err(missing());
            }
            let base = case.base_mva;
            let mut terminals: Vec<u32> = Vec::new();
            for &row in &rows {
                let br = &case.branches[row];
                let winding = power.elements.branches[row].winding;
                // A three-winding leg runs from its winding's bus to the star point.
                for end in [br.from, br.to] {
                    let star = names
                        .get(
                            case.buses
                                .iter()
                                .position(|b| b.id == end)
                                .unwrap_or(usize::MAX),
                        )
                        .is_some_and(|n| n.starts_with("STR_PT_XF_"));
                    if !star && !terminals.contains(&end) {
                        terminals.push(end);
                    }
                }
                let kv = bus_ref(br.from).base_kv;
                let mut fields = vec![
                    num("From bus", br.from as f64, ""),
                    num("To bus", br.to as f64, ""),
                    num("Resistance", br.r, "p.u."),
                    num("Reactance", br.x, "p.u."),
                    num("Charging", br.b, "p.u."),
                ];
                // Ohms help when checking a line against its length.
                if kind == "line" && kv > 0.0 && base > 0.0 {
                    let z = kv * kv / base;
                    fields.push(num("Resistance", br.r * z, "Ω"));
                    fields.push(num("Reactance", br.x * z, "Ω"));
                }
                if kind == "transformer" {
                    fields.push(num(
                        "Ratio",
                        if br.tap == 0.0 { 1.0 } else { br.tap },
                        "p.u.",
                    ));
                    fields.push(num("Phase shift", br.shift, "°"));
                }
                detail.data.push(Group {
                    title: if winding > 0 {
                        format!("Winding {winding}")
                    } else {
                        String::new()
                    },
                    fields,
                });
                if detail.limits.is_empty() {
                    detail.limits = vec![
                        rating("Rate A", br.rate_a),
                        rating("Rate B", br.rate_b),
                        rating("Rate C", br.rate_c),
                    ];
                    // MATPOWER writes ±360 for "no limit".
                    if br.angmin > -360.0 || br.angmax < 360.0 {
                        detail
                            .limits
                            .push(num("Minimum angle difference", br.angmin, "°"));
                        detail
                            .limits
                            .push(num("Maximum angle difference", br.angmax, "°"));
                    }
                }
            }
            detail.online = rows.iter().any(|&r| case.branches[r].online);
            detail.buses = terminals.into_iter().map(bus_ref).collect();
        }
        "generator" => {
            let row = power
                .elements
                .generators
                .iter()
                .position(|e| e.element == id)
                .ok_or_else(missing)?;
            let g = &case.generators[row];
            detail.buses = vec![bus_ref(g.bus)];
            detail.online = g.online;
            let mut fields = vec![
                text("Machine", power.elements.generators[row].machine_id.clone()),
                num("Output", g.pg, "MW"),
                num("Reactive output", g.qg, "Mvar"),
                num("Voltage setpoint", g.vg, "p.u."),
            ];
            if let Some(remote) = g.regulated_bus {
                fields.push(num("Regulated bus", remote as f64, ""));
                fields.push(num("Share of remote Mvar", g.reactive_share, "%"));
            }
            fields.push(num("Machine base", g.mbase, "MVA"));
            detail.data = vec![Group {
                title: String::new(),
                fields,
            }];
            detail.limits = vec![
                num("Minimum output", g.pmin, "MW"),
                num("Maximum output", g.pmax, "MW"),
                num("Minimum reactive", g.qmin, "Mvar"),
                num("Maximum reactive", g.qmax, "Mvar"),
            ];
        }
        "load" => {
            let number: u32 = id
                .split(':')
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or_else(missing)?;
            let b = case
                .buses
                .iter()
                .find(|b| b.id == number)
                .ok_or_else(missing)?;
            detail.buses = vec![bus_ref(number)];
            // nodal keeps each load record; rs-power sums them per bus.
            let record = topology.and_then(|t| t.loads.iter().find(|l| l.id == id));
            if let Some(l) = record {
                detail.online = l.status == 1;
                detail.data = vec![Group {
                    title: String::new(),
                    fields: vec![num("Load", l.p_mw, "MW"), num("Load", l.q_mvar, "Mvar")],
                }];
            } else {
                detail.data = vec![Group {
                    title: String::new(),
                    fields: vec![num("Bus load", b.pd, "MW"), num("Bus load", b.qd, "Mvar")],
                }];
                detail.note = Some(
                    "Power flow sums the loads at a bus, so these are the bus's totals."
                        .to_string(),
                );
            }
        }
        "dcline" => {
            let row = power
                .elements
                .dc_lines
                .iter()
                .position(|e| e == id)
                .ok_or_else(missing)?;
            let r = case
                .dcline
                .as_ref()
                .and_then(|rows| rows.get(row))
                .ok_or_else(missing)?;
            let col = |i: usize| r.get(i).copied().unwrap_or(0.0);
            detail.buses = vec![bus_ref(col(0) as u32), bus_ref(col(1) as u32)];
            detail.online = col(2) != 0.0;
            let kind = match power.dc_line_kinds.get(row).copied() {
                Some("vsc") => "VSC",
                Some("multi_terminal") => "Multi-terminal",
                _ => "Two-terminal",
            };
            detail.data = vec![Group {
                title: String::new(),
                fields: vec![
                    text("Type", kind),
                    num("Scheduled power", col(3), "MW"),
                    num("Sending voltage", col(7), "p.u."),
                    num("Receiving voltage", col(8), "p.u."),
                    num("Fixed loss", col(15), "MW"),
                    num("Loss factor", col(16), ""),
                ],
            }];
            detail.limits = vec![
                num("Minimum power", col(9), "MW"),
                num("Maximum power", col(10), "MW"),
                num("Sending minimum reactive", col(11), "Mvar"),
                num("Sending maximum reactive", col(12), "Mvar"),
                num("Receiving minimum reactive", col(13), "Mvar"),
                num("Receiving maximum reactive", col(14), "Mvar"),
            ];
        }
        "switch" => {
            // Switching devices only exist in nodal's model of the file.
            let e = topology
                .and_then(|t| t.equipment.iter().find(|e| e.id == id))
                .ok_or_else(missing)?;
            detail.buses = e.buses.iter().map(|&b| bus_ref(b as u32)).collect();
            detail.online = e.status == 1;
            let mut fields = vec![text("Circuit", e.circuit.trim())];
            if !e.name.trim().is_empty() {
                fields.push(text("Name", e.name.trim()));
            }
            detail.data = vec![Group {
                title: String::new(),
                fields,
            }];
            detail.note = Some(
                "Switching devices come from the topology model; power flow doesn't report them."
                    .to_string(),
            );
        }
        _ => return Err(format!("{id} isn't an element id.")),
    }
    Ok(detail)
}
