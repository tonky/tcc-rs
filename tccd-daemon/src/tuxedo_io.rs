//! Pure Rust interface to `/dev/tuxedo_io` via ioctl.
//!
//! Talks directly to the `tuxedo_io` kernel module without any C/C++
//! FFI or bindgen — just `nix::ioctl_read!`/`ioctl_write_ptr!` macros
//! wrapping `libc::ioctl()`.

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;

use nix::{ioctl_none, ioctl_read, ioctl_write_ptr};

use crate::io::{AttributeError, TuxedoIO};
use crate::profiles::GpuInfoData;

// ── Magic numbers from tuxedo_io_ioctl.h ────────────────────────────

const IOCTL_MAGIC: u8 = 0xEC;
const MAGIC_READ_CL: u8 = IOCTL_MAGIC + 1;  // 0xED
const MAGIC_WRITE_CL: u8 = IOCTL_MAGIC + 2; // 0xEE
const MAGIC_READ_UW: u8 = IOCTL_MAGIC + 3;  // 0xEF
const MAGIC_WRITE_UW: u8 = IOCTL_MAGIC + 4; // 0xF0

// ── Ioctl declarations ──────────────────────────────────────────────
//
// IMPORTANT: The C header (`tuxedo_io_ioctl.h`) defines all ioctls
// with pointer types, e.g. `_IOR(MAGIC, NR, int32_t*)`. The `_IOR`
// macro encodes `sizeof(type)` into the ioctl request number. On
// 64-bit, `sizeof(int32_t*) == 8`, not 4. We must use `usize` here
// so that `size_of::<usize>() == 8` matches the kernel's encoding.
// The actual payload is still a 32-bit integer in the low bytes.

// ── General ─────────────────────────────────────────────────────────

ioctl_read!(r_hwcheck_cl, IOCTL_MAGIC, 0x05, usize);
ioctl_read!(r_hwcheck_uw, IOCTL_MAGIC, 0x06, usize);

// ── Clevo read ──────────────────────────────────────────────────────

ioctl_read!(r_cl_faninfo1, MAGIC_READ_CL, 0x10, usize);
ioctl_read!(r_cl_faninfo2, MAGIC_READ_CL, 0x11, usize);
ioctl_read!(r_cl_faninfo3, MAGIC_READ_CL, 0x12, usize);
ioctl_read!(r_cl_webcam_sw, MAGIC_READ_CL, 0x13, usize);

// ── Clevo write ─────────────────────────────────────────────────────

ioctl_write_ptr!(w_cl_fanspeed, MAGIC_WRITE_CL, 0x10, usize);
ioctl_write_ptr!(w_cl_fanauto, MAGIC_WRITE_CL, 0x11, usize);
ioctl_write_ptr!(w_cl_webcam_sw, MAGIC_WRITE_CL, 0x12, usize);
ioctl_write_ptr!(w_cl_perf_profile, MAGIC_WRITE_CL, 0x15, usize);

// ── Uniwill read ────────────────────────────────────────────────────

ioctl_read!(r_uw_fanspeed, MAGIC_READ_UW, 0x10, usize);
ioctl_read!(r_uw_fanspeed2, MAGIC_READ_UW, 0x11, usize);
ioctl_read!(r_uw_fan_temp, MAGIC_READ_UW, 0x12, usize);
ioctl_read!(r_uw_fan_temp2, MAGIC_READ_UW, 0x13, usize);
ioctl_read!(r_uw_fans_min_speed, MAGIC_READ_UW, 0x17, usize);
ioctl_read!(r_uw_tdp0, MAGIC_READ_UW, 0x18, usize);
ioctl_read!(r_uw_tdp1, MAGIC_READ_UW, 0x19, usize);
ioctl_read!(r_uw_tdp2, MAGIC_READ_UW, 0x1a, usize);
ioctl_read!(r_uw_tdp0_min, MAGIC_READ_UW, 0x1b, usize);
ioctl_read!(r_uw_tdp1_min, MAGIC_READ_UW, 0x1c, usize);
ioctl_read!(r_uw_tdp2_min, MAGIC_READ_UW, 0x1d, usize);
ioctl_read!(r_uw_tdp0_max, MAGIC_READ_UW, 0x1e, usize);
ioctl_read!(r_uw_tdp1_max, MAGIC_READ_UW, 0x1f, usize);
ioctl_read!(r_uw_tdp2_max, MAGIC_READ_UW, 0x20, usize);
ioctl_read!(r_uw_profs_available, MAGIC_READ_UW, 0x21, usize);

