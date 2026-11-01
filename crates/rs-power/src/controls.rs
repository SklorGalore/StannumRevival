//! Voltage controls applied between Newton solves: switched shunts and
//! transformer tap changers (PSS/E switched shunt MODSW 1–6, winding COD 1),
//! and the FACTS devices that hold bus voltages during each solve.

/// Voltage controls to apply during an AC solve, usually from
/// [`crate::RawImport::voltage_controls`]. Empty by default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoltageControls {
    /// Switched shunts that hold a bus voltage.
    pub switched_shunts: Vec<SwitchedShunt>,
    /// Transformer windings whose taps hold a bus voltage.
    pub taps: Vec<TapControl>,
    /// FACTS devices in service. Their shunt elements hold bus voltages in
    /// AC solves; their series elements also move active power in DC solves.
    pub facts: Vec<FactsDevice>,
}

impl VoltageControls {
    /// True when there is nothing to control.
    pub fn is_empty(&self) -> bool {
        self.switched_shunts.is_empty() && self.taps.is_empty() && self.facts.is_empty()
    }
}

/// How a switched shunt moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShuntMode {
    /// Whole blocks switch in and out to keep the voltage within
    /// `vmin..=vmax` (PSS/E MODSW 1).
    Discrete,
    /// The susceptance moves smoothly within the blocks' range to hold the
    /// voltage at the middle of the band, like a static var compensator
    /// (PSS/E MODSW 2).
    Continuous,
    /// Whole blocks switch to keep the reactive output of the plant at
    /// `bus` within the band (PSS/E MODSW 3). The band is given in
    /// fractions of the plant's total range, QMIN to QMAX.
    PlantReactive {
        /// External ID of the plant's bus.
        bus: u32,
    },
    /// Whole blocks switch to keep the reactive output of a VSC converter
    /// within the band (PSS/E MODSW 4), in fractions of the converter's
    /// MINQ to MAXQ.
    ConverterReactive {
        /// The converter's row in `case.dcline`.
        row: usize,
        /// External ID of the converter's AC bus, one end of that row.
        bus: u32,
    },
    /// Whole blocks switch to keep the susceptance of another switched
    /// shunt within the band (PSS/E MODSW 5), in fractions of that shunt's
    /// range from its largest reactor total to its largest capacitor total.
    ShuntSetting {
        /// Index of the other shunt in [`VoltageControls::switched_shunts`].
        shunt: usize,
    },
    /// Whole blocks switch to keep the reactive output of a FACTS device's
    /// shunt element within the band (PSS/E MODSW 6), in fractions of its
    /// range, -SHMX to +SHMX at its voltage setpoint.
    FactsReactive {
        /// Index of the device in [`VoltageControls::facts`].
        device: usize,
    },
}

impl ShuntMode {
    /// Whether the band is in fractions of a reactive range (modes 3 to 5)
    /// rather than a voltage band in per unit.
    pub fn watches_reactive(self) -> bool {
        !matches!(self, ShuntMode::Discrete | ShuntMode::Continuous)
    }
}

/// A switched shunt in voltage or reactive control.
///
/// Its starting susceptance `initial` is already part of the bus's `bs`
/// (as in MATPOWER); the solver moves `bs` by the change from it.
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchedShunt {
    /// External ID of the bus the shunt is connected to.
    pub bus: u32,
    /// How it switches and what it watches.
    pub mode: ShuntMode,
    /// Bus whose voltage is controlled when it is not `bus` (PSS/E SWREM),
    /// in the voltage modes.
    pub regulated_bus: Option<u32>,
    /// Upper limit of the band (VSWHI): a voltage in per unit, or for the
    /// reactive modes a fraction of the watched device's range.
    pub vmax: f64,
    /// Lower limit of the band (VSWLO), in the same units as `vmax`.
    pub vmin: f64,
    /// Starting susceptance, MVAr injected at 1 p.u. (BINIT).
    pub initial: f64,
    /// Blocks as (number of steps, MVAr per step at 1 p.u.); capacitors are
    /// positive and reactors negative.
    pub blocks: Vec<(u32, f64)>,
    /// One-based source line of the RAW record, for messages.
    pub line: usize,
}

impl SwitchedShunt {
    /// Every susceptance (MVAr at 1 p.u.) the blocks can make, in increasing
    /// order: reactor steps below zero, capacitor steps above.
    pub fn levels(&self) -> Vec<f64> {
        let mut levels = vec![0.0];
        for sign in [-1.0, 1.0] {
            let mut total = 0.0;
            for &(n, b) in &self.blocks {
                if b * sign > 0.0 {
                    for _ in 0..n {
                        total += b;
                        levels.push(total);
                    }
                }
            }
        }
        levels.sort_by(f64::total_cmp);
        levels
    }
}

