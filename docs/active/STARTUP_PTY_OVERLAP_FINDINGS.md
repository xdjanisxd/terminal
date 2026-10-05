# PTY startup overlap investigation

Status: investigation complete; release experiment rejected, production changes reverted,
final repository validation passed. Native acceptance limitations are recorded below.
Branch: perf/startup-pty-overlap. No commit or push.

## Current dependency chain

| Stage | Required inputs / ownership | Current order |
| --- | --- | --- |
| Config | Application::with_pty_wake_proxy calls reload_config; config path and optional workspace/project-root overrides are resolved in the app | Before native window |
| Workspace | reload_config loads the selected WorkspaceDefinition once; load_workspace creates active/inactive TerminalState and parsers, pane sessions, roots and startup-command definitions | Before native window; temporary terminal dimensions are not spawn dimensions |
| Saved Workspace | open_saved_workspace resolves the saved definition before replacing live runtimes, clears pending commands/wakes, sizes panes, then starts sessions | Explicit open path; not an additional unconditional startup restore step |
| Window | App creates hidden native window; physical inner_size and scale_factor become available | Before renderer constructor |
| Fonts | Renderer::new_with_font_settings loads the selected FontSystem, derives CellMetrics from actual scale and logical font size | CPU only; fonts-ready precedes gpu-started |
| GPU | initialize_gpu creates surface/adapter/device/queue | Synchronous on event-loop callback |
| Renderer | Constructor owns selected fonts and exact metrics; reconfigures surface; configured theme applied by app | renderer-ready |
| Initial grid | resize_terminal_to_viewport uses renderer.cell_metrics and pane_dimensions over every restored tab/split | initial-grid-ready, after GPU solely because metrics are exposed on Renderer |
| First frame | App directly attempts up to three presentations while hidden; lazy draw resources initialize here | Must succeed before window becomes visible |
| Visibility/focus | redraw_window records first-frame-presented, makes window visible and focuses it | window-shown |
| Session preparation | start_local_shell snapshots pane launch, startup command, exact terminal dimensions and configured shell | Currently after presentation, synchronous |
| Shell/integration | spawn_config_for_session selects LocalShell vs Command; shell module retains custom args/profile behavior and adds PowerShell hook only when appropriate; root becomes CWD | Before native spawn |
| PTY | PortablePtyBackend opens native PTY at exact rows/columns, acquires handles | pty-create-begin -> pty-created / initial-size-sent |
| Environment/spawn | CommandBuilder copies program/args/CWD/env; spawn_command creates child | environment-build-begin/end -> child-spawn-requested/spawned |
| Workers | PtyWorker obtains exclusive output reader, creates existing session and blocking-reader threads with bounded queues | After synchronous spawn |
| Output/readiness | App drains worker events, advances parser and terminal, processes OSC 7 and OSC 133, issues replies and gated startup command | Event loop only; no renderer in worker |
| Startup commands | dispatch_startup_command refuses until shell_prompt_version != 0; pending command removed only after successful write | Never inferred from child creation or first bytes |
| Presentation of output | Renderer reads current terminal state; startup diagnostics record first-shell-output-rendered | Independent of prompt readiness |

Source entry points: Application::{with_pty_wake_proxy, reload_config, load_workspace,
open_saved_workspace, create_window_and_renderer, resize_terminal_to_viewport,
start_local_shell, spawn_local_shell, drain_pty_events, handle_pty_event,
redraw_window, dispatch_startup_command} in crates/app/src/main.rs;
Renderer::new_with_font_settings in crates/renderer/src/lib.rs;
PortablePtyBackend::spawn and PtyWorker::{start_with_notifier, shutdown_and_join}
in crates/terminal-pty/src/lib.rs; crates/app/src/{shell,startup}.rs.

## Why startup is serialized

This ordering is explicit first-frame protection, not a terminal protocol requirement.
create_window_and_renderer comments that a hidden window may receive no native paint,
and deliberately presents directly before synchronous shell startup using configured
theme/font/DPI/grid. Merely moving start_local_shell earlier would block the event
loop in native spawn and trade first-frame latency for shell latency. The current
constructor also bundles CPU font preparation and GPU initialization, hiding usable
metrics until the GPU is ready. Historical intent beyond the explicit current source
comment was not inferred from commit titles.

## Earliest candidate and necessary changes

