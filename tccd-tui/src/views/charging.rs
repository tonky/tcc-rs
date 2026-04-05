use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::model::Model;
use crate::widgets::form;

pub fn render_charging(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Charging ");

    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    match &model.charging.form {
        Some(form_state) => {
            form::render_form(form_state, inner, frame);
        }
        None => {
            let text = Paragraph::new("  Loading charging settings...");
            frame.render_widget(text, inner);
        }
    }

    // Help bar
    let help = Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("Navigate  "),
        Span::styled("←→/Space ", Style::default().fg(Color::Cyan)),
        Span::raw("Edit  "),
        Span::styled("s ", Style::default().fg(Color::Cyan)),
        Span::raw("Save  "),
        Span::styled("Esc ", Style::default().fg(Color::Cyan)),
        Span::raw("Discard"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[1]);
}