/// A transformer winding whose tap holds a bus voltage within a band.
#[derive(Clone, Debug, PartialEq)]
pub struct TapControl {
    /// Index of the winding's entry in `case.branches`.
    pub branch: usize,
    /// External ID of the controlled bus (PSS/E CONT).
    pub regulated_bus: u32,
    /// Whether raising the ratio raises the controlled bus's voltage: true
    /// when that bus is on the winding's own side, false when it is on the
    /// far side.
    pub raises_voltage: bool,
    /// Upper voltage limit of the band, in per unit (VMA).
    pub vmax: f64,
    /// Lower voltage limit of the band, in per unit (VMI).
    pub vmin: f64,
    /// Starting winding ratio in the record's units (WINDV).
    pub ratio: f64,
    /// Highest winding ratio (RMA).
    pub ratio_max: f64,
    /// Lowest winding ratio (RMI).
    pub ratio_min: f64,
    /// Tap positions from `ratio_min` to `ratio_max` (NTP), at least 2.
    pub positions: u32,
    /// The branch's off-nominal tap per unit of winding ratio, so that
    /// `branch.tap = tap_per_ratio * ratio`.
    pub tap_per_ratio: f64,
    /// One-based source line of the winding record, for messages.
    pub line: usize,
}

impl TapControl {
    /// Ratio change of one tap step.
    pub fn step(&self) -> f64 {
        (self.ratio_max - self.ratio_min) / f64::from(self.positions.max(2) - 1)
    }
}

// ---------------------------------------------------------------------------
// Solver side.

use crate::{
    BusType, Case, Error, Generator, facts::FactsDevice, network::Network, solver::DcLineGenerators,
};
use num_complex::Complex64 as C;

/// What a discrete shunt watches, resolved against the working case.
#[derive(Clone, Debug)]
pub(crate) enum Watch {
    /// The voltage of its own or its regulated bus (MODSW 1).
    Voltage,
    /// The total reactive output of these generators (a plant, or a VSC
    /// converter's terminal generator), kept within a band in Mvar
    /// (MODSW 3 and 4).
    Reactive {
        gens: Vec<usize>,
        band: (f64, f64),
        /// "plant at bus 101" or "VSC converter at bus 101", for messages.
        what: String,
    },
    /// The susceptance of another listed shunt, kept within a band in MVAr
    /// at 1 p.u. (MODSW 5).
    Setting {
        shunt: usize,
        /// External ID of that shunt's bus, for messages.
        bus: u32,
        band: (f64, f64),
    },
}

/// Controls resolved against a working case before solving.
pub(crate) struct Setup<'a> {
    /// Discrete shunts, with their index in [`VoltageControls::switched_shunts`]
    /// and what each watches.
    pub discrete: Vec<(usize, &'a SwitchedShunt, Watch)>,
    /// Continuous shunts and the helper generator standing in for each.
    pub continuous: Vec<(usize, &'a SwitchedShunt, usize)>,
    /// Tap controls, with their index in [`VoltageControls::taps`].
    pub taps: Vec<(usize, &'a TapControl)>,
    /// Generators from this index on are FACTS and continuous-shunt
    /// helpers, which always keep to their reactive limits.
    pub limited_from: usize,
    /// Generators from this index on are continuous-shunt helpers; those
    /// from `limited_from` up to here stand in for FACTS shunt elements.
    pub continuous_from: usize,
    /// The FACTS shunt elements in control, as (device index, helper
    /// generator).
    pub facts: Vec<(usize, usize)>,
    /// Number of listed shunts and taps.
    pub counts: [usize; 2],
}

impl Setup<'_> {
    pub fn none(case: &Case) -> Setup<'static> {
        Setup {
            discrete: Vec::new(),
            continuous: Vec::new(),
            taps: Vec::new(),
            limited_from: case.generators.len(),
            continuous_from: case.generators.len(),
            facts: Vec::new(),
            counts: [0, 0],
        }
    }
}

