//! Meshed VSC DC grids in the MatACDC data model.
//!
//! A set of DC grids is described by three tables in the column order of
//! MatACDC's case files: DC buses (`busdc`), converter stations (`convdc`)
//! and DC branches (`branchdc`), with the AC and DC power bases and the
//! number of poles (`pol`). Each grid is a resistive network; each converter
//! station connects one DC bus to one AC bus. The equations follow
//! J. Beerten, S. Cole and R. Belmans, "Generalized steady-state VSC MTDC
//! model for sequential AC/DC power flow algorithms", IEEE Trans. Power
//! Syst. 27(2), 2012, and the MatACDC 1.0 user manual.
//!
//! **Station.** From the AC bus (`Us`) the station is a transformer
//! `Ztf = Rtf + jXtf`, a filter bus (`Uf`) with a shunt susceptance `Bf`,
//! a phase reactor `Zc = Rc + jXc`, and the converter voltage `Uc`. Any of
//! the transformer, filter and reactor resistance can be zero. Given the
//! bus voltage and the power `Ss` the station injects into the AC bus, the
//! filter and converter voltages and the converter power `Sc` follow
//! directly. Losses depend on the converter current `Ic = |Sc| / |Uc|`:
//! `a + b·Ic + c·Ic²`, with `a` in MW, `b` in kV and `c` in ohms on the
//! converter's AC base voltage. As in MatACDC, the "rectifier" quadratic
//! coefficient applies while the converter delivers power to the AC side
//! (`Pc > 0`) and the "inverter" one while it draws power from it.
//!
//! **Controls.** On the DC side a converter holds its AC power `Ps`
//! (power control), holds its DC bus voltage (slack), or follows a droop
//! line `Pdc = Pdc,set + (Udc − Udc,set) / k`, with `k` in p.u. DC voltage
//! per MW and an optional symmetric deadband around `Udc,set` in which the
//! converter holds `Pdc,set`. Here `Pdc` is the power the converter takes
//! from the DC grid, so a droop converter takes more when the DC voltage is
//! high. On the AC side a converter holds a reactive power `Qs` or its AC
//! bus voltage. A voltage-controlling converter on a bus whose voltage a
//! generator already holds holds `Qs = 0` instead.
//!
//! **DC grid.** DC power at bus `i` is `p · Ui · Σj Gij · Uj` with `p` the
//! number of poles. Every grid needs a slack or droop converter. With more
//! than one slack converter in a grid, the one on the lowest-numbered DC bus
//! keeps the voltage and the others hold their power order, as MatACDC does
//! by default.
//!
//! **Solution.** The AC/DC solution is found sequentially, as in MatACDC:
//! converters load the AC buses as fixed powers (voltage-controlling ones
//! through a generator with unlimited reactive power), the AC network is
//! solved, then the stations and the DC grids, and the slack and droop
//! converters get new AC powers. This repeats until those powers settle.
//!
//! Not modelled: converter current and voltage limits (`ICMAX`, `VCMAX`,
//! `VCMIN`), DC voltage limits, DC branch inductance and capacitance, and
//! MatACDC's infinite AC buses.

use crate::{
    BusType, Case, Error, Generator, PowerFlowOptions, PowerFlowResult, SolveStatus,
    input::{Kind, Parser, lex, read_text},
    mtdc::dense_solve,
};
use num_complex::Complex64 as C;
use std::{collections::HashMap, path::Path};

/// What a converter holds on its DC side (MatACDC `type_dc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridDcControl {
    /// Holds its AC active power `Ps` (`type_dc = 1`).
    Power,
    /// Holds its DC bus voltage (`type_dc = 2`).
    Slack,
    /// Follows a DC voltage droop line (`type_dc = 3`).
    Droop,
}

/// What a converter holds on its AC side (MatACDC `type_ac`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridAcControl {
    /// Holds its reactive power `Qs` (`type_ac = 1`).
    ReactivePower,
    /// Holds its AC bus voltage magnitude (`type_ac = 2`).
    Voltage,
}

/// A DC bus (one `busdc` row).
#[derive(Clone, Debug, PartialEq)]
pub struct VscGridBus {
    /// DC bus number (`busdc_i`).
    pub number: u32,
    /// AC bus of the converter here, or 0 for none (`busac_i`).
    pub ac_bus: u32,
    /// DC grid number (`grid`).
    pub grid: u32,
    /// Power taken from the DC grid in MW (`Pdc`); an output, ignored on input.
    pub p_mw: f64,
    /// DC voltage in p.u. (`Vdc`): the setpoint at a slack converter and the
    /// starting value elsewhere.
    pub vdc: f64,
    /// Base DC voltage in kV (`basekVdc`).
    pub base_kv: f64,
    /// DC voltage limits in p.u. (`Vdcmax`, `Vdcmin`). Not enforced.
    pub vmax: f64,
    pub vmin: f64,
    /// Bus capacitance in p.u. (`Cdc`). Not used in power flow.
    pub capacitance: f64,
}

