use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Render the help overlay as a centered modal on top of the current view.
pub fn render_help(frame: &mut Frame) {
    let area = centered_rect(60, 70, frame.area());

    // Clear the area under the popup
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help — Keybindings ")
        .style(Style::default().bg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        section("Navigation"),
        binding("1-9, 0", "Switch tabs"),
        binding("?", "Toggle this help"),
        binding("q", "Quit (or close editor)"),
        binding("Ctrl+C", "Force quit"),
        Line::from(""),
        section("Profile List"),
        binding("↑/↓ j/k", "Navigate profiles"),
        binding("Enter", "Edit profile"),
        binding("c", "Copy profile"),
        binding("d", "Delete profile"),
        binding("a/b", "Assign to AC/Battery"),
        binding("Esc", "Back to list"),
        Line::from(""),
        section("Fan Curve"),
        binding("←/→", "Select curve point"),
        binding("↑/↓", "Adjust speed ±5%"),
        binding("i", "Insert point after selected"),
        binding("x", "Delete selected (not first/last)"),
        binding("r", "Reset to default 5-point curve"),
        binding("s", "Save curve"),
        binding("Esc", "Discard changes"),
        Line::from(""),
        section("Form Tabs (Settings, Keyboard, etc.)"),
        binding("↑/↓", "Navigate fields"),
        binding("←/→ Space", "Edit field value"),
        binding("s", "Save changes"),
        binding("Esc", "Discard changes"),
        Line::from(""),
        section("Webcam"),
        binding("Tab/Shift+Tab", "Switch device"),
        Line::from(""),
        Line::from(Span::styled(
            "  Press ? or Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn section(title: &str) -> Line<'_> {
    Line::from(Span::styled(
        format!("  ── {} ──", title),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn binding<'a>(keys: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:16}", keys), Style::default().fg(Color::Yellow)),
        Span::raw(desc),
    ])
}

/// Calculate a centered rectangle of the given percentage width and height.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [vy] = vertical.areas(area);
    let [hx] = horizontal.areas(vy);
    hx
}
