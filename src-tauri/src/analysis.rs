//! The Tauri side of the core's command list. Every analysis command goes
//! through `stannum_command`, which hands it to `stannum_core::commands` on a
//! blocking worker so long solves never freeze the window.

use stannum_core::commands::Session;
use stannum_core::logbook;
use stannum_core::Error;
use std::sync::Arc;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, State};

#[derive(Default, Clone)]
pub struct SharedSession(pub Arc<Session>);

/// Run one core command by name (see `stannum_core::commands::COMMANDS`).
/// The result is already JSON, so it goes to the window as it is. A failure
/// reaches the window as `{kind, message}` (see `stannum_core::error`).
#[tauri::command]
pub async fn stannum_command(
    state: State<'_, SharedSession>,
    command: String,
    args: Option<serde_json::Value>,
) -> Result<Response, Error> {
    let session = state.0.clone();
    // Panics are caught inside invoke; this only names a worker that died.
    let command_name = command.clone();
    tauri::async_runtime::spawn_blocking(move || session.invoke(&command, args.unwrap_or_default()))
        .await
        .map_err(|e| {
            Error::internal(format!(
                "Stannum hit an internal error in {command_name}: {e}"
            ))
        })?
        .map(Response::new)
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
