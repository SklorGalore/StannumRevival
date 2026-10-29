//! Detailed two-terminal line-commutated (LCC) DC line model.
//!
//! Each converter is a six-pulse bridge model in PSS/E units (kV, kA, ohms,
//! MW, Mvar). With `E` the AC bus voltage in kV (per-unit voltage times the
//! converter's base kV), `NB` bridges, transformer ratio `TR` and tap `TAP`:
//!
//! - no-load DC voltage: `Vdo = (3√2/π) · NB · E · TR / TAP`
//! - rectifier: `Vdr = Vdo · cos α − NB · (3/π · Xc + 2 · Rc) · Id`
//! - inverter: `Vdi = Vdo · cos γ − NB · (3/π · Xc − 2 · Rc) · Id`
//! - line: `Vdr − Vdi = RDC · Id`
//!
//! `Rc` is the commutating transformer resistance; two phases conduct at a
//! time, so each bridge loses `2 · Rc · Id²`. Reactive power uses the usual
//! power-factor approximation `cos φ = Vd′ / Vdo`, where `Vd′` is the DC
//! voltage before the resistive drop, so `Q = Id · √(Vdo² − Vd′²)`. Both
//! converters absorb reactive power.
//!
//! Controls follow the PSS/E data model: the rectifier holds the power or
//! current order with its firing angle α, and the inverter runs at its
//! minimum extinction angle γ. When the rectifier reaches its minimum α the
//! inverter takes over current control at the order reduced by the margin
//! `DELTI`. In power control, a compounded DC voltage below `VCMOD` switches
//! the order to current at `SETVL / VSCHD`. Between Newton solves the
//! converter taps step within their limits: the inverter tap brings the
//! compounded DC voltage to `VSCHD` and the rectifier tap keeps α within its
//! nominal range.

const BRIDGE: f64 = 3.0 * std::f64::consts::SQRT_2 / std::f64::consts::PI;
const COMMUTATION: f64 = 3.0 / std::f64::consts::PI;

/// Converter end of a two-terminal DC line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LccEnd {
    /// Sending end.
    Rectifier,
    /// Receiving end.
    Inverter,
}

/// Control mode of a two-terminal DC line (RAW `MDC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LccControl {
    /// Out of service (`MDC = 0`).
    Blocked,
    /// Power order in MW (`MDC = 1`).
    Power,
    /// Current order in amps (`MDC = 2`).
    Current,
}

/// One converter of a two-terminal LCC line, with RAW field names.
#[derive(Clone, Debug, PartialEq)]
pub struct LccConverter {
    /// AC bus number (`IPR`, `IPI`).
    pub bus: u32,
    /// Six-pulse bridges in series (`NBR`, `NBI`).
    pub bridges: u32,
    /// Nominal maximum firing (rectifier) or extinction (inverter) angle in
    /// degrees (`ANMXR`, `ANMXI`). The tap keeps the angle below it.
    pub angle_max: f64,
    /// Minimum steady-state angle in degrees (`ANMNR`, `ANMNI`).
    pub angle_min: f64,
    /// Commutating transformer resistance per bridge in ohms (`RCR`, `RCI`).
    pub resistance: f64,
    /// Commutating transformer reactance per bridge in ohms (`XCR`, `XCI`).
    pub reactance: f64,
    /// Primary base AC voltage in kV (`EBASR`, `EBASI`).
    pub base_kv: f64,
    /// Transformer ratio (`TRR`, `TRI`).
    pub ratio: f64,
    /// Tap setting (`TAPR`, `TAPI`).
    pub tap: f64,
    /// Maximum tap (`TMXR`, `TMXI`).
    pub tap_max: f64,
    /// Minimum tap (`TMNR`, `TMNI`).
    pub tap_min: f64,
    /// Tap step; zero means a continuous tap (`STPR`, `STPI`).
    pub tap_step: f64,
    /// Commutating bus, or 0 for the converter bus (`ICR`, `ICI`).
    pub commutating_bus: u32,
    /// AC transformer whose ratio carries the tap: from bus, to bus and
    /// circuit (`IFR`, `ITR`, `IDR`). Not modelled; the tap is applied
    /// inside the converter.
    pub transformer: Option<(u32, u32, String)>,
    /// Series capacitor reactance in ohms for capacitor-commutated
    /// converters (`XCAPR`, `XCAPI`). Not modelled.
    pub capacitor: f64,
}

