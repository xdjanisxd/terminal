# Short-line throughput investigation

Branch: `perf/short-line-output`. Windows x86_64 release measurements, 2026-10-01. Complete and validated, including user-reported manual native acceptance. No commit or push. The production scrolling algorithm and the per-pane 64-event / 4-ms fairness policy are unchanged. Aggregate diagnostics and deterministic diagnostic tests are retained.

## Finding and decision

Short lines are dominated by terminal state scrolling, specifically `Grid::scroll_region_up` copying the remaining flat grid once per line. In the three-run instrumented baseline, parser/state elapsed time was 1496.489 ms; grid scrolling was 1214.616 ms (about 81%), history insertion 186.269 ms, and inclusive LF handling 1448.928 ms. Renderer projection was only a few milliseconds. These are elapsed scope measurements, not sampled CPU profiles; nested timers must not be summed.

A candidate reused grid row buffers and rotated row indices instead of copying cells. It improved parser-only short output from 0.700 to 2.401 MB/s in controlled instrumented comparisons. However, ANSI-heavy visible throughput fell from 4.740 to 1.257 MB/s and its presented-frame median rose to 757. The candidate also increased short-output presentation time substantially. It was rejected and fully reverted, including its grid-specific tests. No production optimization is claimed. Faster state application changed drain/redraw interaction enough that a parser improvement alone did not satisfy the acceptance criteria.

## Unchanged workload

`crates/app/src/throughput/workloads.rs` builds `b"x\r\n".repeat(1024)` and emits that block 341 times: **349,184 lines, 1 visible character per line, CRLF, 1,047,552 payload bytes**, no ANSI. At the harness's 80 columns and 31 rows there is no wrapping. There are 349,154 full-screen upward scrolls. Normal history retains the newest 10,000 rows and trims 339,154 rows; final line order and the blank final cursor row follow normal CR/LF semantics.

This is a deterministic extreme of short records and dense scrolling, representative of one-character status/log output rather than a general average line length. The byte stream and expected operations are deterministic; ConPTY read boundaries, scheduling and elapsed time are not. The benchmark was not shortened, batched differently, or special-cased.

The PTY reads at most 1024 bytes per event. Visible runs generally process about 1028 batches, each containing hundreds of logical lines, potentially split across reads. The parser-only baseline uses 1023-byte chunks. Rendering sees terminal state at bounded drain/redraw boundaries, not every intermediate line state. Semantics are applied to every byte in order. ConPTY/startup adds 141 bytes to short visible runs (1,047,693 total). Long and ANSI visible streams also differ from their parser-only payloads because ConPTY transforms output; comparisons are diagnostic, not identical-stream transport speed claims.

## Diagnostics and scope

`TERMINAL_THROUGHPUT_DIAGNOSTICS=1` enables existing app/PTY/renderer aggregates plus core print, CR, LF, transition, scroll, history push/trim, row allocation, cell copy/clear and elapsed-scope totals. Normal release runs produce no diagnostic output. No per-line logging or new allocator interception was added.

History comparisons additionally set `TERMINAL_THROUGHPUT_HISTORY` and optionally `TERMINAL_THROUGHPUT_PREFILL`, only in the opt-in harness. Capacity must be at most the normal 10,000. Prefill precedes parser-only timing and core aggregate reset; the visible harness's existing end-to-end timer includes prefill and process startup. Normal construction always uses 10,000. These controls are not user configuration or a throughput optimization.

Core totals cover the active screen, ordinary upward scroll handling and LF calls, not every possible editing operation. `transition_moves` counts CR setter calls plus LF calls that actually change cursor position. `prints` counts character dispatches, including any rejected/nonspacing dispatch in other workloads. For this ASCII workload it equals lines. History row allocations are inferred from the existing `row.to_vec()` call, not measured heap events. Renderer buffer allocations are already directly counted. There is no core dirty-row/cell tracking to count: renderer projection scans the viewport; cell writes/clears and redraw/data-frame totals describe the relevant work instead. Parser dispatch itself is not separately timed from state application.

