# Shutdown reliability investigation

Branch: `fix/shutdown-reliability`. Investigation completed on 2026-10-06.

No production shutdown hang was reproduced with close requests delivered to the
terminal window. No production fix is retained. Temporary shutdown tracing,
explicit teardown probes, and the Saved Workspace trigger were reverted.
Runtime ownership, shutdown ordering, shell configuration, and startup behavior
are unchanged. No commits or pushes were made.

## Reproduction and results

Windows x86_64 release builds were tested on Windows build 26300 using Rust
1.98.1 and PowerShell 7.6.6. Each matrix used `cmd.exe`, `pwsh -NoProfile`, and
profile-enabled `pwsh`, with one and three local-shell panes, three repetitions
per combination. PowerShell was configured through the normal application
configuration path, with no arguments for the profile-enabled case. The existing
user configuration selects cmd, so these were isolated configuration files;
the user's configuration was not modified.

| Matrix | Launches | Result |
| --- | ---: | --- |
| Initial `.NET CloseMainWindow()` harness: immediate / ready | 36 | 18 immediate cases needed harness cleanup; 18 ready cases exited |
| Correct terminal HWND, temporary shutdown tracing: immediate / first visible / ready | 54 | All exited with code 0; no forced cleanup or observed surviving descendants |
| Saved Workspace replacement, temporary native trigger: immediate after open / ready | 36 | All exited with code 0; old and replacement workers joined |
| Correct terminal HWND, normal production release: immediate / first visible / ready | 54 | All exited with code 0; no forced cleanup or observed surviving descendants |

The 144 correctly targeted cases did not reproduce a hang. In the normal release
matrix, time from posted close request to process exit was 94–1228 ms. This is an
observation, not a shutdown deadline or performance guarantee. Early close can
wait for synchronous startup work before the event loop handles the request.

Immediate means the real HWND exists, even if hidden. First visible means that
HWND is visible. PowerShell ready means a `prompt-ready` startup milestone for
every pane. Cmd readiness uses `first-shell-output-rendered` per pane: it verifies
startup output, not an OSC prompt boundary or visual prompt inspection.

Saved Workspace testing called the production `open_saved_workspace` method
from a temporary native application trigger after initial pane workers existed.
An isolated Saved Workspaces store supplied the replacement one/three-pane
layout. Closing immediately after replacement and after new prompt/output
milestones exercised teardown of both the original and replacement sessions.
This tested the production loading/teardown method, not interaction with the
picker UI. External process sampling began around close; old-session cleanup
was additionally verified by reader-finished and worker-join-end traces during
replacement. Temporary probes were removed before the final production matrix.

## Exact cause of the reproduced harness failure

During startup, `Process.MainWindowHandle` selected a visible, untitled
`Winit Thread Event Target` helper window. The actual terminal HWND had class
`Window Class` and was initially hidden. `CloseMainWindow()` returned true after
targeting the helper, but the application never received `CloseRequested`.
The terminal subsequently became visible and remained running. Treating this as
an application shutdown hang was incorrect.

Enumerating windows belonging to the launched PID and posting `WM_CLOSE` to the
actual terminal HWND removed that failure in both traced and normal builds.
The earlier PTY-overlap experiment and its exact harness were not recreated;
there is insufficient evidence to assign this cause to its historical failures.
No claim is made that all timing-sensitive production failures are excluded.

## Normal shutdown and cleanup ownership

1. `ApplicationHandler::window_event` handles `CloseRequested` by calling
   `event_loop.exit()`. Other explicit application exit actions use the same
   event-loop exit mechanism. The `exiting` callback reports throughput;
   `run_app` returns and the application is dropped.
2. Application field destruction releases its window reference, then the
   renderer, then active and inactive pane runtimes. The renderer retains its
   own window reference while owning the surface/device/queue; it has no custom
   shutdown join. Temporary explicit drop probes following this order completed
   renderer teardown before PTY teardown in every correctly targeted case.