/// A converter station (one `convdc` row). Powers are positive into the AC
/// grid; impedances are in p.u. on the AC power base and `base_kv`.
#[derive(Clone, Debug, PartialEq)]
pub struct VscGridConverter {
    /// DC bus number (`busdc_i`).
    pub dc_bus: u32,
    pub dc_control: GridDcControl,
    pub ac_control: GridAcControl,
    /// AC active power order `Ps` in MW (`P_g`), for power control.
    pub p_mw: f64,
    /// AC reactive power order `Qs` in Mvar (`Q_g`), for reactive power control.
    pub q_mvar: f64,
    /// AC bus voltage target in p.u. (`Vtar`), for voltage control.
    pub v_target: f64,
    /// Transformer resistance and reactance (`rtf`, `xtf`).
    pub r_tf: f64,
    pub x_tf: f64,
    /// Filter susceptance (`bf`).
    pub b_f: f64,
    /// Phase reactor resistance and reactance (`rc`, `xc`).
    pub r_c: f64,
    pub x_c: f64,
    /// Converter AC base voltage in kV (`basekVac`).
    pub base_kv: f64,
    /// Converter voltage limits in p.u. (`Vmmax`, `Vmmin`). Not enforced.
    pub vc_max: f64,
    pub vc_min: f64,
    /// Converter current limit in p.u. (`Imax`). Not enforced.
    pub ic_max: f64,
    /// In service (`status`).
    pub in_service: bool,
    /// Constant loss in MW (`LossA`).
    pub loss_a: f64,
    /// Linear loss coefficient in kV (`LossB`), times the current in kA.
    pub loss_b: f64,
    /// Quadratic loss coefficients in ohms (`LossCrec`, `LossCinv`).
    pub loss_c_rectifier: f64,
    pub loss_c_inverter: f64,
    /// Droop `k` in p.u. DC voltage per MW (`droop`).
    pub droop: f64,
    /// Droop power setpoint in MW, positive into the AC grid (`Pdcset`).
    pub droop_p_mw: f64,
    /// Droop voltage setpoint in p.u. (`Vdcset`).
    pub droop_v: f64,
    /// Droop deadband half-width in p.u. (`dVdcset`).
    pub droop_deadband: f64,
}

/// A DC branch (one `branchdc` row).
#[derive(Clone, Debug, PartialEq)]
pub struct VscGridBranch {
    pub from: u32,
    pub to: u32,
    /// Resistance in p.u. on the DC power base and the bus base voltage (`r`).
    pub r: f64,
    /// Inductance and total capacitance (`l`, `c`). Not used in power flow.
    pub l: f64,
    pub c: f64,
    /// Ratings in MW (`rateA`, `rateB`, `rateC`). Not enforced.
    pub rate_a: f64,
    pub rate_b: f64,
    pub rate_c: f64,
    /// In service (`status`).
    pub in_service: bool,
}

/// One or more VSC DC grids attached to an AC case (a MatACDC DC case).
#[derive(Clone, Debug, PartialEq)]
pub struct VscGrids {
    /// Power base of the converter impedances in MVA (`baseMVAac`).
    pub base_mva_ac: f64,
    /// Power base of the DC branch resistances in MVA (`baseMVAdc`).
    pub base_mva_dc: f64,
    /// Number of poles (`pol`): 1 for a monopole, 2 for a bipole or a
    /// symmetrically grounded monopole.
    pub poles: f64,
    pub buses: Vec<VscGridBus>,
    pub converters: Vec<VscGridConverter>,
    pub branches: Vec<VscGridBranch>,
}

/// Solved state of one converter. Powers are in MW and Mvar, positive into
/// the AC grid; voltages in p.u. and degrees.
#[derive(Clone, Debug, PartialEq)]
pub struct VscGridConverterSolution {
    pub dc_bus: u32,
    pub ac_bus: u32,
    /// Controls in effect; a voltage-controlling converter on a bus that a
    /// generator holds reports reactive power control.
    pub dc_control: GridDcControl,
    pub ac_control: GridAcControl,
    pub in_service: bool,
    /// Power into the AC bus (`Ps`, `Qs`).
    pub ps_mw: f64,
    pub qs_mvar: f64,
    /// Power at the converter terminal (`Pc`, `Qc`).
    pub pc_mw: f64,
    pub qc_mvar: f64,
    /// Converter voltage.
    pub vc: f64,
    pub vc_deg: f64,
    /// Filter bus voltage.
    pub vf: f64,
    pub vf_deg: f64,
    /// Converter current in p.u. of the AC power base.
    pub ic: f64,
    /// Converter loss in MW.
    pub loss_mw: f64,
    /// Power taken from the DC grid in MW (`Pc + loss`).
    pub pdc_mw: f64,
}

/// Solved state of one DC bus.
#[derive(Clone, Debug, PartialEq)]
pub struct VscGridBusSolution {
    pub number: u32,
    pub grid: u32,
    /// DC voltage in p.u. and kV.
    pub vdc: f64,
    pub kv: f64,
    /// Power taken from the DC grid by the converter here, in MW.
    pub p_mw: f64,
}

/// Solved flow on one DC branch.
#[derive(Clone, Debug, PartialEq)]
pub struct VscGridBranchSolution {
    pub from: u32,
    pub to: u32,
    pub in_service: bool,
    /// Power entering the branch at each end in MW.
    pub from_mw: f64,
    pub to_mw: f64,
    pub loss_mw: f64,
}

