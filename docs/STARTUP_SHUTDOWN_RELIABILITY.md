# Startup and shutdown reliability follow-up

Investigated on Linux KDE Wayland on 2026-10-21, from master `57a7eaf`.
See also [startup presentation](STARTUP.md), [PTY startup investigation](PTY_STARTUP.md),
[previous shutdown investigation](SHUTDOWN_RELIABILITY.md), and
[shell exit ownership](SHELL_EXIT_LIFECYCLE.md).

## Confirmed problems and fixes

1. `PtyWorker::start_with_notifier` consumed a live session and returned immediately
   when `take_output_reader` failed. It never requested termination, leaving cleanup
   to the backend's destruction behavior. A scripted session reproduces the missing
   termination call. Startup now requests termination before returning the original
   reader error. This is failure-path coverage at the PTY boundary; it does not show
   that normal application startup previously encountered this error.
2. `shutdown_and_join` dropped the command sender before joining. Once the queued
   termination command was consumed, `recv_timeout` returned `Disconnected`
   immediately. Until the reader finished or the child exited, the session thread
   repeatedly checked lifecycle with no wait. A gated-reader regression measured
   1,257,825 lifecycle checks in 100 ms before the fix. Keeping the existing sender
   alive until the join completes preserves the existing 20 ms receive wait; the
   corrected test measured five checks in 100 ms. No new polling was introduced.

The event receiver is still discarded before requesting termination, releasing
blocked output producers. Command/event queue capacities, command ordering,
termination behavior, and the two existing worker threads are unchanged. There
are no new allocations, locks, atomics, threads, dependencies, or timers in
production. Successful startup and input/output paths gain no additional work.
The sender is released even when joining reports a worker panic.

## Scope and decisions

Inspected initial presentation, session preparation, tab/split creation, saved
workspace replacement, output/exit routing, worker startup, reader completion,
termination, and application teardown. Existing deduplicated exit identities,
deferred pane removal, and shutdown guards already handle natural shell exits.
Explicit close still preserves the final pane; final natural exit requests
application shutdown. Automatic exits do not save workspace files. Direct-command
argv/cwd handling and shell integration retain their existing paths.

No causal evidence justified moving PTY creation to another thread, changing
renderer teardown, adding forced shutdown deadlines, expanding queues, or adding
process-tree management. Synchronous PTY creation still follows the initial
presentation; this preserves first-frame behavior and avoids a new startup race.
The earlier Windows investigation's incorrectly targeted helper window remains
a harness issue, not evidence for a production workaround.

## Validation

Both new worker regressions failed before the production edits and passed after
them. The PTY package's 30 tests passed. Application lifecycle coverage now has
14 tests, including 12 repeated rounds of tab/split creation with real shell
workers, explicit close racing a sibling's natural exit, preservation of the
survivor, and idempotent final shutdown. This calls production application methods
without a display; it does not automate keyboard shortcuts.

Final Linux validation passed:

- Workspace/all-target tests: 841 passed, 28 ignored, no failures; four doctests.
- Formatting, workspace/all-target check, Clippy with warnings denied, release
  workspace build, and whitespace checks.
- Native application exit matrix: all 12 Bash/Zsh cases passed (zero/nonzero
  exits, splits, tabs, mixed layouts, and immediate command exit at startup).
  Checks include child reaping, surviving sessions, integration-file cleanup,
  and unchanged saved-workspace bytes.
- Native direct-command tests: exact argv and cwd for `-e`, `--execute`, and `--`;
  missing executable without shell fallback; Vim/Neovim exits with Unicode/space
  paths. All three ignored tests passed when explicitly selected.
- Eight release application close cases: Bash/Zsh, one/three tabs, early and
  prompt-ready close. KDE scripts matched only the launched PID and delivered a
  native window close. Every application received `CloseRequested`, exited with
  status zero, removed its integration files, and preserved saved-workspace bytes.
  All shell PIDs recorded by startup commands were gone. Early close can precede
  those markers, so it does not enumerate every early-startup child.

The eight release launches all recorded first presentation before PTY startup.
First presentation was 189–226 ms (median 207 ms); close request submission to
process exit was 72–425 ms. These warm/uncontrolled samples with diagnostics enabled
are observations, not deadlines or a controlled latency comparison. No before/after
startup speedup or memory measurement is claimed. The shutdown regression directly
demonstrates removal of CPU spinning; production changes add no resource owners.

Raw local logs and the isolated KDE close harness are under
`/tmp/startup-shutdown-reliability/`. Native commands:

```sh
cargo test -p terminal-app --test shell_exit_native -- --ignored --nocapture
cargo test -p terminal-app --test execute_native native_execute_ -- --ignored --nocapture
```

`--ignored` explicitly selects display/GPU acceptance tests; `--nocapture` keeps
their raw scenario diagnostics. The `native_execute_` filter excludes the unrelated
installed-launcher test. The full workspace run leaves native tests ignored by default.

## Remaining limits and manual acceptance

The later [blocked PTY shutdown fix](BLOCKED_PTY_SHUTDOWN.md) cancels Unix I/O
before joining and verifies Linux blocked-write recovery. Windows cancellation
remains open. At the time of this investigation, blocking PTY writes already
in progress, platform kill/wait failures, readers kept
open by descendants, and GPU teardown can still block synchronous joins. These
changes establish no bounded shutdown deadline. OS thread creation still uses
`thread::spawn`; thread-resource exhaustion is not converted into a recoverable
startup error. The existing detached config watcher and 20 ms worker lifecycle
polling remain. Arbitrary detached descendants and hostile profiles are outside
the tested process cleanup contract. Native Windows and macOS were not run here.

Linux checklist (Bash and Zsh):

- [ ] Visually confirm first frame, theme, focus, and immediate typing.
- [ ] Rapidly create/close tabs and both split orientations using shortcuts;
  verify focus, final-pane close policy, and unchanged surviving shell state.
- [ ] Run `exit 0`/`exit 23` in foreground/background panes, including the final pane.
- [ ] Close the window during startup, at the prompt, and during heavy output/input;
  check for surviving children and temporary integration files.
- [ ] Restore/replace a saved workspace with running shells and startup commands;
  verify cwd, integration, cleanup, and unchanged saved bytes after natural exit.
- [ ] Check missing shell/command/cwd diagnostics and direct editor exit behavior.

Windows checklist (cmd, PowerShell without profiles, and with normal profiles):

- [ ] Run `scripts/test-shutdown.ps1 -Runs 3` from an interactive desktop after
  `cargo build --release -p terminal-app`; retain its actual-HWND/descendant logs.
- [ ] Repeat the Linux interaction checklist using ConPTY, including rapid splits,
  saved workspace replacement, missing programs, and final-session natural exits.
- [ ] Check first-frame appearance, immediate typing, focus, shell integration,
  and no surviving shell/conhost processes after native close.

macOS still requires native PTY EOF/reaping, window-close, first-frame, and the same
session-interaction acceptance; Linux results do not verify those platform paths.
