//! Tauri commands over `stannum-core`. Each command runs on a blocking
//! worker so long solves never freeze the window.

use stannum_core::contingency::{
    ContingencyDetail, ContingencyRow, ContingencySettings, ContingencySummary, Progress, RunHandle,
};
use stannum_core::diff::{DiffSummary, RecordDetail, RecordRow};
use stannum_core::power::{ResponseTable, SensitivitySummary, SolveReport, SolveSettings};
use stannum_core::study::Edit;
use stannum_core::{logbook, CaseSummary, Workspace};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

#[derive(Default, Clone)]
pub struct SharedWorkspace(Arc<Mutex<Workspace>>);

/// Run `f` with the workspace on a blocking thread.
async fn with<T, F>(state: State<'_, SharedWorkspace>, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut Workspace) -> Result<T, String> + Send + 'static,
{
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut ws = shared
            .lock()
            .map_err(|_| "Workspace is unavailable after an earlier failure.".to_string())?;
        f(&mut ws)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_case(
    state: State<'_, SharedWorkspace>,
    path: String,
) -> Result<CaseSummary, String> {
    with(state, move |ws| ws.open(&PathBuf::from(path))).await
}

#[tauri::command]
pub async fn close_case(state: State<'_, SharedWorkspace>) -> Result<(), String> {
    with(state, |ws| {
        ws.close();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn case_summary(
    state: State<'_, SharedWorkspace>,
) -> Result<Option<CaseSummary>, String> {
    with(state, |ws| Ok(ws.case.as_ref().map(|c| c.summary()))).await
}

/// The flag that stops the solve in progress, if one is running.
#[derive(Default, Clone)]
pub struct SolveRun(Arc<Mutex<Option<Arc<AtomicBool>>>>);

#[tauri::command]
pub async fn solve_power_flow(
    state: State<'_, SharedWorkspace>,
    run: State<'_, SolveRun>,
    settings: SolveSettings,
) -> Result<SolveReport, String> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut slot) = run.0.lock() {
        *slot = Some(flag.clone());
    }
    let current = run.0.clone();
    let mine = flag.clone();
    let result = with(state, move |ws| {
        ws.current_mut()?.solve_cancellable(settings, Some(flag))
    })
    .await;
    // Only clear the slot if a newer solve hasn't taken it.
    if let Ok(mut slot) = current.lock() {
        if slot.as_ref().is_some_and(|f| Arc::ptr_eq(f, &mine)) {
            *slot = None;
        }
    }
    result
}

/// Stop the solve in progress; it keeps the previous results.
#[tauri::command]
pub fn cancel_solve(run: State<'_, SolveRun>) {
    if let Some(flag) = run.0.lock().ok().and_then(|f| f.clone()) {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Attach a Stannum DC grid file to the open case.
#[tauri::command]
pub async fn load_dc_grids(
    state: State<'_, SharedWorkspace>,
    path: String,
) -> Result<stannum_core::dc_grids::DcGridsSummary, String> {
    with(state, move |ws| {
        ws.current_mut()?.load_dc_grids(&PathBuf::from(path))
    })
    .await
}

#[tauri::command]
pub async fn clear_dc_grids(state: State<'_, SharedWorkspace>) -> Result<(), String> {
    with(state, |ws| {
        ws.current_mut()?.clear_dc_grids();
        Ok(())
    })
    .await
}

/// Replace the study's edits; the next solve uses them.
#[tauri::command]
pub async fn set_study_edits(
    state: State<'_, SharedWorkspace>,
    edits: Vec<Edit>,
) -> Result<(), String> {
    with(state, move |ws| ws.current_mut()?.set_edits(edits)).await
}

#[tauri::command]
pub async fn calculate_sensitivities(
    state: State<'_, SharedWorkspace>,
    settings: SolveSettings,
) -> Result<SensitivitySummary, String> {
    with(state, move |ws| ws.current_mut()?.sensitivities(settings)).await
}

#[tauri::command]
pub async fn sensitivity_responses(
    state: State<'_, SharedWorkspace>,
    column: usize,
    quantity: String,
    engineering: bool,
) -> Result<ResponseTable, String> {
    with(state, move |ws| {
        ws.current()?.responses(column, &quantity, engineering)
    })
    .await
}

#[tauri::command]
pub async fn export_matpower(
    state: State<'_, SharedWorkspace>,
    path: String,
    solved: bool,
) -> Result<(), String> {
    with(state, move |ws| {
        ws.current()?.export_matpower(&PathBuf::from(path), solved)
    })
    .await
}

#[tauri::command]
pub async fn diagram_network(
    state: State<'_, SharedWorkspace>,
) -> Result<stannum_core::diagram::CaseNetwork, String> {
    with(state, |ws| ws.current()?.diagram_network()).await
}

/// One element's data and limits from the file, for the element inspector.
#[tauri::command]
pub async fn element_detail(
    state: State<'_, SharedWorkspace>,
    element: String,
) -> Result<stannum_core::inspect::ElementDetail, String> {
    with(state, move |ws| ws.current()?.element_detail(&element)).await
}

/// The open case's record types and their counts, for the Records module.
#[tauri::command]
pub async fn case_records(
    state: State<'_, SharedWorkspace>,
) -> Result<stannum_core::records::RecordBook, String> {
    with(state, |ws| ws.current_mut()?.record_book()).await
}

/// Every record of one type, with every field.
#[tauri::command]
pub async fn record_table(
    state: State<'_, SharedWorkspace>,
    section: String,
) -> Result<stannum_core::records::RecordTable, String> {
    with(state, move |ws| ws.current_mut()?.record_table(&section)).await
}

/// Serialized inside the lock: the snapshot borrows the parsed case.
#[tauri::command]
pub async fn topology_snapshot(
    state: State<'_, SharedWorkspace>,
) -> Result<serde_json::Value, String> {
    with(state, |ws| {
        let snapshot = ws.current()?.topology_snapshot()?;
        serde_json::to_value(snapshot).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn topology_set_merge(
    state: State<'_, SharedWorkspace>,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    with(state, move |ws| {
        let view = ws.current_mut()?.set_merge_zero_impedance(enabled)?;
        serde_json::to_value(view).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn topology_shortest_path(
    state: State<'_, SharedWorkspace>,
    from: i32,
    to: i32,
) -> Result<serde_json::Value, String> {
    with(state, move |ws| {
        let path = ws.current()?.shortest_path(from, to)?;
        serde_json::to_value(path).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn diff_compare(
    state: State<'_, SharedWorkspace>,
    baseline: String,
    comparison: String,
) -> Result<DiffSummary, String> {
    with(state, move |ws| {
        ws.compare(&PathBuf::from(baseline), &PathBuf::from(comparison))
    })
    .await
}

#[tauri::command]
pub async fn diff_section_records(
    state: State<'_, SharedWorkspace>,
    section: String,
) -> Result<Vec<RecordRow>, String> {
    with(state, move |ws| {
        Ok(ws.comparison()?.section_records(&section))
    })
    .await
}

#[tauri::command]
pub async fn diff_record_detail(
    state: State<'_, SharedWorkspace>,
    section: String,
    index: usize,
) -> Result<Option<RecordDetail>, String> {
    with(state, move |ws| {
        Ok(ws.comparison()?.record_detail(&section, index))
    })
    .await
}

/// The contingency run in hand, so it can be cancelled or followed while it
/// runs without the workspace lock.
#[derive(Default, Clone)]
pub struct ContingencyRun(Arc<Mutex<Option<RunHandle>>>);

/// Run contingencies. The run itself doesn't hold the workspace, so the
/// window stays usable and Cancel can reach it.
#[tauri::command]
pub async fn run_contingencies(
    state: State<'_, SharedWorkspace>,
    run: State<'_, ContingencyRun>,
    settings: ContingencySettings,
) -> Result<ContingencySummary, String> {
    let shared = state.0.clone();
    let current = run.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let job = {
            let ws = shared
                .lock()
                .map_err(|_| "Workspace is unavailable after an earlier failure.".to_string())?;
            ws.current()?.contingency_job(settings)?
        };
        if let Ok(mut slot) = current.lock() {
            *slot = Some(job.handle());
        }
        let results = job.run();
        if let Ok(mut slot) = current.lock() {
            *slot = None;
        }
        let results = results?;
        let mut ws = shared
            .lock()
            .map_err(|_| "Workspace is unavailable after an earlier failure.".to_string())?;
        Ok(ws.current_mut()?.set_contingencies(results))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_contingencies(run: State<'_, ContingencyRun>) {
    if let Some(handle) = run.0.lock().ok().and_then(|h| h.clone()) {
        handle.cancel();
    }
}

#[tauri::command]
pub fn contingency_progress(run: State<'_, ContingencyRun>) -> Progress {
    run.0
        .lock()
        .ok()
        .and_then(|h| h.as_ref().map(RunHandle::progress))
        .unwrap_or_default()
}

#[tauri::command]
pub async fn contingency_summary(
    state: State<'_, SharedWorkspace>,
) -> Result<Option<ContingencySummary>, String> {
    with(state, |ws| Ok(ws.current()?.contingency_summary())).await
}

#[tauri::command]
pub async fn contingency_rows(
    state: State<'_, SharedWorkspace>,
    offset: usize,
    limit: usize,
) -> Result<Vec<ContingencyRow>, String> {
    with(state, move |ws| {
        ws.current()?.contingency_rows(offset, limit)
    })
    .await
}

#[tauri::command]
pub async fn contingency_detail(
    state: State<'_, SharedWorkspace>,
    rank: usize,
) -> Result<ContingencyDetail, String> {
    with(state, move |ws| ws.current_mut()?.contingency_detail(rank)).await
}

/// Take one element out of the last AC solve and solve again.
#[tauri::command]
pub async fn trip_and_compare(
    state: State<'_, SharedWorkspace>,
    element: String,
    limit_pct: f64,
) -> Result<ContingencyDetail, String> {
    with(state, move |ws| {
        ws.current()?.trip_and_compare(&element, limit_pct)
    })
    .await
}

/// Log lines numbered above `since`, for the log panel to catch up.
#[tauri::command]
pub fn log_entries(since: u64) -> Vec<logbook::Entry> {
    logbook::since(since)
}

/// Event that carries new log lines to the window, a batch at a time.
pub const LOG_EVENT: &str = "stannum://log";

/// Push new log lines to the window several times a second, so the panel
/// shows a solve's progress while it runs.
pub fn stream_log(app: AppHandle) {
    std::thread::spawn(move || {
        let mut seen = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(150));
            if logbook::latest() == seen {
                continue;
            }
            let batch = logbook::since(seen);
            if let Some(last) = batch.last() {
                seen = last.seq;
                let _ = app.emit(LOG_EVENT, batch);
            }
        }
    });
}