Exact initial dimensions can be derived before GPU initialization: after final
configuration/workspace selection, hidden-window creation and CPU font preparation.
Use the same selected fonts, scale factor, physical inner_size and pane_dimensions
function. Do not use default/provisional grids, duplicate font discovery or alter
renderer ownership of fonts. Confirm a nonzero drawable size; zero-size windows are
not overlap eligible. Reconcile genuine native size/DPI changes through existing
resize handling without an unconditional second PTY resize. Startup maximization
is not configured in branding::window_attributes; manually maximize immediately
and test the existing resize path rather than inventing a maximized-start setting.

A candidate minimal design is a renderer-owned prepared-font value exposing exact
CellMetrics, followed by GPU completion using that value. The app sizes all restored
runtimes, snapshots each final pane launch and dispatches once. Native backend/spawn
would run inside the existing PTY session worker, with an asynchronous factory entry
point; its nested blocking-reader thread and bounded transport remain unchanged.
This design was implemented as a bounded experiment, tested and measured, then
reverted. No new font/factory API remains in the repository.

Keep the GPU callback and parser/state ownership on the event loop. An installed
worker handle represents pending/running/failed startup so renderer recreation
cannot spawn twice. Report asynchronous spawn failure in logs AND workspace notice,
including background panes. Preserve LocalShell/Command distinction, args, inherited
environment, saved roots, integration hook and prompt-version gate.

## Early output and first-frame protection

Existing queues bound raw output: eight events, chunks at most 1 KiB; producers
block rather than discard. Output ordering is FIFO. Parser/state are app-owned,
not renderer-owned, and invalidate_frame only needs an optional window. A queued
wake can safely wait during the synchronous renderer callback. Do not add unlimited
raw buffering or replay already-applied bytes.

There is an important trap: redraw_window currently drains pending PTY output before
rendering. With overlap, the direct first-frame call could parse output and rasterize
new shell glyphs before showing the window. Gate that drain until the first valid
frame is presented, leaving the pending wake intact. Then the ordinary bounded drain
updates terminal state and schedules its next frame. The first visible configured
frame still does not depend on shell output or readiness. A renderer-absent invalidation
must stay coalesced/rearmed; once ready it renders the current terminal state.

Keep replies and readiness dispatch on the same existing app path and FIFO worker
command channel. Audit queue-full behavior explicitly: current reply writes log a
failure if the bounded command queue fills; overlap must not create additional loss
or reorder replies. A worker must never call renderer/window methods.

## Failure and shutdown design obligations

Currently renderer failure occurs before any initial PTY exists. Overlap changes
that invariant. A factory worker must own the session as soon as spawn returns;
shutdown before/during spawn must terminate it, close handles and join both threads.
Exclusive reader acquisition failure must explicitly terminate a created session.
A spawn error must notify the app and finish without blocking GPU completion. Event
receiver disconnection must unblock a full producer queue before joining; existing
shutdown_and_join drops unread events before termination/join. Native spawn itself
is not cancellable through the current portable interface, so joining a pending
spawn may wait for that OS call; measure and report rather than claim instant shutdown.

Renderer failure/first-frame failure must trigger existing app teardown and join
any dispatched workers. A close event queued during synchronous GPU setup can only
be handled when the event-loop callback returns, as today. Add deterministic gated
factory tests for close-before-spawn-completes and failure paths, and native handle/
child checks; static analysis alone is not acceptance evidence.

## Release experiment and decision

**Do not retain overlap.** Native spawn began much earlier and PowerShell prompt
readiness improved, but both PowerShell first-frame medians regressed and all three
worst observed first-frame values increased. Range overlap and the small sample
prevent a strong causal/statistical claim; the evidence does not establish the
required absence of first-frame regression. There is also unresolved native shutdown
reliability. The safe decision is rejection, not acceptance based on theoretical gain.

The experiment split CPU font preparation from GPU construction, reused the exact
font/metrics value, sized final panes before GPU, and ran a finalized native factory
inside the existing PTY session thread. The worker handle covered pending/running/
failed startup and prevented duplicate starts. It added no async runtime/thread pool,
provisional dimensions or general-purpose buffering. Parser/state/replies stayed on
the event loop; early output remained in the existing bounded queue, and initial
redraw/user-event/deferred-drain paths preserved the wake until a valid first frame.
No early output was parsed/rasterized before first presentation. Async failure was
reported through a pane notice and logging. These runtime/API changes and their new
tests were all reverted to HEAD. Existing production startup is unchanged.

## Method and limitations

