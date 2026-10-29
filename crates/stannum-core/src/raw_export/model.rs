//! The power flow model as RAW 35 records, for cases whose own records
//! can't be read one by one (MATPOWER, RAW revisions other than 33 and 35).
//!
//! Written: buses, loads (one per bus), fixed shunts, generators, lines,
//! two-winding transformers (on the system base, ratio in per unit of the
//! bus base), switched shunts, tap changers, reactive flow taps, phase
//! shifters and DC line transformers (COD 4) in control,
//! two-terminal and VSC DC lines with their full data, FACTS devices, areas
//! with their interchange targets, zones and one owner. Multi-terminal DC
//! and anything else the model doesn't hold are left out, with a note.

use super::{Doc, Fields, Item, Notes, Rec, int, num, text};
use crate::power::PowerCase;
use rs_power::BusType;
use std::collections::{BTreeMap, HashMap, HashSet};

/// RAW text fields hold Windows-1252; others become `?`.
fn name(s: &str, width: usize) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            let mut b = [0u8; 4];
            let (_, _, bad) = encoding_rs::WINDOWS_1252.encode(c.encode_utf8(&mut b));
            if bad || c.is_control() { '?' } else { c }
        })
        .take(width)
        .collect();
    while out.chars().count() < width {
        out.push(' ');
    }
    out
}

fn rec(fields: Vec<(&str, super::Value)>) -> Item {
    Item::Rec(Rec::new(
        fields
            .into_iter()
            .map(|(n, v)| (n.to_string(), v))
            .collect::<Fields>(),
    ))
}

fn limit(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        9999.0 * v.signum()
    }
}

/// Ownership fields: owner 1, whole.
fn owned(fields: &mut Vec<(&str, super::Value)>) {
    fields.extend([
        ("O1", int(1)),
        ("F1", num(1.0)),
        ("O2", int(0)),
        ("F2", num(1.0)),
        ("O3", int(0)),
        ("F3", num(1.0)),
        ("O4", int(0)),
        ("F4", num(1.0)),
    ]);
}

