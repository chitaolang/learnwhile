//! LearnWhile — turning time spent waiting on an AI coding agent into short bursts of review.
//!
//! Vocabulary follows `/CONTEXT.md`; decisions live in `docs/adr/`. The crate is a library so
//! that integration tests can boot the host in-process through one seam (`testing`), rather than
//! testing modules the developer never interacts with directly.

pub mod clock;
pub mod event;
pub mod frame;
pub mod hook;
pub mod host;
pub mod listener;
pub mod renderer;
pub mod scheduler;
pub mod socket;
pub mod storage;
pub mod triggers;

#[cfg(feature = "testing")]
pub mod testing;
