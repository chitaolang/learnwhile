//! The FSRS scheduler wrapper (spec §FSRS).
//!
//! Constructed once from `DEFAULT_PARAMETERS` — v1 runs on defaults, no optimization pass — with
//! `desired_retention` read from `config`. This module owns the only dependency on the `fsrs`
//! crate; the rest of the engine speaks in plain `f32`s and this module's own types, so the
//! rating-to-state mapping lives in exactly one place.
//!
//! We do not re-test FSRS's interval maths here (spec §Testing): that belongs to the upstream
//! crate. We test that we call it and return what it hands back.

use anyhow::{Context, Result, anyhow};
use fsrs::{DEFAULT_PARAMETERS, FSRS, MemoryState};

/// The four ratings a developer can give a revealed card (CONTEXT.md). Correctness is not what a
/// Review measures; every one of these completes a Review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

impl Rating {
    /// The integer persisted in `review_history.rating`, matching FSRS's 1..=4 rating order.
    pub fn as_i64(self) -> i64 {
        match self {
            Rating::Again => 1,
            Rating::Hard => 2,
            Rating::Good => 3,
            Rating::Easy => 4,
        }
    }
}

/// A card's current FSRS memory. Absent for a card being reviewed for the first time — the
/// first-Review path passes `None`, not a zeroed memory, since `next_states` treats those
/// differently (milestone risk note).
#[derive(Debug, Clone, Copy)]
pub struct Memory {
    pub stability: f32,
    pub difficulty: f32,
}

/// What FSRS returns for the chosen rating: the card's new memory and the interval, in days, until
/// it is next due. The interval is `f32` as the crate reports it; the caller rounds to whole days
/// when computing a due date.
#[derive(Debug, Clone, Copy)]
pub struct Schedule {
    pub stability: f32,
    pub difficulty: f32,
    pub interval_days: f32,
}

pub struct Scheduler {
    fsrs: FSRS,
    desired_retention: f32,
}

impl Scheduler {
    /// Build the scheduler from FSRS's default parameters and the configured desired retention.
    pub fn new(desired_retention: f32) -> Result<Self> {
        let fsrs =
            FSRS::new(&DEFAULT_PARAMETERS).context("constructing FSRS from default parameters")?;
        Ok(Self {
            fsrs,
            desired_retention,
        })
    }

    /// The schedule for `rating`, given the card's current memory (`None` on first Review) and the
    /// whole days elapsed since its last Review. Computes all four next-states and returns the one
    /// the developer picked.
    pub fn next(
        &self,
        rating: Rating,
        current: Option<Memory>,
        days_elapsed: u32,
    ) -> Result<Schedule> {
        let memory = current.map(|m| MemoryState {
            stability: m.stability,
            difficulty: m.difficulty,
        });
        let states = self
            .fsrs
            .next_states(memory, self.desired_retention, days_elapsed)
            .map_err(|error| anyhow!("fsrs next_states failed: {error:?}"))?;
        let chosen = match rating {
            Rating::Again => states.again,
            Rating::Hard => states.hard,
            Rating::Good => states.good,
            Rating::Easy => states.easy,
        };
        Ok(Schedule {
            stability: chosen.memory.stability,
            difficulty: chosen.memory.difficulty,
            interval_days: chosen.interval,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratings_map_to_the_fsrs_rating_order() {
        assert_eq!(Rating::Again.as_i64(), 1);
        assert_eq!(Rating::Hard.as_i64(), 2);
        assert_eq!(Rating::Good.as_i64(), 3);
        assert_eq!(Rating::Easy.as_i64(), 4);
    }

    #[test]
    fn a_first_review_produces_a_usable_schedule_for_every_rating() {
        let scheduler = Scheduler::new(0.9).expect("scheduler");
        for rating in [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy] {
            let schedule = scheduler.next(rating, None, 0).expect("schedule");
            assert!(schedule.stability > 0.0, "{rating:?} stability");
            assert!(schedule.interval_days.is_finite(), "{rating:?} interval");
            assert!(schedule.interval_days >= 0.0, "{rating:?} interval sign");
        }
    }

    #[test]
    fn easy_schedules_no_sooner_than_again_so_the_mapping_is_not_swapped() {
        // Not a test of FSRS's maths, only that Again and Easy are wired to the right states:
        // a passed card cannot be scheduled sooner than a failed one.
        let scheduler = Scheduler::new(0.9).expect("scheduler");
        let again = scheduler.next(Rating::Again, None, 0).expect("again");
        let easy = scheduler.next(Rating::Easy, None, 0).expect("easy");
        assert!(easy.interval_days >= again.interval_days);
    }
}
