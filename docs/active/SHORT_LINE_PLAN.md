# Short-line investigation plan

Goal: explain short-line throughput on `perf/short-line-output` and optimize only measured, material Terminal-owned costs while preserving semantics and bounded event-loop draining. No commit or push.

1. Define the unchanged workload, parser/PTY chunking, wrapping, history and rendering behavior.
2. Collect three untouched Windows x86_64 release runs of all existing workloads.
3. Extend opt-in aggregate diagnostics for line transitions, scrolling, history allocation/copying and grid movement; compare parser/state and visible rendering. Measure small, normal and prefilled history using diagnostic-only controls; never change production history limits.
4. Identify dominant per-line work. Implement the smallest semantically equivalent optimization only if measured cost is material. Preserve the 64-event / 4-ms per-pane drain bounds.
5. Add deterministic correctness regressions for changed paths; verify focused tests.
6. Repeat release comparisons, including large finite, continuous and input; exercise native input, interrupt, resize and pane/tab progression under sustained short-line output. Record limitations explicitly.
7. Run formatting, workspace all-target check/tests, doctests, Clippy with warnings denied and diff checks. Review scoped changes and reconcile task documentation.

Likely files: terminal-core grid/screens/scrollback and diagnostics, app throughput workload/report, measurement scripts, focused core tests, performance report. Architecture decisions remain in the primary agent; bounded measurement/validation work may use Luna under repository instructions.

Acceptance: evidence includes medians/ranges, byte/event/batch and LF/scroll/history counts, parse and projection/build costs, redraw/render/present counts, frame gaps, input dispatch, queue peak, history scaling and exact bottleneck. No unmeasured production optimization and no benchmark special casing.

Status: all seven steps are complete. All six validation checks passed. The user reported manual native acceptance passed on 2026-10-01: keyboard, Ctrl+C, resize, tab/pane switching and visible pacing remained responsive under sustained short-line output, with no long visible freezes. The final diff review confirmed the row-buffer candidate remains reverted after its controlled ANSI/presentation regression; only validated diagnostics/reporting changes remain. Evidence is in `docs/SHORT_LINE_THROUGHPUT.md`. No commit or push.
