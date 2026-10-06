# First-frame pipeline costs

Investigation on `perf/first-frame-pipeline-costs`, based on `b8f011f` plus the timing-only changes in this working tree. No lazy initialization, backend change, PTY overlap, commit, or push was performed.

## Result

The final 15 native Windows x86_64 release launches measured **140.833 ms (125.185–151.663)** from `fonts-started` to `first-frame-presented`, and **162.037 ms (147.633–182.911)** from Rust main to that milestone. Instance/adapter/device initialization dominates at **85.818 ms** per-run aggregate median. CPU font preparation/metrics follows at **31.022 ms**; metrics alone cost **0.098 ms**. Shader/layout and pipeline creation costs **12.413 ms**. Initial grid and projection work is negligible here.

Whole-wgpu lazy initialization is **not justified as the next task if the existing first frame must be preserved**. That frame needs the wgpu device, queue, surface and rectangle pipeline. Moving their creation later moves the same work before presentation, or changes the first-frame contract. A narrower experiment conditionally creating glyph pipeline/layout/atlas on the first actual glyph is better supported: these resources cost **8.178 ms (7.435–10.887)** together and are unused by this empty, cursor-bearing frame. This is an upper bound on removable measured work, not a demonstrated speedup or an implemented change.

## Exact dependency path

1. `crates/app/src/main.rs::create_window_and_renderer` creates the hidden native window and calls `Renderer::new_with_font_settings`.
2. `fonts-started` → `FontSystem::load_system`: system font discovery, primary/generic family resolution, fallback ordering, cache setup → `cell_metrics`: scaled CPU font metrics → `fonts-ready`. Font loading/metrics uses no wgpu device.
3. `gpu-started` → `initialize_gpu`: `Instance::new`, `create_surface`, blocking `request_adapter`, blocking `request_device` returning **device and queue together** → `gpu-ready`. Queue construction cannot be isolated as a separate public wgpu call. Existing Windows DX12-first selection and fallback remain unchanged.
4. Renderer reconfiguration obtains surface capabilities/default configuration and calls `surface.configure` → `renderer-ready`.
5. App initializes grid sizes → `initial-grid-ready`, prepares UI/pane layout and directly attempts the hidden first redraw → `first-redraw-requested`. The existing retry path remains intact.
6. `Renderer::redraw_panes` projects terminal data; `redraw_data` acquires the surface texture and constructs `DrawResources` on its first use. This creates the shader/layout, rectangle pipeline, glyph pipeline and glyph atlas, even if there are no glyphs. It creates the frame texture view.
7. `DrawResources::draw_pane` generates CPU instances, allocates/uploads needed instance buffers, creates an encoder/render pass, records draws and finishes the command buffer, then submits to the queue. Empty glyph/overlay buffers are not allocated. This workload still draws its cursor rectangle.
8. Renderer calls `pre_present_notify` and `frame.present`, handles the existing suboptimal outcome if necessary, and returns. App records `first-frame-presented`, then shows/focuses the window (`window-shown`), then starts PTYs (`pty-started` → `pty-spawn-complete`). Every final sample obeyed this order and subsequently reached `first-shell-output-rendered`.

## Method and retained evidence

Measured 2026-10-05, Windows 11 Pro build 26300, Intel i5-1250P, Intel Iris Xe driver 32.0.101.7088, native AMD64 / `x86_64-pc-windows-msvc`, rustc 1.98.1. Release binary SHA-256: `9008E419917F4D65EB0390CA093D75BBE9B461882168F969547C3642C0157087`. No `WGPU_BACKEND` override; a separate verbose probe reported DX12. One configured `cmd.exe /d /k` pane, 800×600 at scale 1, default 16-pixel font, 10×19 cells, 80×31 grid. This is the pre-shell frame, not a restored text workload.

Startup timing was enabled and written to a file; aggregate renderer diagnostics were disabled for timed runs. Fresh process per sample, sequential launches, 250 ms between samples after cleanup; caches were not cleared. Final batch was taken after build/test validation finished. No samples in that successful batch were excluded. Raw microsecond scopes, per-run totals, environment and workspace are retained under [measurements/first-frame-pipeline-costs](measurements/first-frame-pipeline-costs/samples.csv).

