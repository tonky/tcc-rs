use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::model::Model;
use crate::widgets::form;

pub fn render_power(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // GPU info
            Constraint::Min(0),    // Power form
            Constraint::Length(2), // Help bar
        ])
        .split(area);

    // GPU info section
    let gpu_block = Block::default()
        .borders(Borders::ALL)
        .title(" GPU Information ");
    let gpu_inner = gpu_block.inner(chunks[0]);
    frame.render_widget(gpu_block, chunks[0]);

    if let Some(ref gpu) = model.power.gpu_info {
        let lines = vec![
            Line::from(vec![
                Span::styled("dGPU: ", Style::default().fg(Color::Cyan)),
                Span::raw(&gpu.dgpu_name),
                Span::raw("  "),
                Span::styled("Temp: ", Style::default().fg(Color::Yellow)),
                Span::raw(gpu.dgpu_temp.map_or("N/A".into(), |t| format!("{:.0}°C", t))),
                Span::raw("  "),
                Span::styled("Usage: ", Style::default().fg(Color::Yellow)),
                Span::raw(gpu.dgpu_usage.map_or("N/A".into(), |u| format!("{:.0}%", u))),
                Span::raw("  "),
                Span::styled("Power: ", Style::default().fg(Color::Yellow)),
                Span::raw(gpu.dgpu_power_draw.map_or("N/A".into(), |p| format!("{:.1}W", p))),
            ]),
            Line::from(vec![
                Span::styled("iGPU: ", Style::default().fg(Color::Cyan)),
                Span::raw(&gpu.igpu_name),
                Span::raw("  "),
                Span::styled("Usage: ", Style::default().fg(Color::Yellow)),
                Span::raw(gpu.igpu_usage.map_or("N/A".into(), |u| format!("{:.0}%", u))),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines), gpu_inner);
    } else {
        frame.render_widget(Paragraph::new("  Loading GPU info..."), gpu_inner);
    }

    // Power settings form
    let power_block = Block::default()
        .borders(Borders::ALL)
        .title(" Power Settings ");
    let power_inner = power_block.inner(chunks[1]);
    frame.render_widget(power_block, chunks[1]);

    match &model.power.form {
        Some(form_state) => {
            form::render_form(form_state, power_inner, frame);
        }
        None => {
            frame.render_widget(Paragraph::new("  Loading power settings..."), power_inner);
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
    frame.render_widget(Paragraph::new(help), chunks[2]);
}
