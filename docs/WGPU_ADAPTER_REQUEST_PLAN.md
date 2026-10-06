# Wgpu adapter-request goal and plan

Branch: `perf/wgpu-adapter-request`. No commit or push.

Goal: Break down remaining adapter-request startup cost on Windows x86_64, distinguish candidate/native-device creation from unused cleanup, and retain only justified low-risk application improvements or diagnostics. Preserve DX12 preference, fallback, WGPU_BACKEND, renderer correctness and native pixel tests; do not switch backend.

1. Complete: trace application → pinned wgpu/core/HAL path and identify synchronous request, candidates, lifetime and fallback policy.
2. Complete: add opt-in temporary timings for DXGI enumeration, native creation, capability queries, surface checks, selection, actual owned native reference release and unused iterator destruction. Buffer output outside the internal request timer after immediate output proved intrusive.
3. Complete: measure 15 initial stock, 15 buffered diagnostic and 15 final stock fresh-process release launches; retain all raw samples and environment metadata.
4. Complete: assess application candidates. No supported low-risk shortcut; retain only diagnostics/findings. Intel native-device creation dominates.
5. Complete: full repository formatting/check/all-target tests/doctests/Clippy/diff validation, native DX12 pipeline/pixel tests, explicit override smoke, recipe reproduction and final dependency audit pass.
6. Complete: findings, plan, performance index and session handoff reconciled; recommendation is shutdown reliability. Goal completion follows final repository audit.

Acceptance evidence and limitations: [WGPU_ADAPTER_REQUEST.md](WGPU_ADAPTER_REQUEST.md). Unrelated pending function-key active task remains separate.
