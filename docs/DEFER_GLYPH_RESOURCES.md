# Deferred glyph resource experiment

Branch: `perf/defer-glyph-resources`. Windows x86_64 release measurements, 2026-10-06. **Complete; experiment reverted. No commit or push.**

The approximately 8 ms candidate did move out of the empty first frame. In ten alternating launch pairs, fonts-started to first-present medians were **127.305 ms eager / 119.174 ms deferred**, an 8.131 ms improvement. However, first-shell-text presentation medians were **225.685 / 230.182 ms**, and the first text frame grew from **7.695 / 16.270 ms**. An earlier candidate launch took **235.819 ms** for that text frame, including **227.570 ms** in glyph pipeline creation. The optimization shifted synchronous work into the first useful frame rather than improving its arrival. It was rejected under the first-text latency gate.

All experimental Rust, tests and measurement-script changes were restored to the starting revision, `c5ebc229d1239cb37c350c2ddb18df22e2847d4e`. Only this report, the completed plan, raw measurement evidence, performance index and session handoff remain. The final renderer still creates glyph resources eagerly; there is no retained startup optimization or backend change.

## Exact resource boundary

The experimental defer moved these resources from `DrawResources::new` into an optional private `GlyphResources`, created synchronously at the first shaped glyph's atlas lookup:

- Glyph bind-group layout: fragment-stage sampled texture and filtering sampler bindings.
- Glyph render pipeline and its pipeline layout, using the existing `glyph_vertex` / `glyph_fragment` shader entry points, vertex layout, blend state and surface format.
- Glyph atlas: `R8Unorm` texture, texture view, linear sampler, bind group, and CPU atlas entry/LRU/slot bookkeeping.

The initial empty terminal has a cursor but no glyph instances. Its backgrounds, cursor, selection rectangles, underlines and pane/UI borders use rectangle rendering and do not access the atlas or glyph pipeline. Text-bearing UI overlays use the same glyph path as terminal text and would trigger initialization in their own first required frame. Glyph instance buffers already allocate on demand, so they offered no additional initial-frame saving.

The shared shader module and rectangle pipeline were retained eagerly because they serve the cursor-bearing first frame. Fonts and metrics, wgpu instance/adapter/device/queue, surface setup, render encoding and presentation were unchanged. PTY startup remained after the first successful present and window show. Whole-wgpu laziness was not attempted.

The candidate used `Option::get_or_insert_with`; initialization completed before glyph rasterization/upload and encoding in the same draw, without an extra redraw request. Same-format resizes retained the resource object; a surface-format rebuild still discarded the complete draw-resource lifetime. Across all 40 candidate launches, each of the three instrumented glyph-resource stages occurred exactly once, in the first-text phase, and never in the empty first-frame phase. All 40 eager launches recorded them once in the initial phase. This verifies the measured startup case, not every possible runtime invalidation.

Two native GPU tests passed for the candidate, covering pipeline validation/identity reuse and an empty cursor draw followed by first-text rendering with backgrounds, glyphs, underlines and cursor. The latter allowed the text draw itself to initialize the atlas and verified a wide-glyph cache entry and output pixels. Renderer library tests also passed (53 passed, four existing ignored). These experimental test changes were reverted with the implementation.

## Measurement method and evidence

There were **80 successful fresh-process launches: 40 eager and 40 deferred**. Four sequential 15-run batches ran eager/deferred/eager/deferred, followed by ten adjacent pairs with alternating order (eager first on odd pairs, deferred first on even pairs). No builds or tests ran during measurements. Caches were left unchanged; these are not controlled cache-cold measurements.

Both saved release executables contained identical temporary diagnostic extensions: startup scopes continued through first shell text; a frame-start milestone preceded redraw after non-whitespace terminal text had been processed; the parser retained milestones after first present and labeled cost rows by phase. Renderer aggregate diagnostics were disabled. Each launch used the same single-pane `cmd.exe /d /k` workspace. The harness checked present/show/PTY ordering, waited for successfully presented shell text, closed the app, and paused 250 ms between launches. All 80 completed without forced cleanup. An initial sandbox-denied harness attempt produced no samples and is excluded.

Timing definitions:

- **Fonts to first present:** `first-frame-presented - fonts-started`, matching the historical investigation.
- **Main to first present / text:** elapsed process milestones from diagnostic initialization near main entry; excludes process creation before main.
- **Show to text:** includes PTY/shell startup, output processing, event-loop scheduling and presentation.
- **Text frame:** immediately before redraw to successful first-shell-text presentation. A retry replaces the frame-start marker; processed-to-text includes any earlier wait/retries.

Scopes measure CPU-side API time, not GPU execution or compositor scanout. The shell-text marker observes terminal text followed by successful presentation, not a screenshot of its physical display. No diagnostics-disabled visual acceptance was performed, and visible hitching is not claimed as directly observed. The long candidate text-frame stall and the lack of earlier useful text were sufficient to reject the defer conservatively; no unverified optimization is retained.

Host: Windows 11 Pro build 26300, Intel Core i5-1250P, Intel Iris Xe driver 32.0.101.7088, Rust 1.98.1, `x86_64-pc-windows-msvc`. `WGPU_BACKEND` was unset, using the existing Windows DX12-first selection. Evidence:

- [All launch samples](measurements/defer-glyph-resources/samples.csv): batch, pair, variant and chronological sequence preserve ordering.
- [All measured scopes](measurements/defer-glyph-resources/costs.csv): includes nested scopes; do not sum inclusive parents with children.
- [Environment and executable hashes by batch](measurements/defer-glyph-resources/environments.json).
- [Measurement workspace](measurements/defer-glyph-resources/workspace.toml).

Eager executable SHA-256: `7856E73FEB24F9C69E158D4CC98428B7748970105B3617D70ADC043EBF282703`. Deferred executable: `AAC4ED1917BB648A7B1565861B9B5204F9384EE3F2CD81E9359D37614AFF30C4`. Each hash remained identical across its variant's batches. Temporary binaries and full launch logs remain in ignored `target/defer-glyph/`; the CSV/environment evidence is retained in documentation. The diagnostic extensions were also reverted, so the current harness retains its prior first-frame-only CSV behavior.

## Results

All values below are milliseconds. Medians include every sample; no outliers were removed. Nearest-rank p95 is the maximum for a ten- or fifteen-sample group, so it is not a stable tail estimate.

The alternating pairs are the primary comparison because sequential batches exhibited substantial drift:

| Metric, 10 launches per variant | Eager median | Deferred median | Deferred minus eager |
| --- | ---: | ---: | ---: |
| Fonts to first present | 127.305 | 119.174 | -8.131 |
| Main to first present | 147.774 | 138.139 | -9.635 |
| Main to first shell text present | 225.685 | 230.182 | +4.497 |
| Window show to shell text present | 52.310 | 65.028 | +12.718 |
| Text processed to text present | 7.873 | 16.494 | +8.621 |
| First text frame | 7.695 | 16.270 | +8.575 |

Median within-pair differences differ from differences between variant medians: fonts-to-present **-9.464 ms**, main-to-present **-8.363 ms**, main-to-text **+3.905 ms**, text frame **+7.709 ms**. Individual paired fonts-to-present differences ranged from -28.510 to +24.298 ms; timing noise remains material. Every paired text-frame difference was slower for the candidate (+5.158 to +23.501 ms).

| Paired range (min–max; also max = p95 here) | Eager | Deferred |
| --- | ---: | ---: |
| Fonts to first present | 120.105–151.968 | 108.991–151.344 |
| Main to first shell text | 206.296–270.960 | 203.295–270.236 |
| First text frame | 6.480–10.085 | 13.818–30.509 |

Sequential batch medians show why the first apparent gain was misleading:

| Batch in execution order | Fonts to present | Main to present | Main to text | Text frame |
| --- | ---: | ---: | ---: | ---: |
| Eager `baseline-b`, n=15 | 136.304 | 157.188 | 240.598 | 7.668 |
| Deferred `candidate-a`, n=15 | 118.918 | 138.913 | 227.723 | 15.232 |
| Eager `baseline-c`, n=15 | 120.160 | 139.056 | 215.298 | 7.428 |
| Deferred `candidate-b`, n=15 | 119.001 | 136.402 | 221.820 | 15.171 |

The first sequential comparison suggested a 17.386 ms fonts-to-present gain; the repeat suggested only 1.159 ms. The alternating measurements recovered a gain consistent with the resource scopes, while consistently showing the text-frame penalty. The eager first batch also had a 1,437.997 ms main-to-first-present outlier. It and the deferred 235.819 ms first-text-frame outlier are retained. Their underlying driver/system causes were not established.

Pooled results are supplementary, not a substitute for controlling order:

| All 40 launches per variant | Eager median / p95 / max | Deferred median / p95 / max |
| --- | ---: | ---: |
| Fonts to first present | 124.917 / 151.779 / 1417.050 | 118.960 / 151.344 / 503.935 |
| Main to first shell text | 219.852 / 270.960 / 1529.024 | 223.370 / 312.148 / 837.619 |
| First text frame | 7.524 / 9.718 / 11.694 | 15.235 / 19.844 / 235.819 |

## Decision and remaining bottleneck

The historical **8.178 ms** defer candidate translated into roughly **8 ms earlier empty presentation** in the alternating confirmation. Its paired glyph-resource scopes measured **7.534 ms eager**, removed from the initial candidate frame, and **8.376 ms deferred**, paid during first text. This was displacement rather than an overall reduction: useful shell text did not arrive earlier and its frame nearly doubled in median duration. The experiment was therefore reverted, including temporary instrumentation, as requested. No architecture, backend, font, presentation-order or terminal behavior change remains.

The dominant remaining startup cost is wgpu instance/adapter/device+queue: paired medians **78.269 ms eager / 78.358 ms deferred**. CPU font preparation plus metrics was **28.357 / 26.752 ms**. These resources remain necessary for the current first frame. This experiment supplies no justification for whole-wgpu laziness or for changing that frame's contents.

## Final validation

The fully restored implementation passed formatting, workspace/all-target checks and tests (650 passed, four existing ignored), four doc tests, Clippy with warnings denied, and the release build. Candidate release builds, focused startup tests, renderer tests and two explicit native GPU tests also passed before measurement. The first final workspace test attempt encountered sandbox `Access denied` while spawning packaged PowerShell; both affected focused tests and the full suite passed with native process permissions. One final release startup smoke reached shell output and closed normally. The final documentation diff passed whitespace checks. No commit or push was made.