Exact short-workload totals (parser and visible):

| Operation | Count |
| --- | ---: |
| Prints / LF / CR, each | 349,184 |
| Cursor transition operations | 349,214 |
| Scrolls / rows scrolled / history pushes / row allocations, each | 349,154 |
| Normal-history trims | 339,154 |
| History cells copied / grid cells cleared, each | 27,932,320 |
| Grid cells moved | 837,969,600 |

PTY reads equal parser batches in these runs. Final visible median is 1028 [1027, 1029], parser-only 1024; queue peak is 10 chunks / 10,240 bytes. Queue peak can include in-flight accounting beyond the eight-event channel capacity. Renderer short runs allocate two buffers, 2672 aggregate bytes. There is no grid-row allocation per scroll; history allocates an owned row per push. Whole-process allocation counts remain unmeasured.

## Release results

Each cell is the median [minimum, maximum] of three completed runs. MB/s means decimal bytes per second divided by 1,000,000. No validation process ran concurrently with the controlled or final measurements. Exploratory candidate runs that overlapped validation were excluded. Runs were sequential, not randomized; timer overhead, thermal state and scheduling variation prevent attributing every difference to instrumentation.

Untouched baseline versus final retained diagnostics, with the same production algorithm:

| Workload, visible MB/s | Untouched baseline | Final diagnostics |
| --- | --- | --- |
| Short | 0.560 [0.531, 0.615] | 0.560 [0.394, 0.589] |
| Large finite | 4.950 [4.841, 5.322] | 4.497 [4.370, 4.543] |
| Long | 5.382 [4.272, 5.478] | 5.024 [4.528, 5.468] |
| ANSI | 4.740 [3.482, 5.366] | 4.665 [3.989, 5.153] |
| Continuous | 6.088 [6.036, 6.135] | 5.810 [5.774, 5.848] |
| Output with input | 6.088 [6.083, 6.101] | 5.690 [5.256, 5.845] |
| Short parser/state-only | 0.598 [0.529, 0.661] | 0.510 [0.502, 0.580] |

The final table measures the diagnostic build under diagnostics. It demonstrates no retained throughput improvement and exposes diagnostic overhead/variation; it is not a claim that normal release production regressed by those percentages. Final short renderer projection was 4.627 [4.326, 6.176] ms, instance generation 3.948 [3.600, 4.350] ms, and total render/present scope 104.553 [93.534, 180.789] ms.

Short elapsed scopes, milliseconds:

| Scope | Pre-candidate instrumented baseline | Rejected candidate, controlled | Final original algorithm |
| --- | --- | --- | --- |
| Parser-only total | 1496.489 [1443.938, 1740.702] | 436.351 [356.940, 444.321] | 2054.785 [1807.066, 2086.424] |
| Parser-only inclusive LF | 1448.928 [1398.046, 1684.934] | 379.211 [310.821, 387.545] | 1988.240 [1748.168, 2019.233] |
| Parser-only history | 186.269 [184.771, 226.697] | 209.276 [171.026, 209.416] | 236.775 [227.409, 262.820] |
| Parser-only grid scroll | 1214.616 [1162.937, 1403.163] | 114.821 [94.108, 122.388] | 1658.479 [1458.591, 1717.769] |
| Visible parse/state | 1423.792 [1419.225, 1759.318] | 475.779 [467.516, 487.075] | 1701.298 [1628.212, 2400.055] |
| Visible inclusive LF | 1378.901 [1370.687, 1700.030] | 417.972 [407.047, 423.278] | 1646.452 [1576.111, 2344.028] |
| Visible history | 195.409 [184.243, 222.072] | 235.897 [235.812, 244.549] | 203.761 [203.520, 215.352] |
| Visible grid scroll | 1147.878 [1128.004, 1420.441] | 116.974 [112.709, 123.323] | 1387.072 [1319.981, 2071.226] |

