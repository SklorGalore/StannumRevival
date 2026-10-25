//! Detailed multi-terminal LCC DC model with a DC network solve.
//!
//! A multi-terminal DC system has DC buses, resistive DC links between
//! them, and line-commutated converters that each sit between one DC bus
//! (`IDC`) and a second DC bus or ground (`IDC2`). Each converter uses the
//! same six-pulse bridge equations as the two-terminal model in
//! [`crate::dc`], in kV, kA, ohms and MW:
//!
//! - rectifier: `Vd = Vdo · cos α − NB · (3/π · Xc + 2 · Rc) · Id`
//! - inverter: `Vd = Vdo · cos γ − NB · (3/π · Xc − 2 · Rc) · Id`
//!
//! The converter at `VCONV` (and `VCONVN` on the negative pole) holds the DC
//! voltage across it at its `SETVL` kV. Every other converter holds its order:
//! `SETVL` MW of DC power in power control or `SETVL` amps in current
//! control, positive for a rectifier and negative for an inverter. The DC
//! network is then solved by Newton's method on the DC bus voltages, with
//! the voltage converters' currents as extra unknowns, so the voltage
//! converters pick up the balance of the system and the link losses.
//!
//! A converter that would need an angle below its minimum to do this runs at
//! that minimum instead: it becomes a source behind its commutation
//! resistance, carries less than its order, and the voltage converters make
//! up the rest. In power control, a converter DC voltage below `VCMOD` turns
//! every power order into a current order at `SETVL` divided by the voltage
//! converter's `SETVL`. Between AC Newton solves, converter taps step to keep
//! each converter's angle within its range, as in the two-terminal model.
//!
//! Not modelled: the participation factors `DCPF` and rectifier margins
//! `MARG` that share out a reduced order, ground resistance `RGRND` (PSS/E
//! uses it only for fault analysis), and link inductance `LDC`.

use crate::dc::{Bridge, LccControl, LccConverter, band_tap};
use std::collections::HashMap;

/// One converter of a multi-terminal DC system, with RAW field names.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcConverter {
    /// Bridge, angle and tap data (`IB`, `N`, `ANGMX`, `ANGMN`, `RC`, `XC`,
    /// `EBAS`, `TR`, `TAP`, `TPMX`, `TPMN`, `TSTP`). The commutating bus,
    /// transformer and capacitor fields are unused.
    pub converter: LccConverter,
    /// Setpoint (`SETVL`): kV at a voltage-controlling converter, otherwise
    /// MW in power control or amps in current control, positive for a
    /// rectifier and negative for an inverter.
    pub setpoint: f64,
    /// Participation factor (`DCPF`). Not modelled.
    pub participation: f64,
    /// Rectifier margin in per unit of the order (`MARG`). Not modelled.
    pub margin: f64,
    /// Converter code (`CNVCOD`): zero or positive on the positive pole,
    /// negative on the negative pole.
    pub code: i32,
    /// The `case.dcline` row the simplified model gives this converter,
    /// linking it to the voltage converter. Voltage converters have none.
    pub row: Option<usize>,
}

/// A DC bus of a multi-terminal system.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcBus {
    /// DC bus number (`IDC`).
    pub number: u32,
    /// AC converter bus connected here, or 0 (`IB`).
    pub ac_bus: u32,
    /// Second DC bus the converter connects to, or 0 for ground (`IDC2`).
    pub second: u32,
    /// Name (`DCNAME`).
    pub name: String,
    /// Resistance to ground in ohms (`RGRND`). Used by PSS/E only for fault
    /// analysis; not modelled.
    pub ground_resistance: f64,
    /// Area, zone and owner (`AREA`, `ZONE`, `OWNER`).
    pub area: u32,
    pub zone: u32,
    pub owner: u32,
}

/// A DC link between two DC buses.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcLink {
    /// From and to DC bus numbers (`IDC`, `JDC`).
    pub from: u32,
    pub to: u32,
    /// Circuit identifier (`DCCKT`).
    pub circuit: String,
    /// Metered end: 1 for `IDC`, 2 for `JDC` (`MET`, revision 31 and later).
    pub metered_end: u8,
    /// Resistance in ohms (`RDC`).
    pub resistance: f64,
    /// Inductance in mH (`LDC`). Not modelled.
    pub inductance: f64,
}

