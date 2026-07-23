//! The Renderer: draws the current card or the idle state, and contains no business logic.
//!
//! The pane is passive (ADR-0001). It never takes foreground focus, so the developer cannot miss
//! a permission prompt because LearnWhile was in the way.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// What the pane is showing. M1 has exactly two states and no card content of its own.
pub enum PaneState<'a> {
    /// Waiting: the open-Trigger set is non-empty.
    Card { front: &'a str },
    /// Not Waiting. M3 replaces this placeholder with real due/new counts and the next due time.
    Idle,
}

pub fn draw(frame: &mut Frame, state: &PaneState) {
    let areas = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(frame.area());

    let block = Block::default().borders(Borders::ALL).title(" LearnWhile ");

    let body = match state {
        PaneState::Card { front } => Paragraph::new(vec![
            Line::from(Span::styled(
                "Waiting",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(*front),
        ]),
        PaneState::Idle => Paragraph::new(vec![
            Line::from(Span::styled(
                "Not waiting",
                Style::default().add_modifier(Modifier::DIM),
            )),
            Line::from(""),
            Line::from("Submit a prompt to your agent and a card will appear here."),
        ]),
    };

    frame.render_widget(body.block(block).wrap(Wrap { trim: true }), areas[0]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "q quit",
            Style::default().add_modifier(Modifier::DIM),
        ))),
        areas[1],
    );
}
