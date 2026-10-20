# Renderer optimization measurements (2026-10-20)

The retained change replaces the GPU atlas's formatted face/weight string with
a private typed identity. It preserves font face, effective weight, glyph ID,
physical size, bold and italic distinctions. On this host, median warm glyph
instance-generation CPU time fell 16–47% across the fixed workloads below.
This is a CPU preparation improvement, not a measured GPU execution or display
latency improvement. No terminal semantics, shaders, scheduling or cache policy
changed. No dependencies were added.

## Architecture and attribution

- `terminal-core` owns grid/history, Unicode occupancy, selection and reflow.
  `snapshot.rs` scans the complete visible rectangle and projects visible cells,
  resolving theme/selection/opacity without changing core state. It still scans
  compact historical padding because selected padding must remain visible.
- `gpu.rs::draw_pane` regenerates instances for every projected cell on each
  present. It consults the bounded shaped-text cache and GPU glyph atlas, clips
  ink to cell spans, then compares instance contents against previous buffers.
  Transparent panes use a composition target and background replacement pass.
- `InstanceBuffer::upload` already retains allocations and uploads changed spans,
  merging excessive fragmentation. Warm static frames in this measurement wrote
  **zero bytes**, with zero raster calls and buffer allocations. For dense text,
  baseline median projection was 151 µs/frame, generation 1,262 µs, upload
  comparison 41 µs, and encoding/submission 106 µs. Instance generation is the
  largest measured CPU stage. Those stages exclude GPU completion waits.
- Every glyph used to call `format!("{}:{:?}", face_id, weight)`, hash that
  allocated string, then compare strings while scanning the atlas LRU and clone
  the string when refreshing recency. A typed `(face_id, effective_weight)`
  identity removes formatting and owned-string copying from those operations.
  The atlas capacity, LRU order and eviction policy remain the same.
- FrameState already coalesces redraw requests; pending PTY output is drained
  before drawing. Compositor redraws must still present after expose/recovery.
  Cursor blinking uses `WaitUntil` with a 500 ms deadline while focused, visible,
  blinking and at the live viewport; otherwise that deadline is disabled. Each
  blink still projects/regenerates visible panes. Skipping those frames would
  change visible behavior; caching requires broader invalidation evidence.

The isolated 100,000-call identity workload measured 4.892 ms before versus
0.053 ms after (median). It only measures identity construction, not total
atlas overhead, and must not be extrapolated into application throughput.
The full frame workloads provide the retention evidence.

## Reproducible methodology

Base: `5e6bf41`, fetched `origin/master`; the requested branch already existed
at that commit with a clean working tree. Baseline production code was left
unchanged while the opt-in measurement harness was added. Its optimized test
executable was preserved before changing the key. Candidate used exactly the
same harness and only the typed-key production change. Binary hashes, compiler,
CPU, font resolution and environment are in
[environment.json](perf/renderer-optimization/environment.json).

Host: Debian Linux x86_64, Intel Core i5-1250P, Intel Iris Xe Vulkan,
Mesa 25.0.7, rustc 1.99.0, Cargo release profile. System monospace resolves to
DejaVu Sans Mono; emoji fallback is Noto Color Emoji. Offscreen RGBA8Unorm
targets, font size 16, scale 1.0. No renderer/startup/throughput tracing during
measurement. A preliminary sandbox run selected llvmpipe; it was excluded and
native GPU access was used for all reported paired samples.

Nine adjacent baseline/candidate pairs alternate AB/BA order. Each process runs
five warmup draws followed by 100 logical frames per workload. Font loading,
parsing/prefill, pipeline creation and initial uploads are outside timing.
Projection is repeated inside each frame. `device.poll(Wait)` once per logical
frame bounds outstanding GPU work. Wall time includes that wait and transparent
composition; CPU stage timers do not. There is no swapchain, compositor, PTY
producer or input driver in this loop. Frequency/governor and CPU affinity were
not pinned. Workload order is fixed, so thermal/cache/order effects remain.

| Workload | Fixture and emitted cells |
| --- | --- |
| sparse | 80×30, short status text on each row; 600 cells |
| dense | 160×50, repeated 36-character Latin/digit alphabet; 7,200 cells |
| unicode | 80×30, Latin, combining, CJK, rocket and ZWJ sequence per row; 180 cells |
| transparent | 80×30, colored blanks/text at opacity 0.5; 330 cells; composition included |
| blink | Dense fixture, cursor data alternates present/absent each frame |
| theme | 80×30, indexed/default colors; default foreground alternates each frame; 390 cells |
| three_panes | Three sequential same-size 80×30 draws, 1,170 cells each, sharing renderer resources |

