//! Transformers between RAW 35 records and Stannum records.
//!
//! RAW gives a transformer's numbers in units its codes choose: CZ for the
//! impedance (system base, its own base, or load loss and |Z|), CW for
//! the winding ratios (pu of the bus's base voltage, kV, or pu of the
//! winding's nominal voltage) and CM for the magnetizing admittance (system
//! base, or no-load loss and exciting current). The Stannum format has one
//! unit for each: impedance and magnetizing admittance in per unit on the
//! transformer's own base, ratios and tap limits in per unit of the
//! winding's nominal voltage.
//!
//! Converting can round a number in its last place. When it does, or when
//! a code has no exact equivalent (load loss in watts), the RAW values are
//! kept in `x.psse.orig` as `[name, value]` pairs; writing RAW again uses
//! them when the transformer still converts to what the record holds, so
//! an unchanged transformer comes back as it was read.

// Windings and winding pairs are indexed in step across several arrays.
#![allow(clippy::needless_range_loop)]

use super::{Ctx, ExCtx, Fields, Item, raw_f64, raw_get, raw_i64, raw_str, rec_item};
use super::{raw_int as int, raw_num as num, raw_text as text};
use crate::diff::value::Value as RawValue;
use stannum_format::{Record, Value};

const PAIRS: [&str; 3] = ["1-2", "2-3", "3-1"];
const PAIR_FIELDS: [&str; 3] = ["12", "23", "31"];
const CONTROLS: [&str; 6] = [
    "none",
    "voltage",
    "reactive_flow",
    "active_flow",
    "dc_line",
    "asymmetric_active_flow",
];

/// The numbers whose units the codes choose, in the format's units.
#[derive(Clone, Debug, PartialEq)]
struct Canon {
    /// Base of each winding pair's impedance (and of the magnetizing
    /// admittance, for the first pair).
    mva: [f64; 3],
    r: [f64; 3],
    x: [f64; 3],
    mag: [f64; 2],
    tap: [f64; 3],
    /// NOMV: the winding's nominal kV, 0 for the bus's base voltage.
    nom: [f64; 3],
    adj_max: [f64; 3],
    adj_min: [f64; 3],
}

impl Canon {
    fn same(&self, other: &Canon) -> bool {
        let bits = |c: &Canon| {
            [c.mva, c.r, c.x, c.tap, c.nom, c.adj_max, c.adj_min]
                .iter()
                .flatten()
                .chain(&c.mag)
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        };
        bits(self) == bits(other)
    }
}

/// Whether a winding's tap limits (RMA, RMI) are ratios, which CW scales,
/// rather than angles or flows.
fn ratio_limits(cod: i64) -> bool {
    matches!(cod.abs(), 0 | 1 | 2 | 4)
}

