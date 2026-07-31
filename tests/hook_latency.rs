//! The hook latency budget (ADR-0008), turned from an intention into an assertion. The hook fires
//! on every prompt submission, so its path must stay cold: no SQLite, no config, no logging.
//!
//! Two checks, because pure wall-clock timing flakes on shared CI: a generous budget on the fastest
//! of several runs catches a gross slowdown, and a database-was-not-created assertion catches the
//! specific regression the budget exists to prevent — someone opening storage on the hook path.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::cargo_bin;

/// Generous on purpose: the hook's own work is sub-millisecond, so even a slow CI spawn sits well
/// under this, while opening a database or loading config would show up against it.
const BUDGET: Duration = Duration::from_millis(300);

#[test]
fn the_hook_stays_cold_within_its_latency_budget() {
    let bin = cargo_bin("learnwhile");
    // A private data/runtime home: no host listens on the socket here, so the hook's connect fails
    // fast, and any database it wrongly opened would land where we can see it.
    let home = tempfile::tempdir().expect("temp home");

    let mut fastest = Duration::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        let status = Command::new(&bin)
            .args(["hook", "--open"])
            .env("XDG_DATA_HOME", home.path())
            .env("XDG_RUNTIME_DIR", home.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run hook");
        assert!(status.success(), "the hook must exit 0 (ADR-0004)");
        fastest = fastest.min(start.elapsed());
    }

    assert!(
        fastest < BUDGET,
        "hook took {fastest:?}, over the {BUDGET:?} budget — did the hook path grow (SQLite? config?)"
    );

    // The clincher, free of timing noise: the hook must not have opened the database. Opening
    // storage would have created this file (ADR-0008 keeps the hook path off SQLite entirely).
    let db = home.path().join("learnwhile").join("learnwhile.db");
    assert!(
        !db.exists(),
        "the hook created a database at {} — storage belongs only to the host",
        db.display()
    );
}
