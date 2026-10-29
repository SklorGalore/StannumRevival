//! RAW 35 records and Stannum records, field by field, for every section
//! but transformers and impedance correction tables (see `transformer.rs`
//! and `mod.rs`).
//!
//! Each [`M`] names a RAW field, the Stannum field it becomes and how its
//! value converts. A RAW field the record leaves out takes PSS/E's default.

use super::{Ctx, ExCtx, Fields, Item, raw_f64, raw_get, raw_i64, raw_str, rec_item};
use super::{lines_item, raw_int as int, raw_num as num, raw_text as text};
use crate::diff::value::Value as RawValue;
use stannum_format::{Record, Value};

#[derive(Clone, Copy)]
pub(crate) enum C {
    /// A float, with PSS/E's default.
    F(f64),
    /// A float written negated (PSS/E's YQ is MVAr injected).
    Neg(f64),
    /// A whole number.
    I(i64),
    /// A whole number where 0 means none.
    OptI,
    /// Text, trimmed, with a default.
    S(&'static str),
    /// 1 in service, 0 out.
    Stat,
    /// 0 or 1, with a default.
    B(i64),
    /// A code with a name for each value, and the default code.
    E(&'static [(i64, &'static str)], i64),
    /// A text code with a name for each value, and the default code.
    Es(&'static [(&'static str, &'static str)], &'static str),
    /// A bus number that must be there.
    Bus,
    /// A bus number where 0 means none.
    OptBus,
    /// An area, zone or owner number, as the id it names.
    Soft(i64),
    /// An area (0), zone (1) or owner (2) left out when it is the bus's.
    BusOf(usize),
    /// A machine base left out when it is the system base.
    Base,
}

pub(crate) struct M(pub &'static str, pub &'static str, pub C);

const MET: &[(i64, &str)] = &[(1, "from"), (2, "to")];

const BUS: &[M] = &[
    M("I", "number", C::I(0)),
    M("NAME", "name", C::S("")),
    M("BASKV", "kv", C::F(0.0)),
    M(
        "IDE",
        "type",
        C::E(&[(1, "pq"), (2, "pv"), (3, "ref"), (4, "isolated")], 1),
    ),
    M("AREA", "area", C::Soft(1)),
    M("ZONE", "zone", C::Soft(1)),
    M("OWNER", "owner", C::Soft(1)),
    M("VM", "vm_pu", C::F(1.0)),
    M("VA", "va_deg", C::F(0.0)),
    M("NVHI", "vmax_pu", C::F(1.1)),
    M("NVLO", "vmin_pu", C::F(0.9)),
    M("EVHI", "evmax_pu", C::F(1.1)),
    M("EVLO", "evmin_pu", C::F(0.9)),
];

const LOAD: &[M] = &[
    M("I", "bus", C::Bus),
    M("ID", "label", C::S("1")),
    M("STAT", "status", C::Stat),
    M("AREA", "area", C::BusOf(0)),
    M("ZONE", "zone", C::BusOf(1)),
    M("PL", "p_mw", C::F(0.0)),
    M("QL", "q_mvar", C::F(0.0)),
    M("IP", "ip_mw", C::F(0.0)),
    M("IQ", "iq_mvar", C::F(0.0)),
    M("YP", "yp_mw", C::F(0.0)),
    M("YQ", "yq_mvar", C::Neg(0.0)),
    M("OWNER", "owner", C::BusOf(2)),
    M("SCALE", "scalable", C::B(1)),
    M("INTRPT", "interruptible", C::B(0)),
    M("DGENP", "dgen_p_mw", C::F(0.0)),
    M("DGENQ", "dgen_q_mvar", C::F(0.0)),
    M("DGENM", "dgen_on", C::B(0)),
    M("LOADTYPE", "load_type", C::S("")),
];

const SHUNT: &[M] = &[
    M("I", "bus", C::Bus),
    M("ID", "label", C::S("1")),
    M("STATUS", "status", C::Stat),
    M("GL", "g_mw", C::F(0.0)),
    M("BL", "b_mvar", C::F(0.0)),
];