/// RAW numbers (with their codes) in the format's units.
fn to_canon(f: &Fields, windings: usize, kv: [f64; 3], sbase: f64) -> Canon {
    let cw = raw_i64(f, "CW", 1);
    let cz = raw_i64(f, "CZ", 1);
    let cm = raw_i64(f, "CM", 1);
    let pairs = if windings == 3 { 3 } else { 1 };
    let mut c = Canon {
        mva: [sbase; 3],
        r: [0.0; 3],
        x: [0.0; 3],
        mag: [0.0; 2],
        tap: [1.0; 3],
        nom: [0.0; 3],
        adj_max: [1.1; 3],
        adj_min: [0.9; 3],
    };
    for p in 0..pairs {
        let sb = raw_f64(f, &format!("SBASE{}", PAIRS[p]), sbase);
        let sb = if sb > 0.0 { sb } else { sbase };
        let (r, x) = (
            raw_f64(f, &format!("R{}", PAIRS[p]), 0.0),
            raw_f64(f, &format!("X{}", PAIRS[p]), 0.0),
        );
        c.mva[p] = sb;
        (c.r[p], c.x[p]) = match cz {
            2 => (r, x),
            3 => {
                let r = r / 1e6 / sb;
                (r, (x * x - r * r).max(0.0).sqrt())
            }
            _ => (r * sb / sbase, x * sb / sbase),
        };
    }
    let (m1, m2) = (raw_f64(f, "MAG1", 0.0), raw_f64(f, "MAG2", 0.0));
    c.mag = match cm {
        2 => {
            let g = m1 / 1e6 / c.mva[0];
            [g, -(m2 * m2 - g * g).max(0.0).sqrt()]
        }
        _ => [m1 * sbase / c.mva[0], m2 * sbase / c.mva[0]],
    };
    for w in 0..windings {
        let n = w + 1;
        let nomv = raw_f64(f, &format!("NOMV{n}"), 0.0);
        let nominal = if nomv != 0.0 { nomv } else { kv[w] };
        let windv = raw_f64(f, &format!("WINDV{n}"), if cw == 2 { nominal } else { 1.0 });
        // How a value in the file's units becomes pu of the nominal voltage.
        let scale = |v: f64| match cw {
            2 if nominal > 0.0 => v / nominal,
            1 if nomv != 0.0 && kv[w] > 0.0 => v * kv[w] / nomv,
            _ => v,
        };
        c.nom[w] = nomv;
        c.tap[w] = scale(windv);
        // Winding 2 of a two-winding transformer has no controls.
        if windings == 3 || w == 0 {
            let cod = raw_i64(f, &format!("COD{n}"), 0);
            let (rma, rmi) = (
                raw_f64(f, &format!("RMA{n}"), 1.1),
                raw_f64(f, &format!("RMI{n}"), 0.9),
            );
            (c.adj_max[w], c.adj_min[w]) = if ratio_limits(cod) {
                (scale(rma), scale(rmi))
            } else {
                (rma, rmi)
            };
        }
    }
    c
}

/// The format's numbers as RAW fields, with the codes that need no
/// conversion: CZ 1 when every base is the system base (else 2), CW 1 when
/// every winding's nominal voltage is its bus's (else 3), CM 1.
fn from_canon(c: &Canon, windings: usize, sbase: f64) -> Fields {
    let pairs = if windings == 3 { 3 } else { 1 };
    let system = c.mva[..pairs].iter().all(|&m| m == sbase);
    let own_nominal = c.nom[..windings].iter().all(|&n| n == 0.0);
    let mut out: Fields = vec![
        ("CW".into(), int(if own_nominal { 1 } else { 3 })),
        ("CZ".into(), int(if system { 1 } else { 2 })),
        ("CM".into(), int(1)),
        ("MAG1".into(), num(c.mag[0] * c.mva[0] / sbase)),
        ("MAG2".into(), num(c.mag[1] * c.mva[0] / sbase)),
    ];
    for p in 0..pairs {
        out.push((format!("R{}", PAIRS[p]), num(c.r[p])));
        out.push((format!("X{}", PAIRS[p]), num(c.x[p])));
        out.push((format!("SBASE{}", PAIRS[p]), num(c.mva[p])));
    }
    for w in 0..windings {
        let n = w + 1;
        out.push((format!("WINDV{n}"), num(c.tap[w])));
        out.push((format!("NOMV{n}"), num(c.nom[w])));
        if windings == 3 || w == 0 {
            out.push((format!("RMA{n}"), num(c.adj_max[w])));
            out.push((format!("RMI{n}"), num(c.adj_min[w])));
        }
    }
    out
}

fn orig_value(entries: &[Value], name: &str) -> Option<RawValue> {
    entries.iter().find_map(|e| {
        let e = e.as_list()?;
        if e.first()?.as_str()? != name {
            return None;
        }
        Some(num(e.get(1)?.as_f64()?))
    })
}

fn windings_of(f: &Fields) -> usize {
    if raw_i64(f, "K", 0) != 0 { 3 } else { 2 }
}

