AGENTS.md is project specific instructions and reference — how to build, deploy, what patterns to follow, where things live.
It rarely changes.

WORKLOG.md is the session diary.
Every time we work on a project, agents log what we investigated, what changed, what we decided, and why.
When I come back days or weeks later, agents read the worklog and pick up where we left off instead of starting cold.

Progress automonously through planned phases.
If some of the phases or order within the phase is unclear - investigate and clarify beforehand.

After each phase - launch 2 sub-agents to review implemented changes to make sure:
  a) They conform to phase specification and requirements, and nothing was missing
  b) See if anything can be improved, refactored or removed.

At the end of each phase - make sure that tests and linters are passing.

Use recent libraries, dependencies and common approaches, as of April 2026.

## Project reference

- **Workspace**: `tccd-daemon` (D-Bus service) + `tccd-tui` (ratatui terminal UI)
- **Build**: `flox activate && just build` (or `cargo build --workspace`)
- **Test**: `just test` (126 tests), `cargo clippy --workspace` (0 warnings)
- **Run (dev)**: `just run-mock-daemon` (term 1) + `just run-tui` (term 2) — both use session bus
- **Run (real)**: `sudo just install && sudo systemctl enable --now tccd` then `tccd-tui`

### Hardware IO

Three `TuxedoIO` trait implementations:
- `IoctlTuxedoIO` — `/dev/tuxedo_io` ioctl (nix crate), auto-detected at startup. Clevo/Uniwill EC fan control, TDP, webcam HW toggle.
- `SysFsTuxedoIO` — Sysfs fallback. Tries tuxedo-drivers platform paths first (`tuxedo_fan_control/`, LED class keyboard), falls back to generic hwmon/cpufreq/backlight.
- `MockTuxedoIO` — Tests + mock daemon.

### Key patterns

- D-Bus: `com.tuxedocomputers.tccd` on `zbus 5`, session bus (dev) or system bus (prod)
- Workers: `fan.rs` (multi-fan per-fan curves, interpolation), `power.rs` (AC/battery auto-switch)
- Profiles: `ProfileStore` with 4 defaults + custom, JSON in `~/.config/tcc-rs/`
- TUI: TEA architecture (`Model → update() → view()`), 10 tabs, form widgets
- Best-effort writes: log PermissionDenied to stderr, never fail the D-Bus call