//! Storage: the only module that issues SQL (spec §Modules).
//!
//! One `rusqlite` connection, the bundled SQLite (no dependency on the user having
//! `libsqlite3`), and one migration run against `PRAGMA user_version` on host startup. The host
//! is the sole owner of this file (ADR-0003); nothing else opens it for writing.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;

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
        if version < 1 {
            let tx = self.conn.transaction()?;
            tx.execute_batch(CREATE_CONFIG)?;
            tx.execute_batch(MIGRATION_V1)?;
            tx.commit()?;
        }
        // `user_version` is a header write, not a bound parameter, so it cannot be prepared.
        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
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
