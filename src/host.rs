//! The host: one event loop owning all state, fed by producer threads over one channel
//! (ADR-0009).
//!
//! Nothing here is behind a lock, because nothing here is shared. Producer threads send; this
//! thread decides.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use anyhow::Result;
use chrono::Duration;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;

use crate::clock::Clock;
use crate::event::Event;
use crate::frame::FrameType;
use crate::renderer::{PaneState, draw};
use crate::triggers::OpenTriggers;

/// The default Trigger expiry (ADR-0006), erring short: expiring early clears a card while the
/// developer is still waiting, which self-corrects on the next Trigger, whereas expiring late
/// pins a stale card up forever.
///
/// M2 moves this into the `config` table, which is where ADR-0006 requires it to live.
pub const DEFAULT_TRIGGER_EXPIRY_SECONDS: i64 = 1800;

/// The hardcoded card M1 renders. Deliberate scaffolding: M2 replaces it with a real card from
/// the deck. Kept as one constant so that replacement is a single deletion.
pub const PLACEHOLDER_CARD_FRONT: &str = "What does FSRS stand for?";

/// Called with the drawn buffer after every frame. Tests use this to observe what the developer
/// would see; the real host passes `None`.
pub type DrawObserver = Box<dyn FnMut(&Buffer) + Send>;

pub struct Host<B: Backend> {
    terminal: Terminal<B>,
    triggers: OpenTriggers,
    clock: Arc<dyn Clock>,
    on_draw: Option<DrawObserver>,
}

impl<B: Backend> Host<B>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn new(terminal: Terminal<B>, clock: Arc<dyn Clock>, expiry: Duration) -> Self {
        Self {
            terminal,
            triggers: OpenTriggers::new(expiry),
            clock,
            on_draw: None,
        }
    }

    pub fn with_draw_observer(mut self, observer: DrawObserver) -> Self {
        self.on_draw = Some(observer);
        self
    }

    /// Consume events until the developer quits or every sender is gone.
    pub fn run(mut self, rx: Receiver<Event>) -> Result<()> {
        self.draw()?;

        while let Ok(event) = rx.recv() {
            match event {
                Event::Frame(frame) => {
                    // Expiry is measured against the host's clock rather than the frame's `at`:
                    // the adapter's clock is not ours to trust, and a skewed one would otherwise
                    // decide when a Trigger drains.
                    let now = self.clock.now();
                    match frame.frame_type {
                        FrameType::TriggerOpen => self.triggers.open(frame.key(), now),
                        FrameType::TriggerClose => self.triggers.close(&frame.key()),
                    };
                }
                Event::Tick => {
                    self.triggers.sweep(self.clock.now());
                }
                Event::Key(key) => {
                    if is_quit(&key) {
                        return Ok(());
                    }
                }
            }
            self.draw()?;
        }

        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        // Disjoint field borrows: the observer inspects the buffer the terminal just produced.
        let Self {
            terminal,
            triggers,
            on_draw,
            ..
        } = self;

        let state = if triggers.is_waiting() {
            PaneState::Card {
                front: PLACEHOLDER_CARD_FRONT,
            }
        } else {
            PaneState::Idle
        };

        let completed = terminal.draw(|frame| draw(frame, &state))?;
        if let Some(observer) = on_draw {
            observer(completed.buffer);
        }
        Ok(())
    }
}

fn is_quit(key: &KeyEvent) -> bool {
    if key.kind == KeyEventKind::Release {
        return false;
    }
    match key.code {
        KeyCode::Char('q') => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}
