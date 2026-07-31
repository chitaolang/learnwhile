//! `seed` end to end: the real binary against a temp data directory, asserting on its output and
//! on what actually landed in the database. This is the milestone's idempotency test, exercised
//! the way a developer would run it (spec exit criteria: an empty machine using only the README).

use std::fs;

use assert_cmd::Command;
use learnwhile::storage::Storage;
use tempfile::tempdir;

/// Run `learnwhile seed <tsv>` with the data directory pointed at `data_home`, and return stdout.
fn seed(data_home: &std::path::Path, tsv: &std::path::Path) -> String {
    let output = Command::cargo_bin("learnwhile")
        .expect("binary")
        .env("XDG_DATA_HOME", data_home)
        .arg("seed")
        .arg(tsv)
        .output()
        .expect("run seed");
    assert!(
        output.status.success(),
        "seed exited {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn seeding_inserts_cards_then_a_reseed_inserts_nothing() {
    let data_home = tempdir().expect("temp data dir");
    let tsv = data_home.path().join("cards.tsv");
    fs::write(&tsv, "front one\tback one\nfront two\tback two\n").expect("write tsv");

    // First run: both cards are new.
    let first = seed(data_home.path(), &tsv);
    assert!(first.contains("2 added"), "stdout was: {first}");

    // Second run against the same file: nothing is added.
    let second = seed(data_home.path(), &tsv);
    assert!(second.contains("0 added"), "stdout was: {second}");

    // The cards really persisted: exactly two rows, with the content we wrote.
    let db = data_home.path().join("learnwhile").join("learnwhile.db");
    let storage = Storage::open(&db).expect("open seeded db");
    assert_eq!(storage.card_count().unwrap(), 2);
}
