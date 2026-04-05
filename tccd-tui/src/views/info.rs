use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::model::Model;

pub fn render_info(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" System Information ");
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    let mut lines = Vec::new();

    let version = model
        .info
        .tcc_version
        .as_deref()
        .unwrap_or("Loading...");
    let daemon_ver = model
        .info
        .daemon_version
        .as_deref()
        .unwrap_or("Loading...");
    let hostname = model.info.hostname.as_deref().unwrap_or("Loading...");
    let kernel = model
        .info
        .kernel_version
        .as_deref()
        .unwrap_or("Loading...");

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  TCC Version:    ", Style::default().fg(Color::Cyan)),
        Span::raw(version),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Daemon Version: ", Style::default().fg(Color::Cyan)),
        Span::raw(daemon_ver),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Hostname:       ", Style::default().fg(Color::Cyan)),
        Span::raw(hostname),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Kernel:         ", Style::default().fg(Color::Cyan)),
        Span::raw(kernel),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);

    // Help bar
    let help = Line::from(vec![
        Span::styled(" ? ", Style::default().fg(Color::Cyan)),
        Span::raw("Help  "),
        Span::styled("q ", Style::default().fg(Color::Cyan)),
        Span::raw("Quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[1]);
}