| Batch (15 samples each) | Main → first present median, min–max ms | Fonts start → first present median, min–max ms |
| --- | ---: | ---: |
| Baseline, pre-change release, startup diagnostics only | 147.254, 138.375–159.427 | 128.898, 120.932–138.134 |
| Instrumented confirmation batch | 146.301, 141.799–164.198 | 126.152, 121.626–145.610 |
| Final instrumented batch, validation finished | 162.037, 147.633–182.911 | 140.833, 125.185–151.663 |

These sequential, unpaired batches show machine/run drift and do **not** prove an improvement, a regression, or the instrumentation overhead. The confirmation batch may overlap validation. Earlier verbose-renderer probes are excluded from the primary numbers because synchronous diagnostic output changes measured cost. An early CIM-permission failure and one harness setup failure (missing workspace before renderer initialization) produced no eligible first-frame samples and are excluded. The harness was corrected and the full final batch rerun.

Reproduce from the repository root (use a fresh output directory):

```powershell
cargo build --release -p terminal-app --quiet
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-first-frame.ps1 -Runs 15 -OutputDirectory target/first-frame-new
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/summarize-first-frame.ps1 -OutputDirectory target/first-frame-new
```

The measurement script supports `-Workspace` for an explicit workspace and `-RendererDiagnostics` for metadata/debug probes. It records environment and binary identity, checks presentation-before-PTY ordering and shell-output readiness, closes each launch, checks its observed descendants, and records forced cleanup. CIM process inventory requires sufficient local permissions. The summary sums repeated scopes within each run, then computes medians and min/max; it also exports `grouped.csv`.

## Timing breakdown

All values below are final-batch **milliseconds**, median and observed min–max across 15 runs. Parent scopes are inclusive; indented child labels below belong to their named parent and must not be added again. Timings are CPU wall-clock duration of work/API calls, including driver waits, not GPU execution timestamps.

| Non-overlapping category | Median | Min–max |
| --- | ---: | ---: |
| CPU font preparation + metrics | 31.022 | 27.397–42.588 |
| wgpu instance + adapter + device/queue | 85.818 | 76.712–91.618 |
| Surface create + capabilities + configure | 5.299 | 4.212–7.905 |
| Shader/layout + rectangle/glyph pipelines | 12.413 | 11.537–15.913 |
| Atlas + frame texture view + instance buffers | 2.113 | 1.788–2.717 |
| Grid + UI/layout + projection + instance generation | 0.169 | 0.131–0.218 |
| Acquire + encode/finish + submit + notify + present | 1.617 | 1.109–3.290 |
| Sum of measured scopes per run | 140.164 | 124.687–150.942 |
| Unscoped elapsed remainder per run | 0.669 | 0.498–0.935 |

Medians of categories do not sum to the median total. The remainder is computed separately for each run against `fonts-started` → `first-frame-presented`; it includes diagnostics between scopes, dispatch, bookkeeping and other untimed work. Nested logging time can appear inside a parent. Grouping separates CPU font work from GPU-related host API work; shader parsing/pipeline construction also includes CPU/driver work.

| Scope | Median | Min–max |
| --- | ---: | ---: |
| font-family-preparation (inclusive) | 30.936 | 27.302–42.411 |
| ↳ system-font-discovery | 29.602 | 26.002–40.124 |
| ↳ font-family-resolution | 0.017 | 0.013–0.178 |
| ↳ fallback-candidate-ordering | 1.252 | 1.069–2.150 |
| ↳ font-cache-setup | 0.004 | 0.003–0.011 |
| font-metrics | 0.098 | 0.053–0.177 |
| wgpu-instance | 6.407 | 5.808–8.639 |
| surface-create | 0.033 | 0.028–0.036 |
| adapter-request | 37.161 | 32.250–42.560 |
| device-request (device + queue) | 42.015 | 38.062–44.178 |
| surface-capabilities-and-config | 0.011 | 0.009–0.012 |
| surface-configure | 5.255 | 4.165–7.862 |
| initial-grid-resize | 0.130 | 0.099–0.169 |
| initial-ui-and-pane-layout | 0.003 | 0.002–0.016 |
| initial-projection | 0.028 | 0.023–0.042 |
| surface-acquire | 0.057 | 0.044–0.185 |
| shader-and-layout (inclusive) | 0.412 | 0.373–0.786 |
| ↳ shader-module-create | 0.335 | 0.313–0.613 |
| ↳ glyph-bind-group-layout | 0.048 | 0.037–0.327 |
| rectangle-pipeline | 5.633 | 5.068–9.212 |
| glyph-pipeline | 6.144 | 5.725–8.772 |
| glyph-atlas (inclusive) | 1.857 | 1.553–2.372 |
| ↳ atlas-texture-create | 1.542 | 1.286–1.924 |
| ↳ atlas-view-sampler-bind-group | 0.246 | 0.168–0.504 |
| frame-texture-view | 0.012 | 0.009–0.021 |
| instance-generation | 0.003 | 0.002–0.003 |
| buffer-create-upload (inclusive) | 0.314 | 0.185–0.549 |
| ↳ instance-buffer-allocation | 0.129 | 0.064–0.227 |
| ↳ instance-buffer-writes | 0.127 | 0.086–0.359 |
| render-encoding (includes encoder.finish) | 0.589 | 0.459–1.229 |
| queue-submit | 0.463 | 0.289–1.168 |
| pre-present-notify | 0.000 | 0.000–0.000 |
| first-present | 0.344 | 0.287–0.840 |