The rejected candidate's visible short rate was 0.680 [0.659, 0.718], parser-only 2.401 [2.358, 2.935]. Its other visible rates were finite 7.794 [6.274, 7.878], continuous 8.362 [8.248, 8.489], input 8.199 [8.070, 8.346], and ANSI 1.257 [1.251, 1.260]. Better plain throughput cannot compensate for the ANSI regression.

## Frames, fairness and input

| Short visible metric | Untouched baseline | Final original algorithm |
| --- | --- | --- |
| Redraw / render / present counts, each | 171 [154, 179] | 175 [171, 229] |
| Terminal-data frames | 170 [153, 178] | 174 [170, 228] |
| Largest frame gap, ms | 28.059 [25.420, 29.011] | 26.206 [23.530, 27.108] |

Final long output presents 184 [170, 201] frames for roughly 9.53 MB of observed PTY bytes, versus short's 175 for roughly 1.05 MB. Thus short has about nine times as many presents per PTY byte, although a similar number of frames per run. Its slower state application consumes the 4-ms drain budget sooner; the 64-event budget does not ensure equal work per event. A single parser batch is allowed to finish, so the final median maximum drain is 6.291 ms rather than a hard 4-ms cutoff.

The candidate short median maximum frame gap was 37.851 [35.421, 45.728] ms, and render/present elapsed total was 911.484 ms versus untouched baseline 117.387 ms. ANSI presents reached 757 [751, 757], consuming roughly 5.2 seconds of its roughly 6.34-second run. In the app, output invalidates the frame and requests redraw at drain boundaries; faster processing can empty the queue more often and change redraw/present cadence. These observations support a scheduling/presentation interaction, but do not isolate the internal DXGI/driver or ConPTY cause. No fairness bounds were removed or extended.

The existing output-with-input harness dispatches an event-loop probe and measures its PTY acknowledgement. Untouched dispatch was 0.960 [0.696, 3.192] ms; final dispatch 6.988 [6.428, 7.396] ms. Acknowledgement was 45.225 [18.486, 133.292] versus 25.036 [3.413, 25.827] ms. These are automated probe measurements, not physical keyboard latency during short output. Candidate dispatch was 1.527 ms median, acknowledgement 137.691 ms median. Final short frame gaps remain bounded in the observed runs; no unbounded drain was restored.

Automated native interactive acceptance was attempted with the final Windows x86_64 release binary and PowerShell 7.6.6 emitting the same CRLF blocks indefinitely. The process reached a running PowerShell pane, but its window was absent from the computer-use tool's targetable windows. The tool's direct launch returned `product policy blocks this app`; the test processes were stopped. The user subsequently reported manual native acceptance passed under sustained short-line output: keyboard input remained responsive, Ctrl+C interrupted promptly, window resize and tab/pane switching remained responsive, and frame pacing was acceptable with no long visible freezes. This is qualitative manual acceptance supplied by the user, separate from the measured automated probe latencies and frame gaps above.

## History scaling and per-line work

Three runs per setting; prefill is excluded from parser/core timing but included in visible end-to-end time:

| History capacity / initial rows | Visible MB/s | Parser-only MB/s | Parser history ms | Parser grid scroll ms |
| --- | --- | --- | --- | --- |
| 32 / 0 | 0.636 [0.570, 0.649] | 0.702 [0.689, 0.732] | 102.941 [98.840, 105.093] | 1291.372 [1237.799, 1316.055] |
| 10,000 / 0 | 0.560 [0.394, 0.589] | 0.510 [0.502, 0.580] | 236.775 [227.409, 262.820] | 1658.479 [1458.591, 1717.769] |
| 10,000 / 10,000 | 0.520 [0.503, 0.557] | 0.588 [0.547, 0.650] | 204.336 [194.765, 212.492] | 1452.047 [1310.480, 1588.936] |

Small history is faster and has lower measured insertion cost. Normal empty history reaches its cap after only about 30 KB, so almost the entire workload already runs at the cap. Starting full is not consistently worse than starting empty. There is no evidence here for unbounded degradation as history grows, nor a clean causal estimate of history-size impact: even unchanged grid scope time varies substantially between sequential settings. Small/full trim counts are 349,122 / 349,154; push/allocation counts remain 349,154.

