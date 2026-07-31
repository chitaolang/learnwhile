//! Honest scheduling (M3), tested through the host boundary: drive Triggers and rating keys, and
//! assert on the pane and the database. Every branch of ADR-0002's order — a due card, a new card,
//! the idle state — is observed in the pane, never by reaching into the selection function. The
//! strict due-before-new *priority* is unit-tested in `learning.rs`; here we watch each branch
//! surface for real.

use chrono::Duration;
use crossterm::event::KeyCode;
use learnwhile::testing::{
    PLACEHOLDER_CARD_BACK, PLACEHOLDER_CARD_FRONT, spawn_test_host,
    spawn_test_host_with_cap_and_cards, spawn_test_host_with_cards,
};

const REVEAL: KeyCode = KeyCode::Char(' ');
const GOOD: KeyCode = KeyCode::Char('3');

/// Rate the currently-shown card Good, revealing first.
fn review_current(host: &learnwhile::testing::TestHost, back: &str) {
    host.press(REVEAL);
    host.wait_for(back);
    host.press(GOOD);
}

#[test]
fn a_reviewed_card_returns_only_once_it_is_due() {
    // The ADR-0002 guarantee, the one users cannot verify for themselves: a card is never pulled
    // forward ahead of its due date, and comes back exactly when it is due.
    let host = spawn_test_host();
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT); // the new-card branch
    review_current(&host, PLACEHOLDER_CARD_BACK);

    // Rated Good: due days out, nothing else to show. The idle branch, and the card is NOT pulled
    // forward even though the developer is still Waiting.
    host.wait_for("Due now:");
    assert!(
        !host.pane().contains(PLACEHOLDER_CARD_FRONT),
        "a not-yet-due card was surfaced ahead of its due date. Pane:\n{}",
        host.pane()
    );

    // Advance past the due date and re-trigger: now the due branch fires and the card returns.
    host.clock.advance(Duration::days(60));
    host.close("s");
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    host.shutdown();
}

#[test]
fn the_daily_cap_stops_new_cards_within_a_day() {
    let host = spawn_test_host_with_cap_and_cards(1, &[("q1", "a1"), ("q2", "a2")]);
    host.open("s");
    host.wait_for("q1"); // first new card, under the cap
    review_current(&host, "a1");

    // Cap of 1 is now spent: the second new card is not introduced, even while Waiting.
    host.wait_for("Due now:");
    let pane = host.pane();
    assert!(
        !pane.contains("q2"),
        "the cap was exceeded: a second new card appeared. Pane:\n{pane}"
    );
    assert!(
        pane.contains("New remaining: 0"),
        "idle pane should report no new cards remaining today. Pane:\n{pane}"
    );

    host.shutdown();
}

#[test]
fn the_daily_cap_rolls_over_after_local_midnight() {
    let host = spawn_test_host_with_cap_and_cards(1, &[("q1", "a1"), ("q2", "a2")]);
    host.open("s");
    host.wait_for("q1");
    review_current(&host, "a1");
    host.wait_for("New remaining: 0"); // cap spent today

    // Advance past local midnight (25h crosses one, in any timezone) and re-trigger. The cap resets,
    // so a card is introduced again — the Question footer proves a card is up rather than the idle.
    host.clock.advance(Duration::hours(25));
    host.close("s");
    host.open("s");
    host.wait_for("space reveal");

    host.shutdown();
}

#[test]
fn the_cap_survives_a_host_restart() {
    let host = spawn_test_host_with_cap_and_cards(1, &[("q1", "a1"), ("q2", "a2")]);
    host.open("s");
    host.wait_for("q1");
    review_current(&host, "a1");
    host.wait_for("New remaining: 0");

    // Reboot against the same database. Today's introduction is in review_history, so the cap is
    // still spent and the second card is not offered.
    let host = host.restart();
    host.open("s");
    host.wait_for("Due now:");
    assert!(
        !host.pane().contains("q2"),
        "the cap did not survive the restart: a second new card appeared. Pane:\n{}",
        host.pane()
    );

    host.shutdown();
}

#[test]
fn the_idle_pane_shows_due_new_and_next_due_counts() {
    let host = spawn_test_host();
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);
    review_current(&host, PLACEHOLDER_CARD_BACK);

    host.wait_for("Due now:");
    let pane = host.pane();
    // Nothing due, no new cards left, and the reviewed card has a future due time.
    assert!(pane.contains("Due now: 0"), "Pane:\n{pane}");
    assert!(pane.contains("New remaining: 0"), "Pane:\n{pane}");
    assert!(pane.contains("Next due:"), "Pane:\n{pane}");
    assert!(
        !pane.contains("nothing scheduled"),
        "a due time should be shown, not 'nothing scheduled'. Pane:\n{pane}"
    );

    host.shutdown();
}

#[test]
fn the_idle_pane_handles_an_empty_deck() {
    // A deck with no cards must render something sensible, not a blank frame or a panic.
    let host = spawn_test_host_with_cards(&[]);
    host.open("s");
    host.wait_for("Due now:");
    let pane = host.pane();
    assert!(pane.contains("Due now: 0"), "Pane:\n{pane}");
    assert!(pane.contains("New remaining: 0"), "Pane:\n{pane}");
    assert!(
        pane.contains("nothing scheduled"),
        "an empty deck has no next due time. Pane:\n{pane}"
    );

    host.shutdown();
}
