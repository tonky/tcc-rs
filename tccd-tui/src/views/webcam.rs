use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use ratatui::Frame;

use crate::model::Model;
use crate::widgets::form;

pub fn render_webcam(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Device selector
            Constraint::Min(0),    // Controls form
            Constraint::Length(2), // Help bar
        ])
        .split(area);

    // Device selector tabs
    let device_names: Vec<String> = if model.webcam.devices.is_empty() {
        vec!["No devices".into()]
    } else {
        model.webcam.devices.iter().map(|d| d.name.clone()).collect()
    };
    let tabs = Tabs::new(device_names)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Webcam Devices "),
        )
        .select(model.webcam.selected_device)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, chunks[0]);

    // Controls form
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Controls ");
    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);

    match &model.webcam.form {
        Some(form_state) => {
            form::render_form(form_state, inner, frame);
        }
        None => {
            let msg = if model.webcam.devices.is_empty() {
                "  No webcam devices detected"
            } else {
                "  Loading controls..."
            };
            frame.render_widget(Paragraph::new(msg), inner);
        }
    }

    // Help bar
    let help = Line::from(vec![
        Span::styled(" ←→ ", Style::default().fg(Color::Cyan)),
        Span::raw("Device  "),
        Span::styled("↑↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("Navigate  "),
        Span::styled("←→/Space ", Style::default().fg(Color::Cyan)),
        Span::raw("Edit  "),
        Span::styled("s ", Style::default().fg(Color::Cyan)),
        Span::raw("Save  "),
        Span::styled("Esc ", Style::default().fg(Color::Cyan)),
        Span::raw("Discard"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[2]);
}
