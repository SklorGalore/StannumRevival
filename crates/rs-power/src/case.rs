/// MATPOWER bus classifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusType {
    /// Specified active and reactive injections (MATPOWER type 1).
    Pq,
    /// Voltage-controlled bus with specified active injection (type 2).
    Pv,
    /// Slack bus anchoring island angle and balancing active power (type 3).
    Reference,
    /// Excluded bus; incident equipment is omitted from the solve (type 4).
    Isolated,
}

/// A bus. Power and shunt values are MW/MVAr on the system base.
#[derive(Clone, Debug, PartialEq)]
pub struct Bus {
    /// Positive, unique external bus ID; need not be consecutive.
    pub id: u32,
    /// Bus classification used to select power-flow equations.
    pub kind: BusType,
    /// Active demand in MW.
    pub pd: f64,
    /// Reactive demand in MVAr.
    pub qd: f64,
    /// Constant-current active load: MW at 1 p.u. voltage, scaling with |V|
    /// (PSS/E IP). Not a MATPOWER column; see [`Bus::load_at`].
    pub pd_i: f64,
    /// Constant-current reactive load: MVAr consumed at 1 p.u. voltage,
    /// scaling with |V| (PSS/E IQ; positive is inductive).
    pub qd_i: f64,
    /// Constant-impedance active load: MW at 1 p.u. voltage, scaling with
    /// |V|² (PSS/E YP).
    pub pd_z: f64,
    /// Constant-impedance reactive load: MVAr consumed at 1 p.u. voltage,
    /// scaling with |V|² (the negative of PSS/E YQ, so positive is inductive).
    pub qd_z: f64,
    /// Shunt conductance: MW consumed at 1 p.u. voltage.
    pub gs: f64,
    /// Shunt susceptance: MVAr injected at 1 p.u. voltage.
    pub bs: f64,
    /// Area identifier, retained as metadata.
    pub area: u32,
    /// Voltage magnitude in per unit: initial on input, solved on output.
    pub vm: f64,
    /// Voltage angle in degrees.
    pub va: f64,
    /// Nominal line-to-line voltage in kV.
    pub base_kv: f64,
    /// Loss-zone identifier, retained as metadata.
    pub zone: u32,
    /// Upper voltage limit in per unit; not enforced by power flow.
    pub vmax: f64,
    /// Lower voltage limit in per unit; not enforced by power flow.
    pub vmin: f64,
    /// Additional numeric MATPOWER columns, retained without interpretation.
    pub extra: Vec<f64>,
}

impl Bus {
    /// Total load in MW and MVAr at voltage magnitude `vm` (p.u.): the
    /// constant-power part `pd`/`qd`, plus the constant-current part times
    /// `vm`, plus the constant-impedance part times `vm²`.
    pub fn load_at(&self, vm: f64) -> (f64, f64) {
        (
            self.pd + self.pd_i * vm + self.pd_z * vm * vm,
            self.qd + self.qd_i * vm + self.qd_z * vm * vm,
        )
    }

    /// Total load in MW and MVAr at the bus's stored voltage `vm`.
    pub fn load(&self) -> (f64, f64) {
        self.load_at(self.vm)
    }

    /// Whether any load at this bus depends on voltage.
    pub fn has_voltage_dependent_load(&self) -> bool {
        [self.pd_i, self.qd_i, self.pd_z, self.qd_z]
            .iter()
            .any(|x| *x != 0.0)
    }

    /// Replace the voltage-dependent load with the constant power it draws
    /// at the stored voltage `vm`, as MATPOWER does on RAW import.
    pub fn freeze_load(&mut self) {
        (self.pd, self.qd) = self.load();
        (self.pd_i, self.qd_i, self.pd_z, self.qd_z) = (0.0, 0.0, 0.0, 0.0);
    }
}

/// A generator in MATPOWER column order, with power in MW/MVAr.
#[derive(Clone, Debug, PartialEq)]
pub struct Generator {
    /// External ID of the connected bus.
    pub bus: u32,
    /// Active output in MW; adjusted for slack generation.
    pub pg: f64,
    /// Reactive output in MVAr; allocated at controlled buses in AC flow.
    pub qg: f64,
    /// Upper reactive limit in MVAr; positive infinity is allowed.
    pub qmax: f64,
    /// Lower reactive limit in MVAr; negative infinity is allowed.
    pub qmin: f64,
    /// Voltage magnitude setpoint in per unit at controlled buses.
    pub vg: f64,
    /// Machine MVA base, retained as metadata; calculations use the system base.
    pub mbase: f64,
    /// Whether this generator is in service.
    pub online: bool,
    /// Upper active limit in MW; not enforced by power flow.
    pub pmax: f64,
    /// Lower active limit in MW; not enforced by power flow.
    pub pmin: f64,
    /// Columns after PMIN, retained without interpretation.
    pub extra: Vec<f64>,
    /// External ID of the bus whose voltage this generator holds at `vg`,
    /// when that is not its own bus (PSS/E IREG). `None` regulates locally.
    /// Not a MATPOWER column, so MATPOWER import and export leave it out.
    pub regulated_bus: Option<u32>,
    /// Percent of the reactive power needed to hold a remote bus that this
    /// generator supplies (PSS/E RMPCT); 100 by default. Generators on
    /// different buses that hold the same bus share in proportion to it.
    pub reactive_share: f64,
}