/// Validate `controls` against `case` and add a helper generator for every
/// continuous shunt, which holds its voltage within the shunt's range like a
/// static var compensator. Shunts at buses that already have an online
/// generator, or that are offline, stay at their starting value, as do
/// every shunt and every tap when `[lock_shunts, lock_taps]` says so.
///
/// `case.generators[..plants]` are the case's own generators, which make up
/// the plants that MODSW 3 shunts watch; `dc` maps DC line rows to the
/// terminal generators that stand in for their converters (MODSW 4).
/// `facts` holds the helper generator of each FACTS device's shunt element
/// (MODSW 6), all added just before this call, from `facts_from` on.
pub(crate) fn setup<'a>(
    case: &mut Case,
    controls: &'a VoltageControls,
    [lock_shunts, lock_taps]: [bool; 2],
    plants: usize,
    dc: &[DcLineGenerators],
    (facts, facts_from): (&[Option<usize>], usize),
) -> Result<Setup<'a>, Error> {
    let index: std::collections::HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let known = |id: u32, what: &str| {
        index
            .get(&id)
            .copied()
            .ok_or_else(|| Error::InvalidCase(format!("{what} references unknown bus {id}")))
    };
    let mut generated = vec![false; case.buses.len()];
    for g in case.generators.iter().filter(|g| g.online) {
        if let Some(&i) = index.get(&g.bus) {
            generated[i] = true;
        }
    }
    let mut setup = Setup {
        discrete: Vec::new(),
        continuous: Vec::new(),
        taps: Vec::new(),
        limited_from: facts_from.min(case.generators.len()),
        continuous_from: case.generators.len(),
        facts: (facts.iter().enumerate())
            .filter_map(|(k, g)| Some((k, (*g)?)))
            .collect(),
        counts: [controls.switched_shunts.len(), controls.taps.len()],
    };
    for (k, shunt) in controls.switched_shunts.iter().enumerate() {
        let i = known(shunt.bus, "switched shunt")?;
        if let Some(r) = shunt.regulated_bus {
            known(r, "switched shunt")?;
        }
        let band_ok = if shunt.mode.watches_reactive() {
            shunt.vmin >= 0.0 && shunt.vmax >= shunt.vmin && shunt.vmax <= 1.0
        } else {
            shunt.vmin > 0.0 && shunt.vmax >= shunt.vmin
        };
        if !(band_ok && shunt.initial.is_finite()) || shunt.blocks.iter().any(|b| !b.1.is_finite())
        {
            return Err(Error::InvalidCase(format!(
                "switched shunt at bus {} has an invalid band or block",
                shunt.bus
            )));
        }
        // The reactive range a MODSW 3-5 shunt keeps a fraction of, as
        // (lowest, highest), with the generators it is measured on.
        let range = |gens: &[usize]| {
            gens.iter().fold((0.0, 0.0), |(lo, hi), &g| {
                (lo + case.generators[g].qmin, hi + case.generators[g].qmax)
            })
        };
        let band =
            |(lo, hi): (f64, f64)| (lo + shunt.vmin * (hi - lo), lo + shunt.vmax * (hi - lo));
        let watch = match shunt.mode {
            ShuntMode::Discrete => Some(Watch::Voltage),
            ShuntMode::Continuous => None,
            ShuntMode::PlantReactive { bus } => {
                known(bus, "switched shunt")?;
                let gens: Vec<usize> = (0..plants.min(case.generators.len()))
                    .filter(|&g| case.generators[g].online && case.generators[g].bus == bus)
                    .collect();
                let (lo, hi) = range(&gens);
                if gens.is_empty() || !(hi - lo > 1e-6 && (hi - lo).is_finite()) {
                    if !lock_shunts {
                        log::info!(
                            "  switched shunt at bus {} watches the plant at bus {bus}, which has no online generator with a reactive range; holding it",
                            shunt.bus
                        );
                    }
                    continue;
                }
                Some(Watch::Reactive {
                    gens,
                    band: band((lo, hi)),
                    what: format!("plant at bus {bus}"),
                })
            }
            ShuntMode::ConverterReactive { row, bus } => {
                known(bus, "switched shunt")?;
                let g = dc.iter().find(|m| m.row == row).and_then(|m| {
                    [m.from_generator, m.to_generator]
                        .into_iter()
                        .find(|&g| case.generators[g].bus == bus)
                });
                let range = g.map(|g| range(&[g]));
                let Some((g, (lo, hi))) = g.zip(range).filter(|(_, (lo, hi))| hi - lo > 1e-6)
                else {
                    if !lock_shunts {
                        log::info!(
                            "  switched shunt at bus {} watches a VSC converter at bus {bus} that is off or in power factor control; holding it",
                            shunt.bus
                        );
                    }
                    continue;
                };
                Some(Watch::Reactive {
                    gens: vec![g],
                    band: band((lo, hi)),
                    what: format!("VSC converter at bus {bus}"),
                })
            }
            ShuntMode::FactsReactive { device } => {
                let Some(fd) = controls.facts.get(device) else {
                    return Err(Error::InvalidCase(format!(
                        "switched shunt at bus {} watches a missing FACTS device {device}",
                        shunt.bus
                    )));
                };
                let Some(g) = facts.get(device).copied().flatten() else {
                    if !lock_shunts {
                        log::info!(
                            "  switched shunt at bus {} watches FACTS device {}, whose shunt element is held; holding it",
                            shunt.bus,
                            fd.name
                        );
                    }
                    continue;
                };
                Some(Watch::Reactive {
                    gens: vec![g],
                    band: band(range(&[g])),
                    what: format!("FACTS device {}", fd.name),
                })
            }
            ShuntMode::ShuntSetting { shunt: other } => {
                let Some(target) = controls.switched_shunts.get(other).filter(|_| other != k)
                else {
                    return Err(Error::InvalidCase(format!(
                        "switched shunt at bus {} watches a missing switched shunt {other}",
                        shunt.bus
                    )));
                };
                let levels = target.levels();
                Some(Watch::Setting {
                    shunt: other,
                    bus: target.bus,
                    band: band((levels[0], levels[levels.len() - 1])),
                })
            }
        };
        if lock_shunts {
            continue;
        }
        match watch {
            Some(watch) => setup.discrete.push((k, shunt, watch)),
            None => {
                if case.buses[i].kind == BusType::Isolated {
                    continue;
                }
                let levels = shunt.levels();
                let (lo, hi) = (levels[0], levels[levels.len() - 1]);
                // A second continuous shunt at a bus joins the first one's
                // helper, which then covers both ranges and holds the higher
                // of their setpoints (as PowerWorld solves the Texas 2000-bus
                // case, whose Del Rio 115 kV bus has shunts set to 1.02 and
                // 1.03 p.u.).
                if let Some(&(_, _, g)) = setup.continuous.iter().find(|c| c.1.bus == shunt.bus) {
                    let helper = &mut case.generators[g];
                    helper.vg = helper.vg.max(0.5 * (shunt.vmax + shunt.vmin));
                    let vg2 = helper.vg * helper.vg;
                    helper.qmax += (hi - shunt.initial) * vg2;
                    helper.qmin += (lo - shunt.initial) * vg2;
                    setup.continuous.push((k, shunt, g));
                    continue;
                }
                let bus = &mut case.buses[i];
                if generated[i] {
                    continue;
                }
                let vg = 0.5 * (shunt.vmax + shunt.vmin);
                if bus.kind == BusType::Pq {
                    bus.kind = BusType::Pv;
                }
                generated[i] = true;
                setup.continuous.push((k, shunt, case.generators.len()));
                case.generators.push(Generator {
                    bus: shunt.bus,
                    pg: 0.0,
                    qg: 0.0,
                    qmax: (hi - shunt.initial) * vg * vg,
                    qmin: (lo - shunt.initial) * vg * vg,
                    vg,
                    mbase: 100.0,
                    online: true,
                    pmax: 0.0,
                    pmin: 0.0,
                    extra: Vec::new(),
                    regulated_bus: shunt.regulated_bus,
                    reactive_share: 100.0,
                });
            }
        }
    }
    for (k, tap) in controls.taps.iter().enumerate() {
        known(tap.regulated_bus, "tap control")?;
        if tap.branch >= case.branches.len()
            || !(tap.vmin > 0.0 && tap.vmax >= tap.vmin)
            || !(tap.ratio_max > tap.ratio_min && tap.ratio > 0.0 && tap.tap_per_ratio > 0.0)
        {
            return Err(Error::InvalidCase(format!(
                "tap control on branch {} has an invalid branch, band or ratio range",
                tap.branch
            )));
        }
        if !lock_taps {
            setup.taps.push((k, tap));
        }
    }
    Ok(setup)
}

