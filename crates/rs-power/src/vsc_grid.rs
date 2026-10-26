//! Meshed and multi-terminal VSC DC grids, solved in sequence with the AC
//! network.
//!
//! The model and the sequential method follow J. Beerten, S. Cole and
//! R. Belmans, "Generalized steady-state VSC MTDC model for sequential AC/DC
//! power flow algorithms", IEEE Trans. Power Syst. 27(2), 2012. Equation
//! numbers below refer to that paper.
//!
//! # Converter station
//!
//! Each converter is a controllable voltage `Uc` behind a phase reactor
//! `Zc`, a filter susceptance `Bf` at the filter bus `Uf`, and a transformer
//! `Ztf` to the AC system bus `Us` (Fig. 2). Any of the three may be left
//! out. Set-points are defined at the system bus, as in practice:
//! `Ss = Ps + jQs` is the power the station injects into the AC grid.
//!
//! - `Is = conj(Ss / Us)`, `Uf = Us + Ztf · Is` (36)
//! - `Ic = Is + jBf · Uf` (37), `Uc = Uf + Zc · Ic` (38)
//! - `Sc = Uc · conj(Ic)` is the power leaving the converter towards the AC
//!   side, (3)–(4).
//! - Loss `Ploss = a + b · Ic + c · Ic²` with `Ic` in kA (10)–(11): `a` in
//!   MW, `b` in kV and `c` in ohms. `c` takes the rectifier value when the
//!   converter takes power from the AC grid (`Pc < 0`) and the inverter value
//!   when it delivers power to it.
//! - DC power injected into the DC grid: `Pdc = −Pc − Ploss` (39).
//!
//! # DC grid
//!
//! Lines are resistances between DC buses. With `p` poles (1 for an
//! asymmetric monopole, 2 for a symmetric monopole or bipole) the power a
//! bus sends into the grid is `P_i = p · U_i · Σ_j (U_i − U_j) / R_ij` (30).
//! Voltages are pole-to-ground and resistances are per pole conductor.
//!
//! On its DC side each converter is one of:
//!
//! - **Power**: holds `Ps` at its AC system bus.
//! - **Slack**: holds its DC bus voltage; its power balances the grid.
//! - **Droop**: its DC-side power output follows its DC bus voltage,
//!   `−Pdc = P_set + k · (U − U_set)`, with `k` in MW per kV.
//!
//! On its AC side it holds a reactive power `Qs` or the AC voltage at its
//! system bus. A voltage-controlling converter is a PV bus in the AC solve.
//!
//! # Sequential solve (Fig. 5)
//!
//! 1. Solve the AC network with each converter as a fixed `Ps`, `Qs`
//!    injection (or a PV bus). Slack converters start from a lossless
//!    estimate (35), droop converters from their power set-point.
//! 2. Check the converter limits (below).
//! 3. Work out `Pdc` of every power-controlled converter from the AC
//!    solution.
//! 4. Solve each DC grid by Newton's method for its bus voltages (45)–(48),
//!    with the slack converters' voltages fixed and the droop converters'
//!    power following their bus voltages.
//! 5. For each slack and droop converter, find the `Ps` that delivers its
//!    DC power, holding `Us` and `Qs` at the AC solution (the DC slack bus
//!    iteration, Fig. 6). The paper does this with a Newton iteration on the
//!    converter and filter voltages; here the station equations above give
//!    `Pdc` explicitly for any `Ps`, so a scalar Newton iteration on `Ps`
//!    solves the same equations.
//! 6. Repeat until no converter's `Ps` or `Qs` moves by more than the AC
//!    tolerance.
//!
//! Every AC island needs its own reference bus, so several asynchronous AC
//! grids can share one case. Every connected part of a DC grid with a
//! converter needs a slack or droop converter.
//!
//! # Converter limits
//!
//! A maximum converter current and maximum and minimum converter voltages
//! are circles in the PQ plane at the system bus, (15) and (23), with the
//! centres and radii of (41)–(44). With `Ps` known they bound `Qs`. Active
//! power has priority: a power-controlled converter outside the current or
//! upper voltage circle has its `Ps` reduced to the edge (paper III-C). A
//! reactive set-point outside the range is held at the nearest limit, and a
//! voltage-controlling converter that needs more reactive power than its
//! range allows holds the limit instead for the rest of the solve, like a
//! generator at a reactive limit. Slack and droop converters cannot be
//! moved without breaking the DC grid's balance, so a violation there is
//! reported, not corrected.

use crate::{
    BusType, Case, Error, Generator, Initialization, Model, PowerFlowOptions, PowerFlowResult,
    SolveStatus, mtdc::dense_solve, solve,
};
use num_complex::Complex64 as C;
use std::collections::HashMap;

/// A DC bus.
#[derive(Clone, Debug, PartialEq)]
pub struct DcGridBus {
    /// Positive number, unique within its grid.
    pub number: u32,
    pub name: String,
}

/// A DC line: a resistance between two DC buses.
#[derive(Clone, Debug, PartialEq)]
pub struct DcGridLine {
    pub from: u32,
    pub to: u32,
    pub name: String,
    /// Resistance of one pole conductor in ohms.
    pub resistance_ohm: f64,
    pub in_service: bool,
}

/// What a converter holds on its DC side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DcControl {
    /// Active power `p_mw` injected into the AC grid at its system bus
    /// (negative: taken from the AC grid).
    Power { p_mw: f64 },
    /// Its DC bus voltage, in per unit of the grid's base kV.
    Slack { v_dc_pu: f64 },
    /// DC-side power out of the DC grid of `p_mw + mw_per_kv · (U − U_set)`,
    /// with `U_set = v_dc_pu` times the grid's base kV.
    Droop {
        p_mw: f64,
        v_dc_pu: f64,
        mw_per_kv: f64,
    },
}

/// What a converter holds on its AC side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AcControl {
    /// Reactive power `q_mvar` injected into the AC grid.
    Reactive { q_mvar: f64 },
    /// AC voltage magnitude at its system bus in per unit.
    Voltage { v_pu: f64 },
}

/// Loss coefficients of `Ploss = a + b · Ic + c · Ic²`, `Ic` in kA.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConverterLosses {
    /// Constant loss in MW.
    pub a_mw: f64,
    /// Linear coefficient in kV (MW per kA).
    pub b_kv: f64,
    /// Quadratic coefficient in ohms while the converter takes power from
    /// the AC grid.
    pub c_rectifier_ohm: f64,
    /// Quadratic coefficient in ohms while it delivers power to the AC grid.
    pub c_inverter_ohm: f64,
}

/// Operating limits in per unit on the case's MVA base and the AC bus's
/// base kV. Zero means no limit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConverterLimits {
    /// Largest converter current `|Ic|`.
    pub current_max_pu: f64,
    /// Largest converter voltage `|Uc|` (over-modulation).
    pub uc_max_pu: f64,
    /// Smallest converter voltage `|Uc|`.
    pub uc_min_pu: f64,
}

