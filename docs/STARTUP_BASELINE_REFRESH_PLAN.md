# Startup baseline refresh goal and plan

Branch: `perf/startup-baseline-refresh`.

## Goal

Create one clean, current Windows x86_64 release startup baseline after the recent startup work, before another Alacritty comparison. Use existing diagnostic markers, fresh processes with warm/uncontrolled caches, multiple cmd.exe / pwsh -NoProfile / normal-profile pwsh runs, milestone medians/ranges and explicit cost ownership. Retain evidence and compare trustworthy documented historical cohorts. No cold-start/reboot acceptance, new production optimization, commit or push. Only measurement tooling may change if needed.

## Plan and completion evidence

- [x] Confirm branch/clean starting state, inspect existing first-frame/PTY tooling, current configuration and relevant documented measurements. Release revision cb11a16144792a130c7d4d4a9c479e152aedbf02.
- [x] Build the current normal-dependency release; record binary/config hashes and Windows/compiler/CPU/GPU provenance.
- [x] Add a measurement-only consolidated harness with isolated configuration copies, rotated shell order, two excluded warmups, all required milestones, readiness validation and descendant cleanup. Preserve active user config.
- [x] Pilot collection and reject incomplete samples. Initial no-profile pilot exited without readiness; its cause remains unknown. Repeat and final smoke passed. Pilot output excluded.
- [x] Collect 15 valid runs for each of three shells, without concurrent validation/builds. Main batch has all 45 runs, all six warmups, all required markers, normal exit/cleanup and no forced termination. No retries/outlier exclusions in the main batch.
- [x] Compute median/min/max for elapsed milestones and same-run durations. Keep nested renderer scopes separate from nonoverlapping stage intervals. Preserve raw traces, configurations, CSVs, provenance and trace hashes.
- [x] Compare with latest stock renderer cohort, paired descriptor-budget work and recent shell/spawn/Alacritty cohorts; distinguish different integration/arguments and cache/system drift from demonstrated improvements.
- [x] Report biggest contributors, improvements and unchanged/slower costs; specify exact external and internal endpoints for the next matched Alacritty experiment.
- [x] Run full repository completion gates, verify harness syntax/final smoke, evidence integrity and final scoped diff. Reconcile session notes without overwriting unrelated function-key task state.

## Result

Complete. See [consolidated report](STARTUP_BASELINE_REFRESH.md) and [retained evidence](perf/startup-baseline-refresh/environment.json). No production code or dependencies changed. No new Alacritty measurements or pixel/cold-start claims. Full validation passed; no commit or push.
