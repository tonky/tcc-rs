use crate::profiles::GpuInfoData;
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum AttributeError {
    #[error("Hardware error: {0}")]
    HardwareError(String),
    #[error("Permission denied: {0} (try running as root or adding user to appropriate group)")]
    PermissionDenied(String),
    #[error("Not found: {0}")]
    NotFound(String),
}

pub trait TuxedoIO {
    fn set_fan_speed_percent(&self, fan_idx: i32, speed: i32) -> Result<(), AttributeError>;
    fn get_fan_speed_percent(&self, fan_idx: i32) -> Result<i32, AttributeError>;
    fn get_fan_rpm(&self, fan_idx: i32) -> Result<u32, AttributeError>;
    fn set_webcam_status(&self, enabled: bool) -> Result<(), AttributeError>;
    fn get_cpu_temperature(&self) -> Result<f64, AttributeError>;
    fn get_cpu_frequency_mhz(&self, cpu_idx: i32) -> Result<f64, AttributeError>;
    fn get_cpu_core_count(&self) -> Result<usize, AttributeError>;
    fn is_ac_power(&self) -> Result<bool, AttributeError>;

    // ─── CPU control ────────────────────────────────────────────────
    fn set_cpu_governor(&self, governor: &str) -> Result<(), AttributeError>;
    fn set_cpu_turbo(&self, enabled: bool) -> Result<(), AttributeError>;
    fn set_cpu_energy_perf(&self, preference: &str) -> Result<(), AttributeError>;

    // ─── Charging ───────────────────────────────────────────────────
    fn set_charge_start_threshold(&self, percent: u8) -> Result<(), AttributeError>;
    fn set_charge_end_threshold(&self, percent: u8) -> Result<(), AttributeError>;
    fn get_charge_thresholds(&self) -> Result<(u8, u8), AttributeError>;

    // ─── Keyboard ───────────────────────────────────────────────────
    fn set_keyboard_brightness(&self, brightness: u8) -> Result<(), AttributeError>;
    fn set_keyboard_color(&self, color: &str) -> Result<(), AttributeError>;
    fn set_keyboard_mode(&self, mode: &str) -> Result<(), AttributeError>;

    // ─── GPU ────────────────────────────────────────────────────────
    fn get_gpu_info(&self) -> Result<GpuInfoData, AttributeError>;

    // ─── Display backlight ──────────────────────────────────────────
    fn get_display_brightness(&self) -> Result<(u32, u32), AttributeError>; // (current, max)
    fn set_display_brightness(&self, value: u32) -> Result<(), AttributeError>;

    // ─── Fan discovery ──────────────────────────────────────────────
    fn get_fan_count(&self) -> Result<usize, AttributeError>;
}

// ─── SysFs-based hardware IO ────────────────────────────────────────

/// Real hardware access via Linux sysfs. Reads work without root;
/// writes may fail with `PermissionDenied` unless the user has
/// appropriate privileges or group membership.
/// Describes what kind of fan control interface was found.
#[derive(Debug, Clone)]
enum FanInterface {
    /// tuxedo-drivers platform device: /sys/devices/platform/{name}/fan{N}_pwm
    TuxedoPlatform(String),
    /// Generic hwmon: /sys/class/hwmon/hwmonN/pwmN
    GenericHwmon(String),
}

/// Describes what kind of keyboard backlight interface was found.
#[derive(Debug, Clone)]
enum KeyboardInterface {
    /// Linux LED class: /sys/class/leds/{name}/ with brightness file
    LedClass { path: String, is_rgb: bool },
    /// Old tuxedo_keyboard platform: /sys/devices/platform/tuxedo_keyboard/
    TuxedoPlatform,
}

pub struct SysFsTuxedoIO {
    /// Cached fan control interface (tuxedo platform or generic hwmon).
    fan_interface: RwLock<Option<FanInterface>>,
    /// Cached path to the hwmon device that has fan RPM readback.
    fan_hwmon_path: RwLock<Option<String>>,
    /// Cached keyboard backlight interface.
    keyboard_interface: RwLock<Option<KeyboardInterface>>,
    /// Cached path to the AC power supply.
    ac_supply_path: RwLock<Option<String>>,
    /// Cached path to the battery.
    battery_path: RwLock<Option<String>>,
    /// Cached path to the display backlight sysfs directory.
    backlight_path: RwLock<Option<String>>,
}

impl Default for SysFsTuxedoIO {
    fn default() -> Self {
        Self::new()
    }
}

impl SysFsTuxedoIO {
    pub fn new() -> Self {
        Self {
            fan_interface: RwLock::new(None),
            fan_hwmon_path: RwLock::new(None),
            keyboard_interface: RwLock::new(None),
            ac_supply_path: RwLock::new(None),
            battery_path: RwLock::new(None),
            backlight_path: RwLock::new(None),
        }
    }

