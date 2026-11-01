//! Every command Stannum's core answers, listed once.
//!
//! The desktop app, the dev bridge and (later) the Python package all reach
//! the core through [`Session::invoke`]: a command name and its arguments as
//! JSON in, the result as JSON text out. Each command is declared once in the
//! `commands!` block below, with its arguments and what it does, so the
//! clients can't drift apart.
//!
//! Commands that work on a case take an optional `case` argument, a
//! [`CaseId`] from [`crate::CaseSummary::id`]. Left out, the command uses the open
//! case. Given, it must name the open case, so a script holding an old id gets
//! an error instead of reaching a different case.

use crate::contingency::{ContingencySettings, ContingencySummary, Progress, RunHandle};
use crate::crash::{self, relock};
use crate::error::{Error, ErrorKind};
use crate::paging::{self, PageQuery};
use crate::power::{SolveReport, SolveSettings};
use crate::record_edit::RecordEdit;
use crate::study::Edit;
use crate::workspace::{CaseId, Workspace};
use crate::{logbook, run_file, study_file};
use schemars::{JsonSchema, Schema, SchemaGenerator, generate::SchemaSettings};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::borrow::Cow;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// Version of the command set. It goes up when a command is removed or an
/// argument or result changes in a way existing callers would notice.
///
/// 2: errors are `{kind, message}` objects instead of text.
/// 3: `record_table` rows and `contingency_rows` come as columns.
/// (Paged tables, `result_buses` and the like, were added within 3.)
pub const API_VERSION: u32 = 3;

/// How many times work done without the workspace lock is redone when the
/// case changes under it, before it waits for the lock instead.
const RETRIES: usize = 3;

/// One command, as `list_commands` describes it.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
pub struct CommandInfo {
    pub name: &'static str,
    /// What it does, from its doc comment.
    #[serde(serialize_with = "write_doc")]
    pub doc: &'static str,
    /// Whether it takes a `case` argument.
    pub takes_case: bool,
    /// Whether it changes the case or writes a file, so Record writes it
    /// into a run file.
    pub action: bool,
}

/// A doc comment as plain text: lines trimmed, no trailing newline.
pub fn clean_doc(doc: &str) -> String {
    doc.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_doc<S: serde::Serializer>(doc: &&'static str, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&clean_doc(doc))
}

/// The answer to `list_commands`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CommandList {
    pub api_version: u32,
    pub commands: Vec<CommandInfo>,
}

/// A result already turned into JSON, with the schema of `T`. For results
/// that borrow the workspace, which is let go before the answer is written.
pub struct Json<T>(Value, PhantomData<T>);

impl<T> Json<T> {
    pub fn from_value(value: Value) -> Self {
        Json(value, PhantomData)
    }

    pub fn of<U: Serialize>(value: &U) -> Result<Self, Error> {
        serde_json::to_value(value)
            .map(|v| Json(v, PhantomData))
            .map_err(|e| Error::internal(e.to_string()))
    }
}

impl<T> Serialize for Json<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<T: JsonSchema> JsonSchema for Json<T> {
    fn schema_name() -> Cow<'static, str> {
        T::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        T::json_schema(generator)
    }

    fn inline_schema() -> bool {
        T::inline_schema()
    }
}

/// The workspace shared by every caller, plus the handles that stop or
/// follow long runs without waiting for the workspace lock.
#[derive(Default)]
pub struct Session {
    workspace: Mutex<Workspace>,
    /// The flag that stops the solve in progress, if one is running.
    solving: Mutex<Option<Arc<AtomicBool>>>,
    /// The contingency run in progress, if one is running.
    contingencies: Mutex<Option<RunHandle>>,
    /// The transfer limit study in progress, if one is running.
    transfers: Mutex<Option<RunHandle>>,
    /// The PV curve study in progress, if one is running.
    pv: Mutex<Option<RunHandle>>,
    qv: Mutex<Option<RunHandle>>,
    /// The secure transfer study in progress, and the last one's results
    /// file.
    secure: Mutex<Option<crate::secure::SecureHandle>>,
    secure_file: Mutex<Option<String>>,
    /// The flag that stops the run file in progress, if one is running.
    running_file: Mutex<Option<Arc<AtomicBool>>>,
    /// Sorted, filtered orders of the tables being paged through, newest
    /// last: scrolling asks for the next page of the same order.
    orders: Mutex<Vec<KeptOrder>>,
}

/// How many sorted orders [`Session`] keeps.
const ORDERS_KEPT: usize = 8;

/// A table's order for one query. `source` holds the rows' owner so its
/// address, part of `key`, can't be reused by other rows meanwhile.
struct KeptOrder {
    key: String,
    _source: Option<Arc<dyn std::any::Any + Send + Sync>>,
    order: Arc<paging::Order>,
}

/// Which Power flow table a page is cut from.
#[derive(Clone, Copy)]
struct ResultTable<R: 'static> {
    name: &'static str,
    rows: fn(&SolveReport) -> &[R],
    view: for<'a> fn(&'a SolveReport) -> paging::StructRows<'a, R>,
    /// Whether a row touches one of the buses passing the bus rules.
    touches: fn(&R, &HashSet<u32>) -> bool,
}

const BUSES: ResultTable<crate::power::BusRow> = ResultTable {
    name: "buses",
    rows: |r| &r.buses,
    view: paging::results::buses,
    touches: |b, p| p.contains(&b.id),
};
const GENERATORS: ResultTable<crate::power::GeneratorRow> = ResultTable {
    name: "generators",
    rows: |r| &r.generators,
    view: paging::results::generators,
    touches: |g, p| p.contains(&g.bus),
};
const BRANCHES: ResultTable<crate::power::BranchRow> = ResultTable {
    name: "branches",
    rows: |r| &r.branches,
    view: paging::results::branches,
    touches: |b, p| p.contains(&b.from) || p.contains(&b.to),
};