const GENERATOR: &[M] = &[
    M("I", "bus", C::Bus),
    M("ID", "label", C::S("1")),
    M("PG", "p_mw", C::F(0.0)),
    M("QG", "q_mvar", C::F(0.0)),
    M("QT", "qmax_mvar", C::F(9999.0)),
    M("QB", "qmin_mvar", C::F(-9999.0)),
    M("VS", "vset_pu", C::F(1.0)),
    M("IREG", "reg_bus", C::OptBus),
    M("NREG", "reg_node", C::OptI),
    M("MBASE", "mva_base", C::Base),
    M("ZR", "source_r_pu", C::F(0.0)),
    M("ZX", "source_x_pu", C::F(1.0)),
    M("RT", "gsu_r_pu", C::F(0.0)),
    M("XT", "gsu_x_pu", C::F(0.0)),
    M("GTAP", "gsu_tap_pu", C::F(1.0)),
    M("STAT", "status", C::Stat),
    M("RMPCT", "rmpct", C::F(100.0)),
    M("PT", "pmax_mw", C::F(9999.0)),
    M("PB", "pmin_mw", C::F(-9999.0)),
    M(
        "BASLOD",
        "dispatch",
        C::E(
            &[(0, "both"), (1, "down_only"), (2, "none"), (3, "up_only")],
            0,
        ),
    ),
    M(
        "WMOD",
        "wind_mode",
        C::E(
            &[
                (0, "conventional"),
                (1, "q_limits"),
                (2, "pf_limits"),
                (3, "fixed_pf"),
            ],
            0,
        ),
    ),
    M("WPF", "wind_pf", C::F(1.0)),
];

const RATES: [(&str, &str); 12] = [
    ("RATE1", "rate1_mva"),
    ("RATE2", "rate2_mva"),
    ("RATE3", "rate3_mva"),
    ("RATE4", "rate4_mva"),
    ("RATE5", "rate5_mva"),
    ("RATE6", "rate6_mva"),
    ("RATE7", "rate7_mva"),
    ("RATE8", "rate8_mva"),
    ("RATE9", "rate9_mva"),
    ("RATE10", "rate10_mva"),
    ("RATE11", "rate11_mva"),
    ("RATE12", "rate12_mva"),
];

const BRANCH: &[M] = &[
    M("I", "from", C::Bus),
    M("J", "to", C::Bus),
    M("CKT", "ckt", C::S("1")),
    M("R", "r_pu", C::F(0.0)),
    M("X", "x_pu", C::F(0.0)),
    M("B", "b_pu", C::F(0.0)),
    M("NAME", "name", C::S("")),
    M("GI", "gi_pu", C::F(0.0)),
    M("BI", "bi_pu", C::F(0.0)),
    M("GJ", "gj_pu", C::F(0.0)),
    M("BJ", "bj_pu", C::F(0.0)),
    M("STAT", "status", C::Stat),
    M("MET", "metered", C::E(MET, 1)),
    M("LEN", "length", C::F(0.0)),
];

const SWITCH: &[M] = &[
    M("I", "from", C::Bus),
    M("J", "to", C::Bus),
    M("CKT", "ckt", C::S("1")),
    M("X", "x_pu", C::F(0.0001)),
    M("STAT", "status", C::Stat),
    M("NSTAT", "normal", C::E(&[(1, "closed"), (0, "open")], 1)),
    M("MET", "metered", C::E(MET, 1)),
    M(
        "STYPE",
        "kind",
        C::E(&[(1, "generic"), (2, "breaker"), (3, "disconnector")], 1),
    ),
    M("NAME", "name", C::S("")),
];

const AREA: &[M] = &[
    M("I", "number", C::I(0)),
    M("ISW", "slack_bus", C::OptBus),
    M("PDES", "export_mw", C::F(0.0)),
    M("PTOL", "tolerance_mw", C::F(10.0)),
    M("ARNAME", "name", C::S("")),
];

const ZONE: &[M] = &[M("I", "number", C::I(0)), M("ZONAME", "name", C::S(""))];
const OWNER: &[M] = &[M("I", "number", C::I(0)), M("OWNAME", "name", C::S(""))];

const TRANSFER: &[M] = &[
    M("ARFROM", "from_area", C::Soft(0)),
    M("ARTO", "to_area", C::Soft(0)),
    M("TRID", "label", C::S("1")),
    M("PTRAN", "p_mw", C::F(0.0)),
];

