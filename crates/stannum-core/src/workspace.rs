//! The open case shared by every Stannum module.
//!
//! Opening a file parses it once per engine: rs-power for power flow,
//! nodal for topology. Either engine may decline a file (for example nodal
//! only reads RAW revision 35); the other modules keep working and the
//! frontend shows why a module is unavailable.

use crate::contingency::{
    ContingencyDetail, ContingencyJob, ContingencyResults, ContingencyRow, ContingencySettings,
    ContingencySummary, Rating,
};
use crate::dc_grids::{self, DcGridsSummary, LoadedDcGrids};
use crate::diff::{self, Comparison};
use crate::error::{Error, ErrorKind};
use crate::geo::{BusCoordinates, CoordinatesSummary, LoadedCoordinates};
use crate::power::{self, PowerCase, SolveSettings};
use crate::record_edit::{self, RecordEdit};
use crate::records::{RecordBook, RecordSource, RecordTable, Records};
use crate::results::ResultName;
use crate::study::{self, Edit};
use crate::topology_wire;
use nodal::topology::{Analysis, GraphData, Path as TopologyPath, Topology, TopologyOptions};
use rs_power::{PowerFlowResult, SensitivityModel};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CaseFormat {
    Matpower,
    Raw,
    /// A Stannum case file (.stn text or .stnb binary), read as the RAW 35
    /// text it converts to.
    Stn,
}

impl CaseFormat {
    /// Whether the case's bytes are RAW text: RAW files, and Stannum case
    /// files once converted.
    pub fn is_raw(self) -> bool {
        matches!(self, CaseFormat::Raw | CaseFormat::Stn)
    }
}

pub struct TopologyState {
    pub case: nodal::model::Case,
    /// Built from the power flow model because nodal can't read the file.
    pub derived: bool,
    pub topology: Topology,
    pub merge_zero_impedance: bool,
}

/// Names one open case in commands and scripts. Ids are never reused within
/// a workspace, so a stale id is refused instead of reaching another case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct CaseId(pub u64);

impl std::fmt::Display for CaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A case's text: MATPOWER or RAW as read or as edited, or for a Stannum
/// case, RAW revision 35 written from it the first time something needs it
/// (topology, Records, case diff, saving). Opening a Stannum case reads it
/// straight into the power flow model, without the text. Cheap to clone.
#[derive(Clone)]
pub struct CaseText(Arc<CaseTextInner>);

struct CaseTextInner {
    text: OnceLock<Vec<u8>>,
    /// The Stannum case and its file name, to write the text from.
    source: Option<(Arc<stannum_format::Document>, String)>,
}

impl CaseText {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Arc::new(CaseTextInner {
            text: OnceLock::from(bytes),
            source: None,
        }))
    }

    fn from_stn(doc: Arc<stannum_format::Document>, name: String) -> Self {
        Self(Arc::new(CaseTextInner {
            text: OnceLock::new(),
            source: Some((doc, name)),
        }))
    }

    pub fn get(&self) -> &[u8] {
        self.0.text.get_or_init(|| {
            let Some((doc, name)) = &self.0.source else {
                return Vec::new();
            };
            let start = Instant::now();
            match crate::stn::to_raw(doc, name) {
                Ok(text) => {
                    log::debug!("Wrote {name} as RAW text ({:.0} ms)", elapsed_ms(start));
                    text.into_bytes()
                }
                Err(e) => {
                    log::warn!("{name} can't be written as RAW text: {e}");
                    Vec::new()
                }
            }
        })
    }

    /// Write the text now, on a thread of its own, so whatever needs it
    /// first doesn't wait as long.
    fn prepare(&self) {
        if self.0.text.get().is_none() {
            let text = self.clone();
            std::thread::spawn(move || {
                text.get();
            });
        }
    }
}

impl std::ops::Deref for CaseText {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.get()
    }
}

pub struct OpenCase {
    /// Set by [`Workspace::open`]; kept when the case is rebuilt in place.
    pub id: CaseId,
    pub path: PathBuf,
    pub name: String,
    pub format: CaseFormat,
    /// Changes whenever the case itself changes (opened, study edits,
    /// Records edits), so work that ran without the workspace lock can tell
    /// whether its results still describe this case.
    pub version: u64,
    pub power: Result<Arc<PowerCase>, String>,
    /// Built the first time a command needs it (see [`Self::topology`]):
    /// opening a large case doesn't wait for a graph nobody has asked for.
    topology: OnceLock<Result<TopologyState, String>>,
    pub solved: Option<Arc<Solved>>,
    /// The last solve from before the study's edits changed, kept as a
    /// starting point for the next solve.
    pub before_edits: Option<Arc<Solved>>,
    pub sensitivity: Option<(SensitivityModel, power::ModelChoice)>,
    /// The study's edits, and the case with them applied (None when there are none).
    pub edits: Vec<Edit>,
    pub edited: Option<Arc<PowerCase>>,
    /// VSC DC grids from a DC grid file, solved with every AC solve.
    pub dc_grids: Option<Arc<LoadedDcGrids>>,
    /// Bus coordinates from a CSV side file, for the map.
    pub coordinates: Option<LoadedCoordinates>,
    /// Where the bus places come from (the side file, the case's own
    /// substation data, or both) and what they covered when last matched to
    /// the case; None when nothing places any bus.
    pub places: Option<CoordinatesSummary>,
    /// The last contingency run, until the study's edits change.
    pub contingencies: Option<ContingencyResults>,
    /// The report of the latest solve (named `base` or `study`, see
    /// [`crate::results`]), until the study's edits change.
    pub report: Option<Arc<power::SolveReport>>,
    /// The last "Trip and compare", kept so its whole solution can be shown.
    pub trip: Option<(String, ContingencyResults)>,
    /// The file's records as tables, read the first time they're asked for.
    pub records: Option<Result<Arc<Records>, String>>,
    /// The case's text as opened, or as edited in Records since.
    pub bytes: CaseText,
    /// Cells edited in Records since the file was opened or saved.
    pub record_edits: usize,
    /// A Stannum case's file as read (or last saved): what only it holds,
    /// such as comments and ids, is carried onto the .stn files written
    /// from the case's RAW form.
    pub stn_source: Option<Arc<stannum_format::Document>>,
}

#[derive(Default)]
pub struct Workspace {
    pub case: Option<OpenCase>,
    pub comparison: Option<Comparison>,
    /// The id the last opened case got.
    last_id: u64,
}

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[schemars(rename = "ModuleStatusOf{T}")]
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

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct PowerInfo {
    pub counts: power::CaseCounts,
    /// The RAW revision the file was read as; null for MATPOWER.
    pub revision: Option<u32>,
    pub warnings: Vec<power::Warning>,
}

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct TopologyInfo {
    /// RAW revision nodal read, or 0 when built from the power flow model.
    pub revision: i32,
    /// Built from the power flow model (MATPOWER, RAW other than revision 35).
    pub derived: bool,
    pub buses: usize,
    pub equipment: usize,
    pub titles: Vec<String>,
}

#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct CaseSummary {
    /// The handle commands take to name this case.
    pub id: CaseId,
    pub path: String,
    pub name: String,
    pub format: CaseFormat,
    pub power: ModuleStatus<PowerInfo>,
    pub topology: ModuleStatus<TopologyInfo>,
    /// RAW files can be used as a comparison baseline.
    pub diffable: bool,
    /// Cells edited in Records and not yet saved to a file.
    pub record_edits: usize,
    /// The attached DC grid file, if any.
    pub dc_grids: Option<DcGridsSummary>,
    /// Where the bus places for the map come from: the attached coordinates
    /// file, the case's own substation data, or both. Null when neither
    /// places any bus.
    pub coordinates: Option<CoordinatesSummary>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TopologySnapshot<'a> {
    /// The case as nodal reads it.
    pub case: &'a nodal::model::Case,
    pub analysis: Analysis,
    pub graph: GraphData,
}