/// How [`Session::run_steps`] runs a run file.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunOptions {
    /// Run the remaining lines after one fails.
    pub keep_going: bool,
    /// Keep each command's answer in the report.
    pub results: bool,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run one command. `args` is a JSON object of its arguments (null is
    /// read as no arguments). The result is JSON text, ready to send.
    ///
    /// A bug that panics while the command runs is logged and answered as an
    /// [`ErrorKind::Internal`] error, so the caller carries on (see
    /// [`crate::crash`]).
    pub fn invoke(&self, command: &str, args: Value) -> Result<String, Error> {
        crash::catch(command, || {
            #[cfg(feature = "fault-injection")]
            crash::inject(command);
            dispatch(self, command, lift_settings(command, args)?)
        })
    }

    /// The workspace, for callers that need it directly (tests, examples).
    pub fn workspace(&self) -> Result<MutexGuard<'_, Workspace>, Error> {
        Ok(relock(&self.workspace))
    }

    fn solve(
        &self,
        case: Option<CaseId>,
        settings: SolveSettings,
    ) -> Result<crate::power::SolveReport, Error> {
        let flag = Arc::new(AtomicBool::new(false));
        *relock(&self.solving) = Some(flag.clone());
        // Only clear the slot if a newer solve hasn't taken it; also if the
        // solve panics.
        let _clear = OnDrop(|| {
            let mut slot = relock(&self.solving);
            if slot.as_ref().is_some_and(|f| Arc::ptr_eq(f, &flag)) {
                *slot = None;
            }
        });
        // The solve runs without the workspace lock, so other commands
        // keep answering meanwhile.
        self.workspace()
            .and_then(|ws| ws.case(case)?.solve_job(settings, Some(flag.clone())))
            .and_then(|job| job.run())
            .and_then(|done| self.workspace()?.case_mut(case)?.finish_solve(done))
    }

    /// The order `key` names, from the kept ones or made now (without the
    /// workspace lock or the kept orders' lock) and kept.
    fn ordered(
        &self,
        key: String,
        source: Option<Arc<dyn std::any::Any + Send + Sync>>,
        make: impl FnOnce() -> paging::Order,
    ) -> Arc<paging::Order> {
        if let Ok(kept) = self.orders.lock()
            && let Some(k) = kept.iter().find(|k| k.key == key)
        {
            return k.order.clone();
        }
        let order = Arc::new(make());
        if let Ok(mut kept) = self.orders.lock() {
            kept.retain(|k| k.key != key);
            if kept.len() >= ORDERS_KEPT {
                kept.remove(0);
            }
            kept.push(KeptOrder {
                key,
                _source: source,
                order: order.clone(),
            });
        }
        order
    }

    /// A page of one of a result's Power flow tables. `bus_rules` keep the
    /// rows touching a bus that passes them (the filter chips).
    fn result_page<R: Serialize + JsonSchema + Clone + Send + Sync + 'static>(
        &self,
        case: Option<CaseId>,
        name: &str,
        table: ResultTable<R>,
        query: PageQuery,
        bus_rules: Vec<paging::Rule>,
    ) -> Result<paging::Page<R>, Error> {
        let report = self.workspace()?.case_mut(case)?.shared_result(name)?;
        let key = format!(
            "result {}|{:p}|{}|{}",
            table.name,
            Arc::as_ptr(&report),
            query.order_key(),
            serde_json::to_string(&bus_rules).unwrap_or_default()
        );
        let source: Arc<dyn std::any::Any + Send + Sync> = report.clone();
        let order = self.ordered(key, Some(source), || {
            let passing = paging::results::passing(&report, &bus_rules);
            let rows = (table.rows)(&report);
            let touches = |i: usize| {
                passing
                    .as_ref()
                    .is_none_or(|p| (table.touches)(&rows[i], p))
            };
            let extra: Option<&dyn Fn(usize) -> bool> = if passing.is_some() {
                Some(&touches)
            } else {
                None
            };
            paging::order(&(table.view)(&report), &query, extra)
        });
        Ok(paging::Page::cut((table.rows)(&report), &order, &query))
    }

    /// The case's records, read first if they haven't been.
    fn records(&self, case: Option<CaseId>) -> Result<Arc<crate::records::Records>, Error> {
        self.read_records(case)?;
        self.workspace()?.case_mut(case)?.shared_records()
    }

    fn record_page(
        &self,
        case: Option<CaseId>,
        section: &str,
        query: PageQuery,
    ) -> Result<crate::records::RecordPage, Error> {
        let records = self.records(case)?;
        let table = records.table(section)?;
        let key = format!(
            "records {section}|{:p}|{}",
            Arc::as_ptr(&records),
            query.order_key()
        );
        let source: Arc<dyn std::any::Any + Send + Sync> = records.clone();
        let order = self.ordered(key, Some(source), || paging::order(table, &query, None));
        Ok(crate::records::RecordPage {
            id: table.id.clone(),
            label: table.label.clone(),
            columns: table.columns.clone(),
            skipped: table.skipped,
            note: table.note.clone(),
            page: paging::Page::cut(&table.rows, &order, &query),
        })
    }

    fn contingency_page(
        &self,
        case: Option<CaseId>,
        query: PageQuery,
    ) -> Result<paging::Page<crate::contingency::ContingencyRow>, Error> {
        let ws = self.workspace()?;
        let results = ws
            .case(case)?
            .contingencies
            .as_ref()
            .ok_or_else(|| Error::not_ready("Run contingencies first."))?;
        let key = format!("contingencies|{}|{}", results.id, query.order_key());
        let order = self.ordered(key, None, || {
            let rows = paging::StructRows {
                rows: &results.rows,
                element: |r: &crate::contingency::ContingencyRow| Some(r.element.clone()),
            };
            paging::order(&rows, &query, None)
        });
        Ok(paging::Page::cut(&results.rows, &order, &query))
    }

    fn cancel_solve(&self) {
        if let Some(flag) = relock(&self.solving).clone() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// The run itself doesn't hold the workspace, so other commands keep
    /// working and Cancel can reach it.
    fn run_contingencies(
        &self,
        case: Option<CaseId>,
        settings: ContingencySettings,
    ) -> Result<ContingencySummary, Error> {
        let (job, version) = {
            let ws = self.workspace()?;
            let open = ws.case(case)?;
            (open.contingency_job(settings)?, open.version)
        };
        *relock(&self.contingencies) = Some(job.handle());
        // Cleared however the run ends, so a crashed run doesn't look busy.
        let clear = OnDrop(|| *relock(&self.contingencies) = None);
        let results = job.run();
        drop(clear);
        let results = results?;
        self.workspace()?
            .case_mut(case)?
            .keep_contingencies(version, results)
    }

    /// A transfer limit study, run without the workspace lock.
    fn transfer_limits(
        &self,
        case: Option<CaseId>,
        settings: crate::transfer::TransferSettings,
    ) -> Result<crate::transfer::TransferResults, Error> {
        let job = self.workspace()?.case(case)?.transfer_job(settings)?;
        *relock(&self.transfers) = Some(job.handle());
        let clear = OnDrop(|| *relock(&self.transfers) = None);
        let results = job.run();
        drop(clear);
        Ok(results?)
    }

    /// A PV curve study, run without the workspace lock.
    fn pv_curve(
        &self,
        case: Option<CaseId>,
        settings: crate::pv::PvSettings,
    ) -> Result<crate::pv::PvResults, Error> {
        let job = self.workspace()?.case(case)?.pv_job(settings)?;
        *relock(&self.pv) = Some(job.handle());
        let clear = OnDrop(|| *relock(&self.pv) = None);
        let results = job.run();
        drop(clear);
        Ok(results?)
    }

    /// A QV curve study, run without the workspace lock.
    fn qv_curve(
        &self,
        case: Option<CaseId>,
        settings: crate::qv::QvSettings,
    ) -> Result<crate::qv::QvResults, Error> {
        let job = self.workspace()?.case(case)?.qv_job(settings)?;
        *relock(&self.qv) = Some(job.handle());
        let clear = OnDrop(|| *relock(&self.qv) = None);
        let results = job.run();
        drop(clear);
        Ok(results?)
    }

    /// A secure transfer study, run without the workspace lock.
    fn secure_transfer(
        &self,
        case: Option<CaseId>,
        settings: crate::secure::SecureTransferSettings,
    ) -> Result<crate::secure::SecureResults, Error> {
        let job = self
            .workspace()?
            .case(case)?
            .secure_transfer_job(settings)?;
        *relock(&self.secure) = Some(job.handle());
        let clear = OnDrop(|| *relock(&self.secure) = None);
        let results = job.run();
        drop(clear);
        let results = results?;
        *relock(&self.secure_file) = Some(results.summary.results_file.clone());
        Ok(results)
    }

    /// One channel's curves from a secure transfer results file: the last
    /// run's when `path` is absent.
    fn secure_transfer_channel(
        &self,
        path: Option<String>,
        channel: usize,
        contingencies: Option<Vec<usize>>,
    ) -> Result<crate::secure::ChannelCurves, Error> {
        let path = match path.or_else(|| relock(&self.secure_file).clone()) {
            Some(p) => p,
            None => return Err(Error::not_ready("Run a secure transfer study first.")),
        };
        Ok(crate::secure::read_channel(
            &path,
            channel,
            contingencies.as_deref(),
        )?)
    }

    fn cancel_secure(&self) {
        if let Some(handle) = relock(&self.secure).clone() {
            handle.cancel();
        }
    }

    fn secure_progress(&self) -> crate::secure::SecureProgress {
        relock(&self.secure)
            .as_ref()
            .map(crate::secure::SecureHandle::progress)
            .unwrap_or_default()
    }

    /// Sensitivities, calculated without the workspace lock.
    fn sensitivities(
        &self,
        case: Option<CaseId>,
        settings: SolveSettings,
    ) -> Result<crate::power::SensitivitySummary, Error> {
        let job = self.workspace()?.case(case)?.sensitivity_job(settings)?;
        let done = job.run()?;
        self.workspace()?.case_mut(case)?.finish_sensitivities(done)
    }

    /// "Trip and compare", solved without the workspace lock.
    fn trip_and_compare(
        &self,
        case: Option<CaseId>,
        element: &str,
        limit_pct: f64,
        ratings: (crate::contingency::Rating, crate::contingency::Rating),
    ) -> Result<crate::contingency::ContingencyDetail, Error> {
        let job = self
            .workspace()?
            .case(case)?
            .trip_job(element, limit_pct, ratings)?;
        let done = job.run()?;
        self.workspace()?.case_mut(case)?.finish_trip(done)
    }

    /// Read the case's records the first time they're asked for, without
    /// the workspace lock. If the case changes meanwhile, read them again.
    fn read_records(&self, case: Option<CaseId>) -> Result<(), Error> {
        for _ in 0..RETRIES {
            let Some(job) = self.workspace()?.case(case)?.records_job() else {
                return Ok(());
            };
            let read = job.run();
            match self.workspace()?.case_mut(case)?.keep_records(read) {
                Err(e) if e.is(ErrorKind::CaseChanged) => continue,
                kept => return kept,
            }
        }
        // Still changing: the caller reads them under the lock.
        Ok(())
    }

    /// Write cell edits into the case and read the edited case back
    /// without the workspace lock. If the case changed meanwhile, the edits
    /// are written again into the case as it is now, as if they had waited
    /// their turn.
    fn apply_record_edits(
        &self,
        case: Option<CaseId>,
        edits: &[RecordEdit],
    ) -> Result<crate::CaseSummary, Error> {
        if edits.is_empty() {
            return Ok(self.workspace()?.case(case)?.summary());
        }
        for _ in 0..RETRIES {
            self.read_records(case)?;
            let job = self.workspace()?.case_mut(case)?.record_edit_job(edits)?;
            let done = job.run()?;
            match self.workspace()?.case_mut(case)?.finish_record_edits(done) {
                Err(e) if e.is(ErrorKind::CaseChanged) => continue,
                applied => return applied,
            }
        }
        // Still changing: apply them under the lock.
        self.workspace()?.case_mut(case)?.apply_record_edits(edits)
    }

    /// Work out a scaling on the case as filed, with its Records edits.
    fn scale_plan(
        &self,
        case: Option<CaseId>,
        settings: &crate::scaling::ScaleSettings,
    ) -> Result<crate::scaling::ScaleReport, Error> {
        let records = self.records(case)?;
        if !records.editable {
            return Err(Error::unsupported(
                "This case's records can't be edited, so it can't be scaled: Stannum writes RAW revisions 33 and 35, and MATPOWER.",
            ));
        }
        let power = self.workspace()?.case(case)?.power.clone()?;
        Ok(crate::scaling::plan(&records, &power, settings)?)
    }

    /// Run a run file's steps in order, telling `each` how every step went
    /// as it finishes. Stops at the first failure unless `keep_going`, and
    /// before the next step once `cancel_run_file` is called.
    pub fn run_steps(
        &self,
        path: &std::path::Path,
        steps: &[run_file::Step],
        options: RunOptions,
        mut each: impl FnMut(&run_file::StepOutcome),
    ) -> Result<run_file::RunReport, Error> {
        let flag = Arc::new(AtomicBool::new(false));
        {
            let mut slot = relock(&self.running_file);
            if slot.is_some() {
                return Err(Error::not_ready(
                    "A run file is already running. Wait for it, or cancel it first.",
                ));
            }
            *slot = Some(flag.clone());
        }
        let clear = OnDrop(|| *relock(&self.running_file) = None);
        let started = Instant::now();
        let mut report = run_file::RunReport {
            path: path.display().to_string(),
            steps: steps.len(),
            outcomes: Vec::new(),
            failed: 0,
            cancelled: false,
            seconds: 0.0,
        };
        log::info!("Running {} ({} steps)", path.display(), steps.len());
        for step in steps {
            if flag.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            let at = Instant::now();
            let answer = self.invoke(&step.command, Value::Object(step.args.clone()));
            let outcome = run_file::StepOutcome {
                line: step.line,
                command: step.command.clone(),
                seconds: at.elapsed().as_secs_f64(),
                result: match (&answer, options.results) {
                    (Ok(text), true) => serde_json::from_str(text).ok(),
                    _ => None,
                },
                error: answer.err(),
            };
            match &outcome.error {
                Some(e) if e.is(ErrorKind::Cancelled) => report.cancelled = true,
                Some(e) => {
                    report.failed += 1;
                    log::warn!("Line {}: {}: {}", step.line, step.command, e.message);
                }
                None => log::debug!(
                    "Line {}: {} ({:.2} s)",
                    step.line,
                    step.command,
                    outcome.seconds
                ),
            }
            let stop = outcome.error.is_some() && (!options.keep_going || report.cancelled);
            each(&outcome);
            report.outcomes.push(outcome);
            if stop {
                break;
            }
        }
        drop(clear);
        report.seconds = started.elapsed().as_secs_f64();
        let ran = report.outcomes.len();
        if report.cancelled {
            log::warn!(
                "Cancelled {} after {ran} of {} steps",
                path.display(),
                report.steps
            );
        } else if report.failed > 0 {
            log::warn!(
                "{}: {} of {ran} steps failed ({} not run)",
                path.display(),
                report.failed,
                report.steps - ran
            );
        } else {
            log::info!(
                "Ran {} ({ran} steps, {:.1} s)",
                path.display(),
                report.seconds
            );
        }
        Ok(report)
    }

    fn run_file(
        &self,
        path: &std::path::Path,
        options: RunOptions,
    ) -> Result<run_file::RunReport, Error> {
        let steps = run_file::read(path)?;
        self.run_steps(path, &steps, options, |_| {})
    }

    /// Stops the run file in progress before its next step, and the solve
    /// or contingency run it is waiting on.
    fn cancel_run_file(&self) {
        let running = relock(&self.running_file).clone();
        if let Some(flag) = running {
            flag.store(true, Ordering::Relaxed);
            self.cancel_solve();
            self.cancel_contingencies();
            self.cancel_transfers();
            self.cancel_pv();
            self.cancel_qv();
            self.cancel_secure();
        }
    }

    fn cancel_contingencies(&self) {
        if let Some(handle) = relock(&self.contingencies).clone() {
            handle.cancel();
        }
    }

    fn cancel_transfers(&self) {
        if let Some(handle) = relock(&self.transfers).clone() {
            handle.cancel();
        }
    }

    fn cancel_pv(&self) {
        if let Some(handle) = relock(&self.pv).clone() {
            handle.cancel();
        }
    }

    fn cancel_qv(&self) {
        if let Some(handle) = relock(&self.qv).clone() {
            handle.cancel();
        }
    }

    fn qv_progress(&self) -> Progress {
        relock(&self.qv)
            .as_ref()
            .map(RunHandle::progress)
            .unwrap_or_default()
    }

    fn pv_progress(&self) -> Progress {
        relock(&self.pv)
            .as_ref()
            .map(RunHandle::progress)
            .unwrap_or_default()
    }

    fn transfer_progress(&self) -> Progress {
        relock(&self.transfers)
            .as_ref()
            .map(RunHandle::progress)
            .unwrap_or_default()
    }

    fn contingency_progress(&self) -> Progress {
        relock(&self.contingencies)
            .as_ref()
            .map(RunHandle::progress)
            .unwrap_or_default()
    }
}