#[derive(Clone)]
struct ShuntState<'a> {
    index: usize,
    shunt: &'a SwitchedShunt,
    /// Index in `case.buses`.
    bus: usize,
    /// What the shunt watches; `None` when it is held from the start (its
    /// bus or controlled bus is offline, or what it watches never moves).
    watching: Option<Watching>,
    levels: Vec<f64>,
    value: f64,
    last: i8,
    reversals: u8,
    /// The level the shunt last moved away from and the watched value it
    /// gave, so that a step back can be judged before it is taken.
    previous: Option<(f64, f64)>,
    /// Moved in the last pass, so its effect on what it watches can be
    /// measured now.
    fresh: bool,
}

/// What a discrete shunt watches during the solve.
#[derive(Clone)]
enum Watching {
    /// Network index of the bus whose voltage is held.
    Voltage(usize),
    /// Generators whose total reactive output is kept in `band` (Mvar).
    Reactive {
        gens: Vec<usize>,
        band: (f64, f64),
        what: String,
    },
    /// Another shunt whose susceptance is kept in `band` (MVAr at 1 p.u.).
    Setting {
        source: Source,
        bus: u32,
        band: (f64, f64),
    },
}

/// Where the susceptance of a watched shunt is read from.
#[derive(Clone, Copy)]
enum Source {
    /// A discrete shunt, by its position in [`Controller::shunts`].
    Discrete(usize),
    /// A continuous shunt: its listed index, starting value, helper
    /// generator, and the network index of its bus.
    Continuous {
        listed: usize,
        initial: f64,
        helper: usize,
        bus: usize,
    },
}

/// Mvar (or MVAr at 1 p.u.) outside a reactive band before a shunt moves.
const REACTIVE_TOLERANCE: f64 = 0.01;

/// Smallest drop in the watched reactive output, per Mvar of susceptance
/// switched in, for a MODSW 3-5 shunt to keep switching.
const MIN_SHUNT_EFFECT: f64 = 0.05;

/// How far `x` is outside `lo..=hi`; zero inside.
fn band_violation(x: f64, lo: f64, hi: f64) -> f64 {
    (lo - x).max(x - hi).max(0.0)
}

/// A value a discrete shunt watches, read between solves.
struct Seen {
    value: f64,
    band: (f64, f64),
    /// A voltage in per unit, rather than Mvar.
    voltage: bool,
    /// What the value belongs to, for messages.
    what: String,
}

impl Seen {
    fn reading(&self, x: f64) -> String {
        if self.voltage {
            format!("{} at {x:.4} pu", self.what)
        } else {
            format!("{} at {x:.1} Mvar", self.what)
        }
    }

    fn band_text(&self) -> String {
        let (lo, hi) = self.band;
        if self.voltage {
            format!("{lo:.4}-{hi:.4} pu")
        } else {
            format!("{lo:.1} to {hi:.1} Mvar")
        }
    }
}

