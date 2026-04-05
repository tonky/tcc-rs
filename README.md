# tcc-rs — Tuxedo Control Center (Rust)

A ground-up Rust rewrite of [TUXEDO Control Center](https://github.com/tuxedocomputers/tuxedo-control-center), replacing the Node.js daemon + Electron UI with a lightweight terminal interface.

![tcc-rs demo](demo.gif)

## Why rewrite?

The original TCC runs a **root Node.js daemon** and an **Electron desktop app**. This means:

- ~200 MB resident memory for a config utility
- A full Chromium instance bundled for the UI
- C++ native addon (`node-addon-api`) for hardware access
- Node.js running as PID 1-adjacent root service — large attack surface

The Rust rewrite eliminates all of that:

| | Original | tcc-rs |
|---|---|---|
| Daemon | Node.js (root systemd) | Rust + tokio (session or system bus) |
| Hardware IO | C++ addon via node-addon-api | Pure Rust ioctl + sysfs (no C++) |
| UI | Electron + Angular (~200 MB) | ratatui TUI (~5 MB) |
| D-Bus | node-dbus-next | zbus 5 (async, zero-copy) |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        User Space                           │
│                                                             │
│  ┌─────────────┐    D-Bus (session)    ┌────────────────┐   │
│  │  tccd-tui   │◄────────────────────►│  tccd-daemon   │   │
│  │  (ratatui)  │                       │  (tokio+zbus)  │   │
│  └─────────────┘                       │                │   │
│                                        │  ┌───────────┐ │   │
│                                        │  │ Workers   │ │   │
│                                        │  │  fan.rs   │ │   │
│                                        │  │  power.rs │ │   │
│                                        │  └───────────┘ │   │
│                                        │                │   │
│                                        │  ┌───────────┐ │   │
│                                        │  │TuxedoIO   │ │   │
│                                        │  │(trait)    │ │   │
│                                        │  └─────┬─────┘ │   │
│                                        │        │       │   │
│                                        │  ┌─────┴─────┐ │   │
│                                        │  │ProfileStore│ │   │
│                                        │  │(JSON file) │ │   │
│                                        │  └───────────┘ │   │
│                                        └───────┬────────┘   │
│                                        ┌───────┴────────┐   │
│                                        │  Auto-detect   │   │
│                                        └──┬──────────┬──┘   │
├───────────────────────────────────────────┼──────────┼───────┤
│                      Kernel              │          │       │
│                                          ▼          ▼       │
│  ┌─────────────────────┐   ┌──────────────────────────────┐ │
│  │   /dev/tuxedo_io    │   │         sysfs                │ │
│  │   (ioctl, if avail) │   │                              │ │
│  │                     │   │  tuxedo_fan_control/fan*_pwm │ │
│  │  • Fan speed (EC)   │   │  /sys/class/leds/*:keyboard/ │ │
│  │  • Webcam toggle    │   │  hwmon/*, thermal/*, drm/*   │ │
│  │  • TDP (PL1/PL2/PL4)│   │  cpufreq/*, power_supply/*  │ │
│  │  • Perf profiles    │   │  backlight/*                 │ │
│  └─────────────────────┘   └──────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Description |
|---|---|
| `tccd-daemon` | D-Bus service with hardware IO, profile management, fan/power workers |
| `tccd-tui` | Terminal UI (ratatui) — 10 tabs, interactive fan curve editor, form widgets |

## What works

- **Fan control** — Multi-fan PWM with temperature-based curve interpolation (linear, 20%/tick smoothing), RPM readback
- **CPU tuning** — Governor, turbo boost, energy performance preference (all cores)
- **TDP control** — PL1/PL2/PL4 power limits with per-device bounds (Uniwill, via tuxedo_io)
- **Profiles** — 4 built-in + custom profiles, AC/battery assignment, JSON persistence
- **Auto-switching** — Polls AC/battery state, auto-applies mapped profile on transition
- **Charging** — Start/end threshold control via sysfs
- **Keyboard** — Brightness, color, mode via LED class or tuxedo_keyboard driver
- **Display** — Backlight brightness read/write via sysfs
- **GPU info** — PCI vendor/device scan, hwmon temperature, PRIME mode detection
- **Webcam** — USB bind/unbind toggle (sysfs) or hardware switch (Clevo, via tuxedo_io)
- **Shutdown** — `shutdown +N` / `shutdown -c` scheduling
- **TUI** — Dashboard, profiles, fan curves, settings, power/display/webcam/keyboard/charging/info tabs

## Tech decisions

- **Auto-detecting hardware IO** — If `/dev/tuxedo_io` exists (tuxedo-drivers loaded), uses pure Rust ioctl for fan control, webcam, and TDP. Otherwise falls back to direct sysfs. No C++ FFI, no bindgen.
- **Session bus by default** — Runs rootless on the session D-Bus. Hardware writes are best-effort (log `PermissionDenied`, don't fail the call). System bus mode available for root deployments.
- **TEA architecture in TUI** — The Elm Architecture (`Model → update() → view()`) keeps TUI logic testable. Side effects are `Command` values returned from pure `update()`, dispatched asynchronously. 120+ tests across the workspace.
- **Trait-based hardware abstraction** — `TuxedoIO` trait (20+ methods) with `IoctlTuxedoIO` (tuxedo_io ioctl), `SysFsTuxedoIO` (generic sysfs), and `MockTuxedoIO` (tests). All hardware access goes through the trait.
- **Best-effort writes** — Hardware writes log errors to stderr but never fail the D-Bus call. The TUI works fully on non-TUXEDO hardware (reads return defaults, writes are silently skipped).

## Hardware compatibility

Requires [tuxedo-drivers](https://github.com/tuxedocomputers/tuxedo-drivers) for TUXEDO-specific hardware. The daemon auto-detects the best interface at startup:

| Interface | When used | Capabilities |
|---|---|---|
| `IoctlTuxedoIO` | `/dev/tuxedo_io` exists | EC-level fan control (Clevo 3-fan, Uniwill 2-fan), TDP (PL1/PL2/PL4), webcam HW switch, perf profiles |
| `SysFsTuxedoIO` | Fallback (always available) | `tuxedo_fan_control/` PWM, LED class keyboard, hwmon, cpufreq, backlight, power_supply, DRM |

Both interfaces are pure Rust. `IoctlTuxedoIO` delegates CPU governor, backlight, charging, and GPU queries to sysfs — ioctl is only used where sysfs doesn't reach (EC registers, TDP).

On non-TUXEDO hardware, `SysFsTuxedoIO` works for generic fan/CPU/backlight control using standard Linux sysfs interfaces.

## Building

```sh
flox activate
just build        # or: cargo build --workspace
just test         # or: cargo test --workspace
```

## Development (mock daemon, no hardware needed)

```sh
just run-mock-daemon   # terminal 1: mock daemon on session bus
just run-tui           # terminal 2: TUI on session bus
```

## Deployment (real hardware)

```sh
# Build and install binaries + systemd service
sudo just install

# Enable and start the daemon
sudo systemctl enable --now tccd-rs

# Run the TUI (as regular user, connects to system bus)
tccd-tui
```

To uninstall: `sudo just uninstall`

## License

See upstream [TUXEDO Control Center](https://github.com/tuxedocomputers/tuxedo-control-center) for license terms.
