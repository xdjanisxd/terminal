# Measurement evidence

See [the report](../../FONT_DISCOVERY_STARTUP.md) for definitions, inclusive scopes, exclusions and interpretation.

`runs.csv` contains 120 valid experimental launches. Timing columns are microseconds unless suffixed `_ms`; counts, run/process/exit fields and booleans are not durations. Empty dependency columns mean detailed profiling was disabled. Summary timing statistics and paired differences are microseconds; the report converts them to milliseconds.

Environment metadata records executable hashes and batch settings. The workspaces preserve default and configured-family cases. `excluded-baseline-a-*` preserves the initial instrumentation batch excluded because stderr reporting delayed startup.

The attribution patch records the temporary overlay against registry fontdb 0.24.0; normal builds do not apply it. `summarize.py` reads original per-launch inputs from ignored `target/font-discovery/`. Those local logs and saved binaries are not portable repository artifacts. The final registry smoke is validation only and excluded from summaries.