/// A two-terminal LCC DC line with every RAW field.
#[derive(Clone, Debug, PartialEq)]
pub struct LccLine {
    /// The `case.dcline` row this line details.
    pub row: usize,
    /// Line name (`NAME`).
    pub name: String,
    /// Control mode (`MDC`).
    pub control: LccControl,
    /// DC line resistance in ohms (`RDC`).
    pub resistance: f64,
    /// Order (`SETVL`): MW in power control, amps in current control.
    /// Positive orders apply at the rectifier, negative ones at the inverter.
    pub setpoint: f64,
    /// Scheduled compounded DC voltage in kV (`VSCHD`).
    pub scheduled_kv: f64,
    /// Compounded DC voltage in kV below which power control switches to
    /// current control (`VCMOD`).
    pub mode_switch_kv: f64,
    /// Compounding resistance in ohms (`RCOMP`): the DC voltage is held at
    /// this distance from the inverter towards the rectifier.
    pub compounding_resistance: f64,
    /// Current margin in per unit of the order (`DELTI`).
    pub current_margin: f64,
    /// Metered end (`METER`).
    pub metered_end: LccEnd,
    /// Minimum compounded DC voltage in kV (`DCVMIN`). Not modelled.
    pub min_compounded_kv: f64,
    /// Capacitor-commutated converter iteration limit (`CCCITMX`).
    pub ccc_iterations: u32,
    /// Capacitor-commutated converter acceleration factor (`CCCACC`).
    pub ccc_acceleration: f64,
    /// Sending converter.
    pub rectifier: LccConverter,
    /// Receiving converter.
    pub inverter: LccConverter,
}

/// Which control holds the DC current at the solution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LccMode {
    /// The rectifier holds the order; the inverter is at minimum γ.
    RectifierControl,
    /// The rectifier is at minimum α; the inverter holds the order less the
    /// current margin.
    InverterControl,
    /// Both converters are at their minimum angles and the current follows
    /// from the AC voltages.
    AngleLimited,
    /// No current flows: the line is out of service or cannot transfer.
    Blocked,
}

/// Solved operating point of one two-terminal LCC line.
#[derive(Clone, Debug, PartialEq)]
pub struct LccSolution {
    /// The `case.dcline` row.
    pub row: usize,
    /// Control holding the current.
    pub mode: LccMode,
    /// True when low DC voltage switched a power order to current control.
    pub reduced_voltage: bool,
    /// DC current in kA.
    pub current_ka: f64,
    /// Rectifier DC voltage in kV.
    pub rectifier_kv: f64,
    /// Inverter DC voltage in kV.
    pub inverter_kv: f64,
    /// Rectifier firing angle in degrees.
    pub alpha: f64,
    /// Inverter extinction angle in degrees.
    pub gamma: f64,
    /// Rectifier tap.
    pub rectifier_tap: f64,
    /// Inverter tap.
    pub inverter_tap: f64,
    /// Active power drawn from the rectifier's AC bus in MW.
    pub rectifier_mw: f64,
    /// Reactive power absorbed at the rectifier's AC bus in Mvar.
    pub rectifier_mvar: f64,
    /// Active power delivered to the inverter's AC bus in MW.
    pub inverter_mw: f64,
    /// Reactive power absorbed at the inverter's AC bus in Mvar.
    pub inverter_mvar: f64,
    /// DC line I²R loss in MW.
    pub line_loss_mw: f64,
    /// Commutating resistance loss of both converters in MW.
    pub converter_loss_mw: f64,
}