// ── Uniwill write ───────────────────────────────────────────────────

ioctl_write_ptr!(w_uw_fanspeed, MAGIC_WRITE_UW, 0x10, usize);
ioctl_write_ptr!(w_uw_fanspeed2, MAGIC_WRITE_UW, 0x11, usize);
ioctl_write_ptr!(w_uw_mode, MAGIC_WRITE_UW, 0x12, usize);
ioctl_write_ptr!(w_uw_mode_enable, MAGIC_WRITE_UW, 0x13, usize);
ioctl_none!(w_uw_fanauto, MAGIC_WRITE_UW, 0x14);
ioctl_write_ptr!(w_uw_tdp0, MAGIC_WRITE_UW, 0x15, usize);
ioctl_write_ptr!(w_uw_tdp1, MAGIC_WRITE_UW, 0x16, usize);
ioctl_write_ptr!(w_uw_tdp2, MAGIC_WRITE_UW, 0x17, usize);
ioctl_write_ptr!(w_uw_perf_prof, MAGIC_WRITE_UW, 0x18, usize);

// ── Hardware family ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HwFamily {
    Clevo,
    Uniwill,
}

// ── IoctlTuxedoIO ───────────────────────────────────────────────────

/// Hardware IO via `/dev/tuxedo_io` ioctl interface.
/// Requires the `tuxedo_io` kernel module to be loaded.
pub struct IoctlTuxedoIO {
    fd: File,
    family: HwFamily,
    /// Delegate to SysFsTuxedoIO for features not covered by ioctl
    /// (CPU governor, charging, display backlight, GPU info, etc.)
    sysfs: crate::io::SysFsTuxedoIO,
}

