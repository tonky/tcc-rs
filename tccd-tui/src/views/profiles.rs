use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};

use crate::model::{Model, ProfileView};
use crate::widgets::form;

pub fn view(model: &Model, frame: &mut Frame, area: Rect) {
    match &model.profiles.view {
        ProfileView::List => render_list(model, frame, area),
        ProfileView::Editor { profile_id } => {
            render_editor(model, frame, area, profile_id)
        }
    }
}

fn render_list(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    // Profile table
    let header = Row::new(vec![
        Cell::from("  Name"),
        Cell::from("AC"),
        Cell::from("BAT"),
        Cell::from("Type"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan));

    let rows: Vec<Row> = model
        .profiles
        .profiles
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_ac = model
                .profiles
                .ac_profile_id
                .as_deref()
                .is_some_and(|id| id == p.id);
            let is_bat = model
                .profiles
                .bat_profile_id
                .as_deref()
                .is_some_and(|id| id == p.id);
            let is_default = p.id.starts_with("__") && p.id.ends_with("__");

            let ac_marker = if is_ac { "●" } else { " " };
            let bat_marker = if is_bat { "●" } else { " " };
            let type_label = if is_default { "Default" } else { "Custom" };

            let style = if i == model.profiles.selected_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(format!("  {}", p.name)),
                Cell::from(ac_marker),
                Cell::from(bat_marker),
                Cell::from(type_label),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Profiles "),
    );

    // Use StatefulWidget for proper scroll tracking
    let mut table_state = TableState::default();
    table_state.select(Some(model.profiles.selected_index));
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    // Help bar
    let help = Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("Navigate  "),
        Span::styled("Enter ", Style::default().fg(Color::Cyan)),
        Span::raw("Edit  "),
        Span::styled("c ", Style::default().fg(Color::Cyan)),
        Span::raw("Copy  "),
        Span::styled("d ", Style::default().fg(Color::Cyan)),
        Span::raw("Delete  "),
        Span::styled("a ", Style::default().fg(Color::Cyan)),
        Span::raw("Set AC  "),
        Span::styled("b ", Style::default().fg(Color::Cyan)),
        Span::raw("Set BAT"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[1]);
}

fn render_editor(model: &Model, frame: &mut Frame, area: Rect, profile_id: &str) {
    let is_default = profile_id.starts_with("__") && profile_id.ends_with("__");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let title = if is_default {
        " Profile Editor (read-only) — Esc/q to go back "
    } else if model.profiles.editor_dirty {
        " Profile Editor (*modified) — s: Save  Esc/q: Back "
    } else {
        " Profile Editor — s: Save  Esc/q: Back "
    };

    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    match &model.profiles.editor_form {
        Some(form_state) => {
            form::render_form(form_state, inner, frame);
        }
        None => {
            let text = Paragraph::new("  Loading...");
            frame.render_widget(text, inner);
        }
    }

    // Help bar
    let mut help_spans = vec![
        Span::styled(" ↑↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("Navigate  "),
    ];
    if !is_default {
        help_spans.extend([
            Span::styled("←→/Space ", Style::default().fg(Color::Cyan)),
            Span::raw("Edit  "),
            Span::styled("s ", Style::default().fg(Color::Cyan)),
            Span::raw("Save  "),
        ]);
    }
    help_spans.extend([
        Span::styled("Esc/q ", Style::default().fg(Color::Cyan)),
        Span::raw("Back"),
    ]);
    frame.render_widget(Paragraph::new(Line::from(help_spans)), chunks[1]);
}
