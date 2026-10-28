//! The window's side of attach (see `stannum_core::attach`): scripts send
//! core commands to the window's own session, and `ui.*` commands to the
//! window, which answers through `attach_ui_reply`.

use serde::Serialize;
use serde_json::Value;
use stannum_core::attach::{self, Server, Window};
use stannum_core::{Error, ErrorKind};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// A `ui.*` command for the window to answer.
pub const UI_EVENT: &str = "stannum://attach-ui";
/// A script changed the case, the study or its results.
pub const CHANGED_EVENT: &str = "stannum://attach-changed";
/// How many scripts are attached.
pub const CLIENTS_EVENT: &str = "stannum://attach-clients";

/// How long a `ui.*` command waits for the window.
const UI_TIMEOUT: Duration = Duration::from_secs(10);

type Reply = mpsc::Sender<Result<Value, Error>>;

pub struct AppWindow {
    app: AppHandle,
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, Reply>>,
    clients: AtomicUsize,
}

#[derive(Serialize, Clone)]
struct UiRequest<'a> {
    id: u64,
    command: &'a str,
    args: Value,
}

#[derive(Serialize, Clone)]
struct Changed<'a> {
    command: &'a str,
    args: &'a Value,
    answer: Value,
}

impl Window for AppWindow {
    fn ui(&self, command: &str, args: Value) -> Result<Value, Error> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.waiting
            .lock()
            .map_err(|_| Error::internal("Attach is unavailable."))?
            .insert(id, tx);
        let sent = self.app.emit(UI_EVENT, UiRequest { id, command, args });
        let got = match sent {
            Ok(()) => rx.recv_timeout(UI_TIMEOUT).map_err(|_| {
                Error::new(
                    ErrorKind::NotReady,
                    "The window didn't answer. Is it still starting?",
                )
            }),
            Err(e) => Err(Error::internal(e.to_string())),
        };
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.remove(&id);
        }
        got?
    }

    fn changed(&self, command: &str, args: &Value, answer: &str) {
        let answer = serde_json::from_str(answer).unwrap_or(Value::Null);
        let _ = self.app.emit(
            CHANGED_EVENT,
            Changed {
                command,
                args,
                answer,
            },
        );
    }

    fn clients(&self, count: usize) {
        self.clients.store(count, Ordering::SeqCst);
        let _ = self.app.emit(CLIENTS_EVENT, count);
    }
}

/// The listener and the window's half, kept for the life of the app.
pub struct Attach {
    window: Arc<AppWindow>,
    server: Option<Server>,
}

/// Start listening, unless `STANNUM_NO_ATTACH` is set. A failure is logged
/// and the app runs without attach.
pub fn start(app: &AppHandle, session: Arc<stannum_core::commands::Session>) -> Attach {
    let window = Arc::new(AppWindow {
        app: app.clone(),
        next: AtomicU64::new(1),
        waiting: Mutex::new(HashMap::new()),
        clients: AtomicUsize::new(0),
    });
    let off = std::env::var_os("STANNUM_NO_ATTACH").is_some_and(|v| !v.is_empty());
    let server = if off {
        log::info!(target: "attach", "Attach is off (STANNUM_NO_ATTACH).");
        None
    } else {
        match attach::ticket_dir() {
            None => {
                log::warn!(target: "attach", "Scripts can't attach: no folder for the ticket.");
                None
            }
            Some(dir) => {
                let version = app.package_info().version.to_string();
                match Server::start(session, window.clone(), &dir, "Stannum", &version) {
                    Ok(server) => Some(server),
                    Err(e) => {
                        log::warn!(target: "attach", "Scripts can't attach: {e}");
                        None
                    }
                }
            }
        }
    };
    Attach { window, server }
}

/// Remove the ticket before the app exits.
pub fn stop(app: &AppHandle) {
    if let Some(state) = app.try_state::<Mutex<Attach>>() {
        if let Ok(mut attach) = state.lock() {
            attach.server = None;
        }
    }
}

/// The window's answer to a `ui.*` command: `ok`, or `error` as
/// `{kind, message}`.
#[tauri::command]
pub fn attach_ui_reply(
    state: State<'_, Mutex<Attach>>,
    id: u64,
    ok: Option<Value>,
    error: Option<Error>,
) {
    let Ok(attach) = state.lock() else { return };
    let Ok(waiting) = attach.window.waiting.lock() else {
        return;
    };
    if let Some(tx) = waiting.get(&id) {
        let _ = tx.send(match error {
            Some(e) => Err(e),
            None => Ok(ok.unwrap_or(Value::Null)),
        });
    }
}

#[derive(Serialize)]
pub struct AttachStatus {
    /// Whether scripts can attach.
    listening: bool,
    port: Option<u16>,
    clients: usize,
}

/// Whether scripts can attach, and how many are attached now.
#[tauri::command]
pub fn attach_status(state: State<'_, Mutex<Attach>>) -> AttachStatus {
    let Ok(attach) = state.lock() else {
        return AttachStatus {
            listening: false,
            port: None,
            clients: 0,
        };
    };
    AttachStatus {
        listening: attach.server.is_some(),
        port: attach.server.as_ref().map(Server::port),
        clients: attach.window.clients.load(Ordering::SeqCst),
    }
}