/// A VSC converter station between an AC bus and a DC bus.
#[derive(Clone, Debug, PartialEq)]
pub struct GridConverter {
    pub name: String,
    /// AC system bus (external bus ID in the case).
    pub ac_bus: u32,
    /// DC bus number in its grid.
    pub dc_bus: u32,
    pub in_service: bool,
    pub dc_control: DcControl,
    pub ac_control: AcControl,
    /// Transformer impedance `Rtf + jXtf` in per unit on the case's base;
    /// zero when there is none.
    pub transformer: C,
    /// Filter susceptance `Bf` in per unit; zero when there is none.
    pub filter_b: f64,
    /// Phase reactor impedance `Rc + jXc` in per unit; zero when there is none.
    pub reactor: C,
    pub losses: ConverterLosses,
    pub limits: ConverterLimits,
}

/// One DC grid with its buses, lines and converters.
#[derive(Clone, Debug, PartialEq)]
pub struct DcGrid {
    pub name: String,
    /// Base (and usually rated) pole-to-ground DC voltage in kV.
    pub base_kv: f64,
    /// 1 for an asymmetric monopole, 2 for a symmetric monopole or bipole.
    pub poles: u8,
    pub buses: Vec<DcGridBus>,
    pub lines: Vec<DcGridLine>,
    pub converters: Vec<GridConverter>,
}

/// Settings of the sequential AC/DC solve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DcGridSettings {
    /// Most AC/DC iterations (each one AC solve and one DC solve).
    pub max_iterations: usize,
    /// Apply the converter limits. When false they are only reported.
    pub enforce_limits: bool,
}

impl Default for DcGridSettings {
    fn default() -> Self {
        Self {
            max_iterations: 30,
            enforce_limits: true,
        }
    }
}

/// How a converter ended up running on its AC side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcState {
    /// Holding its reactive power set-point.
    Reactive,
    /// Holding its AC bus voltage.
    Voltage,
    /// Held at a reactive limit (its set-point or the voltage it was
    /// holding needed more).
    ReactiveLimit,
}

/// Solved state of one converter. Powers in MW and Mvar, voltages in per
/// unit and degrees.
#[derive(Clone, Debug, PartialEq)]
pub struct GridConverterSolution {
    pub name: String,
    pub ac_bus: u32,
    pub dc_bus: u32,
    pub in_service: bool,
    pub dc_control: DcControl,
    pub ac_state: AcState,
    /// Power injected into the AC grid at the system bus.
    pub ps_mw: f64,
    pub qs_mvar: f64,
    pub us_pu: f64,
    pub us_deg: f64,
    pub uf_pu: f64,
    /// Power leaving the converter towards the AC side.
    pub pc_mw: f64,
    pub qc_mvar: f64,
    pub uc_pu: f64,
    pub uc_deg: f64,
    pub current_pu: f64,
    pub current_ka: f64,
    pub loss_mw: f64,
    /// Power injected into the DC grid.
    pub pdc_mw: f64,
    pub udc_pu: f64,
    pub udc_kv: f64,
    /// Reactive range at the solved `Ps` and `Us` (infinite without limits).
    pub q_min_mvar: f64,
    pub q_max_mvar: f64,
    /// A power order was reduced to the converter's limits.
    pub p_limited: bool,
    /// Still outside its limits: a slack or droop converter beyond its
    /// capability, or a point no reactive power can bring inside.
    pub outside_limits: bool,
}

/// Solved DC bus.
#[derive(Clone, Debug, PartialEq)]
pub struct DcGridBusSolution {
    pub number: u32,
    pub name: String,
    pub v_pu: f64,
    pub kv: f64,
    /// Power converters inject here in MW.
    pub injection_mw: f64,
}

/// Solved DC line.
#[derive(Clone, Debug, PartialEq)]
pub struct DcGridLineSolution {
    pub from: u32,
    pub to: u32,
    pub name: String,
    pub in_service: bool,
    /// Current per pole from `from` to `to` in kA.
    pub current_ka: f64,
    /// Power entering at `from` and leaving at `to`, all poles, in MW.
    pub from_mw: f64,
    pub to_mw: f64,
    pub loss_mw: f64,
}

/// Solved DC grid.
#[derive(Clone, Debug, PartialEq)]
pub struct DcGridSolution {
    pub name: String,
    pub converters: Vec<GridConverterSolution>,
    pub buses: Vec<DcGridBusSolution>,
    pub lines: Vec<DcGridLineSolution>,
    pub line_loss_mw: f64,
    pub converter_loss_mw: f64,
}

/// Result of [`solve_dc_grids`].
#[derive(Clone, Debug)]
pub struct AcDcResult {
    /// The last AC solve, on the input case: converter injections and the
    /// helper generators of voltage-controlling converters are taken out
    /// again, so buses and generators keep their input order and loads.
    pub ac: PowerFlowResult,
    /// In the order of the input grids.
    pub grids: Vec<DcGridSolution>,
    /// `Converged` when the AC solve converged and no converter moved by more
    /// than the tolerance; `IterationLimit` when the AC/DC iterations ran out;
    /// otherwise the AC solve's own status.
    pub status: SolveStatus,
    /// AC/DC iterations.
    pub iterations: usize,
    /// Newton steps over all AC solves.
    pub ac_iterations: usize,
    /// Largest change of a converter's `Ps` or `Qs` in the last iteration, MW or Mvar.
    pub last_change_mva: f64,
}

/// A converter's electrical data in per unit, ready to evaluate.
#[derive(Clone, Debug)]
struct Station {
    ztf: C,
    yf: C,
    zc: C,
    /// kA per per-unit current at the AC bus.
    ka_per_pu: f64,
    losses: ConverterLosses,
    limits: ConverterLimits,
    base_mva: f64,
}

/// The station at one operating point. Per unit.
#[derive(Clone, Copy, Debug)]
struct Point {
    uf: C,
    uc: C,
    sc: C,
    ic: f64,
    loss: f64,
    pdc: f64,
}

impl Station {
    /// Station state for system-bus voltage `us` and injection `ss` (36)–(39).
    fn point(&self, us: C, ss: C) -> Point {
        let is = (ss / us).conj();
        let uf = us + self.ztf * is;
        let ic = is + self.yf * uf;
        let uc = uf + self.zc * ic;
        let sc = uc * ic.conj();
        let ka = ic.norm() * self.ka_per_pu;
        let c = if sc.re < 0.0 {
            self.losses.c_rectifier_ohm
        } else {
            self.losses.c_inverter_ohm
        };
        let loss_mw = self.losses.a_mw + self.losses.b_kv * ka + c * ka * ka;
        let loss = loss_mw / self.base_mva;
        Point {
            uf,
            uc,
            sc,
            ic: ic.norm(),
            loss,
            pdc: -sc.re - loss,
        }
    }

