# Current Task

- Goal: audit/minimize startup through first visible frame, with measured safety and native acceptance.
- Branch: `perf/minimal-first-frame`.
- State: dependency/cost audit complete; **no production deferral**. Opt-in monotonic scopes and fresh-process release harness retained. See [report](../MINIMAL_FIRST_FRAME.md) and its per-launch CSV.
- Decisions: no backend/thread/semantic change; shared rectangle resources required, glyph deferral not proven safe; GPU adapter/device creation dominates. Do not infer improvement from sequential diagnostic batches or call them cache-cold.
- Files: app main, renderer startup/font/gpu/lib, scripts/measure-first-frame.ps1, docs/MINIMAL_FIRST_FRAME.md, docs/measurements/minimal-first-frame.csv, startup/current-state links, this handoff and .agent/SESSION.md.
- Validation: Windows x86_64 release build, 7 baseline + 7 diagnostic launches, 3 split/inactive-tab launches, help/silent startup; fmt/check/all-target tests (650 passed, 4 ignored)/doc tests (4 passed)/Clippy passed. Final `git diff --check` passed.
- Remaining: manual release visual/input/focus/resize/Saved Workspace/integration acceptance and several independent cache-cold samples. Native computer-use guidance forbids terminal automation. ARM64 out of scope. No commit/push.
- Best next step: user runs the exact [acceptance checklist](../MINIMAL_FIRST_FRAME.md#reproduce-and-complete-acceptance) and supplies results/cache-cold logs. Goal blocked pending manual acceptance and independent cache-cold evidence; no further safe autonomous action remains.
