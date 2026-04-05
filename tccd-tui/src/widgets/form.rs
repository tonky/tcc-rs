use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

// ─── Field Value ────────────────────────────────────────────────────

/// A typed value that a form field operates on.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Text(String),
    Number(f64),
    Bool(bool),
    /// Index into the options list.
    Select(usize),
}

impl FieldValue {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FieldValue::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            FieldValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FieldValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_select(&self) -> Option<usize> {
        match self {
            FieldValue::Select(i) => Some(*i),
            _ => None,
        }
    }

    pub fn display(&self, options: &[String]) -> String {
        match self {
            FieldValue::Text(s) => s.clone(),
            FieldValue::Number(n) => {
                if *n == (*n as i64) as f64 {
                    format!("{}", *n as i64)
                } else {
                    format!("{:.1}", n)
                }
            }
            FieldValue::Bool(b) => if *b { "On" } else { "Off" }.into(),
            FieldValue::Select(i) => options
                .get(*i)
                .cloned()
                .unwrap_or_else(|| format!("#{}", i)),
        }
    }
}

// ─── Field Kind ─────────────────────────────────────────────────────

/// Describes how a field should behave and render.
#[derive(Debug, Clone)]
pub enum FieldKind {
    /// Free-text input.
    Text { max_len: usize },
    /// Numeric input with min/max/step.
    Number { min: f64, max: f64, step: f64 },
    /// Boolean toggle (Space/Enter to flip).
    Toggle,
    /// Dropdown/select from predefined options (Left/Right to cycle).
    Select { options: Vec<String> },
    /// Read-only display (section headers, computed fields).
    ReadOnly,
}

// ─── Form Field ─────────────────────────────────────────────────────

/// A single form field with its label, kind, and current value.
#[derive(Debug, Clone)]
pub struct FormField {
    pub label: String,
    pub kind: FieldKind,
    pub value: FieldValue,
    /// Whether this field's value has been modified.
    pub dirty: bool,
}

impl FormField {
    pub fn text(label: impl Into<String>, value: impl Into<String>, max_len: usize) -> Self {
        Self {
            label: label.into(),
            kind: FieldKind::Text { max_len },
            value: FieldValue::Text(value.into()),
            dirty: false,
        }
    }

    pub fn number(label: impl Into<String>, value: f64, min: f64, max: f64, step: f64) -> Self {
        Self {
            label: label.into(),
            kind: FieldKind::Number { min, max, step },
            value: FieldValue::Number(value),
            dirty: false,
        }
    }

    pub fn toggle(label: impl Into<String>, value: bool) -> Self {
        Self {
            label: label.into(),
            kind: FieldKind::Toggle,
            value: FieldValue::Bool(value),
            dirty: false,
        }
    }

    pub fn select(
        label: impl Into<String>,
        options: Vec<String>,
        selected: usize,
    ) -> Self {
        Self {
            label: label.into(),
            kind: FieldKind::Select { options },
            value: FieldValue::Select(selected),
            dirty: false,
        }
    }

