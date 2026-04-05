use tccd_daemon::io::{self, SysFsTuxedoIO, TuxedoIO};
use tccd_daemon::tuxedo_io::IoctlTuxedoIO;
use tccd_daemon::profiles::{FanTableEntry, PowerState, ProfileStore, TccProfile};
use tccd_daemon::workers::fan::FanControlTask;
use tccd_daemon::workers::power::PowerStateWorker;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{connection::Builder, fdo, interface};

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string()
}

fn kernel_version() -> String {
    std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(String::from))
        .unwrap_or_else(|| "unknown".into())
}

struct TccDaemon {
    io: Arc<dyn io::TuxedoIO + Send + Sync>,
    fan_task: Arc<FanControlTask>,
    profile_store: Arc<Mutex<ProfileStore>>,
}

#[interface(name = "com.tuxedocomputers.tccd")]
impl TccDaemon {
    async fn set_fan_speed_percent(&mut self, speed: u8) -> fdo::Result<()> {
        self.fan_task.set_manual_speed(0, speed as i32).await;
        println!("Fan speed manual override: {}%", speed);
        Ok(())
    }

    async fn get_fan_speed_percent(&self) -> fdo::Result<u8> {
        let speed = self
            .io
            .get_fan_speed_percent(0)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(speed as u8)
    }

    // ─── Profile Management ─────────────────────────────────────────

    async fn list_profiles(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.list_profiles())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn get_profile(&self, id: String) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        let profile = store
            .get_profile(&id)
            .ok_or_else(|| fdo::Error::Failed(format!("Profile '{}' not found", id)))?;
        serde_json::to_string(profile).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn create_profile(&mut self, json: String) -> fdo::Result<String> {
        let profile: TccProfile =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store
            .create_profile(profile)
            .map_err(fdo::Error::Failed)
    }

    async fn update_profile(&mut self, id: String, json: String) -> fdo::Result<()> {
        let profile: TccProfile =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store
            .update_profile(&id, profile)
            .map_err(fdo::Error::Failed)
    }

    async fn delete_profile(&mut self, id: String) -> fdo::Result<()> {
        let mut store = self.profile_store.lock().await;
        store
            .delete_profile(&id)
            .map_err(fdo::Error::Failed)
    }

    async fn copy_profile(&mut self, id: String) -> fdo::Result<String> {
        let mut store = self.profile_store.lock().await;
        store
            .copy_profile(&id)
            .map_err(fdo::Error::Failed)
    }

    async fn set_active_profile(&mut self, id: String, state: String) -> fdo::Result<()> {
        let power_state = match state.as_str() {
            "power_ac" => PowerState::Ac,
            "power_bat" => PowerState::Battery,
            other => {
                return Err(fdo::Error::Failed(format!(
                    "Invalid power state: '{}'",
                    other
                )))
            }
        };
        let mut store = self.profile_store.lock().await;
        store
            .set_active_profile(&id, power_state)
            .map_err(fdo::Error::Failed)?;

        // Apply the new profile's settings to hardware
        if let Some(profile) = store.get_profile(&id) {
            // Fan curve
            if profile.fan.use_control
                && let Some(ref table) = profile.fan.custom_fan_curve.table_cpu
            {
                self.fan_task.set_cpu_curve(table.clone()).await;
                if let Some(ref gpu_table) = profile.fan.custom_fan_curve.table_gpu {
                    self.fan_task.set_gpu_curve(gpu_table.clone()).await;
                }
            }
            // CPU settings (best-effort)
            if let Err(e) = self.io.set_cpu_governor(&profile.cpu.governor) {
                eprintln!("CPU governor: {}", e);
            }
            // no_turbo field: true means turbo is OFF
            if let Err(e) = self.io.set_cpu_turbo(!profile.cpu.no_turbo) {
                eprintln!("CPU turbo: {}", e);
            }
            if !profile.cpu.energy_performance_preference.is_empty()
                && let Err(e) = self
                    .io
                    .set_cpu_energy_perf(&profile.cpu.energy_performance_preference)
            {
                eprintln!("CPU energy perf: {}", e);
            }
        }
        Ok(())
    }

