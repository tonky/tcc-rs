# Justfile for tcc-rs workspace

default:
    @just --list

# ==== Daemon ====

# Run the daemon on session bus
run-daemon:
    cargo run -p tccd-daemon -- --session

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
    cargo build --workspace

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
