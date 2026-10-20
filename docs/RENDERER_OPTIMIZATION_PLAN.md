# Renderer optimization plan

Base: `5e6bf41` (latest `origin/master` after fetch on 2026-10-20).
Branch: `perf/renderer-optimization`, already at the base with a clean working tree.
No commits or pushes are authorized.

1. Review renderer ownership, prior measurements and rejected experiments.
2. Establish repeated optimized CPU projection and offscreen rendering baselines;
   retain raw samples and environment. Separate generation/upload/submission from
   GPU execution and display latency.
3. Locate the dominant remaining cost in viewport scanning, projection, glyph
   instances, uploads and redraw scheduling. Review reflow, emoji, themes and blink.
4. Trial small changes only where measurements justify them. Keep terminal
   semantics in core; preserve cache identities, Unicode, alpha and frame pacing.
   Reject regressions and avoid speculative damage tracking or redesigns.
5. Repeat equivalent workloads; add focused regressions, then run final formatting,
   workspace checks/tests, Clippy and release build as appropriate. Record native
   acceptance performed and outstanding Linux/Windows checks honestly.
6. Publish methodology, before/after results, retained/rejected work, CPU/GPU/memory
   and responsiveness limits, and a manual acceptance checklist.

The session exposes Default mode only. This file is the explicit working plan;
the task is tracked with the Goal tool.

Completed: architecture/history review, pre-change baseline, nine alternating
hardware workload pairs, nine projection pairs, typed-key trial and retention,
identity regression, native GPU/Wayland correctness checks, final workspace
validation and [measurement report](RENDERER_OPTIMIZATION.md). No rejected
historical approach was reintroduced. Manual Linux/Windows checklist items and
unmeasured latency/GPU execution limits are explicit in the report.
