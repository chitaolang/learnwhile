//! The Review flow, tested through the host boundary: drive reveal and rating keys, and assert on
//! the pane and on what persisted to SQLite. Following the spec's testing decisions, no test reaches
//! into the Review state machine — only the pane and the database, both external to the host.

use crossterm::event::KeyCode;
use learnwhile::testing::{
    PLACEHOLDER_CARD_BACK, PLACEHOLDER_CARD_FRONT, spawn_test_host, spawn_test_host_with_cards,
};
use rusqlite::Connection;

const REVEAL: KeyCode = KeyCode::Char(' ');
const AGAIN: KeyCode = KeyCode::Char('1');
const HARD: KeyCode = KeyCode::Char('2');
const GOOD: KeyCode = KeyCode::Char('3');
const EASY: KeyCode = KeyCode::Char('4');

/// Open a read connection to the host's database for direct assertions.
fn open_db(host_db: &std::path::Path) -> Connection {
    Connection::open(host_db).expect("open db for assertions")
}

#[test]
fn the_question_side_shows_without_the_answer_until_reveal() {
    let host = spawn_test_host();
    host.open("session-a");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    // This is the test that protects the product's premise: assert on the buffer, not on the state
    // machine. Before reveal, the answer must not be anywhere on screen.
    let pane = host.pane();
    assert!(
        !pane.contains(PLACEHOLDER_CARD_BACK),
        "the answer was visible before reveal. Pane:\n{pane}"
    );

    host.press(REVEAL);
    host.wait_for(PLACEHOLDER_CARD_BACK);

    host.shutdown();
}

#[test]
fn a_full_reveal_and_rate_writes_one_history_row_and_advances_the_due_date() {
    let host = spawn_test_host();
    host.open("session-a");
    host.wait_for(PLACEHOLDER_CARD_FRONT);

    host.press(REVEAL);
    host.wait_for(PLACEHOLDER_CARD_BACK);
    host.press(GOOD);

    let db = open_db(host.db_path());
    let rows: i64 = db
        .query_row("SELECT COUNT(*) FROM review_history", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1, "exactly one Review should have been recorded");

    // The card started with a null due date (never reviewed); rating it must set one.
    let due: Option<String> = db
        .query_row("SELECT due FROM cards WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert!(due.is_some(), "the card's due date should have advanced");

    // The card also left the new pool, so its state moved to 'review'.
    let state: String = db
        .query_row("SELECT state FROM cards WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "review");

    host.shutdown();
}

#[test]
fn every_rating_completes_a_review_including_again() {
    // A Review counts as complete regardless of correctness, so each rating — Again included —
    // writes a history row with its own rating value (1..4).
    for (key, expected) in [(AGAIN, 1), (HARD, 2), (GOOD, 3), (EASY, 4)] {
        let host = spawn_test_host();
        host.open("session-a");
        host.wait_for(PLACEHOLDER_CARD_FRONT);
        host.press(REVEAL);
        host.wait_for(PLACEHOLDER_CARD_BACK);
        host.press(key);

        let db = open_db(host.db_path());
        let rating: i64 = db
            .query_row("SELECT rating FROM review_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            rating, expected,
            "rating key {key:?} persisted the wrong value"
        );

        host.shutdown();
    }
}

#[test]
fn a_rated_review_survives_a_host_restart() {
    let host = spawn_test_host();
    host.open("session-a");
    host.wait_for(PLACEHOLDER_CARD_FRONT);
    host.press(REVEAL);
    host.wait_for(PLACEHOLDER_CARD_BACK);
    host.press(GOOD);

    // Reboot against the same database, as if the process had been restarted.
    let host = host.restart();

    let db = open_db(host.db_path());
    let rows: i64 = db
        .query_row("SELECT COUNT(*) FROM review_history", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1, "the Review should still be there after a restart");

    host.shutdown();
}

#[test]
fn a_long_wait_moves_on_to_the_next_card_after_a_rating() {
    // Two cards seeded: rating the first during a wait should surface the second in the same wait
    // (spec user story 12), without the Trigger closing and reopening.
    let host = spawn_test_host_with_cards(&[
        ("first front", "first back"),
        ("second front", "second back"),
    ]);
    host.open("session-a");
    host.wait_for("first front");

    host.press(REVEAL);
    host.wait_for("first back");
    host.press(GOOD);

    // The pane advances to the next card on its own.
    host.wait_for("second front");

    host.shutdown();
}
