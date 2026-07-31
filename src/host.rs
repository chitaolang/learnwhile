//! The host: one event loop owning all state, fed by producer threads over one channel (ADR-0009).
//!
//! Nothing here is behind a lock, because nothing here is shared. Producer threads send; this
//! thread decides.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use anyhow::Result;
use chrono::{DateTime, Duration, Local, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;

use crate::clock::Clock;
use crate::event::Event;
use crate::frame::FrameType;
use crate::learning::{Learning, ReviewView};
use crate::renderer::{PaneState, draw};
use crate::scheduler::Rating;
use crate::triggers::{OpenTriggers, WaitingEdge};

/// The default Trigger expiry (ADR-0006), erring short: expiring early clears a card while the
/// developer is still waiting, which self-corrects on the next Trigger, whereas expiring late
/// pins a stale card up forever.
///
/// The host no longer reads this: as of M2 it reads `trigger_expiry_seconds` from the `config`
/// table, which is where ADR-0006 requires the value to live. This constant is the compile-time
/// twin of the `1800` the storage migration seeds, kept as the default the test harness injects.
pub const DEFAULT_TRIGGER_EXPIRY_SECONDS: i64 = 1800;

/// Called with the drawn buffer after every frame. Tests use this to observe what the developer
/// would see; the real host passes `None`.
pub type DrawObserver = Box<dyn FnMut(&Buffer) + Send>;

pub struct Host<B: Backend> {
    terminal: Terminal<B>,
    triggers: OpenTriggers,
    clock: Arc<dyn Clock>,
    learning: Learning,
    on_draw: Option<DrawObserver>,
}

impl<B: Backend> Host<B>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn new(
        terminal: Terminal<B>,
        clock: Arc<dyn Clock>,
        expiry: Duration,
        learning: Learning,
    ) -> Self {
        Self {
            terminal,
            triggers: OpenTriggers::new(expiry),
            clock,
            learning,
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
                    let edge = match frame.frame_type {
                        FrameType::TriggerOpen => self.triggers.open(frame.key(), now),
                        FrameType::TriggerClose => self.triggers.close(&frame.key()),
                    };
                    // The empty→non-empty edge is when a wait begins: surface a card into it. A
                    // Review already in flight is left untouched (spec §Review flow).
                    if edge == WaitingEdge::BecameWaiting {
                        self.learning.surface()?;
                    }
                }
                Event::Tick => {
                    self.triggers.sweep(self.clock.now());
                }
                Event::Key(key) => {
                    if is_quit(&key) {
                        return Ok(());
                    }
                    // Reveal and ratings only mean anything while a card is up.
                    if self.triggers.is_waiting() {
                        self.handle_review_key(&key)?;
                    }
                }
            }
            self.draw()?;
        }

        Ok(())
    }

    /// Reveal on space; rate on 1..4. Anything else is ignored.
    fn handle_review_key(&mut self, key: &KeyEvent) -> Result<()> {
        if key.kind == KeyEventKind::Release {
            return Ok(());
        }
        match key.code {
            KeyCode::Char(' ') => self.learning.reveal(),
            KeyCode::Char('1') => self.rate(Rating::Again)?,
            KeyCode::Char('2') => self.rate(Rating::Hard)?,
            KeyCode::Char('3') => self.rate(Rating::Good)?,
            KeyCode::Char('4') => self.rate(Rating::Easy)?,
            _ => {}
        }
        Ok(())
    }

    /// Persist the rating, then surface the next card so a long wait can hold several Reviews
    /// (spec user story 12).
    fn rate(&mut self, rating: Rating) -> Result<()> {
        self.learning.rate(rating)?;
        if self.triggers.is_waiting() {
            self.learning.surface()?;
        }
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        // Disjoint field borrows: the observer inspects the buffer the terminal just produced, and
        // the pane view borrows the in-flight card from `learning`.
        let Self {
            terminal,
            triggers,
            learning,
            on_draw,
            ..
        } = self;

        let waiting = triggers.is_waiting();
        let state = match learning.view() {
            ReviewView::Question { front } if waiting => PaneState::Question { front },
            ReviewView::Answer { front, back } if waiting => PaneState::Answer { front, back },
            // Not waiting, or waiting with nothing left to review: the idle pane with real counts.
            _ => {
                let stats = learning.idle_stats()?;
                PaneState::Idle {
                    waiting,
                    due_today: stats.due_today,
                    new_remaining: stats.new_remaining,
                    next_due: stats.next_due.map(format_local),
                }
            }
        };

        let completed = terminal.draw(|frame| draw(frame, &state))?;
        if let Some(observer) = on_draw {
            observer(completed.buffer);
        }
        Ok(())
    }
}

/// Format a due time in the developer's local timezone for the idle pane.
fn format_local(when: DateTime<Utc>) -> String {
    when.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
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
