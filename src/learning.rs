//! The Learning engine: card selection and the Review state machine (spec §Modules, §Review flow).
//!
//! Constructed with an injected [`Clock`] and [`Storage`] handle — a hard interface constraint from
//! the spec, and what lets the whole flow be tested without a real clock. It holds no knowledge of
//! sockets or terminals: the host drives it with `surface`/`reveal`/`rate` and reads [`Learning::view`]
//! to draw.
//!
//! A Review completes only once persisted, and persistence happens on the rating keypress, not on a
//! later flush — a crash must not lose a rating (spec §Review flow).

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Duration, Local, LocalResult, NaiveDate, TimeZone, Utc};

use crate::clock::Clock;
use crate::scheduler::{Memory, Rating, Scheduler};
use crate::storage::{Card, ReviewRecord, Storage};

/// Where a Review is in its lifecycle. `Idle` means no card is in flight — either nothing has been
/// surfaced yet or the last one was rated. Question and Answer each own the in-flight card.
enum ReviewState {
    Idle,
    Question(Card),
    Answer(Card),
}

/// What the pane should show for the current Review. Borrows from the in-flight card. `Question`
/// deliberately carries no `back`, so the answer cannot leak into the buffer before it is revealed.
pub enum ReviewView<'a> {
    Question {
        front: &'a str,
    },
    Answer {
        front: &'a str,
        back: &'a str,
    },
    /// No card in flight: nothing surfaced, or the deck's new cards are exhausted.
    Empty,
}

/// What the idle pane reports (milestone sub-task 5): how many cards are due now, how many new
/// cards can still be introduced today, and when the next card comes due. Enough to tell "nothing
/// due" apart from "not Waiting", so a developer seeing the idle pane does not file a bug.
pub struct IdleStats {
    pub due_today: i64,
    pub new_remaining: i64,
    pub next_due: Option<DateTime<Utc>>,
}

pub struct Learning {
    storage: Storage,
    clock: Arc<dyn Clock>,
    scheduler: Scheduler,
    /// The daily cap on new-card introductions (ADR-0002), read from config. Left configurable and
    /// not tuned in M3: `20` is a guess to revisit once `review_history` holds real data.
    new_cards_per_day: i64,
    /// Identifies this host run in `review_history.session_id`. v1 has no real Session lifecycle
    /// (that is M4); a per-run id keeps the column populated until then.
    session_id: String,
    review: ReviewState,
}

impl Learning {
    /// Build the engine, reading `desired_retention` from config to construct the scheduler.
    pub fn new(storage: Storage, clock: Arc<dyn Clock>) -> Result<Self> {
        let desired_retention = storage.config_f64("desired_retention")? as f32;
        let new_cards_per_day = storage.config_i64("new_cards_per_day")?;
        let scheduler = Scheduler::new(desired_retention)?;
        let session_id = format!("host-{}", clock.now().timestamp());
        Ok(Self {
            storage,
            clock,
            scheduler,
            new_cards_per_day,
            session_id,
            review: ReviewState::Idle,
        })
    }

