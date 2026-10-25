// Formulations and result allocation adapted from MATPOWER 8.1; see NOTICE.
use crate::{
    BusType, Case, Error, Generator, VoltageControls,
    controls::{self, Controller, Setup},
    dc::{self, LccLine, LccSolution, TapTargets, Taps},
    mtdc::{self, MtdcMode, MtdcSolution, MtdcSystem},
    network::Network,
    vsc::{self, DcState, VscAcControl, VscLine, VscSolution},
};
use faer::{
    Mat,
    prelude::Solve,
    sparse::{SparseColMat, Triplet},
};
use num_complex::Complex64 as C;

/// Power-flow formulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// AC power/polar Newton–Raphson with constant-power loads.
    Ac,
    /// Linear DC power flow for active power and bus angles.
    Dc,
}

/// AC voltage initialization. Both choices apply generator setpoints at controlled buses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initialization {
    /// Start from the voltage magnitudes and angles supplied by the case.
    Case,
    /// Start each island at 1 p.u. and its reference-bus angle.
    Flat,
}

/// Solver settings; defaults to AC with a 1e-8 p.u. tolerance and 10 Newton steps.
#[derive(Clone, Debug)]
pub struct PowerFlowOptions {
    /// Formulation; defaults to [`Model::Ac`].
    pub model: Model,
    /// AC starting voltages; defaults to [`Initialization::Case`]. Ignored for DC.
    pub initialization: Initialization,
    /// Infinity norm of the constrained power mismatch, in per unit.
    pub tolerance: f64,
    /// Newton steps per reactive-limit pass. Zero evaluates the initial state.
    pub max_iterations: usize,
    /// Fix AC reactive-limit violations and switch affected buses to PQ.
    /// Also turns on remote voltage regulation ([`Generator::regulated_bus`]);
    /// without limits, every generator holds its own bus.
    /// Defaults to false; enabling this for DC returns [`Error::Unsupported`].
    pub enforce_q_limits: bool,
    /// Reactive-limit violation tolerance in MVAr.
    pub q_limit_tolerance: f64,
    /// How DC lines are modelled; defaults to [`DcModel::Simplified`].
    pub dc_model: DcModel,
    /// Switched shunts and transformer taps that hold bus voltages between
    /// Newton solves (AC only); empty by default. Continuous shunts keep to
    /// their susceptance range even when `enforce_q_limits` is false.
    pub voltage_controls: VoltageControls,
}

/// DC line model for AC solves.
#[derive(Clone, Debug, Default)]
pub enum DcModel {
    /// Every `case.dcline` row is MATPOWER's generator pair: scheduled P,
    /// linear losses, and terminals that hold voltage within their Q range.
    #[default]
    Simplified,
    /// The listed rows use the detailed two-terminal LCC model (see
    /// [`LccLine`]) or VSC model (see [`VscLine`]); other rows stay
    /// simplified. The LCC operating point is recomputed from the terminal
    /// voltages at every Newton step, and the converter terminals become
    /// voltage-dependent loads that absorb reactive power. VSC lines keep
    /// their AC voltage or power factor control, with active power from the
    /// DC-side solution and passes that enforce converter MVA and current
    /// limits. Multi-terminal systems (see [`MtdcSystem`]) solve their DC
    /// network at every Newton step and load their converters' AC buses
    /// like two-terminal LCC lines. DC (linear) solves use the simplified
    /// model.
    Detailed {
        /// Detailed LCC data, each naming its `case.dcline` row.
        lines: Vec<LccLine>,
        /// Detailed VSC data, each naming its `case.dcline` row.
        vsc_lines: Vec<VscLine>,
        /// Detailed multi-terminal DC systems, each naming the
        /// `case.dcline` rows of its converters.
        mtdc: Vec<MtdcSystem>,
        /// Step converter taps between Newton solves to hold the scheduled
        /// DC voltage and keep firing angles in range (PSS/E's DC tap
        /// adjustment). When false, the taps stay at their case values.
        adjust_taps: bool,
    },
}
impl Default for PowerFlowOptions {
    fn default() -> Self {
        Self {
            model: Model::Ac,
            initialization: Initialization::Case,
            tolerance: 1e-8,
            max_iterations: 10,
            enforce_q_limits: false,
            q_limit_tolerance: 5e-6,
            dc_model: DcModel::Simplified,
            voltage_controls: VoltageControls::default(),
        }
    }
}

/// Solver outcome, distinct from input and numerical errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveStatus {
    /// The constrained mismatch meets the requested tolerance.
    Converged,
    /// Newton reached its step limit; the result contains the final iterate.
    IterationLimit,
    /// Reactive-limit enforcement cannot retain a feasible controlled/reference bus.
    ReactiveLimitsInfeasible,
}

/// Final case and diagnostics, including results from unsuccessful iterations.
#[derive(Clone, Debug)]
pub struct PowerFlowResult {
    /// Solved voltages and generator outputs, in original case order and units.
    pub case: Case,
    /// `[PF, QF, PT, QT]` in MW/MVAr, in original branch order.
    /// Positive values enter the branch at that end; excluded branches are zero.
    pub branch_flows: Vec<[f64; 4]>,
    /// Outcome to check even when [`solve`] returns `Ok`.
    pub status: SolveStatus,
    /// Total Newton steps across all reactive-limit passes (one for DC).
    pub iterations: usize,
    /// Infinity norm of the final constrained mismatch in per unit.
    /// This does not measure reactive-limit feasibility.
    pub residual: f64,
    /// Number of passes that fixed one or more generators at reactive limits.
    pub q_limit_passes: usize,
    /// Operating points of the detailed DC lines, in the order given by
    /// [`DcModel::Detailed`]. Empty for the simplified model.
    pub lcc: Vec<LccSolution>,
    /// Number of passes that changed one or more DC converter taps.
    pub dc_tap_passes: usize,
    /// Operating points of the detailed VSC lines, in the order given by
    /// [`DcModel::Detailed`]. Empty for the simplified model.
    pub vsc: Vec<VscSolution>,
    /// Number of passes that reduced VSC output to converter limits.
    pub vsc_limit_passes: usize,
    /// Final susceptance (MVAr at 1 p.u.) of each switched shunt in
    /// [`PowerFlowOptions::voltage_controls`], in its order; also applied to
    /// the buses' `bs` in `case`.
    pub switched_shunts: Vec<f64>,
    /// Final winding ratio of each tap control in
    /// [`PowerFlowOptions::voltage_controls`], in its order; also applied to
    /// the branches' `tap` in `case`.
    pub tap_ratios: Vec<f64>,
    /// Number of passes that switched shunts or moved taps.
    pub voltage_control_passes: usize,
    /// Operating points of the detailed multi-terminal DC systems, in the
    /// order given by [`DcModel::Detailed`]. Empty for the simplified model.
    pub mtdc: Vec<MtdcSolution>,
}

fn linear(
    n: usize,
    entries: &[Triplet<usize, usize, f64>],
    rhs: &[f64],
) -> Result<Vec<f64>, Error> {
    log::trace!(
        "solving sparse linear system (dimension={n}, entries={})",
        entries.len()
    );
    if n == 0 {
        return Ok(Vec::new());
    }
    if rhs.iter().any(|x| !x.is_finite()) || entries.iter().any(|t| !t.val.is_finite()) {
        return Err(Error::Numerical("non-finite linear system".into()));
    }
    let a = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, entries)
        .map_err(|e| Error::Numerical(format!("sparse assembly: {e:?}")))?;
    let lu = a
        .sp_lu()
        .map_err(|e| Error::Numerical(format!("singular or unfactorable system: {e:?}")))?;
    let x = lu.solve(Mat::from_fn(n, 1, |i, _| rhs[i]));
    let out: Vec<_> = (0..n).map(|i| x[(i, 0)]).collect();
    if out.iter().any(|x| !x.is_finite()) {
        return Err(Error::Numerical(
            "linear solve produced non-finite values".into(),
        ));
    }
    let mut residual = rhs.to_vec();
    let mut scale = rhs.iter().fold(0.0_f64, |a, &v| a.max(v.abs()));
    for t in entries {
        residual[t.row] -= t.val * out[t.col];
        scale = scale.max((t.val * out[t.col]).abs());
    }
    if residual.iter().any(|r| r.abs() > 1e-8 * (1.0 + scale)) {
        return Err(Error::Numerical(
            "linear solve failed residual check".into(),
        ));
    }
    Ok(out)
}

pub(crate) fn variables(kinds: &[BusType]) -> (Vec<usize>, Vec<usize>) {
    let angles = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == BusType::Pv)
        .chain(kinds.iter().enumerate().filter(|(_, k)| **k == BusType::Pq))
        .map(|(i, _)| i)
        .collect();
    let pq = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == BusType::Pq)
        .map(|(i, _)| i)
        .collect();
    (angles, pq)
}

/// Which equations and voltage variables a Newton solve uses.
///
/// Angles (and active-power rows) are every non-reference bus. A bus that
/// holds its own voltage has no magnitude variable and no reactive row. A
/// group of buses holding one remote bus's voltage swaps that bus's
/// magnitude variable for its own members' magnitudes, and adds rows that
/// share the group's reactive power by [`Generator::reactive_share`].
pub(crate) struct Layout {
    /// Buses with an angle variable and an active-power row.
    pub angles: Vec<usize>,
    /// Buses with a voltage-magnitude variable.
    pub magnitudes: Vec<usize>,
    /// Buses with a reactive-power row.
    pub q_rows: Vec<usize>,
    /// Reactive-sharing rows within remote regulation groups.
    shares: Vec<Share>,
    /// Remotely held buses and their voltage setpoints in per unit.
    pub targets: Vec<(usize, f64)>,
    /// Buses whose generators hold a remote bus, in bus order.
    pub remote: Vec<usize>,
    /// The bus each entry of `remote` holds.
    pub held: Vec<usize>,
}

