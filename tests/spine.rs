//! The Trigger spine, tested through the host boundary.
//!
//! Every test here drives the host by writing real frames down a real unix socket and asserts on
//! what the developer would see in the pane. Nothing reaches into the open-Trigger set: a test
//! that breaks when that is restructured, while the developer's experience is unchanged, is a bad
//! test.

use chrono::Duration;
use crossterm::event::KeyCode;
use learnwhile::testing::{PLACEHOLDER_CARD_FRONT, spawn_test_host, spawn_test_host_with_expiry};

const IDLE_MARKER: &str = "Not waiting";

#[test]
fn opening_a_trigger_surfaces_a_card_and_closing_clears_it() {
    let host = spawn_test_host();
    host.wait_for(IDLE_MARKER);

    host.open("session-a");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    host.close("session-a");
    host.wait_for(IDLE_MARKER);
    host.wait_for_absent(PLACEHOLDER_CARD_FRONT);

    host.shutdown();
}

#[test]
fn a_duplicate_open_is_idempotent() {
    let host = spawn_test_host();

    host.open("session-a");
    host.open("session-a");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // One close must be enough. If the duplicate had been counted, the card would still be up.
    host.close("session-a");
    host.wait_for(IDLE_MARKER);

    host.shutdown();
}

#[test]
fn a_close_for_an_unknown_trigger_is_ignored() {
    let host = spawn_test_host();

    host.open("session-a");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // A close for a Trigger that was never opened must not clear someone else's card.
    host.close("session-never-opened");
    host.close("session-a");
    host.wait_for(IDLE_MARKER);

    host.shutdown();
}

/// The ADR-0005 case: waiting is aggregate, so one agent returning must not clear the card while
/// the developer is still idle on another.
#[test]
fn two_overlapping_triggers_keep_the_card_up_until_both_close() {
    let host = spawn_test_host();

    host.open("session-a");
    host.open("session-b");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // `close` returns only once the host has applied it, so this assertion is not a race: at this
    // point one agent has definitively returned and the developer is still idle on the other.
    host.close("session-a");
    assert!(
        host.pane().contains(PLACEHOLDER_CARD_FRONT),
        "one agent returning cleared the card while still waiting on the other. Pane:\n{}",
        host.pane()
    );

    host.close("session-b");
    host.wait_for(IDLE_MARKER);

    host.shutdown();
}

/// Order should not matter: closing them the other way round behaves the same.
#[test]
fn overlapping_triggers_clear_on_the_last_close_whichever_it_is() {
    let host = spawn_test_host();

    host.open("session-a");
    host.open("session-b");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    host.close("session-b");
    assert!(
        host.pane().contains(PLACEHOLDER_CARD_FRONT),
        "the card should survive the second Trigger closing first. Pane:\n{}",
        host.pane()
    );

    host.close("session-a");
    host.wait_for(IDLE_MARKER);

    host.shutdown();
}

/// Triggers are keyed by `(adapter, session)`, so two agents in the same session id from
/// different adapters are still two Triggers. M1 has one adapter, but the key is the contract
/// ADR-0005 rests on, and a future Codex adapter drops in on it.
#[test]
fn a_close_from_a_different_session_does_not_clear_another() {
    let host = spawn_test_host();

    host.open("session-a");
    host.open("session-b");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    host.close("session-c");
    assert!(
        host.pane().contains(PLACEHOLDER_CARD_FRONT),
        "an unrelated close cleared the card. Pane:\n{}",
        host.pane()
    );

    host.shutdown();
}

/// ADR-0006: a lost close must not pin the card up forever.
#[test]
fn a_trigger_whose_close_never_arrives_expires_and_clears_the_card() {
    let host = spawn_test_host_with_expiry(Duration::minutes(30));

    host.open("session-crashed");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // Nothing has drained yet: the Trigger is well inside its expiry.
    host.clock.advance(Duration::minutes(29));
    host.tick();
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // Past the expiry, the sweep drops it and the card clears exactly as a real close would.
    host.clock.advance(Duration::minutes(2));
    host.tick();
    host.wait_for(IDLE_MARKER);

    host.shutdown();
}

/// Expiry is measured from open and deliberately not refreshed (ADR-0006), so a duplicate open
/// cannot extend a phantom Trigger's life indefinitely.
#[test]
fn a_repeated_open_does_not_refresh_the_expiry() {
    let host = spawn_test_host_with_expiry(Duration::minutes(30));

    host.open("session-a");
    host.clock.advance(Duration::minutes(20));
    host.open("session-a");

    host.clock.advance(Duration::minutes(11));
    host.tick();
    host.wait_for(IDLE_MARKER);

    host.shutdown();
}

#[test]
fn malformed_input_is_ignored_and_the_host_keeps_serving() {
    let host = spawn_test_host();

    // Not JSON at all.
    host.send_raw("this is not json\n");
    // Valid JSON, wrong shape.
    host.send_raw("{\"hello\":\"world\"}\n");
    // A version the host does not recognise (ADR-0007).
    host.send_raw(
        "{\"v\":99,\"type\":\"trigger_open\",\"adapter\":\"claude-code\",\
         \"session\":\"s\",\"at\":\"2026-01-01T09:00:00Z\"}\n",
    );
    // A line past the maximum length, which must not be buffered indefinitely.
    let oversized = format!("{}\n", "x".repeat(70 * 1024));
    host.send_raw(&oversized);

    // None of that surfaced a card...
    host.tick();
    host.wait_for(IDLE_MARKER);

    // ...and the accept loop is still alive for a valid frame.
    host.open("session-a");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    host.shutdown();
}

#[test]
fn the_pane_starts_idle_and_says_so() {
    let host = spawn_test_host();

    host.wait_for(IDLE_MARKER);
    let pane = host.pane();
    assert!(
        pane.contains("LearnWhile"),
        "the pane should identify itself. Pane:\n{pane}"
    );
    assert!(
        !pane.contains(PLACEHOLDER_CARD_FRONT),
        "no card should be up before any Trigger. Pane:\n{pane}"
    );

    host.shutdown();
}

#[test]
fn quitting_ends_the_host_loop() {
    let host = spawn_test_host();
    host.wait_for(IDLE_MARKER);

    // `shutdown` sends the quit key and joins the loop; it hangs if quit does not take.
    host.shutdown();
}

#[test]
fn a_shutdown_signal_ends_the_host_loop() {
    let host = spawn_test_host();
    host.wait_for(IDLE_MARKER);

    // A SIGINT/SIGTERM reaches the loop as `Event::Shutdown`; the loop must return so the real host
    // restores the terminal. Joins the loop, hanging if the signal is not honoured.
    host.shutdown_via_signal();
}

#[test]
fn escape_does_not_quit() {
    let host = spawn_test_host();
    host.wait_for(IDLE_MARKER);

    host.key(KeyCode::Esc);
    host.open("session-a");
    // The loop is still running, so the Trigger still lands.
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    host.shutdown();
}
