pub mod charging;
pub mod dashboard;
pub mod display;
pub mod fan_curve;
pub mod help;
pub mod info;
pub mod keyboard;
pub mod power;
pub mod profiles;
pub mod settings;
pub mod webcam;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

use crate::model::{ConnectionStatus, Model, Tab};

/// Top-level view dispatch — renders tab bar, active view, and status bar.
pub fn view(model: &Model, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),   // main content
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    render_tab_bar(model, frame, chunks[0]);

    match model.active_tab {
        Tab::Dashboard => dashboard::render_dashboard(model, frame, chunks[1]),
        Tab::Profiles => profiles::view(model, frame, chunks[1]),
        Tab::FanCurve => fan_curve::render_fan_curve(model, frame, chunks[1]),
        Tab::Settings => settings::render_settings(model, frame, chunks[1]),
        Tab::Keyboard => keyboard::render_keyboard(model, frame, chunks[1]),
        Tab::Charging => charging::render_charging(model, frame, chunks[1]),
        Tab::Power => power::render_power(model, frame, chunks[1]),
        Tab::Display => display::render_display(model, frame, chunks[1]),
        Tab::Webcam => webcam::render_webcam(model, frame, chunks[1]),
        Tab::Info => info::render_info(model, frame, chunks[1]),
    }

    render_status_bar(model, frame, chunks[2]);

    // Help overlay on top of everything
    if model.help_visible {
        help::render_help(frame);
    }
}

fn render_tab_bar(model: &Model, frame: &mut Frame, area: ratatui::layout::Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let key = if i < 9 { (i + 1).to_string() } else { "0".to_string() };
            Line::from(format!("{} {}", key, t.label()))
        })
        .collect();
    let selected = Tab::ALL
        .iter()
        .position(|t| *t == model.active_tab)
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" TCC "))
        .select(selected)
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    frame.render_widget(tabs, area);
}

fn render_status_bar(model: &Model, frame: &mut Frame, area: ratatui::layout::Rect) {
    let conn_status = match model.connection_status {
        ConnectionStatus::Connected => {
            Span::styled(" ● Connected ", Style::default().fg(Color::Green))
        }
        ConnectionStatus::Disconnected => {
            Span::styled(" ● Disconnected ", Style::default().fg(Color::Red))
        }
    };

    let notification = model
        .notifications
        .back()
        .map(|n| {
            let color = if n.is_error { Color::Red } else { Color::Green };
            Span::styled(format!(" {} ", n.message), Style::default().fg(color))
        })
        .unwrap_or_else(|| Span::raw(" q: quit  1-9,0: tabs  ?: help "));

    let status = Line::from(vec![conn_status, Span::raw(" │ "), notification]);
    frame.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray)),
        area,
    );
}
