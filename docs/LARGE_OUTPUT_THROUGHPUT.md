# Large-output throughput

Measured on Windows x86_64, 2026-10-01, on `perf/large-output-throughput`, based on HEAD `3a0abef`. No commit or push. Windows 11 Pro 10.0.26300, Intel i5-1250P, Iris Xe driver 32.0.101.7088, rustc 1.98.1 (48a229cea, 2026-09-01). Default renderer/backend selection, scale 1, 800×600 window, 80×31 terminal, default 10,000-row history. These are local release measurements, not hardware-independent budgets. Windows ARM64 is outside this task.

## Audit and retained work

The previous work was partial instrumentation: optional PTY read/pending/peak counters and renderer projection/render/present counters existed, but the app did not consume them. There were no project-owned throughput workloads, recorded baseline runs, optimization, focused throughput tests, or recorded validation results. The task notes still described pipeline inspection. Previous counters were retained and connected to one aggregate exit report. Failed-send and shutdown accounting were completed and tested. No old changes were discarded.

The original runtime used 1,024-byte maximum PTY reads and an eight-event bounded queue; the app drained every pane until empty on a wake and again before rendering, parsing each chunk separately. Previous instrumentation had not changed queue or batching semantics. Existing per-event tracing remains a separately enabled diagnostic; aggregate diagnostics do not enable it.

## Reproduction and definitions

```powershell
cargo build --release -p terminal-app --bin terminal --example throughput-workload
./scripts/measure-throughput.ps1 -Label fair -Runs 3
./scripts/summarize-throughput.ps1 -Label fair
```

The collector runs all six workloads three times, parser then visible per workload, and saves logs plus a median/range summary under ignored `target/perf/throughput/<label>/`. Use a new label for a new binary; do not mix revisions in one directory. Disable other diagnostic and visual-smoke environment switches, and use the default renderer/font/window settings for comparison. The baseline directory was collected with the original unbounded drain and the completed measurement harness; `fair` uses the final bounded continuation. Builds precede measurements. No warm-up exclusion was applied. Baseline and final sets were sequential, not randomized/interleaved across binary revisions; thermal/scheduling variation limits small percentage claims.

`TERMINAL_THROUGHPUT_DIAGNOSTICS=1` enables lifetime counters for normal interactive use, reported once at app exit. Unset means no aggregate logs or PTY counter atomics. `terminal --throughput-run <name>` additionally selects a reproducible one-pane harness, bypassing user workspace/configuration and terminating after EOF, child exit, and the final data frame. It requires the release console helper next to the app under `examples/`. The harness has a 60-second failure deadline and reports `complete=false` on incomplete runs.

Workloads: finite repeats 64-byte printable lines for 8 MiB; short emits `x` plus CRLF for approximately 1 MiB; long repeats 8,192 characters plus CRLF for approximately 8 MiB; ANSI repeats color/reset/truecolor sequences for approximately 8 MiB; continuous writes a sustained, volume-bounded 32 MiB without sleeps; input does the same while an independent child stdin reader acknowledges `probe`. Sizes are rounded down to complete blocks. Input is injected once after 512 KiB has been parsed, through a queued app user event and the normal PTY-directed input writer. It measures event dispatch and subsequent ACK receipt, not physical keyboard-to-screen latency. Child stdout lock contention, ConPTY processing and transport are included in ACK latency.

Visible bytes are actual PTY bytes, not child payload bytes. Windows ConPTY emits additional cursor/wrap controls and normalizes ANSI output, so long output grows and ANSI output shrinks. ANSI stream size and read boundaries can vary by scheduling. Parser-only runs process the original payload in 1,024-byte chunks through the same core parser/state, without PTY, app or renderer; they are diagnostic comparisons, not identical-stream speedups. Parser-only input omits the stdin/ACK exchange. Visible elapsed time covers spawn through final present/exit report; parser elapsed covers only feed/apply. MB/s uses decimal bytes/second. Parse/apply time includes app-owned reply/startup bookkeeping, not just parser instructions.