/// A multi-terminal DC system with every RAW field.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcSystem {
    /// System name or number (`I` or `NAME`).
    pub name: String,
    /// Control mode (`MDC`).
    pub control: LccControl,
    /// AC bus of the positive pole's voltage-controlling converter (`VCONV`).
    pub voltage_bus: u32,
    /// DC voltage in kV below which power orders become current orders (`VCMOD`).
    pub mode_switch_kv: f64,
    /// AC bus of the negative pole's voltage-controlling converter, or 0 (`VCONVN`).
    pub negative_voltage_bus: u32,
    pub converters: Vec<MtdcConverter>,
    pub buses: Vec<MtdcBus>,
    pub links: Vec<MtdcLink>,
}

/// How a converter is operating at the solution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MtdcMode {
    /// Holds the DC voltage across it at its setpoint.
    VoltageControl,
    /// Holds its power or current order.
    OrderControl,
    /// At its minimum angle, short of its voltage or order.
    AngleLimited,
    /// Carries no current.
    Blocked,
}

/// Solved state of one converter.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcConverterSolution {
    /// AC bus number.
    pub bus: u32,
    /// DC bus number (`IDC`).
    pub dc_bus: u32,
    /// The converter's `case.dcline` row, if any.
    pub row: Option<usize>,
    pub mode: MtdcMode,
    /// True when the converter is rectifying, false when inverting.
    pub rectifier: bool,
    /// DC current through the converter in kA.
    pub current_ka: f64,
    /// DC voltage across the converter in kV.
    pub dc_kv: f64,
    /// Firing (rectifier) or extinction (inverter) angle in degrees.
    pub angle: f64,
    /// Tap setting.
    pub tap: f64,
    /// Active power drawn from the AC bus in MW (negative: delivered).
    pub ac_mw: f64,
    /// Reactive power absorbed at the AC bus in Mvar.
    pub ac_mvar: f64,
    /// Commutating resistance loss in MW.
    pub loss_mw: f64,
}

/// Solved voltage of one DC bus.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcBusSolution {
    pub number: u32,
    pub name: String,
    /// Voltage to ground in kV.
    pub kv: f64,
}

/// Solved flow on one DC link.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcLinkSolution {
    pub from: u32,
    pub to: u32,
    pub circuit: String,
    /// Current from the from bus to the to bus in kA.
    pub current_ka: f64,
    /// Power entering the link at the from bus in MW.
    pub from_mw: f64,
    /// Power leaving the link at the to bus in MW.
    pub to_mw: f64,
    pub loss_mw: f64,
}

/// Operating point of one multi-terminal DC system.
#[derive(Clone, Debug, PartialEq)]
pub struct MtdcSolution {
    pub name: String,
    /// True when low DC voltage turned the power orders into current orders.
    pub reduced_voltage: bool,
    /// In the order of [`MtdcSystem::converters`].
    pub converters: Vec<MtdcConverterSolution>,
    /// In the order of [`MtdcSystem::buses`].
    pub buses: Vec<MtdcBusSolution>,
    /// In the order of [`MtdcSystem::links`].
    pub links: Vec<MtdcLinkSolution>,
    /// Total DC link loss in MW.
    pub line_loss_mw: f64,
    /// Total commutating resistance loss in MW.
    pub converter_loss_mw: f64,
}

/// Connections of a validated system, by DC bus index.
#[derive(Clone, Debug)]
pub(crate) struct Topology {
    /// Converter terminals: `IDC` and `IDC2` (`None` for ground).
    a: Vec<usize>,
    b: Vec<Option<usize>>,
    /// +1 on the positive pole, −1 on the negative pole.
    pole: Vec<f64>,
    /// Whether each converter holds the DC voltage.
    voltage: Vec<bool>,
    /// Links as (from, to, conductance in 1/Ω).
    links: Vec<(usize, usize, f64)>,
    /// Starting DC bus voltages in kV.
    start: Vec<f64>,
}

/// What a converter does in the DC network during one solve.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Behaviour {
    /// Holds `s · (Va − Vb)` at this many kV; its current is an unknown.
    Voltage(f64),
    /// Puts this many MW into the DC network (negative: takes it out).
    Power(f64),
    /// Injects this many kA into its `IDC` bus.
    Current(f64),
    /// At minimum angle: `Vd = e − k · Id`, with `r` +1 rectifying, −1 inverting.
    Source { e: f64, k: f64, r: f64 },
    /// No current.
    Off,
}

/// Newton steps allowed for one DC network solve.
const MAX_DC_NEWTON: usize = 50;
/// DC network mismatch tolerance in kA and kV.
const DC_TOLERANCE: f64 = 1e-10;

