//! The Renderer: draws the current Review side or the idle state, and contains no business logic.
//!
//! The pane is passive (ADR-0001). It never takes foreground focus, so the developer cannot miss
//! a permission prompt because LearnWhile was in the way.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// What the pane is showing. The question side deliberately carries no answer text, so revealing is
/// the only way the back reaches the buffer.
pub enum PaneState<'a> {
    /// Waiting, question side: the card's front, before the answer is revealed.
    Question { front: &'a str },
    /// Waiting, answer side: front and back, after the reveal key.
    Answer { front: &'a str, back: &'a str },
    /// Not Waiting, or Waiting with nothing left to review. M3 replaces this placeholder with real
    /// due/new counts and the next due time.
    Idle,
}

pub fn draw(frame: &mut Frame, state: &PaneState) {
    let areas = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(frame.area());

    let block = Block::default().borders(Borders::ALL).title(" LearnWhile ");

    // The footer names the keys available in the current state, so a developer mid-wait never has
    // to remember them (milestone: available keys visible on screen).
    let (body, footer) = match state {
        PaneState::Question { front } => (
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "Waiting",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(*front),
            ]),
            "space reveal    q quit",
        ),
        PaneState::Answer { front, back } => (
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "Waiting",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(*front),
                Line::from(""),
                Line::from(*back),
            ]),
            "1 Again   2 Hard   3 Good   4 Easy    q quit",
        ),
        PaneState::Idle => (
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "Not waiting",
                    Style::default().add_modifier(Modifier::DIM),
                )),
                Line::from(""),
                Line::from("Submit a prompt to your agent and a card will appear here."),
            ]),
            "q quit",
        ),
    };

    frame.render_widget(body.block(block).wrap(Wrap { trim: true }), areas[0]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            footer,
            Style::default().add_modifier(Modifier::DIM),
        ))),
        areas[1],
    );
}
