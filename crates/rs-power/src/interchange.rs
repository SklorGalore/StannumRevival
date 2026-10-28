//! Area interchange control: each area's slack generation moves until the
//! area's net export meets its desired value (PSS/E PDES within PTOL).
use crate::{Case, network::Network};
use num_complex::Complex64 as C;
use std::collections::HashMap;

/// A control area (a PSS/E area data record).
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    /// Area number (`I`); buses name it in [`crate::Bus::area`].
    pub number: u32,
    /// External ID of the area slack bus (`ISW`), whose generators move to
    /// hold the area's interchange. `None` leaves the area uncontrolled.
    pub slack_bus: Option<u32>,
    /// Desired net interchange in MW (`PDES`); positive is export.
    pub desired_mw: f64,
    /// Interchange tolerance in MW (`PTOL`).
    pub tolerance_mw: f64,
    /// Area name (`ARNAME`), trimmed; may be empty.
    pub name: String,
}

/// A load assigned to a different area than the bus it is on. Only these
/// loads matter to interchange, and only in
/// [`InterchangeMode::TieLinesAndLoads`]. Powers are in MW.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaLoad {
    /// External ID of the bus the load is on.
    pub bus: u32,
    /// The area the load belongs to.
    pub area: u32,
    /// Constant-power part in MW.
    pub p: f64,
    /// Constant-current part in MW at 1 p.u. voltage, scaling with |V|.
    pub p_i: f64,
    /// Constant-impedance part in MW at 1 p.u. voltage, scaling with |V|².
    pub p_z: f64,
}

impl AreaLoad {
    /// Active power drawn at voltage magnitude `vm` (p.u.), in MW.
    pub fn at(&self, vm: f64) -> f64 {
        self.p + self.p_i * vm + self.p_z * vm * vm
    }
}

/// What counts as an area's net interchange (PSS/E's area interchange
/// control option).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InterchangeMode {
    /// No control. The result still reports each area's tie-line
    /// interchange.
    #[default]
    Off,
    /// The flows on tie lines: AC branches and DC lines with ends in
    /// different areas.
    TieLines,
    /// Tie lines, plus loads on the area's buses that belong to another
    /// area (exports from the bus's area to the load's area).
    TieLinesAndLoads,
}

/// Area interchange control settings and data, for
/// [`crate::PowerFlowOptions::interchange`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AreaInterchange {
    /// Whether and how areas hold their interchange. Defaults to off.
    pub mode: InterchangeMode,
    /// The control areas. Areas named by buses but not listed here count
    /// as other parties to interchange but hold nothing.
    pub areas: Vec<Area>,
    /// Loads assigned to another area than their bus's, for
    /// [`InterchangeMode::TieLinesAndLoads`].
    pub loads: Vec<AreaLoad>,
}

/// One area's interchange after a solve.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaSolution {
    /// Area number.
    pub number: u32,
    /// Area name from [`Area::name`]; empty for areas not listed.
    pub name: String,
    /// Net interchange in MW, positive for export, by the definition the
    /// solve's [`InterchangeMode`] uses (tie lines alone when control is off).
    pub interchange_mw: f64,
    /// Desired interchange in MW; `None` for areas not listed.
    pub desired_mw: Option<f64>,
    /// Tolerance in MW; zero for areas not listed.
    pub tolerance_mw: f64,
    /// External ID of the area slack bus, if any.
    pub slack_bus: Option<u32>,
    /// Whether the solve held this area's interchange. False when control
    /// is off, and for areas without a usable slack bus or holding an
    /// island's reference bus, whose generation balances the island.
    pub controlled: bool,
    /// How far the solve moved the area slack generation, in MW.
    pub slack_change_mw: f64,
}

/// Tie lines are metered at their from end (PSS/E's default metered end for
/// branches and transformer winding 1), so the receiving area pays for
/// their losses. DC devices are metered at their sending converters.
pub(crate) struct Interchange<'a> {
    opt: &'a AreaInterchange,
    /// The listed areas, the first of each number.
    areas: Vec<&'a Area>,
    /// Area numbers: listed areas first, in order, then the others found
    /// on buses, ascending.
    numbers: Vec<u32>,
    index: HashMap<u32, usize>,
    /// Generators each listed area moves, by index into `case.generators`.
    slack_gens: Vec<Vec<usize>>,
    /// Internal index of each listed area's slack bus.
    slack_bus: Vec<Option<usize>>,
    /// How far the passes have moved each listed area's slack generation.
    moved_mw: Vec<f64>,
    /// The last pass's moves, to take back if the solve after it fails.
    last: Vec<(usize, f64)>,
    /// Fraction of each area's error the next pass moves: halved each time
    /// a solve fails after a pass.
    step: f64,
    passes: usize,
}

/// Times a failed solve after an interchange pass is undone and the pass
/// retried with half the step.
const MAX_RETREATS: usize = 4;