impl MtdcSystem {
    /// Check the data and resolve the network connections.
    pub(crate) fn topology(&self) -> Result<Topology, String> {
        let name = &self.name;
        let fail = |what: String| Err(format!("multi-terminal DC line {name}: {what}"));
        let mut index = HashMap::new();
        for (k, b) in self.buses.iter().enumerate() {
            if b.number == 0 || index.insert(b.number, k).is_some() {
                return fail(format!("DC bus number {} is zero or repeated", b.number));
            }
        }
        if !self.mode_switch_kv.is_finite() {
            return fail("invalid VCMOD".into());
        }
        let mut links = Vec::with_capacity(self.links.len());
        for l in &self.links {
            let (Some(&f), Some(&t)) = (index.get(&l.from), index.get(&l.to)) else {
                return fail(format!(
                    "DC link {}-{} names an unknown DC bus",
                    l.from, l.to
                ));
            };
            if f == t || !(l.resistance.is_finite() && l.resistance > 0.0) {
                return fail(format!(
                    "DC link {}-{} needs two different buses and a positive resistance",
                    l.from, l.to
                ));
            }
            links.push((f, t, 1.0 / l.resistance));
        }
        let n = self.converters.len();
        let (mut a, mut b, mut pole, mut voltage) = (
            Vec::with_capacity(n),
            Vec::with_capacity(n),
            Vec::with_capacity(n),
            Vec::with_capacity(n),
        );
        let mut seen = HashMap::new();
        for c in &self.converters {
            let bus = c.converter.bus;
            if seen.insert(bus, ()).is_some() {
                return fail(format!("converter bus {bus} is repeated"));
            }
            let conv = &c.converter;
            let nb = f64::from(conv.bridges);
            if !conv.is_valid()
                || !c.setpoint.is_finite()
                || conv.reactance <= 0.0
                || 3.0 / std::f64::consts::PI * conv.reactance * nb <= 2.0 * conv.resistance * nb
            {
                return fail(format!("invalid converter data at bus {bus}"));
            }
            let mut hosts = self.buses.iter().filter(|d| d.ac_bus == bus);
            let (Some(host), None) = (hosts.next(), hosts.next()) else {
                return fail(format!(
                    "converter bus {bus} must be IB of exactly one DC bus"
                ));
            };
            a.push(index[&host.number]);
            b.push(match host.second {
                0 => None,
                second => match index.get(&second) {
                    Some(&k) if second != host.number => Some(k),
                    _ => return fail(format!("DC bus {} has an invalid IDC2", host.number)),
                },
            });
            let s = if c.code < 0 { -1.0 } else { 1.0 };
            pole.push(s);
            let holds = bus == self.voltage_bus
                || (self.negative_voltage_bus != 0 && bus == self.negative_voltage_bus);
            if holds && c.setpoint <= 0.0 {
                return fail(format!(
                    "voltage-controlling converter {bus} needs a positive SETVL in kV"
                ));
            }
            if (bus == self.voltage_bus && s < 0.0)
                || (bus == self.negative_voltage_bus && self.negative_voltage_bus != 0 && s > 0.0)
            {
                return fail(format!(
                    "voltage-controlling converter {bus} is on the wrong pole"
                ));
            }
            voltage.push(holds);
        }
        if !self
            .converters
            .iter()
            .any(|c| c.converter.bus == self.voltage_bus)
        {
            return fail(format!(
                "no converter at its voltage-controlling bus {}",
                self.voltage_bus
            ));
        }
        if self.negative_voltage_bus != 0
            && !self
                .converters
                .iter()
                .any(|c| c.converter.bus == self.negative_voltage_bus)
        {
            return fail(format!(
                "no converter at its negative-pole voltage-controlling bus {}",
                self.negative_voltage_bus
            ));
        }
        // Start each pole's buses at its scheduled voltage, then spread that
        // along the links to buses without a converter.
        let scheduled = |s: f64| self.scheduled_kv(s);
        let mut start = vec![None; self.buses.len()];
        for k in 0..n {
            if b[k].is_none() {
                start[a[k]] = Some(pole[k] * scheduled(pole[k]));
            }
        }
        for k in 0..n {
            if let Some(j) = b[k] {
                start[j].get_or_insert(0.0);
                if start[a[k]].is_none() {
                    start[a[k]] = Some(pole[k] * scheduled(pole[k]));
                }
            }
        }
        loop {
            let mut spread = false;
            for &(f, t, _) in &links {
                match (start[f], start[t]) {
                    (Some(x), None) => start[t] = Some(x),
                    (None, Some(x)) => start[f] = Some(x),
                    _ => continue,
                }
                spread = true;
            }
            if !spread {
                break;
            }
        }
        let Some(start) = start.into_iter().collect::<Option<Vec<_>>>() else {
            return fail("a DC bus has no converter or link".into());
        };
        Ok(Topology {
            a,
            b,
            pole,
            voltage,
            links,
            start,
        })
    }