/// Runs its closure when dropped: on return and on panic alike.
struct OnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}

/// Commands that take their solve settings one by one as well as in
/// `settings`: `solve_power_flow method=newton tolerance_mva=0.5`.
const FLAT_SETTINGS: &[&str] = &["solve_power_flow", "calculate_sensitivities"];

/// The settings `command` also takes one by one, beside its `settings`
/// argument (snake_case names); empty for other commands.
pub fn flat_settings(command: &str) -> &'static [String] {
    static SOLVE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    if !FLAT_SETTINGS.contains(&command) {
        return &[];
    }
    SOLVE.get_or_init(|| match serde_json::to_value(SolveSettings::default()) {
        Ok(Value::Object(fields)) => fields.keys().cloned().collect(),
        _ => Vec::new(),
    })
}

/// Moves the settings given one by one (snake_case or camelCase) into
/// `settings`. One given both ways is an error, not a quiet pick.
pub fn lift_settings(command: &str, args: Value) -> Result<Value, Error> {
    let names = flat_settings(command);
    let Value::Object(mut fields) = args else {
        return Ok(args);
    };
    let mut lifted = Map::new();
    for name in names {
        let camel = crate::run_file::camel(name);
        for key in [name.as_str(), camel.as_str()] {
            if let Some(value) = fields.remove(key)
                && lifted.insert(name.clone(), value).is_some()
            {
                return Err(Error::invalid_arguments(format!(
                    "{command}: {name} is given twice."
                )));
            }
            if camel == *name {
                break;
            }
        }
    }
    if lifted.is_empty() {
        return Ok(Value::Object(fields));
    }
    let settings = fields
        .entry("settings")
        .or_insert_with(|| Value::Object(Map::new()));
    if settings.is_null() {
        *settings = Value::Object(Map::new());
    }
    let Value::Object(settings) = settings else {
        return Err(Error::invalid_arguments(format!(
            "{command}: settings must be an object."
        )));
    };
    for (name, value) in lifted {
        if settings.contains_key(&name) {
            return Err(Error::invalid_arguments(format!(
                "{command}: {name} is given both on its own and in settings."
            )));
        }
        settings.insert(name, value);
    }
    Ok(Value::Object(fields))
}