Redraw counts are actual coalesced window requests. Rendered/presented counts are successful renderer frames/present calls; terminal frames are successful presents after visible PTY data changed. Render timing covers CPU generation/upload/submission/surface acquisition/present calls, not GPU completion or display latency; projection is separate and substage timings must not be added to their enclosing total. Frame intervals are between successful present calls. Sparse baseline frames make p95 misleading, so maximum intervals are also reported.

PTY pending metrics include queued output, a producer's in-flight send and a received chunk before its accounting decrement. Thus the eight-slot queue can report a transient peak of ten chunks. Independent relaxed atomic fields are not a transactional queue snapshot. All completed measurements ended with zero pending chunks/bytes. App totals cover all panes; PTY report entries cover workers still owned at exit, and renderer projection covers the app's `redraw_panes` path. These are not aggregate lifetime queue peaks across retired/multiple workers. Frame samples are bounded to 100,000 and recorded only in the workload harness.

## Visible release results

All cells below are median [minimum, maximum] across three complete runs. Elapsed is ms; throughput is MB/s. Raw bytes are stable except ANSI, whose median/range is shown.

| Workload | PTY bytes before → after | Elapsed before | Elapsed after | MB/s before | MB/s after |
|---|---:|---:|---:|---:|---:|
| finite | 8,388,749 → same | 1625.855 [1611.737,1637.374] | 1715.808 [1556.704,2037.644] | 5.160 [5.123,5.205] | 4.889 [4.117,5.389] |
| short | 1,047,693 → same | 1614.283 [1596.668,1617.318] | 1913.753 [1806.747,2080.147] | 0.649 [0.648,0.656] | 0.547 [0.504,0.580] |
| long | 9,530,079 → same | 1520.700 [1458.033,1557.353] | 1922.135 [1872.977,2133.978] | 6.267 [6.119,6.536] | 4.958 [4.466,5.088] |
| ANSI | 7,967,475 [7,967,469,7,967,490] → 7,967,427 [7,967,424,7,967,496] | 2579.062 [2086.860,2721.166] | 2257.439 [2060.396,2370.194] | 3.089 [2.928,3.818] | 3.529 [3.362,3.867] |
| continuous | 33,554,573 → same | 5037.429 [5003.233,5593.799] | 5770.133 [5724.885,5877.114] | 6.661 [5.999,6.707] | 5.815 [5.709,5.861] |
| input | 33,554,596 → same | 5009.008 [4979.183,5218.617] | 5780.191 [5779.625,5871.499] | 6.699 [6.430,6.739] | 5.805 [5.715,5.806] |

The change deliberately restores regular rendering while output continues. Median throughput decreases approximately 5% finite, 16% short, 21% long, and 13% continuous/input. ANSI increases about 14%, but variation and ConPTY stream differences prevent assigning that improvement solely to scheduling. This is a measured fairness improvement with a throughput tradeoff, not a general byte-throughput acceleration.

| Workload | Parser batches before → after | Parse/apply ms before → after | Redraw/render/present before → after | Terminal-data frames before → after |
|---|---:|---:|---:|---:|
| finite | 8209 [8204,8211] → 8210 [8206,8223] | 1512.754 [1496.117,1535.196] → 1382.011 [1247.071,1606.961] | 6 [5,6] → 177 [163,204] | 5 [4,5] → 176 [162,203] |
| short | 1029 [1028,1029] → 1028 [1028,1028] | 1562.944 [1548.247,1569.673] → 1737.631 [1606.172,1861.492] | 4 [4,5] → 173 [166,185] | 3 [3,4] → 172 [165,184] |
| long | 9453 [9433,9456] → 9483 [9457,9586] | 1438.196 [1357.866,1480.825] → 1427.780 [1386.359,1525.870] | 6 [5,10] → 185 [178,204] | 5 [4,9] → 184 [177,203] |
| ANSI | 8250 [8244,8255] → 8231 [8230,8253] | 1568.637 [1274.128,1615.012] → 1343.252 [1308.629,1491.510] | 284 [233,311] → 262 [234,279] | 283 [232,310] → 261 [233,278] |
| continuous | 32835 [32822,32852] → 32827 [32821,32839] | 4857.439 [4837.243,5374.479] → 4892.597 [4863.151,4945.503] | 8 [5,33] → 616 [614,625] | 7 [4,32] → 615 [613,624] |
| input | 32827 [32822,32900] → 32821 [32821,32834] | 4812.069 [4808.479,4899.418] → 4908.661 [4907.375,4914.110] | 9 [7,88] → 619 [619,626] | 8 [6,87] → 618 [618,625] |

