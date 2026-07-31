//! The host's log file (ADR-0007).
//!
//! The host silently ignores malformed frames to stay fail-open (ADR-0004), which makes adapter
//! bugs invisible unless something records them. The pane cannot: it stays passive (ADR-0001). So
//! diagnostics — every discarded frame, every producer-thread death — go to a file here. Never
//! initialised on the hook path, which must stay cold (ADR-0008); only `run_host` calls `init`.

use std::path::{Path, PathBuf};

use tracing_appender::non_blocking::WorkerGuard;

const LOG_DIR_NAME: &str = "learnwhile";
const LOG_FILE_PREFIX: &str = "host.log";

/// `$XDG_STATE_HOME/learnwhile`, falling back to `$HOME/.local/state/learnwhile` — the XDG Base
/// Directory default for state (a log is state, not data). Hand-rolled to match [`crate::socket`]
/// and [`crate::storage`].
pub fn default_log_dir() -> PathBuf {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => match std::env::var_os("HOME") {
            Some(home) if !home.is_empty() => PathBuf::from(home).join(".local").join("state"),
            _ => std::env::temp_dir(),
        },
    };
    base.join(LOG_DIR_NAME)
}

/// Install a file logger under `dir` with **daily rotation**, bounding growth so a long-lived host
/// cannot fill the disk (milestone sub-task 3). Returns a guard that must be held for the process's
/// lifetime: dropping it flushes buffered lines, which is what makes logs survive an abrupt exit.
///
/// A second call is a no-op — the global subscriber can only be set once — so tests that need to
/// capture events use a scoped subscriber instead of this.
pub fn init(dir: &Path) -> WorkerGuard {
    std::fs::create_dir_all(dir).ok();
    let appender = tracing_appender::rolling::daily(dir, LOG_FILE_PREFIX);
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        // A file, not a terminal: no colour codes, and drop the module-path target for brevity.
        .with_ansi(false)
        .with_target(false)
        .try_init();
    guard
}