const SWITCHED: &[M] = &[
    M("I", "bus", C::Bus),
    M("ID", "label", C::S("1")),
    M(
        "MODSW",
        "mode",
        C::E(
            &[
                (0, "locked"),
                (1, "discrete"),
                (2, "continuous"),
                (3, "plant_q"),
                (4, "vsc_q"),
                (5, "shunt_admittance"),
                (6, "facts_q"),
            ],
            1,
        ),
    ),
    M(
        "ADJM",
        "switching",
        C::E(&[(0, "in_order"), (1, "nearest")], 0),
    ),
    M("STAT", "status", C::Stat),
    M("VSWHI", "vmax_pu", C::F(1.0)),
    M("VSWLO", "vmin_pu", C::F(1.0)),
    M("SWREG", "reg_bus", C::OptBus),
    M("NREG", "reg_node", C::OptI),
    M("RMPCT", "rmpct", C::F(100.0)),
    M("RMIDNT", "controller", C::S("")),
    M("BINIT", "b_init_mvar", C::F(0.0)),
];

const FACTS: &[M] = &[
    M("NAME", "name", C::S("")),
    M("I", "from", C::Bus),
    M("J", "to", C::OptBus),
    M(
        "MODE",
        "mode",
        C::E(
            &[
                (0, "off"),
                (1, "normal"),
                (2, "shunt_only"),
                (3, "constant_impedance"),
                (4, "constant_voltage"),
                (5, "ipfc_master_pq"),
                (6, "ipfc_slave_pq"),
                (7, "ipfc_master_voltage"),
                (8, "ipfc_slave_voltage"),
            ],
            1,
        ),
    ),
    M("PDES", "p_set_mw", C::F(0.0)),
    M("QDES", "q_set_mvar", C::F(0.0)),
    M("VSET", "vset_pu", C::F(1.0)),
    M("SHMX", "shunt_max_mva", C::F(9999.0)),
    M("TRMX", "series_max_mw", C::F(9999.0)),
    M("VTMN", "vt_min_pu", C::F(0.9)),
    M("VTMX", "vt_max_pu", C::F(1.1)),
    M("VSMX", "vs_max_pu", C::F(1.0)),
    M("IMX", "imax_mva", C::F(0.0)),
    M("LINX", "linx_pu", C::F(0.05)),
    M("RMPCT", "rmpct", C::F(100.0)),
    M("OWNER", "owner", C::Soft(1)),
    M("SET1", "set1", C::F(0.0)),
    M("SET2", "set2", C::F(0.0)),
    M(
        "VSREF",
        "series_ref",
        C::E(&[(0, "voltage"), (1, "impedance")], 0),
    ),
    M("REMOT", "reg_bus", C::OptBus),
    M("MNAME", "ipfc_partner", C::S("")),
];

const LCC: &[M] = &[
    M("NAME", "name", C::S("")),
    M(
        "MDC",
        "mode",
        C::E(&[(0, "blocked"), (1, "power"), (2, "current")], 0),
    ),
    M("RDC", "r_ohm", C::F(0.0)),
    M("SETVL", "setpoint", C::F(0.0)),
    M("VSCHD", "vsched_kv", C::F(0.0)),
    M("VCMOD", "vcmod_kv", C::F(0.0)),
    M("RCOMP", "rcomp_ohm", C::F(0.0)),
    M("DELTI", "margin", C::F(0.0)),
    M(
        "METER",
        "metered",
        C::Es(&[("I", "inverter"), ("R", "rectifier")], "I"),
    ),
    M("DCVMIN", "dcvmin_kv", C::F(0.0)),
    M("CCCITMX", "ccc_iterations", C::I(20)),
    M("CCCACC", "ccc_accel", C::F(1.0)),
];

/// One LCC converter: RAW field stems (R or I appended) and Stannum fields
/// (rect_ or inv_ prepended).
const LCC_CONVERTER: &[M] = &[
    M("IP", "bus", C::Bus),
    M("NB", "bridges", C::I(1)),
    M("ANMX", "angle_max_deg", C::F(0.0)),
    M("ANMN", "angle_min_deg", C::F(0.0)),
    M("RC", "rc_ohm", C::F(0.0)),
    M("XC", "xc_ohm", C::F(0.0)),
    M("EBAS", "ebase_kv", C::F(0.0)),
    M("TR", "ratio", C::F(1.0)),
    M("TAP", "tap_pu", C::F(1.0)),
    M("TMX", "tap_max_pu", C::F(1.5)),
    M("TMN", "tap_min_pu", C::F(0.51)),
    M("STP", "tap_step_pu", C::F(0.00625)),
    M("IC", "commutating_bus", C::OptBus),
    M("ND", "commutating_node", C::OptI),
    M("IF", "xfmr_from", C::OptBus),
    M("IT", "xfmr_to", C::OptBus),
    M("ID", "xfmr_ckt", C::S("1")),
    M("XCAP", "xcap_ohm", C::F(0.0)),
];