fn control(r: &mut Record, f: &Fields, n: usize, p: &str) {
    let cod = raw_i64(f, &format!("COD{n}"), 0);
    let name = CONTROLS
        .get(cod.unsigned_abs() as usize)
        .copied()
        .unwrap_or("none");
    r.set(&format!("{p}control"), Value::Str(name.into()));
    r.set(&format!("{p}control_on"), Value::Bool(cod >= 0));
    let cont = raw_i64(f, &format!("CONT{n}"), 0);
    if cont != 0 {
        r.set(&format!("{p}reg_bus"), Value::Str(cont.abs().to_string()));
        r.set(&format!("{p}reg_bus_reversed"), Value::Bool(cont < 0));
    }
    let nod = raw_i64(f, &format!("NOD{n}"), 0);
    if nod != 0 {
        r.set(&format!("{p}reg_node"), Value::Int(nod));
    }
    r.set(
        &format!("{p}band_max"),
        Value::Float(raw_f64(f, &format!("VMA{n}"), 1.1)),
    );
    r.set(
        &format!("{p}band_min"),
        Value::Float(raw_f64(f, &format!("VMI{n}"), 0.9)),
    );
    r.set(
        &format!("{p}steps"),
        Value::Int(raw_i64(f, &format!("NTP{n}"), 33)),
    );
    let tab = raw_i64(f, &format!("TAB{n}"), 0);
    if tab != 0 {
        r.set(&format!("{p}impedance_table"), Value::Str(tab.to_string()));
    }
    r.set(
        &format!("{p}comp_r_pu"),
        Value::Float(raw_f64(f, &format!("CR{n}"), 0.0)),
    );
    r.set(
        &format!("{p}comp_x_pu"),
        Value::Float(raw_f64(f, &format!("CX{n}"), 0.0)),
    );
    r.set(
        &format!("{p}comp_angle_deg"),
        Value::Float(raw_f64(f, &format!("CNXA{n}"), 0.0)),
    );
}

fn ratings(r: &mut Record, f: &Fields, n: usize, p: &str) {
    for k in 1..=12 {
        r.set(
            &format!("{p}rate{k}_mva"),
            Value::Float(raw_f64(f, &format!("RATE{n}-{k}"), 0.0)),
        );
    }
}

