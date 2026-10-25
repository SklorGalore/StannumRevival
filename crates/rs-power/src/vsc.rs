//! Detailed two-terminal voltage source converter (VSC) DC line model.
//!
//! One converter holds the DC voltage (RAW `TYPE = 1`) and the other an
//! active power order at its AC bus (`TYPE = 2`, `DCSET` in MW, positive
//! when it feeds the AC system). Units are kV, kA, ohms, MW and Mvar.
//!
//! - Converter loss: `ALOSS + BLOSS · Idc`, at least `MINLOSS` (RAW gives
//!   kW and kW/A).
//! - DC line: `Vk − Vv = RDC · I`, with `I` flowing from the power converter
//!   `k` to the voltage converter `v`, and `Vv = DCSET` of the voltage end.
//! - Power balance at the power converter: `Pk + lossk = −Vk · I`, so the
//!   current solves a quadratic. The voltage converter's AC injection is
//!   `Vv · I − lossv`.
//!
//! The AC side of each converter holds its bus voltage within `MINQ..MAXQ`
//! (`MODE = 1`) or a fixed power factor (`MODE = 2`). When a converter's MVA
//! exceeds `SMAX` or its AC current exceeds `IMAX`, the power order and the
//! reactive power (or its limits, in voltage control) are reduced, weighted
//! by `PWF`: 0 reduces only active power and 1 only reactive power.

/// What a VSC converter controls on its DC side (RAW `TYPE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VscDcControl {
    /// Out of service (`TYPE = 0`).
    OutOfService,
    /// Holds the DC voltage at `DCSET` kV (`TYPE = 1`).
    DcVoltage,
    /// Holds `DCSET` MW at its AC bus (`TYPE = 2`).
    Power,
}

/// What a VSC converter controls on its AC side (RAW `MODE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VscAcControl {
    /// Holds the AC bus voltage at `ACSET` p.u. (`MODE = 1`).
    Voltage,
    /// Holds the power factor `ACSET` (`MODE = 2`).
    PowerFactor,
}

/// One converter of a VSC DC line, with RAW field names.
#[derive(Clone, Debug, PartialEq)]
pub struct VscConverter {
    /// AC bus number (`IBUS`).
    pub bus: u32,
    /// DC-side control (`TYPE`).
    pub dc_control: VscDcControl,
    /// AC-side control (`MODE`).
    pub ac_control: VscAcControl,
    /// DC voltage in kV or power order in MW (`DCSET`).
    pub dc_setpoint: f64,
    /// AC voltage in p.u. or power factor (`ACSET`).
    pub ac_setpoint: f64,
    /// Constant loss in kW (`ALOSS`).
    pub loss_kw: f64,
    /// Loss per DC amp in kW/A (`BLOSS`).
    pub loss_kw_per_amp: f64,
    /// Minimum loss in kW (`MINLOSS`).
    pub min_loss_kw: f64,
    /// MVA rating, or 0 for none (`SMAX`).
    pub smax_mva: f64,
    /// AC current rating in amps, or 0 for none (`IMAX`).
    pub imax_amps: f64,
    /// Power weighting factor for limit reductions, 0 to 1 (`PWF`).
    pub power_weighting: f64,
    /// Reactive limits in Mvar (`MAXQ`, `MINQ`).
    pub qmax: f64,
    pub qmin: f64,
    /// Remotely regulated bus, or 0 for its own (`REMOT`). Not modelled.
    pub remote_bus: u32,
    /// Percent of the remote bus's reactive support (`RMPCT`). Not modelled.
    pub remote_percent: f64,
}

/// A two-terminal VSC DC line with every RAW field.
#[derive(Clone, Debug, PartialEq)]
pub struct VscLine {
    /// The `case.dcline` row this line details.
    pub row: usize,
    /// Line name (`NAME`).
    pub name: String,
    /// In service (`MDC = 1`).
    pub in_service: bool,
    /// DC line resistance in ohms (`RDC`).
    pub resistance: f64,
    /// The two converter records, in RAW order.
    pub converters: [VscConverter; 2],
    /// Which converter is at the `dcline` row's from bus (0 or 1).
    pub from: usize,
}

/// Solved operating point of one VSC DC line, oriented like its `dcline` row.
#[derive(Clone, Debug, PartialEq)]
pub struct VscSolution {
    /// The `case.dcline` row.
    pub row: usize,
    /// DC current from the from end to the to end, in kA.
    pub current_ka: f64,
    /// DC voltage at the from and to converters, in kV.
    pub from_kv: f64,
    pub to_kv: f64,
    /// Active power drawn from the from bus and delivered to the to bus, MW.
    pub from_mw: f64,
    pub to_mw: f64,
    /// Reactive power injected at each AC bus, Mvar.
    pub from_mvar: f64,
    pub to_mvar: f64,
    /// Converter losses at each end, MW.
    pub from_loss_mw: f64,
    pub to_loss_mw: f64,
    /// DC line I²R loss, MW.
    pub line_loss_mw: f64,
    /// Fraction of the power order kept after MVA or current limits.
    pub order_scale: f64,
    /// Whether each end's MVA or current limit reduced its output.
    pub from_limited: bool,
    pub to_limited: bool,
}

