/// MATPOWER bus classifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusType {
    Pq,
    Pv,
    Reference,
    Isolated,
}

/// A bus. Power and shunt values are MW/MVAr on the system base.
#[derive(Clone, Debug)]
pub struct Bus {
    pub id: u32,
    pub kind: BusType,
    pub pd: f64,
    pub qd: f64,
    pub gs: f64,
    pub bs: f64,
    pub area: u32,
    pub vm: f64,
    /// Voltage angle in degrees.
    pub va: f64,
    pub base_kv: f64,
    pub zone: u32,
    pub vmax: f64,
    pub vmin: f64,
    /// Additional numeric MATPOWER columns, retained without interpretation.
    pub extra: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct Generator {
    pub bus: u32,
    pub pg: f64,
    pub qg: f64,
    pub qmax: f64,
    pub qmin: f64,
    pub vg: f64,
    pub mbase: f64,
    pub online: bool,
    pub pmax: f64,
    pub pmin: f64,
    /// Columns after PMIN, retained without interpretation.
    pub extra: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct Branch {
    pub from: u32,
    pub to: u32,
    pub r: f64,
    pub x: f64,
    pub b: f64,
    pub rate_a: f64,
    pub rate_b: f64,
    pub rate_c: f64,
    /// Off-nominal magnitude; zero means unity.
    pub tap: f64,
    /// Phase shift in degrees.
    pub shift: f64,
    pub online: bool,
    pub angmin: f64,
    pub angmax: f64,
    pub extra: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct Case {
    pub base_mva: f64,
    pub buses: Vec<Bus>,
    pub generators: Vec<Generator>,
    pub branches: Vec<Branch>,
    /// Optional cost rows, preserved for future optimization support.
    pub gencost: Option<Vec<Vec<f64>>>,
    /// Literal bus_name, gentype and genfuel string tables.
    pub labels: std::collections::BTreeMap<String, Vec<String>>,
}
