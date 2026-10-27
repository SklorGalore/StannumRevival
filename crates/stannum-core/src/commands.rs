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
use crate::power::SolveSettings;
use crate::record_edit::RecordEdit;
use crate::study::Edit;
use crate::workspace::{CaseId, Workspace};
use crate::{logbook, study_file};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Version of the command set. It goes up when a command is removed or an
/// argument or result changes in a way existing callers would notice.
pub const API_VERSION: u32 = 1;

/// One command, as `list_commands` describes it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct CommandInfo {
    pub name: &'static str,
    /// What it does, from its doc comment.
    pub doc: &'static str,
    /// Whether it takes a `case` argument.
    pub takes_case: bool,
}

/// The answer to `list_commands`.
#[derive(Debug, Serialize)]
pub struct CommandList {
    pub api_version: u32,
    pub commands: Vec<CommandInfo>,
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
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run one command. `args` is a JSON object of its arguments (null is
    /// read as no arguments). The result is JSON text, ready to send.
    pub fn invoke(&self, command: &str, args: Value) -> Result<String, String> {
        dispatch(self, command, args)
    }

    /// The workspace, for callers that need it directly (tests, examples).
    pub fn workspace(&self) -> Result<MutexGuard<'_, Workspace>, String> {
        self.workspace
            .lock()
            .map_err(|_| "Workspace is unavailable after an earlier failure.".to_string())
    }

    fn solve(
        &self,
        case: Option<CaseId>,
        settings: SolveSettings,
    ) -> Result<crate::power::SolveReport, String> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut slot) = self.solving.lock() {
            *slot = Some(flag.clone());
        }
        let result = self.workspace().and_then(|mut ws| {
            ws.case_mut(case)?
                .solve_cancellable(settings, Some(flag.clone()))
        });
        // Only clear the slot if a newer solve hasn't taken it.
        if let Ok(mut slot) = self.solving.lock()
            && slot.as_ref().is_some_and(|f| Arc::ptr_eq(f, &flag))
        {
            *slot = None;
        }
        result
    }

    fn cancel_solve(&self) {
        if let Some(flag) = self.solving.lock().ok().and_then(|f| f.clone()) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// The run itself doesn't hold the workspace, so other commands keep
    /// working and Cancel can reach it.
    fn run_contingencies(
        &self,
        case: Option<CaseId>,
        settings: ContingencySettings,
    ) -> Result<ContingencySummary, String> {
        let job = self.workspace()?.case(case)?.contingency_job(settings)?;
        if let Ok(mut slot) = self.contingencies.lock() {
            *slot = Some(job.handle());
        }
        let results = job.run();
        if let Ok(mut slot) = self.contingencies.lock() {
            *slot = None;
        }
        let results = results?;
        Ok(self.workspace()?.case_mut(case)?.set_contingencies(results))
    }

    fn cancel_contingencies(&self) {
        if let Some(handle) = self.contingencies.lock().ok().and_then(|h| h.clone()) {
            handle.cancel();
        }
    }

    fn contingency_progress(&self) -> Progress {
        self.contingencies
            .lock()
            .ok()
            .and_then(|h| h.as_ref().map(RunHandle::progress))
            .unwrap_or_default()
    }
}

fn parse<T: serde::de::DeserializeOwned>(command: &str, args: Value) -> Result<T, String> {
    let args = if args.is_null() {
        Value::Object(Default::default())
    } else {
        args
    };
    serde_json::from_value(args).map_err(|e| format!("{command}: {e}"))
}

fn json<T: Serialize>(value: Result<T, String>) -> Result<String, String> {
    serde_json::to_string(&value?).map_err(|e| e.to_string())
}

/// Declares each command once: its doc, its arguments (camelCase in JSON)
/// and its body. A command whose arguments include `case` is case-scoped.
macro_rules! commands {
    ($(
        $(#[doc = $doc:literal])*
        $name:ident { $($field:ident : $ty:ty),* $(,)? }
        => |$s:pat_param, $a:pat_param| $body:expr;
    )*) => {
        #[allow(non_camel_case_types, dead_code)]
        mod args {
            use super::*;
            $(
                #[derive(Deserialize)]
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
            },
        )*];

        /// Each command's argument names, as declared (snake_case).
        #[cfg(test)]
        const ARG_NAMES: &[&[&str]] = &[$(&[$(stringify!($field)),*],)*];

        fn dispatch(session: &Session, command: &str, raw: Value) -> Result<String, String> {
            match command {
                $(stringify!($name) => {
                    let $a: args::$name = parse(command, raw)?;
                    let $s = session;
                    json($body)
                })*
                other => Err(format!("Stannum has no command called {other}.")),
            }
        }
    };
}

