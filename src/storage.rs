//! Storage: the only module that issues SQL (spec §Modules).
//!
//! One `rusqlite` connection, the bundled SQLite (no dependency on the user having
//! `libsqlite3`), and one migration run against `PRAGMA user_version` on host startup. The host
//! is the sole owner of this file (ADR-0003); nothing else opens it for writing.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};

const DB_NAME: &str = "learnwhile.db";

/// The one migration. Bumped whenever the schema changes; `migrate` runs every version strictly
/// greater than the database's current `user_version`.
const SCHEMA_VERSION: i64 = 1;

/// The v1 schema and its seed data (spec §Schema). Tables per DESIGN_DRAFT §9. `decks` and the
/// `new`/`review` domain of `cards.state` both exist so post-v1 states need no migration.
///
/// Timestamps are RFC3339 text; FSRS quantities are `REAL`. `stability`, `difficulty`, `due`, and
/// `last_reviewed_at` are nullable because a brand-new card has no memory state and no due date
/// until its first Review.
const MIGRATION_V1: &str = "\
CREATE TABLE decks (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE cards (
    id               INTEGER PRIMARY KEY,
    deck_id          INTEGER NOT NULL REFERENCES decks(id),
    front            TEXT NOT NULL,
    back             TEXT NOT NULL,
    content_hash     TEXT NOT NULL UNIQUE,
    state            TEXT NOT NULL DEFAULT 'new',
    stability        REAL,
    difficulty       REAL,
    due              TEXT,
    reps             INTEGER NOT NULL DEFAULT 0,
    lapses           INTEGER NOT NULL DEFAULT 0,
    last_reviewed_at TEXT,
    created_at       TEXT NOT NULL
);

CREATE TABLE review_history (
    id                INTEGER PRIMARY KEY,
    card_id           INTEGER NOT NULL REFERENCES cards(id),
    session_id        TEXT NOT NULL,
    reviewed_at       TEXT NOT NULL,
    rating            INTEGER NOT NULL,
    stability_before  REAL,
    difficulty_before REAL,
    stability_after   REAL NOT NULL,
    difficulty_after  REAL NOT NULL,
    elapsed_days      INTEGER NOT NULL,
    scheduled_days    REAL NOT NULL
);

INSERT INTO decks (id, name) VALUES (1, 'Default');

INSERT INTO config (key, value) VALUES
    ('trigger_expiry_seconds', '1800'),
    ('desired_retention', '0.9'),
    ('new_cards_per_day', '20');
";

/// The `config` table is created outside `MIGRATION_V1` so the seed `INSERT` above can rely on it
/// existing; keeping it as its own statement also makes the key/value shape obvious.
const CREATE_CONFIG: &str = "\
CREATE TABLE config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// The default deck every v1 card belongs to. v1 has exactly one deck (spec §Schema).
pub const DEFAULT_DECK_ID: i64 = 1;

/// A card as it arrives from `seed`, before it has any FSRS state. Front and back are the parsed
/// TSV columns; the content hash used for idempotency is computed from them at insert time.
pub struct NewCard {
    pub front: String,
    pub back: String,
}

/// What a `seed` run did: how many cards were inserted versus skipped because their content hash
/// was already present.
pub struct SeedOutcome {
    pub added: usize,
    pub skipped: usize,
}

/// One completed Review to persist: the card's new FSRS state and due date, plus the full
/// `review_history` row. `*_before` are `None` on a first Review (no prior memory state), which is
/// how the audit trail distinguishes it. Built by the Learning engine, written by [`Storage`].
pub struct ReviewRecord {
    pub card_id: i64,
    pub session_id: String,
    pub reviewed_at: DateTime<Utc>,
    pub rating: i64,
    pub stability_before: Option<f32>,
    pub difficulty_before: Option<f32>,
    pub stability_after: f32,
    pub difficulty_after: f32,
    pub elapsed_days: i64,
    pub scheduled_days: f32,
    pub new_due: DateTime<Utc>,
    pub new_reps: i64,
    pub new_lapses: i64,
}

