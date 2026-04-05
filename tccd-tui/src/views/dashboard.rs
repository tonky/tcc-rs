use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

use crate::model::Model;

pub fn render_dashboard(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // fan gauges
            Constraint::Min(8),   // sparklines
            Constraint::Length(5), // CPU info
        ])
        .split(area);

    render_fan_section(model, frame, chunks[0]);
    render_sparklines(model, frame, chunks[1]);
    render_cpu_section(model, frame, chunks[2]);
}

fn render_fan_section(model: &Model, frame: &mut Frame, area: Rect) {
    let fan = &model.dashboard.fan;

    if fan.speeds_percent.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Fans ");
        let text = Paragraph::new("  No fan data").block(block);
        frame.render_widget(text, area);
        return;
    }

    let fan_count = fan.speeds_percent.len();
    let constraints: Vec<Constraint> = (0..fan_count)
        .map(|_| Constraint::Ratio(1, fan_count as u32))
        .collect();

    let fan_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, &speed) in fan.speeds_percent.iter().enumerate() {
        let label = format!("{}%", speed);
        let gauge_color = match speed {
            0..=40 => Color::Green,
            41..=70 => Color::Yellow,
            _ => Color::Red,
        };

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Fan {} ", i)),
            )
            .gauge_style(Style::default().fg(gauge_color))
            .percent(speed as u16)
            .label(label);

        frame.render_widget(gauge, fan_chunks[i]);
    }
}

fn render_sparklines(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Fan speed history
    let fan_data: Vec<u64> = model
        .dashboard
        .fan_speed_history
        .iter()
        .map(|v| *v as u64)
        .collect();

    let fan_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Fan Speed History "),
        )
        .data(&fan_data)
        .max(100)
        .style(Style::default().fg(Color::Cyan));

    frame.render_widget(fan_spark, chunks[0]);

    // CPU temperature history
    let cpu_data: Vec<u64> = model
        .dashboard
        .cpu_temp_history
        .iter()
        .map(|v| *v as u64)
        .collect();

    let cpu_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" CPU Temp History "),
        )
        .data(&cpu_data)
        .max(110)
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(cpu_spark, chunks[1]);
}

fn render_cpu_section(model: &Model, frame: &mut Frame, area: Rect) {
    let cpu = &model.dashboard.cpu;

    let temp_str = cpu
        .temperature
        .map(|t| format!("{:.1}°C", t))
        .unwrap_or_else(|| "—".into());

    let freq_str = cpu
        .avg_frequency_mhz
        .map(|f| format!("{:.0} MHz", f))
        .unwrap_or_else(|| "—".into());

    let cores_str = cpu
        .core_count
        .map(|c| format!("{}", c))
        .unwrap_or_else(|| "—".into());

    let power_str = match model.dashboard.power_on_ac {
        Some(true) => "⚡ AC",
        Some(false) => "🔋 Battery",
        None => "—",
    };

    let active_id = match model.dashboard.power_on_ac {
        Some(true) => model.profiles.ac_profile_id.as_deref(),
        Some(false) => model.profiles.bat_profile_id.as_deref(),
        _ => None,
    };
    let profile_name = active_id
        .and_then(|id| {
            model
                .profiles
                .profiles
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.as_str())
        })
        .or(model.dashboard.active_profile_name.as_deref())
        .unwrap_or("—");

    let text = vec![
        Line::from(vec![
            Span::styled("  Temperature: ", Style::default().bold()),
            Span::raw(&temp_str),
            Span::raw("    "),
            Span::styled("Frequency: ", Style::default().bold()),
            Span::raw(&freq_str),
            Span::raw("    "),
            Span::styled("Cores: ", Style::default().bold()),
            Span::raw(&cores_str),
        ]),
        Line::from(vec![
            Span::styled("  Power: ", Style::default().bold()),
            Span::raw(power_str),
            Span::raw("    "),
            Span::styled("Profile: ", Style::default().bold()),
            Span::raw(profile_name),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" CPU ");

    frame.render_widget(Paragraph::new(text).block(block), area);
}
