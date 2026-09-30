# Startup presentation and timing

## Cause and fix

The app previously created a visible native window before loading fonts, initializing the GPU, and synchronously spawning pane PTYs. Its configured theme was available, but no terminal frame covered the native client area during this interval. That ordering exposed the unpainted window/compositor background observed as white. The Windows winit window class does not install an explicit white background brush; the fault is exposing the window before application content is presented.

The app now creates its window hidden using winit's portable visibility API. After renderer/theme initialization and DPI-aware workspace/grid sizing, it presents a complete empty terminal frame directly, shows the window once, and requests focus. PTY spawning follows that presentation. No shell output or prompt is required. Direct presentation avoids depending on hidden-window native paint delivery. Damage queued during configuration is rearmed after window creation so the first explicit redraw request is not coalesced away.

Successful suboptimal presents are distinguished from unsuccessful surface reconfiguration. Only a successful present consumes the one-time visibility gate. Failed/skipped outcomes leave it pending; initial acquisition receives up to three immediate attempts, then reports a useful error and exits rather than leaving a permanently hidden process. Renderer/window initialization errors still report to stderr and exit.

The first frame uses the existing configured font, theme, DPI, pane geometry and terminal state. PTY dimensions and shell/workspace/startup-command construction retain their existing paths. No new threads, dependencies, caches, async runtime, platform hacks or sleeps were added. On platforms where winit does not support explicit visibility (notably Wayland), mapping remains controlled by compositor/buffer attachment; the visibility calls cannot promise a hidden native window there.

## Opt-in diagnostics

Set `TERMINAL_STARTUP_DIAGNOSTICS=1` to emit monotonic elapsed microseconds from main entry. Set `TERMINAL_STARTUP_LOG` to redirect these records to a file (overwritten on each launch); otherwise they go to stderr. A file-open failure falls back to stderr. Normal launches emit no startup timing records, and disabled milestones do not read the clock. Existing renderer diagnostics remain separate.

PowerShell example:

```powershell
$env:TERMINAL_STARTUP_DIAGNOSTICS = '1'
$env:TERMINAL_STARTUP_LOG = "$PWD/startup.log"
& .\target\release\terminal.exe
Remove-Item Env:TERMINAL_STARTUP_DIAGNOSTICS, Env:TERMINAL_STARTUP_LOG
```

Stages include application-started, terminal-state-created, config-loaded (including workspace construction), window-created, fonts-started/ready, gpu-started/ready, renderer-ready, initial-grid-ready, first-redraw-requested, first-frame-presented, window-shown, pty-started, pty-config-ready, pty-spawned (per session), and pty-spawn-complete (all startup sessions). Failure stages cover window creation, renderer initialization and initial presentation.

Main entry excludes OS loading before Rust main. A successful present is a renderer/window-system submission, not proof of compositor scanout or observed pixels. Window-shown records completion of visibility/focus requests. PTY completion does not mean the shell has produced a prompt.

## Measurements

Five sequential native Windows x86_64 release launches per condition, using the same local default configuration and DX12 backend. The baseline build contained timing instrumentation before lifecycle changes; the final build contains the fix. Validation was idle for these samples. These are warm, non-interleaved launches, with no cold-cache or system-load controls. Sample files are local ignored artifacts under target/startup-baseline-{1..5}.log and target/startup-idle-{1..5}.log. Earlier exploratory/loaded samples are excluded.

| Elapsed milestone (ms) | Before, median (range) | After, median (range) |
| --- | ---: | ---: |
| config-loaded | 8.8 (5.5–16.4) | 13.7 (5.7–19.1) |
| window-created | 84.0 (53.4–89.4) | 51.1 (23.0–60.6) |
| fonts-ready | 113.0 (88.5–121.6) | 81.0 (68.9–108.7) |
| gpu-ready | 203.6 (182.5–219.6) | 196.3 (162.9–247.3) |
| renderer-ready | 210.0 (187.8–227.6) | 208.2 (168.0–255.2) |
| initial-grid-ready | 210.2 (188.0–227.8) | 208.5 (168.2–255.5) |
| first-redraw-requested | 1481.8 (1276.9–1552.4) | 208.8 (168.3–255.6) |
| first-frame-presented | 1481.6 (1275.5–1513.2) | 230.7 (193.1–277.3) |
| window-shown | 84.1 (53.6–89.4) | 287.2 (221.7–358.7) |
| pty-spawn-complete | 1454.5 (1253.9–1480.7) | 785.4 (714.6–984.3) |

| Stage duration (ms) | Before, median (range) | After, median (range) |
| --- | ---: | ---: |
| Font loading | 31.5 (29.0–34.9) | 41.1 (29.9–52.8) |
| GPU initialization | 90.9 (78.7–115.0) | 110.2 (90.5–145.4) |
| PTY startup | 1237.5 (1029.3–1265.2) | 448.7 (427.3–736.9) |

The median first presentation changed from 1,481.6 ms to 230.7 ms by removing synchronous PTY startup from its prerequisite chain. Renderer-ready medians stayed similar (210.0/208.2 ms). Native visibility/focus requests took 28.5–98.8 ms after presentation. The old window-shown milestone records immediately visible creation; the new milestone follows first presentation. Thus the configured frame is submitted earlier, while native visibility starts later than the old blank window.

The baseline first-redraw-requested can follow first-frame-presented: pre-window damage had already armed redraw coalescing, and the initial visible-window OS paint produced a frame before an explicit app redraw request. The rearming fix makes the new marker correspond to the first issued native redraw request.

PTY duration also varied considerably across batches. This change does not optimize PTY internals, so the lower final PTY timings are observations, not an attributed improvement. Shell prompt readiness and compositor scanout were not measured. No speculative parallelization or config/font deferral was justified by these samples.

## Validation and native acceptance

All required checks passed on Windows x86_64: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo test --workspace --doc`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`. The final release build also passed.

Focused tests cover the initial hidden state, empty-frame presentation without PTY output, one-time show behavior, successful suboptimal presentation, failed/skipped/reconfigured outcomes, and damage queued before window creation. Existing theme, workspace, startup-command and shell-integration tests remain intact.

Native Windows release launches completed presentation, visibility requests and PTY startup repeatedly, and closed normally. Normal release `--help` printed its usage without timing records. The user completed manual native Windows release acceptance: no visible white startup flash, immediate typing works normally, focus/taskbar behavior is normal, and sizing/placement are normal. Native UI automation was unavailable because the tool rejected Terminal with “product policy blocks this app”; the visual/input acceptance is user-reported. Native Linux/macOS startup was not checked locally.

## Remaining costs

Synchronous native PTY/session creation remains the largest startup operation and blocks event-loop input handling after the first window is shown. Showing the frame earlier improves measured first presentation; these timings do not establish faster shell readiness or quantify input latency. Manual Windows acceptance confirmed immediate typing works normally. Configuration/shell-integration preparation measured below 1 ms in the instrumented follow-up, so deferring it is unjustified. Font discovery/loading, GPU initialization and first-frame pipeline preparation remain visible costs. Visibility/focus requests also have measurable native cost.

A focused follow-up can investigate portable-pty/ConPTY creation internals and event-loop responsiveness using additional measurements before deciding on worker ownership. Persistent caches, single-instance/server architecture and general performance work are outside this task.

The subsequent [PTY and shell startup investigation](PTY_STARTUP.md) extends these opt-in milestones and reports native shell comparisons, ownership limits and the decision to retain the startup sequence.