/// DC-side solution in RAW converter order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DcState {
    /// Current from converter 0 to converter 1, kA.
    pub current_ka: f64,
    /// DC voltages, kV.
    pub kv: [f64; 2],
    /// AC injections, MW (positive into the AC bus).
    pub injection_mw: [f64; 2],
    /// Converter losses, MW.
    pub loss_mw: [f64; 2],
}

impl VscConverter {
    fn loss(&self, current_ka: f64) -> f64 {
        ((self.loss_kw + self.loss_kw_per_amp * current_ka.abs() * 1000.0) / 1000.0)
            .max(self.min_loss_kw / 1000.0)
    }

    /// Reactive output in power factor control for active power `p`.
    pub(crate) fn power_factor_q(&self, p: f64) -> f64 {
        let pf = self.ac_setpoint.clamp(-1.0, 1.0);
        p.abs() * pf.abs().acos().tan() * pf.signum()
    }

    /// MVA limit at an AC voltage of `kv` kV, or infinity.
    pub(crate) fn mva_limit(&self, kv: f64) -> f64 {
        let mut limit = f64::INFINITY;
        if self.smax_mva > 0.0 {
            limit = self.smax_mva;
        }
        if self.imax_amps > 0.0 && kv > 0.0 {
            limit = limit.min(3f64.sqrt() * kv * self.imax_amps / 1000.0);
        }
        limit
    }
}

impl VscLine {
    /// The converter holding power and the one holding DC voltage.
    fn roles(&self) -> Option<(usize, usize)> {
        use VscDcControl::{DcVoltage, Power};
        match (self.converters[0].dc_control, self.converters[1].dc_control) {
            (Power, DcVoltage) => Some((0, 1)),
            (DcVoltage, Power) => Some((1, 0)),
            _ => None,
        }
    }

    /// Check the data a solve needs.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let name = &self.name;
        if !(self.resistance.is_finite() && self.resistance >= 0.0) || self.from > 1 {
            return Err(format!("VSC line {name}: invalid RDC or from end"));
        }
        if !self.in_service {
            return Ok(());
        }
        let Some((_, v)) = self.roles() else {
            return Err(format!(
                "VSC line {name}: the detailed model needs one DC voltage and one power converter"
            ));
        };
        if self.converters[v].dc_setpoint.is_nan() || self.converters[v].dc_setpoint <= 0.0 {
            return Err(format!(
                "VSC line {name}: DC voltage setpoint must be positive"
            ));
        }
        for c in &self.converters {
            let values = [
                c.dc_setpoint,
                c.ac_setpoint,
                c.loss_kw,
                c.loss_kw_per_amp,
                c.min_loss_kw,
                c.smax_mva,
                c.imax_amps,
                c.power_weighting,
                c.qmax,
                c.qmin,
            ];
            if values.iter().any(|x| x.is_nan() || x.is_infinite())
                || c.loss_kw < 0.0
                || c.loss_kw_per_amp < 0.0
                || c.min_loss_kw < 0.0
                || c.qmin > c.qmax
                || (c.ac_control == VscAcControl::PowerFactor && c.ac_setpoint == 0.0)
                || (c.ac_control == VscAcControl::Voltage && c.ac_setpoint <= 0.0)
            {
                return Err(format!("VSC line {name}: invalid converter {} data", c.bus));
            }
        }
        Ok(())
    }

    /// DC solution with the power order scaled by `scale`.
    pub(crate) fn dc_state(&self, scale: f64) -> Result<DcState, String> {
        let (k, v) = self
            .roles()
            .ok_or_else(|| format!("VSC line {}: no power and DC voltage pair", self.name))?;
        let (ck, cv) = (&self.converters[k], &self.converters[v]);
        let r = self.resistance;
        let vv = cv.dc_setpoint;
        let pk = scale * ck.dc_setpoint;
        // Pk + a + b·|I| + Vv·I + R·I² = 0 with I from k to v; the loss is
        // linear until it reaches its floor.
        let solve = |a: f64, b: f64| -> Option<f64> {
            let c0 = pk + a;
            let lin = if c0 < 0.0 { vv + b } else { vv - b };
            let disc = lin * lin - 4.0 * r * c0;
            (disc >= 0.0 && lin > 0.0).then(|| -2.0 * c0 / (lin + disc.sqrt()))
        };
        let mut current = None;
        for (a, b) in [
            (ck.loss_kw / 1000.0, ck.loss_kw_per_amp),
            (ck.min_loss_kw / 1000.0, 0.0),
        ] {
            if let Some(i) = solve(a, b) {
                // Take the linear loss unless it is below the floor.
                if (ck.loss(i) - (a + b * i.abs())).abs() < 1e-12 {
                    current = Some(i);
                    break;
                }
            }
        }
        let i = current.ok_or_else(|| {
            format!(
                "VSC line {}: the DC line cannot carry the power order",
                self.name
            )
        })?;
        let vk = vv + r * i;
        let (loss_k, loss_v) = (ck.loss(i), cv.loss(i));
        let mut state = DcState {
            current_ka: if k == 0 { i } else { -i },
            kv: [0.0; 2],
            injection_mw: [0.0; 2],
            loss_mw: [0.0; 2],
        };
        state.kv[k] = vk;
        state.kv[v] = vv;
        state.injection_mw[k] = pk;
        state.injection_mw[v] = vv * i - loss_v;
        state.loss_mw[k] = loss_k;
        state.loss_mw[v] = loss_v;
        Ok(state)
    }
}

