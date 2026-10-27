//! The native half of the `stannum` Python package.
//!
//! It holds one thing: a [`Session`] of Stannum's command registry, called
//! with a command name and JSON arguments, the same way the app and the dev
//! bridge call it. Everything Pythonic (case objects, tables as NumPy
//! columns, typed exceptions) lives in `python/stannum/`, on top of
//! `Session.invoke`.
//!
//! A command runs on its own thread with the GIL let go, so other Python
//! threads keep running. Meanwhile this thread checks for Ctrl+C every
//! 50 ms; on one it asks the session to stop its solve, contingency run or
//! run file, waits for the command to come back, and raises
//! `KeyboardInterrupt`.

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use serde_json::Value;
use stannum_core::Error;
use stannum_core::commands::{self, API_VERSION};
use std::sync::mpsc;
use std::time::Duration;

create_exception!(
    _native,
    CommandError,
    PyException,
    "A command failed. Args are (kind, message); stannum raises it again as its typed error."
);

/// How often a running command checks for Ctrl+C.
const SIGNAL_POLL: Duration = Duration::from_millis(50);

/// The commands that stop long work. Each is harmless when nothing runs.
const CANCEL: [&str; 3] = ["cancel_solve", "cancel_contingencies", "cancel_run_file"];

/// One workspace of Stannum's core, which holds one open case at a time.
/// `stannum.Case` keeps one each, so cases never share state or a lock.
#[pyclass(frozen, module = "stannum._native")]
struct Session {
    inner: commands::Session,
}

fn command_error(e: Error) -> PyErr {
    CommandError::new_err((e.kind.as_str(), e.message))
}

#[pymethods]
impl Session {
    #[new]
    fn new() -> Self {
        Self {
            inner: commands::Session::new(),
        }
    }

    /// Run one command. `args` is a JSON object of its arguments, as text;
    /// the answer is JSON text. A failure raises `CommandError(kind,
    /// message)`; Ctrl+C stops the work and raises `KeyboardInterrupt`.
    #[pyo3(signature = (command, args = "{}"))]
    fn invoke(&self, py: Python<'_>, command: &str, args: &str) -> PyResult<String> {
        let args: Value = serde_json::from_str(args)
            .map_err(|e| PyValueError::new_err(format!("{command}: arguments aren't JSON: {e}")))?;
        let session = &self.inner;
        let mut interrupted: Option<PyErr> = None;
        let answer = std::thread::scope(|scope| {
            let (tx, rx) = mpsc::channel();
            scope.spawn(move || {
                // The receiver only goes away once this scope ends.
                let _ = tx.send(session.invoke(command, args));
            });
            let mut rx = rx;
            loop {
                // The receiver goes in and comes back out, since only owned
                // values may cross into code that runs without the GIL.
                let (back, got) = py.detach(move || {
                    let got = rx.recv_timeout(SIGNAL_POLL);
                    (rx, got)
                });
                rx = back;
                match got {
                    Ok(answer) => return answer,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(Error::internal(format!(
                            "{command} stopped without an answer."
                        )));
                    }
                }
                if let Err(signal) = py.check_signals()
                    && interrupted.is_none()
                {
                    for cancel in CANCEL {
                        let _ = session.invoke(cancel, Value::Null);
                    }
                    interrupted = Some(signal);
                }
            }
        });
        if let Some(signal) = interrupted {
            return Err(signal);
        }
        answer.map_err(command_error)
    }
}

/// Every command's name, doc and whether it takes a case, as JSON text.
#[pyfunction]
fn command_list() -> PyResult<String> {
    commands::Session::new()
        .invoke("list_commands", Value::Null)
        .map_err(command_error)
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Keep the core's log so `stannum.log()` can read it.
    stannum_core::logbook::install();
    m.add_class::<Session>()?;
    m.add_function(wrap_pyfunction!(command_list, m)?)?;
    m.add("CommandError", m.py().get_type::<CommandError>())?;
    m.add("API_VERSION", API_VERSION)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
