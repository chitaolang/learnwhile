//! The Claude Code Trigger Adapter (ADR-0001), shipped as a subcommand (ADR-0008).
//!
//! This module is the cold path. It resolves the socket path, writes one frame, and exits 0 —
//! whatever happens. No SQLite, no TUI, no config load, no logging. Keeping it that way is a
//! standing discipline, not a one-time change: shared startup code is exactly where
//! initialisation accumulates.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use chrono::Utc;

use crate::frame::{FrameType, TriggerFrame};

pub const ADAPTER_NAME: &str = "claude-code";

/// How long the hook will wait on a wedged-but-alive host before giving up.
///
/// A refused connect returns instantly, so this bounds the only case that can actually stall the
/// developer's agent (ADR-0004).
const WRITE_TIMEOUT: Duration = Duration::from_millis(50);

/// Used when the hook payload carries no session id. Opens and closes still pair up with each
/// other, which is what Trigger identity needs; it only collapses if two agents both hit this
/// path, which would mean Claude Code stopped sending session ids at all.
const UNKNOWN_SESSION: &str = "unknown-session";

/// Map a Claude Code `hook_event_name` onto a Trigger transition.
///
/// A Trigger opens when the developer hands off and closes when the agent needs them back
/// (ADR-0001). Anything not named here is not a handoff boundary and is ignored.
///
/// NOTE: ADR-0001 and the v1 spec name `PermissionRequest` and `Elicitation` as close events.
/// Neither exists in Claude Code — the valid events are PreToolUse, PostToolUse,
/// UserPromptSubmit, Stop, SubagentStop, SessionStart, SessionEnd, and Notification. The
/// permission prompt surfaces as `Notification` (matcher `permission_prompt`), so that is what
/// closes a Trigger here. The ADR needs amending to match; this comment is the marker.
pub fn transition_for(event_name: &str) -> Option<FrameType> {
    match event_name {
        // The developer has handed control to the agent.
        "UserPromptSubmit" => Some(FrameType::TriggerOpen),
        // The agent has finished its turn.
        "Stop" => Some(FrameType::TriggerClose),
        // The agent needs the developer: a permission prompt or an idle input wait.
        "Notification" => Some(FrameType::TriggerClose),
        // Deliberately not a close: a subagent finishing does not hand control back to the
        // developer, and treating it as one would clear a card mid-wait.
        _ => None,
    }
}

/// Run the adapter. Returns nothing to report because nothing is reportable: every outcome,
/// success or failure, is a silent exit 0 (ADR-0004).
pub fn run(socket_path: &Path, forced: Option<FrameType>) {
    let mut stdin_buf = String::new();
    let read_ok = std::io::stdin().read_to_string(&mut stdin_buf).is_ok();

    let payload: Option<serde_json::Value> = if read_ok {
        serde_json::from_str(&stdin_buf).ok()
    } else {
        None
    };

    let frame_type = match forced {
        Some(forced) => forced,
        None => {
            let event_name = payload
                .as_ref()
                .and_then(|p| p.get("hook_event_name"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            match transition_for(event_name) {
                Some(t) => t,
                // Not a handoff boundary — nothing to say.
                None => return,
            }
        }
    };

    let session = payload
        .as_ref()
        .and_then(|p| p.get("session_id"))
        .and_then(|v| v.as_str())
        .unwrap_or(UNKNOWN_SESSION);

    let frame = TriggerFrame::new(frame_type, ADAPTER_NAME, session, Utc::now());
    let Ok(line) = frame.to_line() else {
        return;
    };

    send(socket_path, line.as_bytes());
}

/// Fire and forget. A missing or refused socket returns instantly; a wedged host is bounded by
/// the write timeout.
fn send(socket_path: &Path, bytes: &[u8]) {
    let Ok(mut stream) = UnixStream::connect(socket_path) else {
        return;
    };
    if stream.set_write_timeout(Some(WRITE_TIMEOUT)).is_err() {
        return;
    }
    let _ = stream.write_all(bytes);
    let _ = stream.flush();
}