/// Operating point of a set of VSC DC grids.
#[derive(Clone, Debug, PartialEq)]
pub struct VscGridSolution {
    /// In the order of [`VscGrids::converters`].
    pub converters: Vec<VscGridConverterSolution>,
    /// In the order of [`VscGrids::buses`].
    pub buses: Vec<VscGridBusSolution>,
    /// In the order of [`VscGrids::branches`].
    pub branches: Vec<VscGridBranchSolution>,
    /// AC/DC passes taken.
    pub passes: usize,
    /// Whether the converter powers settled.
    pub converged: bool,
}

impl VscGrids {
    /// Check the grids against the AC case they attach to: every bus and
    /// branch reference resolves, every grid has a slack or droop converter,
    /// and the converter data can be solved.
    pub fn check(&self, case: &Case) -> Result<(), Error> {
        prepare(self, case).map(|_| ())
    }

    /// Total converter loss and DC branch loss in MW.
    pub fn losses(solution: &VscGridSolution) -> (f64, f64) {
        (
            solution.converters.iter().map(|c| c.loss_mw).sum(),
            solution.branches.iter().map(|b| b.loss_mw).sum(),
        )
    }
}

/// Parse a MatACDC DC case: a function returning `baseMVAac`, `baseMVAdc`,
/// `pol`, `busdc`, `convdc` and `branchdc`, or assigning them as fields of
/// a struct. Only literal numbers and matrices are read; no MATLAB code
/// runs. `convdc` rows need the first 20 columns; the droop columns
/// default to zero, and result columns after the 24th are ignored.
pub fn parse_matacdc(input: &str) -> Result<VscGrids, Error> {
    let mut p = Parser {
        tokens: lex(input)?,
        pos: 0,
    };
    p.separators();
    p.function_header()?;
    let mut scalars: HashMap<String, f64> = HashMap::new();
    let mut matrices: HashMap<String, Vec<Vec<f64>>> = HashMap::new();
    loop {
        p.separators();
        if p.pos >= p.tokens.len() {
            break;
        }
        let t = p.take();
        let Kind::Word(mut name) = t.kind.clone() else {
            return Err(t.error("expected an assignment"));
        };
        if name == "return" || name == "end" {
            continue;
        }
        // `mpc.busdc = ...` or `busdc = ...`.
        if p.peek().kind == Kind::Symbol('.') {
            p.take();
            let field = p.take();
            let Kind::Word(field) = field.kind.clone() else {
                return Err(field.error("expected a field name"));
            };
            name = field;
        }
        p.expect(Kind::Symbol('='))?;
        match name.as_str() {
            "busdc" | "convdc" | "branchdc" => {
                if matrices.insert(name.clone(), p.matrix()?).is_some() {
                    return Err(t.error(format!("duplicate {name}")));
                }
            }
            "baseMVAac" | "baseMVAdc" | "pol" => {
                let v = p.take();
                let Kind::Number(n) = v.kind else {
                    return Err(v.error(format!("expected a number for {name}")));
                };
                scalars.insert(name, n);
            }
            "version" => {
                p.take();
            }
            _ => return Err(t.error(format!("unsupported field {name}"))),
        }
        if !matches!(p.peek().kind, Kind::Newline | Kind::Symbol(';')) {
            return Err(p
                .peek()
                .error("expected assignment terminator; expressions are unsupported"));
        }
    }
    let scalar = |name: &str| {
        scalars
            .get(name)
            .copied()
            .ok_or_else(|| Error::InvalidCase(format!("MatACDC case is missing {name}")))
    };
    let mut table = |name: &str, min: usize| {
        let rows = matrices
            .remove(name)
            .ok_or_else(|| Error::InvalidCase(format!("MatACDC case is missing {name}")))?;
        if rows.iter().any(|r| r.len() < min) {
            return Err(Error::InvalidCase(format!(
                "{name} needs at least {min} columns"
            )));
        }
        Ok(rows)
    };
    let id = |n: f64, what: &str| -> Result<u32, Error> {
        if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= u32::MAX as f64 {
            Ok(n as u32)
        } else {
            Err(Error::InvalidCase(format!("invalid {what} {n}")))
        }
    };
    let buses = table("busdc", 9)?
        .into_iter()
        .map(|r| {
            Ok(VscGridBus {
                number: id(r[0], "DC bus number")?,
                ac_bus: id(r[1], "AC bus number")?,
                grid: id(r[2], "DC grid number")?,
                p_mw: r[3],
                vdc: r[4],
                base_kv: r[5],
                vmax: r[6],
                vmin: r[7],
                capacitance: r[8],
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let converters = table("convdc", 20)?
        .into_iter()
        .map(|r| {
            let col = |k: usize| r.get(k).copied().unwrap_or(0.0);
            Ok(VscGridConverter {
                dc_bus: id(r[0], "converter DC bus")?,
                dc_control: match r[1] {
                    1.0 => GridDcControl::Power,
                    2.0 => GridDcControl::Slack,
                    3.0 => GridDcControl::Droop,
                    x => return Err(Error::InvalidCase(format!("invalid converter type_dc {x}"))),
                },
                ac_control: match r[2] {
                    1.0 => GridAcControl::ReactivePower,
                    2.0 => GridAcControl::Voltage,
                    x => return Err(Error::InvalidCase(format!("invalid converter type_ac {x}"))),
                },
                p_mw: r[3],
                q_mvar: r[4],
                v_target: r[5],
                r_tf: r[6],
                x_tf: r[7],
                b_f: r[8],
                r_c: r[9],
                x_c: r[10],
                base_kv: r[11],
                vc_max: r[12],
                vc_min: r[13],
                ic_max: r[14],
                in_service: r[15] > 0.0,
                loss_a: r[16],
                loss_b: r[17],
                loss_c_rectifier: r[18],
                loss_c_inverter: r[19],
                droop: col(20),
                droop_p_mw: col(21),
                droop_v: col(22),
                droop_deadband: col(23),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let branches = table("branchdc", 9)?
        .into_iter()
        .map(|r| {
            Ok(VscGridBranch {
                from: id(r[0], "DC branch from bus")?,
                to: id(r[1], "DC branch to bus")?,
                r: r[2],
                l: r[3],
                c: r[4],
                rate_a: r[5],
                rate_b: r[6],
                rate_c: r[7],
                in_service: r[8] > 0.0,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let grids = VscGrids {
        base_mva_ac: scalar("baseMVAac")?,
        base_mva_dc: scalar("baseMVAdc")?,
        poles: scalar("pol")?,
        buses,
        converters,
        branches,
    };
    log::info!(
        "parsed MatACDC case: {} DC buses, {} converters, {} DC branches, {} poles",
        grids.buses.len(),
        grids.converters.len(),
        grids.branches.len(),
        grids.poles
    );
    Ok(grids)
}

/// Read and parse a MatACDC DC case file; see [`parse_matacdc`].
pub fn load_matacdc(path: impl AsRef<Path>) -> Result<VscGrids, Error> {
    let path = path.as_ref();
    log::info!("loading MatACDC DC case from {}", path.display());
    parse_matacdc(&read_text(path)?)
}

/// Unlimited reactive range of the generator that holds a converter's AC
/// bus voltage, as in MatACDC.
const DUMMY_Q_LIMIT: f64 = 99_999.0;
/// AC/DC passes before giving up.
const MAX_PASSES: usize = 30;
/// Converter powers must settle to this many p.u., or the solve tolerance
/// if that is tighter.
const PASS_TOLERANCE: f64 = 1e-6;

/// How a voltage-controlling converter holds its AC bus.
#[derive(Clone, Debug)]
enum Hold {
    /// Fixed `Qs`.
    None,
    /// Through an added generator, by index in the working case.
    Added(usize),
    /// Through the PQ bus's own generators (made PV), whose reactive power
    /// above their case values is the converter's.
    Existing(Vec<usize>, f64),
}

/// One in-service converter, in p.u. on the AC case's power base.
#[derive(Clone, Debug)]
struct Station {
    /// Index in [`VscGrids::converters`].
    conv: usize,
    /// DC bus index.
    dc: usize,
    /// AC bus index in the case.
    ac: usize,
    dc_control: GridDcControl,
    ac_control: GridAcControl,
    ztf: C,
    bf: f64,
    zc: C,
    loss: [f64; 4],
    droop_k: f64,
    droop_p: f64,
    droop_v: f64,
    droop_band: f64,
    qs: f64,
    v_target: f64,
}

/// Station voltages and powers for a given AC bus voltage and injection.
#[derive(Clone, Copy, Debug)]
struct StationState {
    vf: C,
    vc: C,
    sc: C,
    ic: f64,
    loss: f64,
}

impl Station {
    fn state(&self, vs: C, ss: C) -> StationState {
        let itf = (ss / vs).conj();
        let vf = vs + itf * self.ztf;
        let ssf = vf * itf.conj();
        // The filter absorbs -Bf·|Uf|² of reactive power.
        let scf = ssf - C::new(0.0, self.bf * vf.norm_sqr());
        let i_c = (scf / vf).conj();
        let vc = vf + i_c * self.zc;
        let sc = vc * i_c.conj();
        let ic = sc.norm() / vc.norm();
        let [a, b, c_rec, c_inv] = self.loss;
        let c = if sc.re > 0.0 {
            c_rec
        } else if sc.re < 0.0 {
            c_inv
        } else {
            0.0
        };
        StationState {
            vf,
            vc,
            sc,
            ic,
            loss: a + b * ic + c * ic * ic,
        }
    }

    /// Power the converter takes from the DC grid.
    fn pdc(&self, vs: C, ss: C) -> f64 {
        let s = self.state(vs, ss);
        s.sc.re + s.loss
    }

    /// The AC active power that takes `pdc` from the DC grid, holding the
    /// bus voltage and `Qs`. Starts from `ps`.
    fn ps_for(&self, vs: C, qs: f64, pdc: f64, mut ps: f64) -> Result<f64, Error> {
        for _ in 0..50 {
            let f = self.pdc(vs, C::new(ps, qs)) - pdc;
            if f.abs() < 1e-13 {
                return Ok(ps);
            }
            let h = 1e-7 * (1.0 + ps.abs());
            let df =
                (self.pdc(vs, C::new(ps + h, qs)) - self.pdc(vs, C::new(ps - h, qs))) / (2.0 * h);
            if !df.is_finite() || df.abs() < 1e-12 {
                break;
            }
            ps -= f / df;
        }
        Err(Error::Numerical(
            "a slack or droop converter's AC power did not settle".into(),
        ))
    }
}

/// The grids checked against an AC case and converted to p.u.
struct Prepared {
    stations: Vec<Station>,
    /// Station index on each DC bus.
    on_bus: Vec<Option<usize>>,
    /// DC bus indices of each grid.
    grids: Vec<Vec<usize>>,
    /// Branch ends and conductance in p.u. on the case base, in service only.
    branches: Vec<(usize, usize, f64)>,
    poles: f64,
}

/// True for a positive number, false for zero, negatives and NaN.
fn positive(x: f64) -> bool {
    x > 0.0
}

fn prepare(grids: &VscGrids, case: &Case) -> Result<Prepared, Error> {
    let invalid = |m: String| Err(Error::InvalidCase(m));
    let s = case.base_mva;
    if !(positive(grids.base_mva_ac) && positive(grids.base_mva_dc)) {
        return invalid("MatACDC power bases must be positive".into());
    }
    if !positive(grids.poles) {
        return invalid("the number of DC poles must be positive".into());
    }
    let mut dc_index = HashMap::new();
    for (k, b) in grids.buses.iter().enumerate() {
        if dc_index.insert(b.number, k).is_some() {
            return invalid(format!("DC bus {} is listed twice", b.number));
        }
    }
    let ac_index: HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(k, b)| (b.id, k))
        .collect();
    let mut on_bus = vec![None; grids.buses.len()];
    let mut ac_used = HashMap::new();
    let mut stations = Vec::new();
    for (k, c) in grids.converters.iter().enumerate() {
        let Some(&dc) = dc_index.get(&c.dc_bus) else {
            return invalid(format!("converter {} is on an unknown DC bus", c.dc_bus));
        };
        if !c.in_service {
            continue;
        }
        if on_bus[dc].is_some() {
            return invalid(format!("DC bus {} has two converters", c.dc_bus));
        }
        let ac_bus = grids.buses[dc].ac_bus;
        let Some(&ac) = ac_index.get(&ac_bus) else {
            return invalid(format!(
                "the converter on DC bus {} connects to AC bus {ac_bus}, which is not in the case",
                c.dc_bus
            ));
        };
        if let Some(other) = ac_used.insert(ac_bus, c.dc_bus) {
            return invalid(format!(
                "AC bus {ac_bus} has converters from DC buses {other} and {}",
                c.dc_bus
            ));
        }
        if !positive(c.base_kv) {
            return invalid(format!(
                "the converter on DC bus {} needs a base voltage",
                c.dc_bus
            ));
        }
        let z = s / grids.base_mva_ac;
        let ztf = C::new(c.r_tf, c.x_tf) * z;
        let zc = C::new(c.r_c, c.x_c) * z;
        if zc.norm() == 0.0 {
            return invalid(format!(
                "the converter on DC bus {} needs a phase reactor impedance",
                c.dc_bus
            ));
        }
        let base_ka = s / (3f64.sqrt() * c.base_kv);
        if c.dc_control == GridDcControl::Droop && !positive(c.droop) {
            return invalid(format!(
                "the droop converter on DC bus {} needs a positive droop",
                c.dc_bus
            ));
        }
        let mut ac_control = c.ac_control;
        let mut qs = c.q_mvar / s;
        if ac_control == GridAcControl::Voltage {
            qs = 0.0;
            if matches!(case.buses[ac].kind, BusType::Pv | BusType::Reference) {
                log::warn!(
                    "AC bus {ac_bus} already has its voltage held by a generator; the converter on DC bus {} holds Qs = 0 instead",
                    c.dc_bus
                );
                ac_control = GridAcControl::ReactivePower;
            }
        }
        on_bus[dc] = Some(stations.len());
        stations.push(Station {
            conv: k,
            dc,
            ac,
            dc_control: c.dc_control,
            ac_control,
            ztf,
            bf: c.b_f / z,
            zc,
            loss: [
                c.loss_a / s,
                c.loss_b * base_ka / s,
                c.loss_c_rectifier * base_ka * base_ka / s,
                c.loss_c_inverter * base_ka * base_ka / s,
            ],
            droop_k: c.droop * s,
            droop_p: c.droop_p_mw / s,
            droop_v: c.droop_v,
            droop_band: c.droop_deadband.max(0.0),
            qs,
            v_target: c.v_target,
        });
    }
    let mut branches = Vec::new();
    for b in &grids.branches {
        let (Some(&f), Some(&t)) = (dc_index.get(&b.from), dc_index.get(&b.to)) else {
            return invalid(format!(
                "DC branch {}-{} names an unknown DC bus",
                b.from, b.to
            ));
        };
        if grids.buses[f].grid != grids.buses[t].grid {
            return invalid(format!("DC branch {}-{} joins two DC grids", b.from, b.to));
        }
        if !b.in_service {
            continue;
        }
        let r = b.r * s / grids.base_mva_dc;
        if !positive(r) {
            return invalid(format!(
                "DC branch {}-{} needs a positive resistance",
                b.from, b.to
            ));
        }
        branches.push((f, t, 1.0 / r));
    }
    let mut numbers: Vec<u32> = grids.buses.iter().map(|b| b.grid).collect();
    numbers.sort_unstable();
    numbers.dedup();
    let mut members = Vec::new();
    for &g in &numbers {
        let buses: Vec<usize> = (0..grids.buses.len())
            .filter(|&k| grids.buses[k].grid == g)
            .collect();
        let mut slack: Vec<usize> = buses
            .iter()
            .filter_map(|&k| on_bus[k])
            .filter(|&st| stations[st].dc_control == GridDcControl::Slack)
            .collect();
        slack.sort_by_key(|&st| grids.buses[stations[st].dc].number);
        for &st in slack.iter().skip(1) {
            log::warn!(
                "DC grid {g} has more than one slack converter; the one on DC bus {} becomes power controlled",
                grids.buses[stations[st].dc].number
            );
            stations[st].dc_control = GridDcControl::Power;
        }
        let controlled = buses.iter().filter_map(|&k| on_bus[k]).any(|st| {
            matches!(
                stations[st].dc_control,
                GridDcControl::Slack | GridDcControl::Droop
            )
        });
        if !controlled {
            return invalid(format!(
                "DC grid {g} has no in-service slack or droop converter"
            ));
        }
        members.push(buses);
    }
    Ok(Prepared {
        stations,
        on_bus,
        grids: members,
        branches,
        poles: grids.poles,
    })
}

impl Prepared {
    /// Solve each DC grid for its bus voltages, given the power that each
    /// power-controlled converter takes. Returns the power that every
    /// converter takes, by station.
    fn solve_dc(&self, vdc: &mut [f64], pdc: &[f64], tolerance: f64) -> Result<Vec<f64>, Error> {
        let n = vdc.len();
        let mut g = vec![0.0; n * n];
        for &(f, t, y) in &self.branches {
            g[f * n + f] += y;
            g[t * n + t] += y;
            g[f * n + t] -= y;
            g[t * n + f] -= y;
        }
        let p = self.poles;
        let injection =
            |v: &[f64], i: usize| p * v[i] * (0..n).map(|j| g[i * n + j] * v[j]).sum::<f64>();
        for buses in &self.grids {
            // Unknowns: every bus but the slack's.
            let unknown: Vec<usize> = buses
                .iter()
                .copied()
                .filter(|&k| {
                    self.on_bus[k]
                        .is_none_or(|st| self.stations[st].dc_control != GridDcControl::Slack)
                })
                .collect();
            let m = unknown.len();
            let mut converged = false;
            for _ in 0..50 {
                let mut mismatch = vec![0.0; m];
                let mut jac = vec![0.0; m * m];
                for (r, &i) in unknown.iter().enumerate() {
                    // Injection into the DC grid at bus i, minus what the
                    // converter there delivers.
                    let mut f = injection(vdc, i);
                    let mut diagonal = 0.0;
                    if let Some(st) = self.on_bus[i] {
                        let s = &self.stations[st];
                        match s.dc_control {
                            GridDcControl::Power => f += pdc[st],
                            GridDcControl::Droop => {
                                let dv = vdc[i] - s.droop_v;
                                let held = if dv.abs() <= s.droop_band {
                                    vdc[i]
                                } else {
                                    s.droop_v + s.droop_band.copysign(dv)
                                };
                                f += s.droop_p + (vdc[i] - held) / s.droop_k;
                                // The slope outside the deadband, also inside
                                // it (as MatACDC does), so a grid held only
                                // by droop converters stays solvable.
                                diagonal = 1.0 / s.droop_k;
                            }
                            GridDcControl::Slack => unreachable!(),
                        }
                    }
                    mismatch[r] = f;
                    for (c, &j) in unknown.iter().enumerate() {
                        jac[r * m + c] = p * v_derivative(&g, n, vdc, i, j);
                    }
                    jac[r * m + r] += diagonal;
                }
                if mismatch.iter().all(|f| f.abs() < tolerance) {
                    converged = true;
                    break;
                }
                let dx = dense_solve(&mut jac, mismatch, m).ok_or_else(|| {
                    Error::Numerical(
                        "the DC grid is singular (a DC bus may have no path to a slack or droop converter)"
                            .into(),
                    )
                })?;
                for (r, &i) in unknown.iter().enumerate() {
                    vdc[i] -= dx[r];
                }
            }
            if !converged {
                return Err(Error::Numerical(
                    "the DC grid solve did not converge".into(),
                ));
            }
        }
        Ok(self
            .stations
            .iter()
            .enumerate()
            .map(|(st, s)| match s.dc_control {
                GridDcControl::Power => pdc[st],
                _ => -injection(vdc, s.dc),
            })
            .collect())
    }
}

/// `∂(Ui · Σk Gik Uk) / ∂Uj`.
fn v_derivative(g: &[f64], n: usize, v: &[f64], i: usize, j: usize) -> f64 {
    if i == j {
        (0..n).map(|k| g[i * n + k] * v[k]).sum::<f64>() + v[i] * g[i * n + i]
    } else {
        v[i] * g[i * n + j]
    }
}

/// Solve an AC case with VSC DC grids. `inner` solves the AC case (and any
/// other DC models) with the converters as fixed injections.
pub(crate) fn solve(
    input: &Case,
    opt: &PowerFlowOptions,
    grids: &VscGrids,
    inner: impl Fn(&Case, &PowerFlowOptions) -> Result<PowerFlowResult, Error>,
) -> Result<PowerFlowResult, Error> {
    let prepared = prepare(grids, input)?;
    let stations = &prepared.stations;
    let s = input.base_mva;
    let tolerance = opt.tolerance.min(PASS_TOLERANCE);
    log::info!(
        "VSC DC grids: {} grids, {} converters in service, {} DC branches",
        prepared.grids.len(),
        stations.len(),
        prepared.branches.len()
    );
    // Starting AC powers: power orders, droop setpoints, and the slack
    // converters taking up the rest of their grid without losses.
    let mut ps: Vec<f64> = stations
        .iter()
        .map(|st| match st.dc_control {
            GridDcControl::Power => grids.converters[st.conv].p_mw / s,
            GridDcControl::Droop => st.droop_p,
            GridDcControl::Slack => 0.0,
        })
        .collect();
    for buses in &prepared.grids {
        let in_grid: Vec<usize> = buses.iter().filter_map(|&k| prepared.on_bus[k]).collect();
        let rest: f64 = in_grid
            .iter()
            .filter(|&&st| stations[st].dc_control != GridDcControl::Slack)
            .map(|&st| ps[st])
            .sum();
        for &st in &in_grid {
            if stations[st].dc_control == GridDcControl::Slack {
                ps[st] = -rest;
            }
        }
    }
    let mut vdc: Vec<f64> = grids.buses.iter().map(|b| b.vdc).collect();
    // The working case: converters as loads, and voltage holders.
    let mut base = input.clone();
    let mut holds = Vec::with_capacity(stations.len());
    for st in stations {
        let bus = &mut base.buses[st.ac];
        if st.ac_control != GridAcControl::Voltage {
            holds.push(Hold::None);
            continue;
        }
        bus.kind = BusType::Pv;
        let id = bus.id;
        let existing: Vec<usize> = (0..input.generators.len())
            .filter(|&g| input.generators[g].bus == id && input.generators[g].online)
            .collect();
        if existing.is_empty() {
            holds.push(Hold::Added(base.generators.len()));
            base.generators.push(Generator {
                bus: id,
                pg: 0.0,
                qg: 0.0,
                qmax: DUMMY_Q_LIMIT,
                qmin: -DUMMY_Q_LIMIT,
                vg: st.v_target,
                mbase: grids.base_mva_ac,
                online: true,
                pmax: DUMMY_Q_LIMIT,
                pmin: 0.0,
                extra: Vec::new(),
                regulated_bus: None,
                reactive_share: 100.0,
            });
        } else {
            let q0 = existing
                .iter()
                .map(|&g| input.generators[g].qg)
                .sum::<f64>()
                / s;
            holds.push(Hold::Existing(existing, q0));
        }
    }
    let mut pass_opt = opt.clone();
    let mut last: Option<PowerFlowResult> = None;
    let (mut iterations, mut passes, mut settled) = (0, 0, false);
    let mut qs: Vec<f64> = stations.iter().map(|st| st.qs).collect();
    let mut vs = vec![C::new(1.0, 0.0); stations.len()];
    while passes < MAX_PASSES {
        passes += 1;
        let mut working = base.clone();
        if let Some(previous) = &last {
            for (b, p) in working.buses.iter_mut().zip(&previous.case.buses) {
                (b.vm, b.va) = (p.vm, p.va);
            }
            for (g, p) in working.generators.iter_mut().zip(&previous.case.generators) {
                (g.pg, g.qg) = (p.pg, p.qg);
            }
            // The case changed, so every limit is looked at afresh.
            pass_opt.initialization = crate::Initialization::Case;
        }
        for (k, st) in stations.iter().enumerate() {
            let bus = &mut working.buses[st.ac];
            bus.pd -= ps[k] * s;
            if st.ac_control != GridAcControl::Voltage {
                bus.qd -= st.qs * s;
            }
        }
        let result = inner(&working, &pass_opt)?;
        iterations += result.iterations;
        for (k, st) in stations.iter().enumerate() {
            let b = &result.case.buses[st.ac];
            vs[k] = C::from_polar(b.vm, b.va.to_radians());
            qs[k] = match &holds[k] {
                Hold::None => st.qs,
                Hold::Added(g) => result.case.generators[*g].qg / s,
                Hold::Existing(gens, q0) => {
                    gens.iter()
                        .map(|&g| result.case.generators[g].qg)
                        .sum::<f64>()
                        / s
                        - q0
                }
            };
        }
        let converged_ac = result.status == SolveStatus::Converged;
        last = Some(result);
        if !converged_ac {
            log::warn!("VSC DC grids: the AC solve in pass {passes} did not converge");
            break;
        }
        let pdc: Vec<f64> = stations
            .iter()
            .enumerate()
            .map(|(k, st)| st.pdc(vs[k], C::new(ps[k], qs[k])))
            .collect();
        let taken = prepared.solve_dc(&mut vdc, &pdc, 1e-12)?;
        let mut change = 0.0f64;
        for (k, st) in stations.iter().enumerate() {
            if st.dc_control != GridDcControl::Power {
                let next = st.ps_for(vs[k], qs[k], taken[k], ps[k])?;
                change = change.max((next - ps[k]).abs());
                ps[k] = next;
            }
        }
        log::info!(
            "VSC DC grid pass {passes}: largest slack or droop converter power change {:.3e} MW",
            change * s
        );
        if change < tolerance {
            settled = true;
            break;
        }
    }
    let mut result = last.expect("at least one pass runs");
    if !settled && result.status == SolveStatus::Converged {
        log::warn!("VSC DC grid converter powers still moving after {MAX_PASSES} passes");
        result.status = SolveStatus::IterationLimit;
    }
    result.iterations = iterations;
    // Report the case as given, with the converters' results separate.
    for (b, original) in result.case.buses.iter_mut().zip(&input.buses) {
        (b.kind, b.pd, b.qd) = (original.kind, original.pd, original.qd);
    }
    result.case.generators.truncate(input.generators.len());
    for hold in &holds {
        if let Hold::Existing(gens, _) = hold {
            for &g in gens {
                result.case.generators[g].qg = input.generators[g].qg;
            }
        }
    }
    result.vsc_grids = Some(report(
        s, grids, &prepared, &vs, &ps, &qs, &vdc, passes, settled,
    ));
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn report(
    s: f64,
    grids: &VscGrids,
    prepared: &Prepared,
    vs: &[C],
    ps: &[f64],
    qs: &[f64],
    vdc: &[f64],
    passes: usize,
    converged: bool,
) -> VscGridSolution {
    let mut converters: Vec<VscGridConverterSolution> = grids
        .converters
        .iter()
        .map(|c| VscGridConverterSolution {
            dc_bus: c.dc_bus,
            ac_bus: grids
                .buses
                .iter()
                .find(|b| b.number == c.dc_bus)
                .map_or(0, |b| b.ac_bus),
            dc_control: c.dc_control,
            ac_control: c.ac_control,
            in_service: false,
            ps_mw: 0.0,
            qs_mvar: 0.0,
            pc_mw: 0.0,
            qc_mvar: 0.0,
            vc: 0.0,
            vc_deg: 0.0,
            vf: 0.0,
            vf_deg: 0.0,
            ic: 0.0,
            loss_mw: 0.0,
            pdc_mw: 0.0,
        })
        .collect();
    let mut bus_p = vec![0.0; grids.buses.len()];
    for (k, st) in prepared.stations.iter().enumerate() {
        let state = st.state(vs[k], C::new(ps[k], qs[k]));
        let pdc = state.sc.re + state.loss;
        bus_p[st.dc] = pdc * s;
        converters[st.conv] = VscGridConverterSolution {
            dc_control: st.dc_control,
            ac_control: st.ac_control,
            in_service: true,
            ps_mw: ps[k] * s,
            qs_mvar: qs[k] * s,
            pc_mw: state.sc.re * s,
            qc_mvar: state.sc.im * s,
            vc: state.vc.norm(),
            vc_deg: state.vc.arg().to_degrees(),
            vf: state.vf.norm(),
            vf_deg: state.vf.arg().to_degrees(),
            ic: state.ic,
            loss_mw: state.loss * s,
            pdc_mw: pdc * s,
            ..converters[st.conv].clone()
        };
    }
    let buses = grids
        .buses
        .iter()
        .enumerate()
        .map(|(k, b)| VscGridBusSolution {
            number: b.number,
            grid: b.grid,
            vdc: vdc[k],
            kv: vdc[k] * b.base_kv,
            p_mw: bus_p[k],
        })
        .collect();
    let index: HashMap<u32, usize> = grids
        .buses
        .iter()
        .enumerate()
        .map(|(k, b)| (b.number, k))
        .collect();
    let branches = grids
        .branches
        .iter()
        .map(|b| {
            let (f, t) = (index[&b.from], index[&b.to]);
            if !b.in_service {
                return VscGridBranchSolution {
                    from: b.from,
                    to: b.to,
                    in_service: false,
                    from_mw: 0.0,
                    to_mw: 0.0,
                    loss_mw: 0.0,
                };
            }
            let y = grids.base_mva_dc / (b.r * s);
            let i = (vdc[f] - vdc[t]) * y;
            let from_mw = prepared.poles * vdc[f] * i * s;
            let to_mw = -prepared.poles * vdc[t] * i * s;
            VscGridBranchSolution {
                from: b.from,
                to: b.to,
                in_service: true,
                from_mw,
                to_mw,
                loss_mw: from_mw + to_mw,
            }
        })
        .collect();
    VscGridSolution {
        converters,
        buses,
        branches,
        passes,
        converged,
    }
}