const VSC: &[M] = &[
    M("NAME", "name", C::S("")),
    M("MDC", "status", C::Stat),
    M("RDC", "r_ohm", C::F(0.0)),
];

const VSC_CONVERTER: &[M] = &[
    M("IBUS", "bus", C::Bus),
    M(
        "TYPE",
        "dc_mode",
        C::E(&[(0, "off"), (1, "voltage"), (2, "power")], 0),
    ),
    M(
        "MODE",
        "ac_mode",
        C::E(&[(1, "voltage"), (2, "power_factor")], 1),
    ),
    M("DCSET", "dc_set", C::F(0.0)),
    M("ACSET", "ac_set", C::F(1.0)),
    M("ALOSS", "loss_a_kw", C::F(0.0)),
    M("BLOSS", "loss_b_kw_per_a", C::F(0.0)),
    M("MINLOSS", "loss_min_kw", C::F(0.0)),
    M("SMAX", "smax_mva", C::F(0.0)),
    M("IMAX", "imax_a", C::F(0.0)),
    M("PWF", "pwf", C::F(1.0)),
    M("MAXQ", "qmax_mvar", C::F(9999.0)),
    M("MINQ", "qmin_mvar", C::F(-9999.0)),
    M("REMOT", "reg_bus", C::OptBus),
    M("RMPCT", "rmpct", C::F(100.0)),
];

const MULTI_SECTION: &[M] = &[
    M("I", "from", C::Bus),
    M("J", "to", C::Bus),
    M("ID", "label", C::S("&1")),
    M("MET", "metered", C::E(MET, 1)),
];

/// The Stannum table a RAW section's records go to.
pub(crate) fn table_of(section: &str) -> Option<&'static str> {
    SECTIONS
        .iter()
        .find(|(s, _)| *s == section)
        .map(|(_, t)| *t)
}

/// The RAW section a Stannum table's records go to.
pub(crate) fn section_of(table: &str) -> Option<&'static str> {
    SECTIONS.iter().find(|(_, t)| *t == table).map(|(s, _)| *s)
}

const SECTIONS: &[(&str, &str)] = &[
    ("BUS", "bus"),
    ("LOAD", "load"),
    ("FIXED SHUNT", "shunt"),
    ("GENERATOR", "generator"),
    ("BRANCH", "line"),
    ("SYSTEM SWITCHING DEVICE", "switch"),
    ("TRANSFORMER", "transformer"),
    ("AREA", "area"),
    ("TWO-TERMINAL DC", "lcc_line"),
    ("VSC DC LINE", "vsc_line"),
    ("IMPEDANCE CORRECTION", "impedance_table"),
    ("MULTI-SECTION LINE", "multi_section_line"),
    ("ZONE", "zone"),
    ("INTER-AREA TRANSFER", "area_transfer"),
    ("OWNER", "owner"),
    ("FACTS DEVICE", "facts"),
    ("SWITCHED SHUNT", "switched_shunt"),
];