3. Each pane owns its PTY worker. Closing a pane removes that runtime/worker;
   Saved Workspace replacement takes the active worker and clears inactive
   runtimes before starting replacement sessions. Closing the application
   destroys all remaining pane workers.
4. `PtyWorker::Drop` calls `shutdown_and_join`: first discard the event receiver
   to release blocked bounded-output sends, then send `Terminate`, drop the
   command sender, and join the session thread. That thread joins its separate
   reader before returning and dropping the session. Worker joins are synchronous.
5. The existing portable session termination path drops its writer and calls
   the owned child's `kill()` operation. This behavior predates the investigation;
   it is not a newly introduced graceful shell-exit protocol or fallback kill.
   Natural shell exit is detected through `try_wait` independently of output EOF.
6. Lifecycle refresh records child exit and takes/drops the master PTY handle.
   On Windows the portable-pty ConPTY owner closes the pseudoconsole. The output
   reader signals completion on EOF, read failure, or a disconnected event
   receiver. The session loop waits for both child exit and reader completion.
   Reader/session threads and remaining reader/writer/child/master owners are
   released before the controller's shutdown join returns.

The instrumented runs reached event-loop return, renderer-drop completion,
reader completion, every worker join completion, and final PTY-drop completion.
No retained sender/receiver or ConPTY lifetime problem was demonstrated.
Observed shell/console descendants did not survive correctly targeted close.
Process sampling is not a proof about arbitrary grandchildren that exit between
samples or intentionally detach; the application owns its direct shell child.

## Retained tooling and deterministic coverage

`scripts/test-shutdown.ps1` is a repeatable Windows release matrix. It uses the
application's existing opt-in startup milestones, selects the actual HWND,
observes process descendants, records readiness/close/exit results to CSV, and
fails on a missing close, missing requested readiness, nonzero exit, hang, or
observed surviving child. PID and creation-time checks identify observed
children before cleanup. The harness's observation limits and cleanup apply
only to processes it launched; they do not change production shutdown policy.
The window class match is tied to the current winit backend and must be revisited
if that backend changes.

Run from the repository root with PowerShell 7 on an interactive Windows desktop:

```powershell
cargo build --release -p terminal-app
./scripts/test-shutdown.ps1 -Runs 3 -OutputDirectory target/shutdown-reproduction
```

The new worker test
`dropping_the_controller_with_a_full_event_queue_joins_both_workers` waits for
post-enqueue notifications to deterministically fill the event queue, drops the
controller without draining it, and verifies termination and session destruction
after the join. Its test-only deadline bounds a regression; it adds no runtime
timeout. Existing tests cover natural exit, separate EOF/exit events, output
backpressure, full command queues, explicit termination, controller drop, and
Saved Workspace behavior.

Validation passed: the focused new worker test; release build; formatting check;
workspace/all-target check and tests; workspace doctests; workspace/all-target
Clippy with warnings denied; and whitespace diff check. Existing ignored tests
remain ignored, including the external installed-Codex acceptance probe.
Raw investigation logs and isolated configurations are local ignored artifacts
under `target/shutdown-{baseline,targeted,saved,production}`.

## Remaining reliability risks

Blocking PTY writes, a child that does not terminate as expected, ConPTY closure,
and renderer/device teardown remain possible failure boundaries under workloads
not covered here. No causal failure was observed, so no speculative timeout,
force-kill fallback, thread reordering, or renderer change was added.

The config watcher is detached and can remain sleeping until process exit unless
a later config-change notification fails. It is not explicitly joined. It did
not prevent process exit in these tests; PTY worker joins must not be described
as a join of every application background thread.

Heavy output/input, arbitrary shell profiles, detached descendants, and other
machines/GPU drivers remain outside this finite matrix. If the overlap experiment
is rerun, first verify which HWND receives the close and capture whether
`CloseRequested` is reached before diagnosing worker or renderer teardown.