#[derive(Clone)]
struct TapState<'a> {
    index: usize,
    tap: &'a TapControl,
    controlled: Option<usize>,
    ratio: f64,
    /// Taps holding the same bus, which share each correction.
    sharing: usize,
    last: i8,
    reversals: u8,
    /// Controlled voltage and ratio before the last move, and the solve
    /// round it was made in, to measure how strongly the ratio moves the
    /// voltage.
    before: Option<(f64, f64, usize)>,
    /// Measured voltage change per unit of ratio.
    sensitivity: Option<f64>,
    /// Held because the ratio barely moves, or wrongly moves, the voltage.
    ineffective: bool,
}

/// Steps discrete shunts and transformer taps between Newton solves.
#[derive(Clone)]
pub(crate) struct Controller<'a> {
    shunts: Vec<ShuntState<'a>>,
    taps: Vec<TapState<'a>>,
    /// Continuous shunts as (listed index, helper generator, room below and
    /// above the starting value in MVAr at 1 p.u.). Several shunts at one
    /// bus share a helper.
    helpers: Vec<(usize, usize, f64, f64)>,
    /// Continuous shunts fixed at an end of their range, as (listed index,
    /// susceptance moved into the bus's `bs`, helper generator).
    fixed: Vec<(usize, f64, usize)>,
    /// The reactive range of each helper set aside while its shunts are
    /// fixed, as (helper, qmin, qmax).
    parked: Vec<(usize, f64, f64)>,
    counts: [usize; 2],
}

/// Direction reversals after which a device is held, so that it cannot hunt.
const MAX_REVERSALS: u8 = 2;

/// Smallest relative voltage change per relative ratio change for a tap to
/// keep controlling its bus.
const MIN_TAP_EFFECT: f64 = 0.05;

/// Fraction of a band inside its nearer edge that a tap aims for.
const TARGET_MARGIN: f64 = 0.2;

/// Most tap steps in one pass.
const MAX_TAP_STEPS: f64 = 4.0;

/// Voltage tolerance (p.u.) outside a band before a device moves.
const BAND_TOLERANCE: f64 = 1e-5;

impl<'a> Controller<'a> {
    pub fn new(setup: &Setup<'a>, net: &Network, case: &Case) -> Self {
        let index: std::collections::HashMap<u32, usize> = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let position: std::collections::HashMap<usize, usize> = setup
            .discrete
            .iter()
            .enumerate()
            .map(|(p, d)| (d.0, p))
            .collect();
        let shunts = setup
            .discrete
            .iter()
            .map(|(k, shunt, watch)| {
                let watching = match watch {
                    Watch::Voltage => net
                        .by_id
                        .get(&shunt.regulated_bus.unwrap_or(shunt.bus))
                        .copied()
                        .map(Watching::Voltage),
                    Watch::Reactive { gens, band, what } => Some(Watching::Reactive {
                        gens: gens.clone(),
                        band: *band,
                        what: what.clone(),
                    }),
                    Watch::Setting {
                        shunt: other,
                        bus,
                        band,
                    } => {
                        let source = if let Some(&p) = position.get(other) {
                            Some(Source::Discrete(p))
                        } else {
                            setup.continuous.iter().find(|c| c.0 == *other).and_then(
                                |&(_, target, helper)| {
                                    Some(Source::Continuous {
                                        listed: *other,
                                        initial: target.initial,
                                        helper,
                                        bus: *net.by_id.get(&target.bus)?,
                                    })
                                },
                            )
                        };
                        if source.is_none() {
                            log::info!(
                                "  switched shunt at bus {} watches the switched shunt at bus {}, which does not move; holding it",
                                shunt.bus, bus
                            );
                        }
                        source.map(|source| Watching::Setting {
                            source,
                            bus: *bus,
                            band: *band,
                        })
                    }
                };
                ShuntState {
                    index: *k,
                    shunt,
                    bus: index[&shunt.bus],
                    watching: watching.filter(|_| net.by_id.contains_key(&shunt.bus)),
                    levels: shunt.levels(),
                    value: shunt.initial,
                    last: 0,
                    reversals: 0,
                    previous: None,
                    fresh: false,
                }
            })
            .collect();
        let mut held = std::collections::HashMap::<u32, usize>::new();
        for &(_, tap) in &setup.taps {
            if case.branches[tap.branch].online {
                *held.entry(tap.regulated_bus).or_default() += 1;
            }
        }
        let taps = setup
            .taps
            .iter()
            .map(|&(k, tap)| TapState {
                index: k,
                tap,
                controlled: net
                    .by_id
                    .get(&tap.regulated_bus)
                    .copied()
                    .filter(|_| case.branches[tap.branch].online),
                ratio: tap.ratio,
                sharing: held.get(&tap.regulated_bus).copied().unwrap_or(1).max(1),
                last: 0,
                reversals: 0,
                before: None,
                sensitivity: None,
                ineffective: false,
            })
            .collect();
        Self {
            shunts,
            taps,
            helpers: setup
                .continuous
                .iter()
                .map(|&(k, shunt, g)| {
                    let levels = shunt.levels();
                    let (lo, hi) = (levels[0], levels[levels.len() - 1]);
                    (k, g, shunt.initial - lo, hi - shunt.initial)
                })
                .collect(),
            fixed: Vec::new(),
            parked: Vec::new(),
            counts: setup.counts,
        }
    }

