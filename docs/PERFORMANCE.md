# Performance

The fixed-workload M6 parser/state/projection measurements and reproduction commands are in [BASELINES.md](BASELINES.md). The interactive traces below cover additional app, renderer, and GPU stages.

Correctness and architectural clarity take priority over performance complexity.

## Initial goals

- Cold startup ideally below 100 ms where practical; below 150 ms is initially acceptable
- Stable 60 FPS terminal rendering
- Idle CPU usage approaching zero
- Immediate-feeling input
- Bounded and reasonable memory growth
- No busy polling or unnecessary redraws

## Rules

- Treat targets as budgets measured on named hardware and operating systems, not universal claims.
- Establish baselines before adding performance-specific complexity.
- Profile before replacing dependencies, adding threads, introducing `unsafe`, or performing micro-optimizations.
- Bound scrollback, queues, parser payloads, glyph atlases, and frame work.
- Coalesce resize/redraw work and render only when state changes.

## Measurements to add with implementations

- Cold and warm time to first usable frame
- Idle CPU and resident memory
- Input-to-present latency distributions
- Parser throughput for plain and escape-heavy streams
- Sustained-output behavior and queue pressure
- Resize-storm and high-DPI frame cost
- Glyph atlas hit, miss, and eviction rates
- Release binary size

Record the tool, build profile, commit, OS, CPU, GPU, display scale, and sample method with every baseline. Do not create hard regression gates until measurement noise is understood.

## Interactive frame-pacing investigation (2026-09-23)

The measured base revision was `1cdebdd` on Windows 11 Pro build 26200, an Intel Core i5-1250P and Intel Iris Xe Graphics at display scale 1.0. The app was built with `cargo build --release -p terminal-app` and traced with `TERMINAL_RENDERER_DIAGNOSTICS=1`. A PowerShell Win32 input driver launched the native executable, sent input, and captured stderr. The driver did not reliably deliver character keys, mouse movement, or continuous size changes. A user then performed the requested gestures in a visible release window with the first implementation, followed by another continuous resize after the resize-ordering correction. These are single traces, not latency distributions or paired trials of equal gestures. Keep diagnostics disabled for subjective release rechecks.

| Observed operation | Before | After | Finding |
| --- | --- | --- | --- |
| Initial PTY output at 80 × 31 cells | 5 parser feeds, 5 PTY wakes/drains, 3 projections/submissions/presents; warm output frames wrote 80,112 and 80,880 instance bytes | 5 parser feeds, 4 PTY wakes/drains, 3 projections/submissions/presents; warm output frames wrote 752 and 1,520 instance bytes | Every frame still projects and generates 2,480 cells. Changed-span instance uploads eliminate most repeated transfer. The wake counts are timing-dependent, so one run does not establish a wake reduction. |
| Selection anchor with no selected range | One unchanged frame in the intermediate trace: 2,480 projected cells, 15 cache hits, zero buffer bytes, one submission/present taking 11,172 µs after acquire | No redraw request or present at the selection anchor; moving the focus still invalidates | The anchor itself has no visible effect. Replacing an earlier selection still requests a redraw. |
| One accepted space key during shell startup | No comparable pre-change key trace | One keyboard event and one PTY write; a later drain had 2 feeds/13 bytes and one wake, followed by one frame with 2,480 cells, 31 shape-cache hits, zero raster calls, one 32-byte buffer write, one submission/present (11,918 µs after acquire) | The key was delivered before the prompt; the echo attribution and input-to-present latency are not reliable. |
| PageUp/PageDown with no retained history | Two navigation events, zero viewport changes and redraws | Same | This does not represent scrollback navigation with history. |
| Automated typing burst, selection movement, continuous resize | Events were not delivered reliably | Manual gesture counts below | Startup had three resize events coalesced into one 800 × 600 batch, with zero surface configures or redraw requests from that batch. |

For the comparable warm PTY output frames, projection took 94/88 µs before and 92/145 µs after; instance generation took 4,397/18,352 µs before and 5,919/13,330 µs after; acquire-to-present took 22,975/27,886 µs before and 18,404/22,057 µs after. These isolated cold-font/GPU samples are too noisy to claim a latency percentage. They do show the transfer change: about 80 KB per frame became 0.75–1.5 KB, and a fully unchanged frame needs zero instance writes. Buffer allocation on the first glyph frame and GPU present remain significant.

The implementation now retains instance contents and writes only changed contiguous spans, with a cap on write count; caches shaped text at the current font scale; and checks the glyph atlas before CPU rasterization. The app coalesces pending PTY proxy wakes and omits the initial selection-anchor redraw. The shaping cache is bounded to 512 strings and clears on font-scale changes. Focused tests cover cache reuse, scale invalidation, eviction, and changed upload ranges. Applying a queued resize immediately before a redraw now keeps the surface and terminal grid at the latest window size even if `RedrawRequested` arrives before `about_to_wait`.

### Manual release gestures

The first manual trace used the changed-span upload and shaping cache, before the resize-ordering correction. Counts below are exact for the listed diagnostic sequence spans; timings are accumulated stage times in microseconds. A redraw entails one projection, queue submission, and present in these spans. `present` is measured after surface acquisition and includes submission, so stage times must not be added to it. PTY drain time includes its invalidation call; the parser feed itself is much shorter. The trace has no reliable wall-clock input-to-present timestamp. The diagnostics count accepted selection, viewport, and grid changes, but do not count individual parser-driven `TerminalState` mutations within a feed; parser-feed counts must not be read as mutation counts.