Windows x86_64 release builds, five successful runs per shell per variant. Baseline
logs: target/startup-overlap-short-path/{configured,no-profile,cmd}-1..5.log.
Experiment: target/startup-overlap-after with the same names. Generated JSON summary:
target/startup-overlap-results.json. Logs/artifacts remain untracked under target.
Normal pwsh uses an explicit configured LocalShell with empty custom args (profiles
enabled); NoProfile uses a Command workspace with -NoProfile -NoExit and the existing
integration hook. cmd uses a Command workspace. PowerShell 7.6.6 package executable:
C:\PROGRA~1\WindowsApps\Microsoft.PowerShell_7.6.6.0_x64__8wekyb3d8bbwe\pwsh.exe.
The harness was hosted by Windows PowerShell with process-only execution-policy Bypass.

Earlier alias-hosted profile/no-profile launches failed and are excluded. Their
underlying access-denied cause was not isolated. The baseline generated harness
accidentally appended duplicate scenarios; overwritten final n=5 logs are used,
not ten independent samples. The corrected after batch ran exactly three scenarios
once per iteration. Thus sequencing/cache state and external machine load were not
perfectly matched. No claim of cold-cache performance or statistical significance.

All 15 after runs reached first output rendering; configured/NoProfile reached OSC
133 readiness in 5/5 each. cmd does not emit the readiness marker; its prompt-ready
time is unavailable. prompt-ready is **app observation of parsed OSC 133**, not
the native instant at which the child generated it. Early shell CPR/readiness
handshakes and bounded backpressure can await event-loop processing after first
frame; this is why child-spawn improvement is much larger than prompt improvement.
That mechanism is plausible from ownership/order, not an isolated measured cause.

Absolute timestamps below are elapsed ms; each cell is median [min, max]. The supplied
historical 130–150 ms first-frame figures are not comparable to this machine's
current 675–704 ms baseline. Do not describe these numbers as a retained improvement.

### cmd.exe

| Milestone | Before | Experimental after | Median delta (after − before) |
| --- | --- | --- | --- |
| First frame | 703.92 [677.51, 748.11] | 679.84 [674.39, 887.53] | -24.09 ms |
| Window shown | 717.95 [685.34, 755.53] | 692.36 [686.89, 901.51] | -25.60 ms |
| Child spawned | 744.84 [714.35, 784.22] | 82.66 [79.61, 94.26] | -662.18 ms |
| First PTY bytes | 755.13 [724.74, 794.04] | 91.44 [88.61, 112.91] | -663.69 ms |
| First shell output rendered | 792.32 [767.23, 831.33] | 731.64 [717.09, 947.70] | -60.67 ms |
| Prompt ready | N/A (no OSC 133 in cmd) | N/A (no OSC 133 in cmd) | N/A |

### pwsh -NoProfile

| Milestone | Before | Experimental after | Median delta (after − before) |
| --- | --- | --- | --- |
| First frame | 675.48 [656.85, 719.35] | 700.81 [691.88, 790.45] | 25.33 ms |
| Window shown | 684.05 [666.53, 729.13] | 713.09 [704.41, 810.46] | 29.04 ms |
| Child spawned | 929.81 [868.43, 975.07] | 289.01 [277.10, 332.03] | -640.80 ms |
| First PTY bytes | 939.59 [880.91, 985.55] | 298.23 [286.93, 341.83] | -641.36 ms |
| First shell output rendered | 1389.43 [1358.49, 1438.03] | 1155.00 [1146.69, 1292.39] | -234.43 ms |
| Prompt ready | 1363.39 [1335.97, 1421.66] | 1134.06 [1116.13, 1274.27] | -229.33 ms |

### configured normal pwsh

| Milestone | Before | Experimental after | Median delta (after − before) |
| --- | --- | --- | --- |
| First frame | 684.95 [658.88, 712.77] | 705.84 [689.03, 863.79] | 20.89 ms |
| Window shown | 699.08 [668.19, 722.13] | 715.86 [701.80, 876.55] | 16.77 ms |
| Child spawned | 948.51 [902.44, 985.13] | 306.73 [284.43, 332.05] | -641.79 ms |
| First PTY bytes | 958.49 [911.49, 996.34] | 315.42 [303.45, 341.33] | -643.07 ms |
| First shell output rendered | 1482.32 [1431.31, 1539.53] | 1228.99 [1206.64, 1431.26] | -253.33 ms |
| Prompt ready | 1462.69 [1404.03, 1513.11] | 1217.02 [1179.36, 1404.83] | -245.67 ms |

### Serialized baseline intervals

GPU time is contained in renderer time; do not sum them. Renderer constructor here
includes fonts/GPU/surface, excludes lazy first-frame pipeline setup.

