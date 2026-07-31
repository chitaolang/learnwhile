//! The `config` and `cards` inspection subcommands, exercised on the real binary against a temp
//! data directory — the same way a developer runs them.

use std::fs;

use assert_cmd::Command;
use tempfile::tempdir;

/// Run a subcommand against `data_home`, assert it succeeded, and return stdout.
fn run(data_home: &std::path::Path, args: &[&str]) -> String {
    let output = Command::cargo_bin("learnwhile")
        .expect("binary")
        .env("XDG_DATA_HOME", data_home)
        .args(args)
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Run a subcommand expected to fail, returning stderr.
fn run_failing(data_home: &std::path::Path, args: &[&str]) -> String {
    let output = Command::cargo_bin("learnwhile")
        .expect("binary")
        .env("XDG_DATA_HOME", data_home)
        .args(args)
        .output()
        .expect("run");
    assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn config_lists_and_sets_and_cards_lists() {
    let home = tempdir().expect("temp home");
    let tsv = home.path().join("deck.tsv");
    fs::write(&tsv, "front one\tback one\n").expect("write tsv");
    run(home.path(), &["seed", tsv.to_str().unwrap()]);

    // config with no action lists the seeded defaults.
    let listed = run(home.path(), &["config"]);
    assert!(
        listed.contains("new_cards_per_day = 20"),
        "listed:\n{listed}"
    );

    // config set changes a value, visible on the next list.
    let set = run(home.path(), &["config", "set", "new_cards_per_day", "30"]);
    assert!(set.contains("new_cards_per_day = 30"), "set output:\n{set}");
    let relisted = run(home.path(), &["config"]);
    assert!(
        relisted.contains("new_cards_per_day = 30"),
        "relisted:\n{relisted}"
    );

    // cards lists the seeded card with its state.
    let cards = run(home.path(), &["cards"]);
    assert!(cards.contains("front one"), "cards:\n{cards}");
    assert!(cards.contains("new"), "cards:\n{cards}");
}

#[test]
fn config_set_rejects_a_bad_value_and_an_unknown_key() {
    let home = tempdir().expect("temp home");

    // A non-numeric expiry would break host startup, so it is rejected here.
    let bad = run_failing(
        home.path(),
        &["config", "set", "trigger_expiry_seconds", "abc"],
    );
    assert!(bad.contains("whole number"), "stderr:\n{bad}");

    // An unknown key is rejected rather than written as a junk row.
    let unknown = run_failing(home.path(), &["config", "set", "made_up_key", "1"]);
    assert!(unknown.contains("unknown config key"), "stderr:\n{unknown}");
}
