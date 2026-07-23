//! Fail-open, tested at the adapter as a subprocess.
//!
//! This is the one place a subprocess test is warranted: the claim is that *the real binary*
//! exits 0, and that cannot be made about an in-process harness. If any of these fail, installing
//! LearnWhile costs the developer their agent — the thing ADR-0004 exists to prevent.

use std::io::Read;
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;

/// Generous enough not to flake on a loaded CI box, tight enough that a hook which actually
/// blocked on a wedged host would fail it.
const BUDGET: Duration = Duration::from_secs(2);

/// A realistic Claude Code hook payload for a prompt submission.
const OPEN_PAYLOAD: &str =
    r#"{"session_id":"abc-123","hook_event_name":"UserPromptSubmit","cwd":"/tmp"}"#;

struct Outcome {
    exit_code: Option<i32>,
    elapsed: Duration,
}

fn run_hook(socket_path: &std::path::Path, stdin_payload: &str) -> Outcome {
    let mut command = Command::cargo_bin("learnwhile").expect("built binary");
    command
        .arg("hook")
        .env("XDG_RUNTIME_DIR", socket_path.parent().expect("parent dir"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let started = Instant::now();
    let mut child = command.spawn().expect("spawn hook");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        // A failed write is itself fine — the hook may have already exited. What must not happen
        // is this test blocking or the hook returning non-zero.
        let _ = stdin.write_all(stdin_payload.as_bytes());
    }
    let status = child.wait().expect("wait for hook");

    Outcome {
        exit_code: status.code(),
        elapsed: started.elapsed(),
    }
}

fn assert_fail_open(outcome: Outcome, scenario: &str) {
    assert_eq!(
        outcome.exit_code,
        Some(0),
        "hook must exit 0 {scenario}, so a learning tool can never take down real work"
    );
    assert!(
        outcome.elapsed < BUDGET,
        "hook took {:?} {scenario}, over the {BUDGET:?} budget — a hung host must not stall the agent",
        outcome.elapsed
    );
}

/// The developer has not started LearnWhile at all. Installing the hook must cost them nothing.
#[test]
fn hook_exits_zero_with_no_host_running() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    let outcome = run_hook(&socket_path, OPEN_PAYLOAD);
    assert_fail_open(outcome, "when no host is running");
}

/// A socket file left behind by a crashed host: connecting to it is refused.
#[test]
fn hook_exits_zero_against_a_stale_socket_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    // Bind then drop the listener, leaving the file with nothing behind it.
    let listener = UnixListener::bind(&socket_path).expect("bind");
    drop(listener);
    assert!(socket_path.exists(), "the stale socket file should remain");

    let outcome = run_hook(&socket_path, OPEN_PAYLOAD);
    assert_fail_open(outcome, "against a stale socket file");
}

/// A host that is alive but wedged: bound and accepting, never reading. Connect succeeds, so the
/// write timeout is the only thing standing between the developer and a stalled agent.
#[test]
fn hook_exits_zero_against_a_wedged_host() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    let listener = UnixListener::bind(&socket_path).expect("bind");
    let wedged = std::thread::spawn(move || {
        // Accept and then never read a byte, holding the connection open.
        if let Ok((stream, _)) = listener.accept() {
            std::thread::sleep(Duration::from_secs(5));
            drop(stream);
        }
    });

    let outcome = run_hook(&socket_path, OPEN_PAYLOAD);
    assert_fail_open(outcome, "against a wedged host");

    let _ = wedged.join();
}

/// Garbage on stdin is an adapter-side failure, and must be as silent as any other.
#[test]
fn hook_exits_zero_on_malformed_stdin() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    let outcome = run_hook(&socket_path, "not json at all");
    assert_fail_open(outcome, "on malformed stdin");
}

/// An event that is not a handoff boundary is simply not a Trigger. It must still exit 0.
#[test]
fn hook_exits_zero_on_an_unmapped_event() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    let outcome = run_hook(
        &socket_path,
        r#"{"session_id":"abc-123","hook_event_name":"PreToolUse"}"#,
    );
    assert_fail_open(outcome, "on an event that is not a handoff boundary");
}

/// The adapter's actual job: a real host receives exactly the frame the hook claims to send.
#[test]
fn hook_writes_a_well_formed_open_frame() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    let listener = UnixListener::bind(&socket_path).expect("bind");
    let receiver = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut received = String::new();
        stream.read_to_string(&mut received).expect("read frame");
        received
    });

    let outcome = run_hook(&socket_path, OPEN_PAYLOAD);
    assert_fail_open(outcome, "when a host is listening");

    let received = receiver.join().expect("receiver thread");
    assert!(
        received.ends_with('\n'),
        "frames are newline-terminated (ADR-0007), got {received:?}"
    );

    let frame: serde_json::Value =
        serde_json::from_str(received.trim_end()).expect("frame parses as JSON");
    assert_eq!(frame["v"], 1);
    assert_eq!(frame["type"], "trigger_open");
    assert_eq!(frame["adapter"], "claude-code");
    assert_eq!(frame["session"], "abc-123");
    assert!(
        frame.get("at").and_then(|at| at.as_str()).is_some(),
        "frame carries an RFC3339 timestamp"
    );
}

/// `Stop` is the agent handing control back, so it closes the Trigger.
#[test]
fn hook_maps_stop_to_a_close_frame() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("learnwhile.sock");

    let listener = UnixListener::bind(&socket_path).expect("bind");
    let receiver = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut received = String::new();
        stream.read_to_string(&mut received).expect("read frame");
        received
    });

    let outcome = run_hook(
        &socket_path,
        r#"{"session_id":"abc-123","hook_event_name":"Stop"}"#,
    );
    assert_fail_open(outcome, "on Stop");

    let received = receiver.join().expect("receiver thread");
    let frame: serde_json::Value =
        serde_json::from_str(received.trim_end()).expect("frame parses as JSON");
    assert_eq!(frame["type"], "trigger_close");
}