    /// Whether there is anything to step.
    pub fn is_empty(&self) -> bool {
        self.shunts.is_empty() && self.taps.is_empty()
    }

    /// One control pass at voltages `v`: move every device whose controlled
    /// bus is outside its band (and not held by a generator), updating the
    /// shunts' `bs` and the branches' `tap` in `case`. Returns the number of
    /// devices moved.
    ///
    /// `round` counts the solves so far; a tap's response is measured only
    /// across consecutive rounds, when nothing else changed in between.
    /// Switched shunts and tap changers under voltage control.
    pub fn devices(&self) -> (usize, usize) {
        (self.shunts.len(), self.taps.len())
    }

    /// The value shunt `s` watches now, with its band; `None` while the
    /// shunt has nothing to act on (a voltage held by a generator, say).
    fn watched(&self, s: &ShuntState<'_>, case: &Case, v: &[C], kinds: &[BusType]) -> Option<Seen> {
        Some(match s.watching.as_ref()? {
            &Watching::Voltage(c) => {
                if kinds[c] != BusType::Pq {
                    return None;
                }
                Seen {
                    value: v[c].norm(),
                    band: (s.shunt.vmin, s.shunt.vmax),
                    voltage: true,
                    what: format!("bus {}", s.shunt.regulated_bus.unwrap_or(s.shunt.bus)),
                }
            }
            Watching::Reactive { gens, band, what } => Seen {
                value: gens.iter().map(|&g| case.generators[g].qg).sum(),
                band: *band,
                voltage: false,
                what: what.clone(),
            },
            &Watching::Setting { source, bus, band } => Seen {
                value: match source {
                    Source::Discrete(p) => self.shunts[p].value,
                    Source::Continuous {
                        listed,
                        initial,
                        helper,
                        bus,
                    } => {
                        let fixed: f64 = (self.fixed.iter())
                            .filter(|f| f.0 == listed)
                            .map(|f| f.1)
                            .sum();

                        let vm = v[bus].norm();
                        initial + fixed + case.generators[helper].qg / (vm * vm)
                    }
                },
                band,
                voltage: false,
                what: format!("switched shunt at bus {bus}"),
            },
        })
    }