| Gesture (sequence) | Input, PTY, redraw work | CPU/GPU work and stage times |
| --- | --- | --- |
| One `a` with shell echo (59–78) | 1 key; 2 wakes, 2 queued/drained PTY chunks, 47 bytes and 2 feeds; 1 accepted invalidation/redraw | 2,480 projected cells, 98 µs projection; 41 shape calls/3 misses, 3 rasters; 7,212 µs instance generation; 2 writes/2,000 bytes, 78 µs upload; 313 µs submission, 14,600 µs acquire-to-present |
| `b`–`f` and Backspace (79–259) | 6 keys; 12 wakes/chunks/feeds, 588 bytes; 11 accepted invalidations/redraws | 27,280 projected cells/1,134 µs; 871 shape calls/11 misses, 11 rasters; 53,433 µs generation; 12 writes/4,416 bytes, 492 µs upload; 2,164 µs submission, 145,199 µs acquire-to-present |
| One drag (260–453) | 1 start, 36 moves, 1 end; 13 accepted invalidations/redraws | 32,240 projected cells/1,252 µs; 1,079 shape calls/0 misses or rasters; 60,632 µs generation; 27 writes/12,160 bytes, 940 µs upload; 1,979 µs submission, 172,630 µs acquire-to-present |
| One PageUp with history (4170–4183) | 1 navigation event; 1 accepted invalidation/redraw | 3,201 projected cells/150 µs; 66 shape calls/0 misses; 4,550 µs generation; 2 writes/3,200 bytes, 91 µs upload; 178 µs submission, 12,477 µs acquire-to-present |
| One PageDown with history (4242–4255) | 1 navigation event; 1 accepted invalidation/redraw | 3,014 projected cells/123 µs; 924 shape calls/0 misses; 6,278 µs generation; 3 writes/110,112 bytes, 88 µs upload; 186 µs submission, 14,806 µs acquire-to-present |

The key trace shows shell output sometimes arrives in two chunks across separate frames. For `b`, a 9-byte control chunk caused one zero-upload cursor-change present, followed by an 84-byte chunk and another present. The six-key burst therefore made 11 presents. PTY drain took 12,110 µs for `a` and 132,102 µs total for the burst, while parser feeds took 7 and 55 µs; the present capture and event-loop/invalidation cost dominate parsing. The trace does not prove a safe way to skip the first cursor change without changing visible behavior.

The first continuous resize contained 134 `Resized` events, only 6 applied resize batches/grid/PTY resizes, 141 surface configurations, and 147 projections/submissions/presents (407,442 projected cells, 17,416 µs projection, 765,959 µs instance generation, 851,136 uploaded bytes, 2,081,068 µs acquire-to-present). A later portion isolated 27 size events but only 1 applied batch/grid/PTY resize, 28 configurations, 27 presents, and **zero** instance writes: all 27 acquisitions were suboptimal against a stale configured size. The app was consuming redraws during the drag before its queued resize reached `about_to_wait`.

After applying the queued size at the start of `RedrawRequested`, a second manual drag contained 21 size events, 21 applied batches/grid/PTY resizes, 21 surface configurations, zero suboptimal acquisitions, 32 accepted and 10 coalesced invalidations, and 32 projections/submissions/presents. The 11 additional frames followed 21 PTY output chunks from shell repaint during resizing. It projected 79,171 cells in 4,312 µs, generated instances in 155,314 µs, uploaded 1,680,464 bytes in 63 writes/1,864 µs, and spent 439,544 µs from acquisition through present. Gesture paths and duration differed, so these totals are evidence of correct resize ordering and removal of stale-size reconfiguration, not a comparable overall resize speedup.

Source inspection found no full-grid copy in `TerminalState::begin_selection`, `extend_selection`, or `page_up`/`page_down`. A rendered viewport change still constructs a fresh `TerminalRenderData` with a `String` for each visible cell and regenerates all background/glyph instances. The renderer clears and draws the full surface for every submitted frame. This is visible even on a zero-upload frame: it projects all 2,480 cells and regenerates instances. The upload optimization therefore reduces bytes transferred, while full projection, shaping-cache lookup, instance generation, queue submission, and present remain. Remaining work is a real damage-aware projection/instance update, proof-based suppression of redundant PTY redraws, and input-to-present latency measurement. Preserve surface restore, alternate-screen, selection, scrollback, and DPI correctness when skipping a frame. An exact before-change manual trace for all five gestures is unavailable because automated delivery was intermittent; do not claim a per-gesture latency percentage from these data.

### Windows release recheck

From the repository root in PowerShell, run `cargo build --release -p terminal-app`. For measured gestures, create the ignored log directory with `New-Item -ItemType Directory -Force target/perf`, set `$env:TERMINAL_RENDERER_DIAGNOSTICS='1'`, then run `Start-Process -FilePath .\target\release\terminal-app.exe -Wait -RedirectStandardError .\target\perf\manual_recheck.log`. In the focused terminal window, type one character and wait for echo; type `abcdef` and Backspace; drag across the prompt with the left mouse button; enter `1..150` in PowerShell to create scrollback, then press PageUp and PageDown once each; continuously drag a window edge through several sizes. Pause briefly between operations so their log spans can be identified. Close the app to finish the log. Inspect `target/perf/manual_recheck.log` for `app event=` and `renderer frame=` records, including PTY wakes/drains, projection cells, shape misses, buffer writes/bytes, surface configurations/suboptimal acquisitions, and presents. For each live resize, verify the configured size matches the latest physical size before the frame. Clear diagnostics with `Remove-Item Env:\TERMINAL_RENDERER_DIAGNOSTICS`, then run `cargo run --release -p terminal-app` and repeat the same gestures for the subjective pacing check. A paired before-change manual trace is required before claiming per-gesture latency improvements.
