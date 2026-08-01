//! The Renderer: draws the current Review side or the idle state, and contains no business logic.
//!
//! The pane is passive (ADR-0001). It never takes foreground focus, so the developer cannot miss
//! a permission prompt because LearnWhile was in the way.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::furigana;

/// What the pane is showing. The question side deliberately carries no answer text, so revealing is
/// the only way the back reaches the buffer.
pub enum PaneState<'a> {
    /// Question side: the card's front, before the answer is revealed. `holding` is true when the
    /// card is shown from the idle pane to pay an owed Review under an active Prompt Gate (M7),
    /// rather than during a wait.
    Question { front: &'a str, holding: bool },
    /// Answer side: front and back, after the reveal key. `holding` as in `Question`.
    Answer {
        front: &'a str,
        back: &'a str,
        holding: bool,
    },
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

    // The width available for text inside the block, minus its two border columns. Furigana
    // pre-wraps to this so a reading and its base always share a line.
    let inner_width = areas[0].width.saturating_sub(2) as usize;
    // The heading tells the developer why the card is up: a normal wait, or an active Prompt Gate
    // holding their next prompt until they finish this Review (M7).
    let heading = |holding: bool| {
        let text = if holding {
            "Review to continue"
        } else {
            "Waiting"
        };
        Line::from(Span::styled(
            text,
            Style::default().add_modifier(Modifier::BOLD),
        ))
    };

    // The footer names the keys available in the current state, so a developer mid-wait never has
    // to remember them (milestone: available keys visible on screen). `wrap` is false only on the
    // furigana path, whose lines are already laid out; every other state keeps ratatui's word wrap.
    let (lines, wrap, footer): (Vec<Line>, bool, &str) = match state {
        // The question side shows base kanji only, so a card testing a reading isn't handed its
        // answer (ADR-0013). An unannotated front stays on the exact prior draw path.
        PaneState::Question { front, holding } if furigana::has_ruby(front) => {
            let mut lines = vec![heading(*holding), Line::from("")];
            lines.extend(
                furigana::layout(&furigana::base_only(front), inner_width)
                    .into_iter()
                    .map(Line::from),
            );
            (lines, false, "space reveal    q quit")
        }
        PaneState::Question { front, holding } => (
            vec![heading(*holding), Line::from(""), Line::from(*front)],
            true,
            "space reveal    q quit",
        ),
        // On reveal, front and back render with readings stacked over their kanji.
        PaneState::Answer {
            front,
            back,
            holding,
        } if furigana::has_ruby(front) || furigana::has_ruby(back) => {
            let mut lines = vec![heading(*holding), Line::from("")];
            lines.extend(
                furigana::layout(front, inner_width)
                    .into_iter()
                    .map(Line::from),
            );
            lines.push(Line::from(""));
            lines.extend(
                furigana::layout(back, inner_width)
                    .into_iter()
                    .map(Line::from),
            );
            (lines, false, "1 Again   2 Hard   3 Good   4 Easy    q quit")
        }
        PaneState::Answer {
            front,
            back,
            holding,
        } => (
            vec![
                heading(*holding),
                Line::from(""),
                Line::from(*front),
                Line::from(""),
                Line::from(*back),
            ],
            true,
            "1 Again   2 Hard   3 Good   4 Easy    q quit",
        ),
        PaneState::Idle {
            waiting: is_waiting,
            due_today,
            new_remaining,
            next_due,
        } => {
            let (header, header_modifier) = if *is_waiting {
                ("Waiting", Modifier::BOLD)
            } else {
                ("Not waiting", Modifier::DIM)
            };
            let next = match next_due {
                Some(when) => format!("Next due: {when}"),
                None => "Next due: nothing scheduled".to_string(),
            };
            (
                vec![
                    Line::from(Span::styled(
                        header,
                        Style::default().add_modifier(header_modifier),
                    )),
                    Line::from(""),
                    Line::from(format!(
                        "Due now: {due_today}    New remaining: {new_remaining}"
                    )),
                    Line::from(next),
                ],
                true,
                "q quit",
            )
        }
    };

    let mut body = Paragraph::new(lines).block(block);
    if wrap {
        body = body.wrap(Wrap { trim: true });
    }
    frame.render_widget(body, areas[0]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            footer,
            Style::default().add_modifier(Modifier::DIM),
        ))),
        areas[1],
    );
}