/// Interchange passes before area slack generation is held where it is.
pub(crate) const MAX_INTERCHANGE_PASSES: usize = 20;

impl<'a> Interchange<'a> {
    /// `movable(g)`: whether generator `g` is a real unit that may move
    /// (not a DC line terminal or continuous shunt helper).
    pub fn new(
        opt: &'a AreaInterchange,
        net: &Network,
        case: &Case,
        movable: impl Fn(usize) -> bool,
    ) -> Self {
        let mut numbers: Vec<u32> = Vec::new();
        let mut index = HashMap::new();
        let mut areas = Vec::new();
        for a in &opt.areas {
            if let std::collections::hash_map::Entry::Vacant(e) = index.entry(a.number) {
                e.insert(numbers.len());
                numbers.push(a.number);
                areas.push(a);
            }
        }
        let mut others: Vec<u32> = net
            .external
            .iter()
            .map(|&e| case.buses[e].area)
            .chain(opt.loads.iter().map(|l| l.area))
            .filter(|a| !index.contains_key(a))
            .collect();
        others.sort_unstable();
        others.dedup();
        for a in others {
            index.insert(a, numbers.len());
            numbers.push(a);
        }
        let mut slack_gens = Vec::new();
        let mut slack_bus = Vec::new();
        for a in &areas {
            let i = a.slack_bus.and_then(|id| net.by_id.get(&id).copied());
            let gens: Vec<usize> = i
                .map(|i| {
                    net.gens[i]
                        .iter()
                        .copied()
                        .filter(|&g| movable(g))
                        .collect()
                })
                .unwrap_or_default();
            slack_gens.push(gens);
            slack_bus.push(i);
        }
        let moved_mw = vec![0.0; areas.len()];
        Self {
            opt,
            areas,
            numbers,
            index,
            slack_gens,
            slack_bus,
            moved_mw,
            last: Vec::new(),
            step: 1.0,
            passes: 0,
        }
    }

    /// Whether any area is to hold its interchange.
    pub fn is_active(&self) -> bool {
        self.opt.mode != InterchangeMode::Off && self.slack_gens.iter().any(|g| !g.is_empty())
    }

    pub fn passes(&self) -> usize {
        self.passes
    }

    /// Net export of every area in MW, in `numbers` order. `dc` lists the
    /// DC devices, each as its AC ends (internal bus, MW injected).
    fn measure(&self, net: &Network, case: &Case, v: &[C], dc: &[Vec<(usize, f64)>]) -> Vec<f64> {
        let area = |i: usize| self.index[&case.buses[net.external[i]].area];
        let mut export = vec![0.0; self.numbers.len()];
        for e in &net.edges {
            let (a, b) = (area(e.f), area(e.t));
            if a != b {
                let p = (v[e.f] * (e.yff * v[e.f] + e.yft * v[e.t]).conj()).re * case.base_mva;
                export[a] += p;
                export[b] -= p;
            }
        }
        for ends in dc {
            if ends.iter().all(|&(i, _)| area(i) == area(ends[0].0)) {
                continue;
            }
            // Sending ends export what they draw; receiving ends import it
            // in proportion to what they deliver, so the DC losses fall on
            // the receiving areas.
            let sent: f64 = ends.iter().map(|&(_, p)| (-p).max(0.0)).sum();
            let received: f64 = ends.iter().map(|&(_, p)| p.max(0.0)).sum();
            for &(i, p) in ends {
                export[area(i)] += if p < 0.0 {
                    -p
                } else if received > 0.0 {
                    -p * sent / received
                } else {
                    0.0
                };
            }
        }
        if self.opt.mode == InterchangeMode::TieLinesAndLoads {
            for l in &self.opt.loads {
                let Some(&i) = net.by_id.get(&l.bus) else {
                    continue;
                };
                let (a, b) = (area(i), self.index[&l.area]);
                if a != b {
                    let p = l.at(v[i].norm());
                    export[a] += p;
                    export[b] -= p;
                }
            }
        }
        export
    }

    /// Whether listed area `k` can hold its interchange now: it has slack
    /// generators and no island reference bus (whose generation balances
    /// the island, so moving the area's own would change nothing).
    fn controllable(&self, k: usize, net: &Network, case: &Case, references: &[usize]) -> bool {
        !self.slack_gens[k].is_empty()
            && self.slack_bus[k].is_some_and(|i| !references.contains(&i))
            && !references
                .iter()
                .any(|&r| case.buses[net.external[r]].area == self.numbers[k])
    }

