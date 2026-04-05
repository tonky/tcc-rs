/// Side effects returned from update(). Dispatched by the runtime.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Command {
    Quit,
    #[allow(dead_code)]
    SetFanSpeed(u8),
    #[allow(dead_code)]
    None,
    // Profile commands
    FetchProfiles,
    FetchProfileDetail(String),
    CopyProfile(String),
    DeleteProfile(String),
    SaveProfile { id: String, json: String },
    SetActiveProfile { id: String, state: String },
    FetchAssignments,
    FetchActiveFanCurve,
    SaveFanCurve(String),
    // Settings commands
    FetchSettings,
    SaveSettings(String),
    // Keyboard commands
    FetchKeyboard,
    SaveKeyboard(String),
    // Charging commands
    FetchCharging,
    SaveCharging(String),
    // Power/GPU commands
    FetchGpuInfo,
    FetchPowerSettings,
    SavePowerSettings(String),
    ScheduleShutdown { hours: u32, minutes: u32 },
    CancelShutdown,
    // Display commands
    FetchDisplay,
    SaveDisplay(String),
    // Webcam commands
    FetchWebcamDevices,
    FetchWebcamControls(String), // device path
    SaveWebcamControls { device: String, json: String },
    // Info commands
    FetchSystemInfo,
    // Capabilities
    FetchCapabilities,
}
