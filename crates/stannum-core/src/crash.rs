//! What happens when the core hits a bug.
//!
//! A panic anywhere in the core is written to the log ([`install_hook`]),
//! and a command that panics answers with an
//! [`ErrorKind::Internal`](crate::ErrorKind::Internal) error instead of
//! taking its caller down ([`catch`]). The window, a script or the command
//! line keeps working; the person can save the log with Save diagnostics
//! and report it.
//!
//! [`diagnostics`] is what Save diagnostics adds to the log: versions and
//! the platform, and nothing from the case.

use crate::error::Error;
use schemars::JsonSchema;
use serde::Serialize;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};

/// Where crash lines appear in the log ("core").
const TARGET: &str = "stannum_core::crash";

/// Panics seen since the program started.
static PANICS: AtomicU32 = AtomicU32::new(0);

/// Log every panic, then let the previous hook run too (it prints to
/// stderr). Called by [`crate::logbook::install`]; later calls do nothing.
pub fn install_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            PANICS.fetch_add(1, Ordering::Relaxed);
            let thread = std::thread::current();
            let thread = thread
                .name()
                .map(|n| format!(" on thread {n}"))
                .unwrap_or_default();
            let place = info
                .location()
                .map(|l| format!(" at {}:{}", l.file(), l.line()))
                .unwrap_or_default();
            log::error!(
                target: TARGET,
                "Internal error{thread}{place}: {}",
                payload_text(info.payload())
            );
            let trace = std::backtrace::Backtrace::force_capture();
            log::debug!(target: TARGET, "Backtrace:\n{trace}");
            previous(info);
        }));
    });
}

/// How many panics the program has seen so far.
pub fn panics() -> u32 {
    PANICS.load(Ordering::Relaxed)
}

/// The text a panic was raised with.
fn payload_text(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s
    } else {
        "no message"
    }
}

/// Run `work`, turning a panic into an internal error that names `what`.
/// The panic itself is already in the log, written by the hook.
pub fn catch<T>(what: &str, work: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    match panic::catch_unwind(AssertUnwindSafe(work)) {
        Ok(result) => result,
        Err(payload) => Err(Error::internal(format!(
            "Internal error in {what}: {}. This is a bug in Stannum; please report it.",
            payload_text(payload.as_ref())
        ))),
    }
}

/// Lock `mutex` even if a panic left it poisoned. The work that panicked
/// was stopped, and what it held is used as it was left, so one bug doesn't
/// lock the person out of everything after it.
pub fn relock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned: PoisonError<_>| {
        log::warn!(
            target: TARGET,
            "Carrying on after an internal error interrupted earlier work"
        );
        mutex.clear_poison();
        poisoned.into_inner()
    })
}

/// Versions and platform for a problem report. It holds nothing from the
/// case: no file names, element names or values.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Diagnostics {
    /// stannum-core's version.
    pub core: &'static str,
    /// The command set's version (API_VERSION in the core).
    pub api_version: u32,
    /// rs-power's version (power flow).
    pub rs_power: &'static str,
    /// nodal's version (topology).
    pub nodal: &'static str,
    /// The operating system ("windows", "macos", "linux").
    pub os: &'static str,
    /// The processor ("x86_64", "aarch64").
    pub arch: &'static str,
    /// Whether this is a debug build.
    pub debug_build: bool,
    /// Processor threads available to solves.
    pub threads: usize,
    /// Internal errors (panics) since the program started.
    pub internal_errors: u32,
}

pub fn diagnostics() -> Diagnostics {
    Diagnostics {
        core: env!("CARGO_PKG_VERSION"),
        api_version: crate::commands::API_VERSION,
        rs_power: rs_power::VERSION,
        nodal: nodal::VERSION,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        debug_build: cfg!(debug_assertions),
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        internal_errors: panics(),
    }
}

/// For tests of crash handling: the command named by `STANNUM_FAULT_COMMAND`
/// panics before it runs. Only built with the `fault-injection` feature.
#[cfg(feature = "fault-injection")]
pub fn inject(command: &str) {
    static NAME: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let name = NAME.get_or_init(|| std::env::var("STANNUM_FAULT_COMMAND").ok());
    if name.as_deref() == Some(command) {
        panic!("fault injected into {command} for testing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorKind, logbook};

    #[test]
    fn a_panic_becomes_an_internal_error_and_a_log_line() {
        logbook::install();
        let before = panics();
        let mark = logbook::latest();
        let result: Result<(), Error> = catch("a test", || panic!("boom {}", 7));
        let e = result.unwrap_err();
        assert!(e.is(ErrorKind::Internal));
        assert!(
            e.message.starts_with("Internal error in a test: boom 7."),
            "{}",
            e.message
        );
        assert!(panics() > before);
        let lines = logbook::since(mark);
        assert!(
            lines
                .iter()
                .any(|l| l.level == "error" && l.message.contains(": boom 7")),
            "{lines:#?}"
        );
    }

    #[test]
    fn work_that_returns_is_untouched() {
        assert_eq!(catch("sums", || Ok(2 + 2)).unwrap(), 4);
        let e = catch::<()>("sums", || Err(Error::not_found("gone"))).unwrap_err();
        assert!(e.is(ErrorKind::NotFound));
    }

    #[test]
    fn a_poisoned_lock_still_opens() {
        let m = std::sync::Arc::new(Mutex::new(1));
        let held = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = held.lock().unwrap();
            panic!("poison it");
        })
        .join();
        assert!(m.is_poisoned());
        *relock(&m) += 1;
        assert!(!m.is_poisoned());
        assert_eq!(*m.lock().unwrap(), 2);
    }

    #[test]
    fn diagnostics_name_versions() {
        let d = diagnostics();
        assert_eq!(d.core, env!("CARGO_PKG_VERSION"));
        assert!(!d.rs_power.is_empty() && !d.nodal.is_empty());
        assert!(!d.os.is_empty());
    }
}
