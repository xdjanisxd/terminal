# Font discovery startup: goal and plan

Branch: `perf/font-discovery-startup`. Status: complete and validated; production candidate reverted. The original plan below records the intended method; execution, deviations and the reverted candidate are in [findings](FONT_DISCOVERY_STARTUP.md). No commit or push.

## Goal

Reduce Windows x86_64 release CPU font preparation cost by measuring and removing unnecessary discovery/resolution work. Preserve configured family selection and errors, fallback ordering and coverage, glyph selection, DPI/font sizing, and runtime shaping/rasterization correctness. Keep only optimizations with a repeatable gain and unchanged behavior; otherwise revert production changes and retain findings.

Historical context is approximately 30 ms CPU font work, 0.1 ms metrics, and a larger wgpu initialization cost. These are not this experiment's baseline. Whole-wgpu initialization and the reverted glyph-resource experiment are outside scope.

## Initial source trace

- `crates/renderer/src/lib.rs::Renderer::new_with_font_settings` records `fonts-started`, wraps `FontSystem::load_system` in `font-family-preparation`, computes cell metrics separately, then records `fonts-ready` before GPU initialization.
- `crates/renderer/src/font.rs::load_system` creates a new `fontdb::Database` and calls `load_system_fonts`. Existing `system-font-discovery` timing includes dependency-internal enumeration, loading and metadata parsing.
- On Windows, `from_database` calls `select_primary_face`: one generic-monospace or named-family query, followed by the existing monospace check and error policy.
- `with_primary_face` collects non-primary face IDs and sorts by PostScript name and ID, then creates shaping/scaling contexts and empty fallback/glyph/shaped-text caches. The sorting key currently clones names and formats IDs; whether this matters must be measured.
- Metrics, character support, shaping and rasterization access bytes through `Database::with_face_data`. Trace its actual file/data lifetime before claiming redundant loading or introducing reuse.
- Fallback candidate ordering occurs at startup; character-specific fallback selection occurs during text shaping. Preserve that distinction when reporting costs.

Ownership stays in the renderer's font module. No terminal semantics, backend strategy, public API or threading changes are planned.

## Execution plan

1. **Trace and define costs.** Inspect the locked `fontdb` implementation and enabled features, narrowly following Windows discovery, directory/registry enumeration, file loading/mapping, collection-face parsing, family queries and face-data access. Count calls, files, faces, bytes where obtainable, query repetitions and data reuse. Distinguish metadata parsing during discovery from Swash face access for metrics and rendering. Record operations not executed at startup as such.
2. **Add opt-in measurement.** Extend existing startup diagnostics for database construction, enumeration/querying, file opening/loading, face metadata parsing, configured/generic family lookup, monospace validation, fallback collection/sorting, context/cache setup, selected-face access and metrics. Use aggregate timers/counters for high-frequency operations, avoid per-file synchronous log writes, and report inclusive versus exclusive scopes without double counting. Where library internals cannot be separated from project call sites, use a temporary dependency overlay or native profiling for attribution; restore the dependency/lockfile afterward. Do not replace library behavior to make it measurable. Measure instrumentation overhead with coarse scopes and diagnostics-disabled checks.
3. **Establish a baseline.** Build Windows x86_64 release with identical instrumentation for both variants. Preserve an eager baseline executable and record revision/hash, toolchain, OS/GPU, backend, configuration, workspace and cache conditions. Collect at least 15 fresh-process baseline launches. Track per-run CPU preparation, preparation plus metrics, and `fonts-started` to first present. Exercise default monospace and an explicitly configured installed monospace family separately.
4. **Choose one measured candidate.** Rank sub-costs and repeated operations before editing production behavior. Prefer removing duplicate queries/data access, reusing already-resolved handles/data with correct ownership, avoiding repeated database work, or cheaper equivalent fallback ordering. Evaluate deferred fallback work only if discovery/coverage/order remain identical and first text/fallback use does not incur a stall. Do not assume any candidate is worthwhile. Keep existing libraries; persistent disk caches require clear evidence and a separately justified design, not speculative implementation.
5. **Prove equivalence.** Run affected font tests first, then the renderer crate. Cover named/default selection, missing and proportional-family errors, stable fallback and missing-character handling, collection-face indices, glyph IDs/advances/bitmap output, cache eviction and scale invalidation, and metrics across DPI/font sizes. Compare faces by stable source/face identity rather than database-local IDs alone. Add only tests needed for the chosen change. Run native Windows font shaping/rasterization coverage and diagnostics-disabled visual text/fallback/DPI checks; report unavailable acceptance honestly.
6. **Compare repeated releases.** Run at least 15 candidate launches, then confirm with at least ten adjacent baseline/candidate pairs whose launch order alternates. Do not build/test during measured launches. Keep configuration, instrumentation, backend and cache conditions equal; do not call fresh-process runs cache-cold. Report medians, ranges, individual paired differences and tail/outlier evidence. Retain every valid sample and document exclusions. Check first-shell-text and representative first-fallback rendering too if any work was deferred or font-data lifetime changed.
7. **Retain or revert.** Require unchanged font semantics and repeatable CPU-font improvement beyond observed noise, supported by the measured sub-cost reduction. Inspect fonts-to-present independently because GPU/driver noise can hide a CPU saving. Do not keep a negligible improvement, altered glyph/fallback behavior, or a stall merely shifted to first use. If rejected, restore production code and temporary dependency modifications and keep only useful diagnostics/evidence/findings.
8. **Validate and report.** Apply `validate-rust-change` at completion: formatting, workspace/all-target check/tests, doc tests, Clippy with warnings denied and diff checks, plus release/native smoke appropriate to the final code. Use `finish-task` to reconcile the active task and session notes. Record exact stage definitions/breakdown, dominant sub-cost, changes retained/reverted, before/after CPU and fonts-to-present results, remaining bottleneck, limitations and the evidence-supported next task. Do not commit or push.

## Likely files

- `crates/renderer/src/font.rs`: discovery/resolution instrumentation and any justified optimization/tests.
- `crates/renderer/src/startup.rs`, `crates/renderer/src/lib.rs`: only if diagnostic aggregation or boundary attribution requires changes.
- `scripts/measure-first-frame.ps1`, `scripts/summarize-first-frame.ps1`: explicit executable selection, reproducible comparison metadata and font-stage summaries as needed.
- `docs/FONT_DISCOVERY_STARTUP.md` and `docs/measurements/font-discovery-startup/`: final findings and raw evidence; performance index and session/task handoff.

At planning time, no runtime change, dependency change, benchmark or before/after claim had been made.

## Acceptance and next step

The task completes with either a validated optimization and measurements, or a fully reverted experiment with actionable findings. Rendering/configuration semantics take precedence over speed. Remaining wgpu cost must be reported without expanding this task to GPU initialization.

Original next step: inspect the exact locked `fontdb` Windows discovery implementation and define non-overlapping timing/counter boundaries before collecting the baseline. That investigation and 120 valid repeated launch measurements are now recorded in the findings.