impl IoctlTuxedoIO {
    /// Open `/dev/tuxedo_io` and detect hardware family.
    pub fn open() -> Result<Self, AttributeError> {
        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tuxedo_io")
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => AttributeError::NotFound(
                    "/dev/tuxedo_io not found — is tuxedo_io module loaded?".into(),
                ),
                std::io::ErrorKind::PermissionDenied => {
                    AttributeError::PermissionDenied("/dev/tuxedo_io".into())
                }
                _ => AttributeError::HardwareError(format!("/dev/tuxedo_io: {}", e)),
            })?;

        let family = Self::detect_family(&fd)?;

        Ok(Self {
            fd,
            family,
            sysfs: crate::io::SysFsTuxedoIO::new(),
        })
    }

    fn detect_family(fd: &File) -> Result<HwFamily, AttributeError> {
        let raw = fd.as_raw_fd();
        let mut val: usize = 0;

        // Check Clevo first
        if unsafe { r_hwcheck_cl(raw, &mut val) }.is_ok() && val == 1 {
            return Ok(HwFamily::Clevo);
        }

        // Check Uniwill
        val = 0;
        if unsafe { r_hwcheck_uw(raw, &mut val) }.is_ok() && val == 1 {
            return Ok(HwFamily::Uniwill);
        }

        Err(AttributeError::HardwareError(
            "tuxedo_io: neither Clevo nor Uniwill hardware detected".into(),
        ))
    }

    fn ioctl_err(op: &str, e: nix::Error) -> AttributeError {
        AttributeError::HardwareError(format!("ioctl {}: {}", op, e))
    }

    fn raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }

    // ── Clevo fan helpers ───────────────────────────────────────────

    /// Read Clevo fan info for fan_idx (0-2). Returns raw ACPI result.
    fn cl_read_faninfo(&self, fan_idx: i32) -> Result<i32, AttributeError> {
        let mut val: usize = 0;
        let fd = self.raw_fd();
        let res = match fan_idx {
            0 => unsafe { r_cl_faninfo1(fd, &mut val) },
            1 => unsafe { r_cl_faninfo2(fd, &mut val) },
            2 => unsafe { r_cl_faninfo3(fd, &mut val) },
            _ => return Err(AttributeError::NotFound(format!("Clevo fan {} not supported", fan_idx))),
        };
        res.map_err(|e| Self::ioctl_err("R_CL_FANINFO", e))?;
        Ok(val as i32)
    }

    /// Set Clevo fan speeds. Packs up to 3 fan speeds (0-255 each)
    /// into one i32: fan0=low byte, fan1=byte1, fan2=byte2.
    fn cl_set_fan_speeds(&self, speeds: &[u8; 3]) -> Result<(), AttributeError> {
        let packed: usize = speeds[0] as usize
            | (speeds[1] as usize) << 8
            | (speeds[2] as usize) << 16;
        unsafe { w_cl_fanspeed(self.raw_fd(), &packed) }
            .map_err(|e| Self::ioctl_err("W_CL_FANSPEED", e))?;
        Ok(())
    }

    // ── Uniwill fan helpers ─────────────────────────────────────────

    /// Uniwill fan speed range is 0-200 (0xc8).
    const UW_FAN_MAX: i32 = 200;

    fn uw_read_fanspeed(&self, fan_idx: i32) -> Result<i32, AttributeError> {
        let mut val: usize = 0;
        let fd = self.raw_fd();
        let res = match fan_idx {
            0 => unsafe { r_uw_fanspeed(fd, &mut val) },
            1 => unsafe { r_uw_fanspeed2(fd, &mut val) },
            _ => return Err(AttributeError::NotFound(format!("Uniwill fan {} not supported", fan_idx))),
        };
        res.map_err(|e| Self::ioctl_err("R_UW_FANSPEED", e))?;
        Ok(val as i32)
    }

    fn uw_set_fanspeed(&self, fan_idx: i32, raw: i32) -> Result<(), AttributeError> {
        let fd = self.raw_fd();
        let val = raw as usize;
        let res = match fan_idx {
            0 => unsafe { w_uw_fanspeed(fd, &val) },
            1 => unsafe { w_uw_fanspeed2(fd, &val) },
            _ => return Err(AttributeError::NotFound(format!("Uniwill fan {} not supported", fan_idx))),
        };
        res.map_err(|e| Self::ioctl_err("W_UW_FANSPEED", e))?;
        Ok(())
    }

    // ── TDP helpers ─────────────────────────────────────────────────

    /// Read TDP value (Uniwill only). index: 0=PL1, 1=PL2, 2=PL4.
    pub fn get_tdp(&self, index: u8) -> Result<i32, AttributeError> {
        if self.family != HwFamily::Uniwill {
            return Err(AttributeError::NotFound("TDP control is Uniwill-only".into()));
        }
        let mut val: usize = 0;
        let fd = self.raw_fd();
        let res = match index {
            0 => unsafe { r_uw_tdp0(fd, &mut val) },
            1 => unsafe { r_uw_tdp1(fd, &mut val) },
            2 => unsafe { r_uw_tdp2(fd, &mut val) },
            _ => return Err(AttributeError::NotFound(format!("TDP index {} invalid", index))),
        };
        res.map_err(|e| Self::ioctl_err("R_UW_TDP", e))?;
        Ok(val as i32)
    }

    /// Write TDP value (Uniwill only).
    pub fn set_tdp(&self, index: u8, value: i32) -> Result<(), AttributeError> {
        if self.family != HwFamily::Uniwill {
            return Err(AttributeError::NotFound("TDP control is Uniwill-only".into()));
        }
        let fd = self.raw_fd();
        let val = value as usize;
        let res = match index {
            0 => unsafe { w_uw_tdp0(fd, &val) },
            1 => unsafe { w_uw_tdp1(fd, &val) },
            2 => unsafe { w_uw_tdp2(fd, &val) },
            _ => return Err(AttributeError::NotFound(format!("TDP index {} invalid", index))),
        };
        res.map_err(|e| Self::ioctl_err("W_UW_TDP", e))?;
        Ok(())
    }

    /// Get TDP min/max bounds for a given index.
    pub fn get_tdp_bounds(&self, index: u8) -> Result<(i32, i32), AttributeError> {
        if self.family != HwFamily::Uniwill {
            return Err(AttributeError::NotFound("TDP control is Uniwill-only".into()));
        }
        let mut min_val: usize = 0;
        let mut max_val: usize = 0;
        let fd = self.raw_fd();
        let (r_min, r_max) = match index {
            0 => (
                unsafe { r_uw_tdp0_min(fd, &mut min_val) },
                unsafe { r_uw_tdp0_max(fd, &mut max_val) },
            ),
            1 => (
                unsafe { r_uw_tdp1_min(fd, &mut min_val) },
                unsafe { r_uw_tdp1_max(fd, &mut max_val) },
            ),
            2 => (
                unsafe { r_uw_tdp2_min(fd, &mut min_val) },
                unsafe { r_uw_tdp2_max(fd, &mut max_val) },
            ),
            _ => return Err(AttributeError::NotFound(format!("TDP index {} invalid", index))),
        };
        r_min.map_err(|e| Self::ioctl_err("R_UW_TDP_MIN", e))?;
        r_max.map_err(|e| Self::ioctl_err("R_UW_TDP_MAX", e))?;
        Ok((min_val as i32, max_val as i32))
    }

    /// Reset fans to automatic control.
    pub fn set_fans_auto(&self) -> Result<(), AttributeError> {
        let fd = self.raw_fd();
        match self.family {
            HwFamily::Clevo => {
                let val: usize = 0;
                unsafe { w_cl_fanauto(fd, &val) }
                    .map_err(|e| Self::ioctl_err("W_CL_FANAUTO", e))?;
            }
            HwFamily::Uniwill => {
                unsafe { w_uw_fanauto(fd) }
                    .map_err(|e| Self::ioctl_err("W_UW_FANAUTO", e))?;
            }
        }
        Ok(())
    }
}

