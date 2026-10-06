# Wgpu device initialization investigation

Branch: `perf/wgpu-device-init`. No commit or push.

Goal: break down Windows x86_64 release wgpu startup, establish ownership of the dominant cost, and retain only measured, safe application optimizations or diagnostics/findings.

## Plan and acceptance

1. Trace the application's exact path and pinned wgpu 25.0.2 implementation. Preserve DX12 preference/fallback and `WGPU_BACKEND` overrides.
2. Extend opt-in startup scopes for backend policy, request invocation versus polling, adapter selection/device completion milestones, and request metadata. Existing surface/shader/pipeline scopes remain authoritative. Nested scopes are inclusive and must not be added to their parents.
3. Build the unmodified release baseline and diagnostic release. Measure at least 30 fresh-process launches each, with caches unchanged, consistent workspace, recorded environment and first launch reported separately. Check diagnostic overhead with interleaved comparisons.
4. If an opaque dependency interval dominates, use a temporary local dependency overlay to split DXGI enumeration, candidate device creation/capabilities, adapter selection, and queue/backend/core device setup. Restore manifest/lockfile and rebuild the ordinary binary afterward. Do not retain patched dependencies.
5. Audit redundant application queries, features/limits, synchronization and first-frame resource setup. Attribute API intervals without claiming that all native-call time is driver execution. Retain an optimization only with repeatable improvement and unchanged correctness.
6. Validate focused renderer tests, required full Rust gates, normal/override startup and existing resize/output tests. Document any unavailable native/manual coverage. Preserve the unrelated active function-key task.
7. Report per-stage median/p90/range, dominant stage and ownership, retained changes, first-frame comparison, limitations, and whether a wgpu-specific follow-up should precede an OpenGL prototype.

Status: complete and validated. Retained opt-in diagnostics and a measured DX12 descriptor budget reduction to 16,384. Thirty alternating launches per stock build, three native-trace cohorts, native DX12 pixel tests, override/diagnostics-off smoke and required workspace gates passed. Findings and limitations: [WGPU_DEVICE_INIT.md](WGPU_DEVICE_INIT.md).
