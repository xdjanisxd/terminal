# Terminal Workspace Application

A lightweight, fast, keyboard-first, GPU-accelerated terminal emulator and workspace application for Windows, Linux, and macOS.

The repository has completed its local foundation scaffold. Structural workspace crates and cross-platform CI exist, but no terminal functionality or external dependencies have been implemented.

## Priorities

1. Correct terminal behavior
2. Low startup latency and unnecessary CPU usage
3. Cross-platform consistency
4. Simple, maintainable architecture
5. Keyboard-first usability and configurability

## Repository map

- `crates/`: dependency-free structural Rust workspace crates
- `docs/`: product, architecture, roadmap, conventions, performance, and ADRs
- `.agent/`: operational context for engineering sessions

## Target platforms

- Windows: x86_64 and ARM64
- Linux: x86_64 and ARM64
- macOS: x86_64 and ARM64

Minimum operating-system versions will be derived from the selected dependency versions and platform validation rather than guessed in advance.

## Current validation

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

See `docs/CURRENT_STATE.md` for implemented reality and `docs/ROADMAP.md` for planned work.

## License

Licensed under the MIT License. See `LICENSE`.