/// Copies the properties of the `settings` argument's type into the
/// arguments themselves, for a command in [`FLAT_SETTINGS`].
fn flatten_settings_schema(args: &mut Value, defs: &Map<String, Value>) {
    fn reference(v: &Value) -> Option<&str> {
        v.get("$ref")
            .and_then(Value::as_str)
            .or_else(|| v.get("anyOf")?.as_array()?.iter().find_map(reference))
    }
    let Some(name) =
        reference(&args["properties"]["settings"]).and_then(|r| r.strip_prefix("#/$defs/"))
    else {
        return;
    };
    let Some(Value::Object(fields)) = defs.get(name).and_then(|d| d.get("properties")).cloned()
    else {
        return;
    };
    if let Some(Value::Object(properties)) = args.get_mut("properties") {
        for (field, schema) in fields {
            properties.entry(field).or_insert(schema);
        }
    }
}

fn parse<T: serde::de::DeserializeOwned>(command: &str, args: Value) -> Result<T, Error> {
    let args = if args.is_null() {
        Value::Object(Default::default())
    } else {
        args
    };
    serde_json::from_value(args).map_err(|e| Error::invalid_arguments(format!("{command}: {e}")))
}

fn json<T: Serialize>(value: Result<T, Error>) -> Result<String, Error> {
    serde_json::to_string(&value?).map_err(|e| Error::internal(e.to_string()))
}

/// Where argument types' definitions go before they join the result
/// types' (see [`crate::schema::merge_definitions`]).
const INPUTS: &str = "/$inputs";

/// The generators the schemas come from (JSON Schema 2020-12, named types
/// under `$defs`): one for what the core reads, where a field with a default
/// may be left out, and one for what it writes, where every field is there.
fn schema_generators() -> (SchemaGenerator, SchemaGenerator) {
    let mut inputs = SchemaSettings::draft2020_12().for_deserialize();
    inputs.definitions_path = INPUTS.into();
    let outputs = SchemaSettings::draft2020_12().for_serialize();
    (inputs.into_generator(), outputs.into_generator())
}

/// Declares each command once: its doc, its arguments (camelCase in JSON),
/// its result type and its body. A command whose arguments include `case`
/// is case-scoped.
macro_rules! commands {
    ($(
        $(#[doc = $doc:literal])*
        $name:ident { $($field:ident : $ty:ty),* $(,)? } -> $ret:ty
        => |$s:pat_param, $a:pat_param| $body:expr;
    )*) => {
        #[allow(non_camel_case_types, dead_code)]
        mod args {
            use super::*;
            $(
                #[derive(Deserialize, JsonSchema)]
                #[serde(rename_all = "camelCase", deny_unknown_fields)]
                pub struct $name { $(pub $field: $ty),* }
            )*
        }

        /// Every command, in the order declared.
        pub const COMMANDS: &[CommandInfo] = &[$(
            CommandInfo {
                name: stringify!($name),
                doc: concat!($($doc, "\n",)*),
                takes_case: false $(|| matches!(stringify!($field).as_bytes(), b"case"))*,
                action: crate::run_file::is_action(stringify!($name)),
            },
        )*];

        /// Each command's arguments, as declared (snake_case), and whether
        /// each is a file path. Same order as [`COMMANDS`].
        const ARGS: &[&[(&str, bool)]] = &[$(&[$(
            (stringify!($field), matches!(stringify!($ty).as_bytes(), b"PathBuf")),
        )*],)*];

        fn dispatch(session: &Session, command: &str, raw: Value) -> Result<String, Error> {
            match command {
                $(stringify!($name) => {
                    let $a: args::$name = parse(command, raw)?;
                    let $s = session;
                    let result: Result<$ret, Error> = $body;
                    json(result)
                })*
                other => Err(Error::new(
                    ErrorKind::UnknownCommand,
                    format!("Stannum has no command called {other}."),
                )),
            }
        }

        /// A command's arguments, as declared (snake_case), and whether each
        /// is a file path; None if there is no such command.
        pub fn command_args(command: &str) -> Option<&'static [(&'static str, bool)]> {
            COMMANDS.iter().position(|c| c.name == command).map(|i| ARGS[i])
        }

        /// Every command's arguments and result as JSON Schema, with the
        /// error every command can return. Named types are under `$defs`.
        pub fn schemas() -> Value {
            let (mut inputs, mut outputs) = schema_generators();
            let mut commands = Map::new();
            $(
                let mut arguments = <args::$name as JsonSchema>::json_schema(&mut inputs);
                // The argument struct is named after the command; the
                // command's own entry already says that.
                arguments.remove("title");
                commands.insert(stringify!($name).into(), json!({
                    "doc": clean_doc(concat!($($doc, "\n",)*)),
                    "takes_case": (false $(|| matches!(stringify!($field).as_bytes(), b"case"))*),
                    "action": crate::run_file::is_action(stringify!($name)),
                    "args": arguments,
                    "result": outputs.subschema_for::<$ret>(),
                }));
            )*
            let error = outputs.subschema_for::<Error>();
            let mut defs = outputs.take_definitions(true);
            let prefix = format!("#{INPUTS}/");
            let mapping = crate::schema::merge_definitions(
                &mut defs,
                inputs.take_definitions(true),
                &prefix,
            );
            for command in commands.values_mut() {
                crate::schema::rewrite_refs(&mut command["args"], &prefix, &mapping);
            }
            // Settings taken one by one list beside `settings` with the
            // same schema as inside it.
            for (name, command) in commands.iter_mut() {
                if !flat_settings(name).is_empty() {
                    flatten_settings_schema(&mut command["args"], &defs);
                }
            }
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "title": "Stannum commands",
                "api_version": API_VERSION,
                "element_kinds": crate::elements::KINDS,
                "commands": commands,
                "error": error,
                "$defs": defs,
            })
        }
    };
}