    pub fn read_only(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: FieldKind::ReadOnly,
            value: FieldValue::Text(value.into()),
            dirty: false,
        }
    }

    pub fn section(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: FieldKind::ReadOnly,
            value: FieldValue::Text(String::new()),
            dirty: false,
        }
    }

    fn options(&self) -> &[String] {
        match &self.kind {
            FieldKind::Select { options } => options,
            _ => &[],
        }
    }

    /// Display string for the current value.
    pub fn display_value(&self) -> String {
        self.value.display(self.options())
    }

    /// Returns true if this field is editable.
    pub fn is_editable(&self) -> bool {
        !matches!(self.kind, FieldKind::ReadOnly)
    }

    /// Handle a key press on this field. Returns true if the value changed.
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match &self.kind {
            FieldKind::Text { max_len } => self.handle_text_key(code, *max_len),
            FieldKind::Number { min, max, step } => {
                let (min, max, step) = (*min, *max, *step);
                self.handle_number_key(code, min, max, step)
            }
            FieldKind::Toggle => self.handle_toggle_key(code),
            FieldKind::Select { options } => {
                let len = options.len();
                self.handle_select_key(code, len)
            }
            FieldKind::ReadOnly => false,
        }
    }

    fn handle_text_key(&mut self, code: KeyCode, max_len: usize) -> bool {
        match code {
            KeyCode::Char(c) => {
                if let FieldValue::Text(ref mut s) = self.value
                    && s.len() < max_len
                {
                    s.push(c);
                    self.dirty = true;
                    return true;
                }
                false
            }
            KeyCode::Backspace => {
                if let FieldValue::Text(ref mut s) = self.value
                    && s.pop().is_some()
                {
                    self.dirty = true;
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn handle_number_key(&mut self, code: KeyCode, min: f64, max: f64, step: f64) -> bool {
        if let FieldValue::Number(ref mut n) = self.value {
            match code {
                KeyCode::Right | KeyCode::Char('+') => {
                    let new = (*n + step).min(max);
                    if (new - *n).abs() > f64::EPSILON {
                        *n = new;
                        self.dirty = true;
                        return true;
                    }
                }
                KeyCode::Left | KeyCode::Char('-') => {
                    let new = (*n - step).max(min);
                    if (new - *n).abs() > f64::EPSILON {
                        *n = new;
                        self.dirty = true;
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn handle_toggle_key(&mut self, code: KeyCode) -> bool {
        if let FieldValue::Bool(ref mut b) = self.value {
            match code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    *b = !*b;
                    self.dirty = true;
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn handle_select_key(&mut self, code: KeyCode, option_count: usize) -> bool {
        if option_count == 0 {
            return false;
        }
        if let FieldValue::Select(ref mut idx) = self.value {
            match code {
                KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    *idx = (*idx + 1) % option_count;
                    self.dirty = true;
                    return true;
                }
                KeyCode::Left => {
                    *idx = if *idx == 0 {
                        option_count - 1
                    } else {
                        *idx - 1
                    };
                    self.dirty = true;
                    return true;
                }
                _ => {}
            }
        }
        false
    }
}

// ─── Form State ─────────────────────────────────────────────────────

/// A collection of form fields with a focused field index.
#[derive(Debug, Clone)]
pub struct FormState {
    pub fields: Vec<FormField>,
    pub focused: usize,
}

impl FormState {
    pub fn new(fields: Vec<FormField>) -> Self {
        Self { fields, focused: 0 }
    }

    /// Move focus to next editable field.
    pub fn focus_next(&mut self) {
        let len = self.fields.len();
        if len == 0 {
            return;
        }
        for i in 1..len {
            let idx = (self.focused + i) % len;
            if self.fields[idx].is_editable() {
                self.focused = idx;
                return;
            }
        }
    }

    /// Move focus to previous editable field.
    pub fn focus_prev(&mut self) {
        let len = self.fields.len();
        if len == 0 {
            return;
        }
        for i in 1..len {
            let idx = (self.focused + len - i) % len;
            if self.fields[idx].is_editable() {
                self.focused = idx;
                return;
            }
        }
    }

    /// Handle a key for the currently focused field. Returns true if value changed.
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Down | KeyCode::Tab => {
                self.focus_next();
                true
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.focus_prev();
                true
            }
            _ => {
                if let Some(field) = self.fields.get_mut(self.focused) {
                    field.handle_key(code)
                } else {
                    false
                }
            }
        }
    }

    /// Check if any field has been modified.
    pub fn is_dirty(&self) -> bool {
        self.fields.iter().any(|f| f.dirty)
    }

    /// Reset all dirty flags.
    #[allow(dead_code)]
    pub fn clear_dirty(&mut self) {
        for field in &mut self.fields {
            field.dirty = false;
        }
    }

    /// Find a field by label and return a reference.
    pub fn field_by_label(&self, label: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.label == label)
    }
}

// ─── Rendering ──────────────────────────────────────────────────────

/// Render a form field as a single line (label + value), suitable for a list layout.
pub fn render_form_row(field: &FormField, focused: bool) -> Line<'static> {
    let label_style = if matches!(field.kind, FieldKind::ReadOnly) && field.value == FieldValue::Text(String::new()) {
        // Section header
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let value_str = field.display_value();

    let value_style = if focused && field.is_editable() {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else if field.dirty {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };

    let indicator = if focused { "▸ " } else { "  " };

    let mut spans = vec![
        Span::raw(indicator.to_string()),
        Span::styled(format!("{:<24}", field.label), label_style),
    ];

    // Add type hint for focused editable fields
    if focused && field.is_editable() {
        let hint = match &field.kind {
            FieldKind::Text { .. } => " [type] ",
            FieldKind::Number { .. } => " [←→/+-] ",
            FieldKind::Toggle => " [Space] ",
            FieldKind::Select { .. } => " [←→] ",
            FieldKind::ReadOnly => "",
        };
        spans.push(Span::styled(value_str, value_style));
        spans.push(Span::styled(hint.to_string(), Style::default().fg(Color::DarkGray)));
    } else {
        spans.push(Span::styled(value_str, value_style));
    }

    Line::from(spans)
}

/// Render a complete form as a Paragraph widget.
pub fn render_form(state: &FormState, area: Rect, frame: &mut Frame) {
    let lines: Vec<Line> = state
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| render_form_row(field, i == state.focused))
        .collect();

    let widget = Paragraph::new(lines);
    frame.render_widget(widget, area);
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_field_input() {
        let mut field = FormField::text("Name", "Hello", 20);
        assert!(field.handle_key(KeyCode::Char('!')));
        assert_eq!(field.value, FieldValue::Text("Hello!".into()));
        assert!(field.dirty);

        assert!(field.handle_key(KeyCode::Backspace));
        assert_eq!(field.value, FieldValue::Text("Hello".into()));
    }

    #[test]
    fn text_field_max_len() {
        let mut field = FormField::text("Short", "ab", 3);
        assert!(field.handle_key(KeyCode::Char('c')));
        assert!(!field.handle_key(KeyCode::Char('d'))); // at max
        assert_eq!(field.value, FieldValue::Text("abc".into()));
    }

    #[test]
    fn number_field_increment_decrement() {
        let mut field = FormField::number("Speed", 50.0, 0.0, 100.0, 5.0);
        assert!(field.handle_key(KeyCode::Right));
        assert_eq!(field.value, FieldValue::Number(55.0));
        assert!(field.handle_key(KeyCode::Left));
        assert_eq!(field.value, FieldValue::Number(50.0));
    }

    #[test]
    fn number_field_clamps() {
        let mut field = FormField::number("Value", 98.0, 0.0, 100.0, 5.0);
        assert!(field.handle_key(KeyCode::Right));
        assert_eq!(field.value, FieldValue::Number(100.0));
        assert!(!field.handle_key(KeyCode::Right)); // already at max

        let mut field2 = FormField::number("Value", 2.0, 0.0, 100.0, 5.0);
        assert!(field2.handle_key(KeyCode::Left));
        assert_eq!(field2.value, FieldValue::Number(0.0));
        assert!(!field2.handle_key(KeyCode::Left)); // already at min
    }

    #[test]
    fn toggle_field() {
        let mut field = FormField::toggle("Turbo", false);
        assert!(field.handle_key(KeyCode::Char(' ')));
        assert_eq!(field.value, FieldValue::Bool(true));
        assert!(field.handle_key(KeyCode::Enter));
        assert_eq!(field.value, FieldValue::Bool(false));
    }

    #[test]
    fn select_field_cycles() {
        let options = vec!["A".into(), "B".into(), "C".into()];
        let mut field = FormField::select("Mode", options, 0);
        assert!(field.handle_key(KeyCode::Right));
        assert_eq!(field.value, FieldValue::Select(1));
        assert!(field.handle_key(KeyCode::Right));
        assert_eq!(field.value, FieldValue::Select(2));
        assert!(field.handle_key(KeyCode::Right));
        assert_eq!(field.value, FieldValue::Select(0)); // wraps

        assert!(field.handle_key(KeyCode::Left));
        assert_eq!(field.value, FieldValue::Select(2)); // wraps back
    }

    #[test]
    fn read_only_ignores_input() {
        let mut field = FormField::read_only("ID", "abc123");
        assert!(!field.handle_key(KeyCode::Char('x')));
        assert!(!field.handle_key(KeyCode::Enter));
        assert!(!field.is_editable());
    }

    #[test]
    fn form_state_navigation() {
        let fields = vec![
            FormField::section("── Header ──"),
            FormField::text("Name", "Test", 50),
            FormField::read_only("ID", "abc"),
            FormField::toggle("Enable", true),
        ];
        let mut form = FormState::new(fields);
        assert_eq!(form.focused, 0);

        form.focus_next(); // skip read-only header → Name
        assert_eq!(form.focused, 1);

        form.focus_next(); // skip read-only ID → Enable
        assert_eq!(form.focused, 3);

        form.focus_next(); // wrap → Name
        assert_eq!(form.focused, 1);

        form.focus_prev(); // wrap back → Enable
        assert_eq!(form.focused, 3);
    }

    #[test]
    fn form_state_dirty_tracking() {
        let fields = vec![
            FormField::text("Name", "Test", 50),
            FormField::number("Speed", 50.0, 0.0, 100.0, 5.0),
        ];
        let mut form = FormState::new(fields);
        assert!(!form.is_dirty());

        form.handle_key(KeyCode::Char('!'));
        assert!(form.is_dirty());

        form.clear_dirty();
        assert!(!form.is_dirty());
    }

    #[test]
    fn form_handles_tab_navigation() {
        let fields = vec![
            FormField::text("A", "", 10),
            FormField::text("B", "", 10),
        ];
        let mut form = FormState::new(fields);
        assert_eq!(form.focused, 0);

        form.handle_key(KeyCode::Down);
        assert_eq!(form.focused, 1);

        form.handle_key(KeyCode::Up);
        assert_eq!(form.focused, 0);
    }

    #[test]
    fn display_value_number_formatting() {
        let int_field = FormField::number("Int", 42.0, 0.0, 100.0, 1.0);
        assert_eq!(int_field.display_value(), "42");

        let float_field = FormField::number("Float", 42.5, 0.0, 100.0, 0.5);
        assert_eq!(float_field.display_value(), "42.5");
    }
}