    pub fn pass(&mut self, case: &mut Case, v: &[C], kinds: &[BusType], round: usize) -> usize {
        let mut moved = 0;
        // The first few moves of a pass go to the normal log, the rest to the detailed one.
        let report = |n: usize, text: String| {
            if n < crate::solver::DETAIL_LINES {
                log::info!("{text}");
            } else {
                log::debug!("{text}");
            }
        };
        let free = |i: usize| kinds[i] == BusType::Pq;
        // Read every watched value before any shunt moves, as a MODSW 5
        // shunt reads another shunt's setting.
        let seen: Vec<Option<Seen>> = self
            .shunts
            .iter()
            .map(|s| self.watched(s, case, v, kinds))
            .collect();
        for (s, seen) in self.shunts.iter_mut().zip(seen) {
            let Some(seen) = seen else { continue };
            if s.reversals >= MAX_REVERSALS {
                continue;
            }
            let (x, (lo, hi)) = (seen.value, seen.band);
            // A shunt that barely moves the reactive output it watches (the
            // plant is far away, or something nearer holds the voltage)
            // would run to the end of its range for nothing: put back the
            // last step and hold it.
            if let Some((level, x_level)) = s.previous
                && std::mem::take(&mut s.fresh)
                && !seen.voltage
            {
                let effect = (x_level - x) / (s.value - level);
                if effect < MIN_SHUNT_EFFECT {
                    s.reversals = MAX_REVERSALS;
                    log::info!(
                        "  switched shunt at bus {}: the {:.1} Mvar step moved {} only {:.2} Mvar; back to {level:.1} Mvar and holding it",
                        s.shunt.bus,
                        s.value - level,
                        seen.what,
                        x - x_level,
                    );
                    case.buses[s.bus].bs += level - s.value;
                    s.value = level;
                    moved += 1;
                    continue;
                }
            }
            let tolerance = if seen.voltage {
                BAND_TOLERANCE
            } else {
                REACTIVE_TOLERANCE
            };
            // Below a voltage band, or above a reactive band (the watched
            // device is supplying too much), more susceptance helps.
            let (low, high) = (x < lo - tolerance, x > hi + tolerance);
            let up = if seen.voltage { low } else { high };
            let down = if seen.voltage { high } else { low };
            let (next, dir) = if up {
                (s.levels.iter().copied().find(|&l| l > s.value + 1e-9), 1)
            } else if down {
                (
                    s.levels.iter().rev().copied().find(|&l| l < s.value - 1e-9),
                    -1,
                )
            } else {
                (None, 0)
            };
            let Some(next) = next else { continue };
            // A step back to the level just left means one step jumps right
            // over the band (it is narrower than a step, or has no width at
            // all, VSWLO = VSWHI). Both values are known by now, so settle
            // on whichever level lands nearer the band and hold there, like
            // a target with a deadband of one step, instead of hunting.
            if let Some((level, x_level)) = s.previous
                && s.last != 0
                && s.last != dir
                && (level - next).abs() < 1e-9
            {
                s.reversals = MAX_REVERSALS;
                let here = band_violation(x, lo, hi);
                let there = band_violation(x_level, lo, hi);
                let keep = if there < here { level } else { s.value };
                log::info!(
                    "  switched shunt at bus {}: one step jumps over the band {} ({} with {:.1} Mvar, {} with {:.1} Mvar); holding at {keep:.1} Mvar, nearer the band",
                    s.shunt.bus,
                    seen.band_text(),
                    seen.reading(x),
                    s.value,
                    seen.reading(x_level),
                    level,
                );
                if keep != s.value {
                    case.buses[s.bus].bs += keep - s.value;
                    s.value = keep;
                    moved += 1;
                }
                continue;
            }
            if s.last != 0 && s.last != dir {
                s.reversals += 1;
            }
            s.last = dir;
            s.previous = Some((s.value, x));
            s.fresh = true;
            report(
                moved,
                format!(
                    "  switched shunt at bus {}: {:.1} -> {:.1} Mvar, {} is outside {}",
                    s.shunt.bus,
                    s.value,
                    next,
                    seen.reading(x),
                    seen.band_text()
                ),
            );
            case.buses[s.bus].bs += next - s.value;
            s.value = next;
            moved += 1;
        }
        for t in &mut self.taps {
            let Some(c) = t.controlled.filter(|&c| free(c)) else {
                continue;
            };
            let vm = v[c].norm();
            let sign = if t.tap.raises_voltage { 1.0 } else { -1.0 };
            if let Some((v0, r0, at)) = t.before.take()
                && at + 1 == round
            {
                // Per-unit voltage change per per-unit ratio change; about
                // one against a stiff source, near zero when the controlled
                // bus is the stiff side.
                let s = (vm - v0) / (t.ratio - r0);
                if s * sign * t.ratio / vm < MIN_TAP_EFFECT {
                    t.ineffective = true;
                    let branch = &case.branches[t.tap.branch];
                    log::info!(
                        "  tap on transformer {}-{} barely moves bus {}; holding it",
                        branch.from,
                        branch.to,
                        t.tap.regulated_bus
                    );
                } else {
                    t.sensitivity = Some(s);
                }
            }
            if t.ineffective || t.reversals >= MAX_REVERSALS {
                continue;
            }
            if vm >= t.tap.vmin - BAND_TOLERANCE && vm <= t.tap.vmax + BAND_TOLERANCE {
                continue;
            }
            // Aim a little inside the nearer edge of the band, as tap changers do.
            let margin = TARGET_MARGIN * (t.tap.vmax - t.tap.vmin);
            let target = if vm < t.tap.vmin {
                t.tap.vmin + margin
            } else {
                t.tap.vmax - margin
            };
            // Until a move has been measured, assume a stiff source: the
            // voltage moves by the same fraction as the ratio, shared among
            // the taps holding the bus. That undershoots, never overshoots.
            let change = match t.sensitivity {
                Some(s) => (target - vm) / s,
                None => sign * (target - vm) / vm * t.ratio / t.sharing as f64,
            };
            let step = t.tap.step();
            let steps = (change / step).round().clamp(-MAX_TAP_STEPS, MAX_TAP_STEPS);
            let steps = if steps == 0.0 { change.signum() } else { steps };
            let next = (t.ratio + steps * step).clamp(t.tap.ratio_min, t.tap.ratio_max);
            if (next - t.ratio).abs() < 1e-12 {
                continue;
            }
            let dir = if next > t.ratio { 1 } else { -1 };
            if t.last != 0 && t.last != dir {
                t.reversals += 1;
            }
            t.last = dir;
            let branch = &case.branches[t.tap.branch];
            report(
                moved,
                format!(
                    "  tap on transformer {}-{}: ratio {:.4} -> {next:.4}, bus {} at {vm:.4} pu is outside {:.4}-{:.4}",
                    branch.from, branch.to, t.ratio, t.tap.regulated_bus, t.tap.vmin, t.tap.vmax
                ),
            );
            t.before = Some((vm, t.ratio, round));
            t.ratio = next;
            case.branches[t.tap.branch].tap = t.tap.tap_per_ratio * next;
            case.branches[t.tap.branch].correct_impedance();
            moved += 1;
        }
        moved
    }

    /// Fix the continuous shunt whose helper generator `g` reached reactive
    /// limit `q` at an end of its range: the full susceptance moves into its
    /// bus's `bs` and the helper stops injecting. Returns false when `g` is
    /// not a helper.
    pub fn fix_continuous(&mut self, case: &mut Case, bus: usize, g: usize, q: f64) -> bool {
        let sharing: Vec<(usize, f64, f64)> = (self.helpers.iter())
            .filter(|h| h.1 == g)
            .map(|h| (h.0, h.2, h.3))
            .collect();
        if sharing.is_empty() {
            return false;
        }
        let helper = &mut case.generators[g];
        let b = q / (helper.vg * helper.vg);
        self.parked.push((g, helper.qmin, helper.qmax));
        (helper.qg, helper.qmax, helper.qmin) = (0.0, 0.0, 0.0);
        case.buses[bus].bs += b;
        for (k, part) in split(&sharing, b) {
            self.fixed.push((k, part, g));
        }
        true
    }