pub(crate) fn doc(case_name: &str, power: &PowerCase, notes: &mut Notes) -> Doc {
    let case = &power.case;
    let mut sections: HashMap<String, Vec<Item>> = HashMap::new();
    let mut push =
        |section: &str, item: Item| sections.entry(section.to_string()).or_default().push(item);
    let names = case
        .labels
        .get("bus_name")
        .filter(|n| n.len() == case.buses.len());
    let shunts = &power.voltage_controls.switched_shunts;

    for (i, b) in case.buses.iter().enumerate() {
        let ide = match b.kind {
            BusType::Pq => 1,
            BusType::Pv => 2,
            BusType::Reference => 3,
            BusType::Isolated => 4,
        };
        let bus_name = names.map_or(String::new(), |n| n[i].trim().to_string());
        push(
            "BUS",
            rec(vec![
                ("I", int(i64::from(b.id))),
                ("NAME", text(&name(&bus_name, 12))),
                ("BASKV", num(b.base_kv)),
                ("IDE", int(ide)),
                ("AREA", int(i64::from(b.area))),
                ("ZONE", int(i64::from(b.zone))),
                ("OWNER", int(1)),
                ("VM", num(b.vm)),
                ("VA", num(b.va)),
                ("NVHI", num(b.vmax)),
                ("NVLO", num(b.vmin)),
                ("EVHI", num(b.vmax)),
                ("EVLO", num(b.vmin)),
            ]),
        );
        if [b.pd, b.qd, b.pd_i, b.qd_i, b.pd_z, b.qd_z]
            .iter()
            .any(|v| *v != 0.0)
        {
            push(
                "LOAD",
                rec(vec![
                    ("I", int(i64::from(b.id))),
                    ("ID", text("1 ")),
                    ("STAT", int(1)),
                    ("AREA", int(i64::from(b.area))),
                    ("ZONE", int(i64::from(b.zone))),
                    ("PL", num(b.pd)),
                    ("QL", num(b.qd)),
                    ("IP", num(b.pd_i)),
                    ("IQ", num(b.qd_i)),
                    ("YP", num(b.pd_z)),
                    // `qd_z` is consumption; PSS/E YQ is positive for capacitive load.
                    ("YQ", num(-b.qd_z)),
                    ("OWNER", int(1)),
                    ("SCALE", int(1)),
                    ("INTRPT", int(0)),
                    ("DGENP", num(0.0)),
                    ("DGENQ", num(0.0)),
                    ("DGENM", int(0)),
                    ("LOADTYPE", text("")),
                ]),
            );
        }
        // Switched shunts' starting susceptance is part of the bus shunt.
        let switched: f64 = shunts
            .iter()
            .filter(|s| s.bus == b.id)
            .map(|s| s.initial)
            .sum();
        let bs = b.bs - switched;
        if b.gs != 0.0 || bs.abs() > 1e-9 {
            push(
                "FIXED SHUNT",
                rec(vec![
                    ("I", int(i64::from(b.id))),
                    ("ID", text("1 ")),
                    ("STATUS", int(1)),
                    ("GL", num(b.gs)),
                    ("BL", num(bs)),
                ]),
            );
        }
    }

    // Generators, with the machine ids the case already uses.
    let mut used: HashSet<(u32, String)> = HashSet::new();
    for (k, g) in case.generators.iter().enumerate() {
        let wanted = power
            .elements
            .generators
            .get(k)
            .map_or("1".to_string(), |e| e.machine_id.clone());
        let id = std::iter::once(wanted)
            .chain((1..).map(|n| n.to_string()))
            .find(|id| !used.contains(&(g.bus, id.clone())))
            .unwrap();
        used.insert((g.bus, id.clone()));
        let mut fields = vec![
            ("I", int(i64::from(g.bus))),
            ("ID", text(&name(&id, 2))),
            ("PG", num(g.pg)),
            ("QG", num(g.qg)),
            ("QT", num(limit(g.qmax))),
            ("QB", num(limit(g.qmin))),
            ("VS", num(g.vg)),
            ("IREG", int(i64::from(g.regulated_bus.unwrap_or(0)))),
            ("NREG", int(0)),
            (
                "MBASE",
                num(if g.mbase > 0.0 {
                    g.mbase
                } else {
                    case.base_mva
                }),
            ),
            ("ZR", num(0.0)),
            ("ZX", num(1.0)),
            ("RT", num(0.0)),
            ("XT", num(0.0)),
            ("GTAP", num(1.0)),
            ("STAT", int(i64::from(g.online))),
            ("RMPCT", num(g.reactive_share)),
            ("PT", num(limit(g.pmax))),
            ("PB", num(limit(g.pmin))),
            ("BASLOD", int(0)),
        ];
        owned(&mut fields);
        fields.extend([("WMOD", int(0)), ("WPF", num(1.0))]);
        push("GENERATOR", rec(fields));
    }

    // Lines and transformers, with circuits unique per bus pair.
    let taps: HashMap<usize, &rs_power::TapControl> = power
        .voltage_controls
        .taps
        .iter()
        .map(|t| (t.branch, t))
        .collect();
    let shifters: HashMap<usize, &rs_power::PhaseShifter> = power
        .flow_controls
        .phase_shifters
        .iter()
        .map(|p| (p.branch, p))
        .collect();
    let flow_taps: HashMap<usize, &rs_power::ReactiveTap> = power
        .flow_controls
        .reactive_taps
        .iter()
        .map(|t| (t.branch, t))
        .collect();
    let dc_line_taps: HashMap<usize, &rs_power::DcLineTap> = power
        .flow_controls
        .dc_line_taps
        .iter()
        .map(|t| (t.branch, t))
        .collect();
    let mut circuits: HashSet<(u32, u32, String)> = HashSet::new();
    let mut three_winding = 0;
    let mut charging_ids: HashMap<u32, u32> = HashMap::new();
    for (k, br) in case.branches.iter().enumerate() {
        let element = power.elements.branches.get(k);
        // Older RAW revisions give lines an off-nominal ratio too.
        let transformer = (br.tap != 0.0 && br.tap != 1.0)
            || br.shift != 0.0
            || element.is_some_and(|e| e.element.starts_with("transformer:"));
        if element.is_some_and(|e| e.winding > 0) {
            three_winding += 1;
        }
        let pair = (br.from.min(br.to), br.from.max(br.to));
        let wanted = element.map_or("1".to_string(), |e| e.circuit.clone());
        let ckt = std::iter::once(wanted)
            .chain((1..).map(|n| n.to_string()))
            .find(|c| !circuits.contains(&(pair.0, pair.1, c.to_uppercase())))
            .unwrap();
        circuits.insert((pair.0, pair.1, ckt.to_uppercase()));
        let ckt = name(&ckt, 2);
        let rates = [br.rate_a, br.rate_b, br.rate_c];
        if !transformer {
            let mut fields = vec![
                ("I", int(i64::from(br.from))),
                ("J", int(i64::from(br.to))),
                ("CKT", text(&ckt)),
                ("R", num(br.r)),
                ("X", num(br.x)),
                ("B", num(br.b)),
                ("NAME", text("")),
            ];
            let names: Vec<&str> = vec![
                "RATE1", "RATE2", "RATE3", "RATE4", "RATE5", "RATE6", "RATE7", "RATE8", "RATE9",
                "RATE10", "RATE11", "RATE12",
            ];
            for (i, n) in names.into_iter().enumerate() {
                fields.push((n, num(rates.get(i).copied().unwrap_or(0.0))));
            }
            fields.extend([
                ("GI", num(0.0)),
                ("BI", num(0.0)),
                ("GJ", num(0.0)),
                ("BJ", num(0.0)),
                ("STAT", int(i64::from(br.online))),
                ("MET", int(1)),
                ("LEN", num(0.0)),
            ]);
            owned(&mut fields);
            push("BRANCH", rec(fields));
            continue;
        }
        let ratio = if br.tap == 0.0 { 1.0 } else { br.tap };
        if br.b != 0.0 && br.online {
            // The model's charging sits beside the series impedance, half at
            // each end, the from end seen through the ratio: the same as
            // these two bus shunts.
            for (bus, b) in [(br.from, br.b / 2.0 / (ratio * ratio)), (br.to, br.b / 2.0)] {
                let n = charging_ids.entry(bus).or_insert(0);
                *n += 1;
                push(
                    "FIXED SHUNT",
                    rec(vec![
                        ("I", int(i64::from(bus))),
                        ("ID", text(&name(&format!("T{n}"), 2))),
                        ("STATUS", int(1)),
                        ("GL", num(0.0)),
                        ("BL", num(b * case.base_mva)),
                    ]),
                );
            }
            notes.add("Line charging of transformers written as fixed shunts at their buses: RAW transformers have no line charging");
        }
        let mut fields = vec![
            ("I", int(i64::from(br.from))),
            ("J", int(i64::from(br.to))),
            ("K", int(0)),
            ("CKT", text(&ckt)),
            ("CW", int(1)),
            ("CZ", int(1)),
            ("CM", int(1)),
            ("MAG1", num(0.0)),
            ("MAG2", num(0.0)),
            ("NMETR", int(2)),
            ("NAME", text("")),
            ("STAT", int(i64::from(br.online))),
        ];
        owned(&mut fields);
        fields.extend([
            ("VECGRP", text("")),
            ("R1-2", num(br.r)),
            ("X1-2", num(br.x)),
            ("SBASE1-2", num(case.base_mva)),
            ("WINDV1", num(ratio)),
            ("NOMV1", num(0.0)),
            ("ANG1", num(br.shift)),
        ]);
        let rate_names = [
            "RATE1-1", "RATE1-2", "RATE1-3", "RATE1-4", "RATE1-5", "RATE1-6", "RATE1-7", "RATE1-8",
            "RATE1-9", "RATE1-10", "RATE1-11", "RATE1-12",
        ];
        for (i, n) in rate_names.into_iter().enumerate() {
            fields.push((n, num(rates.get(i).copied().unwrap_or(0.0))));
        }
        let (cod, cont, rma, rmi, vma, vmi, ntp) = match taps.get(&k) {
            Some(t) => {
                // A negative CONT is a bus on the winding's own side that
                // isn't one of its terminals.
                let terminal = t.regulated_bus == br.from || t.regulated_bus == br.to;
                let cont = i64::from(t.regulated_bus);
                let cont = if !terminal && t.raises_voltage {
                    -cont
                } else {
                    cont
                };
                (
                    1,
                    cont,
                    t.ratio_max * t.tap_per_ratio,
                    t.ratio_min * t.tap_per_ratio,
                    t.vmax,
                    t.vmin,
                    i64::from(t.positions),
                )
            }
            // A phase shifter: RMA/RMI are angles, VMA/VMI the MW band.
            None => match shifters.get(&k) {
                Some(ps) => (
                    if ps.asymmetric.is_some() { 5 } else { 3 },
                    0,
                    ps.angle_max,
                    ps.angle_min,
                    ps.mw_max,
                    ps.mw_min,
                    33,
                ),
                // A reactive flow tap: VMA/VMI are the Mvar band.
                None => match (flow_taps.get(&k), dc_line_taps.get(&k)) {
                    (Some(t), _) => (
                        2,
                        0,
                        t.ratio_max * t.tap_per_ratio,
                        t.ratio_min * t.tap_per_ratio,
                        t.mvar_max,
                        t.mvar_min,
                        i64::from(t.positions),
                    ),
                    // Moved by the DC line converter that names it, within
                    // the converter's limits.
                    (None, Some(t)) => (if t.enabled { 4 } else { -4 }, 0, 1.1, 0.9, 1.1, 0.9, 33),
                    (None, None) => (0, 0, 1.1, 0.9, 1.1, 0.9, 33),
                },
            },
        };
        let cnxa = shifters
            .get(&k)
            .and_then(|ps| ps.asymmetric)
            .map_or(0.0, |a| a.connection_angle);
        fields.extend([
            ("COD1", int(cod)),
            ("CONT1", int(cont)),
            ("NOD1", int(0)),
            ("RMA1", num(rma)),
            ("RMI1", num(rmi)),
            ("VMA1", num(vma)),
            ("VMI1", num(vmi)),
            ("NTP1", int(ntp)),
            ("TAB1", int(0)),
            ("CR1", num(0.0)),
            ("CX1", num(0.0)),
            ("CNXA1", num(cnxa)),
            ("WINDV2", num(1.0)),
            ("NOMV2", num(0.0)),
        ]);
        push("TRANSFORMER", rec(fields));
    }
    if three_winding > 0 {
        notes.add(format!(
            "Three-winding transformers written as {three_winding} two-winding transformers to their star buses"
        ));
    }

    // DC lines whose full data the model holds, and FACTS devices.
    let vsc_names = dc_lines(power, &mut push, notes);
    let facts_names = facts(power, &mut push, notes);

    // Switched shunts in voltage or reactive control, numbered per bus.
    let mut shunt_ids: HashMap<u32, usize> = HashMap::new();
    let ids: Vec<String> = shunts
        .iter()
        .map(|s| {
            let n = shunt_ids.entry(s.bus).or_default();
            *n += 1;
            name(&n.to_string(), 2)
        })
        .collect();
    let mut frozen_converter_shunts = 0;
    for (s, id) in shunts.iter().zip(&ids) {
        // MODSW, the bus it watches (SWREG) and the device there (RMIDNT).
        let (modsw, swreg, rmidnt) = match s.mode {
            rs_power::ShuntMode::Discrete => (1, s.regulated_bus.unwrap_or(0), String::new()),
            rs_power::ShuntMode::Continuous => (2, s.regulated_bus.unwrap_or(0), String::new()),
            rs_power::ShuntMode::PlantReactive { bus } => (3, bus, String::new()),
            // The converter's VSC line is written only with its full data;
            // without it the shunt stays at its setting.
            rs_power::ShuntMode::ConverterReactive { row, bus } => match vsc_names.get(&row) {
                Some(line) => (4, bus, line.clone()),
                None => {
                    frozen_converter_shunts += 1;
                    (0, 0, String::new())
                }
            },
            rs_power::ShuntMode::FactsReactive { device } => (
                6,
                power
                    .voltage_controls
                    .facts
                    .get(device)
                    .map_or(0, |d| d.bus),
                facts_names.get(device).cloned().unwrap_or_default(),
            ),
            rs_power::ShuntMode::ShuntSetting { shunt } => (
                5,
                shunts.get(shunt).map_or(0, |o| o.bus),
                ids.get(shunt).cloned().unwrap_or_default(),
            ),
        };
        let mut fields = vec![
            ("I", int(i64::from(s.bus))),
            ("ID", text(id)),
            ("MODSW", int(modsw)),
            ("ADJM", int(0)),
            ("STAT", int(1)),
            ("VSWHI", num(s.vmax)),
            ("VSWLO", num(s.vmin)),
            ("SWREG", int(i64::from(swreg))),
            ("NREG", int(0)),
            ("RMPCT", num(100.0)),
            ("RMIDNT", text(&rmidnt)),
            ("BINIT", num(s.initial)),
        ];
        let blocks: Vec<(String, String, String)> = (1..=8)
            .map(|g| (format!("S{g}"), format!("N{g}"), format!("B{g}")))
            .collect();
        let mut rec_fields: Fields = fields.drain(..).map(|(n, v)| (n.to_string(), v)).collect();
        for ((st, nn, bb), (steps, b)) in blocks.iter().zip(&s.blocks) {
            rec_fields.push((st.clone(), int(1)));
            rec_fields.push((nn.clone(), int(i64::from(*steps))));
            rec_fields.push((bb.clone(), num(*b)));
        }
        push("SWITCHED SHUNT", Item::Rec(Rec::new(rec_fields)));
    }
    if frozen_converter_shunts > 0 {
        notes.add(format!(
            "{frozen_converter_shunts} switched shunts watching a VSC converter (MODSW 4) written as fixed (MODSW 0): their VSC DC lines aren't in the model in full"
        ));
    }

    // Areas, zones and one owner.
    let mut areas: BTreeMap<u32, u32> = BTreeMap::new();
    for b in &case.buses {
        let swing = areas.entry(b.area).or_insert(0);
        if *swing == 0 && b.kind == BusType::Reference {
            *swing = b.id;
        }
    }
    // Control areas from the file keep their slack bus, target and name.
    let listed: HashMap<u32, &rs_power::Area> =
        power.areas.iter().rev().map(|a| (a.number, a)).collect();
    for a in &power.areas {
        areas.entry(a.number).or_insert(0);
    }
    for (area, swing) in &areas {
        let (swing, pdes, ptol, arname) = match listed.get(area) {
            Some(a) => (
                a.slack_bus.unwrap_or(0),
                a.desired_mw,
                a.tolerance_mw,
                a.name.as_str(),
            ),
            None => (*swing, 0.0, 10.0, ""),
        };
        push(
            "AREA",
            rec(vec![
                ("I", int(i64::from(*area))),
                ("ISW", int(i64::from(swing))),
                ("PDES", num(pdes)),
                ("PTOL", num(ptol)),
                ("ARNAME", text(&name(arname, 12))),
            ]),
        );
    }
    if !power.area_loads.is_empty() {
        notes.add(format!(
            "{} loads assigned to another area than their bus's written in their bus's area: the model keeps one load per bus",
            power.area_loads.len()
        ));
    }
    let zones: std::collections::BTreeSet<u32> = case.buses.iter().map(|b| b.zone).collect();
    for zone in zones {
        push(
            "ZONE",
            rec(vec![
                ("I", int(i64::from(zone))),
                ("ZONAME", text(&name("", 12))),
            ]),
        );
    }
    push(
        "OWNER",
        rec(vec![("I", int(1)), ("OWNAME", text(&name("1", 12)))]),
    );

    if case
        .generators
        .iter()
        .any(|g| !g.extra.iter().all(|v| *v == 0.0))
        || case.gencost.is_some()
    {
        notes.add("Generator costs and MATPOWER's extra generator columns left out: RAW has no place for them");
    }

    let source = match power.revision {
        Some(rev) => format!("PSS/E RAW revision {rev}"),
        None => "MATPOWER".to_string(),
    };
    let title = |s: String| s.chars().take(60).collect::<String>();
    Doc {
        revision: 35,
        ident: vec![
            ("IC".into(), int(0)),
            ("SBASE".into(), num(case.base_mva)),
            ("REV".into(), int(35)),
            ("XFRRAT".into(), int(0)),
            ("NXFRAT".into(), int(0)),
            ("BASFRQ".into(), num(60.0)),
        ],
        titles: [
            title(name(case_name, case_name.chars().count())),
            title(format!(
                "Written by Stannum from the {source} power flow model"
            )),
        ],
        sections,
        tail: vec![],
    }
}

