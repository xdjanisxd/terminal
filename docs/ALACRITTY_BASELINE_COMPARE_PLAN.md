# Current-baseline Alacritty startup comparison plan

Branch: perf/alacritty-baseline-compare. Warm/repeated Windows x86_64 only; no optimization, reboot/cold measurement, commit or push.

Authoritative Terminal reference: [startup baseline refresh](STARTUP_BASELINE_REFRESH.md). Verify its executable SHA-256 before collection. Keep historical comparisons separate.

1. Inspect existing tooling and endpoint definitions; preserve active configuration in isolated copies.
2. Match cmd AutoRun and PowerShell profiles, resolved binaries, cwd, font physical size, background/padding and client dimensions/DPI. Use shared readiness probes; label encoded PowerShell launches separately from native LocalShell baseline.
3. Smoke all three scenarios, then collect 15 alternating app pairs per scenario after two excluded warmup pairs. Rotate scenario order. No concurrent build/validation. Retain logs, configs, process provenance, observation gaps, cleanup and hashes.
4. Compare external process creation, window visibility flag, shell process creation and per-shell readiness. Distinguish presentation submission, visible pixels, native spawn-return and PTY Wakeup proxies. Record unavailable endpoints rather than infer them.
5. Summarize per-run intervals and paired deltas; attribute renderer/window, PTY/process, shell/profile, visibility policy and tooling separately. Keep inference bounded by evidence.
6. Validate measurement collection and repository state, preserve unrelated active function-key task, and deliver report recommending no backend prototype, another app-owned bottleneck investigation, or an OpenGL startup prototype.

Status: complete. Collection, attribution and validation are recorded in the [comparison report](ALACRITTY_BASELINE_COMPARE.md).