Redraw, rendered and presented counts happened to match in each recorded run. Read events equal parser batches for these completed one-pane runs. Mean bytes per batch are approximately 1,019–1,022 for plain workloads and 965–968 for ANSI. Read size, queue capacity and one-feed-per-read behavior remain unchanged; no chunks were combined or dropped. Peak accounted chunks were ten in every run. Peak bytes were 10,240 except ANSI: baseline 9,764 [9,764,9,984], final 9,764 [9,757,10,240]. All final scrollback lengths were 10,000.

| Workload | Largest drain ms before → after | Largest frame interval ms before → after | Final frame p95 ms |
|---|---:|---:|---:|
| finite | 1522.782 [857.103,1563.340] → 4.400 [4.394,4.528] | 1528.417 [863.998,1605.220] → 40.108 [38.228,40.361] | 10.344 [10.136,11.457] |
| short | 1568.479 [1553.861,1574.876] → 6.420 [6.221,9.028] | 1577.676 [1562.501,1582.708] → 28.708 [22.110,47.377] | 12.024 [11.799,12.951] |
| long | 1461.292 [998.767,1513.466] → 4.392 [4.289,4.621] | 1468.214 [1000.252,1520.701] → 25.360 [18.157,27.017] | 12.627 [10.777,12.936] |
| ANSI | 47.507 [45.237,58.285] → 4.306 [4.261,4.391] | 52.989 [48.242,68.135] → 29.451 [26.465,49.636] | 12.671 [12.609,13.373] |
| continuous | 3046.738 [2750.195,4922.463] → 4.665 [4.393,4.731] | 3085.575 [2754.533,4956.018] → 37.592 [37.221,40.740] | 10.068 [9.908,10.330] |
| input | 2267.907 [2262.347,4760.982] → 4.640 [4.487,4.698] | 2309.734 [2297.623,4793.278] → 37.077 [30.798,42.069] | 9.912 [9.854,10.703] |

Input dispatch: **2186.157 [2178.453,4674.496] ms → 0.542 [0.068,1.764] ms**. Dispatch-to-ACK: 185.431 [133.258,273.973] → 229.285 [154.344,712.926] ms. ACK did not improve reliably; child/transport latency is separate from event-loop dispatch. No physical keyboard or interactive resize latency claim is made. Native input/present progression, deterministic drain-limit tests, and chunked parser/state equivalence cover the scheduling change. Existing input and resize regressions also run in the workspace suite; a new real-event-loop resize-under-load regression was not added. These measurements do not establish human-visible frame correctness on every platform.

| Workload | Projection ms before → after | CPU render/present ms before → after |
|---|---:|---:|
| finite | 0.384 [0.298,0.464] → 9.820 [7.303,11.199] | 48.320 [44.258,49.157] → 238.920 [197.985,291.424] |
| short | 0.141 [0.134,0.364] → 4.935 [4.516,5.827] | 6.767 [6.126,9.925] → 134.248 [100.942,136.061] |
| long | 0.641 [0.354,0.650] → 11.719 [10.903,14.164] | 11.123 [10.399,15.632] → 364.135 [354.178,463.811] |
| ANSI | 10.114 [6.718,11.847] → 7.666 [7.079,9.141] | 854.356 [708.773,974.405] → 647.631 [600.160,932.681] |
| continuous | 0.424 [0.283,1.438] → 25.958 [25.671,32.821] | 44.037 [37.730,71.901] → 612.847 [600.392,661.745] |
| input | 0.449 [0.336,3.867] → 30.713 [27.065,32.272] | 38.889 [36.606,132.108] → 618.337 [598.779,692.320] |

## Parser/state diagnostic baseline

Three runs per set, ms and MB/s, all ending at 10,000 history rows. Core implementation was unchanged between sets. This variation itself argues against attributing small deltas to app changes.

