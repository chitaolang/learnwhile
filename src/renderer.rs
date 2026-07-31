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
    /// Not Waiting, or Waiting with nothing left to review. Carries the real counts so the pane
    /// tells "nothing due" apart from "not Waiting". `next_due` is preformatted for display.
    Idle {
        waiting: bool,
        due_today: i64,
        new_remaining: i64,
        next_due: Option<String>,
    },
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
        PaneState::Idle {
            waiting,
            due_today,
            new_remaining,
            next_due,
        } => {
            let (header, header_modifier) = if *waiting {
                ("Waiting", Modifier::BOLD)
            } else {
                ("Not waiting", Modifier::DIM)
            };
            let next = match next_due {
                Some(when) => format!("Next due: {when}"),
                None => "Next due: nothing scheduled".to_string(),
            };
            (
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        header,
                        Style::default().add_modifier(header_modifier),
                    )),
                    Line::from(""),
                    Line::from(format!(
                        "Due now: {due_today}    New remaining: {new_remaining}"
                    )),
                    Line::from(next),
                ]),
                "q quit",
            )
        }
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