    /// `Ps` that puts `pdc` into the DC grid at voltage `us` and reactive
    /// injection `qs`, starting from `guess` (the DC slack bus iteration).
    fn ps_for(&self, us: C, qs: f64, pdc: f64, guess: f64) -> Result<f64, String> {
        let mut ps = guess;
        for _ in 0..50 {
            let f = self.point(us, C::new(ps, qs)).pdc - pdc;
            if f.abs() < 1e-12 {
                return Ok(ps);
            }
            let h = 1e-7 * ps.abs().max(1.0);
            let d = (self.point(us, C::new(ps + h, qs)).pdc
                - self.point(us, C::new(ps - h, qs)).pdc)
                / (2.0 * h);
            if !d.is_finite() || d.abs() < 1e-9 {
                break;
            }
            ps -= f / d;
            if !ps.is_finite() {
                break;
            }
        }
        Err("the converter equations have no solution for this DC power".into())
    }

    /// The π-equivalent admittances `Y1` (shunt at the system bus) and `Y2`
    /// (series) of Fig. 4, (18)–(20), written to stay finite without a
    /// filter or transformer.
    fn pi(&self) -> (C, C) {
        let d = self.ztf * self.zc * self.yf + self.zc + self.ztf;
        (self.zc * self.yf / d, C::new(1.0, 0.0) / d)
    }

    /// PQ capability at system-bus voltage magnitude `us` and active power `ps`.
    fn capability(&self, us: f64, ps: f64) -> Capability {
        let mut cap = Capability {
            p: (f64::NEG_INFINITY, f64::INFINITY),
            q: (f64::NEG_INFINITY, f64::INFINITY),
        };
        // Upper (+) or lower (−) crossing of a circle at ps, if it reaches.
        let cross = |centre: C, radius: f64, sign: f64| {
            let dp = ps - centre.re;
            (dp.abs() <= radius).then(|| centre.im + sign * (radius * radius - dp * dp).sqrt())
        };
        let narrow_p = |cap: &mut Capability, centre: C, radius: f64| {
            cap.p.0 = cap.p.0.max(centre.re - radius);
            cap.p.1 = cap.p.1.min(centre.re + radius);
        };
        let l = &self.limits;
        if l.current_max_pu > 0.0 {
            // (41)–(42): centre −Us²·conj(Yf)/(1 + conj(Ztf·Yf)), radius Us·Imax/|1 + Ztf·Yf|.
            let k = C::new(1.0, 0.0) + (self.ztf * self.yf).conj();
            let centre = -us * us * self.yf.conj() / k;
            let radius = us * l.current_max_pu / k.norm();
            narrow_p(&mut cap, centre, radius);
            if let Some(q) = cross(centre, radius, 1.0) {
                cap.q.1 = cap.q.1.min(q);
            }
            if let Some(q) = cross(centre, radius, -1.0) {
                cap.q.0 = cap.q.0.max(q);
            }
        }
        let (y1, y2) = self.pi();
        // (43)–(44): centre −Us²·conj(Y1 + Y2), radius Us·Ucm·|Y2|.
        let centre = -us * us * (y1 + y2).conj();
        if l.uc_max_pu > 0.0 {
            let radius = us * l.uc_max_pu * y2.norm();
            narrow_p(&mut cap, centre, radius);
            if let Some(q) = cross(centre, radius, 1.0) {
                cap.q.1 = cap.q.1.min(q);
            }
        }
        if l.uc_min_pu > 0.0 {
            // Only the upper crossing bounds Q; away from the circle Uc is
            // above its minimum anyway.
            if let Some(q) = cross(centre, us * l.uc_min_pu * y2.norm(), 1.0) {
                cap.q.0 = cap.q.0.max(q);
            }
        }
        if ps < cap.p.0 || ps > cap.p.1 {
            // No reactive power brings this point inside.
            cap.q = (f64::NAN, f64::NAN);
        }
        cap
    }
}

/// Feasible `Ps` range and, at the given `Ps`, `Qs` range (NaN when `Ps` is
/// outside its range).
#[derive(Clone, Copy, Debug)]
struct Capability {
    p: (f64, f64),
    q: (f64, f64),
}

/// A validated converter with its place in the grids and the AC case.
#[derive(Clone, Debug)]
struct Prepared {
    grid: usize,
    /// Index into the grid's converters.
    index: usize,
    /// Index of its DC bus in the grid.
    dc: usize,
    /// Index of its AC bus in the case.
    ac: usize,
    station: Station,
}

/// A validated grid: line conductances in per unit and connected parts.
#[derive(Clone, Debug)]
struct PreparedGrid {
    poles: f64,
    base_kv: f64,
    /// (from, to, conductance) of in-service lines, per unit.
    lines: Vec<(usize, usize, f64)>,
    /// Buses in a part with a slack or droop converter (the others carry
    /// nothing and are reported at zero volts).
    live: Vec<bool>,
    /// Live buses whose voltage is solved for: not held by a slack converter.
    unknown: Vec<usize>,
    /// Starting voltage of each bus: its part's first slack or droop set-point.
    start: Vec<f64>,
}

fn components(n: usize, lines: &[(usize, usize, f64)]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for &(a, b, _) in lines {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    }
    (0..n).map(|i| root(&mut parent, i)).collect()
}

