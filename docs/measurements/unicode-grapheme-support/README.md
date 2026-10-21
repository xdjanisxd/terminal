# Unicode grapheme measurements

Baseline: remote master `e042ac631523e65c54b11432f335a9d00594d5ca`, checked
with `git ls-remote` on 2026-10-21. The clean task branch already pointed there.
Candidate: uncommitted `feat/unicode-grapheme-support` changes.

Host: Linux x86_64, Intel Core i5-1250P, rustc 1.99.0. Offscreen GPU: Vulkan
Mesa llvmpipe (LLVM 19.1.7; Mesa 25.0.7). These are software-rendering results;
no native hardware GPU, compositor or Windows claim follows from them.

## Method

The baseline source was unpacked with `git archive HEAD` before implementation.
Only the identical new parse/shaping benchmark harnesses were added to that
isolated `/tmp` copy. Existing core/projection/GPU harnesses use identical input
on baseline and candidate. Release builds use the same target cache and host.
Benchmarks run sequentially without competing builds/tests in the measured phase.

- New parse benchmark: 1,000 warmups, five samples of 50,000 operations for each
  case, no scrolling. The ASCII case is 36 characters plus carriage return;
  each other case is one cluster plus carriage return. Reports median ns/input.
- Shaping: five samples of 2,000 operations, both bypassing the shaped-text cache
  and hitting it. Font-selection caches are warm. This does **not** measure
  discovery/startup or first-ever fallback lookup. Baseline can render the wrong
  text; its output is a timing reference, not a correctness oracle.
- Existing state/reflow benchmarks: three before/after pairs, median ns/operation.
  The two resizes in a pair are one operation, not one individual resize.
- Projection: the first three short pairs were noisy (the candidate median for
  the visible case was +14.7%, with overlapping ranges). Twenty alternating
  before/after pairs, pinned to the same permitted CPU 0, resolved that result.
  The table uses their medians. Before range: 17,413.9–18,735.2 ns; candidate:
  17,338.4–20,606.3 ns. The original short measurements are not used for claims.
- GPU: three runs of 100 frames per workload, five warmup frames, bounded
  in-flight work and a device wait per frame. Table reports median ns/frame.
  The Unicode workload changes from 180 projected cells to 150 because core now
  combines the ZWJ sequence. Bytes are identical; cell counts should differ.
- Debug invocation of the new parse harness is intentionally a no-op, because
  debug grid invariant checking dominates timing. All measurements use release.

Raw samples and medians are in [benchmarks.json](benchmarks.json); neighboring
logs retain compiler/adapter/workload output. The initial one-off samples were
superseded by the repeated comparisons above.

## Results

| Workload | Before | After | Change |
|---|---:|---:|---:|
| ASCII parse (36 chars, no scrolling) | 2,203.1 ns | 2,186.5 ns | -0.8% |
| Combining cluster parse | 93.4 ns | 132.2 ns | +41.5% |
| ZWJ cluster parse | 223.8 ns | 315.3 ns | +40.9% |
| Flag parse | 189.0 ns | 195.6 ns | +3.5% |
| Keycap parse | 131.6 ns | 246.4 ns | +87.2% |
| ASCII output with scrolling | 7,760.5 ns | 8,012.8 ns | +3.3% |
| Width resize pair, 10,000 history rows | 6,794,186.7 ns | 6,817,921.4 ns | +0.3% |
| Unicode 80↔1 column resize pair | 429,929.4 ns | 427,652.3 ns | -0.5% |
| Visible projection, 80×30 | 17,780.3 ns | 17,650.3 ns | -0.7% |
| Echo plus projection | 18,256.4 ns | 18,190.6 ns | -0.4% |
| Three-pane layout plus projection | 23,287.1 ns | 23,197.6 ns | -0.4% |
| ASCII shaping, shape cache bypassed | 14,182.3 ns | 14,112.7 ns | -0.5% |
| Combining shaping, shape cache bypassed | 19,774.6 ns | 8,859.3 ns | -55.2% |
| ZWJ shaping, shape cache bypassed | 66,395.9 ns | 12,541.5 ns | -81.1% |
| Flag shaping, shape cache bypassed | 23,280.4 ns | 11,860.3 ns | -49.1% |
| Keycap shaping, shape cache bypassed | 50,665.5 ns | 11,981.6 ns | -76.4% |
| ZWJ shaping, shape cache hit | 60.8 ns | 54.3 ns | -10.7% |
| Unicode GPU instance generation / frame | 34,833.9 ns | 23,770.4 ns | -31.8% |
| Unicode GPU wall time / frame | 378,973.5 ns | 348,459.6 ns | -8.1% |
| Dense GPU instance generation / frame | 707,011.3 ns | 746,195.0 ns | +5.5% |
| Dense GPU wall time / frame | 6,048,473.0 ns | 6,193,682.2 ns | +2.4% |