commands! {
    /// The commands this version answers, and the API version.
    list_commands {} => |_, _| Ok(CommandList {
        api_version: API_VERSION,
        commands: COMMANDS.to_vec(),
    });

    /// Open a case file (PSS/E RAW or MATPOWER). It replaces the open case.
    open_case { path: PathBuf } => |s, a| s.workspace()?.open(&a.path);

    /// Close the case.
    close_case { case: Option<CaseId> } => |s, a| {
        let mut ws = s.workspace()?;
        ws.case(a.case)?;
        ws.close();
        Ok(())
    };

    /// The open case's summary, or null when none is open.
    case_summary { case: Option<CaseId> } => |s, a| {
        let ws = s.workspace()?;
        match (&ws.case, a.case) {
            (None, None) => Ok(None),
            _ => ws.case(a.case).map(|c| Some(c.summary())),
        }
    };

    /// Solve the power flow with the study's edits. `cancel_solve` stops it.
    solve_power_flow { case: Option<CaseId>, settings: SolveSettings } =>
        |s, a| s.solve(a.case, a.settings);

    /// Stop the solve in progress; the case keeps its previous results.
    cancel_solve {} => |s, _| {
        s.cancel_solve();
        Ok(())
    };

    /// Attach a Stannum DC grid file to the case.
    load_dc_grids { case: Option<CaseId>, path: PathBuf } =>
        |s, a| s.workspace()?.case_mut(a.case)?.load_dc_grids(&a.path);

    /// Detach the DC grid file.
    clear_dc_grids { case: Option<CaseId> } => |s, a| {
        s.workspace()?.case_mut(a.case)?.clear_dc_grids();
        Ok(())
    };

    /// Replace the study's edits; every later solve uses them.
    set_study_edits { case: Option<CaseId>, edits: Vec<Edit> } =>
        |s, a| s.workspace()?.case_mut(a.case)?.set_edits(a.edits);

    /// Calculate sensitivities at the solved operating point.
    calculate_sensitivities { case: Option<CaseId>, settings: SolveSettings } =>
        |s, a| s.workspace()?.case_mut(a.case)?.sensitivities(a.settings);

    /// One sensitivity column's responses at every bus.
    sensitivity_responses {
        case: Option<CaseId>,
        column: usize,
        quantity: String,
        engineering: bool,
    } => |s, a| s.workspace()?.case(a.case)?.responses(a.column, &a.quantity, a.engineering);

    /// Write the case, solved or as filed, as a MATPOWER file.
    export_matpower { case: Option<CaseId>, path: PathBuf, solved: bool } =>
        |s, a| s.workspace()?.case(a.case)?.export_matpower(&a.path, a.solved);

    /// The case as a drawable network, for one-line diagrams.
    diagram_network { case: Option<CaseId> } =>
        |s, a| s.workspace()?.case(a.case)?.diagram_network();

    /// One element's data and limits from the file.
    element_detail { case: Option<CaseId>, element: String } =>
        |s, a| s.workspace()?.case(a.case)?.element_detail(&a.element);

    /// The case's record types and their counts.
    case_records { case: Option<CaseId> } =>
        |s, a| s.workspace()?.case_mut(a.case)?.record_book();

    /// Every record of one type, with every field.
    record_table { case: Option<CaseId>, section: String } =>
        |s, a| s.workspace()?.case_mut(a.case)?.record_table(&a.section);

    /// Write cell edits into the case and reopen it in place.
    apply_record_edits { case: Option<CaseId>, edits: Vec<RecordEdit> } =>
        |s, a| s.workspace()?.case_mut(a.case)?.apply_record_edits(&a.edits);

    /// Save the case, with its edits, to a new file.
    save_case_as { case: Option<CaseId>, path: PathBuf } =>
        |s, a| s.workspace()?.case_mut(a.case)?.save_as(&a.path);

    /// Islands, bridges and articulation buses, with the case's graph.
    topology_snapshot { case: Option<CaseId> } => |s, a| {
        let ws = s.workspace()?;
        let snapshot = ws.case(a.case)?.topology_snapshot()?;
        serde_json::to_value(snapshot).map_err(|e| e.to_string())
    };

    /// Merge buses joined by zero-impedance lines, or split them again.
    topology_set_merge { case: Option<CaseId>, enabled: bool } =>
        |s, a| s.workspace()?.case_mut(a.case)?.set_merge_zero_impedance(a.enabled);

    /// The shortest path between two buses, or null.
    topology_shortest_path { case: Option<CaseId>, from: i32, to: i32 } =>
        |s, a| s.workspace()?.case(a.case)?.shortest_path(a.from, a.to);

    /// Compare two RAW files record by record.
    diff_compare { baseline: PathBuf, comparison: PathBuf } =>
        |s, a| s.workspace()?.compare(&a.baseline, &a.comparison);

    /// Compare the case's file with the case as it is being worked on.
    diff_compare_working { case: Option<CaseId> } => |s, a| {
        let mut ws = s.workspace()?;
        ws.case(a.case)?;
        ws.compare_working()
    };

    /// The last comparison's records of one type.
    diff_section_records { section: String } =>
        |s, a| Ok(s.workspace()?.comparison()?.section_records(&a.section));

    /// One record of the last comparison, field by field.
    diff_record_detail { section: String, index: usize } =>
        |s, a| Ok(s.workspace()?.comparison()?.record_detail(&a.section, a.index));

    /// Run N-1 contingencies. `cancel_contingencies` stops the run and
    /// `contingency_progress` follows it.
    run_contingencies { case: Option<CaseId>, settings: ContingencySettings } =>
        |s, a| s.run_contingencies(a.case, a.settings);

    /// Stop the contingency run in progress.
    cancel_contingencies {} => |s, _| {
        s.cancel_contingencies();
        Ok(())
    };

    /// How far the contingency run in progress has got.
    contingency_progress {} => |s, _| Ok(s.contingency_progress());

    /// The last contingency run's summary, or null.
    contingency_summary { case: Option<CaseId> } =>
        |s, a| Ok(s.workspace()?.case(a.case)?.contingency_summary());

    /// A page of the last run's ranked results.
    contingency_rows { case: Option<CaseId>, offset: usize, limit: usize } =>
        |s, a| s.workspace()?.case(a.case)?.contingency_rows(a.offset, a.limit);

    /// One outage's flow and voltage changes, by rank.
    contingency_detail { case: Option<CaseId>, rank: usize } =>
        |s, a| s.workspace()?.case_mut(a.case)?.contingency_detail(a.rank);

    /// Take one element out of the last AC solve and solve again.
    trip_and_compare { case: Option<CaseId>, element: String, limit_pct: f64 } =>
        |s, a| s.workspace()?.case(a.case)?.trip_and_compare(&a.element, a.limit_pct);

    /// Log lines numbered above `since`.
    log_entries { since: u64 } => |_, a| Ok(logbook::since(a.since));

    /// Size, hash and date of a file, so a study can tell whether its case changed.
    file_fingerprint { path: PathBuf } => |_, a| study_file::fingerprint(&a.path);

    /// A case's path relative to a study file's folder, if there is one.
    study_relative_path { study: PathBuf, case_path: PathBuf } =>
        |_, a| Ok(study_file::relative_path(&a.study, &a.case_path));

    /// Where a study's case is now (see `study_file::locate_case`).
    study_locate_case { study: PathBuf, absolute: String, relative: Option<String> } =>
        |_, a| Ok(study_file::locate_case(&a.study, &a.absolute, a.relative.as_deref()));
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
        assert!(
            s.invoke("no_such_command", Value::Null)
                .unwrap_err()
                .contains("no command")
        );
        let e = s
            .invoke("open_case", json!({ "path": "x", "pth": "y" }))
            .unwrap_err();
        assert!(e.contains("unknown field"), "{e}");
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
            "path" | "quantity" | "element" | "section" => json!("unused"),
            "edits" => json!([]),
            "engineering" | "solved" | "enabled" => json!(false),
            "limit_pct" => json!(100.0),
            _ => json!(1),
        };
        for (c, names) in COMMANDS.iter().zip(ARG_NAMES).filter(|(c, _)| c.takes_case) {
            let mut args = serde_json::Map::new();
            for name in *names {
                args.insert(camel(name), value(name));
            }
            if c.name != "run_contingencies" && args.contains_key("settings") {
                args.insert("settings".into(), json!(SolveSettings::default()));
            }
            args.insert("case".into(), json!(id + 1));
            let e = s.invoke(c.name, Value::Object(args)).unwrap_err();
            assert_eq!(e, format!("Case {} isn't open.", id + 1), "{}", c.name);
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
        assert!(e.contains("isn't open"), "{e}");
    }

    #[test]
    fn no_case_reads_as_null_or_a_prompt() {
        let s = Session::new();
        assert_eq!(s.invoke("case_summary", Value::Null).unwrap(), "null");
        assert_eq!(
            s.invoke("diagram_network", Value::Null).unwrap_err(),
            "Open a case first."
        );
        assert_eq!(s.invoke("cancel_solve", Value::Null).unwrap(), "null");
    }
}