/// Check the grids against the case and resolve their connections.
fn prepare(case: &Case, grids: &[DcGrid]) -> Result<(Vec<PreparedGrid>, Vec<Prepared>), Error> {
    let fail = |m: String| Err(Error::InvalidCase(m));
    if !(case.base_mva.is_finite() && case.base_mva > 0.0) {
        return fail("the case needs a positive MVA base".into());
    }
    let ac_index: HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let mut prepared_grids = Vec::with_capacity(grids.len());
    let mut converters = Vec::new();
    let mut voltage_buses: HashMap<usize, String> = HashMap::new();
    let mut names = std::collections::HashSet::new();
    for (g, grid) in grids.iter().enumerate() {
        let gname = &grid.name;
        if !names.insert(gname.as_str()) {
            return fail(format!("DC grid name \"{gname}\" is used twice"));
        }
        if !(grid.base_kv.is_finite() && grid.base_kv > 0.0) {
            return fail(format!("DC grid {gname}: base_kv must be positive"));
        }
        if grid.poles != 1 && grid.poles != 2 {
            return fail(format!("DC grid {gname}: poles must be 1 or 2"));
        }
        let mut index = HashMap::new();
        for (k, b) in grid.buses.iter().enumerate() {
            if b.number == 0 || index.insert(b.number, k).is_some() {
                return fail(format!(
                    "DC grid {gname}: DC bus number {} is zero or repeated",
                    b.number
                ));
            }
        }
        let z_base = grid.base_kv * grid.base_kv / case.base_mva;
        let mut lines = Vec::new();
        for l in &grid.lines {
            let (Some(&f), Some(&t)) = (index.get(&l.from), index.get(&l.to)) else {
                return fail(format!(
                    "DC grid {gname}: line {}-{} names an unknown DC bus",
                    l.from, l.to
                ));
            };
            if f == t || !(l.resistance_ohm.is_finite() && l.resistance_ohm > 0.0) {
                return fail(format!(
                    "DC grid {gname}: line {}-{} needs two different buses and a positive resistance",
                    l.from, l.to
                ));
            }
            if l.in_service {
                lines.push((f, t, z_base / l.resistance_ohm));
            }
        }
        let part = components(grid.buses.len(), &lines);
        let mut slack_at = vec![false; grid.buses.len()];
        let mut has_converter = vec![false; grid.buses.len()];
        let mut regulated_part = std::collections::HashSet::new();
        for (c, conv) in grid.converters.iter().enumerate() {
            let cname = if conv.name.is_empty() {
                format!("at AC bus {}", conv.ac_bus)
            } else {
                conv.name.clone()
            };
            let at = format!("DC grid {gname}, converter {cname}");
            let Some(&dc) = index.get(&conv.dc_bus) else {
                return fail(format!("{at}: DC bus {} is not in the grid", conv.dc_bus));
            };
            let Some(&ac) = ac_index.get(&conv.ac_bus) else {
                return fail(format!("{at}: AC bus {} is not in the case", conv.ac_bus));
            };
            let finite = [
                conv.transformer.re,
                conv.transformer.im,
                conv.reactor.re,
                conv.reactor.im,
                conv.filter_b,
            ]
            .iter()
            .all(|x| x.is_finite());
            let l = &conv.losses;
            let losses_ok = [l.a_mw, l.b_kv, l.c_rectifier_ohm, l.c_inverter_ohm]
                .iter()
                .all(|x| x.is_finite() && *x >= 0.0);
            let m = &conv.limits;
            let limits_ok = [m.current_max_pu, m.uc_max_pu, m.uc_min_pu]
                .iter()
                .all(|x| x.is_finite() && *x >= 0.0)
                && (m.uc_max_pu == 0.0 || m.uc_min_pu < m.uc_max_pu);
            if !finite || !losses_ok || !limits_ok {
                return fail(format!(
                    "{at}: impedances must be finite, loss coefficients and limits zero or positive, and uc_min below uc_max"
                ));
            }
            let station = Station {
                ztf: conv.transformer,
                yf: C::new(0.0, conv.filter_b),
                zc: conv.reactor,
                ka_per_pu: case.base_mva / (3f64.sqrt() * case.buses[ac].base_kv),
                losses: conv.losses,
                limits: conv.limits,
                base_mva: case.base_mva,
            };
            if !station.ka_per_pu.is_finite() || station.ka_per_pu <= 0.0 {
                return fail(format!(
                    "{at}: AC bus {} needs a positive base kV",
                    conv.ac_bus
                ));
            }
            let (_, y2) = station.pi();
            if !y2.norm().is_finite() {
                return fail(format!(
                    "{at}: the transformer and phase reactor impedances add up to zero"
                ));
            }
            if !conv.in_service {
                continue;
            }
            if case.buses[ac].kind == BusType::Isolated {
                return fail(format!("{at}: AC bus {} is isolated", conv.ac_bus));
            }
            match conv.dc_control {
                DcControl::Power { p_mw } if p_mw.is_finite() => {}
                DcControl::Slack { v_dc_pu } if v_dc_pu.is_finite() && v_dc_pu > 0.0 => {
                    if std::mem::replace(&mut slack_at[dc], true) {
                        return fail(format!(
                            "{at}: DC bus {} already has a slack converter",
                            conv.dc_bus
                        ));
                    }
                    regulated_part.insert(part[dc]);
                }
                DcControl::Droop {
                    p_mw,
                    v_dc_pu,
                    mw_per_kv,
                } if p_mw.is_finite()
                    && v_dc_pu.is_finite()
                    && v_dc_pu > 0.0
                    && mw_per_kv.is_finite()
                    && mw_per_kv > 0.0 =>
                {
                    regulated_part.insert(part[dc]);
                }
                _ => {
                    return fail(format!(
                        "{at}: invalid DC set-point (voltages and droop must be positive)"
                    ));
                }
            }
            match conv.ac_control {
                AcControl::Reactive { q_mvar } if q_mvar.is_finite() => {}
                AcControl::Voltage { v_pu } if v_pu.is_finite() && v_pu > 0.0 => {
                    if case.buses[ac].kind == BusType::Reference
                        || case
                            .generators
                            .iter()
                            .any(|m| m.online && m.bus == conv.ac_bus)
                    {
                        return fail(format!(
                            "{at}: AC voltage control needs a bus without generators that is not the reference bus (bus {})",
                            conv.ac_bus
                        ));
                    }
                    if let Some(other) = voltage_buses.insert(ac, at.clone()) {
                        return fail(format!(
                            "{at}: {other} already controls the voltage at AC bus {}",
                            conv.ac_bus
                        ));
                    }
                }
                _ => {
                    return fail(format!(
                        "{at}: invalid AC set-point (voltage must be positive)"
                    ));
                }
            }
            has_converter[dc] = true;
            converters.push(Prepared {
                grid: g,
                index: c,
                dc,
                ac,
                station,
            });
        }
        for k in 0..grid.buses.len() {
            if has_converter[k] && !regulated_part.contains(&part[k]) {
                return fail(format!(
                    "DC grid {gname}: the part with DC bus {} has no slack or droop converter",
                    grid.buses[k].number
                ));
            }
        }
        let live: Vec<bool> = (0..grid.buses.len())
            .map(|k| regulated_part.contains(&part[k]))
            .collect();
        let unknown = (0..grid.buses.len())
            .filter(|&k| live[k] && !slack_at[k])
            .collect();
        let mut part_start: HashMap<usize, f64> = HashMap::new();
        for conv in grid.converters.iter().filter(|c| c.in_service) {
            let (DcControl::Slack { v_dc_pu } | DcControl::Droop { v_dc_pu, .. }) = conv.dc_control
            else {
                continue;
            };
            part_start
                .entry(part[index[&conv.dc_bus]])
                .or_insert(v_dc_pu);
        }
        let mut start: Vec<f64> = (0..grid.buses.len())
            .map(|k| part_start.get(&part[k]).copied().unwrap_or(0.0))
            .collect();
        for conv in grid.converters.iter().filter(|c| c.in_service) {
            if let DcControl::Slack { v_dc_pu } = conv.dc_control {
                start[index[&conv.dc_bus]] = v_dc_pu;
            }
        }
        prepared_grids.push(PreparedGrid {
            poles: f64::from(grid.poles),
            base_kv: grid.base_kv,
            lines,
            live,
            unknown,
            start,
        });
    }
    Ok((prepared_grids, converters))
}

/// Check DC grids against a case without solving.
pub fn check_dc_grids(case: &Case, grids: &[DcGrid]) -> Result<(), Error> {
    prepare(case, grids).map(|_| ())
}

/// Power each bus sends into the DC network, per unit (30).
fn network_power(grid: &PreparedGrid, u: &[f64]) -> Vec<f64> {
    let mut p = vec![0.0; u.len()];
    for &(i, j, g) in &grid.lines {
        let current = g * (u[i] - u[j]);
        p[i] += grid.poles * u[i] * current;
        p[j] -= grid.poles * u[j] * current;
    }
    p
}