| Workload | Payload bytes / batches | Baseline elapsed / MB/s | Final elapsed / MB/s |
|---|---:|---:|---:|
| finite | 8,388,608 / 8192 | 1351.799 [1291.005,1970.361] / 6.206 [4.257,6.498] | 1298.550 [1237.376,1757.548] / 6.460 [4.773,6.779] |
| short | 1,047,552 / 1023 | 2071.921 [2061.170,2086.652] / 0.506 [0.502,0.508] | 1994.957 [1587.454,2138.128] / 0.525 [0.490,0.660] |
| long | 8,382,462 / 9207 | 1280.696 [1193.855,1342.533] / 6.545 [6.244,7.021] | 1303.366 [1066.444,1330.577] / 6.431 [6.300,7.860] |
| ANSI | 8,386,560 / 8190 | 1554.194 [1354.715,1605.321] / 5.396 [5.224,6.191] | 1149.390 [1113.382,1375.317] / 7.297 [6.098,7.533] |
| continuous | 33,554,432 / 32768 | 9841.346 [7442.217,9924.407] / 3.410 [3.381,4.509] | 5853.487 [5598.024,6133.794] / 5.732 [5.470,5.994] |
| input | 33,554,432 / 32768 | 6852.068 [6282.538,7136.190] / 4.897 [4.702,5.341] | 5621.377 [5480.304,5754.695] / 5.969 [5.831,6.123] |

## Bottleneck, change and remaining opportunities

The material app-owned bottleneck was an unbounded drain monopolizing the Windows event loop. Plain-output parse/apply accounted for most elapsed time while rendering occurred only a handful of times; input waited seconds. Each pane now yields after 64 events or four elapsed milliseconds, whichever comes first, while always allowing one event. A feed cannot be interrupted, so short-line chunks exceed four milliseconds (observed maximum 9.028 ms). Bounds are per pane, not a global multi-pane deadline. Multi-pane scalability is a future measurement, not a proven latency bound.

Continuation runs from `about_to_wait` with `ControlFlow::Poll` only while work is deferred. The pending-wake flag suppresses repeated proxy notifications until the next bounded drain. Existing pre-redraw drains remain bounded as well. This gives input/window/redraw events a turn, preserves incremental parser state, FIFO replies and transport backpressure, and returns to normal Wait/WaitUntil when the queues are exhausted. No sleeps, dependencies, production threads, queue enlargement or parser/terminal semantic changes were added. Ownership stays in the app, so no architectural decision changed.

An intermediate three-run experiment requeued a proxy wake after each bounded drain. It reduced input dispatch to milliseconds but still allowed Windows proxy-event delivery to run ahead of redraws, retaining second-long frame gaps. That continuation was replaced, not shipped. Earlier incomplete pilot runs (including erroneous helper launch/EOF completion delay) were excluded; only `complete=true` measurements from the corrected harness appear here.

History remains capped at 10,000 for both 8 MiB and 32 MiB streams. Many short lines are much slower per byte than long lines even in parser-only measurements, consistent with row/scroll work dominating that workload. These results establish bounded retained history, not heap usage or isolated scrollback-copy causation. No scrollback optimization was attempted. Next investigations could profile line-feed/history eviction, compare multiple grid widths/history capacities and many panes, and evaluate frame-rate-aware scheduling to recover throughput while retaining input/event fairness. Renderer CPU work is now material in some workloads because regular frames are actually produced; physical keyboard/resize-to-present and GPU completion measurements remain useful follow-ups.

## Validation

All final gates passed on native Windows x86_64: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets` (646 passed, zero failed, four existing ignored), `cargo test --workspace --doc` (four passed), `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`. Logs are under ignored `target/perf/throughput/validation/`. Git emitted only line-ending conversion warnings.

The final release binary also passed a normal-workspace, diagnostics-disabled output smoke: the child completed its output burst, the app closed with exit code zero, and captured stdout/stderr were both empty. Baseline and final release measurements each contain three completed runs of all six visible and parser-only workloads. Windows ARM64 and other platforms were not measured. No commit or push was made.