    /// The scheduled voltage of the pole a converter is on, in kV.
    fn scheduled_kv(&self, pole: f64) -> f64 {
        let bus = if pole < 0.0 && self.negative_voltage_bus != 0 {
            self.negative_voltage_bus
        } else {
            self.voltage_bus
        };
        self.converters
            .iter()
            .find(|c| c.converter.bus == bus)
            .map_or(0.0, |c| c.setpoint)
    }
}

/// The DC network state: bus voltages and the current each converter injects
/// into its `IDC` bus.
struct NetworkState {
    v: Vec<f64>,
    injection: Vec<f64>,
}

/// Solve the DC network for fixed converter behaviours.
fn solve_network(
    topo: &Topology,
    behaviour: &[Behaviour],
    start: &[f64],
) -> Result<NetworkState, String> {
    let n = start.len();
    let held: Vec<usize> = (0..behaviour.len())
        .filter(|&c| matches!(behaviour[c], Behaviour::Voltage(_)))
        .collect();
    let m = n + held.len();
    let mut x: Vec<f64> = start.to_vec();
    x.resize(m, 0.0);
    let across = |x: &[f64], c: usize| x[topo.a[c]] - topo.b[c].map_or(0.0, |j| x[j]);
    for _ in 0..MAX_DC_NEWTON {
        let mut f = vec![0.0; m];
        let mut jac = vec![0.0; m * m];
        for &(i, j, g) in &topo.links {
            let current = g * (x[i] - x[j]);
            f[i] += current;
            f[j] -= current;
            jac[i * m + i] += g;
            jac[i * m + j] -= g;
            jac[j * m + j] += g;
            jac[j * m + i] -= g;
        }
        let mut row_of_held = 0;
        for (c, beh) in behaviour.iter().enumerate() {
            let (a, b) = (topo.a[c], topo.b[c]);
            // Injection into a (and out of b), with its derivatives.
            let (injection, d_across, d_unknown) = match *beh {
                Behaviour::Voltage(kv) => {
                    let k = n + row_of_held;
                    row_of_held += 1;
                    f[k] = topo.pole[c] * across(&x, c) - kv;
                    jac[k * m + a] += topo.pole[c];
                    if let Some(b) = b {
                        jac[k * m + b] -= topo.pole[c];
                    }
                    (x[k], 0.0, Some(k))
                }
                Behaviour::Power(mw) => {
                    let u = across(&x, c);
                    if u.abs() < 1e-6 {
                        return Err("a power-controlled converter has no DC voltage".into());
                    }
                    (mw / u, -mw / (u * u), None)
                }
                Behaviour::Current(ka) => (ka, 0.0, None),
                Behaviour::Source { e, k, r } => {
                    let s = topo.pole[c];
                    let u = across(&x, c);
                    (s * r * e / k - r * u / k, -r / k, None)
                }
                Behaviour::Off => (0.0, 0.0, None),
            };
            f[a] -= injection;
            if let Some(b) = b {
                f[b] += injection;
            }
            let mut add = |row: usize, sign: f64| {
                jac[row * m + a] -= sign * d_across;
                if let Some(b) = b {
                    jac[row * m + b] += sign * d_across;
                }
                if let Some(k) = d_unknown {
                    jac[row * m + k] -= sign;
                }
            };
            add(a, 1.0);
            if let Some(b) = b {
                add(b, -1.0);
            }
        }
        let worst = f.iter().fold(0.0f64, |w, v| w.max(v.abs()));
        if !worst.is_finite() {
            return Err("the DC network solve diverged".into());
        }
        if worst < DC_TOLERANCE {
            let mut injection = vec![0.0; behaviour.len()];
            let mut k = n;
            for (c, beh) in behaviour.iter().enumerate() {
                injection[c] = match *beh {
                    Behaviour::Voltage(_) => {
                        k += 1;
                        x[k - 1]
                    }
                    Behaviour::Power(mw) => mw / across(&x, c),
                    Behaviour::Current(ka) => ka,
                    Behaviour::Source { e, k: kk, r } => {
                        topo.pole[c] * r * e / kk - r * across(&x, c) / kk
                    }
                    Behaviour::Off => 0.0,
                };
            }
            x.truncate(n);
            return Ok(NetworkState { v: x, injection });
        }
        let dx = dense_solve(&mut jac, f, m).ok_or_else(|| {
            "the DC network is singular (a DC bus may have no path to a voltage-controlling converter)"
                .to_string()
        })?;
        for (xi, d) in x.iter_mut().zip(dx) {
            *xi -= d;
        }
    }
    Err("the DC network solve did not converge".into())
}

