use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ─── Profile Data Types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TccProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub display: DisplaySettings,
    pub cpu: CpuSettings,
    pub webcam: WebcamSettings,
    pub fan: FanControlSettings,
    #[serde(rename = "odmProfile")]
    pub odm_profile: OdmProfile,
    #[serde(rename = "odmPowerLimits")]
    pub odm_power_limits: OdmPowerLimits,
    #[serde(rename = "nvidiaPowerCTRLProfile")]
    pub nvidia_power_ctrl: NvidiaPowerCtrl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplaySettings {
    pub brightness: i32,
    #[serde(rename = "useBrightness")]
    pub use_brightness: bool,
    #[serde(rename = "refreshRate")]
    pub refresh_rate: i32,
    #[serde(rename = "useRefRate")]
    pub use_ref_rate: bool,
    #[serde(rename = "xResolution")]
    pub x_resolution: i32,
    #[serde(rename = "yResolution")]
    pub y_resolution: i32,
    #[serde(rename = "useResolution")]
    pub use_resolution: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CpuSettings {
    #[serde(rename = "onlineCores")]
    pub online_cores: Option<u32>,
    #[serde(rename = "useMaxPerfGov")]
    pub use_max_perf_gov: bool,
    #[serde(rename = "scalingMinFrequency")]
    pub scaling_min_frequency: Option<u64>,
    #[serde(rename = "scalingMaxFrequency")]
    pub scaling_max_frequency: Option<u64>,
    pub governor: String,
    #[serde(rename = "energyPerformancePreference")]
    pub energy_performance_preference: String,
    #[serde(rename = "noTurbo")]
    pub no_turbo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebcamSettings {
    pub status: bool,
    #[serde(rename = "useStatus")]
    pub use_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanControlSettings {
    #[serde(rename = "useControl")]
    pub use_control: bool,
    #[serde(rename = "fanProfile")]
    pub fan_profile: String,
    #[serde(rename = "minimumFanspeed")]
    pub minimum_fanspeed: u8,
    #[serde(rename = "maximumFanspeed")]
    pub maximum_fanspeed: u8,
    #[serde(rename = "offsetFanspeed")]
    pub offset_fanspeed: i8,
    #[serde(rename = "customFanCurve")]
    pub custom_fan_curve: FanProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanProfile {
    pub name: Option<String>,
    #[serde(rename = "tableCPU")]
    pub table_cpu: Option<Vec<FanTableEntry>>,
    #[serde(rename = "tableGPU")]
    pub table_gpu: Option<Vec<FanTableEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FanTableEntry {
    pub temp: u8,
    pub speed: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OdmProfile {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OdmPowerLimits {
    #[serde(rename = "tdpValues")]
    pub tdp_values: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NvidiaPowerCtrl {
    #[serde(rename = "cTGPOffset")]
    pub ctgp_offset: i32,
}

// ─── Settings (profile assignments + global config) ─────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PowerState {
    #[serde(rename = "power_ac")]
    Ac,
    #[serde(rename = "power_bat")]
    Battery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TccSettings {
    pub fahrenheit: bool,
    #[serde(rename = "stateMap")]
    pub state_map: HashMap<PowerState, String>,
    #[serde(rename = "cpuSettingsEnabled")]
    pub cpu_settings_enabled: bool,
    #[serde(rename = "fanControlEnabled")]
    pub fan_control_enabled: bool,
}

impl Default for TccSettings {
    fn default() -> Self {
        let mut state_map = HashMap::new();
        state_map.insert(
            PowerState::Ac,
            DEFAULT_CUSTOM_PROFILE_ID.to_string(),
        );
        state_map.insert(
            PowerState::Battery,
            DEFAULT_CUSTOM_PROFILE_ID.to_string(),
        );
        Self {
            fahrenheit: false,
            state_map,
            cpu_settings_enabled: true,
            fan_control_enabled: true,
        }
    }
}

// ─── Keyboard Backlight State ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardBacklightState {
    pub brightness: f64,
    pub color: String,
    pub mode: String,
}

impl Default for KeyboardBacklightState {
    fn default() -> Self {
        Self {
            brightness: 50.0,
            color: "#ffffff".into(),
            mode: "Static".into(),
        }
    }
}

// ─── Charging Settings ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargingSettings {
    #[serde(rename = "chargingProfile")]
    pub charging_profile: String,
    #[serde(rename = "chargingPriority")]
    pub charging_priority: String,
    #[serde(rename = "startThreshold")]
    pub start_threshold: f64,
    #[serde(rename = "endThreshold")]
    pub end_threshold: f64,
}

impl Default for ChargingSettings {
    fn default() -> Self {
        Self {
            charging_profile: "Full Capacity".into(),
            charging_priority: "Battery".into(),
            start_threshold: 80.0,
            end_threshold: 100.0,
        }
    }
}

// ─── GPU / Power types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfoData {
    #[serde(rename = "dgpuName")]
    pub dgpu_name: String,
    #[serde(rename = "dgpuTemp")]
    pub dgpu_temp: Option<f64>,
    #[serde(rename = "dgpuUsage")]
    pub dgpu_usage: Option<f64>,
    #[serde(rename = "dgpuPowerDraw")]
    pub dgpu_power_draw: Option<f64>,
    #[serde(rename = "igpuName")]
    pub igpu_name: String,
    #[serde(rename = "igpuUsage")]
    pub igpu_usage: Option<f64>,
    #[serde(rename = "primeMode")]
    pub prime_mode: String,
    #[serde(rename = "tgpOffset")]
    pub tgp_offset: Option<f64>,
}

impl Default for GpuInfoData {
    fn default() -> Self {
        Self {
            dgpu_name: "NVIDIA GeForce RTX 3060".into(),
            dgpu_temp: Some(45.0),
            dgpu_usage: Some(5.0),
            dgpu_power_draw: Some(15.0),
            igpu_name: "Intel UHD Graphics".into(),
            igpu_usage: Some(10.0),
            prime_mode: "on-demand".into(),
            tgp_offset: Some(0.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSettings {
    #[serde(rename = "primeMode")]
    pub prime_mode: String,
    #[serde(rename = "tgpOffset")]
    pub tgp_offset: f64,
    #[serde(rename = "shutdownHours")]
    pub shutdown_hours: u32,
    #[serde(rename = "shutdownMinutes")]
    pub shutdown_minutes: u32,
    #[serde(rename = "shutdownActive")]
    pub shutdown_active: bool,
}

impl Default for PowerSettings {
    fn default() -> Self {
        Self {
            prime_mode: "on-demand".into(),
            tgp_offset: 0.0,
            shutdown_hours: 0,
            shutdown_minutes: 0,
            shutdown_active: false,
        }
    }
}

// ─── Display types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayModes {
    pub brightness: f64,
    #[serde(rename = "refreshRates")]
    pub refresh_rates: Vec<u32>,
    #[serde(rename = "selectedRefreshRate")]
    pub selected_refresh_rate: u32,
    pub resolutions: Vec<String>,
    #[serde(rename = "selectedResolution")]
    pub selected_resolution: String,
    #[serde(rename = "ycbcr")]
    pub ycbcr: bool,
}

impl Default for DisplayModes {
    fn default() -> Self {
        Self {
            brightness: 80.0,
            refresh_rates: vec![60, 120, 144, 165],
            selected_refresh_rate: 60,
            resolutions: vec!["1920x1080".into(), "2560x1440".into(), "3840x2160".into()],
            selected_resolution: "1920x1080".into(),
            ycbcr: false,
        }
    }
}

// ─── Webcam types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebcamDeviceInfo {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebcamControls {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub sharpness: f64,
    #[serde(rename = "autoExposure")]
    pub auto_exposure: bool,
    pub exposure: f64,
    #[serde(rename = "autoWhiteBalance")]
    pub auto_white_balance: bool,
    #[serde(rename = "whiteBalance")]
    pub white_balance: f64,
}

impl Default for WebcamControls {
    fn default() -> Self {
        Self {
            brightness: 128.0,
            contrast: 128.0,
            saturation: 128.0,
            sharpness: 128.0,
            auto_exposure: true,
            exposure: 500.0,
            auto_white_balance: true,
            white_balance: 4500.0,
        }
    }
}

// ─── Default Profile IDs ────────────────────────────────────────────

pub const PROFILE_MAX_ENERGY_SAVE: &str = "__profile_max_energy_save__";
pub const PROFILE_QUIET: &str = "__profile_silent__";
pub const PROFILE_OFFICE: &str = "__office__";
pub const PROFILE_HIGH_PERFORMANCE: &str = "__high_performance__";
pub const DEFAULT_CUSTOM_PROFILE_ID: &str = "__default_custom_profile__";

// ─── Default Profiles ───────────────────────────────────────────────

fn default_fan_curve() -> FanProfile {
    FanProfile {
        name: Some("Custom".into()),
        table_cpu: Some(vec![
            FanTableEntry { temp: 20, speed: 12 },
            FanTableEntry { temp: 30, speed: 14 },
            FanTableEntry { temp: 40, speed: 22 },
            FanTableEntry { temp: 50, speed: 35 },
            FanTableEntry { temp: 60, speed: 44 },
            FanTableEntry { temp: 70, speed: 56 },
            FanTableEntry { temp: 80, speed: 79 },
            FanTableEntry { temp: 90, speed: 85 },
            FanTableEntry {
                temp: 100,
                speed: 90,
            },
        ]),
        table_gpu: None,
    }
}

pub fn default_profiles() -> Vec<TccProfile> {
    vec![
        TccProfile {
            id: PROFILE_MAX_ENERGY_SAVE.into(),
            name: "Max Energy Save".into(),
            description: "Minimal power consumption".into(),
            display: DisplaySettings {
                brightness: 40,
                use_brightness: true,
                refresh_rate: -1,
                use_ref_rate: false,
                x_resolution: -1,
                y_resolution: -1,
                use_resolution: false,
            },
            cpu: CpuSettings {
                online_cores: None,
                use_max_perf_gov: false,
                scaling_min_frequency: None,
                scaling_max_frequency: None,
                governor: "powersave".into(),
                energy_performance_preference: "power".into(),
                no_turbo: true,
            },
            webcam: WebcamSettings {
                status: true,
                use_status: false,
            },
            fan: FanControlSettings {
                use_control: true,
                fan_profile: "Silent".into(),
                minimum_fanspeed: 0,
                maximum_fanspeed: 100,
                offset_fanspeed: 0,
                custom_fan_curve: default_fan_curve(),
            },
            odm_profile: OdmProfile {
                name: Some("power_save".into()),
            },
            odm_power_limits: OdmPowerLimits {
                tdp_values: vec![5, 10, 15],
            },
            nvidia_power_ctrl: NvidiaPowerCtrl { ctgp_offset: 0 },
        },
        TccProfile {
            id: PROFILE_QUIET.into(),
            name: "Quiet".into(),
            description: "Low noise operation".into(),
            display: DisplaySettings {
                brightness: 50,
                use_brightness: true,
                refresh_rate: -1,
                use_ref_rate: false,
                x_resolution: -1,
                y_resolution: -1,
                use_resolution: false,
            },
            cpu: CpuSettings {
                online_cores: None,
                use_max_perf_gov: false,
                scaling_min_frequency: None,
                scaling_max_frequency: None,
                governor: "powersave".into(),
                energy_performance_preference: "balance_power".into(),
                no_turbo: true,
            },
            webcam: WebcamSettings {
                status: true,
                use_status: false,
            },
            fan: FanControlSettings {
                use_control: true,
                fan_profile: "Silent".into(),
                minimum_fanspeed: 0,
                maximum_fanspeed: 100,
                offset_fanspeed: 0,
                custom_fan_curve: default_fan_curve(),
            },
            odm_profile: OdmProfile {
                name: Some("power_save".into()),
            },
            odm_power_limits: OdmPowerLimits {
                tdp_values: vec![10, 15, 25],
            },
            nvidia_power_ctrl: NvidiaPowerCtrl { ctgp_offset: 0 },
        },
        TccProfile {
            id: PROFILE_OFFICE.into(),
            name: "Office".into(),
            description: "Balanced for everyday use".into(),
            display: DisplaySettings {
                brightness: 60,
                use_brightness: true,
                refresh_rate: -1,
                use_ref_rate: false,
                x_resolution: -1,
                y_resolution: -1,
                use_resolution: false,
            },
            cpu: CpuSettings {
                online_cores: None,
                use_max_perf_gov: false,
                scaling_min_frequency: None,
                scaling_max_frequency: None,
                governor: "powersave".into(),
                energy_performance_preference: "balance_performance".into(),
                no_turbo: false,
            },
            webcam: WebcamSettings {
                status: true,
                use_status: false,
            },
            fan: FanControlSettings {
                use_control: true,
                fan_profile: "Quiet".into(),
                minimum_fanspeed: 0,
                maximum_fanspeed: 100,
                offset_fanspeed: 0,
                custom_fan_curve: default_fan_curve(),
            },
            odm_profile: OdmProfile {
                name: Some("enthusiast".into()),
            },
            odm_power_limits: OdmPowerLimits {
                tdp_values: vec![25, 35, 35],
            },
            nvidia_power_ctrl: NvidiaPowerCtrl { ctgp_offset: 0 },
        },
        TccProfile {
            id: PROFILE_HIGH_PERFORMANCE.into(),
            name: "High Performance".into(),
            description: "Maximum performance".into(),
            display: DisplaySettings {
                brightness: 60,
                use_brightness: true,
                refresh_rate: -1,
                use_ref_rate: false,
                x_resolution: -1,
                y_resolution: -1,
                use_resolution: false,
            },
            cpu: CpuSettings {
                online_cores: None,
                use_max_perf_gov: true,
                scaling_min_frequency: None,
                scaling_max_frequency: None,
                governor: "performance".into(),
                energy_performance_preference: "performance".into(),
                no_turbo: false,
            },
            webcam: WebcamSettings {
                status: true,
                use_status: false,
            },
            fan: FanControlSettings {
                use_control: true,
                fan_profile: "Balanced".into(),
                minimum_fanspeed: 0,
                maximum_fanspeed: 100,
                offset_fanspeed: 0,
                custom_fan_curve: default_fan_curve(),
            },
            odm_profile: OdmProfile {
                name: Some("overboost".into()),
            },
            odm_power_limits: OdmPowerLimits {
                tdp_values: vec![60, 60, 70],
            },
            nvidia_power_ctrl: NvidiaPowerCtrl { ctgp_offset: 0 },
        },
    ]
}

// ─── Profile Store ──────────────────────────────────────────────────

const DEFAULT_IDS: &[&str] = &[
    PROFILE_MAX_ENERGY_SAVE,
    PROFILE_QUIET,
    PROFILE_OFFICE,
    PROFILE_HIGH_PERFORMANCE,
];

#[derive(Debug)]
pub struct ProfileStore {
    profiles: Vec<TccProfile>,
    settings: TccSettings,
    keyboard_state: KeyboardBacklightState,
    charging_settings: ChargingSettings,
    gpu_info: GpuInfoData,
    power_settings: PowerSettings,
    display_modes: DisplayModes,
    webcam_controls: WebcamControls,
    profiles_path: PathBuf,
    settings_path: PathBuf,
}

impl ProfileStore {
    pub fn new(config_dir: &Path) -> Self {
        let profiles_path = config_dir.join("profiles.json");
        let settings_path = config_dir.join("settings.json");

        let mut store = Self {
            profiles: default_profiles(),
            settings: TccSettings::default(),
            keyboard_state: KeyboardBacklightState::default(),
            charging_settings: ChargingSettings::default(),
            gpu_info: GpuInfoData::default(),
            power_settings: PowerSettings::default(),
            display_modes: DisplayModes::default(),
            webcam_controls: WebcamControls::default(),
            profiles_path,
            settings_path,
        };
        store.load();
        store
    }

    fn load(&mut self) {
        // Load custom profiles from disk, merge with defaults
        if let Ok(data) = fs::read_to_string(&self.profiles_path)
            && let Ok(saved) = serde_json::from_str::<Vec<TccProfile>>(&data) {
                // Keep defaults, add any custom profiles
                for profile in saved {
                    if !DEFAULT_IDS.contains(&profile.id.as_str()) {
                        self.profiles.push(profile);
                    }
                }
            }

        if let Ok(data) = fs::read_to_string(&self.settings_path)
            && let Ok(settings) = serde_json::from_str::<TccSettings>(&data) {
                self.settings = settings;
            }
    }

    fn save_profiles(&self) -> Result<(), std::io::Error> {
        let custom: Vec<&TccProfile> = self
            .profiles
            .iter()
            .filter(|p| !DEFAULT_IDS.contains(&p.id.as_str()))
            .collect();

        if let Some(parent) = self.profiles_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&custom)?;
        fs::write(&self.profiles_path, json)
    }

    fn save_settings(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.settings)?;
        fs::write(&self.settings_path, json)
    }

    // ─── Query ──────────────────────────────────────────────────────

    pub fn list_profiles(&self) -> &[TccProfile] {
        &self.profiles
    }

    pub fn get_profile(&self, id: &str) -> Option<&TccProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn is_default(id: &str) -> bool {
        DEFAULT_IDS.contains(&id)
    }

    pub fn get_settings(&self) -> &TccSettings {
        &self.settings
    }

    pub fn active_profile_id(&self, state: PowerState) -> Option<&str> {
        self.settings.state_map.get(&state).map(|s| s.as_str())
    }

    pub fn get_keyboard_state(&self) -> &KeyboardBacklightState {
        &self.keyboard_state
    }

    pub fn get_charging_settings(&self) -> &ChargingSettings {
        &self.charging_settings
    }

    // ─── Mutation ───────────────────────────────────────────────────

    pub fn create_profile(&mut self, mut profile: TccProfile) -> Result<String, String> {
        if profile.id.is_empty() {
            profile.id = generate_profile_id();
        }
        if self.profiles.iter().any(|p| p.id == profile.id) {
            return Err(format!("Profile ID '{}' already exists", profile.id));
        }
        let id = profile.id.clone();
        self.profiles.push(profile);
        self.save_profiles().map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub fn update_profile(&mut self, id: &str, updated: TccProfile) -> Result<(), String> {
        if Self::is_default(id) {
            return Err("Cannot modify default profiles".into());
        }
        let pos = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| format!("Profile '{}' not found", id))?;
        self.profiles[pos] = updated;
        self.save_profiles().map_err(|e| e.to_string())
    }

    pub fn delete_profile(&mut self, id: &str) -> Result<(), String> {
        if Self::is_default(id) {
            return Err("Cannot delete default profiles".into());
        }
        let pos = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| format!("Profile '{}' not found", id))?;
        self.profiles.remove(pos);

        // Clean up state_map references
        for (_state, profile_id) in self.settings.state_map.iter_mut() {
            if profile_id == id {
                *profile_id = DEFAULT_CUSTOM_PROFILE_ID.to_string();
            }
        }

        self.save_profiles().map_err(|e| e.to_string())?;
        self.save_settings().map_err(|e| e.to_string())
    }

    pub fn copy_profile(&mut self, id: &str) -> Result<String, String> {
        let source = self
            .get_profile(id)
            .ok_or_else(|| format!("Profile '{}' not found", id))?
            .clone();

        let mut copy = source;
        copy.id = generate_profile_id();
        copy.name = format!("{} (Copy)", copy.name);
        self.create_profile(copy)
    }

    pub fn set_active_profile(
        &mut self,
        id: &str,
        state: PowerState,
    ) -> Result<(), String> {
        if self.get_profile(id).is_none() {
            return Err(format!("Profile '{}' not found", id));
        }
        self.settings.state_map.insert(state, id.to_string());
        self.save_settings().map_err(|e| e.to_string())
    }

    pub fn update_settings(&mut self, settings: TccSettings) {
        self.settings = settings;
        let _ = self.save_settings();
    }

    pub fn set_keyboard_state(&mut self, state: KeyboardBacklightState) {
        self.keyboard_state = state;
    }

    pub fn set_charging_settings(&mut self, settings: ChargingSettings) {
        self.charging_settings = settings;
    }

    pub fn get_gpu_info(&self) -> &GpuInfoData {
        &self.gpu_info
    }

    pub fn get_power_settings(&self) -> &PowerSettings {
        &self.power_settings
    }

    pub fn set_power_settings(&mut self, settings: PowerSettings) {
        self.power_settings = settings;
    }

    pub fn get_display_modes(&self) -> &DisplayModes {
        &self.display_modes
    }

    pub fn set_display_modes(&mut self, modes: DisplayModes) {
        self.display_modes = modes;
    }

    pub fn list_webcam_devices(&self) -> Vec<WebcamDeviceInfo> {
        vec![
            WebcamDeviceInfo {
                path: "/dev/video0".into(),
                name: "Integrated Webcam".into(),
            },
        ]
    }

    pub fn get_webcam_controls(&self) -> &WebcamControls {
        &self.webcam_controls
    }

    pub fn set_webcam_controls(&mut self, controls: WebcamControls) {
        self.webcam_controls = controls;
    }
}

fn generate_profile_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("profile_{:x}", ts)
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (ProfileStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path());
        (store, dir)
    }

    #[test]
    fn default_profiles_loaded() {
        let (store, _dir) = temp_store();
        assert_eq!(store.list_profiles().len(), 4);
        assert!(store.get_profile(PROFILE_OFFICE).is_some());
    }

    #[test]
    fn create_and_get_profile() {
        let (mut store, _dir) = temp_store();
        let profile = TccProfile {
            id: String::new(),
            name: "Test".into(),
            description: "Test profile".into(),
            ..store.get_profile(PROFILE_OFFICE).unwrap().clone()
        };
        let id = store.create_profile(profile).unwrap();
        assert!(store.get_profile(&id).is_some());
        assert_eq!(store.list_profiles().len(), 5);
    }

    #[test]
    fn cannot_modify_default_profiles() {
        let (mut store, _dir) = temp_store();
        let profile = store.get_profile(PROFILE_OFFICE).unwrap().clone();
        let result = store.update_profile(PROFILE_OFFICE, profile);
        assert!(result.is_err());
    }

    #[test]
    fn delete_custom_profile() {
        let (mut store, _dir) = temp_store();
        let profile = TccProfile {
            id: "custom_1".into(),
            name: "Custom".into(),
            description: "".into(),
            ..store.get_profile(PROFILE_OFFICE).unwrap().clone()
        };
        store.create_profile(profile).unwrap();
        assert_eq!(store.list_profiles().len(), 5);
        store.delete_profile("custom_1").unwrap();
        assert_eq!(store.list_profiles().len(), 4);
    }

    #[test]
    fn copy_profile() {
        let (mut store, _dir) = temp_store();
        let new_id = store.copy_profile(PROFILE_OFFICE).unwrap();
        let copy = store.get_profile(&new_id).unwrap();
        assert!(copy.name.contains("Copy"));
        assert_eq!(store.list_profiles().len(), 5);
    }

    #[test]
    fn set_active_profile() {
        let (mut store, _dir) = temp_store();
        store
            .set_active_profile(PROFILE_HIGH_PERFORMANCE, PowerState::Ac)
            .unwrap();
        assert_eq!(
            store.active_profile_id(PowerState::Ac),
            Some(PROFILE_HIGH_PERFORMANCE)
        );
    }

    #[test]
    fn persistence_round_trip() {
        let dir = tempfile::tempdir().unwrap();

        // Create and save a custom profile
        {
            let mut store = ProfileStore::new(dir.path());
            let profile = TccProfile {
                id: "persist_test".into(),
                name: "Persist".into(),
                description: "".into(),
                ..store.get_profile(PROFILE_OFFICE).unwrap().clone()
            };
            store.create_profile(profile).unwrap();
            store
                .set_active_profile("persist_test", PowerState::Battery)
                .unwrap();
        }

        // Re-load and verify
        {
            let store = ProfileStore::new(dir.path());
            assert!(store.get_profile("persist_test").is_some());
            assert_eq!(
                store.active_profile_id(PowerState::Battery),
                Some("persist_test")
            );
        }
    }

    #[test]
    fn profile_json_compatibility() {
        let profiles = default_profiles();
        let json = serde_json::to_string_pretty(&profiles).unwrap();
        let parsed: Vec<TccProfile> = serde_json::from_str(&json).unwrap();
        assert_eq!(profiles, parsed);
    }

    #[test]
    fn delete_cleans_state_map() {
        let (mut store, _dir) = temp_store();
        let profile = TccProfile {
            id: "to_delete".into(),
            name: "ToDelete".into(),
            description: "".into(),
            ..store.get_profile(PROFILE_OFFICE).unwrap().clone()
        };
        store.create_profile(profile).unwrap();
        store
            .set_active_profile("to_delete", PowerState::Ac)
            .unwrap();
        store.delete_profile("to_delete").unwrap();
        // Should fall back to default
        assert_ne!(
            store.active_profile_id(PowerState::Ac),
            Some("to_delete")
        );
    }
}
