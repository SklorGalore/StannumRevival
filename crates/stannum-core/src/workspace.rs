//! The open case shared by every Stannum module.
//!
//! Opening a file parses it once per engine: rs-power for power flow,
//! nodal for topology. Either engine may decline a file (for example nodal
//! only reads RAW revision 35); the other modules keep working and the
//! frontend shows why a module is unavailable.

use crate::contingency::{
    ContingencyDetail, ContingencyJob, ContingencyResults, ContingencyRow, ContingencySettings,
    ContingencySummary,
};
use crate::dc_grids::{self, DcGridsSummary, LoadedDcGrids};
use crate::diff::{self, Comparison};
use crate::power::{self, PowerCase, SolveSettings};
use crate::records::{RecordBook, RecordTable, Records};
use crate::study::{self, Edit};
use nodal::topology::{Analysis, GraphData, Path as TopologyPath, Topology, TopologyOptions};
use rs_power::{PowerFlowResult, PowerFlowSensitivity};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaseFormat {
    Matpower,
    Raw,
}

pub struct TopologyState {
    pub case: nodal::model::Case,
    /// Built from the power flow model because nodal can't read the file.
    pub derived: bool,
    pub topology: Topology,
    pub merge_zero_impedance: bool,
}

pub struct OpenCase {
    pub path: PathBuf,
    pub name: String,
    pub format: CaseFormat,
    pub power: Result<PowerCase, String>,
    pub topology: Result<TopologyState, String>,
    pub solved: Option<(PowerFlowResult, SolveSettings)>,
    /// The last solve from before the study's edits changed, kept as a
    /// starting point for the next solve.
    pub before_edits: Option<(PowerFlowResult, SolveSettings)>,
    pub sensitivity: Option<(PowerFlowSensitivity, power::ModelChoice)>,
    /// The study's edits, and the case with them applied (None when there are none).
    pub edits: Vec<Edit>,
    pub edited: Option<PowerCase>,
    /// VSC DC grids from a DC grid file, solved with every AC solve.
    pub dc_grids: Option<LoadedDcGrids>,
    /// The last contingency run, until the study's edits change.
    pub contingencies: Option<ContingencyResults>,
    /// The file's records as tables, read the first time they're asked for.
    pub records: Option<Result<Records, String>>,
}