pub(crate) fn import(ctx: &Ctx, f: &Fields, inexact: &mut usize) -> Option<(&'static str, Record)> {
    let w = windings_of(f);
    let buses: Vec<i64> = ["I", "J", "K"][..w]
        .iter()
        .map(|n| raw_i64(f, n, 0).abs())
        .collect();
    if buses.contains(&0) || raw_get(f, "I").is_none() {
        return None;
    }
    let kv = [0, 1, 2].map(|i| buses.get(i).map_or(0.0, |&b| ctx.buses.kv(b)));
    let c = to_canon(f, w, kv, ctx.sbase);
    let mut r = Record::new("");
    let three = w == 3;
    let table = if three { "transformer3" } else { "transformer" };
    if three {
        for (i, b) in buses.iter().enumerate() {
            r.set(&format!("bus{}", i + 1), super::Buses::id(*b));
        }
    } else {
        r.set("from", super::Buses::id(buses[0]));
        r.set("to", super::Buses::id(buses[1]));
    }
    r.set("ckt", Value::Str(raw_str(f, "CKT", "1")));
    let stat = raw_i64(f, "STAT", 1);
    let status = if three {
        ["off", "on", "w2_off", "w3_off", "w1_off"]
            .get(stat as usize)
            .copied()?
    } else {
        if stat == 0 { "off" } else { "on" }
    };
    r.set("status", Value::Str(status.into()));
    r.set("name", Value::Str(raw_str(f, "NAME", "")));
    let nmetr = raw_i64(f, "NMETR", 2);
    let metered = if three {
        ["w1", "w2", "w3"]
            .get((nmetr - 1).clamp(0, 2) as usize)
            .copied()?
    } else if nmetr == 1 {
        "from"
    } else {
        "to"
    };
    r.set("metered", Value::Str(metered.into()));
    let pairs = if three { 3 } else { 1 };
    for p in 0..pairs {
        let (base, rf, xf) = if three {
            (
                format!("mva_base{}", PAIR_FIELDS[p]),
                format!("r{}_pu", PAIR_FIELDS[p]),
                format!("x{}_pu", PAIR_FIELDS[p]),
            )
        } else {
            (
                "mva_base".to_string(),
                "r_pu".to_string(),
                "x_pu".to_string(),
            )
        };
        if c.mva[p] != ctx.sbase {
            r.set(&base, Value::Float(c.mva[p]));
        }
        r.set(&rf, Value::Float(c.r[p]));
        r.set(&xf, Value::Float(c.x[p]));
    }
    r.set("mag_g_pu", Value::Float(c.mag[0]));
    r.set("mag_b_pu", Value::Float(c.mag[1]));
    if three {
        r.set("star_vm_pu", Value::Float(raw_f64(f, "VMSTAR", 1.0)));
        r.set("star_va_deg", Value::Float(raw_f64(f, "ANSTAR", 0.0)));
    }
    for i in 0..w {
        let n = i + 1;
        let p = format!("w{n}_");
        r.set(&format!("{p}tap_pu"), Value::Float(c.tap[i]));
        if c.nom[i] != 0.0 {
            r.set(&format!("{p}kv"), Value::Float(c.nom[i]));
        }
        if three || i == 0 {
            r.set(
                &format!("{p}angle_deg"),
                Value::Float(raw_f64(f, &format!("ANG{n}"), 0.0)),
            );
            let cp = if three { p.clone() } else { String::new() };
            ratings(&mut r, f, n, &cp);
            control(&mut r, f, n, &cp);
            r.set(&format!("{cp}adjust_max"), Value::Float(c.adj_max[i]));
            r.set(&format!("{cp}adjust_min"), Value::Float(c.adj_min[i]));
        }
    }
    // Owners: PSS/E's default is the first bus's owner, whole.
    let owner = ctx.buses.info.get(&buses[0]).map_or(1, |b| b.2);
    let mut owners = Vec::new();
    for k in 1..=4 {
        let o = raw_i64(f, &format!("O{k}"), if k == 1 { owner } else { 0 });
        if o != 0 {
            owners.push(Value::List(vec![
                Value::Str(o.to_string()),
                Value::Float(raw_f64(f, &format!("F{k}"), 1.0)),
            ]));
        }
    }
    let default_owner = owners.len() == 1
        && owners[0] == Value::List(vec![Value::Str(owner.to_string()), Value::Float(1.0)]);
    r.set(
        "owners",
        Value::List(if default_owner { vec![] } else { owners }),
    );
    r.set("vector_group", Value::Str(raw_str(f, "VECGRP", "")));

    // Keep the RAW values when writing the format's numbers back as RAW
    // wouldn't give them exactly.
    let cw = raw_i64(f, "CW", 1);
    let original = |name: &str| -> f64 {
        let default = match name {
            n if n.starts_with("SBASE") => ctx.sbase,
            n if n.starts_with("WINDV") && cw == 2 => {
                let w: usize = n[5..].parse().unwrap_or(1);
                let nomv = raw_f64(f, &format!("NOMV{w}"), 0.0);
                if nomv != 0.0 { nomv } else { kv[w - 1] }
            }
            n if n.starts_with("WINDV") => 1.0,
            n if n.starts_with("RMA") => 1.1,
            n if n.starts_with("RMI") => 0.9,
            _ => 0.0,
        };
        raw_f64(f, name, default)
    };
    let back = from_canon(&c, w, ctx.sbase);
    let values = || {
        back.iter()
            .filter(|(n, _)| !matches!(n.as_str(), "CW" | "CZ" | "CM"))
    };
    if values().any(|(n, v)| original(n).to_bits() != super::value_f64(v).to_bits()) {
        *inexact += 1;
        let mut orig: Vec<Value> = ["CW", "CZ", "CM"]
            .iter()
            .map(|code| {
                Value::List(vec![
                    Value::Str((*code).into()),
                    Value::Int(raw_i64(f, code, 1)),
                ])
            })
            .collect();
        orig.extend(
            values()
                .map(|(n, _)| Value::List(vec![Value::Str(n.clone()), Value::Float(original(n))])),
        );
        r.set("x.psse.orig", Value::List(orig));
    }
    Some((table, r))
}

