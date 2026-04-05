use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};

/// Data received from the daemon via D-Bus polling.
#[derive(Debug, Clone)]
pub enum DataUpdate {
    FanData(FanState),
    CpuMetrics(CpuState),
    PowerState {
        on_ac: bool,
    },
    FanCurveData {
        profile_name: String,
        fan_profile: String,
        curve_cpu: Vec<(f64, f64)>,
    },
    ConnectionLost,
    ConnectionRestored,
    ActionResult {
        action: String,
        result: Result<(), String>,
    },
    ProfileList(Vec<ProfileSummary>),
    ProfileDetail(String), // full JSON
    ProfileAssignments {
        ac: Option<String>,
        bat: Option<String>,
    },
    /// Global settings JSON from daemon.
    SettingsData(String),
    /// Keyboard backlight state JSON from daemon.
    KeyboardData(String),
    /// Charging settings JSON from daemon.
    ChargingData(String),
    /// GPU info JSON from daemon.
    GpuData(String),
    /// Power settings JSON (PRIME mode, TGP, shutdown).
    PowerData(String),
    /// Display settings JSON from daemon.
    DisplayData(String),
    /// Webcam device list JSON from daemon.
    WebcamDevices(String),
    /// Webcam controls JSON for a specific device.
    WebcamControls(String),
    /// System info JSON from daemon.
    SystemInfo(String),
    /// Hardware capabilities JSON from daemon.
    Capabilities(String),
}

#[derive(Debug, Clone, Default)]
pub struct FanState {
    pub speeds_percent: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct CpuState {
    pub temperature: Option<f64>,
    pub avg_frequency_mhz: Option<f64>,
    pub core_count: Option<usize>,
}

/// Lightweight profile summary for the list view (deserialized from daemon JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// All messages that flow through the TEA update loop.
#[derive(Debug)]
pub enum Msg {
    Key(KeyEvent),
    #[allow(dead_code)]
    Resize(u16, u16),
    #[allow(dead_code)]
    Tick,
    Data(DataUpdate),
}