| Interval (ms) | cmd | NoProfile | Normal pwsh |
| --- | --- | --- | --- |
| GPU initialization | 166.35 [152.92, 172.26] | 158.41 [153.69, 160.16] | 165.35 [156.12, 169.31] |
| Renderer constructor | 200.88 [184.43, 216.07] | 192.48 [183.61, 195.17] | 198.20 [189.15, 204.75] |
| Renderer after GPU | 4.66 [4.51, 5.95] | 5.53 [4.76, 6.97] | 5.78 [4.86, 6.53] |
| Native PTY creation | 12.17 [9.49, 13.09] | 11.13 [9.16, 12.93] | 13.75 [9.38, 16.13] |
| Native process spawn | 15.14 [11.52, 15.91] | 234.34 [161.53, 285.55] | 239.57 [208.10, 259.11] |
| Spawn → first PTY bytes | 10.39 [9.82, 11.87] | 10.49 [9.78, 12.48] | 9.98 [8.58, 13.22] |
| Spawn → parsed prompt ready | N/A (no OSC 133 in cmd) | 436.49 [406.16, 484.01] | 501.59 [488.29, 564.60] |
| Fonts-ready → original PTY dispatch | 671.24 [642.80, 707.96] | 638.45 [625.09, 683.88] | 653.10 [625.75, 677.93] |
| Original grid-ready → PTY dispatch | 494.79 [483.98, 535.51] | 472.86 [464.49, 518.34] | 482.26 [462.33, 502.82] |

The fonts/grid milestone gaps are maximum **schedule advance opportunities**, not
real wall-clock gains. Before final font metrics or workspace selection, there is
no acceptable exact-size candidate. At renderer-ready/grid-ready, only first-frame
work can overlap. At fonts-ready, GPU plus first-frame work can overlap. At
first-frame/window-shown, there is no meaningful remaining overlap with initialization.

### Actual experimental overlap

Native startup interval is [pty-create-begin, child-spawned]. Measured overlap is
the intersection of that interval with GPU setup, or with renderer setup from
renderer-concurrent-begin to renderer-ready; no gain is inferred from this alone.

| Interval (ms) | cmd | NoProfile | Normal pwsh |
| --- | --- | --- | --- |
| Native startup ∩ GPU setup | 25.63 [24.07, 27.66] | 164.78 [157.64, 181.50] | 167.43 [164.14, 220.71] |
| Native startup ∩ renderer setup | 25.63 [24.07, 27.66] | 168.84 [161.93, 187.88] | 172.53 [168.62, 220.71] |
| Child-spawned lead before renderer-ready | 137.50 [135.01, 229.18] | 0.00 [0.00, 0.00] | 0.00 [0.00, 64.97] |
| Child-spawned lead before first frame | 600.23 [591.71, 793.28] | 402.87 [374.73, 505.18] | 385.16 [356.98, 579.36] |

Child spawn advanced 641–662 ms at the medians; PowerShell parsed prompt readiness
advanced 229–246 ms. NoProfile/normal first frames regressed 25.33/20.89 ms; cmd
improved 24.09 ms at the median but its maximum increased from 748.11 to 887.53 ms.
Actual overlap in the **retained production code is zero**.

## Correctness, sizing, integration and shutdown evidence

Focused experimental tests passed: terminal-pty worker tests 11/11; terminal-app
129 unit tests plus existing integration suites (3 + 2 + 3 tests). New tests covered
one gated factory start, exact initial size, ordered early output exceeding bounded
queue capacity, ordered writes/resize, pending-factory shutdown/termination/join,
factory failure, reader-acquisition failure cleanup, preserved pre-frame wake,
renderer-absent parser/state updates, coalesced invalidation, exact metrics over
multiple physical-size/DPI grids without unconditional second resize, and visible
startup-error notice. Existing prompt-version/startup-command and pane/grid tests
remained unchanged. No wall-clock thresholds were added. Tests were removed along
with the rejected implementation; this is evidence of the experiment, not new
coverage present in final source.

Native release evidence covers successful cmd and both pwsh launch/output/readiness,
not visual correctness, real multi-monitor DPI, maximized/split/Saved Workspace,
custom args/Git Bash or failure injection. Exact-size source reuse and deterministic
grid tests support the proposed sizing calculation; they do not prove all native
startup sizing cases. No provisional sizing was tried. OSC 7/profile/OSC 133 hooks
and readiness-gated startup dispatch were not changed. No native claim about
startup-command or Saved Workspace acceptance is made.

Native cleanup is unresolved: 14/15 after runs needed forced parent termination
after CloseMainWindow did not exit within 3 seconds; one cmd exited gracefully.
A pre-experiment baseline terminal also accepted close but remained alive and
locked the release executable. It was force-stopped only after verifying its
exact repository executable path; the user's installed Terminal was untouched.
Therefore shutdown trouble predates overlap, but equivalence is not established.
Child/process inventory via CIM was denied; child/PTY/handle leaks cannot be ruled
out or asserted. Forced parent exit is not clean teardown evidence.