impl LccLine {
    /// Check the data a solve needs.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let name = &self.name;
        if !(self.resistance.is_finite() && self.resistance >= 0.0) {
            return Err(format!(
                "DC line {name}: RDC must be finite and non-negative"
            ));
        }
        if self.control == LccControl::Blocked {
            return Ok(());
        }
        if !self.setpoint.is_finite() || !self.current_margin.is_finite() {
            return Err(format!("DC line {name}: invalid order or current margin"));
        }
        if self.control == LccControl::Power
            && (self.scheduled_kv.is_nan() || self.scheduled_kv <= 0.0)
        {
            return Err(format!(
                "DC line {name}: power control needs a positive VSCHD"
            ));
        }
        if !self.compounding_resistance.is_finite() || !self.mode_switch_kv.is_finite() {
            return Err(format!("DC line {name}: invalid RCOMP or VCMOD"));
        }
        for (end, c) in [("rectifier", &self.rectifier), ("inverter", &self.inverter)] {
            if !c.is_valid() {
                return Err(format!("DC line {name}: invalid {end} converter data"));
            }
        }
        Ok(())
    }
}

impl LccConverter {
    /// Whether the bridge, angle and tap data can be solved.
    pub(crate) fn is_valid(&self) -> bool {
        let c = self;
        let finite = [
            c.angle_max,
            c.angle_min,
            c.resistance,
            c.reactance,
            c.ratio,
            c.tap,
            c.tap_max,
            c.tap_min,
            c.tap_step,
        ]
        .iter()
        .all(|x| x.is_finite());
        finite
            && c.bridges > 0
            && c.ratio > 0.0
            && c.tap > 0.0
            && c.tap_min > 0.0
            && c.tap_min <= c.tap_max
            && c.tap_step >= 0.0
            && c.resistance >= 0.0
            && c.reactance >= 0.0
            && (0.0..90.0).contains(&c.angle_min)
            && c.angle_max >= c.angle_min
            && c.angle_max < 90.0
            && c.base_kv.is_finite()
            && c.base_kv >= 0.0
    }
}

/// Per-converter constants for one operating-point evaluation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bridge {
    /// No-load DC voltage in kV.
    pub(crate) vdo: f64,
    /// Commutation drop per kA: `NB · 3/π · Xc`.
    pub(crate) commutation: f64,
    /// Resistive drop per kA: `NB · 2 · Rc`.
    pub(crate) resistive: f64,
    /// Cosine of the minimum angle.
    pub(crate) cos_min: f64,
}

impl Bridge {
    pub(crate) fn new(c: &LccConverter, vm: f64, base_kv: f64, tap: f64) -> Self {
        let nb = f64::from(c.bridges);
        Self {
            vdo: BRIDGE * nb * vm * base_kv * c.ratio / tap,
            commutation: nb * COMMUTATION * c.reactance,
            resistive: nb * 2.0 * c.resistance,
            cos_min: c.angle_min.to_radians().cos(),
        }
    }

    /// AC-side active and absorbed reactive power at internal DC voltage `vd`.
    fn powers(&self, vd: f64, id: f64) -> (f64, f64) {
        (
            vd * id,
            id * (self.vdo * self.vdo - vd * vd).max(0.0).sqrt(),
        )
    }
}

/// Smallest positive root of `c2·x² + c1·x − p = 0` for `p ≥ 0` and `c1 > 0`,
/// or the vertex when no real root exists (the most power the line can carry).
fn order_current(c2: f64, c1: f64, p: f64) -> f64 {
    let disc = c1 * c1 + 4.0 * c2 * p;
    if disc < 0.0 {
        -c1 / (2.0 * c2)
    } else {
        2.0 * p / (c1 + disc.sqrt())
    }
}

/// Converter taps in use, rectifier then inverter.
pub(crate) type Taps = [f64; 2];