commands! {
    /// The commands this version answers, and the API version.
    list_commands {} -> CommandList => |_, _| Ok(CommandList {
        api_version: API_VERSION,
        commands: COMMANDS.to_vec(),
    });

    /// Every command's arguments and result, and the error, as JSON Schema
    /// (2020-12), for clients that check calls or generate types.
    command_schemas {} -> Value => |_, _| Ok(schemas());

    /// Open a case file (PSS/E RAW or MATPOWER). It replaces the open case.
    open_case { path: PathBuf } -> crate::CaseSummary => |s, a| s.workspace()?.open(&a.path);

    /// The small cases built into Stannum, to open with `open_sample`.
    sample_cases {} -> Vec<crate::samples::SampleCase> => |_, _| Ok(crate::samples::list());

    /// Write a built-in sample case (and its bus coordinates, if it has
    /// any) into `dir`, or a `stannum-samples` folder in the temp folder,
    /// and open it. It replaces the open case.
    open_sample { id: String, dir: Option<PathBuf> } -> crate::CaseSummary =>
        |s, a| crate::samples::open(&mut *s.workspace()?, &a.id, a.dir.as_deref());

    /// Close the case.
    close_case { case: Option<CaseId> } -> () => |s, a| {
        let mut ws = s.workspace()?;
        ws.case(a.case)?;
        ws.close();
        Ok(())
    };

    /// The open case's summary, or null when none is open.
    case_summary { case: Option<CaseId> } -> Option<crate::CaseSummary> => |s, a| {
        let ws = s.workspace()?;
        match (&ws.case, a.case) {
            (None, None) => Ok(None),
            _ => ws.case(a.case).map(|c| Some(c.summary())),
        }
    };

    /// Solve the power flow with the study's edits. `cancel_solve` stops it.
    /// Settings go in `settings` or one by one beside it (`method`,
    /// `tolerance_mva`, `start`, `enforce_q_limits`, `switch_shunts`, ...);
    /// settings left out take their defaults.
    solve_power_flow { case: Option<CaseId>, settings: Option<SolveSettings> } -> crate::power::SolveReport =>
        |s, a| s.solve(a.case, a.settings.unwrap_or_default());

    /// Compare the latest solve with the voltages saved in the case file
    /// (the solution of the tool that last saved it, such as PSS/E or
    /// PowerWorld): summary statistics and the `limit` largest differences
    /// (200 by default).
    compare_file_voltages { case: Option<CaseId>, limit: Option<usize> } -> crate::file_voltages::FileVoltageComparison =>
        |s, a| s.workspace()?.case(a.case)?.compare_file_voltages(a.limit.unwrap_or(200));

    /// Stop the solve in progress; the case keeps its previous results.
    cancel_solve {} -> () => |s, _| {
        s.cancel_solve();
        Ok(())
    };

    /// Attach a Stannum DC grid file to the case.
    load_dc_grids { case: Option<CaseId>, path: PathBuf } -> crate::dc_grids::DcGridsSummary =>
        |s, a| s.workspace()?.case_mut(a.case)?.load_dc_grids(&a.path);

    /// Detach the DC grid file.
    clear_dc_grids { case: Option<CaseId> } -> () => |s, a| {
        s.workspace()?.case_mut(a.case)?.clear_dc_grids();
        Ok(())
    };

    /// Attach a bus coordinates CSV (bus, latitude, longitude, optional
    /// substation) to the case, for the map.
    load_coordinates { case: Option<CaseId>, path: PathBuf } -> crate::geo::CoordinatesSummary =>
        |s, a| s.workspace()?.case_mut(a.case)?.load_coordinates(&a.path);

    /// Detach the bus coordinates file.
    clear_coordinates { case: Option<CaseId> } -> () => |s, a| {
        s.workspace()?.case_mut(a.case)?.clear_coordinates();
        Ok(())
    };

    /// Where every bus goes on the map, as columns in bus order, with the
    /// source of each place ("file", "estimated" or "none").
    bus_coordinates { case: Option<CaseId> } -> crate::geo::BusCoordinates =>
        |s, a| s.workspace()?.case(a.case)?.bus_coordinates();

    /// Replace the study's edits; every later solve uses them.
    set_study_edits { case: Option<CaseId>, edits: Vec<Edit> } -> () =>
        |s, a| s.workspace()?.case_mut(a.case)?.set_edits(a.edits);

    /// The study's edits, as `set_study_edits` last set them.
    study_edits { case: Option<CaseId> } -> Vec<Edit> =>
        |s, a| Ok(s.workspace()?.case(a.case)?.edits.clone());

    /// Calculate sensitivities at the solved operating point. Takes the same
    /// settings as `solve_power_flow`, in `settings` or one by one.
    calculate_sensitivities { case: Option<CaseId>, settings: Option<SolveSettings> } -> crate::power::SensitivitySummary =>
        |s, a| s.sensitivities(a.case, a.settings.unwrap_or_default());

    /// One sensitivity column's responses at every bus.
    sensitivity_responses {
        case: Option<CaseId>,
        column: usize,
        quantity: String,
        engineering: bool,
    } -> crate::power::ResponseTable => |s, a| s.workspace()?.case(a.case)?.responses(a.column, &a.quantity, a.engineering);

    /// Write the case, solved or as filed, as a MATPOWER file.
    export_matpower { case: Option<CaseId>, path: PathBuf, solved: bool } -> () =>
        |s, a| s.workspace()?.case(a.case)?.export_matpower(&a.path, a.solved);

    /// Write the case, with its Records and study edits, as a complete PSS/E
    /// RAW file of revision 33 or 35. The answer lists anything the file
    /// couldn't keep.
    export_raw { case: Option<CaseId>, path: PathBuf, revision: u32 } -> crate::raw_export::RawExport =>
        |s, a| s.workspace()?.case_mut(a.case)?.export_raw(&a.path, a.revision);

    /// The case as a drawable network, for one-line diagrams.
    diagram_network { case: Option<CaseId> } -> crate::diagram::CaseNetwork =>
        |s, a| s.workspace()?.case(a.case)?.diagram_network();

    /// Write the case, with its Records and study edits, as a Stannum case
    /// file: binary when `path` ends in `.stnb`, else text
    /// (`.stn`). The answer lists anything converted or kept as RAW lines.
    export_stn { case: Option<CaseId>, path: PathBuf } -> crate::stn::StnExport =>
        |s, a| s.workspace()?.case_mut(a.case)?.export_stn(&a.path);

    /// One element's data and limits from the file.
    element_detail { case: Option<CaseId>, element: String } -> crate::inspect::ElementDetail => |s, a| {
        // Shunts, FACTS devices and the like are read from their records.
        let from_record = crate::elements::kind_of(&a.element)
            .is_some_and(|k| k.inspector == crate::elements::Source::Record);
        if from_record {
            s.read_records(a.case)?;
            s.workspace()?.case_mut(a.case)?.records()?;
        }
        s.workspace()?.case(a.case)?.element_detail(&a.element)
    };

    /// Every kind of element id: its format, the RAW section and MATPOWER
    /// table it comes from, and where the inspector reads it.
    element_kinds {} -> Vec<crate::elements::ElementKind> =>
        |_, _| Ok(crate::elements::KINDS.to_vec());

    /// The case's record types and their counts.
    case_records { case: Option<CaseId> } -> crate::records::RecordBook => |s, a| {
        s.read_records(a.case)?;
        s.workspace()?.case_mut(a.case)?.record_book()
    };

    /// Every record of one type, with every field.
    record_table { case: Option<CaseId>, section: String } -> crate::records::RecordTable => |s, a| {
        s.read_records(a.case)?;
        s.workspace()?.case_mut(a.case)?.record_table(&a.section)
    };

    /// A page of one record type, sorted, searched and filtered in the core
    /// (fields are column positions, "line" and "element").
    record_page { case: Option<CaseId>, section: String, query: Option<PageQuery> } -> crate::records::RecordPage =>
        |s, a| s.record_page(a.case, &a.section, a.query.unwrap_or_default());

    /// How each column of one record type filters: by number, by text, or
    /// by picking one of its few values.
    record_fields { case: Option<CaseId>, section: String } -> Vec<paging::FieldType> =>
        |s, a| Ok(paging::record_field_types(s.records(a.case)?.table(&a.section)?));

    /// The distinct values of one column of a record type, sorted, missing
    /// last: the choices its filter offers. At most `limit` (500 when left out).
    record_values { case: Option<CaseId>, section: String, field: String, limit: Option<usize> } -> Vec<Value> =>
        |s, a| Ok(paging::distinct(s.records(a.case)?.table(&a.section)?, &a.field, a.limit.unwrap_or(500)));

    /// Write cell edits into the case and reopen it in place.
    apply_record_edits { case: Option<CaseId>, edits: Vec<RecordEdit> } -> crate::CaseSummary =>
        |s, a| s.apply_record_edits(a.case, &a.edits);

    /// What `scale_load_generation` would change, without changing anything.
    scale_preview { case: Option<CaseId>, settings: crate::scaling::ScaleSettings } -> crate::scaling::ScaleReport =>
        |s, a| s.scale_plan(a.case, &a.settings);

    /// Scale load and generation at a set of buses (areas, zones, owners,
    /// buses and a kV range) to a target total or by a change in MW or
    /// percent, and write the new values into the case's records.
    scale_load_generation { case: Option<CaseId>, settings: crate::scaling::ScaleSettings } -> crate::scaling::Scaled =>
        |s, a| {
            let report = s.scale_plan(a.case, &a.settings)?;
            let summary = s.apply_record_edits(a.case, &report.edits)?;
            Ok(crate::scaling::Scaled { report, case: summary })
        };

    /// Save the case, with its edits, to a new file.
    save_case_as { case: Option<CaseId>, path: PathBuf } -> crate::CaseSummary =>
        |s, a| s.workspace()?.case_mut(a.case)?.save_as(&a.path);

    /// Islands, bridges and articulation buses, with the case's graph, as
    /// compact columns (see `topology_wire`). Source fields are left out:
    /// `topology_source` sends one element's.
    topology_snapshot { case: Option<CaseId> } -> Json<crate::topology_wire::TopologyWire<'static>> =>
        |s, a| Ok(Json::from_value(s.workspace()?.case(a.case)?.topology_wire()?));

    /// The record an element was read from: its fields and RAW lines, or null.
    topology_source { case: Option<CaseId>, element: String } -> Option<nodal::model::SourceRecord> =>
        |s, a| s.workspace()?.case(a.case)?.topology_source(&a.element);

    /// Merge buses joined by zero-impedance lines, or split them again. The
    /// answer is the new analysis and graph in the same form as `topology_snapshot`.
    topology_set_merge { case: Option<CaseId>, enabled: bool } -> Json<crate::topology_wire::TopologyViewWire<'static>> =>
        |s, a| Ok(Json::from_value(s.workspace()?.case_mut(a.case)?.set_merge_zero_impedance_wire(a.enabled)?));

    /// The shortest path between two buses, or null.
    topology_shortest_path { case: Option<CaseId>, from: i32, to: i32 } -> Option<nodal::topology::Path> =>
        |s, a| s.workspace()?.case(a.case)?.shortest_path(a.from, a.to);

    /// Compare two RAW files record by record.
    diff_compare { baseline: PathBuf, comparison: PathBuf } -> crate::diff::DiffSummary =>
        |s, a| s.workspace()?.compare(&a.baseline, &a.comparison);

    /// Compare the case's file with the case as it is being worked on.
    diff_compare_working { case: Option<CaseId> } -> crate::diff::DiffSummary => |s, a| {
        let mut ws = s.workspace()?;
        ws.case(a.case)?;
        ws.compare_working()
    };

    /// The last comparison's records of one type.
    diff_section_records { section: String } -> Vec<crate::diff::RecordRow> =>
        |s, a| Ok(s.workspace()?.comparison()?.section_records(&a.section));

    /// One record of the last comparison, field by field.
    diff_record_detail { section: String, index: usize } -> Option<crate::diff::RecordDetail> =>
        |s, a| Ok(s.workspace()?.comparison()?.record_detail(&a.section, a.index));

    /// Run N-1 contingencies, or with `settings.planning` the NERC TPL-001
    /// planning events. `cancel_contingencies` stops the run and
    /// `contingency_progress` follows it. Settings left out take their
    /// defaults: every branch and generator, screened, then the worst in AC.
    run_contingencies { case: Option<CaseId>, settings: Option<ContingencySettings> } -> ContingencySummary =>
        |s, a| s.run_contingencies(a.case, a.settings.unwrap_or_default());

    /// How many NERC TPL-001 planning events each category gives on the
    /// open case, before running them with `run_contingencies`. Settings
    /// left out take their defaults: every category Stannum generates, at
    /// 100 kV and above.
    planning_event_counts { case: Option<CaseId>, settings: Option<crate::planning::PlanningSettings> } -> Vec<crate::planning::CategoryCount> =>
        |s, a| s.workspace()?.case(a.case)?.planning_event_counts(&a.settings.unwrap_or_default());

    /// Stop the contingency run in progress.
    cancel_contingencies {} -> () => |s, _| {
        s.cancel_contingencies();
        Ok(())
    };

    /// How far the contingency run in progress has got.
    contingency_progress {} -> Progress => |s, _| Ok(s.contingency_progress());

    /// Transfer limits (DFAX and FCITC): how much power can move from the
    /// source subsystem to the sink before a monitored branch or interface
    /// reaches its limit, in the base case and after each contingency.
    /// Linear factors on top of the AC solve. `cancel_transfer_limits` stops
    /// the study and `transfer_progress` follows it.
    run_transfer_limits { case: Option<CaseId>, settings: crate::transfer::TransferSettings } -> crate::transfer::TransferResults =>
        |s, a| s.transfer_limits(a.case, a.settings);

    /// Stop the transfer limit study in progress.
    cancel_transfer_limits {} -> () => |s, _| {
        s.cancel_transfers();
        Ok(())
    };

    /// How far the transfer limit study in progress has got.
    transfer_progress {} -> Progress => |s, _| Ok(s.transfer_progress());

    /// PV curve (continuation power flow): grows a transfer, or the sink's
    /// load, step by step with full AC solves from the last AC solve until
    /// the network can't carry more (the nose), and records the monitored
    /// bus voltages on the way. `cancel_pv_curve` stops it and
    /// `pv_progress` counts its solves.
    run_pv_curve { case: Option<CaseId>, settings: crate::pv::PvSettings } -> crate::pv::PvResults =>
        |s, a| s.pv_curve(a.case, a.settings);

    /// Stop the PV curve study in progress.
    cancel_pv_curve {} -> () => |s, _| {
        s.cancel_pv();
        Ok(())
    };

    /// How far the PV curve study in progress has got: solves so far.
    pv_progress {} -> Progress => |s, _| Ok(s.pv_progress());

    /// QV curves: a fictitious condenser holds each bus at one voltage
    /// after another, from the last AC solve's voltage up and down, with
    /// full AC solves; its reactive output at each voltage is the curve and
    /// the depth of the curve's bottom is the bus's reactive power margin.
    /// `cancel_qv_curve` stops it and `qv_progress` counts the buses done.
    run_qv_curve { case: Option<CaseId>, settings: crate::qv::QvSettings } -> crate::qv::QvResults =>
        |s, a| s.qv_curve(a.case, a.settings);

    /// Stop the QV curve study in progress.
    cancel_qv_curve {} -> () => |s, _| {
        s.cancel_qv();
        Ok(())
    };

    /// How far the QV curve study in progress has got: buses done.
    qv_progress {} -> Progress => |s, _| Ok(s.qv_progress());

    /// Secure transfer limits: grows a transfer from source to sink by the
    /// major step, solving every contingency in AC at each step, until a
    /// new thermal or voltage problem (as chosen) or a failed solve; then
    /// pins it down with the minor step. Records the chosen channels for
    /// the base case and every contingency at each step in a results file
    /// (`.stnr`). `cancel_secure_transfer` stops it and
    /// `secure_transfer_progress` follows it.
    run_secure_transfer { case: Option<CaseId>, settings: crate::secure::SecureTransferSettings } -> crate::secure::SecureResults =>
        |s, a| s.secure_transfer(a.case, a.settings);

    /// Stop the secure transfer study in progress.
    cancel_secure_transfer {} -> () => |s, _| {
        s.cancel_secure();
        Ok(())
    };

    /// How far the secure transfer study in progress has got.
    secure_transfer_progress {} -> crate::secure::SecureProgress => |s, _| Ok(s.secure_progress());

    /// One channel of a secure transfer study, read from its results file
    /// (the last run's when `path` is absent): the transfer at each step,
    /// and the channel's value there for the base case and each
    /// contingency asked for by index (every one when absent).
    secure_transfer_channel { path: Option<String>, channel: usize, contingencies: Option<Vec<usize>> } -> crate::secure::ChannelCurves =>
        |s, a| s.secure_transfer_channel(a.path, a.channel, a.contingencies);

    /// The last contingency run's summary, or null.
    contingency_summary { case: Option<CaseId> } -> Option<ContingencySummary> =>
        |s, a| Ok(s.workspace()?.case(a.case)?.contingency_summary());

    /// A page of the last run's ranked results.
    contingency_rows { case: Option<CaseId>, offset: usize, limit: usize } -> crate::table::Table<crate::contingency::ContingencyRow> =>
        |s, a| s.workspace()?.case(a.case)?.contingency_rows(a.offset, a.limit);

    /// A page of the last run's results, sorted, searched and filtered in
    /// the core. Left unsorted, the rows keep their rank order.
    contingency_page { case: Option<CaseId>, query: Option<PageQuery> } -> paging::Page<crate::contingency::ContingencyRow> =>
        |s, a| s.contingency_page(a.case, a.query.unwrap_or_default());

    /// One outage's flow and voltage changes, by rank.
    contingency_detail { case: Option<CaseId>, rank: usize } -> crate::contingency::ContingencyDetail =>
        |s, a| s.workspace()?.case_mut(a.case)?.contingency_detail(a.rank);

    /// Write every violation of the last run to a CSV file, one line each:
    /// branch overloads, bus voltages and interfaces, and a line for each
    /// outage whose AC solve failed. The file opens with metadata rows (see
    /// `export_header`) before the table.
    export_contingency_violations { case: Option<CaseId>, path: PathBuf } -> crate::contingency::ViolationsExport =>
        |s, a| s.workspace()?.case(a.case)?.export_contingency_violations(&a.path);

    /// The metadata rows that open a CSV export, as text to put in front of
    /// the table: `header_rows` (how many rows come before the column
    /// names), `title`, the case file and its SHA-256, study edits, the
    /// program, the export time and `settings` (a JSON object, one field
    /// per row), then a blank row.
    export_header {
        case: Option<CaseId>,
        title: String,
        settings: Option<serde_json::Value>,
    } -> crate::export_header::ExportHeader =>
        |s, a| Ok(s.workspace()?.case(a.case)?.export_header(&a.title, a.settings.as_ref()));

    /// Write the last contingency run to a compact results file (`.stnr`):
    /// an Arrow IPC stream, zstd compressed, one row per finding and one per
    /// outage with none. Opens in pyarrow, polars, pandas and DuckDB.
    export_contingency_results { case: Option<CaseId>, path: PathBuf } -> crate::results_file::ResultsExport =>
        |s, a| s.workspace()?.case(a.case)?.export_contingency_results(&a.path);

    /// The subsystem records of a lists file (`.stnl`), as written.
    lists_subsystems { path: String } -> Vec<crate::lists::ListedSubsystem> =>
        |_, a| Ok(crate::lists::subsystems(&a.path)?);

    /// Take one element out of the last AC solve and solve again. Loadings
    /// are measured against `rating` after the trip and `base_rating`
    /// before it (both rate A when left out).
    trip_and_compare { case: Option<CaseId>, element: String, limit_pct: f64, rating: Option<crate::contingency::Rating>, base_rating: Option<crate::contingency::Rating> } -> crate::contingency::ContingencyDetail =>
        |s, a| s.trip_and_compare(a.case, &a.element, a.limit_pct, (a.rating.unwrap_or_default(), a.base_rating.unwrap_or_default()));

    /// A result by name ("base", "study", "contingency 3", "trip line:1:2:1"),
    /// as a full solve report.
    result_report { case: Option<CaseId>, name: String } -> crate::power::SolveReport =>
        |s, a| s.workspace()?.case_mut(a.case)?.result(&a.name);

    /// A page of a result's buses, sorted, searched and filtered in the
    /// core. `name` is a result name, or "latest" for the latest solve. `busRules` keep the buses passing every rule (the Power flow
    /// filter chips: area, base_kv, kind, number, name, vm).
    result_buses { case: Option<CaseId>, name: String, query: Option<PageQuery>, bus_rules: Option<Vec<paging::Rule>> } -> paging::Page<crate::power::BusRow> =>
        |s, a| s.result_page(a.case, &a.name, BUSES, a.query.unwrap_or_default(), a.bus_rules.unwrap_or_default());

    /// A page of a result's generators. `busRules` keep the generators at a
    /// bus passing every rule.
    result_generators { case: Option<CaseId>, name: String, query: Option<PageQuery>, bus_rules: Option<Vec<paging::Rule>> } -> paging::Page<crate::power::GeneratorRow> =>
        |s, a| s.result_page(a.case, &a.name, GENERATORS, a.query.unwrap_or_default(), a.bus_rules.unwrap_or_default());

    /// A page of a result's branches. `busRules` keep the branches with an
    /// end at a bus passing every rule.
    result_branches { case: Option<CaseId>, name: String, query: Option<PageQuery>, bus_rules: Option<Vec<paging::Rule>> } -> paging::Page<crate::power::BranchRow> =>
        |s, a| s.result_page(a.case, &a.name, BRANCHES, a.query.unwrap_or_default(), a.bus_rules.unwrap_or_default());

    /// The distinct values of a bus filter field in a result, sorted,
    /// missing last. At most `limit` (500 when left out).
    result_bus_values { case: Option<CaseId>, name: String, field: String, limit: Option<usize> } -> Vec<Value> =>
        |s, a| Ok(paging::results::bus_values(&*s.workspace()?.case_mut(a.case)?.shared_result(&a.name)?, &a.field, a.limit.unwrap_or(500)));

    /// Log lines numbered above `since`.
    log_entries { since: u64 } -> Vec<logbook::Entry> => |_, a| Ok(logbook::since(a.since));

    /// Versions and platform for a problem report, and how many internal
    /// errors there have been. Nothing from the case.
    diagnostics {} -> crash::Diagnostics => |_, _| Ok(crash::diagnostics());

    /// Size, hash and date of a file, so a study can tell whether its case changed.
    file_fingerprint { path: PathBuf } -> study_file::Fingerprint => |_, a| study_file::fingerprint(&a.path);

    /// A case's path relative to a study file's folder, if there is one.
    study_relative_path { study: PathBuf, case_path: PathBuf } -> Option<String> =>
        |_, a| Ok(study_file::relative_path(&a.study, &a.case_path));

    /// Where a study's case is now (see `study_file::locate_case`).
    study_locate_case { study: PathBuf, absolute: String, relative: Option<String> } -> Option<String> =>
        |_, a| Ok(study_file::locate_case(&a.study, &a.absolute, a.relative.as_deref()));

    /// Run a run file (`.stnrun`): commands one per line, in order (see
    /// `run_file`). Every line is checked first. The run stops at the first
    /// command that fails unless `keepGoing`; `results` keeps each answer.
    run_file { path: PathBuf, keep_going: Option<bool>, results: Option<bool> } -> crate::run_file::RunReport =>
        |s, a| s.run_file(&a.path, RunOptions {
            keep_going: a.keep_going.unwrap_or(false),
            results: a.results.unwrap_or(false),
        });

    /// Stop the run file in progress before its next line, and the solve or
    /// contingency run it is waiting on.
    cancel_run_file {} -> () => |s, _| {
        s.cancel_run_file();
        Ok(())
    };

    /// Write commands as a run file, such as ones from the app's history. Paths in
    /// the run file's folder are written relative to it, and settings at
    /// their defaults are left out.
    save_run_file { path: PathBuf, steps: Vec<crate::run_file::Step> } -> () => |_, a| {
        let folder = crate::run_file::folder_of(&a.path);
        let mut steps = a.steps;
        crate::run_file::without_defaults(&mut steps);
        let text = crate::run_file::format(&steps, Some(&folder), &format!(
            "Stannum run file, recorded in the app.\nRun it from Ctrl+K > Run a run file, or: stannum-cli run {}",
            a.path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
        ))?;
        std::fs::write(&a.path, text).map_err(|e| Error::file(&a.path, e))
    };

    /// Commands as run file text, for copying from the app's history.
    /// Paths stay as given and settings at their defaults are left out.
    run_file_text { steps: Vec<crate::run_file::Step> } -> String => |_, a| {
        let mut steps = a.steps;
        crate::run_file::without_defaults(&mut steps);
        crate::run_file::format(&steps, None, "Stannum run file, copied from the app's history.")
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn case_file() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rs-power/tests/fixtures/case9.m")
    }

    fn camel(name: &str) -> String {
        let mut out = String::new();
        let mut upper = false;
        for ch in name.chars() {
            if ch == '_' {
                upper = true;
            } else if upper {
                out.extend(ch.to_uppercase());
                upper = false;
            } else {
                out.push(ch);
            }
        }
        out
    }

    #[test]
    fn a_crash_holding_the_workspace_leaves_it_usable() {
        crate::logbook::install();
        let session = Session::new();
        session
            .invoke("open_case", json!({ "path": case_file() }))
            .unwrap();
        // A bug that panics while it holds the workspace lock poisons it.
        let crashed: Result<(), Error> = crash::catch("a test", || {
            let _held = session.workspace()?;
            panic!("bug while holding the workspace");
        });
        assert!(crashed.unwrap_err().is(ErrorKind::Internal));
        // Every later command still works, on the case as it was.
        let summary: Value =
            serde_json::from_str(&session.invoke("case_summary", json!({})).unwrap()).unwrap();
        assert_eq!(summary["power"]["info"]["counts"]["buses"], 9, "{summary}");
        session
            .invoke("solve_power_flow", json!({}))
            .expect("solves after the crash");
        let d: Value =
            serde_json::from_str(&session.invoke("diagnostics", json!({})).unwrap()).unwrap();
        assert!(d["internal_errors"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn solve_settings_can_be_given_one_by_one() {
        let session = Session::new();
        session
            .invoke("open_case", json!({ "path": case_file() }))
            .unwrap();
        let solve = |args: Value| -> Result<Value, Error> {
            Ok(serde_json::from_str(&session.invoke("solve_power_flow", args)?).unwrap())
        };
        let r =
            solve(json!({ "method": "newton", "start": "flat", "switch_shunts": false })).unwrap();
        assert_eq!(r["method"], "newton");
        assert_eq!(r["shunts_locked"], true);
        assert_eq!(r["taps_locked"], false);
        // camelCase too, and beside `settings`.
        let r = solve(json!({ "toleranceMva": 1e-6, "settings": { "method": "fixed_slope", "adjust_transformer_taps": false } })).unwrap();
        assert_eq!(r["method"], "fixed_slope");
        assert_eq!(r["taps_locked"], true);
        let r = solve(json!({ "model": "dc" })).unwrap();
        assert_eq!(r["model"], "dc");

        let e = solve(json!({ "method": "newton", "settings": { "method": "auto" } })).unwrap_err();
        assert!(e.is(ErrorKind::InvalidArguments), "{e:?}");
        let e = solve(json!({ "tolerance_mva": 1, "toleranceMva": 2 })).unwrap_err();
        assert!(e.is(ErrorKind::InvalidArguments), "{e:?}");
        let e = solve(json!({ "method": "fastest" })).unwrap_err();
        assert!(e.is(ErrorKind::InvalidArguments), "{e:?}");
        // Other commands don't take solve settings.
        let e = session
            .invoke("case_summary", json!({ "method": "newton" }))
            .unwrap_err();
        assert!(e.is(ErrorKind::InvalidArguments), "{e:?}");
    }

    #[test]
    fn names_are_unique_and_documented() {
        let mut names: Vec<_> = COMMANDS.iter().map(|c| c.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), COMMANDS.len());
        for c in COMMANDS {
            assert!(!c.doc.trim().is_empty(), "{} has no doc", c.name);
        }
    }

    #[test]
    fn unknown_commands_and_arguments_are_refused() {
        let s = Session::new();
        let e = s.invoke("no_such_command", Value::Null).unwrap_err();
        assert_eq!(e.kind, ErrorKind::UnknownCommand);
        assert!(e.message.contains("no command"), "{e}");
        let e = s
            .invoke("open_case", json!({ "path": "x", "pth": "y" }))
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::InvalidArguments);
        assert!(e.message.contains("unknown field"), "{e}");
        let e = s.invoke("open_case", json!({ "path": 3 })).unwrap_err();
        assert_eq!(e.kind, ErrorKind::InvalidArguments);
    }

    #[test]
    fn list_commands_reports_every_command() {
        let s = Session::new();
        let list: Value =
            serde_json::from_str(&s.invoke("list_commands", Value::Null).unwrap()).unwrap();
        assert_eq!(list["api_version"], API_VERSION);
        assert_eq!(list["commands"].as_array().unwrap().len(), COMMANDS.len());
        let solve = COMMANDS
            .iter()
            .find(|c| c.name == "solve_power_flow")
            .unwrap();
        assert!(solve.takes_case);
        let open = COMMANDS.iter().find(|c| c.name == "open_case").unwrap();
        assert!(!open.takes_case);
    }

    #[test]
    fn every_case_command_checks_the_case_id() {
        let s = Session::new();
        let summary: Value = serde_json::from_str(
            &s.invoke("open_case", json!({ "path": case_file() }))
                .unwrap(),
        )
        .unwrap();
        let id = summary["id"].as_u64().unwrap();
        assert!(id >= 1);
        // A wrong id is refused before any argument is used, so the other
        // arguments only need to parse.
        let value = |name: &str| match name {
            "settings" => json!({ "solve": SolveSettings::default() }),
            "path" | "quantity" | "element" | "section" | "name" | "field" | "title" => {
                json!("unused")
            }
            "edits" | "bus_rules" => json!([]),
            "query" => json!({}),
            "engineering" | "solved" | "enabled" => json!(false),
            "limit_pct" => json!(100.0),
            "rating" | "base_rating" => json!("b"),
            _ => json!(1),
        };
        for (c, names) in COMMANDS.iter().zip(ARGS).filter(|(c, _)| c.takes_case) {
            let mut args = serde_json::Map::new();
            for (name, _) in *names {
                args.insert(camel(name), value(name));
            }
            if c.name == "run_transfer_limits"
                || c.name == "run_pv_curve"
                || c.name == "run_secure_transfer"
            {
                args.insert("settings".into(), json!({ "source": {}, "sink": {} }));
            } else if c.name == "run_qv_curve" {
                args.insert("settings".into(), json!({ "buses": [1] }));
            } else if c.name != "run_contingencies" && args.contains_key("settings") {
                args.insert("settings".into(), json!(SolveSettings::default()));
            }
            args.insert("case".into(), json!(id + 1));
            let e = s.invoke(c.name, Value::Object(args)).unwrap_err();
            assert_eq!(e.kind, ErrorKind::CaseNotOpen, "{}", c.name);
            assert_eq!(
                e.message,
                format!("Case {} isn't open.", id + 1),
                "{}",
                c.name
            );
        }
        // The right id, or none, reaches the case.
        assert!(s.invoke("case_summary", json!({ "case": id })).is_ok());
        assert!(
            s.invoke(
                "solve_power_flow",
                json!({ "settings": SolveSettings::default() })
            )
            .is_ok()
        );
        assert!(
            s.invoke(
                "solve_power_flow",
                json!({ "case": id, "settings": SolveSettings::default() })
            )
            .is_ok()
        );
    }

    #[test]
    fn a_reopened_case_gets_a_new_id() {
        let s = Session::new();
        let first: Value = serde_json::from_str(
            &s.invoke("open_case", json!({ "path": case_file() }))
                .unwrap(),
        )
        .unwrap();
        let second: Value = serde_json::from_str(
            &s.invoke("open_case", json!({ "path": case_file() }))
                .unwrap(),
        )
        .unwrap();
        assert_ne!(first["id"], second["id"]);
        let e = s
            .invoke("diagram_network", json!({ "case": first["id"] }))
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::CaseNotOpen);
        assert!(e.message.contains("isn't open"), "{e}");
    }

    #[test]
    fn no_case_reads_as_null_or_a_prompt() {
        let s = Session::new();
        assert_eq!(s.invoke("case_summary", Value::Null).unwrap(), "null");
        assert_eq!(
            s.invoke("diagram_network", Value::Null).unwrap_err(),
            Error::new(ErrorKind::NoCase, "Open a case first.")
        );
        assert_eq!(s.invoke("cancel_solve", Value::Null).unwrap(), "null");
    }
}