/// A card read from storage, with enough state to conduct and persist a Review. `stability` and
/// `difficulty` are `None` for a card not yet reviewed; `last_reviewed_at` and `due` are `None`
/// until its first Review. `last_reviewed_at` is what the elapsed-days calculation reads, and `due`
/// is what selection compares against the clock (ADR-0002).
pub struct Card {
    pub id: i64,
    pub front: String,
    pub back: String,
    pub stability: Option<f32>,
    pub difficulty: Option<f32>,
    pub reps: i64,
    pub lapses: i64,
    pub last_reviewed_at: Option<DateTime<Utc>>,
    pub due: Option<DateTime<Utc>>,
}

pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Open (creating if absent) the database at `path`, run migrations, and return a handle.
    ///
    /// Tests inject a temp path here, exactly as they inject a temp socket path; the host uses
    /// [`default_db_path`].
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating data directory {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening database {}", path.display()))?;
        // Enforce the foreign keys declared in the schema; SQLite leaves them off by default.
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        // A generous busy timeout so a brief write lock (a second connection reading after a
        // Review commits, in tests or later readers) waits rather than erroring immediately.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    /// Run every migration newer than the database's `user_version`, in one transaction, then
    /// stamp the new version. Idempotent: a database already at `SCHEMA_VERSION` does nothing.
    fn migrate(&mut self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version < SCHEMA_VERSION {
            let tx = self.conn.transaction()?;
            tx.execute_batch(CREATE_CONFIG)?;
            tx.execute_batch(MIGRATION_V1)?;
            tx.commit()?;
            // `user_version` is a header write, not a bound parameter, so it cannot be prepared.
            // Stamped only when a migration actually ran, so a normal open performs no write.
            self.conn
                .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(())
    }

    /// A config value as text, or `None` if the key is absent.
    pub fn config_str(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .conn
            .query_row("SELECT value FROM config WHERE key = ?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        Ok(value)
    }

    /// A config value parsed as `i64`. Errors if the key is missing or the stored text is not an
    /// integer, since a malformed config row is a real problem rather than something to paper over.
    pub fn config_i64(&self, key: &str) -> Result<i64> {
        let raw = self
            .config_str(key)?
            .with_context(|| format!("config key {key:?} is missing"))?;
        raw.trim()
            .parse::<i64>()
            .with_context(|| format!("config key {key:?} holds non-integer {raw:?}"))
    }

    /// A config value parsed as `f64` (used for `desired_retention`).
    pub fn config_f64(&self, key: &str) -> Result<f64> {
        let raw = self
            .config_str(key)?
            .with_context(|| format!("config key {key:?} is missing"))?;
        raw.trim()
            .parse::<f64>()
            .with_context(|| format!("config key {key:?} holds non-number {raw:?}"))
    }

    /// Insert each card into the default deck, skipping any whose content hash already exists, so
    /// re-running `seed` on the same file is idempotent (spec §Card seeding). All inserts share one
    /// transaction and one `created_at`. New cards carry no FSRS state: `state` defaults to `new`
    /// and `stability`/`difficulty`/`due`/`last_reviewed_at` stay null until the first Review.
    pub fn seed_cards(
        &mut self,
        cards: &[NewCard],
        created_at: DateTime<Utc>,
    ) -> Result<SeedOutcome> {
        let created = created_at.to_rfc3339();
        let tx = self.conn.transaction()?;
        let mut added = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO cards (deck_id, front, back, content_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(content_hash) DO NOTHING",
            )?;
            for card in cards {
                let hash = content_hash(&card.front, &card.back);
                // `execute` returns rows changed: 1 when inserted, 0 when the hash already existed.
                added += stmt.execute(rusqlite::params![
                    DEFAULT_DECK_ID,
                    &card.front,
                    &card.back,
                    hash,
                    &created,
                ])?;
            }
        }
        tx.commit()?;
        Ok(SeedOutcome {
            added,
            skipped: cards.len() - added,
        })
    }

    /// Total number of cards across all decks. Used by tests and, later, the idle-state counts.
    pub fn card_count(&self) -> Result<i64> {
        let count = self
            .conn
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
        Ok(count)
    }

    /// The first card never yet reviewed (`state = 'new'`), or `None` if the deck holds none. This
    /// is the data behind M2's placeholder `select_next`: rating a card flips it to `'review'`, so
    /// repeated calls walk the new cards and then return `None`. M3 replaces this with the honest
    /// due-then-new selection order (ADR-0002).
    pub fn unreviewed_card(&self) -> Result<Option<Card>> {
        let card = self
            .conn
            .query_row(
                "SELECT id, front, back, stability, difficulty, reps, lapses, last_reviewed_at, due
                 FROM cards WHERE state = 'new' ORDER BY id LIMIT 1",
                [],
                row_to_card,
            )
            .optional()?;
        Ok(card)
    }

    /// The most-overdue card due at or before `now` (ADR-0002's first selection choice), or `None`
    /// if nothing is due. "Due" is compared against the injected clock's `now`, passed in as a
    /// bound parameter, never against SQLite's own time functions — otherwise tests could not
    /// control what counts as due.
    pub fn due_card(&self, now: DateTime<Utc>) -> Result<Option<Card>> {
        let card = self
            .conn
            .query_row(
                "SELECT id, front, back, stability, difficulty, reps, lapses, last_reviewed_at, due
                 FROM cards
                 WHERE state = 'review' AND due IS NOT NULL AND due <= ?1
                 ORDER BY due ASC LIMIT 1",
                [now.to_rfc3339()],
                row_to_card,
            )
            .optional()?;
        Ok(card)
    }

    /// How many new cards were introduced in the half-open window `[start, end)`, derived from
    /// `review_history` rather than a counter so a restart cannot lose or double the count
    /// (milestone sub-task 4). A new-card introduction is a card's *first* Review, which is exactly
    /// the row whose `stability_before` is null (no prior memory state). The window is passed as
    /// bound parameters so the caller decides what "today" means (ADR-0002, local timezone).
    pub fn introductions_between(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM review_history
             WHERE stability_before IS NULL AND reviewed_at >= ?1 AND reviewed_at < ?2",
            [start.to_rfc3339(), end.to_rfc3339()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// How many cards have never been reviewed (`state = 'new'`), for the idle pane's
    /// new-remaining count.
    pub fn new_card_count(&self) -> Result<i64> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE state = 'new'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Persist one completed Review: update the card's FSRS state and due date, and append the
    /// append-only `review_history` row, in a single transaction (spec §Review flow, §Schema). Both
    /// land or neither does, so a crash cannot leave the card advanced without its audit row, nor
    /// the reverse.
    pub fn record_review(&mut self, review: &ReviewRecord) -> Result<()> {
        let reviewed_at = review.reviewed_at.to_rfc3339();
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE cards
             SET stability = ?2, difficulty = ?3, due = ?4, reps = ?5, lapses = ?6,
                 state = 'review', last_reviewed_at = ?7
             WHERE id = ?1",
            rusqlite::params![
                review.card_id,
                review.stability_after,
                review.difficulty_after,
                review.new_due.to_rfc3339(),
                review.new_reps,
                review.new_lapses,
                reviewed_at,
            ],
        )?;
        tx.execute(
            "INSERT INTO review_history
                 (card_id, session_id, reviewed_at, rating,
                  stability_before, difficulty_before, stability_after, difficulty_after,
                  elapsed_days, scheduled_days)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                review.card_id,
                review.session_id,
                reviewed_at,
                review.rating,
                review.stability_before,
                review.difficulty_before,
                review.stability_after,
                review.difficulty_after,
                review.elapsed_days,
                review.scheduled_days,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// Map a `cards` row onto a [`Card`]. A stored timestamp that fails to parse is treated as absent,
/// degrading gracefully rather than failing the read.
fn row_to_card(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    let last_reviewed_at: Option<String> = row.get("last_reviewed_at")?;
    let due: Option<String> = row.get("due")?;
    Ok(Card {
        id: row.get("id")?,
        front: row.get("front")?,
        back: row.get("back")?,
        stability: row.get("stability")?,
        difficulty: row.get("difficulty")?,
        reps: row.get("reps")?,
        lapses: row.get("lapses")?,
        last_reviewed_at: parse_timestamp(last_reviewed_at),
        due: parse_timestamp(due),
    })
}

/// Parse a stored RFC3339 timestamp, treating an unparseable or absent value as `None`.
fn parse_timestamp(raw: Option<String>) -> Option<DateTime<Utc>> {
    raw.and_then(|raw| {
        DateTime::parse_from_rfc3339(&raw)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    })
}

/// A stable 64-bit FNV-1a hash of a card's content, as hex. Chosen over a crypto hash because the
/// only requirement is a stable, collision-resistant key for one local deck — not a dependency.
/// The `0x1f` unit separator keeps `("ab", "c")` from colliding with `("a", "bc")`.
fn content_hash(front: &str, back: &str) -> String {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in front
        .bytes()
        .chain(std::iter::once(0x1f))
        .chain(back.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

/// `$XDG_DATA_HOME/learnwhile/learnwhile.db`, falling back to `$HOME/.local/share/...` when the
/// variable is unset — the XDG Base Directory default. Hand-rolled to match [`crate::socket`]
/// rather than pulling in a directories crate for one path.
pub fn default_db_path() -> PathBuf {
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => match std::env::var_os("HOME") {
            Some(home) if !home.is_empty() => PathBuf::from(home).join(".local").join("share"),
            _ => std::env::temp_dir(),
        },
    };
    base.join("learnwhile").join(DB_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    #[test]
    fn migration_creates_schema_and_seeds_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(DB_NAME);

        let storage = Storage::open(&path).expect("open");

        assert_eq!(storage.config_i64("trigger_expiry_seconds").unwrap(), 1800);
        assert_eq!(storage.config_f64("desired_retention").unwrap(), 0.9);
        assert_eq!(storage.config_i64("new_cards_per_day").unwrap(), 20);

        let deck_name: String = storage
            .conn
            .query_row(
                "SELECT name FROM decks WHERE id = ?1",
                [DEFAULT_DECK_ID],
                |r| r.get(0),
            )
            .expect("default deck");
        assert_eq!(deck_name, "Default");
    }

    #[test]
    fn seeding_inserts_new_cards_and_skips_duplicates_on_reseed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut storage = Storage::open(&dir.path().join(DB_NAME)).expect("open");

        let cards = vec![
            NewCard {
                front: "a".into(),
                back: "1".into(),
            },
            NewCard {
                front: "b".into(),
                back: "2".into(),
            },
        ];

        let first = storage.seed_cards(&cards, Utc::now()).expect("first seed");
        assert_eq!((first.added, first.skipped), (2, 0));

        // Re-seeding the identical cards inserts nothing: the milestone's idempotency test.
        let second = storage.seed_cards(&cards, Utc::now()).expect("second seed");
        assert_eq!((second.added, second.skipped), (0, 2));

        assert_eq!(storage.card_count().unwrap(), 2);
    }

    #[test]
    fn unreviewed_card_returns_a_new_card_and_none_on_an_empty_deck() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut storage = Storage::open(&dir.path().join(DB_NAME)).expect("open");

        // Empty deck: nothing to select.
        assert!(storage.unreviewed_card().unwrap().is_none());

        storage
            .seed_cards(
                &[NewCard {
                    front: "front".into(),
                    back: "back".into(),
                }],
                Utc::now(),
            )
            .expect("seed");

        let card = storage.unreviewed_card().unwrap().expect("a new card");
        assert_eq!(card.front, "front");
        assert_eq!(card.back, "back");
        // A freshly seeded card carries no FSRS state yet.
        assert!(card.stability.is_none());
        assert!(card.last_reviewed_at.is_none());
        assert!(card.due.is_none());
    }

    /// A minimal `ReviewRecord` that sets a card's `due` for selection tests. Only `card_id` and
    /// `new_due` matter here; the rest are plausible filler.
    fn reviewed_with_due(card_id: i64, due: DateTime<Utc>) -> ReviewRecord {
        ReviewRecord {
            card_id,
            session_id: "test".into(),
            reviewed_at: due - Duration::days(3),
            rating: 3,
            stability_before: None,
            difficulty_before: None,
            stability_after: 5.0,
            difficulty_after: 5.0,
            elapsed_days: 0,
            scheduled_days: 3.0,
            new_due: due,
            new_reps: 1,
            new_lapses: 0,
        }
    }

    /// A `ReviewRecord` at a chosen instant. `first` controls whether it looks like a first Review
    /// (null `*_before`, which is what an introduction is) or a repeat.
    fn review_at(card_id: i64, reviewed_at: DateTime<Utc>, first: bool) -> ReviewRecord {
        ReviewRecord {
            card_id,
            session_id: "test".into(),
            reviewed_at,
            rating: 3,
            stability_before: if first { None } else { Some(4.0) },
            difficulty_before: if first { None } else { Some(5.0) },
            stability_after: 5.0,
            difficulty_after: 5.0,
            elapsed_days: 0,
            scheduled_days: 3.0,
            new_due: reviewed_at + Duration::days(3),
            new_reps: 1,
            new_lapses: 0,
        }
    }

    #[test]
    fn introductions_between_counts_only_first_reviews_inside_the_window() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut storage = Storage::open(&dir.path().join(DB_NAME)).expect("open");
        storage
            .seed_cards(
                &[
                    NewCard {
                        front: "a".into(),
                        back: "1".into(),
                    },
                    NewCard {
                        front: "b".into(),
                        back: "2".into(),
                    },
                    NewCard {
                        front: "c".into(),
                        back: "3".into(),
                    },
                ],
                Utc::now(),
            )
            .expect("seed");

        let t = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let start = t - Duration::hours(1);
        let end = t + Duration::hours(1);

        // Counts: a first Review inside the window.
        storage.record_review(&review_at(1, t, true)).expect("r1");
        // Excluded: a first Review outside the window.
        storage
            .record_review(&review_at(2, t + Duration::hours(2), true))
            .expect("r2");
        // Excluded: a repeat Review inside the window is not an introduction.
        storage.record_review(&review_at(3, t, false)).expect("r3");

        assert_eq!(storage.introductions_between(start, end).unwrap(), 1);
    }

    #[test]
    fn new_card_count_drops_as_cards_are_reviewed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut storage = Storage::open(&dir.path().join(DB_NAME)).expect("open");
        storage
            .seed_cards(
                &[
                    NewCard {
                        front: "a".into(),
                        back: "1".into(),
                    },
                    NewCard {
                        front: "b".into(),
                        back: "2".into(),
                    },
                ],
                Utc::now(),
            )
            .expect("seed");

        assert_eq!(storage.new_card_count().unwrap(), 2);

        let t = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        storage
            .record_review(&review_at(1, t, true))
            .expect("review");

        // Card 1 is now 'review', so only one new card remains.
        assert_eq!(storage.new_card_count().unwrap(), 1);
    }

    #[test]
    fn due_card_returns_the_past_due_card_and_ignores_future_and_new_cards() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut storage = Storage::open(&dir.path().join(DB_NAME)).expect("open");
        storage
            .seed_cards(
                &[
                    NewCard {
                        front: "past".into(),
                        back: "p".into(),
                    },
                    NewCard {
                        front: "future".into(),
                        back: "f".into(),
                    },
                    NewCard {
                        front: "new".into(),
                        back: "n".into(),
                    },
                ],
                Utc::now(),
            )
            .expect("seed");

        let now = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        // Card 1 became due yesterday; card 2 is not due until tomorrow; card 3 stays new.
        storage
            .record_review(&reviewed_with_due(1, now - Duration::days(1)))
            .expect("review 1");
        storage
            .record_review(&reviewed_with_due(2, now + Duration::days(1)))
            .expect("review 2");

        // Only the past-due card is returned, never the future one or the new one.
        let due = storage.due_card(now).unwrap().expect("a due card");
        assert_eq!(due.id, 1);
        assert_eq!(due.front, "past");
        assert!(due.due.is_some());

        // Before that card came due, nothing is due at all.
        assert!(storage.due_card(now - Duration::days(2)).unwrap().is_none());
    }

    #[test]
    fn same_front_with_a_different_back_is_a_distinct_card() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut storage = Storage::open(&dir.path().join(DB_NAME)).expect("open");

        let cards = vec![
            NewCard {
                front: "q".into(),
                back: "one".into(),
            },
            NewCard {
                front: "q".into(),
                back: "two".into(),
            },
        ];

        let outcome = storage.seed_cards(&cards, Utc::now()).expect("seed");
        assert_eq!(outcome.added, 2);
    }

    #[test]
    fn migration_is_idempotent_on_an_existing_database() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(DB_NAME);

        // First open migrates from empty; second open re-runs against a populated database.
        Storage::open(&path).expect("first open");
        let storage = Storage::open(&path).expect("second open");

        // The seed did not run twice: exactly one deck and one row per config key.
        let decks: i64 = storage
            .conn
            .query_row("SELECT COUNT(*) FROM decks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(decks, 1);
        let configs: i64 = storage
            .conn
            .query_row("SELECT COUNT(*) FROM config", [], |r| r.get(0))
            .unwrap();
        assert_eq!(configs, 3);
    }
}
