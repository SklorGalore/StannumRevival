//! The application log: what the core and its solvers report while they work.
//!
//! [`install`] registers a [`log`] logger that keeps the most recent lines in
//! memory. The desktop shell streams new lines to the log panel, and the dev
//! bridge serves them on request, both through [`since`].

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lines kept in memory. Older lines are dropped first.
const CAPACITY: usize = 5000;

/// Crates whose debug lines are kept. Everything else logs warnings and errors only.
const OURS: [&str; 3] = ["stannum_core", "rs_power", "nodal"];

/// One log line.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Entry {
    /// Increases by one per line, so a reader can ask for what it hasn't seen.
    pub seq: u64,
    /// Milliseconds since the Unix epoch.
    pub time: u64,
    /// "error", "warn", "info" or "debug".
    pub level: &'static str,
    /// The part of the program that wrote it, such as "rs-power".
    pub source: String,
    pub message: String,
}

struct Logbook {
    entries: Mutex<VecDeque<Entry>>,
    next: AtomicU64,
}

static BOOK: Logbook = Logbook {
    entries: Mutex::new(VecDeque::new()),
    next: AtomicU64::new(1),
};

fn source(target: &str) -> String {
    let krate = target.split("::").next().unwrap_or(target);
    match krate {
        "stannum_core" => "core".into(),
        other => other.replace('_', "-"),
    }
}

fn ours(target: &str) -> bool {
    let krate = target.split("::").next().unwrap_or(target);
    OURS.contains(&krate)
}

impl log::Log for Logbook {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let limit = if ours(metadata.target()) {
            log::Level::Debug
        } else {
            log::Level::Warn
        };
        metadata.level() <= limit
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        push(
            record.level(),
            source(record.target()),
            record.args().to_string(),
        );
    }

    fn flush(&self) {}
}

fn push(level: log::Level, source: String, message: String) {
    let level = match level {
        log::Level::Error => "error",
        log::Level::Warn => "warn",
        log::Level::Info => "info",
        _ => "debug",
    };
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let Ok(mut entries) = BOOK.entries.lock() else {
        return;
    };
    // Numbered under the lock so lines are stored in order.
    let seq = BOOK.next.fetch_add(1, Ordering::Relaxed);
    if entries.len() == CAPACITY {
        entries.pop_front();
    }
    entries.push_back(Entry {
        seq,
        time,
        level,
        source,
        message,
    });
}

/// Register the logger. Later calls, or another logger already set, are ignored.
pub fn install() {
    if log::set_logger(&BOOK).is_ok() {
        log::set_max_level(log::LevelFilter::Debug);
    }
}

/// Lines numbered above `seq`, oldest first. `since(0)` returns every line kept.
pub fn since(seq: u64) -> Vec<Entry> {
    let Ok(entries) = BOOK.entries.lock() else {
        return Vec::new();
    };
    // Sequence numbers are contiguous, so skip straight to the first new line.
    let first = entries.front().map_or(0, |e| e.seq);
    let skip = (seq + 1).saturating_sub(first) as usize;
    entries.iter().skip(skip).cloned().collect()
}

/// The number of the newest line, or 0 before anything is logged.
pub fn latest() -> u64 {
    BOOK.next.load(Ordering::Relaxed) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_our_debug_lines_and_others_warnings() {
        install();
        let before = latest();
        log::debug!(target: "stannum_core::workspace", "ours");
        log::debug!(target: "tao::platform", "chatter");
        log::warn!(target: "tao::platform", "theirs");
        let got: Vec<_> = since(before)
            .into_iter()
            .filter(|e| e.message == "ours" || e.message == "chatter" || e.message == "theirs")
            .map(|e| (e.source, e.level, e.message))
            .collect();
        assert_eq!(
            got,
            vec![
                ("core".to_string(), "debug", "ours".to_string()),
                ("tao".to_string(), "warn", "theirs".to_string()),
            ]
        );
    }

    #[test]
    fn since_returns_only_newer_lines() {
        install();
        log::info!(target: "rs_power::solver", "first");
        let mark = latest();
        log::info!(target: "rs_power::solver", "second");
        let newer = since(mark);
        assert!(newer.iter().all(|e| e.seq > mark));
        assert!(
            newer
                .iter()
                .any(|e| e.message == "second" && e.source == "rs-power")
        );
        assert!(!newer.iter().any(|e| e.message == "first"));
    }
}