#[derive(Serialize, schemars::JsonSchema)]
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

/// A solve's result and the settings it ran with.
pub type Solved = (PowerFlowResult, SolveSettings);

/// What a cancelled solve returns.
pub const SOLVE_CANCELLED: &str = "Solve cancelled. The previous results are kept.";

/// What work returns when the case changed while it ran without the
/// workspace lock: its results describe a case that is no longer open.
pub const CASE_CHANGED: &str =
    "The case changed while this was running, so its results were set aside. Run it again.";

/// A new [`OpenCase::version`], never used before in this process.
fn next_version() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A solve to start the next one from: converged and AC.
fn resumable(solve: &Option<Arc<Solved>>) -> Option<&Arc<Solved>> {
    solve.as_ref().filter(|s| {
        s.1.model == power::ModelChoice::Ac && s.0.status == rs_power::SolveStatus::Converged
    })
}

/// Whether two cases have the same buses in the same order.
fn same_buses(a: &rs_power::Case, b: &rs_power::Case) -> bool {
    a.buses.len() == b.buses.len() && a.buses.iter().zip(&b.buses).all(|(x, y)| x.id == y.id)
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
    if report.phase_shifters_moved > 0 {
        log::info!(
            "Flow controls: {} ended away from their starting angles",
            plural(
                report.phase_shifters_moved,
                "phase shifter",
                "phase shifters"
            )
        );
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl OpenCase {
    pub fn from_bytes(path: &Path, bytes: &[u8]) -> Self {
        if crate::stn::is_stn(bytes) {
            return Self::from_stn(path, bytes);
        }
        let format = match path.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("m") => CaseFormat::Matpower,
            _ => CaseFormat::Raw,
        };
        Self::from_case_bytes(path, format, bytes)
    }

    /// A Stannum case (`.stn` or `.stnb`), read straight into the power
    /// flow model. Its RAW text is written on a thread of its own, for
    /// what needs it later.
    fn from_stn(path: &Path, bytes: &[u8]) -> Self {
        let name = file_name(path);
        let start = Instant::now();
        let doc = match crate::stn::read_file(&name, bytes) {
            Ok(doc) => Arc::new(doc),
            Err(e) => {
                return Self::from_power(
                    path,
                    CaseFormat::Stn,
                    CaseText::new(Vec::new()),
                    Err(e),
                    start,
                );
            }
        };
        log::info!(
            "Read {name} as a Stannum case ({:.0} ms)",
            elapsed_ms(start)
        );
        let start = Instant::now();
        // Warnings point at the .stn file's own lines; a .stnb file has rows, not lines.
        let binary = stannum_format::is_binary(bytes);
        let power = crate::stn::raw_sections(&doc).and_then(|(sections, lines)| {
            let mut power = power::load_sections(sections)?;
            for w in &mut power.warnings {
                w.line = if binary {
                    0
                } else {
                    lines.get(w.line).copied().unwrap_or(0)
                };
            }
            Ok(power)
        });
        let text = CaseText::from_stn(doc.clone(), name);
        let mut case = Self::from_power(path, CaseFormat::Stn, text.clone(), power, start);
        case.stn_source = Some(doc);
        text.prepare();
        case
    }

    /// A case from bytes in its working form: MATPOWER or RAW text.
    fn from_case_bytes(path: &Path, format: CaseFormat, bytes: &[u8]) -> Self {
        let text = decode_text(bytes);
        let start = Instant::now();
        let power = power::load(&text, format == CaseFormat::Matpower);
        Self::from_power(path, format, CaseText::new(bytes.to_vec()), power, start)
    }

    /// A case from its text and its power flow model, read since `start`.
    fn from_power(
        path: &Path,
        format: CaseFormat,
        bytes: CaseText,
        power: Result<PowerCase, String>,
        start: Instant,
    ) -> Self {
        let name = file_name(path);
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
        let mut case = Self {
            id: CaseId(0),
            path: path.to_path_buf(),
            name,
            format,
            version: next_version(),
            power: power.map(Arc::new),
            topology: OnceLock::new(),
            solved: None,
            before_edits: None,
            sensitivity: None,
            edits: vec![],
            edited: None,
            dc_grids: None,
            coordinates: None,
            places: None,
            contingencies: None,
            report: None,
            trip: None,
            records: None,
            bytes,
            record_edits: 0,
            stn_source: None,
        };
        // Without a power flow model the topology is all the case has, so
        // the summary needs to know at once whether it can be built.
        if case.power.is_err() {
            case.topology();
        }
        case.places = case.resolve_places().map(|c| c.summary);
        if let Some(p) = &case.places {
            log::info!(
                "{} places {} of its buses at {} substation{}",
                case.name,
                p.from_case,
                p.case_substations,
                if p.case_substations == 1 { "" } else { "s" }
            );
        }
        case
    }

    /// The case's topology, built the first time it's asked for. nodal reads
    /// RAW revision 35 with every record; anything else that power flow can
    /// read gets a topology built from its model.
    pub fn topology(&self) -> &Result<TopologyState, String> {
        self.topology.get_or_init(|| {
            let start = Instant::now();
            let from_nodal = match self.format {
                CaseFormat::Matpower => {
                    Err("Topology analysis reads PSS/E RAW revision 35 files.".to_string())
                }
                CaseFormat::Raw | CaseFormat::Stn => {
                    nodal::parse_raw(&self.bytes).map_err(|d| d.to_string())
                }
            };
            let topology = match (from_nodal, &self.power) {
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
                Err(e) => log::warn!("Topology can't use {}: {e}", self.name),
            }
            topology
        })
    }

    /// Whether the topology has been built yet.
    pub fn topology_built(&self) -> bool {
        self.topology.get().is_some()
    }

    /// Where every bus goes on the map, from the attached file and the
    /// case's own substation data; None when neither places any bus.
    fn resolve_places(&self) -> Option<BusCoordinates> {
        let power = self.power.as_ref().ok()?;
        crate::geo::resolve(
            self.coordinates.as_ref(),
            &crate::geo::case_places(&power.substations),
            &power.case,
            &crate::geo::spans(&power.elements.branches, &power.branch_lengths),
            &self.name,
        )
    }

    pub fn summary(&self) -> CaseSummary {
        CaseSummary {
            id: self.id,
            path: self.path.display().to_string(),
            name: self.name.clone(),
            format: self.format,
            power: ModuleStatus::from(&self.power, |p| PowerInfo {
                counts: power::counts(&p.case),
                revision: p.revision,
                warnings: p.warnings.clone(),
            }),
            topology: match self.topology.get() {
                Some(t) => ModuleStatus::from(t, |t| TopologyInfo {
                    revision: t.case.revision,
                    derived: t.derived,
                    buses: t.case.buses.len(),
                    equipment: t.case.equipment.len(),
                    titles: t.case.titles.clone(),
                }),
                // Not built yet, which only happens when the power flow
                // model was read: then a topology can always be built.
                None => ModuleStatus {
                    available: true,
                    error: None,
                    info: None,
                },
            },
            diffable: self.format.is_raw(),
            record_edits: self.record_edits,
            dc_grids: self.dc_grids.as_ref().map(|g| g.summary()),
            coordinates: self.places.clone(),
        }
    }

    /// The case power flow works on: the file with the study's edits applied.
    fn power_case(&self) -> Result<&PowerCase, Error> {
        let base = self.power.as_ref().map_err(|e| e.clone())?;
        Ok(self.edited.as_ref().unwrap_or(base))
    }

    /// [`Self::power_case`], shared, for work that runs without the lock.
    fn power_case_shared(&self) -> Result<Arc<PowerCase>, Error> {
        match &self.edited {
            Some(edited) => Ok(edited.clone()),
            None => Ok(self.power.clone()?),
        }
    }

    /// Fail with [`CASE_CHANGED`] unless this is still the case `version`
    /// was taken from.
    fn check_version(&self, version: u64) -> Result<(), Error> {
        if version == self.version {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::CaseChanged, CASE_CHANGED))
        }
    }

    /// Replace the study's edits. Results from before the change are dropped,
    /// since they describe a different case. On a bad edit nothing changes.
    pub fn set_edits(&mut self, edits: Vec<Edit>) -> Result<(), Error> {
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
        self.edited = edited.map(Arc::new);
        self.version = next_version();
        if let Some(solved) = self.solved.take() {
            self.before_edits = Some(solved);
        }
        self.sensitivity = None;
        self.report = None;
        self.trip = None;
        if self.contingencies.take().is_some() {
            log::info!("Contingency results cleared: they were for the case before this change");
        }
        Ok(())
    }

    /// A contingency run over the case with the study's edits, from its last
    /// converged AC solve. Run it with [`ContingencyJob::run`] (it doesn't
    /// need the workspace) and keep the results with
    /// [`Self::set_contingencies`].
    pub fn contingency_job(&self, settings: ContingencySettings) -> Result<ContingencyJob, Error> {
        let power = self.power_case()?;
        let base = &resumable(&self.solved)
            .ok_or_else(|| {
                Error::not_ready(
                    "Solve the case with the AC model first: contingencies start from that solution.",
                )
            })?
            .0;
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

    /// A transfer limit study of the case from its AC solve, to run
    /// without the workspace.
    pub fn transfer_job(
        &self,
        settings: crate::transfer::TransferSettings,
    ) -> Result<crate::transfer::TransferJob, Error> {
        let power = self.power_case()?;
        let base = &resumable(&self.solved)
            .ok_or_else(|| {
                Error::not_ready(
                    "Solve the case with the AC model first: transfer limits start from that solution.",
                )
            })?
            .0;
        let job = crate::transfer::TransferJob::new(power, base, settings)?;
        log::info!("Studying a transfer under {} contingencies", job.len());
        Ok(job)
    }

    /// A PV curve study of the case from its AC solve, with that solve's
    /// settings, to run without the workspace.
    pub fn pv_job(&self, settings: crate::pv::PvSettings) -> Result<crate::pv::PvJob, Error> {
        let power = self.power_case()?;
        let solved = resumable(&self.solved).ok_or_else(|| {
            Error::not_ready(
                "Solve the case with the AC model first: PV curves start from that solution.",
            )
        })?;
        if self.dc_grids.is_some() {
            log::warn!("PV curves leave out the attached VSC DC grids");
        }
        log::info!("Tracing a PV curve");
        Ok(crate::pv::PvJob::new(
            power, &solved.0, &solved.1, settings,
        )?)
    }

    /// A secure transfer study of the case from its AC solve, with that
    /// solve's settings, to run without the workspace.
    pub fn secure_transfer_job(
        &self,
        settings: crate::secure::SecureTransferSettings,
    ) -> Result<crate::secure::SecureTransferJob, Error> {
        let power = self.power_case()?;
        let solved = resumable(&self.solved).ok_or_else(|| {
            Error::not_ready(
                "Solve the case with the AC model first: secure transfer limits start from that solution.",
            )
        })?;
        if self.dc_grids.is_some() {
            log::warn!("Secure transfer limits leave out the attached VSC DC grids");
        }
        let job = crate::secure::SecureTransferJob::new(power, &solved.0, &solved.1, settings)?;
        log::info!(
            "Studying a secure transfer under {} contingencies",
            job.len()
        );
        Ok(job)
    }

    /// A QV curve study of the case from its AC solve, with that solve's
    /// settings, to run without the workspace.
    pub fn qv_job(&self, settings: crate::qv::QvSettings) -> Result<crate::qv::QvJob, Error> {
        let power = self.power_case()?;
        let solved = resumable(&self.solved).ok_or_else(|| {
            Error::not_ready(
                "Solve the case with the AC model first: QV curves start from that solution.",
            )
        })?;
        if self.dc_grids.is_some() {
            log::warn!("QV curves leave out the attached VSC DC grids");
        }
        log::info!("Drawing QV curves at {} buses", settings.buses.len());
        Ok(crate::qv::QvJob::new(
            power, &solved.0, &solved.1, settings,
        )?)
    }

    /// How many planning events each category asked for gives on the case
    /// as the study has it, before running them.
    pub fn planning_event_counts(
        &self,
        settings: &crate::planning::PlanningSettings,
    ) -> Result<Vec<crate::planning::CategoryCount>, Error> {
        Ok(crate::planning::counts(self.power_case()?, settings)?)
    }

    /// Keep a run finished without the workspace, unless the case changed
    /// while it ran (`version` is the case's when the job was taken).
    pub fn keep_contingencies(
        &mut self,
        version: u64,
        results: ContingencyResults,
    ) -> Result<ContingencySummary, Error> {
        if self.check_version(version).is_err() {
            log::warn!("The case changed while contingencies ran, so their results were set aside");
            return Err(Error::new(
                ErrorKind::CaseChanged,
                "The case changed while contingencies ran, so their results were set aside. Run them again.",
            ));
        }
        Ok(self.set_contingencies(results))
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
    ) -> Result<ContingencySummary, Error> {
        let results = self.contingency_job(settings)?.run()?;
        Ok(self.set_contingencies(results))
    }

    fn contingency_results(&self) -> Result<&ContingencyResults, Error> {
        self.contingencies
            .as_ref()
            .ok_or_else(|| Error::not_ready("Run contingencies first."))
    }

    /// Write every violation of the last contingency run to a CSV file.
    pub fn export_contingency_violations(
        &self,
        path: &Path,
    ) -> Result<crate::contingency::ViolationsExport, Error> {
        let results = self
            .contingencies
            .as_ref()
            .ok_or_else(|| Error::not_ready("Run contingencies first."))?;
        let settings = serde_json::to_value(results.settings()).ok();
        let header = self.export_header("Contingency violations", settings.as_ref());
        let file = std::fs::File::create(path)
            .map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
        let mut out = std::io::BufWriter::new(file);
        std::io::Write::write_all(&mut out, header.text.as_bytes())
            .map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
        let written = results
            .write_violations_csv(&mut out)
            .map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
        log::info!(
            "Exported {} contingency violations to {}",
            written.rows,
            path.display()
        );
        Ok(written)
    }

    /// Write the last contingency run to a compact results file (`.stnr`).
    pub fn export_contingency_results(
        &self,
        path: &Path,
    ) -> Result<crate::results_file::ResultsExport, Error> {
        let results = self
            .contingencies
            .as_ref()
            .ok_or_else(|| Error::not_ready("Run contingencies first."))?;
        let file = std::fs::File::create(path)
            .map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
        let written = crate::results_file::write_contingency_results(results, &self.name, file)
            .map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
        log::info!(
            "Exported {} outages ({} violations) to {}, {} bytes",
            written.outages,
            written.violations,
            path.display(),
            written.bytes
        );
        Ok(written)
    }

    pub fn contingency_summary(&self) -> Option<ContingencySummary> {
        self.contingencies.as_ref().map(|c| c.summary.clone())
    }

    pub fn contingency_rows(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<crate::table::Table<ContingencyRow>, Error> {
        Ok(self.contingency_results()?.page(offset, limit))
    }

    /// Take one branch or generator out of the last AC solve and solve again
    /// in AC with the same settings: "Trip and compare". `ratings` are the
    /// ratings after the trip and before it. The contingency
    /// run in hand, if any, is kept.
    pub fn trip_and_compare(
        &mut self,
        element: &str,
        limit_pct: f64,
        ratings: (Rating, Rating),
    ) -> Result<ContingencyDetail, Error> {
        let done = self.trip_job(element, limit_pct, ratings)?.run()?;
        self.finish_trip(done)
    }

    /// Keep a finished trip so its whole solution can be shown, unless the
    /// case changed while it ran.
    pub fn finish_trip(&mut self, done: TripDone) -> Result<ContingencyDetail, Error> {
        self.check_version(done.version)?;
        self.trip = Some((done.element, done.results));
        Ok(done.detail)
    }

    /// [`Self::trip_and_compare`]'s solve, to run with [`TripJob::run`]
    /// without the workspace.
    pub fn trip_job(
        &self,
        element: &str,
        limit_pct: f64,
        ratings: (Rating, Rating),
    ) -> Result<TripJob, Error> {
        let power = self.power_case()?;
        let (base, solve) = &**resumable(&self.solved).ok_or_else(|| {
            Error::not_ready(
                "Solve the case with the AC model first: the trip starts from that solution.",
            )
        })?;
        let settings = ContingencySettings {
            elements: Some(vec![element.to_string()]),
            branches: true,
            generators: true,
            buses: false,
            pairs: false,
            skip_zero_impedance: false,
            zero_impedance_pu: 0.0001,
            method: crate::contingency::Method::Ac,
            ac_count: 1,
            limit_pct,
            rating: ratings.0,
            base_rating: ratings.1,
            solve: solve.clone(),
            ..ContingencySettings::default()
        };
        let job = ContingencyJob::new(power, base, settings).map_err(|e| {
            if e.starts_with("Nothing to take out") {
                format!("{element} is already out of service.")
            } else {
                e
            }
        })?;
        Ok(TripJob {
            version: self.version,
            element: element.to_string(),
            job,
        })
    }

    /// A result by name (see [`crate::results`]): the latest solve, one
    /// outage of the last contingency run, or the last trip, as a full solve
    /// report that every view can show.
    pub fn result(&mut self, name: &str) -> Result<power::SolveReport, Error> {
        self.shared_result(name).map(|r| (*r).clone())
    }

    /// [`Self::result`] without copying it, for paging through its tables.
    /// "latest" names the latest solve, whichever of base and study it is.
    pub fn shared_result(&mut self, name: &str) -> Result<Arc<power::SolveReport>, Error> {
        if name.trim() == "latest" {
            return self
                .report
                .clone()
                .ok_or_else(|| Error::not_ready("Solve the case first."));
        }
        let study = !self.edits.is_empty();
        match name
            .parse::<ResultName>()
            .map_err(Error::invalid_arguments)?
        {
            ResultName::Base if study => Err(Error::not_found(
                "The latest solve includes the study's edits: it's named \"study\".",
            )),
            ResultName::Study if !study => Err(Error::not_found(
                "There are no study edits: the latest solve is named \"base\".",
            )),
            ResultName::Base | ResultName::Study => self
                .report
                .clone()
                .ok_or_else(|| Error::not_ready("Solve the case first.")),
            ResultName::Contingency(rank) => self
                .contingencies
                .as_mut()
                .ok_or_else(|| Error::not_ready("Run contingencies first."))?
                .shared_report(rank),
            ResultName::Trip(element) => match self.trip.as_mut() {
                Some((tripped, results)) if *tripped == element => results.shared_report(1),
                _ => Err(Error::not_ready(format!(
                    "Trip and compare {element} first: only the last trip is kept."
                ))),
            },
        }
    }

    pub fn contingency_detail(&mut self, rank: usize) -> Result<ContingencyDetail, Error> {
        self.contingencies
            .as_mut()
            .ok_or_else(|| Error::not_ready("Run contingencies first."))?
            .detail(rank)
    }

    pub fn solve(&mut self, settings: SolveSettings) -> Result<power::SolveReport, Error> {
        self.solve_cancellable(settings, None)
    }

    /// [`Self::solve`], stopped early when `cancel` is set from another
    /// thread. A cancelled solve keeps the previous results.
    pub fn solve_cancellable(
        &mut self,
        settings: SolveSettings,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<power::SolveReport, Error> {
        let done = self.solve_job(settings, cancel)?.run()?;
        self.finish_solve(done)
    }

    /// A solve of the case with the study's edits, to run with
    /// [`SolveJob::run`] without the workspace and keep with
    /// [`Self::finish_solve`]. Taking it is cheap: it shares the case.
    pub fn solve_job(
        &self,
        settings: SolveSettings,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<SolveJob, Error> {
        // The last converged AC solve: of this case, else from before the
        // study's edits changed.
        let last = resumable(&self.solved)
            .map(|r| (r.clone(), true))
            .or_else(|| resumable(&self.before_edits).map(|r| (r.clone(), false)));
        Ok(SolveJob {
            version: self.version,
            power: self.power_case_shared()?,
            last,
            dc_grids: self.dc_grids.clone(),
            edits: self.edits.clone(),
            settings,
            cancel,
        })
    }

    /// Keep a finished solve as the case's latest, unless the case changed
    /// while it ran.
    pub fn finish_solve(&mut self, done: SolveDone) -> Result<power::SolveReport, Error> {
        if self.check_version(done.version).is_err() {
            log::warn!("The case changed during the solve, so its results were set aside");
            return Err(Error::new(
                ErrorKind::CaseChanged,
                "The case changed during the solve, so its results were set aside. Solve again.",
            ));
        }
        self.solved = Some(Arc::new((done.result, done.settings)));
        self.before_edits = None;
        let report = Arc::new(done.report);
        self.report = Some(report.clone());
        Ok(Arc::unwrap_or_clone(report))
    }

    /// Attach a DC grid file; the next AC solve includes its grids. The file
    /// is checked against the case first, and a bad file changes nothing.
    pub fn load_dc_grids(&mut self, path: &Path) -> Result<DcGridsSummary, Error> {
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
        self.dc_grids = Some(Arc::new(loaded));
        Ok(summary)
    }

    /// Attach a bus coordinates CSV. Rows that can't be read are skipped and
    /// listed in the summary; a file with no usable header changes nothing.
    pub fn load_coordinates(&mut self, path: &Path) -> Result<CoordinatesSummary, Error> {
        let loaded = LoadedCoordinates::read(path)?;
        let power = self.power.as_ref().map_err(|e| e.clone())?;
        let summary = crate::geo::resolve(
            Some(&loaded),
            &crate::geo::case_places(&power.substations),
            &power.case,
            &crate::geo::spans(&power.elements.branches, &power.branch_lengths),
            &self.name,
        )
        .expect("a file always resolves")
        .summary;
        log::info!(
            "Attached {}: {} buses placed from the file, {} from the case, {} estimated (from {}), {} without a place",
            summary.name,
            summary.from_file,
            summary.from_case,
            summary.estimated,
            summary.estimated_from,
            summary.missing
        );
        if summary.unknown_buses > 0 {
            log::warn!(
                "{}: {} name{} a bus this case doesn't have",
                summary.name,
                plural(summary.unknown_buses, "row", "rows"),
                if summary.unknown_buses == 1 { "s" } else { "" }
            );
        }
        for p in &summary.problems {
            log::warn!("{} line {}: {}", summary.name, p.line, p.message);
        }
        if summary.problem_count > summary.problems.len() {
            log::warn!(
                "{}: {} more skipped",
                summary.name,
                plural(
                    summary.problem_count - summary.problems.len(),
                    "line",
                    "lines"
                )
            );
        }
        self.coordinates = Some(loaded);
        self.places = Some(summary.clone());
        Ok(summary)
    }

    /// Detach the coordinates file. The case's own substation places, if
    /// any, still place its buses.
    pub fn clear_coordinates(&mut self) {
        if let Some(c) = self.coordinates.take() {
            log::info!("Detached {}", c.path.display());
        }
        self.places = self.resolve_places().map(|c| c.summary);
    }

    /// Where every bus goes on the map, in the case's bus order. Fails when
    /// neither a coordinates file nor the case's own data places any bus.
    pub fn bus_coordinates(&self) -> Result<BusCoordinates, Error> {
        if let Err(e) = &self.power {
            return Err(e.clone().into());
        }
        self.resolve_places()
            .ok_or_else(|| Error::not_ready("Attach a bus coordinates file first."))
    }

    pub fn clear_dc_grids(&mut self) {
        if let Some(g) = self.dc_grids.take() {
            log::info!("Detached {}", g.path.display());
        }
    }

    pub fn sensitivities(
        &mut self,
        settings: SolveSettings,
    ) -> Result<power::SensitivitySummary, Error> {
        let done = self.sensitivity_job(settings)?.run()?;
        self.finish_sensitivities(done)
    }

    /// Sensitivities of the case with the study's edits, to run with
    /// [`SensitivityJob::run`] without the workspace.
    pub fn sensitivity_job(&self, settings: SolveSettings) -> Result<SensitivityJob, Error> {
        Ok(SensitivityJob {
            version: self.version,
            power: self.power_case_shared()?,
            settings,
        })
    }

    /// Keep finished sensitivities, unless the case changed while they ran.
    pub fn finish_sensitivities(
        &mut self,
        done: SensitivitiesDone,
    ) -> Result<power::SensitivitySummary, Error> {
        self.check_version(done.version)?;
        self.sensitivity = Some((done.sensitivity, done.model));
        Ok(done.summary)
    }

    pub fn responses(
        &self,
        column: usize,
        quantity: &str,
        engineering: bool,
    ) -> Result<power::ResponseTable, Error> {
        let (s, _) = self
            .sensitivity
            .as_ref()
            .ok_or_else(|| Error::not_ready("Calculate sensitivities first."))?;
        let q = power::Quantity::parse(quantity)
            .ok_or_else(|| Error::invalid_arguments(format!("Unknown quantity {quantity}.")))?;
        Ok(power::responses(
            s,
            &self.power_case()?.elements,
            column,
            q,
            engineering,
        )?)
    }

    /// Write the input case, or the latest solved case, as MATPOWER.
    pub fn export_matpower(&self, path: &Path, solved: bool) -> Result<(), Error> {
        let case = if solved {
            &self
                .solved
                .as_ref()
                .ok_or_else(|| Error::not_ready("Solve the case before exporting results."))?
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

    /// Write the case, with its Records edits and the study's edits, as a
    /// complete PSS/E RAW file of revision 33 or 35.
    ///
    /// RAW 33 and 35 cases are written record by record from the file;
    /// anything else from the power flow model (see [`crate::raw_export`]).
    pub fn export_raw(
        &mut self,
        path: &Path,
        revision: u32,
    ) -> Result<crate::raw_export::RawExport, Error> {
        if !matches!(revision, 33 | 35) {
            return Err(Error::invalid_arguments(format!(
                "Stannum writes RAW revisions 33 and 35, not {revision}."
            )));
        }
        let start = Instant::now();
        let from_records = if self.format.is_raw() {
            match self.working_bytes() {
                Ok(bytes) => crate::raw_export::from_raw(&self.name, &bytes, revision),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err(String::new())
        };
        let (bytes, export) = match from_records {
            Ok(done) => done,
            Err(reason) => {
                let power = self.power_case_shared()?;
                let why = match (self.format, power.revision) {
                    (CaseFormat::Matpower, _) => {
                        "Written from the MATPOWER case's power flow model.".to_string()
                    }
                    (_, Some(rev)) if !matches!(rev, 33 | 35) => format!(
                        "Written from the power flow model: Stannum reads RAW revision {rev} for power flow only, not record by record."
                    ),
                    _ => format!(
                        "Written from the power flow model: the file's records couldn't be read one by one ({reason})."
                    ),
                };
                crate::raw_export::from_model(&self.name, &power, revision, why)?
            }
        };
        std::fs::write(path, bytes).map_err(|e| Error::file(path, e))?;
        log::info!(
            "Exported {} as RAW {revision} to {} ({} records, {:.0} ms)",
            self.name,
            path.display(),
            export.records,
            elapsed_ms(start)
        );
        for note in &export.notes {
            log::info!("RAW export: {note}");
        }
        Ok(export)
    }

    /// Write the case, with its Records edits and the study's edits, as a
    /// Stannum case file: `.stnb` (binary) when `path` ends so, else `.stn`
    /// (text). RAW 33 and 35 cases are converted record by record; anything
    /// else from the power flow model.
    pub fn export_stn(&mut self, path: &Path) -> Result<crate::stn::StnExport, Error> {
        let start = Instant::now();
        let from_records = if self.format.is_raw() {
            match self.working_bytes() {
                Ok(bytes) => crate::stn::from_raw(&self.name, &bytes),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err(String::new())
        };
        let (mut doc, export) = match from_records {
            Ok(done) => done,
            Err(reason) => {
                let power = self.power_case_shared()?;
                let why = match (self.format, power.revision) {
                    (CaseFormat::Matpower, _) => {
                        "Written from the MATPOWER case's power flow model.".to_string()
                    }
                    (_, Some(rev)) if !matches!(rev, 33 | 35) => format!(
                        "Written from the power flow model: Stannum reads RAW revision {rev} for power flow only, not record by record."
                    ),
                    _ => format!(
                        "Written from the power flow model: the file's records couldn't be read one by one ({reason})."
                    ),
                };
                crate::stn::from_model(&self.name, &power, why)
            }
        };
        // A Stannum case keeps its file's ids, comments and extensions.
        if let Some(source) = &self.stn_source {
            doc.carry_from(source);
        }
        let bytes = crate::stn::encode(&doc, crate::stn::is_binary_path(path))?;
        std::fs::write(path, bytes).map_err(|e| Error::file(path, e))?;
        log::info!(
            "Exported {} as a Stannum case to {} ({} records, {:.0} ms)",
            self.name,
            path.display(),
            export.records,
            elapsed_ms(start)
        );
        for note in &export.notes {
            log::info!("Stannum case export: {note}");
        }
        Ok(export)
    }

    /// The case as buses, generators and branches for the one-line diagram.
    pub fn diagram_network(&self) -> Result<crate::diagram::CaseNetwork, Error> {
        Ok(crate::diagram::network(self.power_case()?))
    }

    /// One element's data and limits from the file, for the element inspector.
    pub fn element_detail(&self, element: &str) -> Result<crate::inspect::ElementDetail, Error> {
        let power = self.power.as_ref().map_err(|e| e.clone())?;
        if let Some(kind) = crate::elements::kind_of(element)
            && kind.inspector == crate::elements::Source::Record
        {
            return match self.records.as_ref() {
                Some(Ok(records)) => crate::inspect::from_record(power, records, kind, element),
                Some(Err(e)) => Err(e.clone().into()),
                None => Err(Error::not_ready("Read the case's records first.")),
            };
        }
        // RAW 35 records as nodal read them; other files would only get a
        // topology derived from the same model, so don't build one for that.
        let topology = (power.revision == Some(35))
            .then(|| self.topology().as_ref().ok())
            .flatten()
            .filter(|t| !t.derived)
            .map(|t| &t.case);
        let mut detail = crate::inspect::element(power, topology, element)?;
        // Shunts, FACTS devices and the like at a bus, once the records have
        // been read (reading them just for this could take seconds).
        if detail.kind == "bus"
            && let (Some(Ok(records)), Some(bus)) = (self.records.as_ref(), detail.buses.first())
        {
            detail
                .connected
                .extend(crate::inspect::record_elements_at(records, bus.number));
        }
        if let (Some(number), Some(coordinates)) = (
            element
                .strip_prefix("bus:")
                .and_then(|n| n.parse::<u32>().ok()),
            self.places.as_ref().and_then(|_| self.resolve_places()),
        ) {
            detail
                .data
                .push(crate::inspect::location(coordinates.of(number)));
        }
        Ok(detail)
    }

    /// The file's records as tables: RAW revisions 33 and 35 record by
    /// record, anything else as the power flow model.
    pub fn records(&mut self) -> Result<&Records, Error> {
        if let Some(job) = self.records_job() {
            let read = job.run();
            self.keep_records(read)?;
        }
        match self.records.as_ref() {
            Some(Ok(r)) => Ok(r.as_ref()),
            Some(Err(e)) => Err(e.clone().into()),
            None => unreachable!("records were just read"),
        }
    }

    /// The first read of the records, to run with [`RecordsJob::run`]
    /// without the workspace; None once they've been read.
    pub fn records_job(&self) -> Option<RecordsJob> {
        if self.records.is_some() {
            return None;
        }
        Some(RecordsJob {
            version: self.version,
            name: self.name.clone(),
            format: self.format,
            bytes: self.bytes.clone(),
            power: self.power.clone(),
        })
    }

    /// Keep records read by a [`RecordsJob`], unless the case changed while
    /// they were read. Records another read kept first stay.
    pub fn keep_records(&mut self, read: RecordsRead) -> Result<(), Error> {
        self.check_version(read.version)?;
        if self.records.is_none() {
            self.records = Some(read.records.map(Arc::new));
        }
        Ok(())
    }

    pub fn record_book(&mut self) -> Result<RecordBook, Error> {
        Ok(self.records()?.book())
    }

    /// The records, shared, so a page can be cut without the workspace.
    pub fn shared_records(&mut self) -> Result<Arc<Records>, Error> {
        self.records()?;
        match self.records.as_ref() {
            Some(Ok(r)) => Ok(r.clone()),
            Some(Err(e)) => Err(e.clone().into()),
            None => unreachable!("records were just read"),
        }
    }

    pub fn record_table(&mut self, section: &str) -> Result<RecordTable, Error> {
        Ok(self.records()?.table(section)?.clone())
    }

    /// Write edits from Records into the case and open the edited case in
    /// its place, keeping the study's edits and any DC grid file. Nothing
    /// changes when an edit is invalid or the edited case can't be read.
    pub fn apply_record_edits(&mut self, edits: &[RecordEdit]) -> Result<CaseSummary, Error> {
        if edits.is_empty() {
            return Ok(self.summary());
        }
        let done = self.record_edit_job(edits)?.run()?;
        self.finish_record_edits(done)
    }

    /// Record edits to write and reread with [`RecordEditJob::run`] without
    /// the workspace, and keep with [`Self::finish_record_edits`]. Reads
    /// the records here if they haven't been read yet.
    pub fn record_edit_job(&mut self, edits: &[RecordEdit]) -> Result<RecordEditJob, Error> {
        self.records()?;
        let Some(Ok(records)) = self.records.as_ref() else {
            unreachable!("records were just read")
        };
        if !records.editable {
            return Err(Error::unsupported(
                "This case's records can't be edited: Stannum writes RAW revisions 33 and 35, and MATPOWER.",
            ));
        }
        Ok(RecordEditJob {
            version: self.version,
            path: self.path.clone(),
            format: self.format,
            name: self.name.clone(),
            bytes: self.bytes.clone(),
            records: records.clone(),
            power: self.power.clone(),
            study_edits: self.edits.clone(),
            edits: edits.to_vec(),
        })
    }

    /// Put the edited case in this one's place, unless the case changed
    /// while the edits were written.
    pub fn finish_record_edits(&mut self, done: RecordEditsDone) -> Result<CaseSummary, Error> {
        self.check_version(done.version)?;
        let RecordEditsDone {
            mut next,
            updates,
            count,
            start,
            ..
        } = done;
        next.id = self.id;
        // "Save as" may have moved the case to a new file meanwhile.
        next.path = self.path.clone();
        next.name = self.name.clone();
        next.record_edits = self.record_edits + count;
        next.stn_source = self.stn_source.take();
        next.dc_grids = self.dc_grids.take();
        // Matched again, since an edit can add, remove or reconnect buses.
        next.coordinates = self.coordinates.take();
        next.places = match &next.power {
            Ok(_) => next.resolve_places().map(|c| c.summary),
            Err(_) => self.places.take(),
        };
        if let Some(updates) = updates {
            // The RAW tables only change in the edited cells.
            let mut records = self
                .records
                .take()
                .and_then(Result::ok)
                .map(Arc::unwrap_or_clone);
            if let Some(r) = records.as_mut() {
                record_edit::patch(r, updates)?;
            }
            next.records = records.map(|r| Ok(Arc::new(r)));
        }
        // The last converged AC solve stays the next solve's starting point
        // while the buses are the same ones: an edit to a load or a branch
        // then re-solves from where the case was, not from the file.
        if let (Some(last), Ok(power)) = (
            resumable(&self.solved).or_else(|| resumable(&self.before_edits)),
            &next.power,
        ) && same_buses(&last.0.case, &power.case)
        {
            next.before_edits = Some(last.clone());
        }
        log::info!(
            "Applied {} to {} ({:.0} ms)",
            if count == 1 {
                "1 record edit".to_string()
            } else {
                format!("{count} record edits")
            },
            self.name,
            elapsed_ms(start)
        );
        *self = next;
        Ok(self.summary())
    }

    /// The case as it is being worked on, as RAW text: the file as opened,
    /// with Records edits and the study's edits written in. Only RAW
    /// revisions Records can edit (33 and 35) can carry study edits.
    pub fn working_bytes(&mut self) -> Result<Vec<u8>, Error> {
        if !self.format.is_raw() {
            return Err(Error::unsupported(
                "Case diff reads PSS/E RAW files, and this case is MATPOWER.",
            ));
        }
        if self.edits.is_empty() {
            return Ok(self.bytes.to_vec());
        }
        self.records()?;
        let Some(Ok(records)) = self.records.as_ref() else {
            unreachable!("records were just read")
        };
        if !records.editable {
            return Err(Error::unsupported(
                "Stannum can only write study edits into RAW revisions 33 and 35, so this case's study can't be compared with its file.",
            ));
        }
        let base = self.power.as_ref().map_err(|e| e.clone())?;
        let edits = crate::working_case::study_record_edits(records, base, &self.edits)?;
        let (text, _) = record_edit::apply_raw(&decode_text(&self.bytes), records, &edits)?;
        if std::str::from_utf8(&self.bytes).is_ok() {
            return Ok(text.into_bytes());
        }
        let (encoded, _, unmappable) = encoding_rs::WINDOWS_1252.encode(&text);
        if unmappable {
            return Err(
                "The working case has a character this file's encoding (Windows-1252) can't hold."
                    .into(),
            );
        }
        Ok(encoded.into_owned())
    }

    /// Save the case, with its edits, to a new file, and carry on with that file.
    ///
    /// A RAW or Stannum case is saved as a Stannum case file when `path`
    /// ends in `.stn` (text) or `.stnb` (binary), and as RAW otherwise.
    pub fn save_as(&mut self, path: &Path) -> Result<CaseSummary, Error> {
        let to_stn = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("stn") || e.eq_ignore_ascii_case("stnb"));
        let format = match (to_stn, self.format) {
            (true, CaseFormat::Matpower) => {
                return Err(Error::unsupported(
                    "Save a MATPOWER case as a Stannum case with Export instead: it's written from the power flow model.",
                ));
            }
            (true, _) => CaseFormat::Stn,
            (false, CaseFormat::Stn) => CaseFormat::Raw,
            (false, format) => format,
        };
        if format == CaseFormat::Stn {
            let (mut doc, _) = crate::stn::from_raw(&self.name, &self.bytes)?;
            if let Some(source) = &self.stn_source {
                doc.carry_from(source);
            }
            let bytes = crate::stn::encode(&doc, crate::stn::is_binary_path(path))?;
            std::fs::write(path, bytes).map_err(|e| Error::file(path, e))?;
            self.stn_source = Some(Arc::new(doc));
        } else {
            std::fs::write(path, self.bytes.get()).map_err(|e| Error::file(path, e))?;
            self.stn_source = None;
        }
        log::info!("Saved {} as {}", self.name, path.display());
        self.format = format;
        self.path = path.to_path_buf();
        self.name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        self.record_edits = 0;
        Ok(self.summary())
    }

    fn topology_state(&self) -> Result<&TopologyState, Error> {
        Ok(self.topology().as_ref().map_err(|e| e.clone())?)
    }

    pub fn topology_snapshot(&self) -> Result<TopologySnapshot<'_>, Error> {
        let t = self.topology_state()?;
        Ok(TopologySnapshot {
            case: &t.case,
            analysis: t.topology.analyze(&t.case),
            graph: t.topology.graph_data(),
        })
    }

    /// The snapshot as the app receives it, in [`topology_wire`] form.
    pub fn topology_wire(&self) -> Result<serde_json::Value, Error> {
        let t = self.topology_state()?;
        let analysis = t.topology.analyze(&t.case);
        let graph = t.topology.graph_data();
        let view = topology_wire::view_wire(&t.case, &analysis, &graph)?;
        serde_json::to_value(topology_wire::TopologyWire {
            case: topology_wire::case_wire(&t.case),
            analysis: view.analysis,
            graph: view.graph,
        })
        .map_err(|e| Error::internal(e.to_string()))
    }

    /// The record an element was read from, or `None` when there's no such element.
    pub fn topology_source(
        &self,
        element: &str,
    ) -> Result<Option<nodal::model::SourceRecord>, Error> {
        let t = self.topology_state()?;
        Ok(topology_wire::source(&t.case, element).cloned())
    }

    /// Merge zero-impedance branches or split them again, and send the new
    /// analysis and graph in [`topology_wire`] form.
    pub fn set_merge_zero_impedance_wire(
        &mut self,
        enabled: bool,
    ) -> Result<serde_json::Value, Error> {
        self.set_merge_zero_impedance(enabled)?;
        let t = self.topology_state()?;
        let analysis = t.topology.analyze(&t.case);
        let graph = t.topology.graph_data();
        serde_json::to_value(topology_wire::view_wire(&t.case, &analysis, &graph)?)
            .map_err(|e| Error::internal(e.to_string()))
    }

    pub fn set_merge_zero_impedance(&mut self, enabled: bool) -> Result<TopologyView, Error> {
        self.topology();
        let t = self
            .topology
            .get_mut()
            .expect("just built")
            .as_mut()
            .map_err(|e| e.clone())?;
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

    pub fn shortest_path(&self, from: i32, to: i32) -> Result<Option<TopologyPath>, Error> {
        let t = self.topology_state()?;
        t.topology
            .shortest_path(&t.case, from, to)
            .map_err(|d| Error::not_found(d.to_string()))
    }
}

fn not_open(id: Option<CaseId>) -> Error {
    match id {
        Some(id) => Error::new(ErrorKind::CaseNotOpen, format!("Case {id} isn't open.")),
        None => Error::new(ErrorKind::NoCase, "Open a case first."),
    }
}

fn check_id(case: &OpenCase, id: Option<CaseId>) -> Result<(), Error> {
    match id {
        Some(id) if id != case.id => Err(not_open(Some(id))),
        _ => Ok(()),
    }
}

impl Workspace {
    pub fn open(&mut self, path: &Path) -> Result<CaseSummary, Error> {
        log::info!("Opening {}", path.display());
        let bytes = std::fs::read(path).map_err(|e| Error::file(path, e))?;
        let mut case = OpenCase::from_bytes(path, &bytes);
        if case.power.is_err()
            && case.topology().is_err()
            && matches!(case.format, CaseFormat::Matpower | CaseFormat::Stn)
        {
            return Err(case.power.err().unwrap_or_default().into());
        }
        self.last_id += 1;
        case.id = CaseId(self.last_id);
        let summary = case.summary();
        self.case = Some(case);
        Ok(summary)
    }

    pub fn close(&mut self) {
        self.case = None;
    }

    pub fn current(&self) -> Result<&OpenCase, Error> {
        self.case(None)
    }

    pub fn current_mut(&mut self) -> Result<&mut OpenCase, Error> {
        self.case_mut(None)
    }

    /// The case `id` names, or the open case when `id` is None.
    pub fn case(&self, id: Option<CaseId>) -> Result<&OpenCase, Error> {
        let case = self.case.as_ref().ok_or_else(|| not_open(id))?;
        check_id(case, id)?;
        Ok(case)
    }

    pub fn case_mut(&mut self, id: Option<CaseId>) -> Result<&mut OpenCase, Error> {
        let case = self.case.as_mut().ok_or_else(|| not_open(id))?;
        check_id(case, id)?;
        Ok(case)
    }

    pub fn compare(
        &mut self,
        baseline: &Path,
        comparison: &Path,
    ) -> Result<diff::DiffSummary, Error> {
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

    /// Compare the file on disk (baseline) with the case as it is being
    /// worked on: Records edits and the study's edits.
    pub fn compare_working(&mut self) -> Result<diff::DiffSummary, Error> {
        self.comparison = None;
        let start = Instant::now();
        let case = self.current_mut()?;
        let file = std::fs::read(&case.path).map_err(|e| Error::file(&case.path, e))?;
        let file = crate::stn::raw_bytes_of(&case.name, file)?;
        let working = case.working_bytes()?;
        let name = case.name.clone();
        let (record_edits, study_edits) = (case.record_edits, case.edits.len());
        let before = diff::parse_bytes(&name, &file).map_err(|e| e.to_string())?;
        let after =
            diff::parse_bytes(&format!("{name} (working)"), &working).map_err(|e| e.to_string())?;
        let result = diff::compare_cases(before, after);
        let summary = result.summary();
        log::info!(
            "Case diff of {name} with its {record_edits} record edit(s) and {study_edits} study edit(s): {} differing records ({:.0} ms)",
            summary.differing,
            elapsed_ms(start)
        );
        self.comparison = Some(result);
        Ok(summary)
    }

    pub fn comparison(&self) -> Result<&Comparison, Error> {
        self.comparison
            .as_ref()
            .ok_or_else(|| Error::not_ready("Compare two files first."))
    }
}

/// A solve taken out of the open case. It shares what it reads with the
/// case, so it runs without the workspace lock and the other modules stay
/// usable meanwhile.
pub struct SolveJob {
    version: u64,
    power: Arc<PowerCase>,
    /// The solve to start from, and whether it was of this case (else from
    /// before the study's edits changed).
    last: Option<(Arc<Solved>, bool)>,
    dc_grids: Option<Arc<LoadedDcGrids>>,
    edits: Vec<Edit>,
    settings: SolveSettings,
    cancel: Option<Arc<AtomicBool>>,
}

/// A finished [`SolveJob`], to keep with [`OpenCase::finish_solve`].
pub struct SolveDone {
    version: u64,
    result: PowerFlowResult,
    settings: SolveSettings,
    report: power::SolveReport,
}

impl SolveJob {
    pub fn run(self) -> Result<SolveDone, Error> {
        let SolveJob {
            version,
            power,
            last,
            dc_grids,
            edits,
            settings,
            cancel,
        } = self;
        let power = power.as_ref();
        let start = Instant::now();
        let cold = settings.options_for(power);
        let mut options = cold.clone();
        options.cancel = cancel;
        let failed = |e: rs_power::Error| match e {
            rs_power::Error::Cancelled => {
                log::info!("Solve cancelled; the previous results are kept");
                Error::cancelled(SOLVE_CANCELLED)
            }
            e => Error::from(study::explain(e.to_string(), &edits)),
        };
        let warm = match (&last, settings.start) {
            (Some((last, same_case)), power::StartChoice::Last)
                if settings.model == power::ModelChoice::Ac =>
            {
                if !same_case {
                    log::info!(
                        "Starting from the last solve, from before the case or study changed"
                    );
                }
                Some(power::warm_start(&power.case, &last.0, &mut options))
            }
            (None, power::StartChoice::Last) if settings.model == power::ModelChoice::Ac => {
                log::info!(
                    "No converged AC solve to start from yet; starting from the case voltages"
                );
                None
            }
            _ => None,
        };
        let grids = dc_grids
            .as_deref()
            .filter(|_| settings.model == power::ModelChoice::Ac);
        if dc_grids.is_some() && grids.is_none() {
            log::warn!("VSC DC grids need the AC power flow; this DC solve leaves them out");
        }
        let run = |input: &rs_power::Case, options: &rs_power::PowerFlowOptions| {
            Ok::<_, Error>(match grids {
                Some(file) => {
                    let acdc = rs_power::solve_dc_grids(
                        input,
                        options,
                        &file.grids,
                        &rs_power::DcGridSettings::default(),
                    )
                    .map_err(failed)?;
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
                None => (rs_power::solve(input, options).map_err(failed)?, None),
            })
        };
        let (mut result, mut vsc) = run(warm.as_ref().unwrap_or(&power.case), &options)?;
        if warm.is_some() && result.status != rs_power::SolveStatus::Converged {
            // The last solution can be a poor start for different settings,
            // such as the first solve with Q limits after one without them.
            log::warn!(
                "The solve from the last solution did not converge; solving again from the case voltages"
            );
            let mut retry = cold.clone();
            retry.cancel = options.cancel.clone();
            (result, vsc) = run(&power.case, &retry)?;
        }
        let solved_ms = elapsed_ms(start);
        let mut report = power::report(
            &result,
            &power.elements,
            &power.dc_line_kinds,
            // Count moves from the file's settings, not from the last solve.
            &cold.voltage_controls,
            &cold.flow_controls,
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
        log_report(&report, solved_ms);
        Ok(SolveDone {
            version,
            result,
            settings,
            report,
        })
    }
}

/// A "Trip and compare" solve taken out of the open case.
pub struct TripJob {
    version: u64,
    element: String,
    job: ContingencyJob,
}

/// A finished [`TripJob`], to keep with [`OpenCase::finish_trip`].
pub struct TripDone {
    version: u64,
    element: String,
    results: ContingencyResults,
    detail: ContingencyDetail,
}

impl TripJob {
    pub fn run(self) -> Result<TripDone, Error> {
        let mut results = self.job.run()?;
        let detail = results.detail(1)?;
        log::info!(
            "Tripped {}: {} overload{} new or worse",
            self.element,
            detail.overloads.len(),
            if detail.overloads.len() == 1 { "" } else { "s" }
        );
        Ok(TripDone {
            version: self.version,
            element: self.element,
            results,
            detail,
        })
    }
}

/// Sensitivities taken out of the open case, to run without the lock.
pub struct SensitivityJob {
    version: u64,
    power: Arc<PowerCase>,
    settings: SolveSettings,
}

/// Finished sensitivities, to keep with [`OpenCase::finish_sensitivities`].
pub struct SensitivitiesDone {
    version: u64,
    sensitivity: SensitivityModel,
    model: power::ModelChoice,
    summary: power::SensitivitySummary,
}

impl SensitivityJob {
    pub fn run(self) -> Result<SensitivitiesDone, Error> {
        let power = self.power.as_ref();
        let start = Instant::now();
        let s = SensitivityModel::new(&power.case, &self.settings.options_for(power))
            .map_err(|e| e.to_string())?;
        let summary = power::sensitivity_summary(&s, self.settings.model, elapsed_ms(start));
        log::info!(
            "Sensitivities: {} injection columns, {} branches ({:.0} ms)",
            summary.injections.len(),
            summary.branches,
            summary.elapsed_ms
        );
        Ok(SensitivitiesDone {
            version: self.version,
            sensitivity: s,
            model: self.settings.model,
            summary,
        })
    }
}

/// The first read of a case's records, to run without the lock.
pub struct RecordsJob {
    version: u64,
    name: String,
    format: CaseFormat,
    bytes: CaseText,
    power: Result<Arc<PowerCase>, String>,
}

/// Records read by a [`RecordsJob`], to keep with [`OpenCase::keep_records`].
pub struct RecordsRead {
    version: u64,
    records: Result<Records, String>,
}

impl RecordsJob {
    pub fn run(self) -> RecordsRead {
        let start = Instant::now();
        let raw = match self.format {
            CaseFormat::Raw | CaseFormat::Stn => Some(
                crate::diff::parser::parse_bytes_for_viewing(&self.name, &self.bytes)
                    .map_err(|e| e.to_string()),
            ),
            CaseFormat::Matpower => None,
        };
        let records = match (raw, &self.power) {
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
        if let Ok(r) = &records {
            let count: usize = r.tables.iter().map(|t| t.rows.len()).sum();
            log::info!(
                "Read {count} records in {} tables ({:.0} ms)",
                r.tables.len(),
                elapsed_ms(start)
            );
        }
        RecordsRead {
            version: self.version,
            records,
        }
    }
}

/// Record edits taken out of the open case: writing them in and reading
/// the edited case back runs without the lock.
pub struct RecordEditJob {
    version: u64,
    path: PathBuf,
    format: CaseFormat,
    name: String,
    bytes: CaseText,
    records: Arc<Records>,
    power: Result<Arc<PowerCase>, String>,
    study_edits: Vec<Edit>,
    edits: Vec<RecordEdit>,
}

/// The edited case from a [`RecordEditJob`], to put in place with
/// [`OpenCase::finish_record_edits`].
pub struct RecordEditsDone {
    version: u64,
    next: OpenCase,
    /// The cells to patch in the RAW tables (None: the model's tables are
    /// read again).
    updates: Option<Vec<record_edit::CellUpdate>>,
    count: usize,
    start: Instant,
}

impl RecordEditJob {
    pub fn run(self) -> Result<RecordEditsDone, Error> {
        let start = Instant::now();
        let records = self.records.as_ref();
        let edits = &self.edits;
        let (bytes, updates) = match records.source {
            RecordSource::Raw => {
                let (text, updates) =
                    record_edit::apply_raw(&decode_text(&self.bytes), records, edits)?;
                // Keep the file's encoding: UTF-8 if it was, else Windows-1252.
                let bytes = if std::str::from_utf8(&self.bytes).is_ok() {
                    text.into_bytes()
                } else {
                    let (encoded, _, unmappable) = encoding_rs::WINDOWS_1252.encode(&text);
                    if unmappable {
                        return Err("A new value has a character this file's encoding (Windows-1252) can't hold.".into());
                    }
                    encoded.into_owned()
                };
                (bytes, Some(updates))
            }
            RecordSource::Model => {
                let power = self.power.as_ref().map_err(|e| e.clone())?;
                let case = record_edit::apply_model(&power.case, records, edits)?;
                let text = rs_power::format_matpower(&case).map_err(|e| e.to_string())?;
                (text.into_bytes(), None)
            }
        };
        let mut next = OpenCase::from_case_bytes(&self.path, self.format, &bytes);
        if let (Ok(_), Err(e)) = (&self.power, &next.power) {
            return Err(format!("The edited case can't be solved: {e}").into());
        }
        if !self.study_edits.is_empty()
            && let Err(e) = next.set_edits(self.study_edits.clone())
        {
            log::warn!(
                "The study's edits no longer apply to the edited {}: {e}",
                self.name
            );
        }
        Ok(RecordEditsDone {
            version: self.version,
            next,
            updates,
            count: edits.len(),
            start,
        })
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