Release history is a `VecDeque<Vec<Cell>>`: eviction is O(1), row capture copies/allocates 80 cells, not all history. Debug-only history invariant scans are O(history) but absent in release measurements. Grid movement is O(viewport rows × columns) per short line. Newly exposed row clearing is O(columns), rendition copies happen as cells are moved, and cursor/wrap operations are bounded. No per-line selection/search full-history work was found; app selection clearing is per parser chunk. Hyperlink reclamation can scan retained cells at its URI cap, but this no-ANSI workload never triggers it. Renderer projection scans visible rows per frame and was not dominant in the original algorithm. No speculative concurrency or scrollback redesign was introduced.

## Reproduction, validation and follow-up

Build `cargo build --release -p terminal-app --bin terminal --example throughput-workload`. Run `scripts/measure-throughput.ps1 -Runs 3 -Label <label>` and `scripts/summarize-throughput.ps1 -Label <label>`. For history comparisons pass `-Workloads short` and the diagnostic history environment variables; reset them afterward. Three-run raw logs are ignored artifacts under `target/perf/throughput/{short-before,short-profile,short-after-controlled,short-final,short-final-small,short-final-full}`. Only `short-after-controlled` is the isolated candidate series; other exploratory candidate logs are excluded.

The retained tests compare observed/unobserved scrolling after each operation, exact grid/history order and aggregate movement counts; diagnostic zero/small history preserves eviction order. Existing tests cover CR/LF, regions, alternate screens, ANSI, cursor/wrap, wide/combining cells and ordered replies. No wall-clock assertions were added. Final `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo test --workspace --doc`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check` all passed. There were 648 passing tests, four existing ignored tests, and four passing doctests. Logs are under `target/perf/throughput/validation-final`. No dependency, threading, PTY interface or renderer ownership change was required; core exposes a narrow diagnostic snapshot/configuration boundary to the existing app harness.

Follow-up opportunities: profile why faster state application increases redraw/present cost before retrying stable row buffers; then measure row recycling for history allocations with controlled randomized history runs. Keep the bounded drain policy and every terminal semantic operation. The measured pathological grid copy is real, but its replacement is not safe to ship on the evidence gathered here.

### Native acceptance reproduction checklist

The prepared local configuration is `target/perf/throughput/native-short.toml` (an ignored diagnostic artifact). On the interactive Windows desktop, from the repository directory, run:

```powershell
.\target\release\terminal.exe --workspace .\target\perf\throughput\native-short.toml
```

It launches installed PowerShell with `-NoProfile -NoExit -Command` and this sustained loop:

```powershell
$block = "x`r`n" * 1024; while ($true) { [Console]::Write($block) }
```

For another machine, use its installed `pwsh` path in the configuration.

1. While output is running, resize the window several times. Verify the window and terminal dimensions progress and rendering continues.
2. Press Ctrl+Shift+E to split, Ctrl+Shift+Right/Left to switch panes, Ctrl+Shift+T to create a tab, and Ctrl+Tab to switch tabs. Verify focus/title/layout changes progress during output. New local-shell panes inherit the same loop.
3. Press Ctrl+C in each output pane. Verify output stops promptly and the PowerShell prompt returns. Record any noticeable delay or stalled frame interval; do not infer physical-key latency from the automated probe.
4. At the prompt, type `Write-Output 'input-ok'` and Enter. Verify characters appear and the command runs promptly. Repeat the output loop and interrupt once more to check sustained behavior.
5. Close only the test tabs/window. Report pass/fail for keyboard, interruption, resize, tabs, panes and visible pacing, including any delay. A failed check prevents completion; do not relax fairness to recover throughput.

Result: manual native acceptance passed, as reported by the user on 2026-10-01. Keyboard, Ctrl+C, resize, tab/pane switching and visible pacing all passed under sustained short-line output. The final diff review confirmed the row-buffer candidate remains reverted and only validated diagnostics/reporting changes remain; no production optimization is retained.
