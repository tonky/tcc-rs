use std::collections::VecDeque;
use std::time::Duration;

use crate::msg::{CpuState, FanState, ProfileSummary};
use crate::widgets::form::FormState;

/// Which tab is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Profiles,
    FanCurve,
    Settings,
    Keyboard,
    Charging,
    Power,
    Display,
    Webcam,
    Info,
}

impl Tab {
    pub const ALL: &[Tab] = &[
        Tab::Dashboard,
        Tab::Profiles,
        Tab::FanCurve,
        Tab::Settings,
        Tab::Keyboard,
        Tab::Charging,
        Tab::Power,
        Tab::Display,
        Tab::Webcam,
        Tab::Info,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Profiles => "Profiles",
            Tab::FanCurve => "Fan Curve",
            Tab::Settings => "Settings",
            Tab::Keyboard => "Keyboard",
            Tab::Charging => "Charging",
            Tab::Power => "Power",
            Tab::Display => "Display",
            Tab::Webcam => "Webcam",
            Tab::Info => "Info",
        }
    }
}

/// D-Bus connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
}

/// Poll interval configuration.
#[derive(Debug, Clone)]
pub struct PollConfig {
    pub fast: Duration,
    pub medium: Duration,
    #[allow(dead_code)]
    pub slow: Duration,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            fast: Duration::from_secs(1),
            medium: Duration::from_secs(5),
            slow: Duration::from_secs(20),
        }
    }
}

/// A status bar notification.
#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub is_error: bool,
}

/// Dashboard telemetry state — holds current + recent history.
#[derive(Debug, Clone, Default)]
pub struct DashboardState {
    pub fan: FanState,
    pub cpu: CpuState,
    pub fan_speed_history: VecDeque<f64>,
    pub cpu_temp_history: VecDeque<f64>,
    pub power_on_ac: Option<bool>,
    pub active_profile_name: Option<String>,
}

impl DashboardState {
    const MAX_HISTORY: usize = 60;

    pub fn push_fan_speed(&mut self, speed: f64) {
        if self.fan_speed_history.len() >= Self::MAX_HISTORY {
            self.fan_speed_history.pop_front();
        }
        self.fan_speed_history.push_back(speed);
    }

    pub fn push_cpu_temp(&mut self, temp: f64) {
        if self.cpu_temp_history.len() >= Self::MAX_HISTORY {
            self.cpu_temp_history.pop_front();
        }
        self.cpu_temp_history.push_back(temp);
    }
}

/// Single source of truth for the entire TUI.
#[derive(Debug, Clone)]
pub struct Model {
    pub active_tab: Tab,
    pub dashboard: DashboardState,
    pub profiles: ProfilesState,
    pub fan_curve: FanCurveState,
    pub settings: SettingsState,
    pub keyboard: KeyboardState,
    pub charging: ChargingState,
    pub power: PowerTabState,
    pub display: DisplayTabState,
    pub webcam: WebcamTabState,
    pub info: InfoState,
    pub help_visible: bool,
    #[allow(dead_code)]
    pub no_color: bool,
    pub notifications: VecDeque<Notification>, // capped at MAX_NOTIFICATIONS
    pub poll_config: PollConfig,
    pub connection_status: ConnectionStatus,
    pub dirty: bool,
    pub should_quit: bool,
}

impl Model {
    const MAX_NOTIFICATIONS: usize = 32;

    pub fn push_notification(&mut self, notification: Notification) {
        if self.notifications.len() >= Self::MAX_NOTIFICATIONS {
            self.notifications.pop_front();
        }
        self.notifications.push_back(notification);
    }
}

impl Default for Model {
    fn default() -> Self {
        Self {
            active_tab: Tab::Dashboard,
            dashboard: DashboardState::default(),
            profiles: ProfilesState::default(),
            fan_curve: FanCurveState::default(),
            settings: SettingsState::default(),
            keyboard: KeyboardState::default(),
            charging: ChargingState::default(),
            power: PowerTabState::default(),
            display: DisplayTabState::default(),
            webcam: WebcamTabState::default(),
            info: InfoState::default(),
            help_visible: false,
            no_color: std::env::var("NO_COLOR").is_ok(),
            notifications: VecDeque::new(),
            poll_config: PollConfig::default(),
            connection_status: ConnectionStatus::Disconnected,
            dirty: true,
            should_quit: false,
        }
    }
}

