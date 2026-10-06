# Saved Workspace cmd startup commands

Branch: `fix/cmd-workspace-startup-command`. No commit or push.

## Goal and completed plan

Restore saved pane startup-command dispatch for cmd.exe while retaining saved
CWDs, layout/focus, configured shell arguments, PowerShell behavior, session
classification, initialization ordering, and exactly-once dispatch.

1. Trace workspace load, fresh pane creation, shell spawn, readiness, dispatch.
2. Compare shell integration and rule out command encoding/classification.
3. Add the missing cmd readiness hook within existing application shell policy.
4. Test native cmd/PowerShell multi-pane restores, exact Codex launch, and focused
   readiness/argument/session behavior; run the repository completion gates.

## Trace and root cause

`SavedWorkspaces::load` deserializes the named template.
`Application::open_saved_workspace` validates it through
`Workspace::from_definition`, creates fresh pane runtimes, restores focus, and
calls `start_local_shell`. Each pane's saved startup command becomes runtime
`pending_startup_command`. `spawn_config_for_session` chooses the current
configured LocalShell or the saved direct Command executable, then applies the
saved launch CWD separately. Shell spawn goes through the normal PTY backend.

Foreground `handle_pty_event` and `handle_background_pty_event` parse output and
call `issue_startup_command_if_ready`. Its dispatcher requires a nonzero
`TerminalState::shell_prompt_version`, established by OSC 133;A. The PowerShell
prompt hook already emitted this marker; cmd's visible prompt did not. Cmd's
startup command remained pending indefinitely before any write was attempted.

The failure was a readiness/shell-integration assumption. Session classification,
quoting, command wrapping, and newline encoding were not the cause: both local
shells share the same dispatcher, which sends the unchanged command bytes plus
CR and consumes the pending command only after successful queueing.

## Fix

App-owned shell policy appends `$e]133;A$e\` to interactive cmd's `PROMPT`
environment value. Cmd expands `$e` to ESC, producing an OSC prompt marker with
an ST terminator. The inherited/explicit visible prompt is preserved, falling
back to `$P$G`; applying the hook again does not append another suffix.
Configured and default cmd local shells use the hook. Arguments are unchanged,
and readiness follows `/k` initialization. `/c` and direct Command sessions
remain unhooked. No command-specific handling or core/parser changes were added.

## Evidence

- Native Saved Workspace test: actual cmd.exe, PowerShell 7, and Windows
  PowerShell PTYs pass two-pane reopening. Different commands append exactly
  one result each in different saved CWDs containing spaces and Unicode. The
  saved split ratio, focus, roots, and metadata remain equal after execution.
  Cmd `/k` initialization also executes before prompt-driven startup.
- `codex --no-daemon` runs through the actual cmd PTY and shared dispatcher. The
  installed CLI responds with its `TERM=dumb` confirmation. The bounded probe
  stops there without answering; it proves launch, not full TUI operation.
  Restricted Codex temp-directory access warnings are also present. This probe
  is ignored by default because it requires an external installed CLI:
  `cargo test -p terminal-app cmd_saved_startup_launches_installed_codex -- --ignored --nocapture`.
- Focused tests cover fragmented BEL/ST markers, banners that are not readiness,
  exact commands plus CR (including quoted metacharacters), no repeat dispatch,
  preserving visible prompt/environment/arguments, hook idempotence, and
  excluding `/c`, unrelated executable names, and direct Command sessions.
  The existing failed-write/retry test remains in force.

Native tests use production configuration, parser, and foreground/background
event handlers with manually attached workers because the headless test has no
winit wake proxy. Visual app interaction is not claimed. Installed PowerShell
tests require native process permissions in this environment.

Completion validation passes: `cargo fmt --all -- --check`,
`cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`,
`cargo test --workspace --doc`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`.
The app suite has 134 passing tests and one ignored external Codex probe,
which was explicitly run and passed separately; four doctests pass.

## Remaining limitations

AutoRun scripts, `/k` commands, or extensions that replace `PROMPT` can remove
the readiness marker; they must preserve it. Explicit `/c` shells do not become
interactive. Cmd still has no OSC 7 CWD reporting, so saved known launch roots
remain the available CWD metadata. Other shells and renamed/wrapped cmd
executables receive no new integration. Full Codex TUI acceptance remains
outside the bounded launch probe.
