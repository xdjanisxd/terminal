# Defer glyph resources: goal and plan

Branch: `perf/defer-glyph-resources`. Status: complete; candidate implemented, measured and reverted after failing the first-text latency gate. No commit or push. See [findings and raw measurements](DEFER_GLYPH_RESOURCES.md).

## Goal

Reduce Windows x86_64 release first-frame startup cost by deferring only glyph-specific renderer resources unused by the initial empty, cursor-bearing frame. Preserve background, cursor, selection/UI, text, renderer lifecycle and backend behavior. Initialize resources once per draw-resource lifetime, render the first required glyphs in that same frame, and retain the change only if repeated measurements show a meaningful startup improvement without visible hitching or stale output.

The prior investigation measured 140.833 ms median from fonts start to first present, including approximately 85.818 ms for wgpu initialization, 31.022 ms for CPU fonts, and an 8.178 ms glyph-resource candidate. These are historical context, not the baseline for this experiment. See [first-frame pipeline costs](FIRST_FRAME_PIPELINE_COSTS.md).

## Findings from initial inspection

- `crates/renderer/src/gpu.rs::DrawResources::new` eagerly creates the glyph bind-group layout, glyph render pipeline (including its pipeline layout), and `GlyphAtlas`: texture, view, sampler, bind group and associated CPU bookkeeping. These are the candidate resources.
- The shared shader module and rectangle pipeline remain necessary for the first cursor-bearing frame. Device/queue, surface, fonts/metrics and rectangle rendering retain their existing initialization behavior. Glyph and overlay instance buffers already allocate on demand.
- `DrawResources::draw_pane` accesses the atlas while generating glyph instances, before render encoding. Initialization must therefore occur before the first required atlas access, not merely at the final glyph draw call. Blank/background/underline-only content must not initialize glyph resources.
- `crates/renderer/src/lib.rs` retains draw resources across same-format resize and discards them when the surface format requires a rebuild. Exactly-once means once within each such lifetime, with legitimate recreation after invalidation.
- The existing measurement harness waits for `first-shell-output-rendered`, but its CSV parser stops at first present. Detailed startup scopes stop at window show. Extend measurement coverage before collecting either comparison build.

## Execution plan

1. **Confirm resource use and lifecycle.** Trace terminal and UI/overlay text through projection, shaping, atlas use and draw encoding. Check the shared shader and pipeline inputs, multi-pane frames, surface failure/retry paths, format rebuilds, font/DPI changes and existing native GPU tests. Keep ownership in the renderer; no public API, backend, PTY ordering or terminal-semantics change is intended.

2. **Prepare comparable instrumentation and capture baseline.** Extend `scripts/measure-first-frame.ps1` and `scripts/summarize-first-frame.ps1` to retain first-text milestones and durations. Verify the existing milestone corresponds to a successfully presented frame with shell glyphs, rather than receipt of bytes or a cursor-only control sequence. Add narrowly scoped opt-in timing for first shell-output readiness, first glyph-resource creation, and first text frame through present where existing hooks cannot distinguish these. Preserve the disabled diagnostics path and use identical instrumentation in both builds. Build `terminal-app` for Windows x86_64 release and save an identifiable baseline binary before changing initialization.

3. **Implement the smallest renderer change.** Group glyph-only GPU resources in optional renderer-owned state. Retain the shared shader and surface format needed to build them later without recompiling the shared shader. Initialize synchronously at the first actual glyph requirement, before atlas upload and encoding, then reuse the completed state. Include text required by initial UI or restored content. Never skip or postpone a text frame to perform initialization. Preserve existing resource invalidation and atlas/cache behavior.

4. **Verify correctness and initialization count.** Extend focused renderer/native GPU tests for an empty cursor frame with absent glyph resources, background/selection/underline/UI rectangles without glyphs, a first text frame with correct pixels and resources initialized, repeated and multi-pane text frames reusing those resources, and resize/format rebuild behavior. Exercise UI text, Unicode/fallback, DPI/font changes and surface retry behavior using existing tests or targeted native probes. Inspect the visible release first-text transition for hitching and stale frames with diagnostics disabled.

5. **Measure both builds under matching conditions.** Use the same workspace, shell, window size, font, backend, host and instrumentation. Run at least 15 fresh-process samples per build, sequentially with complete app/descendant cleanup and the existing inter-run pause. Alternate baseline and candidate batches, then repeat the comparison to distinguish the expected small gain from the drift seen in previous measurements. Keep builds and validation workloads outside timed batches. Retain raw logs, CSVs, binary hashes, environment/backend metadata, cleanup results and all successful samples; explain failed launches separately.

   Report medians, p95 and min/max for main-to-first-present, fonts-start-to-first-present, main-to-first-shell-text-present, window-show-to-first-shell-text-present, shell-output-ready-to-text-present, and first-text frame CPU duration. Report deferred initialization cost separately. The output-ready interval and frame cost distinguish renderer delay from variable shell startup. Presentation timestamps describe host submission, not compositor scanout; visual acceptance is a separate check.

6. **Apply the keep/revert gate.** Retain the defer only if the first-frame benefit repeats beyond observed run/batch noise and the first-text measurements plus visible inspection show no noticeable stall or stale frame. Compare the measured benefit with the historical approximately 8 ms candidate, accounting for driver/compiler work that may still occur earlier. If the benefit is negligible, cannot be distinguished from noise after confirmation, or introduces visible hitching, revert the resource deferral and retain measurement findings and useful instrumentation. Do not compensate by changing the backend or first-frame contents.

7. **Validate and finish.** Use the repository `validate-rust-change` skill for completion checks after the final code state is selected, and `finish-task` for documentation/task reconciliation. Update implemented-behavior documentation only for changes actually retained. Produce a findings report listing exact resources deferred (or reverted), both timing comparisons, first-text/visual results, whether the approximately 8 ms candidate became real startup improvement, and the remaining dominant startup cost. No commit or push.

## Expected files

Primary implementation: `crates/renderer/src/gpu.rs`; lifecycle integration only if needed in `crates/renderer/src/lib.rs`. Measurement support: `crates/renderer/src/startup.rs`, the existing shell-output timing owner if needed, and the two first-frame scripts. Retained evidence/report: a new task-specific directory under `docs/measurements/` and a focused findings document linked from `docs/PERFORMANCE.md`.

## Acceptance criteria

- Empty first-frame contents and presentation-before-show/PTY ordering remain correct.
- Glyph resources remain absent until a frame requires glyph rendering, including UI text.
- Initialization occurs once per resource lifetime, with correct recreation after invalidation.
- First text is rendered in its original frame with no stale output or visible initialization hitch.
- Repeated native Windows x86_64 release measurements cover both first present and first shell text.
- The final implementation passes required validation; an unsuccessful optimization is reverted with findings retained.
- Renderer/backend behavior is preserved, and nothing is committed or pushed.
