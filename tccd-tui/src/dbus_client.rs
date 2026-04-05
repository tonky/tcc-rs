use zbus::{Result, proxy};

/// D-Bus proxy generated from the daemon's interface.
/// Matches the `com.tuxedocomputers.tccd` interface in tccd-daemon.
#[proxy(
    interface = "com.tuxedocomputers.tccd",
    default_service = "com.tuxedocomputers.tccd",
    default_path = "/com/tuxedocomputers/tccd"
)]
trait TccdDaemon {
    async fn set_fan_speed_percent(&self, speed: u8) -> Result<()>;
    async fn get_fan_speed_percent(&self) -> Result<u8>;
    async fn list_profiles(&self) -> Result<String>;
    async fn get_profile(&self, id: String) -> Result<String>;
    async fn create_profile(&self, json: String) -> Result<String>;
    async fn update_profile(&self, id: String, json: String) -> Result<()>;
    async fn delete_profile(&self, id: String) -> Result<()>;
    async fn copy_profile(&self, id: String) -> Result<String>;
    async fn set_active_profile(&self, id: String, state: String) -> Result<()>;
    async fn get_profile_assignments(&self) -> Result<String>;
    async fn get_cpu_info(&self) -> Result<String>;
    async fn get_power_state(&self) -> Result<String>;
    async fn get_active_fan_curve(&self) -> Result<String>;
    async fn set_fan_curve(&self, json: String) -> Result<()>;
    async fn get_global_settings(&self) -> Result<String>;
    async fn set_global_settings(&self, json: String) -> Result<()>;
    async fn get_keyboard_state(&self) -> Result<String>;
    async fn set_keyboard_state(&self, json: String) -> Result<()>;
    async fn get_charging_settings(&self) -> Result<String>;
    async fn set_charging_settings(&self, json: String) -> Result<()>;
    async fn get_gpu_info(&self) -> Result<String>;
    async fn get_power_settings(&self) -> Result<String>;
    async fn set_power_settings(&self, json: String) -> Result<()>;
    async fn schedule_shutdown(&self, hours: u32, minutes: u32) -> Result<()>;
    async fn cancel_shutdown(&self) -> Result<()>;
    async fn get_display_modes(&self) -> Result<String>;
    async fn set_display_settings(&self, json: String) -> Result<()>;
    async fn list_webcam_devices(&self) -> Result<String>;
    async fn get_webcam_controls(&self, device: String) -> Result<String>;
    async fn set_webcam_controls(&self, device: String, json: String) -> Result<()>;
    async fn get_system_info(&self) -> Result<String>;
    async fn get_capabilities(&self) -> Result<String>;
}

/// Manages the D-Bus connection to the daemon, with reconnect support.
pub struct DaemonClient {
    proxy: Option<TccdDaemonProxy<'static>>,
    use_session_bus: bool,
}

impl DaemonClient {
    pub fn new(use_session_bus: bool) -> Self {
        Self {
            proxy: None,
            use_session_bus,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let connection = if self.use_session_bus {
            zbus::Connection::session().await?
        } else {
            zbus::Connection::system().await?
        };
        self.proxy = Some(TccdDaemonProxy::new(&connection).await?);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.proxy.is_some()
    }

    fn proxy(&self) -> Result<&TccdDaemonProxy<'static>> {
        self.proxy
            .as_ref()
            .ok_or(zbus::Error::Failure("Not connected to daemon".into()))
    }

    pub async fn get_fan_speed_percent(&self) -> Result<u8> {
        self.proxy()?.get_fan_speed_percent().await
    }

    pub async fn set_fan_speed_percent(&self, speed: u8) -> Result<()> {
        self.proxy()?.set_fan_speed_percent(speed).await
    }

    pub async fn list_profiles(&self) -> Result<String> {
        self.proxy()?.list_profiles().await
    }

    pub async fn get_profile(&self, id: &str) -> Result<String> {
        self.proxy()?.get_profile(id.to_string()).await
    }