/// Values the tap controls aim at, from the latest operating point.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TapTargets {
    /// Current order in kA (after any voltage mode switch).
    order_ka: f64,
    /// Rectifier internal voltage `Vdr + NB·(3/π·Xc + 2·Rc)·Id` the order needs, in kV.
    rectifier_internal_kv: f64,
}

/// Operating point of `line` at AC voltage magnitudes `vr` and `vi` (per unit)
/// with converter base voltages `kv` (kV, rectifier then inverter).
pub(crate) fn operating_point(
    line: &LccLine,
    vr: f64,
    vi: f64,
    kv: [f64; 2],
    taps: Taps,
) -> (LccSolution, TapTargets) {
    let rect = Bridge::new(&line.rectifier, vr, kv[0], taps[0]);
    let inv = Bridge::new(&line.inverter, vi, kv[1], taps[1]);
    let r = line.resistance;
    let mut out = LccSolution {
        row: line.row,
        mode: LccMode::Blocked,
        reduced_voltage: false,
        current_ka: 0.0,
        rectifier_kv: 0.0,
        inverter_kv: 0.0,
        alpha: line.rectifier.angle_min,
        gamma: line.inverter.angle_min,
        rectifier_tap: taps[0],
        inverter_tap: taps[1],
        rectifier_mw: 0.0,
        rectifier_mvar: 0.0,
        inverter_mw: 0.0,
        inverter_mvar: 0.0,
        line_loss_mw: 0.0,
        converter_loss_mw: 0.0,
    };
    let mut targets = TapTargets::default();
    let order = line.setpoint.abs();
    if line.control == LccControl::Blocked || order == 0.0 {
        return (out, targets);
    }
    // Normal operation: inverter at minimum γ, so Vdi = a − bi·Id.
    let a = inv.vdo * inv.cos_min;
    let bi = inv.commutation - inv.resistive;
    let br = rect.commutation + rect.resistive;
    if a <= 0.0 {
        return (out, targets);
    }
    let mut id = match line.control {
        LccControl::Current => order / 1000.0,
        _ if line.setpoint > 0.0 => order_current(r - bi, a, order),
        _ => order_current(-bi, a, order),
    };
    if line.control == LccControl::Power {
        let compounded = a - bi * id + line.compounding_resistance * id;
        if compounded < line.mode_switch_kv {
            id = order / line.scheduled_kv;
            out.reduced_voltage = true;
        }
    }
    targets.order_ka = id;
    let vdi = a - bi * id;
    let vdr = vdi + r * id;
    targets.rectifier_internal_kv = vdr + br * id;
    let cos_alpha = targets.rectifier_internal_kv / rect.vdo;
    let rectifier_holds = rect.vdo > 0.0 && vdi > 0.0 && cos_alpha <= rect.cos_min;
    let (id, vdr, vdi, cos_alpha, cos_gamma, mode) = if rectifier_holds {
        (
            id,
            vdr,
            vdi,
            cos_alpha,
            inv.cos_min,
            LccMode::RectifierControl,
        )
    } else {
        // Rectifier at minimum α: the inverter holds the reduced order with γ.
        let id_margin = id * (1.0 - line.current_margin);
        let vdr = rect.vdo * rect.cos_min - br * id_margin;
        let vdi = vdr - r * id_margin;
        let cos_gamma = (vdi + bi * id_margin) / inv.vdo;
        if cos_gamma <= inv.cos_min && vdi > 0.0 {
            (
                id_margin,
                vdr,
                vdi,
                rect.cos_min,
                cos_gamma,
                LccMode::InverterControl,
            )
        } else {
            // Both converters at minimum angles.
            let denominator = r + br - bi;
            let id = if denominator > 0.0 {
                ((rect.vdo * rect.cos_min - a) / denominator).max(0.0)
            } else {
                0.0
            };
            let vdr = rect.vdo * rect.cos_min - br * id;
            let vdi = a - bi * id;
            let mode = if id > 0.0 {
                LccMode::AngleLimited
            } else {
                LccMode::Blocked
            };
            (id, vdr, vdi, rect.cos_min, inv.cos_min, mode)
        }
    };
    if mode == LccMode::Blocked {
        return (out, targets);
    }
    let (pr, qr) = rect.powers(vdr + rect.resistive * id, id);
    let (pi, qi) = inv.powers(vdi - inv.resistive * id, id);
    out.mode = mode;
    out.current_ka = id;
    out.rectifier_kv = vdr;
    out.inverter_kv = vdi;
    out.alpha = cos_alpha.clamp(-1.0, 1.0).acos().to_degrees();
    out.gamma = cos_gamma.clamp(-1.0, 1.0).acos().to_degrees();
    out.rectifier_mw = pr;
    out.rectifier_mvar = qr;
    out.inverter_mw = pi;
    out.inverter_mvar = qi;
    out.line_loss_mw = r * id * id;
    out.converter_loss_mw = (rect.resistive + inv.resistive) * id * id;
    (out, targets)
}