fn maps_of(table: &str) -> &'static [M] {
    match table {
        "bus" => BUS,
        "load" => LOAD,
        "shunt" => SHUNT,
        "generator" => GENERATOR,
        "line" => BRANCH,
        "switch" => SWITCH,
        "area" => AREA,
        "zone" => ZONE,
        "owner" => OWNER,
        "area_transfer" => TRANSFER,
        "switched_shunt" => SWITCHED,
        "facts" => FACTS,
        "lcc_line" => LCC,
        "vsc_line" => VSC,
        "multi_section_line" => MULTI_SECTION,
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// RAW → Stannum.

/// One RAW field's value as the Stannum field takes it: Ok(None) leaves
/// the field out (it then means its default, or "none"); Err when the
/// value has no Stannum equivalent (a code with no name), so the record is
/// kept as RAW lines instead.
fn import_one(ctx: &Ctx, f: &Fields, raw: &str, c: C, bus: i64) -> Result<Option<Value>, ()> {
    let present = raw_get(f, raw).is_some();
    Ok(Some(match c {
        C::F(d) => Value::Float(raw_f64(f, raw, d)),
        C::Neg(d) => Value::Float(match raw_f64(f, raw, d) {
            0.0 => 0.0,
            v => -v,
        }),
        C::I(d) => Value::Int(raw_i64(f, raw, d)),
        C::OptI => match raw_i64(f, raw, 0) {
            0 => return Ok(None),
            n => Value::Int(n),
        },
        C::S(d) => Value::Str(raw_str(f, raw, d)),
        C::Stat => Value::Str(if raw_i64(f, raw, 1) == 0 { "off" } else { "on" }.into()),
        C::B(d) => Value::Bool(raw_i64(f, raw, d) != 0),
        C::E(codes, d) => {
            let code = raw_i64(f, raw, d);
            Value::Str(codes.iter().find(|(c, _)| *c == code).ok_or(())?.1.into())
        }
        C::Es(codes, d) => {
            let code = raw_str(f, raw, d).to_uppercase();
            let code = if code.is_empty() { d.to_string() } else { code };
            Value::Str(codes.iter().find(|(c, _)| *c == code).ok_or(())?.1.into())
        }
        C::Bus => super::Buses::id(raw_i64(f, raw, 0).abs()),
        C::OptBus => match raw_i64(f, raw, 0) {
            0 => return Ok(None),
            n => super::Buses::id(n.abs()),
        },
        C::Soft(d) => Value::Str(raw_i64(f, raw, d).to_string()),
        C::BusOf(k) => {
            let own = ctx.buses.info.get(&bus).map(|b| [b.0, b.1, b.2][k]);
            let v = if present {
                raw_i64(f, raw, 0)
            } else {
                own.ok_or(())?
            };
            if Some(v) == own {
                return Ok(None);
            }
            Value::Str(v.to_string())
        }
        C::Base => {
            let v = raw_f64(f, raw, ctx.sbase);
            if v == ctx.sbase || v == 0.0 {
                return Ok(None);
            }
            Value::Float(v)
        }
    }))
}

fn import_maps(
    ctx: &Ctx,
    f: &Fields,
    maps: &[M],
    raw_suffix: &str,
    stn_prefix: &str,
    bus: i64,
    r: &mut Record,
) -> Option<()> {
    for M(raw, stn, c) in maps {
        let name = format!("{raw}{raw_suffix}");
        if matches!(c, C::Bus) && raw_get(f, &name).is_none() {
            return None;
        }
        if let Some(v) = import_one(ctx, f, &name, *c, bus).ok()? {
            r.set(&format!("{stn_prefix}{stn}"), v);
        }
    }
    Some(())
}

/// Owners O1..O4 with fractions F1..F4: empty when it is PSS/E's default
/// (the first owner `default`, whole).
fn import_owners(f: &Fields, default: i64, suffix: &str) -> Value {
    let mut out = Vec::new();
    for k in 1..=4 {
        let o = raw_i64(
            f,
            &format!("O{k}{suffix}"),
            if k == 1 { default } else { 0 },
        );
        if o != 0 {
            let fr = raw_f64(f, &format!("F{k}{suffix}"), 1.0);
            out.push(Value::List(vec![
                Value::Str(o.to_string()),
                Value::Float(fr),
            ]));
        }
    }
    let is_default = out.len() == 1
        && out[0] == Value::List(vec![Value::Str(default.to_string()), Value::Float(1.0)]);
    if is_default {
        Value::List(vec![])
    } else {
        Value::List(out)
    }
}

pub(crate) fn import(ctx: &Ctx, section: &str, f: &Fields) -> Option<(&'static str, Record)> {
    let table = table_of(section)?;
    let mut r = Record::new("");
    let first_bus = ["I", "IPR", "IBUS"]
        .iter()
        .find_map(|n| raw_get(f, n).map(|_| raw_i64(f, n, 0)))
        .unwrap_or(0);
    let bus_owner = ctx.buses.info.get(&first_bus).map_or(1, |b| b.2);
    match table {
        "lcc_line" => {
            import_maps(ctx, f, LCC, "", "", 0, &mut r)?;
            import_maps(ctx, f, LCC_CONVERTER, "R", "rect_", 0, &mut r)?;
            import_maps(ctx, f, LCC_CONVERTER, "I", "inv_", 0, &mut r)?;
            return Some((table, r));
        }
        "vsc_line" => {
            import_maps(ctx, f, VSC, "", "", 0, &mut r)?;
            r.set("owners", import_owners(f, 1, ""));
            import_maps(ctx, f, VSC_CONVERTER, "", "c1_", 0, &mut r)?;
            import_maps(ctx, f, VSC_CONVERTER, "#2", "c2_", 0, &mut r)?;
            return Some((table, r));
        }
        _ => {}
    }
    import_maps(ctx, f, maps_of(table), "", "", first_bus, &mut r)?;
    if matches!(table, "generator" | "line") {
        r.set("owners", import_owners(f, bus_owner, ""));
    }
    if matches!(table, "line" | "switch") {
        for (raw, stn) in RATES {
            r.set(stn, Value::Float(raw_f64(f, raw, 0.0)));
        }
    }
    if table == "switched_shunt" {
        r.set("blocks", import_blocks(f));
    }
    if table == "multi_section_line" {
        let mut via = Vec::new();
        for k in 1..=9 {
            match raw_i64(f, &format!("DUM{k}"), 0) {
                0 => break,
                n => via.push(super::Buses::id(n.abs())),
            }
        }
        r.set("via", Value::List(via));
    }
    if table == "facts" && r.str("name").is_none_or(str::is_empty) {
        return None;
    }
    Some((table, r))
}

/// Switched shunt blocks: `[count, mvar]`, with `off` third when the block
/// is out of service, up to the last block that has steps.
fn import_blocks(f: &Fields) -> Value {
    let mut blocks = Vec::new();
    for g in 1..=8 {
        let n = raw_i64(f, &format!("N{g}"), 0);
        let b = raw_f64(f, &format!("B{g}"), 0.0);
        let s = raw_i64(f, &format!("S{g}"), 1);
        let mut block = vec![Value::Int(n), Value::Float(b)];
        if s == 0 {
            block.push(Value::Str("off".into()));
        }
        blocks.push((n != 0 || b != 0.0, Value::List(block)));
    }
    let last = blocks
        .iter()
        .rposition(|(used, _)| *used)
        .map_or(0, |i| i + 1);
    Value::List(blocks.into_iter().take(last).map(|(_, v)| v).collect())
}

// ---------------------------------------------------------------------------
// Stannum → RAW.

fn export_one(
    ctx: &ExCtx,
    table: &str,
    r: &Record,
    stn: &str,
    c: C,
    bus: i64,
) -> Result<RawValue, String> {
    let v = ctx.doc.value(table, r, stn);
    let s = || v.and_then(Value::as_str).unwrap_or("");
    Ok(match c {
        C::F(_) => num(v.and_then(Value::as_f64).unwrap_or(0.0)),
        C::Neg(_) => num(match v.and_then(Value::as_f64).unwrap_or(0.0) {
            0.0 => 0.0,
            v => -v,
        }),
        C::I(_) | C::OptI => int(v.and_then(Value::as_i64).unwrap_or(0)),
        C::S(_) => text(s()),
        C::Stat => int(i64::from(s() != "off")),
        C::B(_) => int(i64::from(v.and_then(Value::as_bool).unwrap_or(false))),
        C::E(codes, d) => int(codes.iter().find(|(_, n)| *n == s()).map_or(d, |(c, _)| *c)),
        C::Es(codes, d) => text(codes.iter().find(|(_, n)| *n == s()).map_or(d, |(c, _)| *c)),
        C::Bus => int(ctx.bus(v)?),
        C::OptBus => int(if v.is_some() { ctx.bus(v)? } else { 0 }),
        C::Soft(d) => int(s().parse().unwrap_or(d)),
        C::BusOf(k) => int(match v {
            Some(v) => v.as_str().and_then(|s| s.parse().ok()).unwrap_or(1),
            None => ctx.buses.get(&bus).map_or(1, |b| [b.0, b.1, b.2][k]),
        }),
        C::Base => num(v.and_then(Value::as_f64).unwrap_or(ctx.sbase)),
    })
}

fn export_maps(
    ctx: &ExCtx,
    table: &str,
    r: &Record,
    maps: &[M],
    raw_suffix: &str,
    stn_prefix: &str,
    out: &mut Fields,
) -> Result<(), String> {
    // Names are only built when a prefix or suffix changes them: this runs
    // for every field of every record when a case is opened.
    let stn_name = |stn: &'static str| -> std::borrow::Cow<'static, str> {
        if stn_prefix.is_empty() {
            stn.into()
        } else {
            format!("{stn_prefix}{stn}").into()
        }
    };
    let bus = match maps.first() {
        Some(M(_, stn, C::Bus)) => ctx.bus(r.get(&stn_name(stn)))?,
        _ => 0,
    };
    out.reserve(maps.len());
    for M(raw, stn, c) in maps {
        let name = if raw_suffix.is_empty() {
            raw.to_string()
        } else {
            format!("{raw}{raw_suffix}")
        };
        out.push((name, export_one(ctx, table, r, &stn_name(stn), *c, bus)?));
    }
    Ok(())
}

fn export_owners(ctx: &ExCtx, table: &str, r: &Record, default: i64, out: &mut Fields) {
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
        list.push((default, 1.0));
    }
    for k in 0..4 {
        let (o, f) = list.get(k).copied().unwrap_or((0, 1.0));
        out.push((format!("O{}", k + 1), int(o)));
        out.push((format!("F{}", k + 1), num(f)));
    }
}