    #[allow(dead_code)]
    pub async fn create_profile(&self, json: &str) -> Result<String> {
        self.proxy()?.create_profile(json.to_string()).await
    }

    pub async fn update_profile(&self, id: &str, json: &str) -> Result<()> {
        self.proxy()?
            .update_profile(id.to_string(), json.to_string())
            .await
    }

    pub async fn delete_profile(&self, id: &str) -> Result<()> {
        self.proxy()?.delete_profile(id.to_string()).await
    }

    pub async fn copy_profile(&self, id: &str) -> Result<String> {
        self.proxy()?.copy_profile(id.to_string()).await
    }

    pub async fn set_active_profile(&self, id: &str, state: &str) -> Result<()> {
        self.proxy()?
            .set_active_profile(id.to_string(), state.to_string())
            .await
    }

    pub async fn get_profile_assignments(&self) -> Result<String> {
        self.proxy()?.get_profile_assignments().await
    }

    pub async fn get_cpu_info(&self) -> Result<String> {
        self.proxy()?.get_cpu_info().await
    }

    pub async fn get_power_state(&self) -> Result<String> {
        self.proxy()?.get_power_state().await
    }

    pub async fn get_active_fan_curve(&self) -> Result<String> {
        self.proxy()?.get_active_fan_curve().await
    }

    pub async fn set_fan_curve(&self, json: &str) -> Result<()> {
        self.proxy()?.set_fan_curve(json.to_string()).await
    }

    pub async fn get_global_settings(&self) -> Result<String> {
        self.proxy()?.get_global_settings().await
    }

    pub async fn set_global_settings(&self, json: &str) -> Result<()> {
        self.proxy()?.set_global_settings(json.to_string()).await
    }

    pub async fn get_keyboard_state(&self) -> Result<String> {
        self.proxy()?.get_keyboard_state().await
    }

    pub async fn set_keyboard_state(&self, json: &str) -> Result<()> {
        self.proxy()?.set_keyboard_state(json.to_string()).await
    }

    pub async fn get_charging_settings(&self) -> Result<String> {
        self.proxy()?.get_charging_settings().await
    }

    pub async fn set_charging_settings(&self, json: &str) -> Result<()> {
        self.proxy()?.set_charging_settings(json.to_string()).await
    }

    pub async fn get_gpu_info(&self) -> Result<String> {
        self.proxy()?.get_gpu_info().await
    }

    pub async fn get_power_settings(&self) -> Result<String> {
        self.proxy()?.get_power_settings().await
    }

    pub async fn set_power_settings(&self, json: &str) -> Result<()> {
        self.proxy()?.set_power_settings(json.to_string()).await
    }

    pub async fn schedule_shutdown(&self, hours: u32, minutes: u32) -> Result<()> {
        self.proxy()?.schedule_shutdown(hours, minutes).await
    }

    pub async fn cancel_shutdown(&self) -> Result<()> {
        self.proxy()?.cancel_shutdown().await
    }

    pub async fn get_display_modes(&self) -> Result<String> {
        self.proxy()?.get_display_modes().await
    }

    pub async fn set_display_settings(&self, json: &str) -> Result<()> {
        self.proxy()?.set_display_settings(json.to_string()).await
    }

    pub async fn list_webcam_devices(&self) -> Result<String> {
        self.proxy()?.list_webcam_devices().await
    }

    pub async fn get_webcam_controls(&self, device: &str) -> Result<String> {
        self.proxy()?.get_webcam_controls(device.to_string()).await
    }

    pub async fn set_webcam_controls(&self, device: &str, json: &str) -> Result<()> {
        self.proxy()?
            .set_webcam_controls(device.to_string(), json.to_string())
            .await
    }

    pub async fn get_system_info(&self) -> Result<String> {
        self.proxy()?.get_system_info().await
    }

    pub async fn get_capabilities(&self) -> Result<String> {
        self.proxy()?.get_capabilities().await
    }
}
