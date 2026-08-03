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
use crate::frame::{FrameType, TriggerKey, Verdict};
use crate::learning::{Learning, ReviewView};
use crate::renderer::{PaneState, draw};
use crate::scheduler::Rating;
use crate::triggers::{OpenTriggers, WaitingEdge};

/// The Session-scoped review debt the Prompt Gate reads (spec §Review debt, ADR-0014). `None` at the
/// start of a cycle, `Owed` once a card is surfaced and not yet rated, `Paid` once any rating lands.
/// Only `Owed` blocks the next prompt, and a rating cannot re-arm it within a cycle, which keeps the
/// gate to "one Review per handoff" rather than "clear the whole queue". Tracked whether or not a
/// gate is active; read only when one is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Debt {
    None,
    Owed,
    Paid,
}

/// The Prompt Gate's Session state (M7): the review debt, plus whether a gate query has been seen
/// this Session. Bundled so the "one Review per handoff" invariant lives in one place. Tracked
/// whether or not the developer opted in; only read when they have.
struct Gate {
    debt: Debt,
    /// Set the first time a gate query arrives this Session. Until then the idle pane never shows a
    /// card, so a developer who never passes `--gate` sees v1 behaviour unchanged (ADR-0015).
    active: bool,
}

impl Gate {
    fn new() -> Self {
        Self {
            debt: Debt::None,
            active: false,
        }
    }

    /// Record that this Session is using the gate.
    fn seen(&mut self) {
        self.active = true;
    }

    /// Whether the next prompt must be held: a Review is owed.
    fn owed(&self) -> bool {
        self.debt == Debt::Owed
    }

    /// Whether an owed card should be held on the idle pane for payment — only under an active gate.
    fn holding(&self) -> bool {
        self.active && self.owed()
    }

    /// Start a new cycle, so the next surfaced card re-arms the debt.
    fn begin_cycle(&mut self) {
        self.debt = Debt::None;
    }

    /// A rating clears the debt for this cycle. A resurfaced card cannot re-arm it (arming only
    /// fires from `None`), so the gate stays "one Review per handoff".
    fn pay(&mut self) {
        self.debt = Debt::Paid;
    }

    /// Arm the debt if a card is now in flight and none was owed yet. An idle wait (no card) arms
    /// nothing; a `Paid` debt is left paid.
    fn arm(&mut self, card_in_flight: bool) {
        if self.debt == Debt::None && card_in_flight {
            self.debt = Debt::Owed;
        }
    }
}

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
    /// The Prompt Gate's Session state (M7).
    gate: Gate,
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
            gate: Gate::new(),
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
                Event::Frame(frame) => match frame.frame_type {
                    FrameType::TriggerOpen => self.open_and_surface(frame.key())?,
                    FrameType::TriggerClose => {
                        self.triggers.close(&frame.key());
                    }
                    // Gate queries reach the loop as `Event::GateQuery`, never here: the listener
                    // routes them so it can write the verdict back on the connection.
                    FrameType::GateQuery => {}
                },
                // A `--gate` hook asking whether a Review is owed before its prompt proceeds (M7).
                Event::GateQuery { key, reply } => {
                    self.gate.seen();
                    if self.gate.owed() {
                        // Hold the prompt. The owed card is already shown while idle, so the
                        // developer can clear it. The Trigger does not open: no handoff happened.
                        let _ = reply.send(Verdict::Block);
                    } else {
                        let _ = reply.send(Verdict::Allow);
                        // A new cycle: clear the debt, then open the Trigger exactly as a plain open.
                        self.gate.begin_cycle();
                        self.open_and_surface(key)?;
                    }
                }
                Event::Tick => {
                    self.triggers.sweep(self.clock.now());
                }
                // A termination signal: leave the loop so `run_host` restores the terminal, exactly
                // as the quit key does.
                Event::Shutdown => return Ok(()),
                Event::Key(key) => {
                    if is_quit(&key) {
                        return Ok(());
                    }
                    // Reveal and ratings only mean anything while a card is up — while Waiting, or
                    // while paying an owed Review from the idle pane under an active gate (M7).
                    if self.showing_card() {
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
        self.gate.pay();
        if self.triggers.is_waiting() {
            self.learning.surface()?;
        }
        Ok(())
    }

    /// Whether a card is on screen and its keys are live: while Waiting, or while an active gate is
    /// holding the next prompt on an owed Review, so the developer can pay it from the idle pane.
    fn showing_card(&self) -> bool {
        self.triggers.is_waiting() || self.gate.holding()
    }

    /// Arm the debt from a surfacing: a card in flight incurs the Review debt, an idle wait does not.
    fn arm_debt_if_owed(&mut self) {
        let card_in_flight = !matches!(self.learning.view(), ReviewView::Empty);
        self.gate.arm(card_in_flight);
    }

    /// Open a Trigger and, if this began a wait, surface a card into it, then arm the debt. Shared by
    /// a plain `TriggerOpen` frame and the gate's allow branch, which are the same handoff. Expiry is
    /// measured against the host's clock, not the frame's `at`: the adapter's clock is not ours to
    /// trust, and a skewed one would otherwise decide when a Trigger drains (ADR-0006). A Review
    /// already in flight is left untouched (spec §Review flow).
    fn open_and_surface(&mut self, key: TriggerKey) -> Result<()> {
        let now = self.clock.now();
        if self.triggers.open(key, now) == WaitingEdge::BecameWaiting {
            self.learning.surface()?;
        }
        self.arm_debt_if_owed();
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        // Computed before the field borrow below: a card shows while Waiting, or while an active gate
        // holds an owed Review. `holding` is that second case — shown from the idle pane, not a wait.
        let showing = self.showing_card();
        let waiting = self.triggers.is_waiting();
        let holding = showing && !waiting;

        // Disjoint field borrows: the observer inspects the buffer the terminal just produced, and
        // the pane view borrows the in-flight card from `learning`.
        let Self {
            terminal,
            learning,
            on_draw,
            ..
        } = self;

        let state = match learning.view() {
            ReviewView::Question { front } if showing => PaneState::Question { front, holding },
            ReviewView::Answer { front, back } if showing => PaneState::Answer {
                front,
                back,
                holding,
            },
            // Not waiting and no owed card to pay: the idle pane with real counts.
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