The experimental factory teardown disconnected the event receiver before terminate
and join, including pending-spawn and full-queue cases. It could not cancel an
in-flight native spawn and could wait for it. Native renderer failure with a live
child, close-during-GPU/spawn, OS handles and real-thread cleanup were not accepted.
Reverting removes those new failure interactions; existing shutdown reliability
remains a separate investigation, not a fix claimed by this task.

## Retained changes and next investigation

Only this report/plan and measurement script improvements remain. The script accepts
an explicit executable/config, rejects missing configured prompt when requested,
avoids empty Start-Process ArgumentList, restores child diagnostic/config environment,
reports forced parent shutdown, and can fail on it with -RequireGracefulShutdown.
No renderer/runtime/API/dependency/architecture changes remain; no ADR, roadmap or
current-state behavior update is warranted. No commit or push.

First-frame logs show approximately 200+ ms each in rectangle-pipeline and
glyph-pipeline initialization; those costs dominate the hundreds of milliseconds
after renderer-ready. The next startup task should measure and reduce that first
valid frame's pipeline creation cost, with shutdown reliability investigated before
any renewed overlap acceptance. Do not choose wgpu lazy initialization blindly: GPU
setup itself is about 160 ms and delaying it could merely shift work into presentation.

## Exact native release acceptance checklist (future overlap candidate)

Native Windows app automation is unavailable. These checks remain unperformed;
they would block accepting a renewed production candidate. There is no retained
overlap release requiring visual acceptance now. Build cargo build --release
-p terminal-app and use target/release/terminal.exe; final rebuild restores the
pre-experiment executable. Save fixtures under target, not the real user store.

Run the script from Windows PowerShell with -NoProfile -ExecutionPolicy Bypass
-File scripts/measure-pty-startup.ps1 -Runs 5 -PowerShellProgram <real exe>
-ConfiguredConfig <fixture> -RequireConfiguredPrompt -OutputDirectory <fresh directory>.
Use [shell] program = <TOML-escaped real pwsh path>, args = [] in the fixture.
RequireGracefulShutdown makes any forced parent shutdown fail explicitly. Keep
font/theme/workspace/DPI fixed; run matched interleaved variants, exclude failed
spawns, and capture cold/warm caches separately.

1. Launch cmd, NoProfile, normal configured pwsh: no white flash; hidden until valid
   frame; correct font/background; one prompt; complete output; unchanged focus/
   taskbar. Type immediately at show and verify exactly one input dispatch.
2. Default/small/large windows and 100/125/150/200 percent DPI: query
   $Host.UI.RawUI.WindowSize (pwsh) or mode (cmd); compare exact rows/columns with
   pane layout. Print dimensions from a startup Command before output. No transient
   incorrect wrap/grid. No configured maximized-start setting currently exists.
3. Immediately maximize/restore/resize and move across monitors: correct sizes and
   no gratuitous initial resize, lost input or delayed first frame.
4. Saved Workspace with tabs/splits, distinct CWDs, active pane and startup commands:
   every pane starts once with correct size/root/title; commands wait for OSC 133;
   replacing workspace terminates old children.
5. Custom args/Command/Git Bash: preserve selection, profiles, OSC 7 CWD and output.
   Verify no duplicate prompt and immediate resize/typing.
6. Fault-inject GPU/first-frame failure after dispatched spawn; invalid shell path;
   close during spawn/GPU and heavy output: useful errors, renderer not deadlocked,
   verified child/PTY/handle/thread cleanup. Check with native process inventory.
7. Disable diagnostics: normal startup remains silent.

## Final validation

Final reverted-tree validation passed (exit 0 for each):

- cargo fmt --all -- --check
- cargo check --workspace --all-targets --quiet
- cargo test --workspace --all-targets --quiet (zero failed; four existing ignored tests)
- cargo test --workspace --doc --quiet
- cargo clippy --workspace --all-targets -- -D warnings
- git diff --check

Final cargo build --release -p terminal-app --quiet also passed, restoring the
release executable to unchanged production behavior. PowerShell parser validation
of the retained script passed. A one-run-per-scenario release smoke test of the
retained harness passed readiness checks for configured normal pwsh, profile
Command, NoProfile and cmd (exit 0). All four required forced parent termination,
now visibly reported by the harness; child cleanup remains unverified. This smoke
run used the rebuilt, reverted executable and confirms the shutdown concern
persists without overlap. It is script verification, not a new before/after sample.
Windows ARM64 is out of scope.
