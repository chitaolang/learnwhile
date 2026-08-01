//! The single event type (ADR-0009).
//!
//! Every input reaches the loop as one of these, on one channel. Producer threads translate and
//! send; they hold no state and make no decisions.

use std::sync::mpsc::Sender;

use crossterm::event::KeyEvent;

use crate::frame::{TriggerFrame, TriggerKey, Verdict};

#[derive(Debug, Clone)]
pub enum Event {
    /// A valid frame off the socket. Malformed lines never get this far.
    Frame(TriggerFrame),
    /// A `--gate` hook's request/response (ADR-0016): before opening a Trigger it asks whether a
    /// Review is owed. The host replies down `reply`, and on allow opens the Trigger itself. This is
    /// the one exchange carved out of the otherwise one-way frame protocol.
    GateQuery {
        key: TriggerKey,
        reply: Sender<Verdict>,
    },
    /// A keypress. Tests inject these directly rather than through a terminal.
    Key(KeyEvent),
    /// The expiry sweep's own timer (ADR-0006), never driven by frame arrival.
    Tick,
    /// A termination signal (SIGINT/SIGTERM), translated by the signal producer thread. Handled
    /// like the quit key so the loop returns through the terminal-restoring path (M5), rather than
    /// the process being killed with the terminal still in raw mode.
    Shutdown,
}