    /// Undo [`Self::fix_continuous`]: the shunts at bus `bus` behind helper
    /// `g` hold the voltage again, from where they were fixed. Returns
    /// whether there was anything to undo.
    pub fn release_continuous(&mut self, case: &mut Case, bus: usize, g: usize) -> bool {
        let Some(p) = self.parked.iter().position(|p| p.0 == g) else {
            return false;
        };
        let (_, qmin, qmax) = self.parked.remove(p);
        let b: f64 = self.fixed.iter().filter(|f| f.2 == g).map(|f| f.1).sum();
        self.fixed.retain(|f| f.2 != g);
        case.buses[bus].bs -= b;
        let helper = &mut case.generators[g];
        (helper.qmin, helper.qmax) = (qmin, qmax);
        helper.qg = b * helper.vg * helper.vg;
        true
    }

    /// Whether `g` is a continuous-shunt helper now fixed at an end of its range.
    pub fn is_fixed(&self, g: usize) -> bool {
        self.parked.iter().any(|p| p.0 == g)
    }

    /// Go back to `earlier`, the state before the last pass, after the solve
    /// that pass led to failed, and hold every device that pass moved.
    /// Returns the number held.
    pub fn undo(&mut self, earlier: Self) -> usize {
        let moved_shunts: Vec<bool> = (self.shunts.iter().zip(&earlier.shunts))
            .map(|(now, then)| now.value != then.value)
            .collect();
        let moved_taps: Vec<bool> = (self.taps.iter().zip(&earlier.taps))
            .map(|(now, then)| now.ratio != then.ratio)
            .collect();
        *self = earlier;
        let mut held = 0;
        for (s, _) in self.shunts.iter_mut().zip(&moved_shunts).filter(|x| *x.1) {
            s.reversals = MAX_REVERSALS;
            held += 1;
        }
        for (t, _) in self.taps.iter_mut().zip(&moved_taps).filter(|x| *x.1) {
            t.ineffective = true;
            held += 1;
        }
        held
    }

    /// Final shunt values (MVAr at 1 p.u.) for every listed shunt, with
    /// continuous and frozen ones at their starting value, and final ratios
    /// for every listed tap control.
    pub fn values(&self, controls: Option<&VoltageControls>) -> (Vec<f64>, Vec<f64>) {
        let mut shunts = vec![f64::NAN; self.counts[0]];
        let mut taps = vec![f64::NAN; self.counts[1]];
        if let Some(c) = controls {
            for (k, s) in c.switched_shunts.iter().enumerate() {
                shunts[k] = s.initial;
            }
            for (k, t) in c.taps.iter().enumerate() {
                taps[k] = t.ratio;
            }
        }
        for s in &self.shunts {
            shunts[s.index] = s.value;
        }
        for t in &self.taps {
            taps[t.index] = t.ratio;
        }
        for &(k, b, _) in &self.fixed {
            shunts[k] += b;
        }
        (shunts, taps)
    }
}

/// Turn each continuous shunt's helper generator output into the shunt's
/// final susceptance, in `shunts` and in its bus's `bs`, once the helpers
/// have been removed from `case`.
pub(crate) fn finish(setup: &Setup<'_>, case: &mut Case, shunts: &mut [f64]) {
    let mut done = std::collections::HashSet::new();
    for &(_, shunt, g) in &setup.continuous {
        if !done.insert(g) {
            continue;
        }
        let q = case.generators[g].qg;
        let Some(bus) = case.buses.iter_mut().find(|b| b.id == shunt.bus) else {
            continue;
        };
        let b = q / (bus.vm * bus.vm);
        bus.bs += b;
        // The helper is gone, so its bus goes back to a load bus.
        if bus.kind == BusType::Pv {
            bus.kind = BusType::Pq;
        }
        let sharing: Vec<(usize, f64, f64)> = (setup.continuous.iter())
            .filter(|c| c.2 == g)
            .map(|&(k, s, _)| {
                let levels = s.levels();
                (
                    k,
                    s.initial - levels[0],
                    levels[levels.len() - 1] - s.initial,
                )
            })
            .collect();
        for (k, part) in split(&sharing, b) {
            if let Some(value) = shunts.get_mut(k) {
                *value += part;
            }
        }
    }
}

/// Share `b` (MVAr at 1 p.u., from the starting values) between the
/// continuous shunts `sharing` = (listed index, room below, room above) in
/// proportion to their room in that direction.
fn split(sharing: &[(usize, f64, f64)], b: f64) -> Vec<(usize, f64)> {
    let room = |s: &(usize, f64, f64)| if b >= 0.0 { s.2 } else { s.1 }.max(0.0);
    let total: f64 = sharing.iter().map(room).sum();
    sharing
        .iter()
        .map(|s| {
            let part = if total > 0.0 {
                b * room(s) / total
            } else {
                b / sharing.len() as f64
            };
            (s.0, part)
        })
        .collect()
}
