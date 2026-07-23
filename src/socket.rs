//! Socket path resolution (ADR-0004).
//!
//! This is the *only* thing the hook path is permitted to resolve — no config load, no database,
//! no TUI (ADR-0008).

use std::path::PathBuf;

const SOCKET_NAME: &str = "learnwhile.sock";

/// `$XDG_RUNTIME_DIR/learnwhile.sock`, falling back to `/tmp` when the variable is unset.
///
/// The fallback is documented rather than clever: `$XDG_RUNTIME_DIR` is absent on plenty of macOS
/// and container setups, and a hook that cannot resolve a path would be a silent no-op forever.
pub fn default_socket_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir).join(SOCKET_NAME),
        _ => std::env::temp_dir().join(SOCKET_NAME),
    }
}
