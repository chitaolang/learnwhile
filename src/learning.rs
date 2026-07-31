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
use chrono::Duration;

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

pub struct Learning {
    storage: Storage,
    clock: Arc<dyn Clock>,
    scheduler: Scheduler,
    /// Identifies this host run in `review_history.session_id`. v1 has no real Session lifecycle
    /// (that is M4); a per-run id keeps the column populated until then.
    session_id: String,
    review: ReviewState,
}

impl Learning {
    /// Build the engine, reading `desired_retention` from config to construct the scheduler.
    pub fn new(storage: Storage, clock: Arc<dyn Clock>) -> Result<Self> {
        let desired_retention = storage.config_f64("desired_retention")? as f32;
        let scheduler = Scheduler::new(desired_retention)?;
        let session_id = format!("host-{}", clock.now().timestamp());
        Ok(Self {
            storage,
            clock,
            scheduler,
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

    /// Placeholder selection (milestone step 9): any unreviewed card. Deliberate scaffolding — M3
    /// replaces it with the honest due → new → idle order (ADR-0002). Kept to a handful of lines
    /// on purpose; no due-date logic accumulates here.
    fn select_next(&self) -> Result<Option<Card>> {
        self.storage.unreviewed_card()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::TestClock;
    use crate::storage::NewCard;
    use chrono::{DateTime, TimeZone, Utc};

    fn epoch() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap()
    }

    /// A Learning engine over a temp database seeded with one card. Returns the db path so a test
    /// can reopen it and confirm what persisted, and the TempDir so the file outlives the call.
    fn one_card() -> (Learning, std::path::PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("learnwhile.db");
        let mut storage = Storage::open(&path).expect("open");
        storage
            .seed_cards(
                &[NewCard {
                    front: "Q".into(),
                    back: "A".into(),
                }],
                epoch(),
            )
            .expect("seed");
        let learning = Learning::new(storage, TestClock::new(epoch())).expect("learning");
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
}