/// The record's numbers in the format's units, as read from the record.
fn canon_of(ctx: &ExCtx, table: &str, r: &Record, w: usize) -> Canon {
    let three = w == 3;
    let mut c = Canon {
        mva: [ctx.sbase; 3],
        r: [0.0; 3],
        x: [0.0; 3],
        mag: [ctx.f(table, r, "mag_g_pu"), ctx.f(table, r, "mag_b_pu")],
        tap: [1.0; 3],
        nom: [0.0; 3],
        adj_max: [1.1; 3],
        adj_min: [0.9; 3],
    };
    for p in 0..if three { 3 } else { 1 } {
        let (base, rf, xf) = if three {
            (
                format!("mva_base{}", PAIR_FIELDS[p]),
                format!("r{}_pu", PAIR_FIELDS[p]),
                format!("x{}_pu", PAIR_FIELDS[p]),
            )
        } else {
            (
                "mva_base".to_string(),
                "r_pu".to_string(),
                "x_pu".to_string(),
            )
        };
        c.mva[p] = r.float(&base).unwrap_or(ctx.sbase);
        c.r[p] = ctx.f(table, r, &rf);
        c.x[p] = ctx.f(table, r, &xf);
    }
    for i in 0..w {
        let p = format!("w{}_", i + 1);
        c.tap[i] = ctx.f(table, r, &format!("{p}tap_pu"));
        c.nom[i] = r.float(&format!("{p}kv")).unwrap_or(0.0);
        if three || i == 0 {
            let cp = if three { p.clone() } else { String::new() };
            c.adj_max[i] = ctx.f(table, r, &format!("{cp}adjust_max"));
            c.adj_min[i] = ctx.f(table, r, &format!("{cp}adjust_min"));
        }
    }
    c
}

