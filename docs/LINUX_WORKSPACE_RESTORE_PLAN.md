# Linux Saved Workspace restoration

Branch: `fix/linux-workspace-restore`. Do not commit or push.

Status: completed. Findings, validation, native evidence, and limitations are
recorded in [the restoration report](LINUX_WORKSPACE_RESTORE.md). The plan below
preserves the initial investigation and completion criteria.

## Goal

Fix Saved Workspace restoration on Linux so each local-shell pane restores its
saved working directory and executes its explicit startup command exactly once,
after the shell is ready. Preserve pane layout, focus, shell configuration, and
Windows behavior. Do not special-case Codex.

## Initial evidence

- `Application::snapshot_workspace` in `crates/app/src/main.rs` captures CWD
  from terminal OSC 7 metadata, falling back to the configured pane root.
- `dispatch_startup_command` waits for a nonzero OSC 133 prompt version and
  consumes the pending command only after the PTY write is successfully queued.
- `crates/app/src/shell.rs` currently adds prompt integration for CMD and
  PowerShell, but no Bash/Zsh integration. The README documents the absence of
  Bash/Zsh directory reporting.
- `crates/terminal-pty/src/lib.rs` applies the requested working directory when
  building the child command. Whether restoration supplies the correct value,
  and whether startup files subsequently change it, still needs verification.

These are investigation leads, not a verified root-cause report.

## Plan

1. **Trace and reproduce the full restoration path.** Follow pane roots and
   explicit commands through workspace snapshotting, TOML save/load, workspace
   reconstruction, foreground/background runtime creation, PTY spawn, shell
   initialization, output parsing, and dispatch. Capture a two-pane reproduction
   with distinct directories and commands; compare saved TOML, spawn CWD, and
   CWD after startup. Include `cd ~/codes/terminal` and `codex --no-daemon` as
   manual examples, without making Codex a test dependency.
2. **Determine where directory information fails.** Distinguish missing or
   incorrect OSC 7 metadata, serialization or root-precedence errors, ignored
   spawn configuration, and shell startup files changing CWD. Check active and
   inactive panes, directory changes after launch, absolute paths containing
   spaces/Unicode, and URI encoding/decoding. Define and document behavior for
   missing directories and profiles that intentionally change directory.
3. **Investigate readiness and compare Windows.** Trace Bash/Zsh startup order,
   interactive/login/command modes, custom prompts, and existing hooks. Verify
   OSC 133 parsing, including fragmented output, and determine when a prompt
   really permits command input. Compare CMD initialization and PowerShell's
   profile-preserving OSC 7/133 hook, pending-command ownership, and existing
   foreground/background native tests. Do not use banner output or elapsed
   time alone as readiness.
4. **Implement the smallest verified fix.** Add session-local Bash/Zsh directory
   and readiness reporting where needed, preserving user initialization,
   arguments, environment, and prompt behavior. Correct any proven save/load,
   CWD, or dispatch defects. Keep shell policy in the app, terminal semantics in
   `terminal-core`, and PTY/platform behavior behind existing interfaces. Avoid
   edits to users' startup files and unnecessary dependencies. Keep explicit
   noninteractive/script invocations intact and avoid dispatch into them.
5. **Add focused regression coverage.** Cover Linux two-pane CWD snapshot and
   TOML round-trip, root selection at spawn, Bash/Zsh hook behavior, readiness
   before dispatch, failed queue attempts, exactly-once successful dispatch,
   repeated prompt reports, and runtime/focus switching. Test configured startup
   commands separately from manually typed commands; no process inference.
   Preserve existing CMD/PowerShell coverage.
6. **Verify native Linux restoration.** Exercise real Bash and Zsh PTYs with
   controlled startup files and the normal user configuration where available.
   Save and reopen a two-pane workspace with distinct roots, focus, and explicit
   commands. Use per-pane append-only marker files containing CWD to prove
   correct directory and one execution, including the inactive pane. Exercise
   subsequent prompts and focus changes to detect redispatch. Test startup
   initialization that delays readiness, custom prompts/hooks, command exit
   returning to the shell, and explicit command mode. Verify the release UI
   restoration path as well as the native PTY tests. Report unavailable shells
   or desktop automation honestly.
7. **Run normal Rust validation.** Run the repository checks below and resolve
   relevant failures. Native Windows execution is separate from Linux checks;
   report whether Windows verification is source/test review or native results.
8. **Update documentation and report.** Align the README/configuration docs with
   the implemented shell support. Record verified root causes, the fix, native
   Bash/Zsh two-pane results, exactly-once evidence, validation results, and
   remaining limitations in `docs/LINUX_WORKSPACE_RESTORE.md`.

## Validation commands

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
cargo build --locked --release -p terminal-app --bin terminal
```

## Completion criteria

- Saving after changing directory captures each pane's current directory;
  reopening restores both directories and the saved layout/focus.
- Explicit startup commands execute in the intended pane and directory after
  readiness, exactly once per new local-shell session. Panes without commands
  remain idle; unsupported/noninteractive modes do not receive injected input.
- Shell configuration and normal initialization remain effective, with explicit
  behavior documented for initialization that changes CWD or replaces hooks.
- Native Bash/Zsh and two-pane results are recorded, normal Rust checks pass,
  and any Windows verification limits are stated.
- No commits or pushes are made.