`three_panes` is a CPU repeated-pane proxy using the same offscreen target and
clear path, not a complete split-window layout or asynchronous multi-PTY test.
Blink/theme alternate every frame to isolate rendering cost, not their natural
event frequency or config/palette command overhead. Unicode fallback and ZWJ
shaping follow existing behavior; this benchmark does not establish additional
grapheme support. No atlas-capacity churn occurs in these warm fixtures.

The test is ignored in normal suites. To reproduce, build with
`cargo test -p terminal-renderer --release --offline --no-run`, preserve the
reported test executable for each revision outside `target`, then run:

```sh
python3 docs/perf/renderer-optimization/compare.py /path/to/baseline /path/to/candidate --runs 9 --output /tmp/renderer-paired
python3 docs/perf/renderer-optimization/summarize.py /tmp/renderer-paired/samples.csv
```

`--runs` selects paired process repetitions; `--output` retains every raw log and
the CSV. The driver invokes only `renderer_workload_baselines` with `--ignored`
and `--nocapture`, one test thread, and checks for the native integrated adapter
used here. Adapt that explicit adapter check for another host; never silently
mix software and hardware results. On baseline revision, copy the candidate's
`gpu_bench.rs` and its four-line `cfg(test)` module declaration into `gpu.rs`
without the production key change. This creates the identical baseline harness.

## Results

Medians of nine process samples, **µs per logical frame**. The three-pane row
includes all three draws. Changes are ratios of medians. Faster-pair counts
compare adjacent process samples; these are not confidence intervals or gates.
Full samples and ranges are retained in
[paired/samples.csv](perf/renderer-optimization/paired/samples.csv).

| Workload | Generation before | After | Change | Faster pairs | Wall before | After |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| sparse | 126.61 | 89.98 | −28.9% | 8/9 | 969.70 | 853.92 |
| dense | 1261.73 | 669.40 | −46.9% | 9/9 | 3359.17 | 2456.21 |
| unicode | 38.44 | 32.34 | −15.9% | 8/9 | 905.01 | 743.32 |
| transparent | 94.93 | 68.06 | −28.3% | 8/9 | 1443.23 | 1284.67 |
| blink | 1253.67 | 723.09 | −42.3% | 9/9 | 3086.57 | 2412.22 |
| theme | 103.84 | 70.96 | −31.7% | 9/9 | 1084.72 | 799.55 |
| three_panes | 841.90 | 611.03 | −27.4% | 9/9 | 2086.55 | 1857.72 |

Dense generation sample ranges were 1,150–1,460 µs before and 598–859 µs after.
Sparse ranges overlap (109–145 before, 63–145 after); small-workload conclusions
are less strong. Projection/upload/submission varied across runs, including
some increases; none of their changes are attributed to this optimization.
Wall-time medians improved 11–27%, but include synchronization/driver scheduling
and are not GPU timestamp or input-to-present measurements.

The existing CPU-only projection bench was also repeated in nine alternating
process pairs (500 operations per sample), using preserved optimized binaries:

| Existing benchmark | Before µs/op | After µs/op |
| --- | ---: | ---: |
| visible_projection_80x30 | 20.420 | 20.616 |
| output_echo_plus_projection | 18.622 | 18.689 |
| three_pane_layout_plus_projection | 23.963 | 24.093 |

These roughly 0.4–1.0% differences do not establish a projection regression or
improvement; projection code is unchanged. [projection.csv](perf/renderer-optimization/projection.csv)
retains operation counts and exact elapsed times. Reproduce this CPU-only fixture
with `cargo bench -p terminal-renderer --bench projection_baselines --offline`;
repeat the resulting executable rather than including compilation in timing.

Across the complete benchmark process, median user CPU time was 0.571→0.400 s;
system CPU was 0.147→0.150 s. Median peak RSS was 90,344→90,732 KiB, with
overlapping ranges (89,244–90,812 and 90,184–90,960 KiB). No resident-memory
improvement is established. The key no longer allocates a face string per glyph
lookup, but allocation counts were not instrumented. Atlas storage remains
4 MiB RGBA; glyph instances remain 52 bytes. No GPU-memory saving is claimed.

All paired warm samples had zero raster calls. Static/blink/three-pane cases
uploaded zero instance bytes. Theme alternation wrote the same 1,996,800 bytes
per 100 frames before and after. Showing a previously uploaded cursor reuses
its unchanged contents; hiding it needs no upload. No GPU execution-time,
GPU utilization/power, event-loop stall percentile, idle CPU, concurrent-pane
fairness or input latency improvement has been measured.

## Recent features and remaining work

Width reflow remains O(retained cells), in terminal-core, and is outside the
retained change. A fresh current optimized `state_baselines` run is retained as
[core-current.txt](perf/renderer-optimization/core-current.txt); it is a current
cost observation, not a before/after renderer result. An earlier run concurrent
with debug workspace tests is marked excluded. Height/no-op resize behavior and
the history-preserving algorithm were left intact. See [SCROLLBACK_REFLOW.md](SCROLLBACK_REFLOW.md).
The current 10k-history width pair took 8.124 ms (about 4.06 ms per width
change), versus 31.5 µs for a height pair and 1.2 ns for an unchanged-size call.
These single-run core costs must not be attributed to the renderer key change.