/// A line or transformer using the MATPOWER from-side tap convention.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    /// External ID of the from-side bus.
    pub from: u32,
    /// External ID of the to-side bus.
    pub to: u32,
    /// Series resistance in per unit on the system base.
    pub r: f64,
    /// Series reactance in per unit on the system base.
    pub x: f64,
    /// Total line-charging susceptance in per unit, split equally between ends.
    pub b: f64,
    /// Long-term rating in MVA; retained but not enforced.
    pub rate_a: f64,
    /// Short-term rating in MVA; retained but not enforced.
    pub rate_b: f64,
    /// Emergency rating in MVA; retained but not enforced.
    pub rate_c: f64,
    /// Off-nominal magnitude; zero means unity.
    pub tap: f64,
    /// Phase shift in degrees.
    pub shift: f64,
    /// Whether this branch is in service.
    pub online: bool,
    /// Lower from-minus-to angle limit in degrees; not enforced.
    pub angmin: f64,
    /// Upper from-minus-to angle limit in degrees; not enforced.
    pub angmax: f64,
    /// Columns after ANGMAX, retained without updating stored flow results.
    pub extra: Vec<f64>,
    /// Magnetizing conductance in per unit on the system base, connected
    /// from the from bus to ground on the bus side of the tap (PSS/E MAG1,
    /// modelled on winding one). Not a MATPOWER column; zero for lines.
    pub mag_g: f64,
    /// Magnetizing susceptance in per unit on the system base (PSS/E MAG2,
    /// negative for an inductive exciting current). Not a MATPOWER column.
    pub mag_b: f64,
    /// The impedance correction table (PSS/E TAB) that scales this
    /// winding's impedance with its ratio or angle. When set, `r` and `x`
    /// are the corrected values; [`Branch::correct_impedance`] works them
    /// out again after `tap` or `shift` changes. Not a MATPOWER column.
    pub correction: Option<Box<ImpedanceCorrection>>,
}

/// A transformer winding's impedance correction table (PSS/E impedance
/// correction data), with the winding's nominal impedance it scales.
#[derive(Clone, Debug, PartialEq)]
pub struct ImpedanceCorrection {
    /// The table's number in the source case.
    pub table: u32,
    /// `(T, F)` points in strictly ascending `T`: the winding's ratio in per
    /// unit of its bus base voltage (or, with [`Self::by_angle`], its phase
    /// shift in degrees) and the complex factor that multiplies the nominal
    /// impedance. Revisions before 34 give real factors.
    pub points: Vec<(f64, f64, f64)>,
    /// Whether `T` is the phase shift angle rather than the ratio.
    pub by_angle: bool,
    /// The winding's ratio is `branch.tap / tap_per_ratio` (a two-winding
    /// transformer's tap is the ratio of its winding ratios).
    pub tap_per_ratio: f64,
    /// Nominal series resistance and reactance, per unit on the system base.
    pub r: f64,
    pub x: f64,
}

impl ImpedanceCorrection {
    /// The factor at `t`, linear between points and held at the end
    /// points' factors beyond them.
    pub fn factor(&self, t: f64) -> (f64, f64) {
        let p = &self.points;
        match p.len() {
            0 => return (1.0, 0.0),
            1 => return (p[0].1, p[0].2),
            _ => {}
        }
        if t <= p[0].0 {
            return (p[0].1, p[0].2);
        }
        for w in p.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t <= b.0 {
                let s = (t - a.0) / (b.0 - a.0);
                return (a.1 + s * (b.1 - a.1), a.2 + s * (b.2 - a.2));
            }
        }
        let last = p[p.len() - 1];
        (last.1, last.2)
    }
}

impl Branch {
    /// Work out `r` and `x` again from the impedance correction table at
    /// the branch's present tap or angle. Does nothing without a table.
    pub fn correct_impedance(&mut self) {
        let Some(c) = &self.correction else { return };
        let t = if c.by_angle {
            self.shift
        } else {
            let tap = if self.tap == 0.0 { 1.0 } else { self.tap };
            tap / c.tap_per_ratio
        };
        let (fr, fi) = c.factor(t);
        // (r + jx)(fr + j fi)
        self.r = c.r * fr - c.x * fi;
        self.x = c.r * fi + c.x * fr;
    }

    /// Set the corrected impedance directly (an edit of `r` or `x`): the
    /// nominal impedance becomes what the present factor turns into it.
    pub fn set_impedance(&mut self, r: f64, x: f64) {
        (self.r, self.x) = (r, x);
        let tap = self.tap;
        let shift = self.shift;
        if let Some(c) = &mut self.correction {
            let t = if c.by_angle {
                shift
            } else {
                (if tap == 0.0 { 1.0 } else { tap }) / c.tap_per_ratio
            };
            let (fr, fi) = c.factor(t);
            let d = fr * fr + fi * fi;
            if d > 0.0 {
                // (r + jx) / (fr + j fi)
                c.r = (r * fr + x * fi) / d;
                c.x = (x * fr - r * fi) / d;
            }
        }
    }
}

/// An editable case. Parsing preserves data; [`crate::solve`] validates the network.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// System power base in MVA; must be finite and positive for solving.
    pub base_mva: f64,
    /// Bus rows in external order.
    pub buses: Vec<Bus>,
    /// Generator rows in external order.
    pub generators: Vec<Generator>,
    /// Branch rows in external order.
    pub branches: Vec<Branch>,
    /// MATPOWER DC line rows (at least 17 columns), retained for import/export
    /// and represented as paired terminal injections during power flow.
    pub dcline: Option<Vec<Vec<f64>>>,
    /// Optional cost rows, preserved for future optimization support.
    pub gencost: Option<Vec<Vec<f64>>>,
    /// Literal bus_name, gentype and genfuel string tables.
    pub labels: std::collections::BTreeMap<String, Vec<String>>,
}