#[derive(Default)]
pub struct Workspace {
    pub case: Option<OpenCase>,
    pub comparison: Option<Comparison>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ModuleStatus<T: Serialize> {
    pub available: bool,
    pub error: Option<String>,
    pub info: Option<T>,
}

impl<T: Serialize> ModuleStatus<T> {
    fn from<S>(r: &Result<S, String>, info: impl FnOnce(&S) -> T) -> Self {
        match r {
            Ok(s) => Self {
                available: true,
                error: None,
                info: Some(info(s)),
            },
            Err(e) => Self {
                available: false,
                error: Some(e.clone()),
                info: None,
            },
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct PowerInfo {
    pub counts: power::CaseCounts,
    pub warnings: Vec<power::Warning>,
}

#[derive(Serialize, Debug, Clone)]
pub struct TopologyInfo {
    /// RAW revision nodal read, or 0 when built from the power flow model.
    pub revision: i32,
    /// Built from the power flow model (MATPOWER, RAW other than revision 35).
    pub derived: bool,
    pub buses: usize,
    pub equipment: usize,
    pub titles: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct CaseSummary {
    pub path: String,
    pub name: String,
    pub format: CaseFormat,
    pub power: ModuleStatus<PowerInfo>,
    pub topology: ModuleStatus<TopologyInfo>,
    /// RAW files can be used as a comparison baseline.
    pub diffable: bool,
    /// The attached DC grid file, if any.
    pub dc_grids: Option<DcGridsSummary>,
}

#[derive(Serialize)]
pub struct TopologySnapshot<'a> {
    pub case: &'a nodal::model::Case,
    pub analysis: Analysis,
    pub graph: GraphData,
}

#[derive(Serialize)]
pub struct TopologyView {
    pub analysis: Analysis,
    pub graph: GraphData,
}

/// UTF-8 first, then Windows-1252 (the encoding PSS/E exports use).
pub fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

/// A solve to start the next one from: converged and AC.
fn resumable(solve: &Option<(PowerFlowResult, SolveSettings)>) -> Option<&PowerFlowResult> {
    solve
        .as_ref()
        .filter(|(r, settings)| {
            settings.model == power::ModelChoice::Ac && r.status == rs_power::SolveStatus::Converged
        })
        .map(|(r, _)| r)
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The solved case in a few lines: outcome, totals, limits and controls.
fn log_report(report: &power::SolveReport, solved_ms: f64) {
    let outcome = format!(
        "Power flow: {} after {} iterations, largest mismatch {:.1e} MVA ({:.0} ms)",
        report.status, report.iterations, report.residual_mva, solved_ms
    );
    // rs-power has already reported a converged solve; say so again only
    // when it needs attention.
    if !report.converged {
        log::warn!("{outcome}");
    }
    log::info!(
        "Totals: generation {:.1} MW, load {:.1} MW, losses {:.1} MW",
        report.total_generation_mw,
        report.total_load_mw,
        report.total_losses_mw
    );
    let l = &report.limits;
    let mut found = Vec::new();
    if let Some((bus, by)) = l.worst_voltage {
        found.push(format!(
            "{} outside voltage limits (worst bus {bus}, {by:.4} pu outside)",
            plural(l.voltage_violations, "bus", "buses")
        ));
    }
    if let Some((bus, by)) = l.worst_reactive {
        found.push(format!(
            "{} outside Q limits (worst at bus {bus}, {by:.1} Mvar outside)",
            plural(l.reactive_violations, "generator", "generators")
        ));
    }
    if l.branch_overloads > 0
        && let Some((from, to, pct)) = l.worst_branch
    {
        found.push(format!(
            "{} over rate A (worst {from}-{to} at {pct:.1}%)",
            plural(l.branch_overloads, "branch", "branches")
        ));
    }
    if found.is_empty() {
        log::info!("Operating limits: everything within limits");
    } else {
        log::warn!("Operating limits: {}", found.join("; "));
    }
    if report.shunts_switched + report.taps_moved > 0 {
        log::info!(
            "Voltage controls: {} and {} ended away from their starting settings",
            plural(report.shunts_switched, "switched shunt", "switched shunts"),
            plural(report.taps_moved, "tap", "taps")
        );
    }
}

impl OpenCase {
    pub fn from_bytes(path: &Path, bytes: &[u8]) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let format = match path.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("m") => CaseFormat::Matpower,
            _ => CaseFormat::Raw,
        };
        let text = decode_text(bytes);
        let start = Instant::now();
        let power = power::load(&text, format == CaseFormat::Matpower);
        match &power {
            Ok(p) => {
                let c = power::counts(&p.case);
                log::info!(
                    "Read {name} for power flow: {} buses, {} generators, {} branches ({:.0} ms)",
                    c.buses,
                    c.generators,
                    c.branches,
                    elapsed_ms(start)
                );
                if !p.warnings.is_empty() {
                    log::warn!("{}", warnings_summary(&p.warnings, &name));
                }
            }
            Err(e) => log::warn!("Power flow can't use {name}: {e}"),
        }
        let start = Instant::now();
        let from_nodal = match format {
            CaseFormat::Matpower => {
                Err("Topology analysis reads PSS/E RAW revision 35 files.".to_string())
            }
            CaseFormat::Raw => nodal::parse_raw(bytes).map_err(|d| d.to_string()),
        };
        // nodal reads RAW revision 35 with every record; anything else that
        // power flow can read gets a topology built from its model.
        let topology = match (from_nodal, &power) {
            (Ok(case), _) => Ok((case, false)),
            (Err(_), Ok(p)) => Ok((crate::derived_topology::from_power(p), true)),
            (Err(e), Err(_)) => Err(e),
        }
        .map(|(case, derived)| TopologyState {
            topology: Topology::new(&case),
            case,
            derived,
            merge_zero_impedance: false,
        });
        match &topology {
            Ok(t) => log::info!(
                "Built topology{}: {} buses, {} equipment ({:.0} ms)",
                if t.derived {
                    " from the power flow model"
                } else {
                    ""
                },
                t.case.buses.len(),
                t.case.equipment.len(),
                elapsed_ms(start)
            ),
            Err(e) => log::warn!("Topology can't use {name}: {e}"),
        }
        Self {
            path: path.to_path_buf(),
            name,
            format,
            power,
            topology,
            solved: None,
            before_edits: None,
            sensitivity: None,
            edits: vec![],
            edited: None,
            dc_grids: None,
            contingencies: None,
            records: None,
        }
    }

    pub fn summary(&self) -> CaseSummary {
        CaseSummary {
            path: self.path.display().to_string(),
            name: self.name.clone(),
            format: self.format,
            power: ModuleStatus::from(&self.power, |p| PowerInfo {
                counts: power::counts(&p.case),
                warnings: p.warnings.clone(),
            }),
            topology: ModuleStatus::from(&self.topology, |t| TopologyInfo {
                revision: t.case.revision,
                derived: t.derived,
                buses: t.case.buses.len(),
                equipment: t.case.equipment.len(),
                titles: t.case.titles.clone(),
            }),
            diffable: self.format == CaseFormat::Raw,
            dc_grids: self.dc_grids.as_ref().map(LoadedDcGrids::summary),
        }
    }

    /// The case power flow works on: the file with the study's edits applied.
    fn power_case(&self) -> Result<&PowerCase, String> {
        let base = self.power.as_ref().map_err(|e| e.clone())?;
        Ok(self.edited.as_ref().unwrap_or(base))
    }

    /// Replace the study's edits. Results from before the change are dropped,
    /// since they describe a different case. On a bad edit nothing changes.
    pub fn set_edits(&mut self, edits: Vec<Edit>) -> Result<(), String> {
        let base = self.power.as_ref().map_err(|e| e.clone())?;
        let edited = if edits.is_empty() {
            None
        } else {
            Some(study::apply(base, &edits)?)
        };
        match edits.len() {
            0 => log::info!("Study cleared: solves use the case as filed"),
            n => log::info!(
                "Study: {n} edit{} applied to the case",
                if n == 1 { "" } else { "s" }
            ),
        }
        self.edits = edits;
        self.edited = edited;
        if let Some(solved) = self.solved.take() {
            self.before_edits = Some(solved);
        }
        self.sensitivity = None;
        if self.contingencies.take().is_some() {
            log::info!("Contingency results cleared: they were for the case before this change");
        }
        Ok(())
    }

    /// A contingency run over the case with the study's edits, from its last
    /// converged AC solve. Run it with [`ContingencyJob::run`] (it doesn't
    /// need the workspace) and keep the results with
    /// [`Self::set_contingencies`].
    pub fn contingency_job(&self, settings: ContingencySettings) -> Result<ContingencyJob, String> {
        let power = self.power_case()?;
        let base = resumable(&self.solved).ok_or(
            "Solve the case with the AC model first: contingencies start from that solution.",
        )?;
        if self.dc_grids.is_some() {
            log::warn!("Contingencies leave out the attached VSC DC grids");
        }
        let job = ContingencyJob::new(power, base, settings)?;
        log::info!(
            "Running {} contingenc{}",
            job.len(),
            if job.len() == 1 { "y" } else { "ies" }
        );
        Ok(job)
    }

    /// Keep a finished run and return its summary.
    pub fn set_contingencies(&mut self, results: ContingencyResults) -> ContingencySummary {
        let summary = results.summary.clone();
        self.contingencies = Some(results);
        summary
    }

    /// Run contingencies here and keep the results (for callers without a
    /// separate worker thread).
    pub fn run_contingencies(
        &mut self,
        settings: ContingencySettings,
    ) -> Result<ContingencySummary, String> {
        let results = self.contingency_job(settings)?.run()?;
        Ok(self.set_contingencies(results))
    }

    fn contingency_results(&self) -> Result<&ContingencyResults, String> {
        self.contingencies
            .as_ref()
            .ok_or_else(|| "Run contingencies first.".into())
    }

    pub fn contingency_summary(&self) -> Option<ContingencySummary> {
        self.contingencies.as_ref().map(|c| c.summary.clone())
    }

    pub fn contingency_rows(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ContingencyRow>, String> {
        Ok(self.contingency_results()?.page(offset, limit))
    }

    /// Take one branch or generator out of the last AC solve and solve again
    /// in AC with the same settings: "Trip and compare". The contingency
    /// run in hand, if any, is kept.
    pub fn trip_and_compare(
        &self,
        element: &str,
        limit_pct: f64,
    ) -> Result<ContingencyDetail, String> {
        let power = self.power_case()?;
        let (base, solve) = self
            .solved
            .as_ref()
            .filter(|(r, s)| {
                s.model == power::ModelChoice::Ac && r.status == rs_power::SolveStatus::Converged
            })
            .ok_or("Solve the case with the AC model first: the trip starts from that solution.")?;
        let settings = ContingencySettings {
            elements: Some(vec![element.to_string()]),
            branches: true,
            generators: true,
            method: crate::contingency::Method::Ac,
            ac_count: 1,
            limit_pct,
            solve: solve.clone(),
        };
        let job = ContingencyJob::new(power, base, settings).map_err(|e| {
            if e.starts_with("Nothing to take out") {
                format!("{element} is already out of service.")
            } else {
                e
            }
        })?;
        let mut results = job.run()?;
        let detail = results.detail(1)?;
        log::info!(
            "Tripped {element}: {} overload{} new or worse",
            detail.overloads.len(),
            if detail.overloads.len() == 1 { "" } else { "s" }
        );
        Ok(detail)
    }

    pub fn contingency_detail(&mut self, rank: usize) -> Result<ContingencyDetail, String> {
        self.contingencies
            .as_mut()
            .ok_or("Run contingencies first.")?
            .detail(rank)
    }

    pub fn solve(&mut self, settings: SolveSettings) -> Result<power::SolveReport, String> {
        let power = self.power_case()?;
        let start = Instant::now();
        let cold = settings.options_for(power);
        let mut options = cold.clone();
        // The last converged AC solve: of this case, else from before the
        // study's edits changed.
        let last = resumable(&self.solved)
            .map(|r| (r, true))
            .or_else(|| resumable(&self.before_edits).map(|r| (r, false)));
        let warm = match (last, settings.start) {
            (Some((last, same_case)), power::StartChoice::Last)
                if settings.model == power::ModelChoice::Ac =>
            {
                if !same_case {
                    log::info!("Starting from the last solve, from before the study changed");
                }
                Some(power::warm_start(&power.case, last, &mut options))
            }
            (None, power::StartChoice::Last) if settings.model == power::ModelChoice::Ac => {
                log::info!(
                    "No converged AC solve to start from yet; starting from the case voltages"
                );
                None
            }
            _ => None,
        };
        let input = warm.as_ref().unwrap_or(&power.case);
        let grids = self
            .dc_grids
            .as_ref()
            .filter(|_| settings.model == power::ModelChoice::Ac);
        if self.dc_grids.is_some() && grids.is_none() {
            log::warn!("VSC DC grids need the AC power flow; this DC solve leaves them out");
        }
        let (result, vsc) = match grids {
            Some(file) => {
                let acdc = rs_power::solve_dc_grids(
                    input,
                    &options,
                    &file.grids,
                    &rs_power::DcGridSettings::default(),
                )
                .map_err(|e| study::explain(e.to_string(), &self.edits))?;
                let names = power::bus_names(&power.case);
                let index: std::collections::HashMap<u32, usize> = power
                    .case
                    .buses
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (b.id, i))
                    .collect();
                let vsc = dc_grids::report(file, &acdc, |bus| {
                    index
                        .get(&bus)
                        .and_then(|&i| names.get(i))
                        .cloned()
                        .unwrap_or_default()
                });
                let mut ac = acdc.ac;
                // The AC/DC iteration's outcome, not only the last AC solve's.
                ac.status = acdc.status;
                ac.iterations = acdc.ac_iterations;
                (ac, Some(vsc))
            }
            None => (
                rs_power::solve(input, &options)
                    .map_err(|e| study::explain(e.to_string(), &self.edits))?,
                None,
            ),
        };
        let solved_ms = elapsed_ms(start);
        let mut report = power::report(
            &result,
            &power.elements,
            &power.dc_line_kinds,
            // Count moves from the file's settings, not from the last solve.
            &cold.voltage_controls,
            &settings,
            elapsed_ms(start),
        );
        if let Some(vsc) = vsc {
            let lost: f64 = vsc
                .grids
                .iter()
                .map(|g| g.line_loss_mw + g.converter_loss_mw)
                .sum();
            report.total_losses_mw += lost;
            log::info!(
                "VSC DC grids: {} after {} AC/DC iterations; {:.1} MW lost in converters and DC lines",
                if vsc.converged {
                    "converged"
                } else {
                    "not converged"
                },
                vsc.iterations,
                lost
            );
            report.vsc_grids = Some(vsc);
        }
        self.solved = Some((result, settings));
        self.before_edits = None;
        log_report(&report, solved_ms);
        Ok(report)
    }

    /// Attach a DC grid file; the next AC solve includes its grids. The file
    /// is checked against the case first, and a bad file changes nothing.
    pub fn load_dc_grids(&mut self, path: &Path) -> Result<DcGridsSummary, String> {
        let loaded = LoadedDcGrids::read(path)?;
        rs_power::check_dc_grids(&self.power_case()?.case, &loaded.grids)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let summary = loaded.summary();
        log::info!(
            "Attached {} with {} DC grid{} ({} converters); solve to include them",
            summary.name,
            summary.grids.len(),
            if summary.grids.len() == 1 { "" } else { "s" },
            summary.grids.iter().map(|g| g.converters).sum::<usize>()
        );
        self.dc_grids = Some(loaded);
        Ok(summary)
    }

    pub fn clear_dc_grids(&mut self) {
        if let Some(g) = self.dc_grids.take() {
            log::info!("Detached {}", g.path.display());
        }
    }

    pub fn sensitivities(
        &mut self,
        settings: SolveSettings,
    ) -> Result<power::SensitivitySummary, String> {
        let power = self.power_case()?;
        let start = Instant::now();
        let s = rs_power::calculate_sensitivities(&power.case, &settings.options_for(power))
            .map_err(|e| e.to_string())?;
        let summary = power::sensitivity_summary(&s, settings.model, elapsed_ms(start));
        log::info!(
            "Sensitivities: {} injection columns, {} branches ({:.0} ms)",
            summary.injections.len(),
            summary.branches,
            summary.elapsed_ms
        );
        self.sensitivity = Some((s, settings.model));
        Ok(summary)
    }

    pub fn responses(
        &self,
        column: usize,
        quantity: &str,
        engineering: bool,
    ) -> Result<power::ResponseTable, String> {
        let (s, _) = self
            .sensitivity
            .as_ref()
            .ok_or("Calculate sensitivities first.")?;
        let q = power::Quantity::parse(quantity).ok_or("Unknown quantity")?;
        power::responses(s, &self.power_case()?.elements, column, q, engineering)
    }

    /// Write the input case, or the latest solved case, as MATPOWER.
    pub fn export_matpower(&self, path: &Path, solved: bool) -> Result<(), String> {
        let case = if solved {
            &self
                .solved
                .as_ref()
                .ok_or("Solve the case before exporting results.")?
                .0
                .case
        } else {
            &self.power_case()?.case
        };
        rs_power::save_matpower(path, case).map_err(|e| e.to_string())?;
        log::info!(
            "Exported the {} case to {}",
            if solved { "solved" } else { "input" },
            path.display()
        );
        Ok(())
    }

    /// The case as buses, generators and branches for the one-line diagram.
    pub fn diagram_network(&self) -> Result<crate::diagram::CaseNetwork, String> {
        Ok(crate::diagram::network(self.power_case()?))
    }

    /// One element's data and limits from the file, for the element inspector.
    pub fn element_detail(&self, element: &str) -> Result<crate::inspect::ElementDetail, String> {
        let power = self.power.as_ref().map_err(|e| e.clone())?;
        let topology = self
            .topology
            .as_ref()
            .ok()
            .filter(|t| !t.derived)
            .map(|t| &t.case);
        crate::inspect::element(power, topology, element)
    }

    /// The file's records as tables: RAW revisions 33 and 35 record by
    /// record, anything else as the power flow model.
    pub fn records(&mut self) -> Result<&Records, String> {
        if self.records.is_none() {
            let start = Instant::now();
            let raw = match self.format {
                CaseFormat::Raw => Some(
                    std::fs::read(&self.path)
                        .map_err(|e| format!("{}: {e}", self.path.display()))
                        .and_then(|bytes| {
                            crate::diff::parser::parse_bytes_for_viewing(&self.name, &bytes)
                                .map_err(|e| e.to_string())
                        }),
                ),
                CaseFormat::Matpower => None,
            };
            let built = match (raw, &self.power) {
                (Some(Ok(case)), _) => Ok(crate::records::from_raw(&case)),
                (raw, Ok(power)) => {
                    if let Some(Err(e)) = &raw {
                        log::info!(
                            "Showing {}'s records from the power flow model: {e}",
                            self.name
                        );
                    }
                    Ok(crate::records::from_model(power, raw.is_some()))
                }
                (Some(Err(e)), Err(_)) => Err(e),
                (None, Err(e)) => Err(e.clone()),
            };
            if let Ok(r) = &built {
                let count: usize = r.tables.iter().map(|t| t.rows.len()).sum();
                log::info!(
                    "Read {count} records in {} tables ({:.0} ms)",
                    r.tables.len(),
                    elapsed_ms(start)
                );
            }
            self.records = Some(built);
        }
        match self.records.as_ref() {
            Some(Ok(r)) => Ok(r),
            Some(Err(e)) => Err(e.clone()),
            None => unreachable!(),
        }
    }

    pub fn record_book(&mut self) -> Result<RecordBook, String> {
        Ok(self.records()?.book())
    }

    pub fn record_table(&mut self, section: &str) -> Result<RecordTable, String> {
        Ok(self.records()?.table(section)?.clone())
    }

    fn topology_state(&self) -> Result<&TopologyState, String> {
        self.topology.as_ref().map_err(|e| e.clone())
    }

    pub fn topology_snapshot(&self) -> Result<TopologySnapshot<'_>, String> {
        let t = self.topology_state()?;
        Ok(TopologySnapshot {
            case: &t.case,
            analysis: t.topology.analyze(&t.case),
            graph: t.topology.graph_data(),
        })
    }

    pub fn set_merge_zero_impedance(&mut self, enabled: bool) -> Result<TopologyView, String> {
        let t = self.topology.as_mut().map_err(|e| e.clone())?;
        let start = Instant::now();
        t.topology = Topology::with_options(
            &t.case,
            TopologyOptions {
                merge_zero_impedance: enabled,
            },
        );
        t.merge_zero_impedance = enabled;
        log::info!(
            "Rebuilt topology {} zero-impedance merging ({:.0} ms)",
            if enabled { "with" } else { "without" },
            elapsed_ms(start)
        );
        Ok(TopologyView {
            analysis: t.topology.analyze(&t.case),
            graph: t.topology.graph_data(),
        })
    }

    pub fn shortest_path(&self, from: i32, to: i32) -> Result<Option<TopologyPath>, String> {
        let t = self.topology_state()?;
        t.topology
            .shortest_path(&t.case, from, to)
            .map_err(|d| d.to_string())
    }
}

impl Workspace {
    pub fn open(&mut self, path: &Path) -> Result<CaseSummary, String> {
        log::info!("Opening {}", path.display());
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let case = OpenCase::from_bytes(path, &bytes);
        if case.power.is_err() && case.topology.is_err() && case.format == CaseFormat::Matpower {
            return Err(case.power.err().unwrap_or_default());
        }
        let summary = case.summary();
        self.case = Some(case);
        Ok(summary)
    }

