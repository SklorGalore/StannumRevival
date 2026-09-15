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