// ── TuxedoIO trait implementation ───────────────────────────────────

impl TuxedoIO for IoctlTuxedoIO {
    fn set_fan_speed_percent(&self, fan_idx: i32, speed: i32) -> Result<(), AttributeError> {
        let clamped = speed.clamp(0, 100);
        match self.family {
            HwFamily::Clevo => {
                // Clevo packs all 3 fans into one ioctl call.
                // For simplicity, set the requested fan and leave others at current.
                let pwm = (clamped as f64 * 255.0 / 100.0).round() as u8;
                let mut speeds = [0u8; 3];
                // Read current speeds for other fans
                for i in 0..3i32 {
                    if i == fan_idx {
                        speeds[i as usize] = pwm;
                    } else {
                        let info = self.cl_read_faninfo(i).unwrap_or(0);
                        speeds[i as usize] = (info & 0xFF) as u8;
                    }
                }
                self.cl_set_fan_speeds(&speeds)
            }
            HwFamily::Uniwill => {
                let raw = (clamped as f64 * Self::UW_FAN_MAX as f64 / 100.0).round() as i32;
                self.uw_set_fanspeed(fan_idx, raw)
            }
        }
    }

    fn get_fan_speed_percent(&self, fan_idx: i32) -> Result<i32, AttributeError> {
        match self.family {
            HwFamily::Clevo => {
                let info = self.cl_read_faninfo(fan_idx)?;
                let raw = info & 0xFF; // low byte is duty
                Ok((raw as f64 * 100.0 / 255.0).round() as i32)
            }
            HwFamily::Uniwill => {
                let raw = self.uw_read_fanspeed(fan_idx)?;
                Ok((raw as f64 * 100.0 / Self::UW_FAN_MAX as f64).round() as i32)
            }
        }
    }

    fn get_fan_rpm(&self, fan_idx: i32) -> Result<u32, AttributeError> {
        // ioctl doesn't give RPM directly — delegate to sysfs hwmon
        self.sysfs.get_fan_rpm(fan_idx)
    }