/// One sharing row: `(Qa + qa) / wa − (Qb + qb) / wb = 0`, where `Q` is the
/// bus's reactive mismatch and `q` its scheduled generator output, so each
/// sum is the generator reactive power needed at that bus.
struct Share {
    a: usize,
    b: usize,
    weights: [f64; 2],
    scheduled: [f64; 2],
}

impl Layout {
    /// The plain layout: PV buses hold their own voltage.
    pub(crate) fn plain(kinds: &[BusType]) -> Self {
        let (angles, pq) = variables(kinds);
        Self {
            angles,
            magnitudes: pq.clone(),
            q_rows: pq,
            shares: Vec::new(),
            targets: Vec::new(),
            remote: Vec::new(),
            held: Vec::new(),
        }
    }

    /// The layout for `kinds`, with remote regulation from the generators'
    /// [`Generator::regulated_bus`]. A group falls back to local control
    /// when its bus is a reference bus, lies in another island, or holds a
    /// remote bus itself.
    fn new(net: &Network, case: &Case, kinds: &[BusType]) -> Self {
        let mut layout = Self::plain(kinds);
        let n = kinds.len();
        let mut island = vec![0; n];
        for (k, buses) in net.islands.iter().enumerate() {
            for &i in buses {
                island[i] = k;
            }
        }
        let wanted: Vec<Option<usize>> = (0..n)
            .map(|i| {
                if kinds[i] != BusType::Pv {
                    return None;
                }
                net.gens[i]
                    .iter()
                    .find_map(|&g| case.generators[g].regulated_bus)
                    .and_then(|id| net.by_id.get(&id).copied())
                    .filter(|&t| t != i && island[t] == island[i])
            })
            .collect();
        let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        let mut fallback = 0;
        for (i, target) in wanted.iter().enumerate() {
            let Some(t) = *target else { continue };
            if kinds[t] == BusType::Reference || wanted[t].is_some() {
                fallback += 1;
                continue;
            }
            groups.entry(t).or_default().push(i);
        }
        if fallback > 0 {
            log::debug!(
                "{fallback} generator buses hold their own voltage because their regulated bus is a reference bus or regulates remotely"
            );
        }
        if groups.is_empty() {
            return layout;
        }
        let weight = |i: usize| {
            let w: f64 = net.gens[i]
                .iter()
                .map(|&g| {
                    let share = case.generators[g].reactive_share;
                    if share.is_finite() && share > 0.0 {
                        share
                    } else {
                        100.0
                    }
                })
                .sum();
            w / 100.0
        };
        let scheduled = |i: usize| {
            net.gens[i]
                .iter()
                .map(|&g| case.generators[g].qg)
                .sum::<f64>()
                / case.base_mva
        };
        let mut held = vec![false; n];
        for (&t, members) in &mut groups {
            if kinds[t] == BusType::Pq {
                held[t] = true;
                let first = &case.generators[net.gens[members[0]][0]];
                layout.targets.push((t, first.vg));
            } else {
                // A generator at the held bus joins the group and fixes its voltage.
                members.insert(0, t);
            }
            for &m in members.iter() {
                if m != t {
                    layout.remote.push(m);
                }
            }
            let b = members[0];
            for &a in &members[1..] {
                layout.shares.push(Share {
                    a,
                    b,
                    weights: [weight(a), weight(b)],
                    scheduled: [scheduled(a), scheduled(b)],
                });
            }
        }
        let mut pairs: Vec<_> = layout
            .remote
            .iter()
            .map(|&m| (m, wanted[m].unwrap()))
            .collect();
        pairs.sort_unstable();
        (layout.remote, layout.held) = pairs.into_iter().unzip();
        layout.magnitudes.retain(|&i| !held[i]);
        layout.magnitudes.extend(layout.remote.iter().copied());
        layout
    }

    /// Unknowns (and equations) in this layout.
    fn len(&self) -> usize {
        self.angles.len() + self.magnitudes.len()
    }

    /// Row and column indices of each bus's variables and equations.
    fn indices(&self, n: usize) -> Indices {
        let mut angle = vec![None; n];
        let mut magnitude = vec![None; n];
        let mut reactive: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for (k, &i) in self.angles.iter().enumerate() {
            angle[i] = Some(k);
        }
        for (k, &i) in self.magnitudes.iter().enumerate() {
            magnitude[i] = Some(k + self.angles.len());
        }
        for (k, &i) in self.q_rows.iter().enumerate() {
            reactive[i].push((k + self.angles.len(), 1.0));
        }
        let first = self.angles.len() + self.q_rows.len();
        for (k, share) in self.shares.iter().enumerate() {
            reactive[share.a].push((first + k, 1.0 / share.weights[0]));
            reactive[share.b].push((first + k, -1.0 / share.weights[1]));
        }
        Indices {
            angle,
            magnitude,
            reactive,
        }
    }
}

/// Per-bus positions in a [`Layout`]'s Jacobian.
struct Indices {
    /// Angle column, which is also the bus's active-power row.
    angle: Vec<Option<usize>>,
    /// Voltage-magnitude column.
    magnitude: Vec<Option<usize>>,
    /// Rows that the bus's reactive mismatch enters, with coefficients.
    reactive: Vec<Vec<(usize, f64)>>,
}

fn mismatch(net: &Network, v: &[C], s: &[C], layout: &Layout) -> Result<(Vec<f64>, f64), Error> {
    let power = net.power(v);
    if power.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
        return Err(Error::Numerical("non-finite power mismatch".into()));
    }
    let q = |i: usize| (power[i] - s[i]).im;
    let f: Vec<_> = layout
        .angles
        .iter()
        .map(|&i| (power[i] - s[i]).re)
        .chain(layout.q_rows.iter().map(|&i| q(i)))
        .chain(layout.shares.iter().map(|x| {
            (q(x.a) + x.scheduled[0]) / x.weights[0] - (q(x.b) + x.scheduled[1]) / x.weights[1]
        }))
        .collect();
    let norm = f.iter().fold(0.0_f64, |a, &b| a.max(b.abs()));
    Ok((f, norm))
}

pub(crate) fn jacobian(
    net: &Network,
    v: &[C],
    angles: &[usize],
    pq: &[usize],
) -> Vec<Triplet<usize, usize, f64>> {
    let layout = Layout {
        angles: angles.to_vec(),
        magnitudes: pq.to_vec(),
        q_rows: pq.to_vec(),
        shares: Vec::new(),
        targets: Vec::new(),
        remote: Vec::new(),
        held: Vec::new(),
    };
    layout_jacobian(net, v, &layout)
}

fn layout_jacobian(net: &Network, v: &[C], layout: &Layout) -> Vec<Triplet<usize, usize, f64>> {
    let at = layout.indices(v.len());
    let mut entries = Vec::new();
    for (i, row) in net.y.iter().enumerate() {
        let current: C = row.iter().map(|(&j, &y)| y * v[j]).sum();
        for (&j, &y) in row {
            let mut da = -C::i() * v[i] * (y * v[j]).conj();
            let mut dm = v[i] * (y * v[j] / v[j].norm()).conj();
            if i == j {
                da += C::i() * v[i] * current.conj();
                dm += v[i] / v[i].norm() * current.conj();
            }
            if let Some(r) = at.angle[i] {
                if let Some(c) = at.angle[j] {
                    entries.push(Triplet::new(r, c, da.re));
                }
                if let Some(c) = at.magnitude[j] {
                    entries.push(Triplet::new(r, c, dm.re));
                }
            }
            for &(r, k) in &at.reactive[i] {
                if let Some(c) = at.angle[j] {
                    entries.push(Triplet::new(r, c, k * da.im));
                }
                if let Some(c) = at.magnitude[j] {
                    entries.push(Triplet::new(r, c, k * dm.im));
                }
            }
        }
    }
    entries
}

/// A detailed DC line during an AC solve.
#[derive(Clone)]
struct LccTerminal<'a> {
    line: &'a LccLine,
    /// Internal bus indices, rectifier then inverter.
    ends: [usize; 2],
    /// Converter base voltages in kV.
    kv: [f64; 2],
    taps: Taps,
    solution: LccSolution,
    targets: TapTargets,
}

impl LccTerminal<'_> {
    fn evaluate(&self, vm: [f64; 2]) -> (LccSolution, TapTargets) {
        dc::operating_point(self.line, vm[0], vm[1], self.kv, self.taps)
    }

    /// Per-unit injections at the two ends (negative: the converters load the bus).
    fn injections(solution: &LccSolution, base_mva: f64) -> [C; 2] {
        [
            -C::new(solution.rectifier_mw, solution.rectifier_mvar) / base_mva,
            C::new(solution.inverter_mw, -solution.inverter_mvar) / base_mva,
        ]
    }
}

/// A detailed VSC line during an AC solve: its terminal generators (from
/// end, then to end of its `dcline` row) and the scale on its power order.
pub(crate) struct VscTerminal<'a> {
    line: &'a VscLine,
    gens: [usize; 2],
    scale: f64,
    state: DcState,
    limited: [bool; 2],
}