    /// What the pane should render for the current Review.
    pub fn view(&self) -> ReviewView<'_> {
        match &self.review {
            ReviewState::Idle => ReviewView::Empty,
            ReviewState::Question(card) => ReviewView::Question { front: &card.front },
            ReviewState::Answer(card) => ReviewView::Answer {
                front: &card.front,
                back: &card.back,
            },
        }
    }

    /// Surface a card if none is in flight. Called when the developer becomes Waiting and again
    /// after a rating, so a long wait holds several Reviews. A Review already in flight is left
    /// untouched, so clearing and re-showing the pane never discards a half-finished Review.
    pub fn surface(&mut self) -> Result<()> {
        if !matches!(self.review, ReviewState::Idle) {
            return Ok(());
        }
        if let Some(card) = self.select_next()? {
            self.review = ReviewState::Question(card);
        }
        Ok(())
    }

    /// Reveal the answer: Question → Answer. A no-op in any other state.
    pub fn reveal(&mut self) {
        match std::mem::replace(&mut self.review, ReviewState::Idle) {
            ReviewState::Question(card) => self.review = ReviewState::Answer(card),
            other => self.review = other,
        }
    }

    /// Rate a revealed card, persisting the result and returning to Idle. A no-op unless a card is
    /// revealed (Answer): a rating key pressed on the question side does nothing but wait for reveal.
    /// The card is persisted before this returns, so a crash immediately after cannot lose it.
    pub fn rate(&mut self, rating: Rating) -> Result<()> {
        let card = match std::mem::replace(&mut self.review, ReviewState::Idle) {
            ReviewState::Answer(card) => card,
            other => {
                self.review = other;
                return Ok(());
            }
        };

        let now = self.clock.now();
        let (memory, elapsed_days) = match card.last_reviewed_at {
            // First Review: pass None, not a zeroed memory, and record zero elapsed days.
            None => (None, 0),
            Some(last) => {
                let memory = match (card.stability, card.difficulty) {
                    (Some(stability), Some(difficulty)) => Some(Memory {
                        stability,
                        difficulty,
                    }),
                    _ => None,
                };
                let days = (now.date_naive() - last.date_naive()).num_days().max(0) as u32;
                (memory, days)
            }
        };

        let schedule = self.scheduler.next(rating, memory, elapsed_days)?;
        let new_due = now + Duration::days(schedule.interval_days.round() as i64);

        self.storage.record_review(&ReviewRecord {
            card_id: card.id,
            session_id: self.session_id.clone(),
            reviewed_at: now,
            rating: rating.as_i64(),
            stability_before: card.stability,
            difficulty_before: card.difficulty,
            stability_after: schedule.stability,
            difficulty_after: schedule.difficulty,
            elapsed_days: i64::from(elapsed_days),
            scheduled_days: schedule.interval_days,
            new_due,
            new_reps: card.reps + 1,
            new_lapses: card.lapses + i64::from(rating == Rating::Again),
        })?;

        Ok(())
    }

    /// The honest selection order (ADR-0002), evaluated fresh on each surfacing: a genuinely due
    /// card, else a new card while today's introductions are under the daily cap, else nothing (the
    /// idle state). A not-yet-due card is never pulled forward. M4 adds the one bounded exception
    /// (the lapse queue, ADR-0010); until then this order holds without exception.
    fn select_next(&self) -> Result<Option<Card>> {
        let now = self.clock.now();
        if let Some(card) = self.storage.due_card(now)? {
            return Ok(Some(card));
        }
        if self.introductions_today(now)? < self.new_cards_per_day {
            return self.storage.unreviewed_card();
        }
        Ok(None)
    }

    /// How many new cards have been introduced during the local-timezone day containing `now`.
    /// Shared by selection and the idle pane's new-remaining count.
    fn introductions_today(&self, now: DateTime<Utc>) -> Result<i64> {
        let (start, end) = local_day_bounds(now, &Local);
        self.storage.introductions_between(start, end)
    }

    /// The counts the idle pane shows. `new_remaining` is what a developer will still be offered
    /// today: the cap minus today's introductions, but never more than the new cards left in the
    /// deck, and never negative.
    pub fn idle_stats(&self) -> Result<IdleStats> {
        let now = self.clock.now();
        let introduced = self.introductions_today(now)?;
        let new_available = self.storage.new_card_count()?;
        let new_remaining = (self.new_cards_per_day - introduced)
            .max(0)
            .min(new_available);
        Ok(IdleStats {
            due_today: self.storage.due_count(now)?,
            new_remaining,
            next_due: self.storage.next_due_after(now)?,
        })
    }
}

/// The half-open UTC window `[start, end)` covering the local-timezone day that contains `now`.
/// The daily cap resets on the developer's own day boundary (milestone sub-task 3), not UTC, so it
/// does not reset mid-afternoon for anyone. Generic over the timezone so it can be unit-tested with
/// fixed offsets, free of the machine's actual zone; the host passes `Local`.
fn local_day_bounds<Tz: TimeZone>(now: DateTime<Utc>, tz: &Tz) -> (DateTime<Utc>, DateTime<Utc>) {
    let today = now.with_timezone(tz).date_naive();
    let tomorrow = today.succ_opt().unwrap_or(today);
    (
        local_midnight_utc(today, tz),
        local_midnight_utc(tomorrow, tz),
    )
}

