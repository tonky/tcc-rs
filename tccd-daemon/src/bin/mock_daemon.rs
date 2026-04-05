use std::sync::Arc;
use std::time::Duration;
use rand::Rng;
use tokio::sync::Mutex;
use tokio::time;
use zbus::{interface, fdo};

use tccd_daemon::profiles::{
    ChargingSettings, DisplayModes, KeyboardBacklightState, PowerSettings,
    ProfileStore, TccProfile, PowerState, WebcamControls,
};

struct DaemonState {
    fan_speed_percent: u8,
    cpu_temp: f64,
    cpu_freq: f64,
    core_count: usize,
    on_ac: bool,
}

struct MockDaemon {
    state: Arc<Mutex<DaemonState>>,
    profile_store: Arc<Mutex<ProfileStore>>,
}

#[interface(name = "com.tuxedocomputers.tccd")]
impl MockDaemon {
    async fn set_fan_speed_percent(&mut self, speed: u8) -> fdo::Result<()> {
        self.state.lock().await.fan_speed_percent = speed;
        println!("Fan speed set to {}%", speed);
        Ok(())
    }

    async fn get_fan_speed_percent(&self) -> fdo::Result<u8> {
        Ok(self.state.lock().await.fan_speed_percent)
    }

    async fn get_cpu_info(&self) -> fdo::Result<String> {
        let s = self.state.lock().await;
        let info = serde_json::json!({
            "temperature": s.cpu_temp,
            "avgFrequencyMhz": s.cpu_freq,
            "coreCount": s.core_count,
        });
        serde_json::to_string(&info).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn get_power_state(&self) -> fdo::Result<String> {
        let s = self.state.lock().await;
        Ok(if s.on_ac { "ac" } else { "battery" }.into())
    }

    async fn get_active_fan_curve(&self) -> fdo::Result<String> {
        let s = self.state.lock().await;
        let power_state = if s.on_ac { PowerState::Ac } else { PowerState::Battery };
        let store = self.profile_store.lock().await;
        let profile = store
            .active_profile_id(power_state)
            .and_then(|id| store.get_profile(id))
            .or_else(|| store.list_profiles().first());
        let profile = profile
            .ok_or_else(|| fdo::Error::Failed("No profiles".into()))?;
        let fan_json = serde_json::to_value(&profile.fan)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let result = serde_json::json!({
            "profileName": profile.name,
            "fan": fan_json,
        });
        serde_json::to_string(&result).map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_fan_curve(&mut self, json: String) -> fdo::Result<()> {
        let entries: Vec<tccd_daemon::profiles::FanTableEntry> =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let s = self.state.lock().await;
        let power_state = if s.on_ac {
            tccd_daemon::profiles::PowerState::Ac
        } else {
            tccd_daemon::profiles::PowerState::Battery
        };
        drop(s);
        let mut store = self.profile_store.lock().await;
        let profile_id = store
            .active_profile_id(power_state)
            .map(|s| s.to_string())
            .ok_or_else(|| fdo::Error::Failed("No active profile".into()))?;
        let mut profile = store
            .get_profile(&profile_id)
            .ok_or_else(|| fdo::Error::Failed("Profile not found".into()))?
            .clone();
        profile.fan.custom_fan_curve.table_cpu = Some(entries);
        store
            .update_profile(&profile_id, profile)
            .map_err(fdo::Error::Failed)
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
        store.create_profile(profile).map_err(fdo::Error::Failed)
    }

    async fn update_profile(&mut self, id: String, json: String) -> fdo::Result<()> {
        let profile: TccProfile =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.update_profile(&id, profile).map_err(fdo::Error::Failed)
    }

    async fn delete_profile(&mut self, id: String) -> fdo::Result<()> {
        let mut store = self.profile_store.lock().await;
        store.delete_profile(&id).map_err(fdo::Error::Failed)
    }

    async fn copy_profile(&mut self, id: String) -> fdo::Result<String> {
        let mut store = self.profile_store.lock().await;
        store.copy_profile(&id).map_err(fdo::Error::Failed)
    }

    async fn set_active_profile(&mut self, id: String, state: String) -> fdo::Result<()> {
        let power_state = match state.as_str() {
            "power_ac" => PowerState::Ac,
            "power_bat" => PowerState::Battery,
            other => {
                return Err(fdo::Error::Failed(format!("Invalid power state: '{}'", other)))
            }
        };
        let mut store = self.profile_store.lock().await;
        store.set_active_profile(&id, power_state).map_err(fdo::Error::Failed)
    }

    async fn get_profile_assignments(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_settings())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
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
        println!("Settings updated");
        Ok(())
    }

    // ─── Keyboard Backlight ─────────────────────────────────────────

    async fn get_keyboard_state(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_keyboard_state())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_keyboard_state(&mut self, json: String) -> fdo::Result<()> {
        let state: KeyboardBacklightState =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.set_keyboard_state(state);
        println!("Keyboard backlight updated");
        Ok(())
    }

    // ─── Charging ───────────────────────────────────────────────────

    async fn get_charging_settings(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_charging_settings())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_charging_settings(&mut self, json: String) -> fdo::Result<()> {
        let state: ChargingSettings =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.set_charging_settings(state);
        println!("Charging settings updated");
        Ok(())
    }

    // ─── GPU / Power ────────────────────────────────────────────────

    async fn get_gpu_info(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_gpu_info())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn get_power_settings(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_power_settings())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_power_settings(&mut self, json: String) -> fdo::Result<()> {
        let settings: PowerSettings =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.set_power_settings(settings);
        println!("Power settings updated");
        Ok(())
    }

    async fn schedule_shutdown(&self, hours: u32, minutes: u32) -> fdo::Result<()> {
        println!("Shutdown scheduled in {}h {}m", hours, minutes);
        Ok(())
    }

    async fn cancel_shutdown(&self) -> fdo::Result<()> {
        println!("Shutdown cancelled");
        Ok(())
    }

    // ─── Display ────────────────────────────────────────────────────

    async fn get_display_modes(&self) -> fdo::Result<String> {
        let store = self.profile_store.lock().await;
        serde_json::to_string(store.get_display_modes())
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn set_display_settings(&mut self, json: String) -> fdo::Result<()> {
        let modes: DisplayModes =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.set_display_modes(modes);
        println!("Display settings updated");
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
        let controls: WebcamControls =
            serde_json::from_str(&json).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let mut store = self.profile_store.lock().await;
        store.set_webcam_controls(controls);
        println!("Webcam controls updated");
        Ok(())
    }

    async fn get_system_info(&self) -> fdo::Result<String> {
        let info = serde_json::json!({
            "tccVersion": env!("CARGO_PKG_VERSION"),
            "daemonVersion": env!("CARGO_PKG_VERSION"),
            "hostname": "mock-host",
            "kernelVersion": "6.8.0-mock",
        });
        serde_json::to_string(&info).map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}

#[tokio::main]
async fn main() -> zbus::Result<()> {
    let state = Arc::new(Mutex::new(DaemonState {
        fan_speed_percent: 35,
        cpu_temp: 45.0,
        cpu_freq: 2400.0,
        core_count: 8,
        on_ac: true,
    }));

    let tmp_dir = std::env::temp_dir().join("tccd-mock");
    let profile_store = Arc::new(Mutex::new(ProfileStore::new(&tmp_dir)));

    let mock = MockDaemon {
        state: state.clone(),
        profile_store,
    };

    let _conn = zbus::connection::Builder::session()?
        .name("com.tuxedocomputers.tccd")?
        .serve_at("/com/tuxedocomputers/tccd", mock)?
        .build()
        .await?;

    println!("Mock TCCD daemon running on session D-Bus.");
    println!("Use 'just run-tui-dev' in another terminal.");

    // Simulate changing hardware telemetry
    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_millis(2000)).await;
            let mut s = state.lock().await;
            let mut rng = rand::thread_rng();

            let temp_delta: f64 = rng.gen_range(-2.0..=3.0);
            s.cpu_temp = (s.cpu_temp + temp_delta).clamp(35.0, 90.0);

            // Fan reacts to temp
            s.fan_speed_percent = match s.cpu_temp as u32 {
                0..=45 => rng.gen_range(15..=25),
                46..=60 => rng.gen_range(30..=50),
                61..=75 => rng.gen_range(50..=70),
                _ => rng.gen_range(75..=95),
            };

            let base_freq = if s.cpu_temp > 80.0 { 1800.0 } else { 3200.0 };
            s.cpu_freq = base_freq + rng.gen_range(0.0..800.0);
        }
    });

    std::future::pending::<()>().await;
    Ok(())
}
