# Justfile for tcc-rs workspace

default:
    @just --list

# ==== Daemon ====

# Run the daemon on session bus
run-daemon:
    cargo run -p tccd-daemon --bin tccd-daemon -- --session

# Run the mock daemon (for TUI development without hardware)
run-mock-daemon:
    cargo run -p tccd-daemon --bin mock_daemon -- --session

# Run daemon tests
test-daemon:
    cargo test -p tccd-daemon

# ==== TUI ====

# Run the TCC TUI (terminal interface, session bus for local dev)
run-tui:
    cargo run -p tccd-tui -- --session

# Run TUI tests
test-tui:
    cargo test -p tccd-tui

# ==== Workspace ====

# Build everything
build:
    cargo build --release --workspace

# Run all tests
test:
    cargo test --workspace

# Check + lint
check:
    cargo check --workspace
    cargo clippy --workspace

# Clean build artifacts
clean:
    cargo clean

# ==== Deployment ====

# Install daemon + TUI binaries and systemd service (run as root)
install: build
    install -Dm755 target/release/tccd-daemon /usr/local/bin/tccd-daemon
    install -Dm755 target/release/tccd-tui /usr/local/bin/tccd-tui
    install -Dm644 dist/tccd-rs.service /etc/systemd/system/tccd-rs.service
    install -Dm644 dist/com.tuxedocomputers.tccd.conf /etc/dbus-1/system.d/com.tuxedocomputers.tccd.conf
    systemctl daemon-reload
    @echo "Installed. Enable with: systemctl enable --now tccd-rs"

# Uninstall daemon + TUI binaries and systemd service (run as root)
uninstall:
    -systemctl disable --now tccd-rs
    rm -f /usr/local/bin/tccd-daemon /usr/local/bin/tccd-tui
    rm -f /etc/systemd/system/tccd-rs.service
    rm -f /etc/dbus-1/system.d/com.tuxedocomputers.tccd.conf
    systemctl daemon-reload
    @echo "Uninstalled."