// ─── Profiles state ─────────────────────────────────────────────────

/// Sub-view the profiles tab can be in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum ProfileView {
    #[default]
    List,
    Editor { profile_id: String },
}

/// State for the profiles tab.
#[derive(Debug, Clone, Default)]
pub struct ProfilesState {
    pub profiles: Vec<ProfileSummary>,
    pub selected_index: usize,
    pub view: ProfileView,
    pub ac_profile_id: Option<String>,
    pub bat_profile_id: Option<String>,
    /// Serialized JSON of the profile being edited (full profile data).
    pub editing_json: Option<String>,
    /// Form state for the profile editor.
    pub editor_form: Option<FormState>,
    /// Dirty flag — editor has unsaved changes.
    pub editor_dirty: bool,
}

// ─── Fan curve state ────────────────────────────────────────────────

/// State for the fan curve visualization and editing tab.
#[derive(Debug, Clone, Default)]
pub struct FanCurveState {
    pub curve_points: Vec<(f64, f64)>,
    pub original_points: Vec<(f64, f64)>,
    pub selected_point: usize,
    pub fan_profile_name: String,
}

impl FanCurveState {
    pub fn is_dirty(&self) -> bool {
        self.curve_points != self.original_points
    }
}

// ─── Settings state ─────────────────────────────────────────────────

/// State for the global settings tab.
#[derive(Debug, Clone, Default)]
pub struct SettingsState {
    pub form: Option<FormState>,
    pub loaded: bool,
}

// ─── Keyboard backlight state ───────────────────────────────────────

/// State for the keyboard backlight tab.
#[derive(Debug, Clone, Default)]
pub struct KeyboardState {
    pub form: Option<FormState>,
    pub loaded: bool,
}

// ─── Charging state ─────────────────────────────────────────────────

/// State for the charging settings tab.
#[derive(Debug, Clone, Default)]
pub struct ChargingState {
    pub form: Option<FormState>,
    pub loaded: bool,
}

// ─── Power tab state ────────────────────────────────────────────────

/// State for the power/GPU tab.
#[derive(Debug, Clone, Default)]
pub struct PowerTabState {
    pub form: Option<FormState>,
    pub gpu_info: Option<GpuInfo>,
    pub loaded: bool,
}

/// GPU metrics shown in dashboard and power tab.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct GpuInfo {
    pub dgpu_name: String,
    pub dgpu_temp: Option<f64>,
    pub dgpu_usage: Option<f64>,
    pub dgpu_power_draw: Option<f64>,
    pub igpu_name: String,
    pub igpu_usage: Option<f64>,
    pub prime_mode: String,
    pub tgp_offset: Option<f64>,
}

// ─── Display tab state ──────────────────────────────────────────────

/// State for the display settings tab.
#[derive(Debug, Clone, Default)]
pub struct DisplayTabState {
    pub form: Option<FormState>,
    pub original_json: Option<String>,
    pub loaded: bool,
}

// ─── Webcam tab state ───────────────────────────────────────────────

/// State for the webcam controls tab.
#[derive(Debug, Clone, Default)]
pub struct WebcamTabState {
    pub form: Option<FormState>,
    pub devices: Vec<WebcamDevice>,
    pub selected_device: usize,
    pub loaded: bool,
}

/// A detected webcam device.
#[derive(Debug, Clone)]
pub struct WebcamDevice {
    pub path: String,
    pub name: String,
}

// ─── Info tab state ─────────────────────────────────────────────────

/// State for the system info tab.
#[derive(Debug, Clone, Default)]
pub struct InfoState {
    pub tcc_version: Option<String>,
    pub daemon_version: Option<String>,
    pub hostname: Option<String>,
    pub kernel_version: Option<String>,
    pub loaded: bool,
}