/// One record of several lines, written as given: the VSC and FACTS
/// sections have no layout in revision 33, so they are copied as lines
/// between revisions, like the records read from a file.
fn lines_rec(lines: Vec<Vec<super::Value>>) -> Item {
    let lines = lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|v| match v {
                    super::Value::Text(_) => super::token(v, "string"),
                    super::Value::Number(_) => super::token(v, "number"),
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    Item::Rec(Rec {
        fields: Fields::new(),
        lines: Some(lines),
    })
}

/// A name for each device that is unique, not empty and fits `width`,
/// keeping the case's own where it can.
fn unique_names<'a>(
    wanted: impl Iterator<Item = &'a str>,
    prefix: &str,
    width: usize,
) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::new();
    wanted
        .enumerate()
        .map(|(k, w)| {
            let w: String = name(w.trim(), width).trim_end().to_string();
            let n = std::iter::once(w)
                .chain((k + 1..).map(|n| format!("{prefix}{n}")))
                .find(|n| !n.is_empty() && !used.contains(&n.to_uppercase()))
                .unwrap();
            used.insert(n.to_uppercase());
            n
        })
        .collect()
}

/// Two-terminal and VSC DC lines with their full data. Returns the name
/// written for each VSC line, by its `case.dcline` row.
fn dc_lines(
    power: &PowerCase,
    push: &mut impl FnMut(&str, Item),
    notes: &mut Notes,
) -> HashMap<usize, String> {
    let lcc_names = unique_names(power.lcc_lines.iter().map(|l| l.name.as_str()), "DC", 12);
    for (l, line_name) in power.lcc_lines.iter().zip(&lcc_names) {
        let converter = |c: &rs_power::LccConverter| {
            let (from, to, ckt) = c.transformer.clone().unwrap_or((0, 0, "1".to_string()));
            vec![
                int(i64::from(c.bus)),
                int(i64::from(c.bridges)),
                num(c.angle_max),
                num(c.angle_min),
                num(c.resistance),
                num(c.reactance),
                num(c.base_kv),
                num(c.ratio),
                num(c.tap),
                num(c.tap_max),
                num(c.tap_min),
                num(c.tap_step),
                int(i64::from(c.commutating_bus)),
                int(0),
                int(i64::from(from)),
                int(i64::from(to)),
                text(&name(&ckt, 2)),
                num(c.capacitor),
            ]
        };
        let names = [
            "IPR", "NBR", "ANMXR", "ANMNR", "RCR", "XCR", "EBASR", "TRR", "TAPR", "TMXR", "TMNR",
            "STPR", "ICR", "NDR", "IFR", "ITR", "IDR", "XCAPR",
        ];
        let mut fields: Fields = [
            ("NAME", text(line_name)),
            (
                "MDC",
                int(match l.control {
                    rs_power::LccControl::Blocked => 0,
                    rs_power::LccControl::Power => 1,
                    rs_power::LccControl::Current => 2,
                }),
            ),
            ("RDC", num(l.resistance)),
            ("SETVL", num(l.setpoint)),
            ("VSCHD", num(l.scheduled_kv)),
            ("VCMOD", num(l.mode_switch_kv)),
            ("RCOMP", num(l.compounding_resistance)),
            ("DELTI", num(l.current_margin)),
            (
                "METER",
                text(match l.metered_end {
                    rs_power::LccEnd::Rectifier => "R",
                    rs_power::LccEnd::Inverter => "I",
                }),
            ),
            ("DCVMIN", num(l.min_compounded_kv)),
            ("CCCITMX", int(i64::from(l.ccc_iterations))),
            ("CCCACC", num(l.ccc_acceleration)),
        ]
        .into_iter()
        .map(|(n, v)| (n.to_string(), v))
        .collect();
        for (end, c) in [("R", &l.rectifier), ("I", &l.inverter)] {
            for (n, v) in names.iter().zip(converter(c)) {
                fields.push((format!("{}{end}", &n[..n.len() - 1]), v));
            }
        }
        push("TWO-TERMINAL DC", Item::Rec(Rec::new(fields)));
    }

    let vsc_names = unique_names(power.vsc_lines.iter().map(|l| l.name.as_str()), "VSC", 12);
    let mut by_row = HashMap::new();
    for (l, line_name) in power.vsc_lines.iter().zip(&vsc_names) {
        let mut lines = vec![vec![
            text(line_name),
            int(i64::from(l.in_service)),
            num(l.resistance),
            int(1),
            num(1.0),
            int(0),
            num(1.0),
            int(0),
            num(1.0),
            int(0),
            num(1.0),
        ]];
        for c in &l.converters {
            lines.push(vec![
                int(i64::from(c.bus)),
                int(match c.dc_control {
                    rs_power::VscDcControl::OutOfService => 0,
                    rs_power::VscDcControl::DcVoltage => 1,
                    rs_power::VscDcControl::Power => 2,
                }),
                int(match c.ac_control {
                    rs_power::VscAcControl::Voltage => 1,
                    rs_power::VscAcControl::PowerFactor => 2,
                }),
                num(c.dc_setpoint),
                num(c.ac_setpoint),
                num(c.loss_kw),
                num(c.loss_kw_per_amp),
                num(c.min_loss_kw),
                num(c.smax_mva),
                num(c.imax_amps),
                num(c.power_weighting),
                num(limit(c.qmax)),
                num(limit(c.qmin)),
                int(i64::from(c.remote_bus)),
                num(c.remote_percent),
            ]);
        }
        push("VSC DC LINE", lines_rec(lines));
        by_row.insert(l.row, line_name.clone());
    }

    if !power.mtdc.is_empty() {
        let n = power.mtdc.len();
        notes.add(format!(
            "{n} multi-terminal DC system{} left out: Stannum doesn't write them from the power flow model",
            if n == 1 { "" } else { "s" }
        ));
    }
    // Rows without full data: MATPOWER's, and VSC lines the reader keeps
    // only as a lossless row.
    let rows = power.case.dcline.as_ref().map_or(0, Vec::len);
    let multi = power
        .dc_line_kinds
        .iter()
        .filter(|k| **k == "multi_terminal")
        .count();
    let left = rows.saturating_sub(power.lcc_lines.len() + power.vsc_lines.len() + multi);
    if left > 0 {
        notes.add(format!(
            "{left} DC line{} left out: the model holds only a lossless approximation of {}",
            if left == 1 { "" } else { "s" },
            if left == 1 { "it" } else { "them" }
        ));
    }
    by_row
}