    /// Move the slack generation of every area outside its tolerance by its
    /// error. Returns the number of areas moved; zero once all are within
    /// tolerance or the pass limit is reached.
    #[allow(clippy::too_many_arguments)]
    pub fn pass(
        &mut self,
        net: &Network,
        case: &mut Case,
        references: &[usize],
        v: &[C],
        dc: &[Vec<(usize, f64)>],
        floor_mw: f64,
    ) -> usize {
        if !self.is_active() || self.passes >= MAX_INTERCHANGE_PASSES {
            return 0;
        }
        let export = self.measure(net, case, v, dc);
        let mut moves = Vec::new();
        for (k, a) in self.areas.iter().enumerate() {
            if !self.controllable(k, net, case, references) {
                continue;
            }
            let error = a.desired_mw - export[k];
            if error.abs() > a.tolerance_mw.max(floor_mw) {
                moves.push((k, export[k], error));
            }
        }
        if moves.is_empty() {
            return 0;
        }
        if !self.last.is_empty() {
            // The solve after the last pass converged: step up again.
            self.step = (self.step * 2.0).min(1.0);
        }
        self.passes += 1;
        self.last.clear();
        log::info!(
            "Area interchange pass {}: {} outside {} tolerance",
            self.passes,
            if moves.len() == 1 {
                "1 area".to_string()
            } else {
                format!("{} areas", moves.len())
            },
            if moves.len() == 1 { "its" } else { "their" }
        );
        for (n, &(k, actual, error)) in moves.iter().enumerate() {
            let a = &self.areas[k];
            let error = error * self.step;
            self.moved_mw[k] += error;
            self.last.push((k, error));
            let gens = &self.slack_gens[k];
            // Share the change in proportion to machine base, equally when
            // the bases are missing.
            let bases: Vec<f64> = gens
                .iter()
                .map(|&g| case.generators[g].mbase.max(0.0))
                .collect();
            let total: f64 = bases.iter().sum();
            for (&g, &base) in gens.iter().zip(&bases) {
                let share = if total > 0.0 {
                    base / total
                } else {
                    1.0 / gens.len() as f64
                };
                case.generators[g].pg += error * share;
            }
            let text = format!(
                "  area {}{}: interchange {:.1} MW, desired {:.1} MW; slack bus {} generation {:+.1} MW",
                a.number,
                if a.name.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", a.name)
                },
                actual,
                a.desired_mw,
                a.slack_bus.unwrap_or_default(),
                error
            );
            if n < crate::solver::DETAIL_LINES {
                log::info!("{text}");
            } else {
                log::debug!("{text}");
            }
        }
        if self.passes == MAX_INTERCHANGE_PASSES {
            log::warn!(
                "area interchange still outside tolerance after {MAX_INTERCHANGE_PASSES} passes; holding area slack generation"
            );
        }
        moves.len()
    }

    /// The solve after the last pass failed and the caller has restored the
    /// state before it: forget that pass's moves and halve the step.
    /// Returns false, leaving the moves, once there have been too many.
    pub fn retreat(&mut self) -> bool {
        if self.last.is_empty() || self.step < 0.5f64.powi(MAX_RETREATS as i32) {
            return false;
        }
        for &(k, error) in &self.last {
            self.moved_mw[k] -= error;
        }
        self.last.clear();
        self.step *= 0.5;
        true
    }

    /// Every area's interchange at voltages `v`, after the last pass.
    pub fn report(
        &self,
        net: &Network,
        case: &Case,
        references: &[usize],
        v: &[C],
        dc: &[Vec<(usize, f64)>],
    ) -> Vec<AreaSolution> {
        let export = self.measure(net, case, v, dc);
        let on = self.opt.mode != InterchangeMode::Off;
        self.numbers
            .iter()
            .enumerate()
            .map(|(k, &number)| {
                let listed = self.areas.get(k);
                AreaSolution {
                    number,
                    name: listed.map(|a| a.name.clone()).unwrap_or_default(),
                    interchange_mw: export[k],
                    desired_mw: listed.map(|a| a.desired_mw),
                    tolerance_mw: listed.map_or(0.0, |a| a.tolerance_mw),
                    slack_bus: listed.and_then(|a| a.slack_bus),
                    controlled: on
                        && listed.is_some()
                        && self.controllable(k, net, case, references),
                    slack_change_mw: self.moved_mw.get(k).copied().unwrap_or(0.0),
                }
            })
            .collect()
    }
}

/// The AC ends of the DC devices during a solve, each as (internal bus, MW
/// injected into the AC network): the generator pairs standing in for
/// simplified and VSC lines, then the detailed LCC and multi-terminal
/// terminals given as `(ends, MW)` lists.
pub(crate) fn dc_ends(
    net: &Network,
    case: &Case,
    pairs: &[[usize; 2]],
    detailed: impl Iterator<Item = Vec<(usize, f64)>>,
) -> Vec<Vec<(usize, f64)>> {
    let mut out: Vec<Vec<(usize, f64)>> = pairs
        .iter()
        .filter_map(|pair| {
            pair.iter()
                .map(|&g| {
                    let gen_data = &case.generators[g];
                    net.by_id.get(&gen_data.bus).map(|&i| (i, gen_data.pg))
                })
                .collect()
        })
        .collect();
    out.extend(detailed);
    out
}