/// Solve one DC grid's bus voltages (45)–(48). `fixed` injections are per
/// unit power into each bus; `droop` lists (bus, P_set, U_set, k) in per
/// unit with the injection `−(P_set + k · (U − U_set))`. `u` holds the
/// starting voltages, with slack buses already at their set-points.
fn solve_dc_network(
    grid: &PreparedGrid,
    fixed: &[f64],
    droop: &[(usize, f64, f64, f64)],
    u: &mut [f64],
) -> Result<(), String> {
    let m = grid.unknown.len();
    if m == 0 {
        return Ok(());
    }
    let mut row = vec![usize::MAX; u.len()];
    for (r, &k) in grid.unknown.iter().enumerate() {
        row[k] = r;
    }
    for _ in 0..50 {
        let p = network_power(grid, u);
        let mut injection = fixed.to_vec();
        for &(k, p_set, u_set, gain) in droop {
            injection[k] -= p_set + gain * (u[k] - u_set);
        }
        let f: Vec<f64> = grid.unknown.iter().map(|&k| injection[k] - p[k]).collect();
        let worst = f.iter().fold(0.0f64, |w, x| w.max(x.abs()));
        if !worst.is_finite() {
            break;
        }
        if worst < 1e-12 {
            return Ok(());
        }
        // Jacobian of f = injection − p.
        let mut jac = vec![0.0; m * m];
        for &(i, j, g) in &grid.lines {
            for (a, b) in [(i, j), (j, i)] {
                let ra = row[a];
                if ra == usize::MAX {
                    continue;
                }
                // ∂p_a/∂u_a = p · (Σ g (u_a − u_b) + u_a g) ; ∂p_a/∂u_b = −p · u_a g
                jac[ra * m + ra] -= grid.poles * (g * (u[a] - u[b]) + u[a] * g);
                if row[b] != usize::MAX {
                    jac[ra * m + row[b]] += grid.poles * u[a] * g;
                }
            }
        }
        for &(k, _, _, gain) in droop {
            if row[k] != usize::MAX {
                jac[row[k] * m + row[k]] -= gain;
            }
        }
        let Some(dx) = dense_solve(&mut jac, f, m) else {
            return Err("the DC network equations are singular".into());
        };
        for (r, &k) in grid.unknown.iter().enumerate() {
            u[k] -= dx[r];
            if !(u[k].is_finite() && u[k] > 0.0) {
                return Err(
                    "the DC voltages collapsed: the grid cannot carry the power ordered".into(),
                );
            }
        }
    }
    Err("the DC network solve did not converge".into())
}

/// What a converter holds on its AC side during one AC solve.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Reactive {
    /// A fixed `Qs` injection (per unit).
    Set(f64),
    /// Its AC bus voltage, as a PV bus.
    Voltage(f64),
}

/// The reported state of a converter that is out of service.
fn idle(c: &GridConverter) -> GridConverterSolution {
    GridConverterSolution {
        name: c.name.clone(),
        ac_bus: c.ac_bus,
        dc_bus: c.dc_bus,
        in_service: false,
        dc_control: c.dc_control,
        ac_state: AcState::Reactive,
        ps_mw: 0.0,
        qs_mvar: 0.0,
        us_pu: 0.0,
        us_deg: 0.0,
        uf_pu: 0.0,
        pc_mw: 0.0,
        qc_mvar: 0.0,
        uc_pu: 0.0,
        uc_deg: 0.0,
        current_pu: 0.0,
        current_ka: 0.0,
        loss_mw: 0.0,
        pdc_mw: 0.0,
        udc_pu: 0.0,
        udc_kv: 0.0,
        q_min_mvar: f64::NEG_INFINITY,
        q_max_mvar: f64::INFINITY,
        p_limited: false,
        outside_limits: false,
    }
}

/// DC power (per unit, into the grid) of a droop converter at bus voltage `u`.
fn droop_pdc(control: DcControl, u: f64, base_kv: f64, base_mva: f64) -> f64 {
    match control {
        DcControl::Droop {
            p_mw,
            v_dc_pu,
            mw_per_kv,
        } => -(p_mw + mw_per_kv * base_kv * (u - v_dc_pu)) / base_mva,
        _ => 0.0,
    }
}