ASCII parsing/shaping and sparse projection remain within small run variation;
ASCII output with history allocation is +3.3%. Existing reflow is unchanged in
scope and cell size, and the 10,000-row width pair is +0.3%.

Unicode parsing now does bounded segmentation and whole-string width calculation:
combining and ZWJ cases rise about 40%, and keycaps 87% (only 115 ns more per
input here). This cost is accepted for correct cluster semantics, with no ASCII
segmentation or heap allocation. A first implementation repeatedly searched
fallback fonts on every shaping miss; that version was rejected. Bounded
fallback memoization and keeping the complete cluster on one face produce the
shaping results above.

Warm Unicode GPU instance generation improves 31.8%. Dense ASCII generation is
+5.5% and wall time +2.4% in this software GPU comparison; this small cost and
host noise are disclosed, not presented as a win. The absolute generation
increase is about 39 microseconds for 7,200 projected cells per frame. Hardware
acceptance remains necessary before claiming equivalent native performance.
Across unchanged GPU frames, raster calls and buffer bytes stay zero, and the
acceptance test additionally asserts zero shaping misses, buffer writes and
buffer allocations.

## Commands

```sh
cargo bench -p terminal-core --bench unicode_baselines --offline
cargo bench -p terminal-core --bench state_baselines --offline
cargo bench -p terminal-renderer --bench projection_baselines --offline
cargo test -p terminal-renderer --release grapheme_shaping_baselines --offline -- --ignored --nocapture
cargo test -p terminal-renderer --release renderer_workload_baselines --offline -- --ignored --nocapture
```

`--bench` selects the named fixed-work harness; `--release` enables optimized
test binaries; `--offline` uses the cached dependency graph. `--` passes following
flags to Rust's test runner: `--ignored` explicitly opts into native/font/GPU
requirements and `--nocapture` retains raw adapter and measurement output.
The shape/GPU harnesses are opt-in and have no timing thresholds.

## Verification

Full workspace `--all-targets` validation passed 872 tests with 30 ignored.
The final no-op invalidation fix was followed by 127 focused state/integration
passes (including 15 grapheme tests), 877 completion workspace/doc-test passes
(30 ignored),
workspace all-target checks, Clippy with warnings denied, formatting and release
build. Final logs are retained alongside these measurements.

The initial all-target run was interrupted in the new debug-only timing harness;
a release-only guard was added and the complete all-target run then passed. No
unit/integration failure was hidden by that interruption. An early color test
incorrectly required chromatic ink from grayscale Noto family artwork; the
final test checks visible ink, RGBA content and tint invariance for that fixture.

Installed-font shaping and offscreen GPU acceptance pass. The GPU test covers
12 format/scale/opacity scenarios with representative emoji distributed among
them. First/bold shaping checks cover ZWJ profession/family, flag, modifier,
keycap and emoji presentation. No fuzz campaign or interactive Linux/Windows
acceptance was run. The [manual checklist](../../UNICODE_GRAPHEME_SUPPORT.md#linux-and-windows-manual-acceptance-checklist)
remains pending on both platforms.
