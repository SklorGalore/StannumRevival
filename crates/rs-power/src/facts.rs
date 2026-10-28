//! FACTS devices (PSS/E FACTS control device data).
//!
//! Every device has a shunt element at its sending-end bus that holds a
//! voltage (`VSET`) with a current limit (`SHMX`), like a STATCOM. A device
//! with a terminal-end bus also has a series element, which in normal mode
//! sends a set active and reactive power (`PDES`, `QDES`) to that bus, like
//! a unified power flow controller.

use crate::{BusType, Case, Error, Generator};

/// A FACTS device in service, usually from [`crate::RawImport::voltage_controls`].
#[derive(Clone, Debug, PartialEq)]
pub struct FactsDevice {
    /// Device name (`NAME`, or the device number `N` before revision 33).
    pub name: String,
    /// External ID of the sending-end bus, where the shunt element connects (I).
    pub bus: u32,
    /// External ID of the terminal-end bus of the series element (J); `None`
    /// for a shunt-only device such as a STATCOM.
    pub terminal_bus: Option<u32>,
    /// Bus whose voltage the shunt element holds when it is not `bus`
    /// (PSS/E REMOT, or FCREG from revision 35).
    pub regulated_bus: Option<u32>,
    /// Voltage the shunt element holds, in per unit (VSET).
    pub vset: f64,
    /// Largest shunt current, as MVA at 1 p.u. voltage (SHMX). The shunt's
    /// reactive output is kept within ±`shunt_max` × `vset` Mvar.
    pub shunt_max: f64,
    /// Active power the series element sends to `terminal_bus`, in MW (PDES).
    pub p_desired: f64,
    /// Reactive power the series element sends to `terminal_bus`, in Mvar (QDES).
    pub q_desired: f64,
    /// Percent of the reactive power holding `regulated_bus` that this
    /// device gives (RMPCT).
    pub reactive_share: f64,
    /// Starting output of the shunt element in Mvar, zero from a file. A
    /// solve starting from an earlier solution
    /// ([`crate::Initialization::Solution`]) with reactive limits on keeps
    /// a shunt element that starts at a limit there, like a generator.
    pub initial_mvar: f64,
    /// One-based source line of the RAW record, for messages.
    pub line: usize,
}

impl FactsDevice {
    /// Reactive range of the shunt element in Mvar, as (lowest, highest).
    pub fn shunt_range(&self) -> (f64, f64) {
        let q = self.shunt_max * self.vset;
        (-q, q)
    }
}

/// Operating point of a FACTS device after a solve.
#[derive(Clone, Debug, PartialEq)]
pub struct FactsSolution {
    /// Reactive power the shunt element injects at its bus, in Mvar.
    pub shunt_mvar: f64,
    /// Whether the shunt element ended at an end of its range, so that it
    /// no longer holds its voltage.
    pub at_limit: bool,
    /// Whether the shunt element was held at zero output: its bus already
    /// has a generator holding the voltage, or is isolated, or the solve
    /// was DC.
    pub held: bool,
    /// Active power the series element sends to the terminal-end bus, in
    /// MW; zero for a shunt-only device.
    pub series_mw: f64,
    /// Reactive power the series element sends to the terminal-end bus, in
    /// Mvar; zero for a shunt-only device.
    pub series_mvar: f64,
}

/// FACTS devices applied to a working case before solving.
#[derive(Clone, Debug, Default)]
pub(crate) struct Setup {
    /// The helper generator standing in for each device's shunt element,
    /// in device order; `None` when the shunt is held.
    pub helpers: Vec<Option<usize>>,
    /// Whether each device's series element is applied.
    series: Vec<bool>,
    /// Load changes made for series elements, as (bus index, MW, Mvar), to
    /// take back out of the solved case.
    loads: Vec<(usize, f64, f64)>,
}

