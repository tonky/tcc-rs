use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph};

use crate::model::Model;

pub fn render_fan_curve(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.fan_curve;

    if state.curve_points.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Fan Curve — No data ");
        frame.render_widget(block, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(2)])
        .split(area);

    // Build datasets
    let curve_data: Vec<(f64, f64)> = state.curve_points.clone();

    // All curve points as visible markers
    let all_points_data: Vec<(f64, f64)> = state.curve_points.clone();

    // Current operating point (defined outside if-let so reference lives long enough)
    let current_temp = model.dashboard.cpu.temperature;
    let current_speed = model.dashboard.fan.speeds_percent.first().copied();
    let current_point_data = match (current_temp, current_speed) {
        (Some(t), Some(s)) => vec![(t, s as f64)],
        _ => vec![],
    };

    // Selected point
    let selected_point_data = state
        .curve_points
        .get(state.selected_point)
        .map(|&(t, s)| vec![(t, s)])
        .unwrap_or_default();

    let mut datasets = vec![
        Dataset::default()
            .name(state.fan_profile_name.as_str())
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&curve_data),
        Dataset::default()
            .name("")
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Scatter)
            .style(Style::default().fg(Color::White))
            .data(&all_points_data),
    ];

    if !current_point_data.is_empty() {
        let (t, s) = current_point_data[0];
        datasets.push(
            Dataset::default()
                .name(format!("Now: {:.0}°C/{:.0}%", t, s))
                .marker(symbols::Marker::Dot)
                .style(Style::default().fg(Color::Yellow))
                .data(&current_point_data),
        );
    }

    if !selected_point_data.is_empty() {
        let (t, s) = selected_point_data[0];
        datasets.push(
            Dataset::default()
                .name(format!("Sel: {:.0}°C/{:.0}%", t, s))
                .marker(symbols::Marker::Block)
                .style(Style::default().fg(Color::Magenta))
                .data(&selected_point_data),
        );
    }

    let x_labels: Vec<Span> = ["0", "25", "50", "75", "100"]
        .iter()
        .map(|s| Span::raw(*s))
        .collect();
    let y_labels: Vec<Span> = ["0", "25", "50", "75", "100"]
        .iter()
        .map(|s| Span::raw(*s))
        .collect();

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Fan Curve — {} ", state.fan_profile_name)),
        )
        .x_axis(
            Axis::default()
                .title("Temp (°C)")
                .bounds([0.0, 105.0])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("Speed (%)")
                .bounds([0.0, 105.0])
                .labels(y_labels),
        );

    frame.render_widget(chart, chunks[0]);

    // Info bar
    let selected_info = state
        .curve_points
        .get(state.selected_point)
        .map(|(t, s)| format!("Selected: {:.0}°C → {:.0}%", t, s))
        .unwrap_or_default();

    let current_info = match (current_temp, current_speed) {
        (Some(t), Some(s)) => format!("Current: {:.0}°C → {}%", t, s),
        _ => String::new(),
    };

    let help = Line::from(vec![
        Span::styled(" ←→ ", Style::default().fg(Color::Cyan)),
        Span::raw("Select  "),
        Span::styled("↑↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("Adjust  "),
        Span::styled("i ", Style::default().fg(Color::Cyan)),
        Span::raw("Insert  "),
        Span::styled("x ", Style::default().fg(Color::Cyan)),
        Span::raw("Delete  "),
        Span::styled("r ", Style::default().fg(Color::Cyan)),
        Span::raw("Reset  "),
        if state.is_dirty() {
            Span::styled("s ", Style::default().fg(Color::Yellow))
        } else {
            Span::styled("s ", Style::default().fg(Color::DarkGray))
        },
        if state.is_dirty() {
            Span::raw("Save  ")
        } else {
            Span::styled("Save  ", Style::default().fg(Color::DarkGray))
        },
        Span::raw(&selected_info),
        Span::raw("  "),
        Span::raw(&current_info),
        if state.is_dirty() {
            Span::styled("  [modified]", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("")
        },
    ]);
    frame.render_widget(Paragraph::new(help), chunks[1]);
}