fn raw_kind(v: &RawValue) -> &'static str {
    match v {
        RawValue::Text(_) => "string",
        RawValue::Number(_) => "number",
    }
}

pub(crate) fn export(ctx: &ExCtx, table: &str, r: &Record) -> Result<Item, String> {
    let mut out = Fields::new();
    match table {
        "lcc_line" => {
            export_maps(ctx, table, r, LCC, "", "", &mut out)?;
            export_maps(ctx, table, r, LCC_CONVERTER, "R", "rect_", &mut out)?;
            export_maps(ctx, table, r, LCC_CONVERTER, "I", "inv_", &mut out)?;
            return Ok(rec_item(out));
        }
        "vsc_line" => {
            let mut head = Fields::new();
            export_maps(ctx, table, r, VSC, "", "", &mut head)?;
            export_owners(ctx, table, r, 1, &mut head);
            let mut c1 = Fields::new();
            export_maps(ctx, table, r, VSC_CONVERTER, "", "c1_", &mut c1)?;
            let mut c2 = Fields::new();
            export_maps(ctx, table, r, VSC_CONVERTER, "", "c2_", &mut c2)?;
            let line = |fields: &Fields| -> Vec<(RawValue, &str)> {
                fields
                    .iter()
                    .map(|(n, v)| {
                        let kind = if matches!(n.as_str(), "NAME") {
                            "string"
                        } else {
                            raw_kind(v)
                        };
                        (v.clone(), kind)
                    })
                    .collect()
            };
            return Ok(lines_item(vec![line(&head), line(&c1), line(&c2)]));
        }
        "multi_section_line" => {
            export_maps(ctx, table, r, MULTI_SECTION, "", "", &mut out)?;
            let via = ctx
                .doc
                .value(table, r, "via")
                .and_then(Value::as_list)
                .unwrap_or_default();
            for v in via {
                out.push(("DUM".into(), int(ctx.bus(Some(v))?)));
            }
            let line: Vec<(RawValue, &str)> = out
                .iter()
                .map(|(n, v)| (v.clone(), if n == "ID" { "string" } else { raw_kind(v) }))
                .collect();
            return Ok(lines_item(vec![line]));
        }
        _ => {}
    }
    export_maps(ctx, table, r, maps_of(table), "", "", &mut out)?;
    let bus = ctx
        .bus(r.get(match table {
            "line" | "switch" | "facts" => "from",
            "bus" | "area" | "zone" | "owner" | "area_transfer" => "",
            _ => "bus",
        }))
        .unwrap_or(0);
    if matches!(table, "line" | "switch") {
        for (raw, stn) in RATES {
            out.push((raw.into(), num(ctx.f(table, r, stn))));
        }
    }
    if matches!(table, "generator" | "line") {
        let owner = ctx.buses.get(&bus).map_or(1, |b| b.2);
        export_owners(ctx, table, r, owner, &mut out);
    }
    if table == "switched_shunt" {
        let blocks = ctx
            .doc
            .value(table, r, "blocks")
            .and_then(Value::as_list)
            .unwrap_or_default();
        for (g, b) in blocks.iter().take(8).enumerate() {
            let b = b.as_list().unwrap_or_default();
            let on = b.get(2).and_then(Value::as_str) != Some("off");
            out.push((format!("S{}", g + 1), int(i64::from(on))));
            out.push((
                format!("N{}", g + 1),
                int(b.first().and_then(Value::as_i64).unwrap_or(0)),
            ));
            out.push((
                format!("B{}", g + 1),
                num(b.get(1).and_then(Value::as_f64).unwrap_or(0.0)),
            ));
        }
    }
    Ok(rec_item(out))
}
