//! The single event type (ADR-0009).
//!
//! Every input reaches the loop as one of these, on one channel. Producer threads translate and
//! send; they hold no state and make no decisions.

use crossterm::event::KeyEvent;

use crate::frame::TriggerFrame;

#[derive(Debug, Clone)]
pub enum Event {
    /// A valid frame off the socket. Malformed lines never get this far.
    Frame(TriggerFrame),
    /// A keypress. Tests inject these directly rather than through a terminal.
    Key(KeyEvent),
    /// The expiry sweep's own timer (ADR-0006), never driven by frame arrival.
    Tick,
    /// A termination signal (SIGINT/SIGTERM), translated by the signal producer thread. Handled
    /// like the quit key so the loop returns through the terminal-restoring path (M5), rather than
    /// the process being killed with the terminal still in raw mode.
    Shutdown,
}