Zero means below integer-microsecond resolution, not absence of work. Before `fonts-started`, final-batch window creation/branding alone had median 15.460 ms (13.210–22.757); this is outside the renderer-start interval.

## Required work and defer candidates

| Work | First-frame requirement / next investigation |
| --- | --- |
| Primary font resolution and metrics | Required for identical cell geometry, grid sizing and cursor. Metrics are too small to justify deferral. Investigate discovery/loading strategy while preserving configured families and fallback behavior; do not assume scanning all installed fonts is the only possible implementation. |
| Full discovery and fallback ordering/cache setup | Current font implementation performs them before metrics. Fallback ordering and caches could potentially wait for actual glyphs, but discovery is coupled to primary resolution and platform font semantics; a safe replacement needs its own correctness investigation. |
| Instance, compatible adapter, device/queue | Required by the current wgpu first frame. Largest host/driver cost; useful target for initialization analysis, but whole-wgpu laziness alone does not remove it. |
| Surface configuration/acquisition | Required for submitting the same frame. No supported deferral beyond presentation. |
| Shader and rectangle pipeline/buffer | Required for cursor/background/decorations in this workload. Changing to a clear-only frame would change behavior. |
| Glyph pipeline, glyph layout and atlas | Best bounded defer candidate: unused until nonempty glyph draws. Must initialize before first text/overlay draw, handle atlas ownership and surface-format rebuilds, and test nonempty initial UI/workspace cases. Can move latency to first shell text; measure both milestones. |
| Grid/projection and encoding/submit/present | Required for the same content and submission; measured costs small. |

## Shutdown, validation and limits

Shutdown investigation was limited to cleanup isolation. `PtyWorker::Drop` uses `shutdown_and_join`: it discards event reception, sends termination, drops command sending and joins the worker. No shutdown code was modified. Each final run observed two descendants, reached shell output, and closed with **zero forced cleanup**, taking **99.51–125.84 ms** for app/observed-child cleanup. The confirmation batch also closed without force. This does not reproduce, diagnose or fix the earlier overlap shutdown issue, nor establish safety for other shells/workloads. Cleanup occurs after the timed interval, and launches are sequential.

Release build, focused startup diagnostics tests, formatting, workspace all-target check/tests, doc tests, Clippy with warnings denied, and whitespace checks passed. Existing startup lifecycle tests remain unchanged. The expanded scopes reuse disabled-path startup diagnostics (no clock reads when disabled) and retain old milestone/scope names. No runtime initialization order or architecture boundary changed.

Limitations: one host/driver/backend and default one-pane empty frame; warm OS/font/driver caches, uncontrolled background load/power/thermal state, sequential batches, synchronous log observer effect. No fresh-boot/cold-cache distribution, compositor scanout timing, GPU completion timestamps, other platforms/adapters, or new manual visual/input acceptance. `first-frame-presented` means submission before native show, not visible pixels. API scopes do not isolate driver internals, and device/queue are a combined request. No latency preservation claim follows from these unpaired batches; instrumentation-only source review and existing tests support behavior preservation, not a precise overhead bound.