    fn set_webcam_status(&self, enabled: bool) -> Result<(), AttributeError> {
        match self.family {
            HwFamily::Clevo => {
                let val: usize = if enabled { 1 } else { 0 };
                unsafe { w_cl_webcam_sw(self.raw_fd(), &val) }
                    .map_err(|e| Self::ioctl_err("W_CL_WEBCAM_SW", e))?;
                Ok(())
            }
            HwFamily::Uniwill => {
                // Uniwill doesn't have webcam ioctl — fall back to USB sysfs
                self.sysfs.set_webcam_status(enabled)
            }
        }
    }

    // ── Delegate to sysfs for generic Linux subsystems ──────────────

    fn get_cpu_temperature(&self) -> Result<f64, AttributeError> {
        self.sysfs.get_cpu_temperature()
    }

    fn get_cpu_frequency_mhz(&self, cpu_idx: i32) -> Result<f64, AttributeError> {
        self.sysfs.get_cpu_frequency_mhz(cpu_idx)
    }

    fn get_cpu_core_count(&self) -> Result<usize, AttributeError> {
        self.sysfs.get_cpu_core_count()
    }

    fn is_ac_power(&self) -> Result<bool, AttributeError> {
        self.sysfs.is_ac_power()
    }

    fn set_cpu_governor(&self, governor: &str) -> Result<(), AttributeError> {
        self.sysfs.set_cpu_governor(governor)
    }

    fn set_cpu_turbo(&self, enabled: bool) -> Result<(), AttributeError> {
        self.sysfs.set_cpu_turbo(enabled)
    }

    fn set_cpu_energy_perf(&self, preference: &str) -> Result<(), AttributeError> {
        self.sysfs.set_cpu_energy_perf(preference)
    }

    fn set_charge_start_threshold(&self, percent: u8) -> Result<(), AttributeError> {
        self.sysfs.set_charge_start_threshold(percent)
    }

    fn set_charge_end_threshold(&self, percent: u8) -> Result<(), AttributeError> {
        self.sysfs.set_charge_end_threshold(percent)
    }

    fn get_charge_thresholds(&self) -> Result<(u8, u8), AttributeError> {
        self.sysfs.get_charge_thresholds()
    }

    fn set_charging_profile(&self, profile: &str) -> Result<(), AttributeError> {
        self.sysfs.set_charging_profile(profile)
    }

    fn get_charging_profile(&self) -> Result<String, AttributeError> {
        self.sysfs.get_charging_profile()
    }

    fn set_charging_priority(&self, priority: &str) -> Result<(), AttributeError> {
        self.sysfs.set_charging_priority(priority)
    }

    fn get_charging_priority(&self) -> Result<String, AttributeError> {
        self.sysfs.get_charging_priority()
    }

    fn set_keyboard_brightness(&self, brightness: u8) -> Result<(), AttributeError> {
        self.sysfs.set_keyboard_brightness(brightness)
    }

    fn set_keyboard_color(&self, color: &str) -> Result<(), AttributeError> {
        self.sysfs.set_keyboard_color(color)
    }

    fn set_keyboard_mode(&self, mode: &str) -> Result<(), AttributeError> {
        self.sysfs.set_keyboard_mode(mode)
    }

    fn get_gpu_info(&self) -> Result<GpuInfoData, AttributeError> {
        self.sysfs.get_gpu_info()
    }

    fn get_display_brightness(&self) -> Result<(u32, u32), AttributeError> {
        self.sysfs.get_display_brightness()
    }

    fn set_display_brightness(&self, value: u32) -> Result<(), AttributeError> {
        self.sysfs.set_display_brightness(value)
    }

    fn get_fan_count(&self) -> Result<usize, AttributeError> {
        match self.family {
            HwFamily::Clevo => Ok(3), // Clevo always exposes 3 fan info channels
            HwFamily::Uniwill => Ok(2), // Uniwill has fanspeed + fanspeed2
        }
    }
}