/// Solve `a · x = b` in place by Gaussian elimination with partial pivoting.
fn dense_solve(a: &mut [f64], mut b: Vec<f64>, m: usize) -> Option<Vec<f64>> {
    let scale = a.iter().fold(0.0f64, |w, v| w.max(v.abs())).max(1e-300);
    for col in 0..m {
        let pivot =
            (col..m).max_by(|&i, &j| a[i * m + col].abs().total_cmp(&a[j * m + col].abs()))?;
        if a[pivot * m + col].abs() <= 1e-13 * scale {
            return None;
        }
        if pivot != col {
            for k in 0..m {
                a.swap(pivot * m + k, col * m + k);
            }
            b.swap(pivot, col);
        }
        for row in col + 1..m {
            let factor = a[row * m + col] / a[col * m + col];
            if factor != 0.0 {
                for k in col..m {
                    a[row * m + k] -= factor * a[col * m + k];
                }
                b[row] -= factor * b[col];
            }
        }
    }
    let mut x = vec![0.0; m];
    for row in (0..m).rev() {
        let sum: f64 = (row + 1..m).map(|k| a[row * m + k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row * m + row];
    }
    Some(x)
}

/// A converter's DC voltage, current and direction from the network state.
fn converter_state(
    topo: &Topology,
    behaviour: Behaviour,
    state: &NetworkState,
    c: usize,
) -> (f64, f64, f64) {
    let s = topo.pole[c];
    let vd = s * (state.v[topo.a[c]] - topo.b[c].map_or(0.0, |j| state.v[j]));
    let flow = s * state.injection[c];
    match behaviour {
        Behaviour::Source { r, .. } => (vd, r * flow, r),
        _ => {
            let r = if flow >= 0.0 { 1.0 } else { -1.0 };
            (vd, flow.abs(), r)
        }
    }
}

/// Operating point of `system` with converter AC voltages `vm` (per unit),
/// base voltages `kv` (kV) and taps, in converter order. `start` gives
/// starting DC bus voltages, such as the previous solution's.
pub(crate) fn operating_point(
    system: &MtdcSystem,
    topo: &Topology,
    vm: &[f64],
    kv: &[f64],
    taps: &[f64],
    start: Option<&[f64]>,
) -> Result<MtdcSolution, String> {
    let n = system.converters.len();
    let bridges: Vec<Bridge> = (0..n)
        .map(|c| Bridge::new(&system.converters[c].converter, vm[c], kv[c], taps[c]))
        .collect();
    let blocked = system.control == LccControl::Blocked;
    let mut behaviour: Vec<Behaviour> = (0..n)
        .map(|c| {
            let conv = &system.converters[c];
            if blocked {
                Behaviour::Off
            } else if topo.voltage[c] {
                Behaviour::Voltage(conv.setpoint)
            } else if system.control == LccControl::Current {
                Behaviour::Current(topo.pole[c] * conv.setpoint / 1000.0)
            } else {
                Behaviour::Power(conv.setpoint)
            }
        })
        .collect();
    let ordered = behaviour.clone();
    let mut reduced = false;
    let mut reverts = vec![0usize; n];
    let start = start
        .filter(|s| s.len() == topo.start.len())
        .unwrap_or(&topo.start);
    let mut state = if behaviour.iter().all(|b| *b == Behaviour::Off) {
        NetworkState {
            v: vec![0.0; topo.start.len()],
            injection: vec![0.0; n],
        }
    } else {
        solve_network(topo, &behaviour, start)?
    };
    for _ in 0..3 * n + 3 {
        let mut changed = false;
        if system.control == LccControl::Power && !reduced && system.mode_switch_kv > 0.0 {
            let low = (0..n).any(|c| {
                behaviour[c] != Behaviour::Off
                    && converter_state(topo, behaviour[c], &state, c).0 < system.mode_switch_kv
            });
            if low {
                reduced = true;
                for c in 0..n {
                    if let Behaviour::Power(mw) = ordered[c] {
                        let current = topo.pole[c] * mw / system.scheduled_kv(topo.pole[c]);
                        if matches!(behaviour[c], Behaviour::Power(_)) {
                            behaviour[c] = Behaviour::Current(current);
                        }
                    }
                }
                changed = true;
            }
        }
        for c in 0..n {
            if changed {
                break;
            }
            let (vd, id, r) = converter_state(topo, behaviour[c], &state, c);
            let br = &bridges[c];
            let k = br.commutation + r * br.resistive;
            match behaviour[c] {
                Behaviour::Off => {}
                Behaviour::Source { .. } => {
                    if id < -1e-12 {
                        behaviour[c] = Behaviour::Off;
                        changed = true;
                    } else if reverts[c] < 2 {
                        // Leave the angle limit when the order (or voltage) is in reach again.
                        let back = match ordered[c] {
                            Behaviour::Voltage(target) => vd > target + 1e-9,
                            Behaviour::Power(mw) => vd > 0.0 && id > mw.abs() / vd * (1.0 + 1e-9),
                            Behaviour::Current(ka) => id > ka.abs() * (1.0 + 1e-9),
                            _ => false,
                        };
                        if back {
                            reverts[c] += 1;
                            behaviour[c] = if reduced {
                                match ordered[c] {
                                    Behaviour::Power(mw) => Behaviour::Current(
                                        topo.pole[c] * mw / system.scheduled_kv(topo.pole[c]),
                                    ),
                                    other => other,
                                }
                            } else {
                                ordered[c]
                            };
                            changed = true;
                        }
                    }
                }
                _ => {
                    let order_sign = match behaviour[c] {
                        Behaviour::Power(mw) => mw.signum(),
                        Behaviour::Current(ka) => (topo.pole[c] * ka).signum(),
                        _ => r,
                    };
                    let reversed = !topo.voltage[c] && order_sign * r < 0.0 && id > 1e-9;
                    let cos = (vd + k * id) / br.vdo;
                    if br.vdo <= 0.0 || vd <= 0.0 || reversed || cos > br.cos_min + 1e-12 {
                        let r = if topo.voltage[c] { r } else { order_sign };
                        let k = br.commutation + r * br.resistive;
                        behaviour[c] = Behaviour::Source {
                            e: br.vdo.max(0.0) * br.cos_min,
                            k,
                            r,
                        };
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
        let from = state.v.clone();
        state = if behaviour.iter().all(|b| *b == Behaviour::Off) {
            NetworkState {
                v: vec![0.0; topo.start.len()],
                injection: vec![0.0; n],
            }
        } else {
            solve_network(topo, &behaviour, &from)?
        };
    }
    let mut out = MtdcSolution {
        name: system.name.clone(),
        reduced_voltage: reduced,
        converters: Vec::with_capacity(n),
        buses: system
            .buses
            .iter()
            .zip(&state.v)
            .map(|(b, &kv)| MtdcBusSolution {
                number: b.number,
                name: b.name.clone(),
                kv,
            })
            .collect(),
        links: Vec::with_capacity(system.links.len()),
        line_loss_mw: 0.0,
        converter_loss_mw: 0.0,
    };
    for (l, &(i, j, g)) in system.links.iter().zip(&topo.links) {
        let current = g * (state.v[i] - state.v[j]);
        let loss = current * current / g;
        out.line_loss_mw += loss;
        out.links.push(MtdcLinkSolution {
            from: l.from,
            to: l.to,
            circuit: l.circuit.clone(),
            current_ka: current,
            from_mw: state.v[i] * current,
            to_mw: state.v[j] * current,
            loss_mw: loss,
        });
    }
    for c in 0..n {
        let conv = &system.converters[c];
        let br = &bridges[c];
        let (vd, id, r) = converter_state(topo, behaviour[c], &state, c);
        let (mode, id) = match behaviour[c] {
            Behaviour::Off => (MtdcMode::Blocked, 0.0),
            _ if id <= 1e-12 => (MtdcMode::Blocked, 0.0),
            Behaviour::Source { .. } => (MtdcMode::AngleLimited, id),
            Behaviour::Voltage(_) => (MtdcMode::VoltageControl, id),
            _ => (MtdcMode::OrderControl, id),
        };
        let k = br.commutation + r * br.resistive;
        let angle = if matches!(mode, MtdcMode::AngleLimited | MtdcMode::Blocked) || br.vdo <= 0.0 {
            conv.converter.angle_min
        } else {
            ((vd + k * id) / br.vdo)
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        };
        let internal = vd + r * br.resistive * id;
        let loss = br.resistive * id * id;
        out.converter_loss_mw += loss;
        out.converters.push(MtdcConverterSolution {
            bus: conv.converter.bus,
            dc_bus: system.buses[topo.a[c]].number,
            row: conv.row,
            mode,
            rectifier: r > 0.0,
            current_ka: id,
            dc_kv: vd,
            angle,
            tap: taps[c],
            ac_mw: r * vd * id + loss,
            ac_mvar: id * (br.vdo * br.vdo - internal * internal).max(0.0).sqrt(),
            loss_mw: loss,
        });
    }
    Ok(out)
}

/// Taps that bring each converter's angle into its range at the latest
/// operating point, holding voltage converters at their setpoints.
pub(crate) fn adjust_taps(
    system: &MtdcSystem,
    topo: &Topology,
    solution: &MtdcSolution,
    vm: &[f64],
    kv: &[f64],
    taps: &[f64],
) -> Vec<f64> {
    let mut next = taps.to_vec();
    if system.control == LccControl::Blocked {
        return next;
    }
    for (c, s) in solution.converters.iter().enumerate() {
        if s.mode == MtdcMode::Blocked {
            continue;
        }
        let conv = &system.converters[c];
        let unit = Bridge::new(&conv.converter, vm[c], kv[c], 1.0);
        let r = if s.rectifier { 1.0 } else { -1.0 };
        let k = unit.commutation + r * unit.resistive;
        let (vd, id) = if topo.voltage[c] {
            (conv.setpoint, s.current_ka)
        } else if s.dc_kv > 0.0 {
            let order = if system.control == LccControl::Current {
                conv.setpoint.abs() / 1000.0
            } else if solution.reduced_voltage {
                conv.setpoint.abs() / system.scheduled_kv(topo.pole[c])
            } else {
                conv.setpoint.abs() / s.dc_kv
            };
            (s.dc_kv, order)
        } else {
            continue;
        };
        let needed = vd + k * id;
        if needed <= 0.0 || unit.vdo <= 0.0 {
            continue;
        }
        let c_conv = &conv.converter;
        let hi = c_conv.angle_max.to_radians();
        let angle = (needed * taps[c] / unit.vdo).clamp(-1.0, 1.0).acos();
        let in_range = s.mode != MtdcMode::AngleLimited && angle <= hi + 1e-9;
        if !in_range {
            next[c] = band_tap(c_conv, unit.vdo, needed, angle);
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converter(bus: u32, setpoint: f64, angle_min: f64) -> MtdcConverter {
        MtdcConverter {
            converter: LccConverter {
                bus,
                bridges: 2,
                angle_max: angle_min + 5.0,
                angle_min,
                resistance: 0.0,
                reactance: 10.0,
                base_kv: 500.0,
                ratio: 0.4,
                tap: 1.0,
                tap_max: 1.5,
                tap_min: 0.5,
                tap_step: 0.00625,
                commutating_bus: 0,
                transformer: None,
                capacitor: 0.0,
            },
            setpoint,
            participation: 1.0,
            margin: 0.0,
            code: 1,
            row: None,
        }
    }

    fn bus(number: u32, ac_bus: u32) -> MtdcBus {
        MtdcBus {
            number,
            ac_bus,
            second: 0,
            name: String::new(),
            ground_resistance: 0.0,
            area: 1,
            zone: 1,
            owner: 1,
        }
    }

    fn link(from: u32, to: u32, resistance: f64) -> MtdcLink {
        MtdcLink {
            from,
            to,
            circuit: "1".into(),
            metered_end: 1,
            resistance,
            inductance: 0.0,
        }
    }

    /// Two rectifiers and the voltage-holding inverter on a star.
    fn star() -> MtdcSystem {
        MtdcSystem {
            name: "star".into(),
            control: LccControl::Power,
            voltage_bus: 3,
            mode_switch_kv: 0.0,
            negative_voltage_bus: 0,
            converters: vec![
                converter(1, 200.0, 5.0),
                converter(2, 100.0, 5.0),
                converter(3, 500.0, 15.0),
            ],
            buses: vec![bus(1, 1), bus(2, 2), bus(3, 3), bus(4, 0)],
            links: vec![link(1, 4, 10.0), link(2, 4, 10.0), link(3, 4, 10.0)],
        }
    }

    fn solve(system: &MtdcSystem, vm: &[f64], taps: &[f64]) -> MtdcSolution {
        let topo = system.topology().unwrap();
        let kv = vec![500.0; system.converters.len()];
        operating_point(system, &topo, vm, &kv, taps, None).unwrap()
    }

    #[test]
    fn orders_hold_and_the_voltage_converter_takes_the_balance() {
        let system = star();
        let s = solve(&system, &[1.0; 3], &[0.9, 0.9, 1.0]);
        let [a, b, v] = &s.converters[..] else {
            panic!()
        };
        assert_eq!(a.mode, MtdcMode::OrderControl);
        assert!((a.dc_kv * a.current_ka - 200.0).abs() < 1e-6);
        assert!((b.dc_kv * b.current_ka - 100.0).abs() < 1e-6);
        assert_eq!(v.mode, MtdcMode::VoltageControl);
        assert!(!v.rectifier);
        assert!((v.dc_kv - 500.0).abs() < 1e-9);
        // DC power balance: what the rectifiers send arrives less link losses.
        let sent = a.dc_kv * a.current_ka + b.dc_kv * b.current_ka;
        assert!((sent - v.dc_kv * v.current_ka - s.line_loss_mw).abs() < 1e-6);
        // Current balance at the star bus.
        assert!((a.current_ka + b.current_ka - v.current_ka).abs() < 1e-9);
        // Link losses by hand: I²R on each leg.
        let hand = 10.0 * (a.current_ka.powi(2) + b.current_ka.powi(2) + v.current_ka.powi(2));
        assert!((s.line_loss_mw - hand).abs() < 1e-9);
        // AC side: the inverter delivers what the rectifiers draw less losses.
        let ac: f64 = s.converters.iter().map(|c| c.ac_mw).sum();
        assert!((ac - s.line_loss_mw - s.converter_loss_mw).abs() < 1e-6);
        assert!(s.converters.iter().all(|c| c.ac_mvar > 0.0));
    }

    #[test]
    fn rectifier_short_of_voltage_runs_at_minimum_angle() {
        let system = star();
        // A weak AC voltage at bus 1 reaches the DC voltage at α = 5°, but
        // not with the drop its full order needs.
        let vm = [0.86, 1.0, 1.0];
        let s = solve(&system, &vm, &[0.9, 0.9, 1.0]);
        let a = &s.converters[0];
        assert_eq!(a.mode, MtdcMode::AngleLimited);
        assert!(a.dc_kv * a.current_ka < 200.0);
        assert!((a.angle - 5.0).abs() < 1e-12);
        // The voltage converter still holds the DC voltage.
        assert!((s.converters[2].dc_kv - 500.0).abs() < 1e-9);
        // Taps then move to bring α back into range.
        let topo = system.topology().unwrap();
        let next = adjust_taps(&system, &topo, &s, &vm, &[500.0; 3], &[0.9, 0.9, 1.0]);
        assert!(next[0] < 0.9);
        let s = solve(&system, &vm, &next);
        assert_eq!(s.converters[0].mode, MtdcMode::OrderControl);
        assert!(s.converters[0].angle >= 5.0 - 1e-9);
    }

    #[test]
    fn low_voltage_switches_power_orders_to_current() {
        let mut system = star();
        system.mode_switch_kv = 600.0;
        let s = solve(&system, &[1.0; 3], &[0.9, 0.9, 1.0]);
        assert!(s.reduced_voltage);
        assert!((s.converters[0].current_ka - 200.0 / 500.0).abs() < 1e-9);
    }

    #[test]
    fn current_control_and_blocked_systems() {
        let mut system = star();
        system.control = LccControl::Current;
        system.converters[0].setpoint = 400.0;
        system.converters[1].setpoint = 200.0;
        let s = solve(&system, &[1.0; 3], &[0.9, 0.9, 1.0]);
        assert!((s.converters[0].current_ka - 0.4).abs() < 1e-9);
        assert!((s.converters[2].current_ka - 0.6).abs() < 1e-9);
        system.control = LccControl::Blocked;
        let s = solve(&system, &[1.0; 3], &[0.9, 0.9, 1.0]);
        assert!(
            s.converters
                .iter()
                .all(|c| c.mode == MtdcMode::Blocked && c.ac_mw == 0.0)
        );
    }

    #[test]
    fn rejects_bad_topology() {
        let mut system = star();
        system.buses[0].ac_bus = 0;
        assert!(system.topology().is_err());
        let mut system = star();
        system.links[0].resistance = 0.0;
        assert!(system.topology().is_err());
        let mut system = star();
        system.voltage_bus = 7;
        assert!(system.topology().is_err());
    }

    #[test]
    fn dense_solve_matches_hand_solution() {
        let mut a = vec![2.0, 1.0, 1.0, 3.0];
        let x = dense_solve(&mut a, vec![3.0, 5.0], 2).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-12 && (x[1] - 1.4).abs() < 1e-12);
        let mut singular = vec![1.0, 2.0, 2.0, 4.0];
        assert!(dense_solve(&mut singular, vec![1.0, 2.0], 2).is_none());
    }
}