    fn read_sysfs(path: &str) -> Result<String, AttributeError> {
        std::fs::read_to_string(path)
            .map(|s| s.trim().to_string())
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    AttributeError::PermissionDenied(path.to_string())
                }
                std::io::ErrorKind::NotFound => {
                    AttributeError::NotFound(path.to_string())
                }
                _ => AttributeError::HardwareError(format!("{}: {}", path, e)),
            })
    }

    fn write_sysfs(path: &str, value: &str) -> Result<(), AttributeError> {
        std::fs::write(path, value).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                AttributeError::PermissionDenied(path.to_string())
            }
            std::io::ErrorKind::NotFound => {
                AttributeError::NotFound(path.to_string())
            }
            _ => AttributeError::HardwareError(format!("{}: {}", path, e)),
        })
    }

    /// Find fan control interface. Tries in order:
    /// 1. tuxedo-drivers platform device (tuxedo_fan_control, tuxedo_tuxi_fan_control)
    /// 2. Generic hwmon with pwm1
    fn find_fan_interface(&self) -> Result<FanInterface, AttributeError> {
        if let Some(ref iface) = *self.fan_interface.read().unwrap() {
            return Ok(iface.clone());
        }

        // 1. Check tuxedo-drivers platform devices
        let platform_base = "/sys/devices/platform";
        for name in &["tuxedo_fan_control", "tuxedo_tuxi_fan_control"] {
            let path = format!("{}/{}", platform_base, name);
            if std::path::Path::new(&path).join("fan1_pwm").exists() {
                let iface = FanInterface::TuxedoPlatform(path);
                *self.fan_interface.write().unwrap() = Some(iface.clone());
                return Ok(iface);
            }
        }

        // 2. Fall back to generic hwmon with pwm1
        let hwmon_base = "/sys/class/hwmon";
        if let Ok(entries) = std::fs::read_dir(hwmon_base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("pwm1").exists() {
                    let hwmon_path = path.to_string_lossy().to_string();
                    let iface = FanInterface::GenericHwmon(hwmon_path);
                    *self.fan_interface.write().unwrap() = Some(iface.clone());
                    return Ok(iface);
                }
            }
        }

        Err(AttributeError::NotFound(
            "No fan control interface found (tuxedo_fan_control or hwmon with PWM)".to_string(),
        ))
    }

    /// Find the hwmon device that has fan RPM readback (fan*_input).
    fn find_fan_hwmon(&self) -> Result<String, AttributeError> {
        if let Some(ref path) = *self.fan_hwmon_path.read().unwrap() {
            return Ok(path.clone());
        }

        let hwmon_base = "/sys/class/hwmon";
        let entries = std::fs::read_dir(hwmon_base).map_err(|e| {
            AttributeError::NotFound(format!("{}: {}", hwmon_base, e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            // Look for fan0_input (tuxedo hwmon) or fan1_input (standard)
            if path.join("fan0_input").exists() || path.join("fan1_input").exists() {
                let hwmon_path = path.to_string_lossy().to_string();
                *self.fan_hwmon_path.write().unwrap() = Some(hwmon_path.clone());
                return Ok(hwmon_path);
            }
        }

        Err(AttributeError::NotFound(
            "No hwmon device with fan RPM readback found".to_string(),
        ))
    }

    /// Find keyboard backlight interface. Tries in order:
    /// 1. Linux LED class: /sys/class/leds/rgb:keyboard/ (multi-color)
    /// 2. Linux LED class: /sys/class/leds/white:keyboard/ (single-color)
    /// 3. Old tuxedo_keyboard platform device
    fn find_keyboard(&self) -> Result<KeyboardInterface, AttributeError> {
        if let Some(ref iface) = *self.keyboard_interface.read().unwrap() {
            return Ok(iface.clone());
        }

        // 1. RGB LED class (NB04)
        let rgb_path = "/sys/class/leds/rgb:keyboard";
        if std::path::Path::new(rgb_path).join("brightness").exists() {
            let iface = KeyboardInterface::LedClass {
                path: rgb_path.to_string(),
                is_rgb: true,
            };
            *self.keyboard_interface.write().unwrap() = Some(iface.clone());
            return Ok(iface);
        }

        // 2. White LED class (NB05)
        let white_path = "/sys/class/leds/white:keyboard";
        if std::path::Path::new(white_path).join("brightness").exists() {
            let iface = KeyboardInterface::LedClass {
                path: white_path.to_string(),
                is_rgb: false,
            };
            *self.keyboard_interface.write().unwrap() = Some(iface.clone());
            return Ok(iface);
        }

        // 3. Old platform device
        let platform_path = "/sys/devices/platform/tuxedo_keyboard";
        if std::path::Path::new(platform_path).exists() {
            let iface = KeyboardInterface::TuxedoPlatform;
            *self.keyboard_interface.write().unwrap() = Some(iface.clone());
            return Ok(iface);
        }

        Err(AttributeError::NotFound(
            "No keyboard backlight interface found (LED class or tuxedo_keyboard)".to_string(),
        ))
    }

    /// Find a power supply of type "Mains" (AC adapter).
    fn find_ac_supply(&self) -> Result<String, AttributeError> {
        // Check cache first
        if let Some(ref path) = *self.ac_supply_path.read().unwrap() {
            return Ok(path.clone());
        }

        let ps_base = "/sys/class/power_supply";
        let entries = std::fs::read_dir(ps_base).map_err(|e| {
            AttributeError::NotFound(format!("{}: {}", ps_base, e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            let type_path = path.join("type");
            if let Ok(ptype) = std::fs::read_to_string(&type_path)
                && ptype.trim() == "Mains"
            {
                let ac_path = path.to_string_lossy().to_string();
                *self.ac_supply_path.write().unwrap() = Some(ac_path.clone());
                return Ok(ac_path);
            }
        }

        Err(AttributeError::NotFound(
            "No AC power supply found in /sys/class/power_supply".to_string(),
        ))
    }

    /// Find a battery power supply.
    fn find_battery(&self) -> Result<String, AttributeError> {
        // Check cache first
        if let Some(ref path) = *self.battery_path.read().unwrap() {
            return Ok(path.clone());
        }

        let ps_base = "/sys/class/power_supply";
        let entries = std::fs::read_dir(ps_base).map_err(|e| {
            AttributeError::NotFound(format!("{}: {}", ps_base, e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            let type_path = path.join("type");
            if let Ok(ptype) = std::fs::read_to_string(&type_path)
                && ptype.trim() == "Battery"
            {
                let bat_path = path.to_string_lossy().to_string();
                *self.battery_path.write().unwrap() = Some(bat_path.clone());
                return Ok(bat_path);
            }
        }

        Err(AttributeError::NotFound(
            "No battery found in /sys/class/power_supply".to_string(),
        ))
    }

    /// Find a backlight device in /sys/class/backlight/.
    fn find_backlight(&self) -> Result<String, AttributeError> {
        if let Some(ref path) = *self.backlight_path.read().unwrap() {
            return Ok(path.clone());
        }

        let bl_base = "/sys/class/backlight";
        let entries = std::fs::read_dir(bl_base).map_err(|e| {
            AttributeError::NotFound(format!("{}: {}", bl_base, e))
        })?;

        // Prefer intel_backlight or amdgpu_bl*, fall back to any
        let mut fallback: Option<String> = None;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let p = entry.path().to_string_lossy().to_string();
            if name.contains("intel") || name.starts_with("amdgpu") {
                *self.backlight_path.write().unwrap() = Some(p.clone());
                return Ok(p);
            }
            if fallback.is_none() {
                fallback = Some(p);
            }
        }

        if let Some(p) = fallback {
            *self.backlight_path.write().unwrap() = Some(p.clone());
            return Ok(p);
        }

        Err(AttributeError::NotFound(
            "No backlight device found in /sys/class/backlight".to_string(),
        ))
    }

    /// Scan DRM/PCI for GPU devices. Returns (dGPU, iGPU) info.
    fn scan_gpus() -> GpuInfoData {
        let drm_base = "/sys/class/drm";
        let mut dgpu_name = String::new();
        let mut igpu_name = String::new();
        let mut dgpu_temp: Option<f64> = None;

        let entries = match std::fs::read_dir(drm_base) {
            Ok(e) => e,
            Err(_) => return GpuInfoData::default(),
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Only look at cardN entries (not cardN-* connectors)
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }

            let device_dir = entry.path().join("device");
            let vendor = std::fs::read_to_string(device_dir.join("vendor"))
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            let device_id = std::fs::read_to_string(device_dir.join("device"))
                .unwrap_or_default()
                .trim()
                .to_lowercase();

            if vendor.is_empty() {
                continue;
            }

            let gpu_label = Self::pci_gpu_label(&vendor, &device_id);

            let is_discrete = vendor == "0x10de" || vendor == "0x1002"; // NVIDIA or AMD dGPU
            let is_intel = vendor == "0x8086";

            if is_discrete && dgpu_name.is_empty() {
                dgpu_name = gpu_label;
                // Try to find hwmon temp for dGPU
                dgpu_temp = Self::read_gpu_hwmon_temp(&device_dir);
            } else if is_intel && igpu_name.is_empty() {
                igpu_name = gpu_label;
            } else if dgpu_name.is_empty() && !is_intel {
                dgpu_name = gpu_label;
                dgpu_temp = Self::read_gpu_hwmon_temp(&device_dir);
            } else if igpu_name.is_empty() {
                igpu_name = gpu_label;
            }
        }

        GpuInfoData {
            dgpu_name: if dgpu_name.is_empty() { "None".into() } else { dgpu_name },
            dgpu_temp,
            dgpu_usage: None,
            dgpu_power_draw: None,
            igpu_name: if igpu_name.is_empty() { "None".into() } else { igpu_name },
            igpu_usage: None,
            prime_mode: Self::detect_prime_mode(),
            tgp_offset: None,
        }
    }

    /// Generate a human-readable GPU label from PCI vendor/device IDs.
    fn pci_gpu_label(vendor: &str, device_id: &str) -> String {
        let vendor_name = match vendor {
            "0x10de" => "NVIDIA",
            "0x1002" => "AMD",
            "0x8086" => "Intel",
            other => other,
        };
        format!("{} [{}]", vendor_name, device_id)
    }

    /// Try to read GPU temperature from the DRM device's hwmon.
    fn read_gpu_hwmon_temp(device_dir: &std::path::Path) -> Option<f64> {
        let hwmon_dir = device_dir.join("hwmon");
        let entries = std::fs::read_dir(&hwmon_dir).ok()?;
        for entry in entries.flatten() {
            let temp_path = entry.path().join("temp1_input");
            if let Ok(val) = std::fs::read_to_string(&temp_path)
                && let Ok(millideg) = val.trim().parse::<f64>()
            {
                return Some(millideg / 1000.0);
            }
        }
        None
    }

    /// Detect PRIME render offload mode from switcheroo or prime-select.
    fn detect_prime_mode() -> String {
        // Check switcheroo
        let switcheroo = "/sys/kernel/debug/vgaswitcheroo/switch";
        if let Ok(content) = std::fs::read_to_string(switcheroo) {
            if content.contains("DynPwr") {
                return "on-demand".into();
            }
            if content.contains("DynOff") {
                return "integrated".into();
            }
        }
        // Check for NVIDIA prime-select via /etc/prime-discrete
        if std::path::Path::new("/etc/prime-discrete").exists() {
            return "nvidia".into();
        }
        "on-demand".into()
    }
}

impl TuxedoIO for SysFsTuxedoIO {
    fn set_fan_speed_percent(&self, fan_idx: i32, speed: i32) -> Result<(), AttributeError> {
        let iface = self.find_fan_interface()?;
        let pwm_value = (speed.clamp(0, 100) as f64 * 255.0 / 100.0).round() as u32;

        match iface {
            FanInterface::TuxedoPlatform(path) => {
                let fan_num = fan_idx + 1; // tuxedo uses 1-based: fan1_pwm, fan2_pwm
                let enable_path = format!("{}/fan{}_pwm_enable", path, fan_num);
                Self::write_sysfs(&enable_path, "1")?; // 1 = manual
                let pwm_path = format!("{}/fan{}_pwm", path, fan_num);
                Self::write_sysfs(&pwm_path, &pwm_value.to_string())
            }
            FanInterface::GenericHwmon(hwmon) => {
                let fan_num = fan_idx + 1; // hwmon uses 1-based: pwm1, pwm2
                let enable_path = format!("{}/pwm{}_enable", hwmon, fan_num);
                Self::write_sysfs(&enable_path, "1")?;
                let pwm_path = format!("{}/pwm{}", hwmon, fan_num);
                Self::write_sysfs(&pwm_path, &pwm_value.to_string())
            }
        }
    }

    fn get_fan_speed_percent(&self, fan_idx: i32) -> Result<i32, AttributeError> {
        let iface = self.find_fan_interface()?;

        let pwm_str = match iface {
            FanInterface::TuxedoPlatform(path) => {
                let fan_num = fan_idx + 1;
                Self::read_sysfs(&format!("{}/fan{}_pwm", path, fan_num))?
            }
            FanInterface::GenericHwmon(hwmon) => {
                let fan_num = fan_idx + 1;
                Self::read_sysfs(&format!("{}/pwm{}", hwmon, fan_num))?
            }
        };

        let pwm: f64 = pwm_str.parse().map_err(|_| {
            AttributeError::HardwareError(format!("Invalid PWM value: {}", pwm_str))
        })?;
        Ok((pwm * 100.0 / 255.0).round() as i32)
    }

    fn get_fan_rpm(&self, fan_idx: i32) -> Result<u32, AttributeError> {
        let hwmon = self.find_fan_hwmon()?;
        // tuxedo hwmon uses 0-based (fan0_input), standard uses 1-based (fan1_input)
        let path_0based = format!("{}/fan{}_input", hwmon, fan_idx);
        let path_1based = format!("{}/fan{}_input", hwmon, fan_idx + 1);
        let path = if std::path::Path::new(&path_0based).exists() {
            path_0based
        } else {
            path_1based
        };
        let rpm_str = Self::read_sysfs(&path)?;
        rpm_str.parse().map_err(|_| {
            AttributeError::HardwareError(format!("Invalid RPM value: {}", rpm_str))
        })
    }

    fn set_webcam_status(&self, enabled: bool) -> Result<(), AttributeError> {
        // Find webcam USB device. Common approach: look for video-class USB devices.
        // For now, scan /sys/bus/usb/devices/*/bInterfaceClass for 0e (video).
        let usb_base = "/sys/bus/usb/devices";
        let entries = std::fs::read_dir(usb_base).map_err(|e| {
            AttributeError::NotFound(format!("{}: {}", usb_base, e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            let product = path.join("product");
            if let Ok(name) = std::fs::read_to_string(&product) {
                let name_lower = name.trim().to_lowercase();
                if name_lower.contains("camera") || name_lower.contains("webcam") {
                    let dev_id = entry.file_name().to_string_lossy().to_string();
                    let action = if enabled { "bind" } else { "unbind" };
                    let driver_path = format!("/sys/bus/usb/drivers/usb/{}", action);
                    return Self::write_sysfs(&driver_path, &dev_id);
                }
            }
        }

        Err(AttributeError::NotFound("No USB webcam device found".to_string()))
    }

    fn get_cpu_temperature(&self) -> Result<f64, AttributeError> {
        // Try thermal zones, prefer "x86_pkg_temp" or "coretemp", fall back to zone0
        let thermal_base = "/sys/class/thermal";
        if let Ok(entries) = std::fs::read_dir(thermal_base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && !name.starts_with("thermal_zone")
                {
                    continue;
                }
                if let Ok(ttype) = std::fs::read_to_string(path.join("type")) {
                    let ttype = ttype.trim();
                    if ttype == "x86_pkg_temp" || ttype.starts_with("coretemp") {
                        let temp_str = Self::read_sysfs(
                            &path.join("temp").to_string_lossy(),
                        )?;
                        let millideg: f64 = temp_str.parse().map_err(|_| {
                            AttributeError::HardwareError(format!(
                                "Invalid temp value: {}",
                                temp_str
                            ))
                        })?;
                        return Ok(millideg / 1000.0);
                    }
                }
            }
        }

        // Fallback: thermal_zone0
        let temp_str = Self::read_sysfs("/sys/class/thermal/thermal_zone0/temp")?;
        let millideg: f64 = temp_str.parse().map_err(|_| {
            AttributeError::HardwareError(format!("Invalid temp value: {}", temp_str))
        })?;
        Ok(millideg / 1000.0)
    }

    fn get_cpu_frequency_mhz(&self, cpu_idx: i32) -> Result<f64, AttributeError> {
        let path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq",
            cpu_idx
        );
        let khz_str = Self::read_sysfs(&path)?;
        let khz: f64 = khz_str.parse().map_err(|_| {
            AttributeError::HardwareError(format!("Invalid frequency value: {}", khz_str))
        })?;
        Ok(khz / 1000.0)
    }

    fn get_cpu_core_count(&self) -> Result<usize, AttributeError> {
        // Read /sys/devices/system/cpu/present — returns range like "0-7"
        let present = Self::read_sysfs("/sys/devices/system/cpu/present")?;
        // Parse "N-M" or "N"
        if let Some((_, end)) = present.split_once('-') {
            let max: usize = end.parse().map_err(|_| {
                AttributeError::HardwareError(format!(
                    "Invalid CPU present range: {}",
                    present
                ))
            })?;
            Ok(max + 1)
        } else {
            Ok(1)
        }
    }

    fn is_ac_power(&self) -> Result<bool, AttributeError> {
        let ac_path = self.find_ac_supply()?;
        let online = Self::read_sysfs(&format!("{}/online", ac_path))?;
        Ok(online == "1")
    }

    fn set_cpu_governor(&self, governor: &str) -> Result<(), AttributeError> {
        let cores = self.get_cpu_core_count()?;
        for i in 0..cores {
            let path = format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_governor",
                i
            );
            Self::write_sysfs(&path, governor)?;
        }
        Ok(())
    }

    fn set_cpu_turbo(&self, enabled: bool) -> Result<(), AttributeError> {
        // Intel: no_turbo (1 = disabled, 0 = enabled — inverted logic)
        let intel_path = "/sys/devices/system/cpu/intel_pstate/no_turbo";
        if std::path::Path::new(intel_path).exists() {
            return Self::write_sysfs(intel_path, if enabled { "0" } else { "1" });
        }
        // AMD / generic: boost (1 = enabled, 0 = disabled)
        let boost_path = "/sys/devices/system/cpu/cpufreq/boost";
        if std::path::Path::new(boost_path).exists() {
            return Self::write_sysfs(boost_path, if enabled { "1" } else { "0" });
        }
        Err(AttributeError::NotFound(
            "No turbo/boost control found (intel_pstate/no_turbo or cpufreq/boost)".to_string(),
        ))
    }

    fn set_cpu_energy_perf(&self, preference: &str) -> Result<(), AttributeError> {
        let cores = self.get_cpu_core_count()?;
        for i in 0..cores {
            let path = format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/energy_performance_preference",
                i
            );
            if std::path::Path::new(&path).exists() {
                Self::write_sysfs(&path, preference)?;
            }
        }
        Ok(())
    }

    fn set_charge_start_threshold(&self, percent: u8) -> Result<(), AttributeError> {
        let bat_path = self.find_battery()?;
        Self::write_sysfs(
            &format!("{}/charge_control_start_threshold", bat_path),
            &percent.to_string(),
        )
    }

    fn set_charge_end_threshold(&self, percent: u8) -> Result<(), AttributeError> {
        let bat_path = self.find_battery()?;
        Self::write_sysfs(
            &format!("{}/charge_control_end_threshold", bat_path),
            &percent.to_string(),
        )
    }

    fn get_charge_thresholds(&self) -> Result<(u8, u8), AttributeError> {
        let bat_path = self.find_battery()?;
        let start = Self::read_sysfs(&format!("{}/charge_control_start_threshold", bat_path))?;
        let end = Self::read_sysfs(&format!("{}/charge_control_end_threshold", bat_path))?;
        let start: u8 = start.parse().unwrap_or(0);
        let end: u8 = end.parse().unwrap_or(100);
        Ok((start, end))
    }

    fn set_keyboard_brightness(&self, brightness: u8) -> Result<(), AttributeError> {
        match self.find_keyboard()? {
            KeyboardInterface::LedClass { path, .. } => {
                Self::write_sysfs(
                    &format!("{}/brightness", path),
                    &brightness.to_string(),
                )
            }
            KeyboardInterface::TuxedoPlatform => {
                Self::write_sysfs(
                    "/sys/devices/platform/tuxedo_keyboard/brightness",
                    &brightness.to_string(),
                )
            }
        }
    }

    fn set_keyboard_color(&self, color: &str) -> Result<(), AttributeError> {
        let color = color.trim_start_matches('#');
        match self.find_keyboard()? {
            KeyboardInterface::LedClass { path, is_rgb } => {
                if is_rgb {
                    // RGB LED class uses multi_intensity: "R G B" (0-255 each)
                    if color.len() >= 6 {
                        let r = u8::from_str_radix(&color[0..2], 16).unwrap_or(255);
                        let g = u8::from_str_radix(&color[2..4], 16).unwrap_or(255);
                        let b = u8::from_str_radix(&color[4..6], 16).unwrap_or(255);
                        Self::write_sysfs(
                            &format!("{}/multi_intensity", path),
                            &format!("{} {} {}", r, g, b),
                        )
                    } else {
                        Err(AttributeError::HardwareError(
                            format!("Invalid color hex: {}", color),
                        ))
                    }
                } else {
                    // White-only keyboard — color changes not supported
                    Ok(())
                }
            }
            KeyboardInterface::TuxedoPlatform => {
                Self::write_sysfs(
                    "/sys/devices/platform/tuxedo_keyboard/color_left",
                    color,
                )
            }
        }
    }

    fn set_keyboard_mode(&self, mode: &str) -> Result<(), AttributeError> {
        match self.find_keyboard()? {
            KeyboardInterface::LedClass { .. } => {
                // LED class doesn't have a separate mode file; mode is
                // controlled via triggers or is hardware-fixed.
                Ok(())
            }
            KeyboardInterface::TuxedoPlatform => {
                Self::write_sysfs(
                    "/sys/devices/platform/tuxedo_keyboard/mode",
                    mode,
                )
            }
        }
    }

    fn get_gpu_info(&self) -> Result<GpuInfoData, AttributeError> {
        Ok(Self::scan_gpus())
    }

    fn get_display_brightness(&self) -> Result<(u32, u32), AttributeError> {
        let bl = self.find_backlight()?;
        let current_str = Self::read_sysfs(&format!("{}/actual_brightness", bl))?;
        let max_str = Self::read_sysfs(&format!("{}/max_brightness", bl))?;
        let current: u32 = current_str.parse().map_err(|_| {
            AttributeError::HardwareError(format!("Invalid brightness: {}", current_str))
        })?;
        let max: u32 = max_str.parse().map_err(|_| {
            AttributeError::HardwareError(format!("Invalid max brightness: {}", max_str))
        })?;
        Ok((current, max))
    }

    fn set_display_brightness(&self, value: u32) -> Result<(), AttributeError> {
        let bl = self.find_backlight()?;
        Self::write_sysfs(&format!("{}/brightness", bl), &value.to_string())
    }

    fn get_fan_count(&self) -> Result<usize, AttributeError> {
        let iface = self.find_fan_interface()?;
        let mut count = 0;
        match iface {
            FanInterface::TuxedoPlatform(path) => {
                for i in 1..=8 {
                    if std::path::Path::new(&format!("{}/fan{}_pwm", path, i)).exists() {
                        count = i as usize;
                    } else {
                        break;
                    }
                }
            }
            FanInterface::GenericHwmon(hwmon) => {
                for i in 1..=8 {
                    if std::path::Path::new(&format!("{}/pwm{}", hwmon, i)).exists() {
                        count = i as usize;
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(count)
    }
}

pub struct MockTuxedoIO {
    pub(crate) fan_speeds: RwLock<HashMap<i32, i32>>,
    pub(crate) webcam_status: RwLock<bool>,
    pub(crate) cpu_temperature: RwLock<f64>,
    pub(crate) cpu_frequencies: RwLock<HashMap<i32, f64>>,
    pub(crate) cpu_core_count: RwLock<usize>,
    pub(crate) ac_power: RwLock<bool>,
    pub(crate) cpu_governor: RwLock<String>,
    pub(crate) cpu_turbo: RwLock<bool>,
    pub(crate) cpu_energy_perf: RwLock<String>,
    pub(crate) charge_start: RwLock<u8>,
    pub(crate) charge_end: RwLock<u8>,
    pub(crate) kbd_brightness: RwLock<u8>,
    pub(crate) kbd_color: RwLock<String>,
    pub(crate) kbd_mode: RwLock<String>,
    pub(crate) gpu_info: RwLock<GpuInfoData>,
    pub(crate) display_brightness: RwLock<u32>,
    pub(crate) display_max_brightness: RwLock<u32>,
    pub(crate) fan_count: RwLock<usize>,
}

impl Default for MockTuxedoIO {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTuxedoIO {
    pub fn new() -> Self {
        Self {
            fan_speeds: RwLock::new(HashMap::new()),
            webcam_status: RwLock::new(false),
            cpu_temperature: RwLock::new(45.0),
            cpu_frequencies: RwLock::new(HashMap::new()),
            cpu_core_count: RwLock::new(4),
            ac_power: RwLock::new(true),
            cpu_governor: RwLock::new("powersave".to_string()),
            cpu_turbo: RwLock::new(true),
            cpu_energy_perf: RwLock::new("balance_performance".to_string()),
            charge_start: RwLock::new(0),
            charge_end: RwLock::new(100),
            kbd_brightness: RwLock::new(50),
            kbd_color: RwLock::new("ffffff".to_string()),
            kbd_mode: RwLock::new("0".to_string()),
            gpu_info: RwLock::new(GpuInfoData::default()),
            display_brightness: RwLock::new(200),
            display_max_brightness: RwLock::new(255),
            fan_count: RwLock::new(1),
        }
    }
}

impl TuxedoIO for MockTuxedoIO {
    fn set_fan_speed_percent(&self, fan_idx: i32, speed: i32) -> Result<(), AttributeError> {
        self.fan_speeds.write().unwrap().insert(fan_idx, speed);
        Ok(())
    }

    fn get_fan_speed_percent(&self, fan_idx: i32) -> Result<i32, AttributeError> {
        let speed = self
            .fan_speeds
            .read()
            .unwrap()
            .get(&fan_idx)
            .copied()
            .unwrap_or(0);
        Ok(speed)
    }

    fn get_fan_rpm(&self, _fan_idx: i32) -> Result<u32, AttributeError> {
        Ok(1200) // fixed mock RPM
    }

    fn set_webcam_status(&self, enabled: bool) -> Result<(), AttributeError> {
        *self.webcam_status.write().unwrap() = enabled;
        Ok(())
    }

    fn get_cpu_temperature(&self) -> Result<f64, AttributeError> {
        Ok(*self.cpu_temperature.read().unwrap())
    }

    fn get_cpu_frequency_mhz(&self, cpu_idx: i32) -> Result<f64, AttributeError> {
        Ok(self
            .cpu_frequencies
            .read()
            .unwrap()
            .get(&cpu_idx)
            .copied()
            .unwrap_or(2000.0))
    }

    fn get_cpu_core_count(&self) -> Result<usize, AttributeError> {
        Ok(*self.cpu_core_count.read().unwrap())
    }

    fn is_ac_power(&self) -> Result<bool, AttributeError> {
        Ok(*self.ac_power.read().unwrap())
    }

    fn set_cpu_governor(&self, governor: &str) -> Result<(), AttributeError> {
        *self.cpu_governor.write().unwrap() = governor.to_string();
        Ok(())
    }

    fn set_cpu_turbo(&self, enabled: bool) -> Result<(), AttributeError> {
        *self.cpu_turbo.write().unwrap() = enabled;
        Ok(())
    }

    fn set_cpu_energy_perf(&self, preference: &str) -> Result<(), AttributeError> {
        *self.cpu_energy_perf.write().unwrap() = preference.to_string();
        Ok(())
    }

    fn set_charge_start_threshold(&self, percent: u8) -> Result<(), AttributeError> {
        *self.charge_start.write().unwrap() = percent;
        Ok(())
    }

    fn set_charge_end_threshold(&self, percent: u8) -> Result<(), AttributeError> {
        *self.charge_end.write().unwrap() = percent;
        Ok(())
    }

    fn get_charge_thresholds(&self) -> Result<(u8, u8), AttributeError> {
        let start = *self.charge_start.read().unwrap();
        let end = *self.charge_end.read().unwrap();
        Ok((start, end))
    }

    fn set_keyboard_brightness(&self, brightness: u8) -> Result<(), AttributeError> {
        *self.kbd_brightness.write().unwrap() = brightness;
        Ok(())
    }

    fn set_keyboard_color(&self, color: &str) -> Result<(), AttributeError> {
        *self.kbd_color.write().unwrap() = color.trim_start_matches('#').to_string();
        Ok(())
    }

    fn set_keyboard_mode(&self, mode: &str) -> Result<(), AttributeError> {
        *self.kbd_mode.write().unwrap() = mode.to_string();
        Ok(())
    }

    fn get_gpu_info(&self) -> Result<GpuInfoData, AttributeError> {
        Ok(self.gpu_info.read().unwrap().clone())
    }

    fn get_display_brightness(&self) -> Result<(u32, u32), AttributeError> {
        let current = *self.display_brightness.read().unwrap();
        let max = *self.display_max_brightness.read().unwrap();
        Ok((current, max))
    }

    fn set_display_brightness(&self, value: u32) -> Result<(), AttributeError> {
        *self.display_brightness.write().unwrap() = value;
        Ok(())
    }

    fn get_fan_count(&self) -> Result<usize, AttributeError> {
        Ok(*self.fan_count.read().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_hardware() {
        let mock = MockTuxedoIO::new();
        assert!(mock.set_fan_speed_percent(0, 80).is_ok());

        let speeds = mock.fan_speeds.read().unwrap();
        assert_eq!(speeds.get(&0), Some(&80));
        drop(speeds); // Drop read lock before writing again
        assert!(mock.set_webcam_status(true).is_ok());
        assert!(*mock.webcam_status.read().unwrap());
    }

    #[test]
    fn test_mock_cpu_and_power() {
        let mock = MockTuxedoIO::new();
        assert_eq!(mock.get_cpu_temperature().unwrap(), 45.0);
        assert_eq!(mock.get_cpu_core_count().unwrap(), 4);
        assert_eq!(mock.get_cpu_frequency_mhz(0).unwrap(), 2000.0);
        assert!(mock.is_ac_power().unwrap());

        *mock.cpu_temperature.write().unwrap() = 72.5;
        assert_eq!(mock.get_cpu_temperature().unwrap(), 72.5);

        *mock.ac_power.write().unwrap() = false;
        assert!(!mock.is_ac_power().unwrap());
    }

    #[test]
    fn test_sysfs_read_nonexistent() {
        let err = SysFsTuxedoIO::read_sysfs("/tmp/nonexistent_sysfs_path_12345");
        assert!(matches!(err, Err(AttributeError::NotFound(_))));
    }

    #[test]
    fn test_sysfs_read_write_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test_value");
        std::fs::write(&file, "42\n").unwrap();

        let val = SysFsTuxedoIO::read_sysfs(file.to_str().unwrap()).unwrap();
        assert_eq!(val, "42");

        SysFsTuxedoIO::write_sysfs(file.to_str().unwrap(), "100").unwrap();
        let val = SysFsTuxedoIO::read_sysfs(file.to_str().unwrap()).unwrap();
        assert_eq!(val, "100");
    }

    #[test]
    fn test_sysfs_find_fan_hwmon() {
        let dir = tempfile::tempdir().unwrap();
        // Create a fake hwmon device with pwm1
        let hwmon0 = dir.path().join("hwmon0");
        std::fs::create_dir(&hwmon0).unwrap();
        std::fs::write(hwmon0.join("pwm1"), "128").unwrap();
        std::fs::write(hwmon0.join("pwm1_enable"), "1").unwrap();

        let sysfs = SysFsTuxedoIO::new();
        *sysfs.fan_interface.write().unwrap() =
            Some(FanInterface::GenericHwmon(hwmon0.to_string_lossy().to_string()));

        let speed = sysfs.get_fan_speed_percent(0).unwrap();
        // 128/255 * 100 = 50.2 → rounds to 50
        assert_eq!(speed, 50);
    }

    #[test]
    fn test_sysfs_fan_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let hwmon0 = dir.path().join("hwmon0");
        std::fs::create_dir(&hwmon0).unwrap();
        std::fs::write(hwmon0.join("pwm1"), "0").unwrap();
        std::fs::write(hwmon0.join("pwm1_enable"), "0").unwrap();

        let sysfs = SysFsTuxedoIO::new();
        *sysfs.fan_interface.write().unwrap() =
            Some(FanInterface::GenericHwmon(hwmon0.to_string_lossy().to_string()));

        sysfs.set_fan_speed_percent(0, 75).unwrap();

        // Check enable was set to manual
        let enable = std::fs::read_to_string(hwmon0.join("pwm1_enable")).unwrap();
        assert_eq!(enable, "1");

        // 75% of 255 = 191.25 → 191
        let pwm = std::fs::read_to_string(hwmon0.join("pwm1")).unwrap();
        assert_eq!(pwm, "191");

        // Read back
        let speed = sysfs.get_fan_speed_percent(0).unwrap();
        assert_eq!(speed, 75);
    }

    #[test]
    fn test_sysfs_tuxedo_platform_fan() {
        let dir = tempfile::tempdir().unwrap();
        let platform = dir.path().join("tuxedo_fan_control");
        std::fs::create_dir(&platform).unwrap();
        std::fs::write(platform.join("fan1_pwm"), "0").unwrap();
        std::fs::write(platform.join("fan1_pwm_enable"), "0").unwrap();
        std::fs::write(platform.join("fan2_pwm"), "0").unwrap();
        std::fs::write(platform.join("fan2_pwm_enable"), "0").unwrap();

        let sysfs = SysFsTuxedoIO::new();
        *sysfs.fan_interface.write().unwrap() =
            Some(FanInterface::TuxedoPlatform(platform.to_string_lossy().to_string()));

        // Set fan 0 (maps to fan1_pwm) to 60%
        sysfs.set_fan_speed_percent(0, 60).unwrap();
        let pwm = std::fs::read_to_string(platform.join("fan1_pwm")).unwrap();
        assert_eq!(pwm, "153"); // 60% of 255 = 153

        // Set fan 1 (maps to fan2_pwm) to 80%
        sysfs.set_fan_speed_percent(1, 80).unwrap();
        let pwm = std::fs::read_to_string(platform.join("fan2_pwm")).unwrap();
        assert_eq!(pwm, "204"); // 80% of 255 = 204

        // Read back
        assert_eq!(sysfs.get_fan_speed_percent(0).unwrap(), 60);
        assert_eq!(sysfs.get_fan_speed_percent(1).unwrap(), 80);

        // Fan count
        assert_eq!(sysfs.get_fan_count().unwrap(), 2);
    }

    #[test]
    fn test_sysfs_cpu_temperature_live() {
        // This test reads the actual system thermal zone — skip if not available
        let sysfs = SysFsTuxedoIO::new();
        if let Ok(temp) = sysfs.get_cpu_temperature() {
            assert!(temp > 0.0 && temp < 150.0, "Temp out of range: {}", temp);
        }
    }

    #[test]
    fn test_sysfs_cpu_core_count_live() {
        let sysfs = SysFsTuxedoIO::new();
        if let Ok(count) = sysfs.get_cpu_core_count() {
            assert!(count >= 1, "Core count should be >= 1: {}", count);
        }
    }

    #[test]
    fn test_sysfs_cpu_frequency_live() {
        let sysfs = SysFsTuxedoIO::new();
        if let Ok(freq) = sysfs.get_cpu_frequency_mhz(0) {
            assert!(freq > 100.0 && freq < 10000.0, "Freq out of range: {}", freq);
        }
    }

    #[test]
    fn test_attribute_error_variants() {
        let hw = AttributeError::HardwareError("test".into());
        assert!(hw.to_string().contains("Hardware error"));

        let perm = AttributeError::PermissionDenied("/sys/test".into());
        assert!(perm.to_string().contains("Permission denied"));
        assert!(perm.to_string().contains("root"));

        let nf = AttributeError::NotFound("/sys/test".into());
        assert!(nf.to_string().contains("Not found"));
    }

    #[test]
    fn test_mock_cpu_control() {
        let mock = MockTuxedoIO::new();

        assert!(mock.set_cpu_governor("performance").is_ok());
        assert_eq!(*mock.cpu_governor.read().unwrap(), "performance");

        assert!(mock.set_cpu_turbo(false).is_ok());
        assert!(!*mock.cpu_turbo.read().unwrap());

        assert!(mock.set_cpu_energy_perf("power").is_ok());
        assert_eq!(*mock.cpu_energy_perf.read().unwrap(), "power");
    }

    #[test]
    fn test_mock_charge_thresholds() {
        let mock = MockTuxedoIO::new();

        assert!(mock.set_charge_start_threshold(20).is_ok());
        assert!(mock.set_charge_end_threshold(80).is_ok());

        let (start, end) = mock.get_charge_thresholds().unwrap();
        assert_eq!(start, 20);
        assert_eq!(end, 80);
    }

    #[test]
    fn test_mock_keyboard() {
        let mock = MockTuxedoIO::new();

        assert!(mock.set_keyboard_brightness(75).is_ok());
        assert_eq!(*mock.kbd_brightness.read().unwrap(), 75);

        assert!(mock.set_keyboard_color("#ff0000").is_ok());
        assert_eq!(*mock.kbd_color.read().unwrap(), "ff0000");

        assert!(mock.set_keyboard_mode("breathing").is_ok());
        assert_eq!(*mock.kbd_mode.read().unwrap(), "breathing");
    }

    #[test]
    fn test_mock_gpu_info() {
        let mock = MockTuxedoIO::new();
        let info = mock.get_gpu_info().unwrap();
        assert!(!info.dgpu_name.is_empty());
        assert!(!info.igpu_name.is_empty());
    }

    #[test]
    fn test_mock_display_brightness() {
        let mock = MockTuxedoIO::new();

        let (current, max) = mock.get_display_brightness().unwrap();
        assert_eq!(current, 200);
        assert_eq!(max, 255);

        assert!(mock.set_display_brightness(128).is_ok());
        let (current, _) = mock.get_display_brightness().unwrap();
        assert_eq!(current, 128);
    }

    #[test]
    fn test_mock_fan_count() {
        let mock = MockTuxedoIO::new();
        assert_eq!(mock.get_fan_count().unwrap(), 1);

        *mock.fan_count.write().unwrap() = 3;
        assert_eq!(mock.get_fan_count().unwrap(), 3);
    }

    #[test]
    fn test_sysfs_fan_count() {
        let dir = tempfile::tempdir().unwrap();
        let hwmon0 = dir.path().join("hwmon0");
        std::fs::create_dir(&hwmon0).unwrap();
        std::fs::write(hwmon0.join("pwm1"), "128").unwrap();
        std::fs::write(hwmon0.join("pwm2"), "128").unwrap();
        std::fs::write(hwmon0.join("pwm1_enable"), "1").unwrap();

        let sysfs = SysFsTuxedoIO::new();
        *sysfs.fan_interface.write().unwrap() =
            Some(FanInterface::GenericHwmon(hwmon0.to_string_lossy().to_string()));

        let count = sysfs.get_fan_count().unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_sysfs_backlight_caching() {
        let dir = tempfile::tempdir().unwrap();
        let bl = dir.path().join("intel_backlight");
        std::fs::create_dir(&bl).unwrap();
        std::fs::write(bl.join("actual_brightness"), "100\n").unwrap();
        std::fs::write(bl.join("max_brightness"), "1000\n").unwrap();
        std::fs::write(bl.join("brightness"), "100").unwrap();

        let sysfs = SysFsTuxedoIO::new();
        *sysfs.backlight_path.write().unwrap() = Some(bl.to_string_lossy().to_string());

        let (current, max) = sysfs.get_display_brightness().unwrap();
        assert_eq!(current, 100);
        assert_eq!(max, 1000);

        sysfs.set_display_brightness(500).unwrap();
        let written = std::fs::read_to_string(bl.join("brightness")).unwrap();
        assert_eq!(written, "500");
    }
}