/// New `(P, Q)` after an MVA limit `limit` with power weighting `pwf`:
/// `P · (1 − (1 − pwf)·t)` and `Q · (1 − pwf·t)` for the smallest `t` that
/// fits, then active power alone if reactive power reaches zero first.
pub(crate) fn reduce(p: f64, q: f64, limit: f64, pwf: f64) -> (f64, f64) {
    if p.hypot(q) <= limit {
        return (p, q);
    }
    let w = pwf.clamp(0.0, 1.0);
    let at = |t: f64| (p * (1.0 - (1.0 - w) * t), q * (1.0 - w * t));
    let (p1, q1) = at(1.0);
    if p1.hypot(q1) > limit {
        // Even the full weighted reduction is not enough: keep its Q and
        // cut P to fit.
        let pmax = (limit * limit - q1 * q1).max(0.0).sqrt();
        return (p.signum() * pmax.min(p.abs()), q1);
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        let (pm, qm) = at(mid);
        if pm.hypot(qm) > limit {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    at(hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converter(bus: u32, dc_control: VscDcControl, dc_setpoint: f64) -> VscConverter {
        VscConverter {
            bus,
            dc_control,
            ac_control: VscAcControl::Voltage,
            dc_setpoint,
            ac_setpoint: 1.0,
            loss_kw: 100.0,
            loss_kw_per_amp: 0.1,
            min_loss_kw: 50.0,
            smax_mva: 0.0,
            imax_amps: 0.0,
            power_weighting: 1.0,
            qmax: 100.0,
            qmin: -100.0,
            remote_bus: 0,
            remote_percent: 100.0,
        }
    }

    fn line(order: f64) -> VscLine {
        VscLine {
            row: 0,
            name: "v".into(),
            in_service: true,
            resistance: 0.71,
            converters: [
                converter(1, VscDcControl::Power, order),
                converter(2, VscDcControl::DcVoltage, 100.0),
            ],
            from: 0,
        }
    }

    #[test]
    fn rectifying_power_converter_balances_losses() {
        let s = line(-209.0).dc_state(1.0).unwrap();
        // Power into DC at the rectifier = sent along the line.
        assert!(s.current_ka > 0.0);
        let line_loss = 0.71 * s.current_ka * s.current_ka;
        let total = -s.injection_mw[0] - s.injection_mw[1];
        assert!((total - line_loss - s.loss_mw[0] - s.loss_mw[1]).abs() < 1e-9);
        assert!((s.kv[0] - 100.0 - 0.71 * s.current_ka).abs() < 1e-12);
        // 0.1 MW + 0.1 MW/kA · I at each end.
        assert!((s.loss_mw[0] - (0.1 + 0.1 * s.current_ka)).abs() < 1e-12);
    }

    #[test]
    fn inverting_power_converter_draws_from_the_voltage_end() {
        let s = line(150.0).dc_state(1.0).unwrap();
        assert!(s.current_ka < 0.0);
        assert!(s.injection_mw[1] < -150.0);
        let line_loss = 0.71 * s.current_ka * s.current_ka;
        let total = -s.injection_mw[0] - s.injection_mw[1];
        assert!((total - line_loss - s.loss_mw[0] - s.loss_mw[1]).abs() < 1e-9);
    }

    #[test]
    fn minimum_loss_floor_applies_at_low_current() {
        let s = line(-1.0).dc_state(1.0).unwrap();
        assert!((s.loss_mw[0] - 0.1 - 0.1 * s.current_ka).abs() < 1e-12);
        let mut l = line(-1.0);
        l.converters[0].min_loss_kw = 500.0;
        let s = l.dc_state(1.0).unwrap();
        assert!((s.loss_mw[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn weighted_reduction_fits_the_limit() {
        let (p, q) = reduce(300.0, 400.0, 400.0, 1.0);
        assert!((p - 300.0).abs() < 1e-9 && (p.hypot(q) - 400.0).abs() < 1e-6);
        let (p, q) = reduce(300.0, 400.0, 400.0, 0.0);
        assert!((q - 400.0).abs() < 1e-9 && p.abs() < 1e-4, "{p} {q}");
        let (p, q) = reduce(300.0, 400.0, 400.0, 0.5);
        assert!(p < 300.0 && q < 400.0 && (p.hypot(q) - 400.0).abs() < 1e-6);
        assert_eq!(reduce(3.0, 4.0, 10.0, 0.5), (3.0, 4.0));
    }
}