/// PSS/E's default converter tap step (`STP`).
pub(crate) const DEFAULT_TAP_STEP: f64 = 0.00625;

/// Round `tap` to the converter's tap steps and limits.
pub(crate) fn step_tap(c: &LccConverter, tap: f64, round: fn(f64) -> f64) -> f64 {
    // The limits themselves are reachable even when they are not a whole
    // number of steps apart.
    if tap >= c.tap_max {
        c.tap_max
    } else if tap <= c.tap_min {
        c.tap_min
    } else if c.tap_step > 0.0 {
        (c.tap_min + round((tap - c.tap_min) / c.tap_step) * c.tap_step).clamp(c.tap_min, c.tap_max)
    } else {
        tap
    }
}

/// Taps that move the inverter's compounded voltage to `VSCHD` and the
/// rectifier's α into its nominal range, given the latest operating point.
pub(crate) fn adjust_taps(
    line: &LccLine,
    solution: &LccSolution,
    targets: &TapTargets,
    vm: [f64; 2],
    kv: [f64; 2],
    taps: Taps,
) -> Taps {
    if line.control == LccControl::Blocked || targets.order_ka <= 0.0 {
        return taps;
    }
    let id = targets.order_ka;
    let mut next = taps;
    // Inverter: hold VSCHD at the compounding point with γ at minimum.
    let inv = &line.inverter;
    let unit = Bridge::new(inv, vm[1], kv[1], 1.0);
    let wanted = line.scheduled_kv - line.compounding_resistance * id;
    let internal = wanted + (unit.commutation - unit.resistive) * id;
    if line.scheduled_kv > 0.0 && internal > 0.0 {
        next[1] = step_tap(inv, unit.vdo * unit.cos_min / internal, f64::round);
    }
    // Rectifier: keep α within its range at the current needed.
    let rect = &line.rectifier;
    let unit = Bridge::new(rect, vm[0], kv[0], 1.0);
    let needed = if solution.mode == LccMode::RectifierControl {
        solution.rectifier_kv + (unit.commutation + unit.resistive) * solution.current_ka
    } else {
        targets.rectifier_internal_kv
    };
    if needed > 0.0 && unit.vdo > 0.0 {
        let cos_alpha = needed / (unit.vdo / taps[0]);
        let hi = rect.angle_max.to_radians();
        let alpha = cos_alpha.clamp(-1.0, 1.0).acos();
        let in_range = solution.mode == LccMode::RectifierControl && alpha <= hi + 1e-9;
        if !in_range {
            next[0] = band_tap(rect, unit.vdo, needed, alpha);
        }
    }
    next
}