/// FACTS devices in service, in normal mode. Returns the name written for
/// each, in device order.
fn facts(power: &PowerCase, push: &mut impl FnMut(&str, Item), notes: &mut Notes) -> Vec<String> {
    let devices = &power.voltage_controls.facts;
    let names = unique_names(devices.iter().map(|d| d.name.as_str()), "FACTS", 12);
    for (d, device_name) in devices.iter().zip(&names) {
        push(
            "FACTS DEVICE",
            lines_rec(vec![vec![
                text(device_name),
                int(i64::from(d.bus)),
                int(i64::from(d.terminal_bus.unwrap_or(0))),
                int(1),
                num(d.p_desired),
                num(d.q_desired),
                num(d.vset),
                num(d.shunt_max),
                num(9999.0),
                num(0.9),
                num(1.1),
                num(1.0),
                num(0.0),
                num(0.05),
                num(d.reactive_share),
                int(1),
                num(0.0),
                num(0.0),
                int(0),
                int(i64::from(d.regulated_bus.unwrap_or(0))),
                text(""),
            ]]),
        );
    }
    if devices.iter().any(|d| d.terminal_bus.is_some()) {
        notes.add("FACTS series elements written without their limits (TRMX, VTMN, VTMX, VSMX, IMX) or impedance (LINX), which the model doesn't hold: PSS/E defaults used");
    }
    names
}