    pub fn close(&mut self) {
        self.case = None;
    }

    pub fn current(&self) -> Result<&OpenCase, String> {
        self.case
            .as_ref()
            .ok_or_else(|| "Open a case first.".into())
    }

    pub fn current_mut(&mut self) -> Result<&mut OpenCase, String> {
        self.case
            .as_mut()
            .ok_or_else(|| "Open a case first.".into())
    }

    pub fn compare(
        &mut self,
        baseline: &Path,
        comparison: &Path,
    ) -> Result<diff::DiffSummary, String> {
        // Clear first so a failure never leaves stale results on screen.
        self.comparison = None;
        let start = Instant::now();
        log::info!(
            "Comparing {} with {}",
            baseline.display(),
            comparison.display()
        );
        let before = diff::parse_path(baseline).map_err(|e| e.to_string())?;
        let after = diff::parse_path(comparison).map_err(|e| e.to_string())?;
        let result = diff::compare_cases(before, after);
        let summary = result.summary();
        log::info!(
            "Case diff: {} differing records{} ({:.0} ms)",
            summary.differing,
            if summary.complete {
                ""
            } else {
                ", comparison incomplete"
            },
            elapsed_ms(start)
        );
        self.comparison = Some(result);
        Ok(summary)
    }

    pub fn comparison(&self) -> Result<&Comparison, String> {
        self.comparison
            .as_ref()
            .ok_or_else(|| "Compare two files first.".into())
    }
}

/// One log line for a case's conversion warnings: how many, and of how many
/// kinds, naming the most common.
fn warnings_summary(warnings: &[power::Warning], name: &str) -> String {
    let mut kinds: Vec<(&str, usize)> = Vec::new();
    for w in warnings {
        match kinds.iter_mut().find(|(m, _)| *m == w.message) {
            Some((_, n)) => *n += 1,
            None => kinds.push((&w.message, 1)),
        }
    }
    kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
    let total = warnings.len();
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            one.to_string()
        } else {
            many.to_string()
        }
    };
    let (top, n) = kinds[0];
    let what = format!(
        "{total} conversion {}",
        plural(total, "warning", "warnings")
    );
    let line = if kinds.len() == 1 {
        format!("{what} while reading {name}, all \"{top}\"")
    } else {
        format!(
            "{what} of {} kinds while reading {name}, most often \"{top}\" ({n})",
            kinds.len()
        )
    };
    line + "; see Power flow for the list"
}

#[cfg(test)]
mod warnings_summary_tests {
    use super::*;

    fn w(message: &str) -> power::Warning {
        power::Warning {
            line: 1,
            message: message.into(),
            element: None,
        }
    }

    #[test]
    fn names_the_most_common_kind() {
        let list = [w("a"), w("b"), w("b")];
        assert_eq!(
            warnings_summary(&list, "x.raw"),
            "3 conversion warnings of 2 kinds while reading x.raw, most often \"b\" (2); see Power flow for the list"
        );
        assert_eq!(
            warnings_summary(&[w("a")], "x.raw"),
            "1 conversion warning while reading x.raw, all \"a\"; see Power flow for the list"
        );
    }
}
