//! Quick diagnostic: probe `/dev/tuxedo_io` hwcheck ioctls.
//!
//! The C header uses `_IOR(MAGIC, NR, int32_t*)` — the sizeof is on
//! a *pointer* (8 bytes on x86_64), NOT on the value (4 bytes).
//! We compare both encodings here.
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use nix::ioctl_read;

const IOCTL_MAGIC: u8 = 0xEC;
const MAGIC_READ_UW: u8 = IOCTL_MAGIC + 3;

// --- Wrong (size=4, what we had) ---
ioctl_read!(r_hwcheck_cl_bad, IOCTL_MAGIC, 0x05, i32);
ioctl_read!(r_hwcheck_uw_bad, IOCTL_MAGIC, 0x06, i32);
ioctl_read!(r_uw_fanspeed_bad, MAGIC_READ_UW, 0x10, i32);

// --- Correct (size=8, matching C `int32_t*` = pointer) ---
ioctl_read!(r_hwcheck_cl_ptr, IOCTL_MAGIC, 0x05, usize);
ioctl_read!(r_hwcheck_uw_ptr, IOCTL_MAGIC, 0x06, usize);
ioctl_read!(r_uw_fanspeed_ptr, MAGIC_READ_UW, 0x10, usize);
ioctl_read!(r_uw_fanspeed2_ptr, MAGIC_READ_UW, 0x11, usize);
ioctl_read!(r_uw_fan_temp_ptr, MAGIC_READ_UW, 0x12, usize);

fn main() {
    let fd = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tuxedo_io")
        .expect("Failed to open /dev/tuxedo_io");
    let raw = fd.as_raw_fd();

    println!("=== size=4 (i32) — WRONG ===");
    let mut val: i32 = -1;
    let res = unsafe { r_hwcheck_cl_bad(raw, &mut val) };
    println!("r_hwcheck_cl: result={:?}, val={}", res, val);
    val = -1;
    let res = unsafe { r_hwcheck_uw_bad(raw, &mut val) };
    println!("r_hwcheck_uw: result={:?}, val={}", res, val);
    val = -1;
    let res = unsafe { r_uw_fanspeed_bad(raw, &mut val) };
    println!("r_uw_fanspeed: result={:?}, val={}", res, val);

    println!("\n=== size=8 (usize/ptr) — CORRECT ===");
    let mut pval: usize = 0xDEAD;
    let res = unsafe { r_hwcheck_cl_ptr(raw, &mut pval) };
    println!("r_hwcheck_cl: result={:?}, val={}", res, pval);
    pval = 0xDEAD;
    let res = unsafe { r_hwcheck_uw_ptr(raw, &mut pval) };
    println!("r_hwcheck_uw: result={:?}, val={}", res, pval);
    pval = 0xDEAD;
    let res = unsafe { r_uw_fanspeed_ptr(raw, &mut pval) };
    println!("r_uw_fanspeed(0): result={:?}, val={}", res, pval);
    pval = 0xDEAD;
    let res = unsafe { r_uw_fanspeed2_ptr(raw, &mut pval) };
    println!("r_uw_fanspeed2(1): result={:?}, val={}", res, pval);
    pval = 0xDEAD;
    let res = unsafe { r_uw_fan_temp_ptr(raw, &mut pval) };
    println!("r_uw_fan_temp: result={:?}, val={}", res, pval);
}