    async fn get_profile_assignments(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_settings())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    // ─── Telemetry ──────────────────────────────────────────────────

    async fn get_cpu_info(&self) -> fdo::Result<String> {
        let temp = self
            .io
            .get_cpu_temperature()
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let cores = self
            .io
            .get_cpu_core_count()
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut frequencies = Vec::new();
        for i in 0..cores as i32 {
            if let Ok(freq) = self.io.get_cpu_frequency_mhz(i) {
                frequencies.push(freq);
            }
        }
        let avg_freq = if frequencies.is_empty() {
            0.0
        } else {
            frequencies.iter().sum::<f64>() / frequencies.len() as f64
        };
        let info = serde_json::json!({
            "temperature": temp,
            "avgFrequencyMhz": avg_freq,
            "coreCount": cores,
        });
        serde_json::to_string(&info).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn get_power_state(&self) -> fdo::Result<String> {
        let ac = self
            .io
            .is_ac_power()
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(if ac { "ac".into() } else { "battery".into() })
    }

    async fn get_active_fan_curve(&self) -> fdo::Result<String> {
        let ac = self
            .io
            .is_ac_power()
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let state = if ac {
            PowerState::Ac
        } else {
            PowerState::Battery
        };
        let store = self.profile_store.lock().await;
        let profile = store
            .active_profile_id(state)
            .and_then(|id| store.get_profile(id))
            .or_else(|| store.list_profiles().first());
        let profile = profile
            .ok_or_else(|| fdo::Error::Failed("No profiles available".into()))?;
        let fan_json = serde_json::to_value(&profile.fan)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let result = serde_json::json!({
            "profileName": profile.name,
            "fan": fan_json,
        });
        serde_json::to_string(&result).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_fan_curve(&mut self, json: String) -> fdo::Result<()> {
        let entries: Vec<FanTableEntry> =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let ac = self
            .io
            .is_ac_power()
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let state = if ac {
            PowerState::Ac
        } else {
            PowerState::Battery
        };
        let mut store = self.profile_store.lock().await;
        let profile_id = store
            .active_profile_id(state)
            .map(|s| s.to_string())
            .ok_or_else(|| fdo::Error::Failed("No active profile".into()))?;
        let mut profile = store
            .get_profile(&profile_id)
            .ok_or_else(|| fdo::Error::Failed("Profile not found".into()))?
            .clone();
        profile.fan.custom_fan_curve.table_cpu = Some(entries.clone());
        store
            .update_profile(&profile_id, profile)
            .map_err(fdo::Error::Failed)?;

        // Apply updated curve to the running fan task immediately
        self.fan_task.set_cpu_curve(entries).await;
        Ok(())
    }

    // ─── Global Settings ────────────────────────────────────────────

    async fn get_global_settings(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_settings())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_global_settings(&mut self, json: String) -> fdo::Result<()> {
        let settings: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        let current = store.get_settings().clone();
        let updated = tccd_daemon::profiles::TccSettings {
            fahrenheit: settings
                .get("fahrenheit")
                .and_then(|v| v.as_bool())
                .unwrap_or(current.fahrenheit),
            cpu_settings_enabled: settings
                .get("cpuSettingsEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(current.cpu_settings_enabled),
            fan_control_enabled: settings
                .get("fanControlEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(current.fan_control_enabled),
            state_map: current.state_map,
        };
        store.update_settings(updated);
        Ok(())
    }

    // ─── Keyboard Backlight ─────────────────────────────────────────

    async fn get_keyboard_state(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_keyboard_state())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_keyboard_state(&mut self, json: String) -> fdo::Result<()> {
        let state: tccd_daemon::profiles::KeyboardBacklightState =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;

        // Apply to hardware (best-effort — log errors but don't fail)
        if let Err(e) = self.io.set_keyboard_brightness(state.brightness as u8) {
            eprintln!("Keyboard brightness: {}", e);
        }
        if let Err(e) = self.io.set_keyboard_color(&state.color) {
            eprintln!("Keyboard color: {}", e);
        }
        if let Err(e) = self.io.set_keyboard_mode(&state.mode) {
            eprintln!("Keyboard mode: {}", e);
        }

        let mut store = self.profile_store.lock().await;
        store.set_keyboard_state(state);
        Ok(())
    }

    // ─── Charging ───────────────────────────────────────────────────

    async fn get_charging_settings(&self) -> fdo::Result<String> {
        let mut settings = {
            let store = self.profile_store.lock().await;
            store.get_charging_settings().clone()
        };
        // Overlay real hardware values if available
        if let Ok(profile) = self.io.get_charging_profile() {
            settings.charging_profile = sysfs_to_charging_profile(&profile).to_string();
        }
        if let Ok(priority) = self.io.get_charging_priority() {
            settings.charging_priority = sysfs_to_charging_priority(&priority).to_string();
        }
        serde_json::to_string(&settings)
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_charging_settings(&mut self, json: String) -> fdo::Result<()> {
        let state: tccd_daemon::profiles::ChargingSettings =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;

        // Apply charging profile/priority to tuxedo sysfs (best-effort)
        let sysfs_profile = charging_profile_to_sysfs(&state.charging_profile);
        if let Err(e) = self.io.set_charging_profile(sysfs_profile) {
            eprintln!("Charging profile: {}", e);
        }
        let sysfs_priority = charging_priority_to_sysfs(&state.charging_priority);
        if let Err(e) = self.io.set_charging_priority(sysfs_priority) {
            eprintln!("Charging priority: {}", e);
        }

        // Apply charging thresholds to hardware (best-effort)
        let start = state.start_threshold as u8;
        let end = state.end_threshold as u8;
        if let Err(e) = self.io.set_charge_start_threshold(start) {
            eprintln!("Charge start threshold: {}", e);
        }
        if let Err(e) = self.io.set_charge_end_threshold(end) {
            eprintln!("Charge end threshold: {}", e);
        }

        let mut store = self.profile_store.lock().await;
        store.set_charging_settings(state);
        Ok(())
    }

    // ─── GPU / Power ────────────────────────────────────────────────

    async fn get_gpu_info(&self) -> fdo::Result<String> {
        let info = self
            .io
            .get_gpu_info()
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        serde_json::to_string(&info).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn get_power_settings(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_power_settings())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_power_settings(&mut self, json: String) -> fdo::Result<()> {
        let settings: tccd_daemon::profiles::PowerSettings =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.set_power_settings(settings);
        Ok(())
    }

    async fn schedule_shutdown(&self, hours: u32, minutes: u32) -> fdo::Result<()> {
        let total_minutes = hours * 60 + minutes;
        if total_minutes == 0 {
            return Err(fdo::Error::Failed("Shutdown time must be > 0".into()));
        }
        let output = tokio::process::Command::new("shutdown")
            .arg(format!("+{}", total_minutes))
            .output()
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to run shutdown: {}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(fdo::Error::Failed(format!("shutdown failed: {}", stderr)));
        }
        println!("Shutdown scheduled in {}h {}m (+{} min)", hours, minutes, total_minutes);
        Ok(())
    }

    async fn cancel_shutdown(&self) -> fdo::Result<()> {
        let output = tokio::process::Command::new("shutdown")
            .arg("-c")
            .output()
            .await
            .map_err(|e| fdo::Error::Failed(format!("Failed to cancel shutdown: {}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(fdo::Error::Failed(format!("cancel failed: {}", stderr)));
        }
        println!("Shutdown cancelled");
        Ok(())
    }

    // ─── Display ────────────────────────────────────────────────────

    async fn get_display_modes(&self) -> fdo::Result<String> {
        let mut modes = {
            let store = self.profile_store.lock().await;
            store.get_display_modes().clone()
        };
        // Overlay real brightness if available
        if let Ok((current, max)) = self.io.get_display_brightness()
            && max > 0
        {
            modes.brightness = (current as f64 / max as f64) * 100.0;
        }
        serde_json::to_string(&modes).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_display_settings(&mut self, json: String) -> fdo::Result<()> {
        let modes: tccd_daemon::profiles::DisplayModes =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;

        // Apply brightness to hardware (best-effort)
        if let Ok((_, max)) = self.io.get_display_brightness()
            && max > 0
        {
            let raw = ((modes.brightness / 100.0) * max as f64).round() as u32;
            if let Err(e) = self.io.set_display_brightness(raw) {
                eprintln!("Display brightness: {}", e);
            }
        }

        let mut store = self.profile_store.lock().await;
        store.set_display_modes(modes);
        Ok(())
    }

    // ─── Webcam ─────────────────────────────────────────────────────

    async fn list_webcam_devices(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(&store.list_webcam_devices())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn get_webcam_controls(&self, _device: String) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_webcam_controls())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_webcam_controls(&mut self, _device: String, json: String) -> fdo::Result<()> {
        let controls: tccd_daemon::profiles::WebcamControls =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.set_webcam_controls(controls);
        Ok(())
    }

    async fn get_system_info(&self) -> fdo::Result<String> {
        let info = serde_json::json!({
            "tccVersion": env!("CARGO_PKG_VERSION"),
            "daemonVersion": env!("CARGO_PKG_VERSION"),
            "hostname": hostname(),
            "kernelVersion": kernel_version(),
        });
        serde_json::to_string(&info).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn get_capabilities(&self) -> fdo::Result<String> {
        let charge_thresholds = self.io.get_charge_thresholds().is_ok();
        let charging_profile = self.io.get_charging_profile().is_ok();
        let fan_ok = self.io.get_fan_count().map(|c| c > 0).unwrap_or(false);
        let display_ok = self.io.get_display_brightness().is_ok();

        let result = serde_json::json!({
            "chargeThresholds": charge_thresholds,
            "chargingProfile": charging_profile,
            "fanControl": fan_ok,
            "displayBrightness": display_ok,
        });
        serde_json::to_string(&result).map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}

/// Map TUI charging profile name → sysfs value.
fn charging_profile_to_sysfs(profile: &str) -> &str {
    match profile {
        "Full Capacity" => "high_capacity",
        "Reduced" => "balanced",
        "Stationary" => "stationary",
        _ => "high_capacity",
    }
}

/// Map sysfs charging profile → TUI name.
fn sysfs_to_charging_profile(sysfs: &str) -> &str {
    match sysfs {
        "high_capacity" => "Full Capacity",
        "balanced" => "Reduced",
        "stationary" => "Stationary",
        _ => "Full Capacity",
    }
}

/// Map TUI charging priority name → sysfs value.
fn charging_priority_to_sysfs(priority: &str) -> &str {
    match priority {
        "Battery" => "charge_battery",
        "Performance" => "performance",
        _ => "charge_battery",
    }
}

/// Map sysfs charging priority → TUI name.
fn sysfs_to_charging_priority(sysfs: &str) -> &str {
    match sysfs {
        "charge_battery" => "Battery",
        "performance" => "Performance",
        _ => "Battery",
    }
}

fn use_session_bus() -> bool {
    std::env::args().any(|a| a == "--session" || a == "--bus=session")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Auto-detect hardware IO: prefer tuxedo_io ioctl if available, fall back to sysfs
    let hw_io: Arc<dyn TuxedoIO + Send + Sync> = match IoctlTuxedoIO::open() {
        Ok(ioctl_io) => {
            eprintln!("Using tuxedo_io ioctl interface");
            Arc::new(ioctl_io)
        }
        Err(e) => {
            eprintln!("tuxedo_io not available ({}), using sysfs fallback", e);
            Arc::new(SysFsTuxedoIO::new())
        }
    };
    let fan_task = Arc::new(FanControlTask::new(hw_io.clone(), 500));
    fan_task.spawn();

    let config_dir = dirs_config_path();
    let profile_store = Arc::new(Mutex::new(ProfileStore::new(&config_dir)));

    // Load active profile's fan curve on startup
    {
        let store = profile_store.lock().await;
        let ac = hw_io.is_ac_power().unwrap_or(true);
        let state = if ac { PowerState::Ac } else { PowerState::Battery };
        if let Some(profile) = store
            .active_profile_id(state)
            .and_then(|id| store.get_profile(id))
            .or_else(|| store.list_profiles().first())
            && profile.fan.use_control
            && let Some(ref table) = profile.fan.custom_fan_curve.table_cpu
        {
            fan_task.set_cpu_curve(table.clone()).await;
            if let Some(ref gpu_table) = profile.fan.custom_fan_curve.table_gpu {
                fan_task.set_gpu_curve(gpu_table.clone()).await;
            }
            println!("Loaded fan curve from profile '{}' ({} points)", profile.name, table.len());
        }
    }

    let daemon = TccDaemon {
        io: hw_io.clone(),
        fan_task: fan_task.clone(),
        profile_store: profile_store.clone(),
    };

    // Spawn power state monitor for auto-switching profiles on AC↔battery
    let power_worker = PowerStateWorker::new(
        hw_io,
        fan_task,
        profile_store,
        tokio::time::Duration::from_secs(5),
    );
    power_worker.spawn();

    let session = use_session_bus();
    let builder = if session {
        Builder::session()?
    } else {
        Builder::system()?
    };
    let _connection = builder
        .name("com.tuxedocomputers.tccd")?
        .serve_at("/com/tuxedocomputers/tccd", daemon)?
        .build()
        .await?;

    let bus_type = if session { "session" } else { "system" };
    println!("TCC Daemon running on {} bus (config: {:?})", bus_type, config_dir);
    std::future::pending::<()>().await;
    Ok(())
}

fn dirs_config_path() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("TCCD_CONFIG_DIR") {
        return std::path::PathBuf::from(dir);
    }
    // When running as system service, use a system-wide config path.
    // When running in session mode (dev), use per-user config.
    if use_session_bus()
        && let Some(config) = dirs::config_dir()
    {
        return config.join("tcc-rs");
    }
    std::path::PathBuf::from("/etc/tcc-rs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tccd_daemon::io::TuxedoIO;
    use tccd_daemon::profiles;
    use zbus::proxy;

    #[proxy(
        interface = "com.tuxedocomputers.tccd",
        default_service = "com.tuxedocomputers.tccd",
        default_path = "/com/tuxedocomputers/tccd"
    )]
    trait TccDaemonClient {
        async fn set_fan_speed_percent(&self, speed: u8) -> zbus::Result<()>;
        async fn get_fan_speed_percent(&self) -> zbus::Result<u8>;
        async fn list_profiles(&self) -> zbus::Result<String>;
        async fn get_profile(&self, id: String) -> zbus::Result<String>;
        async fn create_profile(&self, json: String) -> zbus::Result<String>;
        async fn update_profile(&self, id: String, json: String) -> zbus::Result<()>;
        async fn delete_profile(&self, id: String) -> zbus::Result<()>;
        async fn copy_profile(&self, id: String) -> zbus::Result<String>;
        async fn set_active_profile(&self, id: String, state: String) -> zbus::Result<()>;
        async fn get_profile_assignments(&self) -> zbus::Result<String>;
        async fn get_cpu_info(&self) -> zbus::Result<String>;
        async fn get_power_state(&self) -> zbus::Result<String>;
        async fn get_active_fan_curve(&self) -> zbus::Result<String>;
        async fn set_fan_curve(&self, json: String) -> zbus::Result<()>;
        async fn get_global_settings(&self) -> zbus::Result<String>;
        async fn set_global_settings(&self, json: String) -> zbus::Result<()>;
        async fn get_keyboard_state(&self) -> zbus::Result<String>;
        async fn set_keyboard_state(&self, json: String) -> zbus::Result<()>;
        async fn get_charging_settings(&self) -> zbus::Result<String>;
        async fn set_charging_settings(&self, json: String) -> zbus::Result<()>;
        async fn get_gpu_info(&self) -> zbus::Result<String>;
        async fn get_power_settings(&self) -> zbus::Result<String>;
        async fn set_power_settings(&self, json: String) -> zbus::Result<()>;
        async fn schedule_shutdown(&self, hours: u32, minutes: u32) -> zbus::Result<()>;
        async fn cancel_shutdown(&self) -> zbus::Result<()>;
        async fn get_display_modes(&self) -> zbus::Result<String>;
        async fn set_display_settings(&self, json: String) -> zbus::Result<()>;
        async fn list_webcam_devices(&self) -> zbus::Result<String>;
        async fn get_webcam_controls(&self, device: String) -> zbus::Result<String>;
        async fn set_webcam_controls(&self, device: String, json: String) -> zbus::Result<()>;
        async fn get_system_info(&self) -> zbus::Result<String>;
        async fn get_capabilities(&self) -> zbus::Result<String>;
    }

    fn test_daemon(mock_io: Arc<io::MockTuxedoIO>, fan_task: Arc<FanControlTask>) -> TccDaemon {
        let tmp = tempfile::tempdir().unwrap();
        let profile_store = Arc::new(Mutex::new(ProfileStore::new(tmp.path())));
        TccDaemon {
            io: mock_io,
            fan_task,
            profile_store,
        }
    }

    #[tokio::test]
    async fn test_dbus_fan_speed_loopback() -> Result<(), Box<dyn std::error::Error>> {
        let mock_io = Arc::new(io::MockTuxedoIO::new());
        mock_io.set_fan_speed_percent(0, 40).unwrap();

        let fan_task = Arc::new(FanControlTask::new(mock_io.clone(), 5));
        let handle = fan_task.spawn();

        let daemon = test_daemon(mock_io, fan_task.clone());
        let cx = zbus::connection::Builder::session()?
            .serve_at("/com/tuxedocomputers/tccd", daemon)?
            .build()
            .await?;

        let proxy = TccDaemonClientProxy::builder(&cx)
            .destination(cx.unique_name().unwrap())?
            .build()
            .await?;

        let initial_speed: u8 = proxy.get_fan_speed_percent().await?;
        assert_eq!(initial_speed, 40);

        proxy.set_fan_speed_percent(90).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let new_speed: u8 = proxy.get_fan_speed_percent().await?;
        assert_eq!(new_speed, 90);

        *fan_task.active.lock().await = false;
        handle.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_dbus_profile_crud() -> Result<(), Box<dyn std::error::Error>> {
        let mock_io = Arc::new(io::MockTuxedoIO::new());
        let fan_task = Arc::new(FanControlTask::new(mock_io.clone(), 5));

        let daemon = test_daemon(mock_io, fan_task);
        let cx = zbus::connection::Builder::session()?
            .serve_at("/com/tuxedocomputers/tccd", daemon)?
            .build()
            .await?;

        let proxy = TccDaemonClientProxy::builder(&cx)
            .destination(cx.unique_name().unwrap())?
            .build()
            .await?;

        // List default profiles
        let list_json = proxy.list_profiles().await?;
        let profiles: Vec<TccProfile> = serde_json::from_str(&list_json)?;
        assert_eq!(profiles.len(), 4);

        // Get a specific profile
        let office_json = proxy
            .get_profile(profiles::PROFILE_OFFICE.into())
            .await?;
        let office: TccProfile = serde_json::from_str(&office_json)?;
        assert_eq!(office.name, "Office");

        // Copy a profile
        let copy_id = proxy
            .copy_profile(profiles::PROFILE_OFFICE.into())
            .await?;
        let copy_json = proxy.get_profile(copy_id.clone()).await?;
        let copy: TccProfile = serde_json::from_str(&copy_json)?;
        assert!(copy.name.contains("Copy"));

        // Update the copy
        let mut updated = copy.clone();
        updated.name = "My Custom".into();
        proxy
            .update_profile(copy_id.clone(), serde_json::to_string(&updated)?)
            .await?;
        let updated_json = proxy.get_profile(copy_id.clone()).await?;
        let updated_profile: TccProfile = serde_json::from_str(&updated_json)?;
        assert_eq!(updated_profile.name, "My Custom");

        // Set active profile
        proxy
            .set_active_profile(copy_id.clone(), "power_ac".into())
            .await?;
        let assignments_json = proxy.get_profile_assignments().await?;
        let settings: profiles::TccSettings = serde_json::from_str(&assignments_json)?;
        assert_eq!(settings.state_map[&PowerState::Ac], copy_id);

        // Delete the copy
        proxy.delete_profile(copy_id.clone()).await?;
        let list_after = proxy.list_profiles().await?;
        let profiles_after: Vec<TccProfile> = serde_json::from_str(&list_after)?;
        assert_eq!(profiles_after.len(), 4);

        Ok(())
    }
}
