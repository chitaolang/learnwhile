//! The Claude Code Trigger Adapter (ADR-0001), shipped as a subcommand (ADR-0008).
//!
//! This module is the cold path. It resolves the socket path, writes one frame, and exits 0 —
//! whatever happens. No SQLite, no TUI, no config load, no logging. Keeping it that way is a
//! standing discipline, not a one-time change: shared startup code is exactly where
//! initialisation accumulates.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use chrono::Utc;

use crate::frame::{FrameType, TriggerFrame, Verdict};

pub const ADAPTER_NAME: &str = "claude-code";

/// How long the hook will wait on a wedged-but-alive host before giving up.
///
/// A refused connect returns instantly, so this bounds the only case that can actually stall the
/// developer's agent (ADR-0004).
const WRITE_TIMEOUT: Duration = Duration::from_millis(50);

/// The whole gate round-trip budget under `--gate` (ADR-0016): connect, write the query, read the
/// verdict. A gate is only ever as present as a host that answers within this, so exceeding it fails
/// open (the prompt proceeds). Kept short: this is on the developer's critical path when gated.
const GATE_TIMEOUT: Duration = Duration::from_millis(250);

/// What the hook prints to stdout to block a prompt (M7). Claude Code shows `reason` to the
/// developer and erases the prompt; the developer, not the agent, is who the gate addresses.
const BLOCK_OUTPUT: &str = r#"{"decision":"block","reason":"Finish one review to continue."}"#;

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

/// Run the adapter. Returns nothing to report because nothing is reportable: every outcome is a
/// silent exit 0 (ADR-0004). The one exception is a `--gate` block, which prints a verdict to stdout
/// for Claude Code to act on, then still exits 0.
pub fn run(socket_path: &Path, forced: Option<FrameType>, gated: bool) {
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

    // Under `--gate`, a handoff (an open) becomes a request/response: the host decides whether a
    // Review is owed, and opens the Trigger itself only when it allows the prompt (ADR-0016). Every
    // other event, and the un-gated hook, stays fire-and-forget.
    if gated && frame_type == FrameType::TriggerOpen {
        if query_gate(socket_path, session, GATE_TIMEOUT) == Verdict::Block {
            block_the_prompt();
        }
        return;
    }

    let frame = TriggerFrame::new(frame_type, ADAPTER_NAME, session, Utc::now());
    let Ok(line) = frame.to_line() else {
        return;
    };

    send(socket_path, line.as_bytes());
}

/// Ask the host for a verdict within `timeout`, failing open to `Allow` on any error, timeout, or
/// absent host so a gated prompt is never held by a problem on our side (ADR-0004). This is the one
/// gate client, shared by the real hook (which passes [`GATE_TIMEOUT`]) and the test harness (which
/// passes a generous one), so the harness exercises it rather than a copy.
pub(crate) fn query_gate(socket_path: &Path, session: &str, timeout: Duration) -> Verdict {
    let frame = TriggerFrame::new(FrameType::GateQuery, ADAPTER_NAME, session, Utc::now());
    let Ok(line) = frame.to_line() else {
        return Verdict::Allow;
    };
    ask(socket_path, line.as_bytes(), timeout).unwrap_or(Verdict::Allow)
}

/// One bounded round-trip: connect, write the query, read the verdict line. `None` on any failure.
fn ask(socket_path: &Path, bytes: &[u8], timeout: Duration) -> Option<Verdict> {
    let stream = UnixStream::connect(socket_path).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    let mut reader = BufReader::new(stream);
    reader.get_mut().write_all(bytes).ok()?;
    reader.get_mut().flush().ok()?;
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    Some(Verdict::from_line(&line))
}

/// Print the block verdict and flush it. The flush is load-bearing: `main` exits via
/// `std::process::exit`, which does not flush stdout, so a buffered verdict would be lost.
fn block_the_prompt() {
    let mut out = std::io::stdout();
    let _ = out.write_all(BLOCK_OUTPUT.as_bytes());
    let _ = out.flush();
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