impl VscTerminal<'_> {
    /// RAW converter index at row end `e` (0 from, 1 to).
    fn converter(&self, e: usize) -> usize {
        if e == 0 {
            self.line.from
        } else {
            1 - self.line.from
        }
    }

    /// Write the DC solution into the terminal generators.
    fn apply(&self, case: &mut Case) {
        for e in 0..2 {
            let c = self.converter(e);
            let p = self.state.injection_mw[c];
            let g = &mut case.generators[self.gens[e]];
            g.pg = p;
            let conv = &self.line.converters[c];
            if conv.ac_control == VscAcControl::PowerFactor {
                let q = conv.power_factor_q(p);
                (g.qg, g.qmin, g.qmax) = (q, q, q);
            }
        }
    }

    fn solution(&self, case: &Case) -> VscSolution {
        let (f, t) = (self.converter(0), self.converter(1));
        let s = &self.state;
        let current = if f == 0 { s.current_ka } else { -s.current_ka };
        VscSolution {
            row: self.line.row,
            current_ka: current,
            from_kv: s.kv[f],
            to_kv: s.kv[t],
            from_mw: -s.injection_mw[f],
            to_mw: s.injection_mw[t],
            from_mvar: case.generators[self.gens[0]].qg,
            to_mvar: case.generators[self.gens[1]].qg,
            from_loss_mw: s.loss_mw[f],
            to_loss_mw: s.loss_mw[t],
            line_loss_mw: self.line.resistance * current * current,
            order_scale: self.scale,
            from_limited: self.limited[0],
            to_limited: self.limited[1],
        }
    }
}

/// The reported operating point of a detailed VSC line that carries nothing.
fn idle_vsc(line: &VscLine) -> VscSolution {
    VscSolution {
        row: line.row,
        current_ka: 0.0,
        from_kv: 0.0,
        to_kv: 0.0,
        from_mw: 0.0,
        to_mw: 0.0,
        from_mvar: 0.0,
        to_mvar: 0.0,
        from_loss_mw: 0.0,
        to_loss_mw: 0.0,
        line_loss_mw: 0.0,
        order_scale: 1.0,
        from_limited: false,
        to_limited: false,
    }
}

/// Reduce VSC output where a converter exceeds its MVA or current limit.
/// Returns whether anything changed that needs another Newton solve.
fn vsc_limit_pass(
    terminals: &mut [VscTerminal<'_>],
    net: &Network,
    case: &mut Case,
    v: &[C],
    enforce_q_limits: bool,
) -> Result<bool, Error> {
    let mut changed = false;
    for t in terminals.iter_mut() {
        let mut scale = t.scale;
        for e in 0..2 {
            let conv = &t.line.converters[t.converter(e)];
            let g = &case.generators[t.gens[e]];
            let Some(&i) = net.by_id.get(&g.bus) else {
                continue;
            };
            let kv = v[i].norm() * case.buses[net.external[i]].base_kv;
            let limit = conv.mva_limit(kv);
            let s = g.pg.hypot(g.qg);
            if s <= limit * (1.0 + 1e-6) + 1e-6 {
                continue;
            }
            t.limited[e] = true;
            let (p, q) = if conv.ac_control == VscAcControl::PowerFactor {
                // P and Q move together at a fixed power factor.
                (g.pg * limit / s, g.qg * limit / s)
            } else {
                vsc::reduce(g.pg, g.qg, limit, conv.power_weighting)
            };
            if g.pg.abs() > 1e-9 && p.abs() < g.pg.abs() * (1.0 - 1e-9) {
                scale = scale.min(t.scale * p.abs() / g.pg.abs());
            }
            if conv.ac_control == VscAcControl::Voltage && q.abs() < g.qg.abs() * (1.0 - 1e-9) {
                let g = &mut case.generators[t.gens[e]];
                let cap = q.abs();
                if g.qmax > cap || g.qmin < -cap {
                    g.qmax = g.qmax.min(cap);
                    g.qmin = g.qmin.max(-cap);
                    // Tighter limits only act when they are enforced.
                    changed |= enforce_q_limits;
                }
            }
        }
        if scale < t.scale - 1e-12 {
            t.scale = scale;
            t.state = t.line.dc_state(scale).map_err(Error::InvalidCase)?;
            t.apply(case);
            changed = true;
        }
    }
    Ok(changed)
}

/// A detailed multi-terminal DC system during an AC solve.
#[derive(Clone)]
struct MtdcTerminal<'a> {
    system: &'a MtdcSystem,
    topology: mtdc::Topology,
    /// Internal AC bus index of each converter.
    ends: Vec<usize>,
    /// Converter base voltages in kV.
    kv: Vec<f64>,
    taps: Vec<f64>,
    solution: MtdcSolution,
}

impl MtdcTerminal<'_> {
    fn evaluate(&self, vm: &[f64]) -> Result<MtdcSolution, Error> {
        // Start from the latest DC voltages unless the system carried nothing.
        let warm: Vec<f64> = self.solution.buses.iter().map(|b| b.kv).collect();
        let start = self
            .solution
            .converters
            .iter()
            .any(|c| c.mode != MtdcMode::Blocked)
            .then_some(warm.as_slice());
        mtdc::operating_point(self.system, &self.topology, vm, &self.kv, &self.taps, start)
            .map_err(Error::Numerical)
    }

    /// Per-unit injections at each converter's bus (negative: a load).
    fn injections(solution: &MtdcSolution, base_mva: f64) -> Vec<C> {
        solution
            .converters
            .iter()
            .map(|c| -C::new(c.ac_mw, c.ac_mvar) / base_mva)
            .collect()
    }
}

/// A detailed DC line or system whose converters load AC buses.
#[derive(Clone)]
enum DcTerminal<'a> {
    Lcc(LccTerminal<'a>),
    Mtdc(MtdcTerminal<'a>),
}

impl DcTerminal<'_> {
    /// Internal AC bus index of each converter.
    fn ends(&self) -> &[usize] {
        match self {
            Self::Lcc(t) => &t.ends,
            Self::Mtdc(t) => &t.ends,
        }
    }

    /// Injections at the converter buses for magnitudes `vm`, without
    /// storing the operating point.
    fn trial(&self, vm: &[f64], base_mva: f64) -> Result<Vec<C>, Error> {
        Ok(match self {
            Self::Lcc(t) => {
                LccTerminal::injections(&t.evaluate([vm[0], vm[1]]).0, base_mva).to_vec()
            }
            Self::Mtdc(t) => MtdcTerminal::injections(&t.evaluate(vm)?, base_mva),
        })
    }

    /// Store the operating point at magnitudes `vm`.
    fn update(&mut self, vm: &[f64]) -> Result<(), Error> {
        match self {
            Self::Lcc(t) => (t.solution, t.targets) = t.evaluate([vm[0], vm[1]]),
            Self::Mtdc(t) => t.solution = t.evaluate(vm)?,
        }
        Ok(())
    }

    /// Injections of the stored operating point.
    fn injections(&self, base_mva: f64) -> Vec<C> {
        match self {
            Self::Lcc(t) => LccTerminal::injections(&t.solution, base_mva).to_vec(),
            Self::Mtdc(t) => MtdcTerminal::injections(&t.solution, base_mva),
        }
    }

    /// Step the converter taps; returns whether any moved.
    fn adjust_taps(&mut self, vm: &[f64]) -> bool {
        match self {
            Self::Lcc(t) => {
                let next = dc::adjust_taps(
                    t.line,
                    &t.solution,
                    &t.targets,
                    [vm[0], vm[1]],
                    t.kv,
                    t.taps,
                );
                std::mem::replace(&mut t.taps, next) != next
            }
            Self::Mtdc(t) => {
                let next =
                    mtdc::adjust_taps(t.system, &t.topology, &t.solution, vm, &t.kv, &t.taps);
                let changed = next != t.taps;
                t.taps = next;
                changed
            }
        }
    }
}

/// Terminal voltage magnitudes of a detailed DC line or system.
fn end_magnitudes(t: &DcTerminal<'_>, v: &[C]) -> Vec<f64> {
    t.ends().iter().map(|&i| v[i].norm()).collect()
}

