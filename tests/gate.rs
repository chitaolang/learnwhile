//! The Prompt Gate (M7), tested through the host boundary and, for fail-open, the real binary.
//!
//! Host-boundary tests drive the gate exactly as the `--gate` hook does: `gate_query` writes a gate
//! query over the real socket and reads back the verdict, then assertions are on the pane and the
//! verdict. Fail-open and the block output are claims about the real binary, so those run it as a
//! subprocess.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use crossterm::event::KeyCode;
use learnwhile::frame::Verdict;
use learnwhile::testing::{
    PLACEHOLDER_CARD_BACK, PLACEHOLDER_CARD_FRONT, spawn_test_host, spawn_test_host_with_cards,
};

const REVEAL: KeyCode = KeyCode::Char(' ');
const GOOD: KeyCode = KeyCode::Char('3');

#[test]
fn with_no_gate_the_idle_pane_never_shows_a_card() {
    // The plain open/close path: a card shows while Waiting and the pane goes idle on close. No gate
    // query was ever made, so owed-card-while-idle stays off (ADR-0015) and v1 behavior holds.
    let host = spawn_test_host();
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);
    host.close("s");
    host.wait_for_absent(PLACEHOLDER_CARD_FRONT);
    assert!(
        host.pane().contains("Not waiting"),
        "the pane should be idle after close. Pane:\n{}",
        host.pane()
    );
}

#[test]
fn an_owed_review_blocks_the_next_prompt() {
    let host = spawn_test_host();

    // First prompt: allowed, and a card surfaces into the wait.
    assert_eq!(host.gate_query("s"), Verdict::Allow);
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // The agent returns without a review being done. The owed card stays shown while idle so the
    // debt is payable, and the heading says why it is up.
    host.close("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);
    assert!(
        host.pane().contains("Review to continue"),
        "the owed card should be held for review. Pane:\n{}",
        host.pane()
    );

    // The next prompt is blocked, because the Review is still owed.
    assert_eq!(host.gate_query("s"), Verdict::Block);
    assert!(
        host.pane().contains(PLACEHOLDER_CARD_FRONT),
        "the owed card should still be up after the block. Pane:\n{}",
        host.pane()
    );
}

#[test]
fn completing_the_review_from_the_idle_pane_allows_the_next_prompt() {
    let host = spawn_test_host();

    assert_eq!(host.gate_query("s"), Verdict::Allow);
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // Agent returns; pay the debt from the idle pane (not Waiting): reveal, then rate.
    host.close("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);
    host.press(REVEAL);
    host.wait_for(PLACEHOLDER_CARD_BACK);
    host.press(GOOD);
    host.wait_for_absent(PLACEHOLDER_CARD_FRONT);

    // With the debt paid, the next prompt goes through.
    assert_eq!(host.gate_query("s"), Verdict::Allow);
}

#[test]
fn reviewing_during_the_wait_also_clears_the_debt() {
    let host = spawn_test_host();

    assert_eq!(host.gate_query("s"), Verdict::Allow);
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // Pay it during the wait, before the agent even returns.
    host.press(REVEAL);
    host.wait_for(PLACEHOLDER_CARD_BACK);
    host.press(GOOD);

    // The next prompt is allowed: one Review this cycle was enough.
    assert_eq!(host.gate_query("s"), Verdict::Allow);
}

#[test]
fn the_gate_allows_when_nothing_is_reviewable() {
    // An empty deck surfaces no card, so no debt is ever incurred and the gate never blocks.
    let host = spawn_test_host_with_cards(&[]);
    assert_eq!(host.gate_query("s"), Verdict::Allow);
    assert_eq!(host.gate_query("s"), Verdict::Allow);
    assert!(
        !host.pane().contains("Review to continue"),
        "nothing was owed, so nothing should be held. Pane:\n{}",
        host.pane()
    );
}

// --- Fail-open and the block output, exercised on the real binary. ---

const OPEN_PAYLOAD: &str =
    r#"{"session_id":"abc","hook_event_name":"UserPromptSubmit","cwd":"/tmp"}"#;

/// Generous enough not to flake on a loaded box, tight enough that a hook which actually hung on a
/// wedged host would fail it.
const BUDGET: Duration = Duration::from_secs(2);

struct HookRun {
    exit_code: Option<i32>,
    stdout: String,
    elapsed: Duration,
}

fn run_gated_hook(socket_dir: &std::path::Path, payload: &str) -> HookRun {
    let mut command = Command::cargo_bin("learnwhile").expect("built binary");
    command
        .arg("hook")
        .arg("--gate")
        .env("XDG_RUNTIME_DIR", socket_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let started = Instant::now();
    let mut child = command.spawn().expect("spawn hook");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(payload.as_bytes());
    }
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut stdout)
        .expect("read stdout");
    let status = child.wait().expect("wait for hook");
    HookRun {
        exit_code: status.code(),
        stdout,
        elapsed: started.elapsed(),
    }
}

#[test]
fn gated_hook_fails_open_and_is_silent_with_no_host() {
    let dir = tempfile::tempdir().expect("temp dir");
    let run = run_gated_hook(dir.path(), OPEN_PAYLOAD);
    assert_eq!(
        run.exit_code,
        Some(0),
        "a gated hook must exit 0 with no host"
    );
    assert!(
        run.stdout.trim().is_empty(),
        "with no host the prompt must not be blocked, got stdout: {:?}",
        run.stdout
    );
    assert!(run.elapsed < BUDGET, "gated hook took {:?}", run.elapsed);
}

/// A fake host that answers the first gate query with `verdict`, so we can test the hook's own
/// output without booting a real host. It writes the verdict with the real encoder, so the test
/// exercises the actual wire form the host produces.
fn fake_host_replying(socket_path: std::path::PathBuf, verdict: Verdict) {
    std::thread::spawn(move || {
        let listener = UnixListener::bind(&socket_path).expect("bind fake host");
        if let Ok((stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream);
            let mut query = String::new();
            let _ = reader.read_line(&mut query); // the gate query line
            let _ = reader.get_mut().write_all(verdict.to_line().as_bytes());
            let _ = reader.get_mut().flush();
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

#[test]
fn gated_hook_prints_the_block_verdict_when_the_host_blocks() {
    let dir = tempfile::tempdir().expect("temp dir");
    fake_host_replying(dir.path().join("learnwhile.sock"), Verdict::Block);
    // Give the fake host a moment to bind before the hook connects.
    std::thread::sleep(Duration::from_millis(50));

    let run = run_gated_hook(dir.path(), OPEN_PAYLOAD);
    assert_eq!(run.exit_code, Some(0), "a blocked hook still exits 0");
    assert!(
        run.stdout.contains(r#""decision":"block""#),
        "a block verdict must print the block decision, got: {:?}",
        run.stdout
    );
}

#[test]
fn gated_hook_is_silent_when_the_host_allows() {
    let dir = tempfile::tempdir().expect("temp dir");
    fake_host_replying(dir.path().join("learnwhile.sock"), Verdict::Allow);
    std::thread::sleep(Duration::from_millis(50));

    let run = run_gated_hook(dir.path(), OPEN_PAYLOAD);
    assert_eq!(run.exit_code, Some(0));
    assert!(
        run.stdout.trim().is_empty(),
        "an allowed prompt must not be blocked, got stdout: {:?}",
        run.stdout
    );
}
