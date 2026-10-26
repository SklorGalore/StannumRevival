//! Tauri commands over `stannum-core`. Each command runs on a blocking
//! worker so long solves never freeze the window.

use stannum_core::diff::{DiffSummary, RecordDetail, RecordRow};
use stannum_core::power::{ResponseTable, SensitivitySummary, SolveReport, SolveSettings};
use stannum_core::study::Edit;
use stannum_core::{logbook, CaseSummary, Workspace};
use std::path::PathBuf;
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

#[tauri::command]
pub async fn solve_power_flow(
    state: State<'_, SharedWorkspace>,
    settings: SolveSettings,
) -> Result<SolveReport, String> {
    with(state, move |ws| ws.current_mut()?.solve(settings)).await
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