The RGBA emoji atlas uses four times the former R8 storage and expands mask
bytes on cold insertion. These costs are unchanged; warm Unicode work benefits
less because projection/submission and combining-text allocations form a larger
share. Cold color decoding and eviction-heavy frames need separate measurement.
Theme switches still reproject colors and upload changed glyph/rectangle data;
they reuse fonts/atlas when font settings are unchanged. Blink keeps its existing
500 ms sleep/wakeup behavior; reducing per-frame CPU work does not prove lower
idle CPU or more responsive input.

## Retained and rejected choices

Retained: typed face/effective-weight atlas identity, a focused deterministic
hash/equality regression across two faces and three weight states, and opt-in
measurement tools. Existing native GPU tests verify weight/style separation,
emoji, color/alpha and warm-cache behavior.

No additional production experiment was necessary or retained. Considered but
deferred: row/damage projection caches, skipping compositor/cursor presents,
instance reuse across frames/panes, replacing LRU policy, and changing upload
strategy. Their invalidation, ordering, memory and fairness risks exceed the
evidence from these workloads. Remaining linear viewport and cache scans are
visible debt, not claimed optimizations. No regressing experimental production
code remains.

Previously rejected work stays rejected: row-buffer rotation regressed visible
ANSI throughput ([SHORT_LINE_THROUGHPUT.md](SHORT_LINE_THROUGHPUT.md)); deferred
glyph resources worsened first text ([DEFER_GLYPH_RESOURCES.md](DEFER_GLYPH_RESOURCES.md));
fallback sorting lacked total CPU evidence ([FONT_DISCOVERY_STARTUP.md](FONT_DISCOVERY_STARTUP.md));
adapter filtering lacked safe retry evidence ([WGPU_ADAPTER_REQUEST.md](WGPU_ADAPTER_REQUEST.md)).
None was reintroduced.

## Validation and manual acceptance

Final automated results and native trace are retained under
[perf/renderer-optimization](perf/renderer-optimization/). Native GPU readback
checks cover unorm/sRGB targets, opacity 1.0/0.5, emoji at 1×/1.25×/2×,
variable weights, underline/cursor, palette and URL-hint alpha. These are
correctness checks, not paired performance samples.

Automated validation details are recorded in `validation.txt`. Full manual
Linux/Windows acceptance remains outstanding. Run optimized binaries with
diagnostics disabled for subjective checks; use the same font, size, DPI,
window geometry, theme, opacity, power state and byte workloads on each revision.
For traces, enable renderer diagnostics separately and compare frame counts,
projection/generation time, writes/bytes, resize configuration and presents.

| Manual check | Linux KDE Wayland | Windows native DX12 |
| --- | --- | --- |
| Type/backspace, shell redraw, selection/drag, search and URL hints; sharp text and immediate feedback | Pending | Pending |
| Vim/Neovim/TUI alternate screen, mouse reporting and input isolation | Pending | Pending |
| Latin/CJK/combining/emoji, regular/bold/italic, colored blanks and cursor shapes | Pending | Pending |
| Opaque first, then opacity 0/0.5/0.85 and blur where supported; compare edge alpha/colors | Pending | Pending |
| Continuous shrink/grow with 0/2k/10k history; no stale scaling, trails, selection loss or reflow corruption | Pending | Pending |
| Move between 1×/1.25×/2× displays, change font size, minimize/restore, expose and recreate surface | Pending | Pending |
| Switch dark/light/custom themes during output and while idle; no stale colors or atlas loss | Pending | Pending |
| Watch blinking block/beam/underline while idle, unfocused and in history; no unintended wakes or trails | Pending | Pending |
| Three panes: simultaneous output, input in focused pane, pane switching and resize; no starvation | Pending | Pending |
| Compare identical sustained plain/ANSI/Unicode streams; record CPU/RSS/frame percentiles and byte counts | Pending | Pending |

The automated KDE lifecycle probe exercises font reload/size/reset and renderer
recreation on a real Wayland window. It does not replace the manual interaction,
multi-monitor or resize-pacing checklist. Windows was not executed in this pass.

Final checks passed: workspace all-target tests (795 passed, 26 opt-in tests
ignored), four doc tests, seven explicitly executed Intel GPU tests, one KDE
Wayland lifecycle test, formatting, workspace all-target check, Clippy with
warnings denied, release application build and whitespace validation. The new
identity regression verifies distinct faces, absent/present effective weights
and hash/equality reuse. Ignored tests requiring unrelated platforms/tools were
not all executed.