/// Apply `devices` to `case`: each series element becomes a fixed transfer
/// of `p_desired` and `q_desired` from its sending-end bus to its
/// terminal-end bus, and (when `ac`) each shunt element a helper generator
/// that holds its voltage within its reactive range. A shunt element at a
/// bus that already has an online generator, or at an isolated bus, is held
/// at zero.
pub(crate) fn setup(case: &mut Case, devices: &[FactsDevice], ac: bool) -> Result<Setup, Error> {
    let index: std::collections::HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let known = |id: u32| {
        index
            .get(&id)
            .copied()
            .ok_or_else(|| Error::InvalidCase(format!("FACTS device references unknown bus {id}")))
    };
    let mut generated = vec![false; case.buses.len()];
    for g in case.generators.iter().filter(|g| g.online) {
        if let Some(&i) = index.get(&g.bus) {
            generated[i] = true;
        }
    }
    let mut setup = Setup::default();
    for device in devices {
        let i = known(device.bus)?;
        if let Some(r) = device.regulated_bus {
            known(r)?;
        }
        if !(device.vset > 0.0
            && device.shunt_max >= 0.0
            && device.p_desired.is_finite()
            && device.q_desired.is_finite()
            && device.shunt_max.is_finite())
        {
            return Err(Error::InvalidCase(format!(
                "FACTS device {} at bus {} has an invalid VSET, SHMX, PDES or QDES",
                device.name, device.bus
            )));
        }
        let before = setup.loads.len();
        if let Some(j) = device.terminal_bus {
            let j = known(j)?;
            let live = |k: usize| case.buses[k].kind != BusType::Isolated;
            if live(i) && live(j) && i != j {
                let (p, q) = (device.p_desired, device.q_desired);
                case.buses[i].pd += p;
                case.buses[i].qd += q;
                case.buses[j].pd -= p;
                case.buses[j].qd -= q;
                setup.loads.push((i, p, q));
                setup.loads.push((j, -p, -q));
            }
        }
        setup.series.push(setup.loads.len() > before);
        let bus = &mut case.buses[i];
        if !ac || bus.kind == BusType::Isolated || device.shunt_max == 0.0 {
            setup.helpers.push(None);
            continue;
        }
        if generated[i] {
            log::info!(
                "  FACTS device {} at bus {} shares its bus with a generator that holds the voltage; its shunt element is held at zero",
                device.name,
                device.bus
            );
            setup.helpers.push(None);
            continue;
        }
        if bus.kind == BusType::Pq {
            bus.kind = BusType::Pv;
        }
        let (qmin, qmax) = device.shunt_range();
        setup.helpers.push(Some(case.generators.len()));
        case.generators.push(Generator {
            bus: device.bus,
            pg: 0.0,
            qg: device.initial_mvar.clamp(qmin, qmax),
            qmax,
            qmin,
            vg: device.vset,
            mbase: 100.0,
            online: true,
            pmax: 0.0,
            pmin: 0.0,
            extra: Vec::new(),
            regulated_bus: device.regulated_bus.filter(|&r| r != device.bus),
            reactive_share: device.reactive_share,
        });
    }
    Ok(setup)
}

/// Read each device's operating point from the solved `case` and take the
/// series transfers back out of its loads. Helper generators are removed
/// later with the other added generators. A shunt element's output is not
/// folded into the bus: `case` holds the network as given, and the
/// solutions say what the devices added.
pub(crate) fn finish(
    setup: &Setup,
    devices: &[FactsDevice],
    case: &mut Case,
) -> Vec<FactsSolution> {
    for &(i, p, q) in &setup.loads {
        case.buses[i].pd -= p;
        case.buses[i].qd -= q;
    }
    devices
        .iter()
        .zip(setup.helpers.iter().zip(&setup.series))
        .map(|(device, (&helper, &series))| {
            let (shunt_mvar, at_limit) = match helper {
                Some(g) => {
                    let gen_data = &case.generators[g];
                    let q = gen_data.qg;
                    let tolerance = 1e-6_f64.max(1e-6 * gen_data.qmax.abs());
                    (
                        q,
                        q >= gen_data.qmax - tolerance || q <= gen_data.qmin + tolerance,
                    )
                }
                None => (0.0, false),
            };
            FactsSolution {
                shunt_mvar,
                at_limit,
                held: helper.is_none(),
                series_mw: if series { device.p_desired } else { 0.0 },
                series_mvar: if series { device.q_desired } else { 0.0 },
            }
        })
        .collect()
}
