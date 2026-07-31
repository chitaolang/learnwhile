//! Session continuity (M4), tested through the host boundary: a Review survives the rhythm of real
//! agent use. A card in flight outlives the pane clearing — whether the clear came from the agent
//! returning or the Trigger expiring (ADR-0006) — and resumes in the state it was left in. An
//! unfinished card is never discarded or escalated (abandonment carries no penalty).
//!
//! These behaviours fall out of the existing state ownership (the Renderer stops drawing while
//! Learning keeps its state); these tests exist to guarantee that stays true.

use chrono::Duration;
use crossterm::event::KeyCode;
use learnwhile::testing::{
    PLACEHOLDER_CARD_BACK, PLACEHOLDER_CARD_FRONT, spawn_test_host, spawn_test_host_with_expiry,
};
use rusqlite::Connection;

const REVEAL: KeyCode = KeyCode::Char(' ');
const IDLE_MARKER: &str = "Due now:";

#[test]
fn a_revealed_card_resumes_still_revealed_after_the_agent_returns() {
    let host = spawn_test_host();
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);
    host.press(REVEAL);
    host.wait_for(PLACEHOLDER_CARD_BACK);

    // The agent comes back: the pane clears to idle, and the answer is off-screen.
    host.close("s");
    host.wait_for(IDLE_MARKER);
    assert!(
        !host.pane().contains(PLACEHOLDER_CARD_BACK),
        "the answer should be off-screen once the pane clears. Pane:\n{}",
        host.pane()
    );

    // The next wait resumes the same Review, still revealed — the developer is not asked to recall
    // something they already saw.
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_BACK);

    host.shutdown();
}

#[test]
fn an_in_flight_card_survives_its_trigger_expiring() {
    // Risk from the milestone: expiry (ADR-0006) clears the pane, but the developer may still be
    // waiting, so it must not discard the in-flight Review.
    let host = spawn_test_host_with_expiry(Duration::minutes(30));
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);
    host.press(REVEAL);
    host.wait_for(PLACEHOLDER_CARD_BACK);

    // The close never arrives; the Trigger expires and the sweep clears the pane.
    host.clock.advance(Duration::minutes(31));
    host.tick();
    host.wait_for(IDLE_MARKER);
    assert!(!host.pane().contains(PLACEHOLDER_CARD_BACK));

    // The in-flight card is intact: a new Trigger resumes it, still revealed.
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_BACK);

    host.shutdown();
}

#[test]
fn an_unfinished_card_is_neither_discarded_nor_escalated() {
    // Abandonment: ignoring LearnWhile during a wait carries no penalty and no nagging — no timeout
    // on the in-flight card, no self-review.
    let host = spawn_test_host();
    host.open("s");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // Many Waiting/idle cycles without ever rating.
    for _ in 0..5 {
        host.close("s");
        host.wait_for(IDLE_MARKER);
        host.open("s");
        host.wait_for(PLACEHOLDER_CARD_FRONT);
    }

    // The card is still there, unrated: nothing reviewed itself while it was ignored.
    let db = Connection::open(host.db_path()).expect("open db");
    let reviews: i64 = db
        .query_row("SELECT COUNT(*) FROM review_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        reviews, 0,
        "an abandoned card must not be reviewed on its own"
    );

    host.shutdown();
}