pub(crate) fn export(ctx: &ExCtx, table: &str, r: &Record) -> Result<Item, String> {
    let three = table == "transformer3";
    let w = if three { 3 } else { 2 };
    let buses: Vec<i64> = if three {
        (1..=3)
            .map(|k| ctx.bus(r.get(&format!("bus{k}"))))
            .collect::<Result<_, _>>()?
    } else {
        vec![ctx.bus(r.get("from"))?, ctx.bus(r.get("to"))?]
    };
    let kv = [0, 1, 2].map(|i| {
        buses
            .get(i)
            .and_then(|b| ctx.buses.get(b))
            .map_or(0.0, |b| b.3)
    });
    let c = canon_of(ctx, table, r, w);
    let mut converted = from_canon(&c, w, ctx.sbase);
    // The file's own values, when the record still converts from them.
    if let Some(orig) = r.get("x.psse.orig").and_then(Value::as_list) {
        let mut overlay = converted.clone();
        for (name, v) in overlay.iter_mut() {
            if let Some(o) = orig_value(orig, name) {
                *v = match name.as_str() {
                    "CW" | "CZ" | "CM" => int(super::value_f64(&o) as i64),
                    _ => o,
                };
            }
        }
        // Whether each winding's tap limits are ratios depends on its control.
        let mut check = overlay.clone();
        for i in 0..w {
            let n = i + 1;
            if three || i == 0 {
                let cod = CONTROLS
                    .iter()
                    .position(|c| {
                        *c == ctx.s(
                            table,
                            r,
                            &format!(
                                "{}control",
                                if three {
                                    format!("w{n}_")
                                } else {
                                    String::new()
                                }
                            ),
                        )
                    })
                    .unwrap_or(0) as i64;
                check.push((format!("COD{n}"), int(cod)));
            }
        }
        if to_canon(&check, w, kv, ctx.sbase).same(&c) {
            converted = overlay;
        }
    }
    let mut out: Fields = Vec::new();
    out.push(("I".into(), int(buses[0])));
    out.push(("J".into(), int(buses[1])));
    out.push(("K".into(), int(if three { buses[2] } else { 0 })));
    out.push(("CKT".into(), text(ctx.s(table, r, "ckt"))));
    let stat = ctx.s(table, r, "status");
    let stat = if three {
        ["off", "on", "w2_off", "w3_off", "w1_off"]
            .iter()
            .position(|s| *s == stat)
            .unwrap_or(1) as i64
    } else {
        i64::from(stat != "off")
    };
    out.push(("STAT".into(), int(stat)));
    let metered = ctx.s(table, r, "metered");
    let nmetr = match metered {
        "from" | "w1" => 1,
        "w3" => 3,
        _ => 2,
    };
    out.push(("NMETR".into(), int(nmetr)));
    out.push(("NAME".into(), text(ctx.s(table, r, "name"))));
    let owner = ctx.buses.get(&buses[0]).map_or(1, |b| b.2);
    let owners = ctx
        .doc
        .value(table, r, "owners")
        .and_then(Value::as_list)
        .unwrap_or_default();
    let mut list: Vec<(i64, f64)> = owners
        .iter()
        .filter_map(|o| {
            let o = o.as_list()?;
            Some((o.first()?.as_str()?.parse().ok()?, o.get(1)?.as_f64()?))
        })
        .collect();
    if list.is_empty() {
        list.push((owner, 1.0));
    }
    for k in 0..4 {
        let (o, f) = list.get(k).copied().unwrap_or((0, 1.0));
        out.push((format!("O{}", k + 1), int(o)));
        out.push((format!("F{}", k + 1), num(f)));
    }
    out.push(("VECGRP".into(), text(ctx.s(table, r, "vector_group"))));
    if three {
        out.push(("VMSTAR".into(), num(ctx.f(table, r, "star_vm_pu"))));
        out.push(("ANSTAR".into(), num(ctx.f(table, r, "star_va_deg"))));
    }
    for i in 0..w {
        let n = i + 1;
        if !(three || i == 0) {
            continue;
        }
        let p = if three {
            format!("w{n}_")
        } else {
            String::new()
        };
        out.push((
            format!("ANG{n}"),
            num(ctx.f(table, r, &format!("w{n}_angle_deg"))),
        ));
        for k in 1..=12 {
            out.push((
                format!("RATE{n}-{k}"),
                num(ctx.f(table, r, &format!("{p}rate{k}_mva"))),
            ));
        }
        let control = ctx.s(table, r, &format!("{p}control"));
        let cod = CONTROLS.iter().position(|c| *c == control).unwrap_or(0) as i64;
        let on = ctx
            .doc
            .value(table, r, &format!("{p}control_on"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        out.push((format!("COD{n}"), int(if on { cod } else { -cod })));
        let cont = match r.get(&format!("{p}reg_bus")) {
            Some(v) => {
                let b = ctx.bus(Some(v))?;
                let reversed = ctx
                    .doc
                    .value(table, r, &format!("{p}reg_bus_reversed"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if reversed { -b } else { b }
            }
            None => 0,
        };
        out.push((format!("CONT{n}"), int(cont)));
        out.push((
            format!("NOD{n}"),
            int(r.int(&format!("{p}reg_node")).unwrap_or(0)),
        ));
        out.push((
            format!("VMA{n}"),
            num(ctx.f(table, r, &format!("{p}band_max"))),
        ));
        out.push((
            format!("VMI{n}"),
            num(ctx.f(table, r, &format!("{p}band_min"))),
        ));
        out.push((
            format!("NTP{n}"),
            int(ctx
                .doc
                .value(table, r, &format!("{p}steps"))
                .and_then(Value::as_i64)
                .unwrap_or(33)),
        ));
        let tab = r
            .get(&format!("{p}impedance_table"))
            .and_then(Value::as_str)
            .and_then(|id| {
                ctx.doc
                    .records("impedance_table")
                    .iter()
                    .find(|t| t.id == id)
            })
            .and_then(|t| t.int("number"))
            .unwrap_or(0);
        out.push((format!("TAB{n}"), int(tab)));
        out.push((
            format!("CR{n}"),
            num(ctx.f(table, r, &format!("{p}comp_r_pu"))),
        ));
        out.push((
            format!("CX{n}"),
            num(ctx.f(table, r, &format!("{p}comp_x_pu"))),
        ));
        out.push((
            format!("CNXA{n}"),
            num(ctx.f(table, r, &format!("{p}comp_angle_deg"))),
        ));
    }
    out.extend(converted);
    Ok(rec_item(out))
}