/// The tap that brings a converter's angle (now `angle` radians, with the
/// converter needing `needed` kV of internal voltage from `vdo` kV of no-load
/// voltage at tap 1) into its range. A larger tap lowers Vdo and so the
/// angle. The result is rounded towards the inside of the range; when one
/// tap step is wider than the range, the angle stays above its hard minimum
/// and may end a little above its nominal maximum.
pub(crate) fn band_tap(c: &LccConverter, vdo: f64, needed: f64, angle: f64) -> f64 {
    let (lo, hi) = (c.angle_min.to_radians(), c.angle_max.to_radians());
    let target = angle.clamp(lo, hi);
    let exact = vdo * target.cos() / needed;
    let down = step_tap(c, exact, f64::floor);
    if angle > hi && hi > lo {
        let up = step_tap(c, exact, f64::ceil);
        let reached = (needed * up / vdo).clamp(-1.0, 1.0).acos();
        if reached >= lo - 1e-9 {
            return up;
        }
    }
    down
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converter(bus: u32, base_kv: f64) -> LccConverter {
        LccConverter {
            bus,
            bridges: 2,
            angle_max: 20.0,
            angle_min: 15.0,
            resistance: 0.0,
            reactance: 10.0,
            base_kv,
            ratio: 0.4,
            tap: 1.0,
            tap_max: 1.5,
            tap_min: 0.5,
            tap_step: 0.00625,
            commutating_bus: 0,
            transformer: None,
            capacitor: 0.0,
        }
    }

    fn line() -> LccLine {
        LccLine {
            row: 0,
            name: "test".into(),
            control: LccControl::Power,
            resistance: 10.0,
            setpoint: 500.0,
            scheduled_kv: 250.0,
            mode_switch_kv: 0.0,
            compounding_resistance: 0.0,
            current_margin: 0.1,
            metered_end: LccEnd::Inverter,
            min_compounded_kv: 0.0,
            ccc_iterations: 0,
            ccc_acceleration: 0.0,
            rectifier: converter(1, 500.0),
            inverter: converter(2, 500.0),
        }
    }

    #[test]
    fn rectifier_holds_power_order_and_both_ends_absorb() {
        let l = line();
        let (s, _) = operating_point(&l, 1.0, 1.0, [500.0, 500.0], [0.8, 1.0]);
        assert_eq!(s.mode, LccMode::RectifierControl);
        assert!((s.rectifier_mw - 500.0).abs() < 1e-9);
        assert!((s.rectifier_mw - s.inverter_mw - s.line_loss_mw).abs() < 1e-9);
        assert!(s.rectifier_mvar > 0.0 && s.inverter_mvar > 0.0);
        assert!(s.alpha >= 15.0);
        assert!((s.gamma - 15.0).abs() < 1e-9);
    }

    #[test]
    fn rectifier_at_minimum_alpha_hands_current_to_inverter() {
        let l = line();
        // A weak rectifier AC voltage cannot reach the needed DC voltage.
        let (s, _) = operating_point(&l, 0.8, 1.0, [500.0, 500.0], [1.0, 1.0]);
        assert_eq!(s.mode, LccMode::InverterControl);
        assert!((s.alpha - 15.0).abs() < 1e-9);
        assert!(s.gamma > 15.0);
    }

    #[test]
    fn taps_bring_alpha_into_range_and_voltage_to_schedule() {
        let mut l = line();
        l.scheduled_kv = 450.0;
        let kv = [500.0, 500.0];
        let mut taps = [1.0, 1.0];
        let mut s = operating_point(&l, 1.0, 1.0, kv, taps).0;
        for _ in 0..10 {
            let (sol, targets) = operating_point(&l, 1.0, 1.0, kv, taps);
            let next = adjust_taps(&l, &sol, &targets, [1.0, 1.0], kv, taps);
            s = sol;
            if next == taps {
                break;
            }
            taps = next;
        }
        assert_eq!(s.mode, LccMode::RectifierControl);
        assert!(
            s.alpha >= 15.0 - 1e-9 && s.alpha <= 20.0 + 1e-9,
            "{}",
            s.alpha
        );
        // One inverter tap step moves Vdi by well under 1%.
        assert!((s.inverter_kv - 450.0).abs() < 4.0, "{}", s.inverter_kv);
    }
}