/// Update every detailed DC line at voltages `v`; return per-bus injections.
fn dc_injections(
    terminals: &mut [DcTerminal<'_>],
    v: &[C],
    base_mva: f64,
) -> Result<Vec<C>, Error> {
    let mut s = vec![C::new(0.0, 0.0); v.len()];
    for t in terminals {
        let vm = end_magnitudes(t, v);
        t.update(&vm)?;
        for (&i, z) in t.ends().iter().zip(t.injections(base_mva)) {
            s[i] += z;
        }
    }
    Ok(s)
}

/// Jacobian terms for the DC injections' dependence on terminal voltage
/// magnitudes, by central differences of the operating point.
fn dc_jacobian(
    terminals: &[DcTerminal<'_>],
    v: &[C],
    layout: &Layout,
    base_mva: f64,
) -> Result<Vec<Triplet<usize, usize, f64>>, Error> {
    if terminals.is_empty() {
        return Ok(Vec::new());
    }
    let at = layout.indices(v.len());
    let mut entries = Vec::new();
    for t in terminals {
        let ends = t.ends();
        let vm = end_magnitudes(t, v);
        for (j, &bus) in ends.iter().enumerate() {
            // Converters sharing a bus move together; take each bus once.
            if ends[..j].contains(&bus) {
                continue;
            }
            let Some(col) = at.magnitude[bus] else {
                continue;
            };
            let h = 1e-6 * vm[j].max(1e-3);
            let (mut plus, mut minus) = (vm.clone(), vm.clone());
            for (k, &other) in ends.iter().enumerate() {
                if other == bus {
                    plus[k] += h;
                    minus[k] -= h;
                }
            }
            let sp = t.trial(&plus, base_mva)?;
            let sm = t.trial(&minus, base_mva)?;
            for (k, &end) in ends.iter().enumerate() {
                // The mismatch is power(v) − s, so its derivative is −ds/dVm.
                let d = -(sp[k] - sm[k]) / (2.0 * h);
                if let Some(row) = at.angle[end] {
                    entries.push(Triplet::new(row, col, d.re));
                }
                for &(row, c) in &at.reactive[end] {
                    entries.push(Triplet::new(row, col, c * d.im));
                }
            }
        }
    }
    Ok(entries)
}

fn newton(
    net: &Network,
    case: &Case,
    kinds: &[BusType],
    v: &mut [C],
    terminals: &mut [DcTerminal<'_>],
    opt: &PowerFlowOptions,
) -> Result<(SolveStatus, usize, f64), Error> {
    let layout = if opt.enforce_q_limits {
        Layout::new(net, case, kinds)
    } else {
        Layout::plain(kinds)
    };
    for &(i, vm) in &layout.targets {
        v[i] = C::from_polar(vm, v[i].arg());
    }
    let base = net.injections(case);
    log::debug!(
        "starting Newton solve ({} angle variables, {} magnitude variables, {} remotely held buses, {} detailed DC lines)",
        layout.angles.len(),
        layout.magnitudes.len(),
        layout.targets.len(),
        terminals.len()
    );
    for iteration in 0..=opt.max_iterations {
        let s = if terminals.is_empty() {
            base.clone()
        } else {
            let dc = dc_injections(terminals, v, case.base_mva)?;
            base.iter().zip(dc).map(|(a, b)| a + b).collect()
        };
        let (f, residual) = mismatch(net, v, &s, &layout)?;
        if residual < opt.tolerance {
            log::debug!("Newton solve converged at iteration {iteration}: residual={residual:.6e}");
            return Ok((SolveStatus::Converged, iteration, residual));
        }
        if iteration == opt.max_iterations {
            log::warn!("Newton solve reached iteration limit {iteration}: residual={residual:.6e}");
            return Ok((SolveStatus::IterationLimit, iteration, residual));
        }
        debug_assert_eq!(f.len(), layout.len());
        let mut j = layout_jacobian(net, v, &layout);
        j.extend(dc_jacobian(terminals, v, &layout, case.base_mva)?);
        let dx = linear(f.len(), &j, &f.iter().map(|x| -x).collect::<Vec<_>>())?;
        let mut va: Vec<_> = v.iter().map(|z| z.arg()).collect();
        let mut vm: Vec<_> = v.iter().map(|z| z.norm()).collect();
        let scale = step_scale(&dx[layout.angles.len()..], &layout.magnitudes, &vm);
        if scale < 1.0 {
            log::debug!(
                "Newton iteration {iteration}: step scaled by {scale:.3e} to keep voltage magnitudes positive"
            );
        }
        for (k, &i) in layout.angles.iter().enumerate() {
            va[i] += scale * dx[k];
        }
        for (k, &i) in layout.magnitudes.iter().enumerate() {
            vm[i] += scale * dx[k + layout.angles.len()];
        }
        for i in 0..v.len() {
            if !vm[i].is_finite() || vm[i] <= 0.0 || !va[i].is_finite() {
                return Err(Error::Numerical(
                    "Newton step produced a non-positive or non-finite voltage magnitude".into(),
                ));
            }
            v[i] = C::from_polar(vm[i], va[i]);
        }
    }
    unreachable!()
}

/// Largest relative magnitude change a Newton step may make at any bus.
const MAX_MAGNITUDE_STEP: f64 = 0.5;

/// Factor that shortens a Newton step so that no voltage magnitude changes
/// by more than [`MAX_MAGNITUDE_STEP`] of its present value. Ordinary steps
/// are far smaller and keep the full Newton step.
fn step_scale(dvm: &[f64], buses: &[usize], vm: &[f64]) -> f64 {
    dvm.iter()
        .zip(buses)
        .map(|(&d, &i)| MAX_MAGNITUDE_STEP * vm[i] / d.abs())
        .fold(1.0, f64::min)
}

fn allocate(
    net: &Network,
    case: &mut Case,
    kinds: &[BusType],
    references: &[usize],
    v: &[C],
    terminals: &[DcTerminal<'_>],
) {
    let mut power = net.power(v);
    // Generators supply the network injection plus what the converters draw.
    for t in terminals {
        for (&i, z) in t.ends().iter().zip(t.injections(case.base_mva)) {
            power[i] -= z;
        }
    }
    for (i, gens) in net.gens.iter().enumerate() {
        if kinds[i] == BusType::Pq || gens.is_empty() {
            continue;
        }
        let b = &case.buses[net.external[i]];
        let total = power[i].im * case.base_mva + b.qd;
        let proxy = total.abs()
            + gens
                .iter()
                .map(|&g| {
                    let g = &case.generators[g];
                    (if g.qmin.is_finite() {
                        g.qmin.abs()
                    } else {
                        0.0
                    }) + (if g.qmax.is_finite() {
                        g.qmax.abs()
                    } else {
                        0.0
                    })
                })
                .sum::<f64>();
        let limits: Vec<_> = gens
            .iter()
            .map(|&g| {
                let g = &case.generators[g];
                (
                    if g.qmin.is_infinite() {
                        g.qmin.signum() * proxy
                    } else {
                        g.qmin
                    },
                    if g.qmax.is_infinite() {
                        g.qmax.signum() * proxy
                    } else {
                        g.qmax
                    },
                )
            })
            .collect();
        let lo: f64 = limits.iter().map(|x| x.0).sum();
        let hi: f64 = limits.iter().map(|x| x.1).sum();
        for (k, &g) in gens.iter().enumerate() {
            case.generators[g].qg = if gens.len() == 1 {
                total
            } else if (hi - lo).abs() < 10.0 * f64::EPSILON {
                limits[k].0 + (total - lo) / gens.len() as f64
            } else {
                limits[k].0 + (total - lo) / (hi - lo + f64::EPSILON) * (limits[k].1 - limits[k].0)
            };
        }
    }
    for &reference in references {
        let gens = &net.gens[reference];
        let other: f64 = gens.iter().skip(1).map(|&g| case.generators[g].pg).sum();
        case.generators[gens[0]].pg =
            power[reference].re * case.base_mva + case.buses[net.external[reference]].pd - other;
    }
}

fn dc(net: &Network, mut case: Case, opt: &PowerFlowOptions) -> Result<PowerFlowResult, Error> {
    let n = net.external.len();
    let vars: Vec<_> = (0..n).filter(|i| !net.references.contains(i)).collect();
    log::debug!(
        "starting DC solve ({n} buses, {} variables, {} branches)",
        vars.len(),
        net.edges.len()
    );
    let mut idx = vec![None; n];
    for (k, &i) in vars.iter().enumerate() {
        idx[i] = Some(k);
    }
    let mut entries = Vec::new();
    let mut va: Vec<_> = net
        .external
        .iter()
        .map(|&e| case.buses[e].va.to_radians())
        .collect();
    let mut injection: Vec<_> = net
        .injections(&case)
        .iter()
        .enumerate()
        .map(|(i, s)| s.re - case.buses[net.external[i]].gs / case.base_mva)
        .collect();
    let mut coeffs = Vec::new();
    for e in &net.edges {
        let b = &case.branches[e.original];
        if b.x == 0.0 {
            return Err(Error::InvalidCase(
                "DC flow requires nonzero active branch reactance".into(),
            ));
        }
        let coeff = 1.0 / b.x / if b.tap == 0.0 { 1.0 } else { b.tap };
        let shift = -coeff * b.shift.to_radians();
        coeffs.push((coeff, shift));
        injection[e.f] -= shift;
        injection[e.t] += shift;
        for (i, j, x) in [
            (e.f, e.f, coeff),
            (e.f, e.t, -coeff),
            (e.t, e.f, -coeff),
            (e.t, e.t, coeff),
        ] {
            if let Some(r) = idx[i] {
                if let Some(c) = idx[j] {
                    entries.push(Triplet::new(r, c, x));
                } else {
                    injection[i] -= x * va[j];
                }
            }
        }
    }
    let x = linear(
        vars.len(),
        &entries,
        &vars.iter().map(|&i| injection[i]).collect::<Vec<_>>(),
    )?;
    for (k, &i) in vars.iter().enumerate() {
        va[i] = x[k];
    }
    let mut flows = vec![[0.0; 4]; case.branches.len()];
    let mut balance = vec![0.0; n];
    for (e, (coeff, shift)) in net.edges.iter().zip(coeffs) {
        let p = coeff * (va[e.f] - va[e.t]) + shift;
        flows[e.original] = [p * case.base_mva, 0.0, -p * case.base_mva, 0.0];
        balance[e.f] += p;
        balance[e.t] -= p;
    }
    let expected = net.injections(&case);
    let residual = vars
        .iter()
        .map(|&i| {
            (balance[i] - expected[i].re + case.buses[net.external[i]].gs / case.base_mva).abs()
        })
        .fold(0.0_f64, f64::max);
    if !residual.is_finite() || residual >= opt.tolerance {
        return Err(Error::Numerical(
            "DC solution failed power balance tolerance".into(),
        ));
    }
    log::debug!("DC solve converged: residual={residual:.6e}");
    for &reference in &net.references {
        let gens = &net.gens[reference];
        let other: f64 = gens.iter().skip(1).map(|&g| case.generators[g].pg).sum();
        let b = &case.buses[net.external[reference]];
        case.generators[gens[0]].pg = balance[reference] * case.base_mva + b.pd + b.gs - other;
    }
    for (i, &e) in net.external.iter().enumerate() {
        case.buses[e].vm = 1.0;
        case.buses[e].va = va[i].to_degrees();
    }
    zero_offline(net, &mut case);
    Ok(PowerFlowResult {
        case,
        branch_flows: flows,
        status: SolveStatus::Converged,
        iterations: 1,
        residual,
        q_limit_passes: 0,
        lcc: Vec::new(),
        dc_tap_passes: 0,
        vsc: Vec::new(),
        vsc_limit_passes: 0,
        switched_shunts: Vec::new(),
        tap_ratios: Vec::new(),
        voltage_control_passes: 0,
        mtdc: Vec::new(),
    })
}

fn zero_offline(net: &Network, case: &mut Case) {
    for g in &mut case.generators {
        if !g.online || !net.by_id.contains_key(&g.bus) {
            g.pg = 0.0;
            g.qg = 0.0;
        }
    }
}

fn solve_core(
    input: &Case,
    opt: &PowerFlowOptions,
    lines: &[&LccLine],
    systems: &[&MtdcSystem],
    adjust_taps: bool,
    vsc_terminals: &mut [VscTerminal<'_>],
    setup: &Setup<'_>,
) -> Result<PowerFlowResult, Error> {
    if !opt.tolerance.is_finite()
        || opt.tolerance <= 0.0
        || !opt.q_limit_tolerance.is_finite()
        || opt.q_limit_tolerance < 0.0
    {
        return Err(Error::InvalidCase("invalid solver tolerance".into()));
    }
    if opt.model == Model::Dc && opt.enforce_q_limits {
        return Err(Error::Unsupported(
            "reactive-limit enforcement is only available for AC".into(),
        ));
    }
    let mut net = Network::new(input)?;
    let mut case = input.clone();
    if opt.model == Model::Dc {
        return dc(&net, case, opt);
    }
    let base_kv = |c: &dc::LccConverter, name: &str| -> Result<(usize, f64), Error> {
        let Some(&i) = net.by_id.get(&c.bus) else {
            return Err(Error::InvalidCase(format!(
                "DC line {name} references an unknown or isolated bus {}",
                c.bus
            )));
        };
        let kv = if c.base_kv > 0.0 {
            c.base_kv
        } else {
            case.buses[net.external[i]].base_kv
        };
        if kv.is_nan() || kv <= 0.0 {
            return Err(Error::InvalidCase(format!(
                "DC line {name} needs a converter base voltage (EBAS or bus base kV)"
            )));
        }
        Ok((i, kv))
    };
    let mut controller = Controller::new(setup, &net, &case);
    let mut terminals = Vec::with_capacity(lines.len() + systems.len());
    for &line in lines {
        let mut ends = [0; 2];
        let mut kv = [0.0; 2];
        for (k, c) in [&line.rectifier, &line.inverter].into_iter().enumerate() {
            (ends[k], kv[k]) = base_kv(c, &line.name)?;
        }
        let taps = [line.rectifier.tap, line.inverter.tap];
        let (solution, targets) = dc::operating_point(line, 1.0, 1.0, kv, taps);
        terminals.push(DcTerminal::Lcc(LccTerminal {
            line,
            ends,
            kv,
            taps,
            solution,
            targets,
        }));
    }
    for &system in systems {
        let topology = system.topology().map_err(Error::InvalidCase)?;
        let (mut ends, mut kv) = (Vec::new(), Vec::new());
        for c in &system.converters {
            let (i, k) = base_kv(&c.converter, &system.name)?;
            ends.push(i);
            kv.push(k);
        }
        let taps: Vec<f64> = system.converters.iter().map(|c| c.converter.tap).collect();
        let solution =
            mtdc::operating_point(system, &topology, &vec![1.0; ends.len()], &kv, &taps, None)
                .map_err(Error::Numerical)?;
        terminals.push(DcTerminal::Mtdc(MtdcTerminal {
            system,
            topology,
            ends,
            kv,
            taps,
            solution,
        }));
    }
    let mut kinds = net.kinds.clone();
    let mut references = net.references.clone();
    let mut v = vec![C::new(1.0, 0.0); net.external.len()];
    for (island, &reference) in net.islands.iter().zip(&net.references) {
        let reference_angle = case.buses[net.external[reference]].va.to_radians();
        for &i in island {
            let b = &case.buses[net.external[i]];
            v[i] = match opt.initialization {
                Initialization::Case => C::from_polar(b.vm, b.va.to_radians()),
                Initialization::Flat => C::from_polar(1.0, reference_angle),
            };
        }
    }
    // Remote regulation needs reactive limits: without them a small unit
    // holding a distant bus drives its own terminal to any voltage, so those
    // generators hold their own bus instead, as in MATPOWER.
    let (remote, held) = if opt.enforce_q_limits {
        let layout = Layout::new(&net, &case, &kinds);
        (layout.remote, layout.held)
    } else {
        (Vec::new(), Vec::new())
    };
    // Buses holding a remote voltage keep their starting magnitude. Those
    // whose generators all start at a reactive limit start there as PQ
    // buses, which a solved case needs to converge; they return to holding
    // the remote voltage when the limit-check pass finds it on the wrong side.
    let mut released = vec![None; kinds.len()];
    if opt.enforce_q_limits {
        let mut started = 0;
        for &i in &remote {
            let at_limit = |g: &Generator| {
                g.qg >= g.qmax - opt.q_limit_tolerance || g.qg <= g.qmin + opt.q_limit_tolerance
            };
            let gens = &net.gens[i];
            if gens.iter().all(|&g| at_limit(&case.generators[g])) {
                let mut upper = 0;
                for &g in gens {
                    let gen_data = &mut case.generators[g];
                    if gen_data.qg >= gen_data.qmax - opt.q_limit_tolerance {
                        upper += 1;
                    }
                    gen_data.qg = gen_data.qg.clamp(gen_data.qmin, gen_data.qmax);
                }
                kinds[i] = BusType::Pq;
                released[i] = Some(2 * upper >= gens.len());
                started += 1;
            }
        }
        if started > 0 {
            log::debug!(
                "{started} remote-regulating generator buses start at their reactive limits"
            );
        }
    }
    for (i, gens) in net.gens.iter().enumerate() {
        if kinds[i] != BusType::Pq && remote.binary_search(&i).is_err() {
            for &g in gens {
                v[i] = C::from_polar(case.generators[g].vg, v[i].arg());
            }
        }
    }
    let mut restores = vec![0; remote.len()];
    let (mut iterations, mut passes, mut tap_passes, mut vsc_passes) = (0, 0, 0, 0);
    let mut control_passes = 0;
    let mut round = 0;
    // State before the last control pass, to go back to if the solve after
    // it fails.
    let mut checkpoint = None;
    let (status, residual) = loop {
        round += 1;
        let (status, it, residual) = newton(&net, &case, &kinds, &mut v, &mut terminals, opt)?;
        iterations += it;
        if status != SolveStatus::Converged
            && let Some((earlier, saved_case, saved_v, saved_terminals)) = checkpoint.take()
        {
            let held = controller.undo(earlier);
            log::warn!(
                "solve failed after voltage control pass {control_passes}; undoing it and holding {held} shunts or taps"
            );
            (case, v, terminals) = (saved_case, saved_v, saved_terminals);
            net = Network::new(&case)?;
            continue;
        }
        checkpoint = None;
        allocate(&net, &mut case, &kinds, &references, &v, &terminals);
        if status != SolveStatus::Converged {
            break (status, residual);
        }
        if adjust_taps && tap_passes < MAX_TAP_PASSES {
            let mut changed = 0;
            for t in &mut terminals {
                let vm = end_magnitudes(t, &v);
                if t.adjust_taps(&vm) {
                    changed += 1;
                }
            }
            if changed > 0 {
                tap_passes += 1;
                log::debug!("DC tap pass {tap_passes} changed taps on {changed} lines");
                if tap_passes == MAX_TAP_PASSES {
                    log::warn!(
                        "DC converter taps still moving after {MAX_TAP_PASSES} passes; holding them"
                    );
                }
                continue;
            }
        }
        if vsc_passes < MAX_TAP_PASSES
            && vsc_limit_pass(vsc_terminals, &net, &mut case, &v, opt.enforce_q_limits)?
        {
            vsc_passes += 1;
            log::debug!("VSC limit pass {vsc_passes} reduced converter output");
            continue;
        }
        // Continuous-shunt helpers always keep to their range.
        let limited = |g: usize| opt.enforce_q_limits || g >= setup.limited_from;
        let limits = opt.enforce_q_limits || !setup.continuous.is_empty();
        let mut violations = Vec::new();
        let mut remaining = Vec::new();
        for (i, gens) in net.gens.iter().enumerate().filter(|_| limits) {
            if kinds[i] != BusType::Pq {
                remaining.extend(gens.iter().copied());
            }
            for &g in gens.iter().filter(|&&g| limited(g)) {
                let gen_data = &case.generators[g];
                if gen_data.qg > gen_data.qmax + opt.q_limit_tolerance {
                    violations.push((i, g, gen_data.qmax, true));
                } else if gen_data.qg < gen_data.qmin - opt.q_limit_tolerance {
                    violations.push((i, g, gen_data.qmin, false));
                }
            }
        }
        // A remote-regulating bus at a limit holds its bus again once that
        // bus's voltage is past the setpoint on the side the limit pushes.
        let mut restored = Vec::new();
        for (k, &i) in remote.iter().enumerate() {
            let Some(upper) = released[i] else { continue };
            if restores[k] >= MAX_RESTORES {
                continue;
            }
            let setpoint = case.generators[net.gens[i][0]].vg;
            let vm = v[held[k]].norm();
            if (upper && vm > setpoint + RESTORE_TOLERANCE)
                || (!upper && vm < setpoint - RESTORE_TOLERANCE)
            {
                restored.push(k);
            }
        }
        if violations.is_empty() && restored.is_empty() {
            // Shunts and taps move once the generators have settled, so they
            // see the final reactive limits rather than fight them.
            if !controller.is_empty() && control_passes < MAX_CONTROL_PASSES {
                let saved = (
                    controller.clone(),
                    case.clone(),
                    v.clone(),
                    terminals.clone(),
                );
                let moved = controller.pass(&mut case, &v, &kinds, round);
                if moved > 0 {
                    control_passes += 1;
                    log::debug!(
                        "voltage control pass {control_passes} moved {moved} shunts or taps"
                    );
                    if control_passes == MAX_CONTROL_PASSES {
                        log::warn!(
                            "switched shunts or taps still moving after {MAX_CONTROL_PASSES} passes; holding them"
                        );
                    }
                    net = Network::new(&case)?;
                    checkpoint = Some(saved);
                    continue;
                }
            }
            break (status, residual);
        }
        log::warn!(
            "reactive-limit pass {} is enforcing {} generator violations and returning {} remote-regulating buses to voltage control",
            passes + 1,
            violations.len(),
            restored.len()
        );
        let mut island_of = vec![0; net.external.len()];
        for (k, island) in net.islands.iter().enumerate() {
            for &i in island {
                island_of[i] = k;
            }
        }
        let mut gen_island = vec![usize::MAX; case.generators.len()];
        for (i, gens) in net.gens.iter().enumerate() {
            for &g in gens {
                gen_island[g] = island_of[i];
            }
        }
        let mut violating_bus = vec![false; net.external.len()];
        let mut violating_gen = vec![false; case.generators.len()];
        for &(i, g, _, _) in &violations {
            violating_bus[i] = true;
            violating_gen[g] = true;
        }
        for (k, island) in net.islands.iter().enumerate() {
            let island_remaining: Vec<_> = remaining
                .iter()
                .copied()
                .filter(|&g| gen_island[g] == k)
                .collect();
            let island_violations: Vec<_> = violations
                .iter()
                .filter(|violation| island_of[violation.0] == k)
                .collect();
            let all_generators_violate_one_side = !island_remaining.is_empty()
                && island_remaining.len() == island_violations.len()
                && island_remaining.iter().all(|&g| violating_gen[g])
                && island_violations
                    .iter()
                    .all(|x| x.3 == island_violations[0].3);
            let no_reference_candidate = island
                .iter()
                .all(|&i| kinds[i] == BusType::Pq || violating_bus[i]);
            if all_generators_violate_one_side || no_reference_candidate {
                return finish_ac(
                    input,
                    &net,
                    case,
                    kinds,
                    references,
                    v,
                    SolveStatus::ReactiveLimitsInfeasible,
                    iterations,
                    residual,
                    [passes, tap_passes, vsc_passes],
                    &terminals,
                    vsc_terminals,
                )
                .map(|r| with_controls(r, &controller, opt, control_passes));
            }
        }
        let mut fixed = false;
        for &(i, g, limit, upper) in &violations {
            kinds[i] = BusType::Pq;
            if g >= setup.limited_from {
                // A continuous shunt at an end of its range is a fixed shunt
                // from then on, so its output follows the voltage squared.
                fixed |= controller.fix_continuous(&mut case, net.external[i], g, limit);
                continue;
            }
            case.generators[g].qg = limit;
            if remote.binary_search(&i).is_ok() {
                released[i] = Some(upper);
            }
        }
        for k in restored {
            let i = remote[k];
            if kinds[i] == BusType::Pq {
                kinds[i] = BusType::Pv;
            }
            released[i] = None;
            restores[k] += 1;
        }
        if fixed {
            net = Network::new(&case)?;
        }
        passes += 1;
        for (island, reference) in net.islands.iter().zip(&mut references) {
            if kinds[*reference] == BusType::Pq {
                let i = island
                    .iter()
                    .copied()
                    .find(|&i| kinds[i] == BusType::Pv)
                    .expect("reference candidate checked above");
                *reference = i;
                kinds[i] = BusType::Reference;
            }
        }
        if passes > case.generators.len() {
            return Err(Error::Numerical(
                "reactive-limit loop did not terminate".into(),
            ));
        }
    };
    finish_ac(
        input,
        &net,
        case,
        kinds,
        references,
        v,
        status,
        iterations,
        residual,
        [passes, tap_passes, vsc_passes],
        &terminals,
        vsc_terminals,
    )
    .map(|r| with_controls(r, &controller, opt, control_passes))
}

/// Record the switched shunts' and taps' final settings on a result.
fn with_controls(
    mut result: PowerFlowResult,
    controller: &Controller<'_>,
    opt: &PowerFlowOptions,
    passes: usize,
) -> PowerFlowResult {
    (result.switched_shunts, result.tap_ratios) = controller.values(Some(&opt.voltage_controls));
    result.voltage_control_passes = passes;
    result
}

/// Voltage control passes before switched shunts and taps are held.
const MAX_CONTROL_PASSES: usize = 30;

/// Times a remote-regulating bus may return from a reactive limit to
/// voltage control, so that the limit loop cannot cycle.
const MAX_RESTORES: usize = 3;

/// How far past its setpoint (per unit) a remotely held bus must be before
/// a generator at a reactive limit takes control of it again.
const RESTORE_TOLERANCE: f64 = 1e-4;

/// Tap passes before the DC converter taps are held where they are.
const MAX_TAP_PASSES: usize = 20;

#[allow(clippy::too_many_arguments)]
fn finish_ac(
    input: &Case,
    net: &Network,
    mut case: Case,
    kinds: Vec<BusType>,
    references: Vec<usize>,
    v: Vec<C>,
    status: SolveStatus,
    iterations: usize,
    residual: f64,
    [passes, tap_passes, vsc_passes]: [usize; 3],
    terminals: &[DcTerminal<'_>],
    vsc_terminals: &[VscTerminal<'_>],
) -> Result<PowerFlowResult, Error> {
    let mut offsets = vec![0.0; net.external.len()];
    for ((island, &original), &reference) in
        net.islands.iter().zip(&net.references).zip(&references)
    {
        let offset = if passes > 0 && reference != original {
            input.buses[net.external[original]].va - v[original].arg().to_degrees()
        } else {
            0.0
        };
        for &i in island {
            offsets[i] = offset;
        }
    }
    for (i, &e) in net.external.iter().enumerate() {
        case.buses[e].vm = v[i].norm();
        case.buses[e].va = v[i].arg().to_degrees() + offsets[i];
        case.buses[e].kind = kinds[i];
    }
    let mut flows = vec![[0.0; 4]; case.branches.len()];
    for e in &net.edges {
        let sf = v[e.f] * (e.yff * v[e.f] + e.yft * v[e.t]).conj() * case.base_mva;
        let st = v[e.t] * (e.ytf * v[e.f] + e.ytt * v[e.t]).conj() * case.base_mva;
        flows[e.original] = [sf.re, sf.im, st.re, st.im];
    }
    zero_offline(net, &mut case);
    let vsc = vsc_terminals.iter().map(|t| t.solution(&case)).collect();
    Ok(PowerFlowResult {
        case,
        branch_flows: flows,
        status,
        iterations,
        residual,
        q_limit_passes: passes,
        lcc: terminals
            .iter()
            .filter_map(|t| match t {
                DcTerminal::Lcc(t) => Some(t.solution.clone()),
                DcTerminal::Mtdc(_) => None,
            })
            .collect(),
        dc_tap_passes: tap_passes,
        vsc,
        vsc_limit_passes: vsc_passes,
        switched_shunts: Vec::new(),
        tap_ratios: Vec::new(),
        voltage_control_passes: 0,
        mtdc: terminals
            .iter()
            .filter_map(|t| match t {
                DcTerminal::Mtdc(t) => Some(t.solution.clone()),
                DcTerminal::Lcc(_) => None,
            })
            .collect(),
    })
}

pub(crate) struct DcLineGenerators {
    row: usize,
    from_generator: usize,
    to_generator: usize,
}

fn dc_bus(value: f64) -> Option<u32> {
    if value.is_finite() && value >= 1.0 && value <= u32::MAX as f64 && value.fract() == 0.0 {
        Some(value as u32)
    } else {
        None
    }
}

pub(crate) fn add_dc_line_generators(case: &mut Case) -> Result<Vec<DcLineGenerators>, Error> {
    add_dc_line_generators_except(case, &[])
}

/// As [`add_dc_line_generators`], leaving out the rows marked in `skip`
/// (detailed DC lines), which are still validated.
fn add_dc_line_generators_except(
    case: &mut Case,
    skip: &[bool],
) -> Result<Vec<DcLineGenerators>, Error> {
    let Some(lines) = &case.dcline else {
        return Ok(Vec::new());
    };
    let buses: std::collections::HashMap<_, _> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, bus)| (bus.id, i))
        .collect();
    let original_generators = case.generators.len();
    let mut mappings = Vec::new();
    let mut from_generators = Vec::new();
    let mut to_generators = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        if line.len() < 17 {
            return Err(Error::InvalidCase(
                "dcline requires at least 17 columns".into(),
            ));
        }
        if line.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidCase("dcline values must be finite".into()));
        }
        let Some(from) = dc_bus(line[0]) else {
            return Err(Error::InvalidCase("invalid dcline from bus".into()));
        };
        let Some(to) = dc_bus(line[1]) else {
            return Err(Error::InvalidCase("invalid dcline to bus".into()));
        };
        let (Some(&from_bus), Some(&to_bus)) = (buses.get(&from), buses.get(&to)) else {
            return Err(Error::InvalidCase("dcline references unknown bus".into()));
        };
        if line[2] != 0.0 && line[2] != 1.0 {
            return Err(Error::InvalidCase("dcline status must be 0 or 1".into()));
        }
        if line[9] > line[10] || line[11] > line[12] || line[13] > line[14] {
            return Err(Error::InvalidCase("invalid dcline power limits".into()));
        }
        if line[7] <= 0.0 || line[8] <= 0.0 {
            return Err(Error::InvalidCase(
                "dcline voltage setpoints must be positive".into(),
            ));
        }
        if line[2] == 0.0 || skip.get(row).copied().unwrap_or(false) {
            continue;
        }
        if case.buses[from_bus].kind == BusType::Isolated
            || case.buses[to_bus].kind == BusType::Isolated
        {
            return Err(Error::InvalidCase(
                "active dcline references isolated bus".into(),
            ));
        }
        // A terminal with a zero-width reactive range (a VSC end in power
        // factor control) injects that fixed Q instead of holding voltage.
        let fixed_from = line[11] == line[12];
        let fixed_to = line[13] == line[14];
        if !fixed_from && case.buses[from_bus].kind != BusType::Reference {
            case.buses[from_bus].kind = BusType::Pv;
        }
        if !fixed_to && case.buses[to_bus].kind != BusType::Reference {
            case.buses[to_bus].kind = BusType::Pv;
        }
        let from_generator = original_generators + from_generators.len();
        from_generators.push(Generator {
            bus: from,
            pg: -line[3],
            qg: if fixed_from { line[11] } else { line[5] },
            qmax: line[12],
            qmin: line[11],
            vg: line[7],
            mbase: 100.0,
            online: true,
            pmax: f64::INFINITY,
            pmin: f64::NEG_INFINITY,
            extra: Vec::new(),
            regulated_bus: None,
            reactive_share: 100.0,
        });
        to_generators.push(Generator {
            bus: to,
            pg: line[3] - line[15] - line[16] * line[3],
            qg: if fixed_to { line[13] } else { line[6] },
            qmax: line[14],
            qmin: line[13],
            vg: line[8],
            mbase: 100.0,
            online: true,
            pmax: f64::INFINITY,
            pmin: f64::NEG_INFINITY,
            extra: Vec::new(),
            regulated_bus: None,
            reactive_share: 100.0,
        });
        mappings.push(DcLineGenerators {
            row,
            from_generator,
            to_generator: 0,
        });
    }
    let from_count = from_generators.len();
    for (index, mapping) in mappings.iter_mut().enumerate() {
        mapping.to_generator = original_generators + from_count + index;
    }
    case.generators.extend(from_generators);
    case.generators.extend(to_generators);
    Ok(mappings)
}

/// The operating point reported for a detailed line that is switched off.
fn blocked(line: &LccLine) -> LccSolution {
    let mut off = line.clone();
    off.control = dc::LccControl::Blocked;
    let taps = [line.rectifier.tap, line.inverter.tap];
    dc::operating_point(&off, 1.0, 1.0, [1.0, 1.0], taps).0
}

/// The operating point reported for a multi-terminal system that is blocked.
fn idle_mtdc(system: &MtdcSystem) -> Result<MtdcSolution, Error> {
    let mut off = system.clone();
    off.control = dc::LccControl::Blocked;
    let topology = off.topology().map_err(Error::InvalidCase)?;
    let n = off.converters.len();
    let taps: Vec<f64> = off.converters.iter().map(|c| c.converter.tap).collect();
    mtdc::operating_point(&off, &topology, &vec![1.0; n], &vec![1.0; n], &taps, None)
        .map_err(Error::Numerical)
}

/// Report a multi-terminal system in its `dcline` rows. Each row links one
/// converter to the voltage-controlling converter. The converter's end
/// carries its own power; the voltage converter's end carries a share of
/// the voltage converters' reactive power and of the system's losses, in
/// proportion to each converter's DC power, so the rows add up to the
/// system's losses.
fn write_mtdc_rows(
    system: &MtdcSystem,
    solution: &MtdcSolution,
    lines: &mut [Vec<f64>],
    vm: &impl Fn(u32) -> f64,
) {
    let loss = solution.line_loss_mw + solution.converter_loss_mw;
    let pairs: Vec<_> = system.converters.iter().zip(&solution.converters).collect();
    let held_q: f64 = pairs
        .iter()
        .filter(|(c, _)| c.row.is_none())
        .map(|(_, s)| s.ac_mvar)
        .sum();
    let weight = |s: &mtdc::MtdcConverterSolution| (s.dc_kv * s.current_ka).abs();
    let total: f64 = pairs
        .iter()
        .filter(|(c, _)| c.row.is_some())
        .map(|(_, s)| weight(s))
        .sum();
    for (c, s) in &pairs {
        let Some(r) = c.row else { continue };
        let share = if total > 0.0 { weight(s) / total } else { 0.0 };
        let row = &mut lines[r];
        if row[0] as u32 == c.converter.bus {
            row[3] = s.ac_mw;
            row[4] = s.ac_mw - loss * share;
            row[5] = -s.ac_mvar;
            row[6] = -held_q * share;
        } else {
            row[4] = -s.ac_mw;
            row[3] = -s.ac_mw + loss * share;
            row[6] = -s.ac_mvar;
            row[5] = -held_q * share;
        }
        row[7] = vm(row[0] as u32);
        row[8] = vm(row[1] as u32);
    }
}

/// Validate and solve a case. Each active electrical island needs one reference bus.
/// Active MATPOWER DC lines are represented internally by a generator at each
/// terminal. Input data is never modified.
///
/// # Errors
///
/// Returns [`Error::InvalidCase`] for invalid data or options,
/// [`Error::Unsupported`] for unsupported configurations, and
/// [`Error::Numerical`] for failed numerical operations. Nonconvergence and
/// reactive-limit infeasibility return an `Ok` result with a non-converged
/// [`SolveStatus`].
pub fn solve(input: &Case, opt: &PowerFlowOptions) -> Result<PowerFlowResult, Error> {
    let started = std::time::Instant::now();
    log::info!(
        "starting {:?} power-flow solve: {} buses, {} generators, {} branches, initialization={:?}, tolerance={:.6e}, max_iterations={}, enforce_q_limits={}",
        opt.model,
        input.buses.len(),
        input.generators.len(),
        input.branches.len(),
        opt.initialization,
        opt.tolerance,
        opt.max_iterations,
        opt.enforce_q_limits
    );
    let original_generators = input.generators.len();
    let mut working = input.clone();
    let rows = input.dcline.as_ref().map_or(0, Vec::len);
    #[allow(clippy::type_complexity)]
    let (detailed, adjust_taps, detailed_vsc, detailed_mtdc): (
        Vec<&LccLine>,
        bool,
        Vec<&VscLine>,
        Vec<&MtdcSystem>,
    ) = match &opt.dc_model {
        DcModel::Detailed {
            lines,
            vsc_lines,
            mtdc,
            adjust_taps,
        } if opt.model == Model::Ac => {
            let mut seen = vec![false; rows];
            let mut claim = |row: usize, name: &str| {
                if row >= rows || std::mem::replace(&mut seen[row], true) {
                    Err(Error::InvalidCase(format!(
                        "detailed DC line {name} names a missing or repeated dcline row {row}"
                    )))
                } else {
                    Ok(())
                }
            };
            for line in lines {
                line.validate().map_err(Error::InvalidCase)?;
                claim(line.row, &line.name)?;
            }
            for line in vsc_lines {
                line.validate().map_err(Error::InvalidCase)?;
                claim(line.row, &line.name)?;
            }
            for system in mtdc {
                system.topology().map_err(Error::InvalidCase)?;
                for row in system.converters.iter().filter_map(|c| c.row) {
                    claim(row, &system.name)?;
                }
            }
            (
                lines.iter().collect(),
                *adjust_taps,
                vsc_lines.iter().collect(),
                mtdc.iter().collect(),
            )
        }
        _ => (Vec::new(), false, Vec::new(), Vec::new()),
    };
    // Detailed VSC rows keep their terminal generators, with active power
    // from the DC-side solution.
    let mut vsc_states = Vec::new();
    if let Some(table) = &mut working.dcline {
        for line in &detailed_vsc {
            let row = &mut table[line.row];
            if row[2] == 0.0 || !line.in_service {
                row[2] = 0.0;
                continue;
            }
            let state = line.dc_state(1.0).map_err(Error::InvalidCase)?;
            let (f, t) = (line.from, 1 - line.from);
            row[3] = -state.injection_mw[f];
            row[15] = row[3] - state.injection_mw[t];
            row[16] = 0.0;
            for (k, c) in [(11, f), (13, t)] {
                let conv = &line.converters[c];
                if conv.ac_control == VscAcControl::PowerFactor {
                    let q = conv.power_factor_q(state.injection_mw[c]);
                    (row[k], row[k + 1]) = (q, q);
                } else {
                    (row[k], row[k + 1]) = (conv.qmin, conv.qmax);
                }
            }
            vsc_states.push((*line, state));
        }
    }
    let mut skip = vec![false; rows];
    for line in &detailed {
        skip[line.row] = true;
    }
    for system in &detailed_mtdc {
        for row in system.converters.iter().filter_map(|c| c.row) {
            skip[row] = true;
        }
    }
    let mappings = add_dc_line_generators_except(&mut working, &skip).inspect_err(|error| {
        log::error!("power-flow setup failed while converting DC lines: {error}");
    })?;
    // Rows switched off in the case stay off in the detailed model too.
    let active: Vec<_> = detailed
        .iter()
        .copied()
        .filter(|line| {
            input
                .dcline
                .as_ref()
                .is_some_and(|rows| rows[line.row][2] != 0.0)
        })
        .collect();
    // A system with any of its rows switched off is blocked.
    let row_on = |row: usize| {
        input
            .dcline
            .as_ref()
            .is_some_and(|rows| rows[row][2] != 0.0)
    };
    let active_mtdc: Vec<_> = detailed_mtdc
        .iter()
        .copied()
        .filter(|system| {
            system.control != dc::LccControl::Blocked
                && system.converters.iter().filter_map(|c| c.row).all(row_on)
        })
        .collect();
    let mut vsc_terminals: Vec<_> = vsc_states
        .into_iter()
        .filter_map(|(line, state)| {
            let m = mappings.iter().find(|m| m.row == line.row)?;
            Some(VscTerminal {
                line,
                gens: [m.from_generator, m.to_generator],
                scale: 1.0,
                state,
                limited: [false; 2],
            })
        })
        .collect();
    let setup = if opt.model == Model::Ac {
        controls::setup(&mut working, &opt.voltage_controls)?
    } else {
        Setup::none(&working)
    };
    let mut result = solve_core(
        &working,
        opt,
        &active,
        &active_mtdc,
        adjust_taps,
        &mut vsc_terminals,
        &setup,
    )
    .inspect_err(|error| {
        log::error!("{:?} power-flow solve failed: {error}", opt.model);
    })?;
    controls::finish(&setup, &mut result.case, &mut result.switched_shunts);
    let solved_vsc = std::mem::take(&mut result.vsc);
    result.vsc = detailed_vsc
        .iter()
        .map(|line| {
            solved_vsc
                .iter()
                .find(|s| s.row == line.row)
                .cloned()
                .unwrap_or_else(|| idle_vsc(line))
        })
        .collect();
    let vms: std::collections::HashMap<_, _> =
        result.case.buses.iter().map(|b| (b.id, b.vm)).collect();
    let vm = |bus: u32| vms.get(&bus).copied().unwrap_or(1.0);
    if let Some(lines) = &mut result.case.dcline {
        let mut solutions = result.lcc.iter();
        let mut all = Vec::with_capacity(detailed.len());
        for line in &detailed {
            let row = &mut lines[line.row];
            let solution = if row[2] != 0.0 {
                solutions.next().cloned()
            } else {
                None
            };
            let solution = solution.unwrap_or_else(|| blocked(line));
            row[3] = solution.rectifier_mw;
            row[4] = solution.inverter_mw;
            row[5] = -solution.rectifier_mvar;
            row[6] = -solution.inverter_mvar;
            row[7] = vm(line.rectifier.bus);
            row[8] = vm(line.inverter.bus);
            all.push(solution);
        }
        result.lcc = all;
    }
    let mut solved_mtdc = std::mem::take(&mut result.mtdc).into_iter();
    result.mtdc = detailed_mtdc
        .iter()
        .map(|&system| {
            if active_mtdc.iter().any(|&a| std::ptr::eq(a, system)) {
                solved_mtdc.next()
            } else {
                None
            }
            .map_or_else(|| idle_mtdc(system), Ok)
        })
        .collect::<Result<_, _>>()?;
    if let Some(lines) = &mut result.case.dcline {
        for (system, solution) in detailed_mtdc.iter().zip(&result.mtdc) {
            write_mtdc_rows(system, solution, lines, &vm);
        }
    }
    if let Some(lines) = &mut result.case.dcline {
        for mapping in mappings {
            let from = &result.case.generators[mapping.from_generator];
            let to = &result.case.generators[mapping.to_generator];
            let line = &mut lines[mapping.row];
            line[3] = -from.pg;
            line[4] = to.pg;
            line[5] = from.qg;
            line[6] = to.qg;
            line[7] = from.vg;
            line[8] = to.vg;
        }
    }
    result.case.generators.truncate(original_generators);
    log::info!(
        "finished {:?} power-flow solve: status={:?}, iterations={}, residual={:.6e}, q_limit_passes={}, elapsed={} ms",
        opt.model,
        result.status,
        result.iterations,
        result.residual,
        result.q_limit_passes,
        started.elapsed().as_millis()
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytical_jacobian_matches_central_differences() {
        let mut case = crate::parse_matpower(include_str!("../tests/fixtures/case9.m")).unwrap();
        case.branches[1].tap = 1.07;
        case.branches[1].shift = 13.0;
        case.buses[4].gs = 3.0;
        case.buses[4].bs = 2.0;
        let net = Network::new(&case).unwrap();
        let v: Vec<_> = (0..net.external.len())
            .map(|i| C::from_polar(0.95 + 0.01 * i as f64, 0.025 * i as f64))
            .collect();
        let layout = Layout::plain(&net.kinds);
        let (angles, pq) = (&layout.angles, &layout.magnitudes);
        let n = angles.len() + pq.len();
        let mut exact = vec![vec![0.0; n]; n];
        for t in jacobian(&net, &v, angles, pq) {
            exact[t.row][t.col] += t.val;
        }
        let s = net.injections(&case);
        let h = 1e-6;
        for col in 0..n {
            let mut plus = v.clone();
            let mut minus = v.clone();
            if col < angles.len() {
                let i = angles[col];
                plus[i] *= C::from_polar(1.0, h);
                minus[i] *= C::from_polar(1.0, -h);
            } else {
                let i = pq[col - angles.len()];
                plus[i] = C::from_polar(v[i].norm() + h, v[i].arg());
                minus[i] = C::from_polar(v[i].norm() - h, v[i].arg());
            }
            let (fp, _) = mismatch(&net, &plus, &s, &layout).unwrap();
            let (fm, _) = mismatch(&net, &minus, &s, &layout).unwrap();
            for row in 0..n {
                assert!(
                    (exact[row][col] - (fp[row] - fm[row]) / (2.0 * h)).abs() < 1e-7,
                    "Jacobian ({row},{col})"
                );
            }
        }
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn remote_regulation_jacobian_matches_central_differences() {
        let mut case = crate::parse_matpower(include_str!("../tests/fixtures/case9.m")).unwrap();
        // Buses 2 and 3 share holding bus 7, two to one.
        case.generators[1].regulated_bus = Some(7);
        case.generators[2].regulated_bus = Some(7);
        case.generators[2].reactive_share = 50.0;
        let net = Network::new(&case).unwrap();
        let layout = Layout::new(&net, &case, &net.kinds);
        assert_eq!(layout.targets.len(), 1);
        assert_eq!(layout.remote, [1, 2]);
        assert_eq!(layout.shares.len(), 1);
        let v: Vec<_> = (0..net.external.len())
            .map(|i| C::from_polar(0.95 + 0.01 * i as f64, 0.025 * i as f64))
            .collect();
        let n = layout.len();
        assert_eq!(
            n,
            layout.angles.len() + layout.q_rows.len() + layout.shares.len()
        );
        let mut exact = vec![vec![0.0; n]; n];
        for t in layout_jacobian(&net, &v, &layout) {
            exact[t.row][t.col] += t.val;
        }
        let s = net.injections(&case);
        let h = 1e-6;
        for col in 0..n {
            let mut plus = v.clone();
            let mut minus = v.clone();
            if col < layout.angles.len() {
                let i = layout.angles[col];
                plus[i] *= C::from_polar(1.0, h);
                minus[i] *= C::from_polar(1.0, -h);
            } else {
                let i = layout.magnitudes[col - layout.angles.len()];
                plus[i] = C::from_polar(v[i].norm() + h, v[i].arg());
                minus[i] = C::from_polar(v[i].norm() - h, v[i].arg());
            }
            let (fp, _) = mismatch(&net, &plus, &s, &layout).unwrap();
            let (fm, _) = mismatch(&net, &minus, &s, &layout).unwrap();
            for row in 0..n {
                assert!(
                    (exact[row][col] - (fp[row] - fm[row]) / (2.0 * h)).abs() < 1e-7,
                    "Jacobian ({row},{col})"
                );
            }
        }
    }

    #[test]
    fn singular_sparse_system_returns_error() {
        let entries = [
            Triplet::new(0, 0, 1.0),
            Triplet::new(0, 1, 1.0),
            Triplet::new(1, 0, 1.0),
            Triplet::new(1, 1, 1.0),
        ];
        assert!(linear(2, &entries, &[1.0, 2.0]).is_err());
    }
}
