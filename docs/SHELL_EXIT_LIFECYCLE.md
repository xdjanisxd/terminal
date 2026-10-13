# Shell exit lifecycle

A PTY child's natural exit closes its pane, regardless of its exit status. Closing
the last pane in a tab removes that tab. When no running pane remains, the
application exits through the winit event loop and returns from `main`.
Other panes retain their sessions, terminal contents, and focus where possible.

## Root cause and implementation

The PTY session already polls/reaps child processes and emits one `Exited` event.
The application previously only logged foreground exits and ignored background
exits. Its explicit pane-close operation intentionally retained the final pane.

The application now collects exited pane IDs during foreground/background event
draining, then applies removals in workspace order after the drain. This avoids
changing an event's owner midway through a batch and deduplicates exit events.
Pane-specific workspace removal reuses explicit close rules, preserving focus in
surviving tabs and collapsing split layouts. The application handles the final
session separately, keeping the workspace's nonempty invariant until shutdown.
An exit request prevents restarting workers. Workspace replacement clears stale
pending identities; stale exits of explicitly removed panes are ignored.

Removed runtimes drop their PTY workers. `PtyWorker::shutdown_and_join` disconnects
the event receiver, requests termination, and joins the session and reader
threads. Session polling reaps the child and releases the native PTY master.
Application shutdown releases remaining workers before releasing Linux renderer
and window resources. The diagnostic throughput runner retains its existing
EOF/exit/final-frame completion and reporting behavior.

Automatic exits do not save workspaces. Named Saved Workspaces remain explicit
snapshots. Explicit user pane-close behavior remains unchanged, including retaining
the final pane; window close still shuts down the application.

## Validation

Normal validation:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Focused tests cover zero/nonzero exits, foreground/background panes, tab removal,
final-session shutdown, simultaneous/duplicate/stale exits, EOF without child exit,
workspace replacement, saved layout preservation, explicit closes, and exits racing
shutdown. Real-worker tests send `exit 23` through PTY input and exercise the
production application event drain and worker cleanup. On Linux they use Bash and
Zsh (Zsh is skipped if absent); on Windows they use CMD and Windows PowerShell.

Native Linux process test (requires Bash, Zsh, a display, and a GPU):

```sh
cargo test -p terminal-app --test shell_exit_native -- --ignored --nocapture
```

This launches the actual application for each shell in six scenarios: successful
single-pane exit, nonzero single-pane exit, split panes, separate tabs, mixed
panes/tabs, and a command exiting during startup. Restored local-shell startup
commands execute the exits. Multi-session scenarios keep a surviving shell behind
a file gate and verify it stays alive while other children are reaped. The test
waits for the application process to terminate successfully, verifies shell PIDs
are gone, checks per-session temporary integration files are released, and checks
Saved Workspace bytes are unchanged. It retains temporary logs/configurations and
prints their location for inspection.

Windows-specific real-worker regressions run in the Windows test suite. A Linux
host cannot execute ConPTY or verify native Windows application shutdown; those
checks require a Windows runner. Linux native acceptance does not prove Windows
native process behavior.