/// Solve the AC case and the DC grids in sequence.
///
/// `opt` is used for every AC solve; its tolerance (per unit) also bounds
/// how far a converter's `Ps` and `Qs` may move in the last iteration. The
/// linear DC model is not supported.
pub fn solve_dc_grids(
    case: &Case,
    opt: &PowerFlowOptions,
    grids: &[DcGrid],
    settings: &DcGridSettings,
) -> Result<AcDcResult, Error> {
    if opt.model != Model::Ac {
        return Err(Error::Unsupported(
            "VSC DC grids need the AC power flow".into(),
        ));
    }
    let (pgrids, convs) = prepare(case, grids)?;
    let base = case.base_mva;
    let n = convs.len();
    let conv = |k: usize| &grids[convs[k].grid].converters[convs[k].index];
    let tol = opt.tolerance;

    // Starting point (35): power and droop converters at their set-points,
    // slack converters sharing their grid's lossless balance.
    let mut ps: Vec<f64> = (0..n)
        .map(|k| match conv(k).dc_control {
            DcControl::Power { p_mw } | DcControl::Droop { p_mw, .. } => p_mw / base,
            DcControl::Slack { .. } => 0.0,
        })
        .collect();
    for g in 0..grids.len() {
        let is_slack = |k: usize| matches!(conv(k).dc_control, DcControl::Slack { .. });
        let members: Vec<usize> = (0..n).filter(|&k| convs[k].grid == g).collect();
        let slacks = members.iter().filter(|&&k| is_slack(k)).count();
        if slacks > 0 {
            let rest: f64 = members
                .iter()
                .filter(|&&k| !is_slack(k))
                .map(|&k| ps[k])
                .sum();
            for &k in members.iter().filter(|&&k| is_slack(k)) {
                ps[k] = -rest / slacks as f64;
            }
        }
    }
    let mut reactive: Vec<Reactive> = (0..n)
        .map(|k| match conv(k).ac_control {
            AcControl::Reactive { q_mvar } => Reactive::Set(q_mvar / base),
            AcControl::Voltage { v_pu } => Reactive::Voltage(v_pu),
        })
        .collect();
    // Voltage-controlling converters held at their upper (true) or lower limit.
    let mut pinned: Vec<Option<bool>> = vec![None; n];
    // Reactive set-points held inside the converter's range.
    let mut clamped = vec![false; n];
    let mut p_limited = vec![false; n];
    let mut udc: Vec<Vec<f64>> = pgrids.iter().map(|pg| pg.start.clone()).collect();

    let first_gen = case.generators.len();
    let mut start = case.clone();
    let mut options = opt.clone();
    let (mut iterations, mut ac_iterations) = (0, 0);
    let mut last_change = f64::INFINITY;
    let mut status = SolveStatus::IterationLimit;
    let mut last: Option<(PowerFlowResult, Vec<C>, Vec<f64>)> = None;
    while iterations < settings.max_iterations {
        iterations += 1;
        // 1. AC solve, the converters as fixed injections or PV buses.
        let mut work = start.clone();
        for k in 0..n {
            let b = convs[k].ac;
            match reactive[k] {
                Reactive::Voltage(v) => {
                    let bus = &mut work.buses[b];
                    if bus.kind == BusType::Pq {
                        bus.kind = BusType::Pv;
                    }
                    work.generators.push(Generator {
                        bus: bus.id,
                        pg: ps[k] * base,
                        qg: 0.0,
                        qmax: f64::INFINITY,
                        qmin: f64::NEG_INFINITY,
                        vg: v,
                        mbase: base,
                        online: true,
                        pmax: ps[k] * base,
                        pmin: ps[k] * base,
                        extra: Vec::new(),
                        regulated_bus: None,
                        reactive_share: 100.0,
                    });
                }
                Reactive::Set(q) => {
                    work.buses[b].pd -= ps[k] * base;
                    work.buses[b].qd -= q * base;
                }
            }
        }
        let result = solve(&work, &options)?;
        ac_iterations += result.iterations;
        // Later AC solves start from this one's voltages.
        for (to, from) in start.buses.iter_mut().zip(&result.case.buses) {
            (to.vm, to.va) = (from.vm, from.va);
        }
        options.initialization = Initialization::Case;
        let us: Vec<C> = convs
            .iter()
            .map(|c| {
                let b = &result.case.buses[c.ac];
                C::from_polar(b.vm, b.va.to_radians())
            })
            .collect();
        let mut helper = first_gen;
        let qs: Vec<f64> = (0..n)
            .map(|k| match reactive[k] {
                Reactive::Voltage(_) => {
                    helper += 1;
                    result.case.generators[helper - 1].qg / base
                }
                Reactive::Set(q) => q,
            })
            .collect();
        if result.status != SolveStatus::Converged {
            status = result.status;
            last = Some((result, us, qs));
            break;
        }

        // 2. Active power limits: a power order runs as close to its set-point
        // as the converter's limits at this AC voltage allow.
        let mut next_ps = ps.clone();
        for k in 0..n {
            let DcControl::Power { p_mw } = conv(k).dc_control else {
                continue;
            };
            let order = p_mw / base;
            let (lo, hi) = if settings.enforce_limits {
                convs[k].station.capability(us[k].norm(), order).p
            } else {
                (f64::NEG_INFINITY, f64::INFINITY)
            };
            p_limited[k] = order < lo || order > hi;
            if p_limited[k] {
                let margin = 1e-9 * (hi - lo).abs().max(1.0);
                next_ps[k] = order.clamp(lo + margin, hi - margin);
            } else {
                next_ps[k] = order;
            }
        }

        // 3. DC power of the power-controlled converters from the AC solution.
        let pdc: Vec<f64> = (0..n)
            .map(|k| convs[k].station.point(us[k], C::new(ps[k], qs[k])).pdc)
            .collect();
        // 4.–5. Each DC network, then the slack and droop converters' Ps.
        for (g, pg) in pgrids.iter().enumerate() {
            let members: Vec<usize> = (0..n).filter(|&k| convs[k].grid == g).collect();
            let mut fixed = vec![0.0; grids[g].buses.len()];
            let mut droop = Vec::new();
            for &k in &members {
                match conv(k).dc_control {
                    DcControl::Power { .. } => fixed[convs[k].dc] += pdc[k],
                    DcControl::Droop {
                        p_mw,
                        v_dc_pu,
                        mw_per_kv,
                    } => droop.push((
                        convs[k].dc,
                        p_mw / base,
                        v_dc_pu,
                        mw_per_kv * pg.base_kv / base,
                    )),
                    DcControl::Slack { .. } => {}
                }
            }
            solve_dc_network(pg, &fixed, &droop, &mut udc[g])
                .map_err(|e| Error::Numerical(format!("DC grid {}: {e}", grids[g].name)))?;
            let into_network = network_power(pg, &udc[g]);
            for &k in &members {
                let dc = convs[k].dc;
                let u = udc[g][dc];
                let target = match conv(k).dc_control {
                    DcControl::Power { .. } => continue,
                    control @ DcControl::Droop { .. } => droop_pdc(control, u, pg.base_kv, base),
                    DcControl::Slack { .. } => {
                        // What its bus sends into the network, less the
                        // bus's other converters.
                        let others: f64 = members
                            .iter()
                            .filter(|&&j| j != k && convs[j].dc == dc)
                            .map(|&j| match conv(j).dc_control {
                                DcControl::Power { .. } => pdc[j],
                                control => droop_pdc(control, u, pg.base_kv, base),
                            })
                            .sum();
                        into_network[dc] - others
                    }
                };
                next_ps[k] = convs[k]
                    .station
                    .ps_for(us[k], qs[k], target, ps[k])
                    .map_err(|e| {
                        Error::Numerical(format!(
                            "DC grid {}, converter {}: {e}",
                            grids[g].name,
                            conv(k).name
                        ))
                    })?;
            }
        }

        // Reactive set-points for the next AC solve, within the converter
        // range at the new operating point.
        let mut next_reactive = reactive.clone();
        let mut switched = false;
        for k in 0..n {
            let (lo, hi) = convs[k].station.capability(us[k].norm(), next_ps[k]).q;
            let valid = settings.enforce_limits && !lo.is_nan() && lo <= hi;
            next_reactive[k] = match (conv(k).ac_control, pinned[k]) {
                (AcControl::Reactive { q_mvar }, _) => {
                    let q = q_mvar / base;
                    let held = if valid { q.clamp(lo, hi) } else { q };
                    clamped[k] = held != q;
                    Reactive::Set(held)
                }
                (AcControl::Voltage { .. }, Some(upper)) if valid => {
                    Reactive::Set(if upper { hi } else { lo })
                }
                (AcControl::Voltage { .. }, Some(_)) => reactive[k],
                (AcControl::Voltage { v_pu }, None) => {
                    if valid && qs[k] > hi + tol {
                        pinned[k] = Some(true);
                        switched = true;
                        Reactive::Set(hi)
                    } else if valid && qs[k] < lo - tol {
                        pinned[k] = Some(false);
                        switched = true;
                        Reactive::Set(lo)
                    } else {
                        Reactive::Voltage(v_pu)
                    }
                }
            };
        }

        let mut change: f64 = 0.0;
        for k in 0..n {
            change = change.max((next_ps[k] - ps[k]).abs());
            if let (Reactive::Set(a), Reactive::Set(b)) = (next_reactive[k], reactive[k]) {
                change = change.max((a - b).abs());
            }
        }
        last_change = change * base;
        log::info!(
            "AC/DC iteration {iterations}: {} AC Newton steps, largest converter change {:.3e} MVA{}",
            result.iterations,
            last_change,
            if switched {
                "; a converter reached a reactive limit"
            } else {
                ""
            }
        );
        last = Some((result, us, qs));
        if change < tol && !switched {
            status = SolveStatus::Converged;
            break;
        }
        ps = next_ps;
        reactive = next_reactive;
    }
    let (mut ac, us, qs) = last.expect("at least one iteration runs");
    if status == SolveStatus::IterationLimit {
        log::warn!(
            "AC/DC solve stopped after {iterations} iterations; converters still moving by {last_change:.3e} MVA"
        );
    }
    // Take the converters back out of the solved case.
    ac.case.generators.truncate(first_gen);
    for (to, from) in ac.case.buses.iter_mut().zip(&case.buses) {
        (to.kind, to.pd, to.qd) = (from.kind, from.pd, from.qd);
    }

    let mut out = Vec::with_capacity(grids.len());
    for (g, (grid, pg)) in grids.iter().zip(&pgrids).enumerate() {
        let u = &udc[g];
        let mut injection = vec![0.0; grid.buses.len()];
        let mut converter_loss = 0.0;
        let mut converters = Vec::with_capacity(grid.converters.len());
        for (c, gc) in grid.converters.iter().enumerate() {
            let Some(k) = convs.iter().position(|p| p.grid == g && p.index == c) else {
                converters.push(idle(gc));
                continue;
            };
            let st = &convs[k].station;
            let pt = st.point(us[k], C::new(ps[k], qs[k]));
            let (lo, hi) = st.capability(us[k].norm(), ps[k]).q;
            let outside = lo.is_nan() || lo > hi || qs[k] > hi + tol || qs[k] < lo - tol;
            injection[convs[k].dc] += pt.pdc * base;
            converter_loss += pt.loss * base;
            let ac_state = match (gc.ac_control, pinned[k]) {
                (_, Some(_)) => AcState::ReactiveLimit,
                (AcControl::Voltage { .. }, None) => AcState::Voltage,
                (AcControl::Reactive { .. }, None) if clamped[k] => AcState::ReactiveLimit,
                _ => AcState::Reactive,
            };
            let dc = convs[k].dc;
            converters.push(GridConverterSolution {
                name: gc.name.clone(),
                ac_bus: gc.ac_bus,
                dc_bus: gc.dc_bus,
                in_service: true,
                dc_control: gc.dc_control,
                ac_state,
                ps_mw: ps[k] * base,
                qs_mvar: qs[k] * base,
                us_pu: us[k].norm(),
                us_deg: us[k].arg().to_degrees(),
                uf_pu: pt.uf.norm(),
                pc_mw: pt.sc.re * base,
                qc_mvar: pt.sc.im * base,
                uc_pu: pt.uc.norm(),
                uc_deg: pt.uc.arg().to_degrees(),
                current_pu: pt.ic,
                current_ka: pt.ic * st.ka_per_pu,
                loss_mw: pt.loss * base,
                pdc_mw: pt.pdc * base,
                udc_pu: u[dc],
                udc_kv: u[dc] * grid.base_kv,
                q_min_mvar: lo * base,
                q_max_mvar: hi * base,
                p_limited: p_limited[k],
                outside_limits: outside,
            });
        }
        let buses = grid
            .buses
            .iter()
            .enumerate()
            .map(|(k, b)| {
                let v = if pg.live[k] { u[k] } else { 0.0 };
                DcGridBusSolution {
                    number: b.number,
                    name: b.name.clone(),
                    v_pu: v,
                    kv: v * grid.base_kv,
                    injection_mw: injection[k],
                }
            })
            .collect();
        let mut line_loss = 0.0;
        let mut in_service = pg.lines.iter();
        let lines = grid
            .lines
            .iter()
            .map(|l| {
                let mut row = DcGridLineSolution {
                    from: l.from,
                    to: l.to,
                    name: l.name.clone(),
                    in_service: l.in_service,
                    current_ka: 0.0,
                    from_mw: 0.0,
                    to_mw: 0.0,
                    loss_mw: 0.0,
                };
                if l.in_service {
                    let &(i, j, gij) = in_service.next().expect("one entry per in-service line");
                    let current = gij * (u[i] - u[j]);
                    row.current_ka = current * base / grid.base_kv;
                    row.from_mw = pg.poles * u[i] * current * base;
                    row.to_mw = pg.poles * u[j] * current * base;
                    row.loss_mw = row.from_mw - row.to_mw;
                    line_loss += row.loss_mw;
                }
                row
            })
            .collect();
        out.push(DcGridSolution {
            name: grid.name.clone(),
            converters,
            buses,
            lines,
            line_loss_mw: line_loss,
            converter_loss_mw: converter_loss,
        });
    }
    Ok(AcDcResult {
        ac,
        grids: out,
        status,
        iterations,
        ac_iterations,
        last_change_mva: last_change,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Converter data of Beerten, Cole and Belmans (2012), Table I, on a
    /// 100 MVA base. Converters 1–3 have no filter.
    fn paper_station(filter: bool, kv: f64, a: f64, b: f64, c_to_ac: f64, c_to_dc: f64) -> Station {
        Station {
            ztf: C::new(0.0015, 0.1121),
            yf: C::new(0.0, if filter { 0.0887 } else { 0.0 }),
            zc: C::new(0.0001, 0.16428),
            ka_per_pu: 100.0 / (3f64.sqrt() * kv),
            losses: ConverterLosses {
                a_mw: a,
                b_kv: b,
                c_rectifier_ohm: c_to_dc,
                c_inverter_ohm: c_to_ac,
            },
            limits: ConverterLimits::default(),
            base_mva: 100.0,
        }
    }

    /// The station equations reproduce every converter in Tables II and III
    /// from its system-bus quantities, to the tables' rounding.
    ///
    /// Two readings of Table I were needed: the coefficient printed as
    /// `c_rec` matches the converters that deliver power to the AC grid and
    /// `c_inv` the ones that take it, and the constant `a` of the 200 MW
    /// converters 4 and 6 is 2.206 MW (the 100 MW converters 5 and 7 use
    /// 1.103 MW), as for converters 3 and 1–2 of the other grid.
    #[test]
    // −3.14 Mvar is a table value, not π.
    #[allow(clippy::approx_constant)]
    fn reproduces_the_papers_converter_results() {
        // Ps, Qs, Us, δs; then Pc, Qc, Uc, δc, Ploss, Pdc.
        type Row = (Station, [f64; 4], [f64; 6]);
        let small = || paper_station(false, 138.0, 1.103, 0.887, 2.885, 4.371);
        let rows: Vec<Row> = vec![
            (
                small(),
                [66.84, 0.0, 1.025, -8.93],
                [66.91, 11.75, 1.042, 1.04, 1.56, -68.47],
            ),
            (
                small(),
                [75.00, 5.86, 1.000, -4.88],
                [75.09, 21.50, 1.038, 6.63, 1.67, -76.76],
            ),
            (
                paper_station(false, 138.0, 2.206, 0.887, 1.442, 2.185),
                [-150.0, 0.0, 1.050, -0.08],
                [-149.67, 56.40, 1.120, -20.73, 3.52, 146.16],
            ),
            (
                paper_station(true, 345.0, 2.206, 1.8, 5.94, 9.0),
                [124.43, 0.0, 1.020, 0.0],
                [124.67, 31.36, 1.061, 18.41, 2.81, -127.49],
            ),
            (
                paper_station(true, 345.0, 1.103, 1.8, 11.88, 18.0),
                [-50.0, 0.0, 1.050, 10.11],
                [-49.96, -3.46, 1.042, 2.90, 1.36, 48.60],
            ),
            (
                paper_station(true, 345.0, 2.206, 1.8, 5.94, 9.0),
                [-135.0, 0.0, 1.014, 10.25],
                [-134.72, 39.26, 1.062, -9.89, 3.04, 131.67],
            ),
            (
                paper_station(true, 345.0, 1.103, 1.8, 11.88, 18.0),
                [50.0, 0.0, 1.039, 14.60],
                [50.04, -3.14, 1.033, 21.96, 1.33, -51.37],
            ),
        ];
        for (n, (st, [ps, qs, us, ds], want)) in rows.iter().enumerate() {
            let us = C::from_polar(*us, ds.to_radians());
            let p = st.point(us, C::new(ps / 100.0, qs / 100.0));
            let got = [
                p.sc.re * 100.0,
                p.sc.im * 100.0,
                p.uc.norm(),
                p.uc.arg().to_degrees(),
                p.loss * 100.0,
                p.pdc * 100.0,
            ];
            // Inputs are rounded to 0.01 MW and 0.01°, so allow a little
            // more than the tables' own rounding.
            let tol = [0.012, 0.012, 0.0006, 0.012, 0.006, 0.012];
            for q in 0..6 {
                assert!(
                    (got[q] - want[q]).abs() <= tol[q],
                    "converter {}: quantity {q} is {} against {}",
                    n + 1,
                    got[q],
                    want[q]
                );
            }
        }
    }

    #[test]
    fn slack_iteration_inverts_the_station() {
        let st = paper_station(true, 345.0, 2.206, 1.8, 5.94, 9.0);
        let us = C::from_polar(1.02, 0.1);
        for ps in [-1.5, -0.2, 0.0, 0.4, 1.3] {
            let pdc = st.point(us, C::new(ps, 0.1)).pdc;
            let back = st.ps_for(us, 0.1, pdc, 0.0).unwrap();
            assert!((back - ps).abs() < 1e-9, "{ps} came back as {back}");
        }
        // The paper's DC slack, converter 1: Pdc −68.47 MW gives Ps 66.84 MW.
        let st = paper_station(false, 138.0, 1.103, 0.887, 2.885, 4.371);
        let us = C::from_polar(1.025, (-8.93f64).to_radians());
        let ps = st.ps_for(us, 0.0, -0.6847, 0.5).unwrap();
        assert!((ps * 100.0 - 66.84).abs() < 0.01, "{ps}");
    }

    #[test]
    fn capability_circles_are_the_current_and_voltage_limits() {
        for filter in [false, true] {
            let mut st = paper_station(filter, 345.0, 0.0, 0.0, 0.0, 0.0);
            st.limits = ConverterLimits {
                current_max_pu: 1.1,
                uc_max_pu: 1.15,
                uc_min_pu: 0.0,
            };
            let us = C::from_polar(1.01, 0.2);
            for ps in [-0.9, 0.0, 0.7] {
                let cap = st.capability(us.norm(), ps);
                let (lo, hi) = cap.q;
                let at_lo = st.point(us, C::new(ps, lo));
                let at_hi = st.point(us, C::new(ps, hi));
                // The lower limit is the current; the upper is whichever is
                // tighter, and both stay within the other.
                assert!((at_lo.ic - 1.1).abs() < 1e-9, "{filter} {ps}: {}", at_lo.ic);
                assert!(at_hi.ic <= 1.1 + 1e-9 && at_hi.uc.norm() <= 1.15 + 1e-9);
                assert!((at_hi.ic - 1.1).abs() < 1e-9 || (at_hi.uc.norm() - 1.15).abs() < 1e-9);
                let mid = st.point(us, C::new(ps, 0.5 * (lo + hi)));
                assert!(mid.ic < 1.1 && mid.uc.norm() < 1.15);
            }
            // Beyond the current circle no Q helps.
            let cap = st.capability(us.norm(), 1.2);
            assert!(cap.q.0.is_nan() && cap.p.1 < 1.2);
            // The lower voltage limit bounds Q from below.
            st.limits = ConverterLimits {
                current_max_pu: 0.0,
                uc_max_pu: 0.0,
                uc_min_pu: 0.95,
            };
            let lo = st.capability(us.norm(), 0.3).q.0;
            assert!((st.point(us, C::new(0.3, lo)).uc.norm() - 0.95).abs() < 1e-9);
        }
    }

    fn line_grid(poles: f64, g: f64) -> PreparedGrid {
        // Bus 0 held by a slack converter, buses 1 and 2 solved for.
        PreparedGrid {
            poles,
            base_kv: 300.0,
            lines: vec![(0, 1, g), (1, 2, g)],
            live: vec![true; 3],
            unknown: vec![1, 2],
            start: vec![1.0; 3],
        }
    }

    #[test]
    fn dc_network_matches_a_hand_solution() {
        // A power converter puts 0.8 p.u. into bus 2; bus 1 is a junction.
        // Current I flows 2 → 1 → 0: I = (U2 − U0) / (2R) and
        // p · U2 · I = 0.8 with U0 = 1, so U2 solves a quadratic.
        for poles in [1.0, 2.0] {
            let g = 50.0;
            let grid = line_grid(poles, g);
            let mut u = vec![1.0; 3];
            solve_dc_network(&grid, &[0.0, 0.0, 0.8], &[], &mut u).unwrap();
            let r = 2.0 / g;
            let u2 = (1.0 + (1.0 + 4.0 * r * 0.8 / poles).sqrt()) / 2.0;
            assert!((u[2] - u2).abs() < 1e-12, "{} {u2}", u[2]);
            assert!((u[1] - (1.0 + u2) / 2.0).abs() < 1e-12);
            // What the slack takes out is the order less the line losses.
            let p = network_power(&grid, &u);
            let current = (u2 - 1.0) / r;
            assert!((p[0] + 0.8 - poles * current * current * r).abs() < 1e-12);
        }
    }

    #[test]
    fn droop_converters_follow_their_lines() {
        // No slack: two droop converters at buses 0 and 2 balance a power
        // converter at bus 1.
        let grid = PreparedGrid {
            poles: 2.0,
            base_kv: 300.0,
            lines: vec![(0, 1, 40.0), (1, 2, 20.0)],
            live: vec![true; 3],
            unknown: vec![0, 1, 2],
            start: vec![1.0; 3],
        };
        let droop = [(0, 0.2, 1.0, 10.0), (2, -0.1, 1.0, 5.0)];
        let mut u = vec![1.0; 3];
        solve_dc_network(&grid, &[0.0, 0.6, 0.0], &droop, &mut u).unwrap();
        let p = network_power(&grid, &u);
        for &(k, p_set, u_set, gain) in &droop {
            assert!((p[k] + p_set + gain * (u[k] - u_set)).abs() < 1e-12);
        }
        assert!((p[1] - 0.6).abs() < 1e-12);
        // Losses come out of the droop converters.
        let losses: f64 = p.iter().sum();
        assert!(losses > 0.0 && losses < 1e-2);
    }
}