/// The UTC instant of local midnight on `date`. Handles the rare daylight-saving cases so it never
/// panics: at a fall-back the day starts at the earlier midnight; at a spring-forward gap it starts
/// when the clock jumps forward.
fn local_midnight_utc<Tz: TimeZone>(date: NaiveDate, tz: &Tz) -> DateTime<Utc> {
    let midnight = date.and_hms_opt(0, 0, 0).expect("00:00:00 is a valid time");
    match tz.from_local_datetime(&midnight) {
        LocalResult::Single(dt) => dt.with_timezone(&Utc),
        LocalResult::Ambiguous(earliest, _latest) => earliest.with_timezone(&Utc),
        LocalResult::None => {
            // Local midnight fell in a spring-forward gap; step forward to the first valid instant.
            let mut candidate = midnight;
            for _ in 0..(24 * 60) {
                candidate += Duration::minutes(1);
                if let LocalResult::Single(dt) = tz.from_local_datetime(&candidate) {
                    return dt.with_timezone(&Utc);
                }
            }
            Utc.from_utc_datetime(&midnight)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::TestClock;
    use crate::storage::NewCard;
    use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};

    fn epoch() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap()
    }

    /// A Learning engine over a temp database seeded with `cards`. Returns the shared clock (to
    /// advance), the db path (to reopen and inspect), and the TempDir (to keep the file alive).
    fn learning_with(
        cards: &[(&str, &str)],
    ) -> (
        Learning,
        Arc<TestClock>,
        std::path::PathBuf,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("learnwhile.db");
        let mut storage = Storage::open(&path).expect("open");
        let seeded: Vec<NewCard> = cards
            .iter()
            .map(|(front, back)| NewCard {
                front: (*front).to_string(),
                back: (*back).to_string(),
            })
            .collect();
        storage.seed_cards(&seeded, epoch()).expect("seed");
        let clock = TestClock::new(epoch());
        let learning = Learning::new(storage, clock.clone()).expect("learning");
        (learning, clock, path, dir)
    }

    /// A Learning engine over one card, for the tests that only need the db path.
    fn one_card() -> (Learning, std::path::PathBuf, tempfile::TempDir) {
        let (learning, _clock, path, dir) = learning_with(&[("Q", "A")]);
        (learning, path, dir)
    }

    #[test]
    fn surface_shows_the_question_side_only() {
        let (mut learning, _path, _dir) = one_card();
        learning.surface().expect("surface");
        match learning.view() {
            ReviewView::Question { front } => assert_eq!(front, "Q"),
            _ => panic!("expected the question side"),
        }
    }

    #[test]
    fn a_rating_key_before_reveal_does_nothing() {
        let (mut learning, _path, _dir) = one_card();
        learning.surface().expect("surface");
        learning.rate(Rating::Good).expect("rate");
        // Still on the question side: rating a card that has not been revealed is a no-op.
        assert!(matches!(learning.view(), ReviewView::Question { .. }));
    }

    #[test]
    fn reveal_then_rate_persists_and_advances_the_card() {
        let (mut learning, path, _dir) = one_card();
        learning.surface().expect("surface");
        learning.reveal();
        assert!(matches!(
            learning.view(),
            ReviewView::Answer { back: "A", .. }
        ));

        learning.rate(Rating::Good).expect("rate");
        // Back to Idle, and there is nothing left to surface.
        assert!(matches!(learning.view(), ReviewView::Empty));
        learning.surface().expect("resurface");
        assert!(matches!(learning.view(), ReviewView::Empty));

        // Reopen the database: the card left the new pool, exactly one history row was written, and
        // the card now has a due date.
        let storage = Storage::open(&path).expect("reopen");
        assert!(storage.unreviewed_card().unwrap().is_none());

        let conn = rusqlite::Connection::open(&path).unwrap();
        let history: i64 = conn
            .query_row("SELECT COUNT(*) FROM review_history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(history, 1);
        let due: Option<String> = conn
            .query_row("SELECT due FROM cards WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert!(
            due.is_some(),
            "the card should have a due date after review"
        );
    }

    #[test]
    fn a_new_card_is_selected_when_nothing_is_due() {
        let (mut learning, _clock, _path, _dir) = learning_with(&[("q1", "a1")]);
        learning.surface().expect("surface");
        // Nothing has been reviewed, so nothing is due; a new card is introduced.
        match learning.view() {
            ReviewView::Question { front } => assert_eq!(front, "q1"),
            _ => panic!("expected a new card"),
        }
    }

    #[test]
    fn a_due_card_is_selected_before_a_new_one() {
        let (mut learning, clock, _path, _dir) = learning_with(&[("q1", "a1"), ("q2", "a2")]);
        // Review q1 so it gains a future due date.
        learning.surface().expect("surface");
        learning.reveal();
        learning.rate(Rating::Good).expect("rate");

        // Advance well past q1's interval so it is due again; q2 is still new.
        clock.advance(Duration::days(60));
        learning.surface().expect("surface");
        match learning.view() {
            ReviewView::Question { front } => {
                assert_eq!(front, "q1", "the due card must win over the new one")
            }
            _ => panic!("expected the due card"),
        }
    }

    #[test]
    fn a_not_yet_due_card_is_never_surfaced() {
        let (mut learning, _clock, _path, _dir) = learning_with(&[("only", "card")]);
        // Review the only card: its due date moves into the future.
        learning.surface().expect("surface");
        learning.reveal();
        learning.rate(Rating::Good).expect("rate");

        // Nothing is due and no new card remains, so nothing may be surfaced — not even this one
        // card, ahead of its due date. This is the ADR-0002 guarantee.
        learning.surface().expect("surface");
        assert!(matches!(learning.view(), ReviewView::Empty));
    }

    #[test]
    fn the_daily_cap_stops_new_card_introductions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("learnwhile.db");
        let mut storage = Storage::open(&path).expect("open");
        storage
            .seed_cards(
                &[
                    NewCard {
                        front: "q1".into(),
                        back: "a1".into(),
                    },
                    NewCard {
                        front: "q2".into(),
                        back: "a2".into(),
                    },
                ],
                epoch(),
            )
            .expect("seed");
        // Cap the day at a single new card, before Learning reads config.
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE config SET value = '1' WHERE key = 'new_cards_per_day'",
                [],
            )
            .unwrap();

        let mut learning = Learning::new(storage, TestClock::new(epoch())).expect("learning");

        // The first new card is under the cap.
        learning.surface().expect("surface");
        assert!(matches!(
            learning.view(),
            ReviewView::Question { front: "q1" }
        ));
        learning.reveal();
        learning.rate(Rating::Good).expect("rate");

        // The cap is now spent: the second new card is not introduced, even while Waiting.
        learning.surface().expect("surface");
        assert!(matches!(learning.view(), ReviewView::Empty));
    }

    #[test]
    fn idle_stats_report_due_new_remaining_and_next_due() {
        let (mut learning, clock, _path, _dir) =
            learning_with(&[("q1", "a1"), ("q2", "a2"), ("q3", "a3")]);
        // Introduce and review q1: it gains a future due date.
        learning.surface().expect("surface");
        learning.reveal();
        learning.rate(Rating::Good).expect("rate");

        let stats = learning.idle_stats().expect("stats");
        // Nothing is due yet; one new card was introduced today, two remain of the deck's three.
        assert_eq!(stats.due_today, 0);
        assert_eq!(stats.new_remaining, 2);
        assert!(
            stats.next_due.is_some(),
            "q1's future due should be reported"
        );

        // Advance past q1's interval: it is now due.
        clock.advance(Duration::days(60));
        assert_eq!(learning.idle_stats().expect("stats").due_today, 1);
    }

    #[test]
    fn local_day_bounds_track_a_positive_offset() {
        // +08:00: local midnight is 16:00 the previous UTC day.
        let tz = FixedOffset::east_opt(8 * 3600).unwrap();
        // 2026-06-01 00:30 local = 2026-05-31 16:30 UTC.
        let now = Utc.with_ymd_and_hms(2026, 5, 31, 16, 30, 0).unwrap();
        let (start, end) = local_day_bounds(now, &tz);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 5, 31, 16, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 6, 1, 16, 0, 0).unwrap());
        assert!(start <= now && now < end);
    }

    #[test]
    fn local_day_bounds_handle_a_negative_offset_near_midnight() {
        // -05:00, just before local midnight: the UTC date is already tomorrow, but the local day
        // is still today — the classic off-by-one a UTC boundary gets wrong.
        let tz = FixedOffset::west_opt(5 * 3600).unwrap();
        // 2026-06-01 23:59 local = 2026-06-02 04:59 UTC.
        let now = Utc.with_ymd_and_hms(2026, 6, 2, 4, 59, 0).unwrap();
        let (start, end) = local_day_bounds(now, &tz);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 6, 1, 5, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 6, 2, 5, 0, 0).unwrap());
        assert!(start <= now && now < end);
    }
}
