# Windows wgpu device initialization

Branch: `perf/wgpu-device-init`. Windows x86_64 release investigation, 2026-10-06. No backend switch, commit or push.

## Result

Retain opt-in diagnostics and reduce `Limits::max_non_sampler_bindings` from the wgpu default 1,000,000 to 16,384. This is an application-owned DX12 descriptor-heap sizing decision. In alternating five-run batches (30 launches per build), median device/queue request fell **51.670 → 31.182 ms** (39.7%) and first frame **213.571 → 181.545 ms** (15.0%, 32.026 ms). Median instance/surface/adapter/device initialization fell **109.438 → 84.236 ms**. The remaining dominant application interval is **adapter request**, 43.812 ms median, versus device request 31.182 ms.

First-frame p90 did **not** improve: 329.824 → 332.349 ms. Ranges were 177.418–421.386 ms before and 151.890–1,218.808 ms after. The after outlier had ~403 ms rectangle and ~371 ms glyph pipeline work. Do not promise a tail improvement or generalize this single-host result to every driver. These runs measured more than the earlier ~80 ms GPU baseline; that earlier figure is context, not the before cohort here.

## Measurement method and evidence

Windows 11 Pro build 26300, Core i5-1250P, Intel Iris Xe driver 32.0.101.7088, rustc 1.98.1, x86_64-pc-windows-msvc, release profile. Existing `scripts/measure-first-frame.ps1` starts fresh processes with a fixed cmd workspace, unchanged caches, waits for rendered shell output, checks first-present/show/PTY ordering, then closes the window and descendants. No build or validation ran during measurement. This is process-cold, **not reboot/cache-cold**, and includes scheduler/load variation.

The principal comparison uses diagnostic-enabled stock-dependency binaries with identical instrumentation: before five launches, after five launches, repeated six times. Nearest-rank p90; even-count median averages central values; durations are wall-clock microseconds. Backend override unset. SHA256/environment per cohort/batch are retained in [environment.json](perf/wgpu-device-init/environment.json). Raw [samples](perf/wgpu-device-init/paired-samples.csv) and [stage costs](perf/wgpu-device-init/paired-costs.csv) are retained. All 60 launches exited normally without forced cleanup.

Batch median first frames before/after (ms): 241.527/216.242, 212.590/169.555, 214.551/171.852, 197.308/158.446, 209.214/276.906, 230.989/185.218. Five of six batches improved. First process in the principal before/after cohorts was 262.624/1,218.808 ms; it is included, not discarded. Separate exploratory baseline and instrumented runs informed the investigation but are not pooled into this comparison. Opt-in logging overhead and clock reads remain in both principal builds; absolute uninstrumented timings can differ.

## Application timing breakdown

| Stage | Before median / p90 / range (ms) | After median / p90 / range (ms) |
|---|---:|---:|
| gpu-backend-policy | 0.013 / 0.018 / 0.010–0.109 | 0.011 / 0.020 / 0.010–0.038 |
| wgpu-instance | 8.189 / 12.372 / 6.338–14.905 | 8.432 / 12.735 / 6.060–20.605 |
| surface-create | 0.034 / 0.045 / 0.030–0.058 | 0.035 / 0.069 / 0.030–0.076 |
| adapter-request-call | 45.901 / 80.779 / 39.414–105.510 | 43.812 / 78.230 / 37.481–151.391 |
| adapter-request-poll | 0.007 / 0.016 / 0.006–0.028 | 0.007 / 0.012 / 0.006–0.017 |
| device-request-call | 51.669 / 82.579 / 41.157–107.565 | 31.182 / 52.789 / 27.212–161.961 |
| device-request-poll | 0.003 / 0.006 / 0.002–0.012 | 0.003 / 0.005 / 0.002–0.007 |
| gpu-initialize | 109.438 / 160.996 / 90.863–227.470 | 84.236 / 143.224 / 72.959–323.076 |
| adapter-info | 0.001 / 0.002 / 0.000–0.005 | 0.001 / 0.002 / 0.000–0.003 |
| surface-capabilities-and-config | 0.011 / 0.020 / 0.008–0.055 | 0.010 / 0.015 / 0.008–0.022 |
| surface-configure | 6.411 / 10.741 / 4.402–30.746 | 6.413 / 9.063 / 4.746–13.702 |
| shader-module-create | 0.417 / 0.848 / 0.321–1.108 | 0.409 / 0.774 / 0.322–1.719 |
| glyph-bind-group-layout | 0.051 / 0.100 / 0.040–1.260 | 0.051 / 0.081 / 0.037–0.177 |
| rectangle-pipeline | 7.357 / 13.139 / 5.506–14.640 | 7.307 / 10.221 / 5.439–402.758 |
| glyph-pipeline | 8.420 / 17.406 / 5.948–19.944 | 7.655 / 10.118 / 6.227–371.201 |
| glyph-atlas | 2.413 / 3.979 / 1.520–6.163 | 2.172 / 2.863 / 1.455–4.810 |

Scopes are inclusive: do not sum `gpu-initialize`, request parents or shader/atlas parents with their children. Costs CSV sums repeated scopes within a run through first frame. `adapter-selected` and `device-queue-ready` are timestamp milestones, not additional work. Configuration here includes initial surface setup; shaders, pipelines, atlas and submission occur later, before first present. No application pipeline cache is supplied. The release metadata reports Fxc and the default VALIDATION_INDIRECT_CALL flag; no Dxc initialization or custom cache load was introduced. Pipeline backend work remains within the existing pipeline scopes.

## Exact path and ownership

- `Renderer::new_with_font_settings` prepares fonts/metrics, then `initialize_gpu`, then adapter metadata and initial surface reconfiguration. First rendering initializes the existing shaders, rectangle/glyph pipelines and atlas before present. No resources are deferred to first text.
- Application backend policy uses `InstanceDescriptor::from_env_or_default`. Without a Windows override it tries DX12 first and the original available backend set on initialization failure. `WGPU_BACKEND` takes precedence. The timing/limit changes leave that policy intact.
- In pinned wgpu 25.0.2, `wgpu/src/backend/wgpu_core.rs` synchronously calls core for request_adapter/request_device and returns ready futures. Polling is only microseconds. Moving these calls into speculative async code would not eliminate their native work.
- `wgpu-core/src/instance.rs::request_adapter` enumerates and exposes backend candidates, checks surface compatibility, sorts only for a non-default power preference, then chooses the first and drops the remainder. This renderer has no duplicate enumeration request. Cached `get_info` is ~1 µs.
- `wgpu-hal/src/dx12/instance.rs` loads D3D12/DXGI and creates the factory. Surface creation initially stores HWND/factory state; swapchain creation is in configure. Native DXGI/D3D12 calls enter OS/driver components.
- `wgpu-hal/src/dx12/adapter.rs::Adapter::expose` calls **D3D12CreateDevice for each candidate** and queries capabilities. `Adapter::open` reuses the selected native device, creates a queue and backend resources. Thus the name “device request” does not include all native device creation.
- `wgpu-hal/src/dx12/device.rs::Device::new` sets up allocator, fence, zero buffer, indirect command signatures, shader-visible descriptor heap, sampler heap and resource pools. `descriptor.rs::GeneralHeap::new` sizes its non-sampler heap from the requested limit. The pinned `wgpu-types` limit documentation explicitly describes its large upfront DX12 allocation. Other default limits/features were retained.

The application owns descriptor budget, backend policy and resource descriptors. wgpu core owns candidate selection, compatibility/limit validation and resource bookkeeping. The DX12 backend owns native enumeration/exposure, heaps, fences/signatures and swapchain orchestration. Driver/OS components implement native calls and destruction. Wall-clock scope boundaries cannot separate driver, OS, scheduling and wrapper CPU time; ETW/native tracing would be needed for that attribution.

## Dependency-internal breakdown

A measurement-only overlay of pinned hal/core sources adds RAII scopes, preserving production calls. It was built separately, measured, and removed from manifests/lockfile. No dependency patch is retained. [Recipe](perf/wgpu-device-init/native-overlay.json), [raw internal scopes](perf/wgpu-device-init/native-costs.csv), and preparation script make the trace repeatable. There are two earlier 15-run hal-only cohorts at 1M/16K descriptors and a later 15-run detailed core+hal cohort at 16K. These are distinct experiments, with extra logging; do not subtract them from principal stock timings or sum their medians as a total.

| Internal stage (optimized detailed build) | Median / p90 / range (ms) |
|---|---:|
| d3d12-library-load | 0.447 / 0.630 / 0.350–0.699 |
| dxgi-library-factory | 6.892 / 7.858 / 6.000–8.283 |
| dxgi-enumerate | 0.265 / 0.375 / 0.255–0.532 |
| d3d12-create-device | 37.286 / 43.375 / 34.407–103.936 |
| dx12-adapter-expose | 55.262 / 60.758 / 51.535–120.546 |
| core-adapter-selection-and-discard | 43.195 / 49.877 / 39.539–61.843 |
| d3d12-create-queue | 6.214 / 7.520 / 4.871–7.731 |
| d3d12-create-idle-fence | 0.038 / 0.063 / 0.028–0.073 |
| d3d12-zero-buffer | 0.362 / 0.630 / 0.264–0.672 |
| d3d12-command-signature | 0.527 / 0.850 / 0.372–76.801 |
| d3d12-shader-visible-descriptor-heap | 1.371 / 2.765 / 0.977–3.505 |
| dx12-sampler-heap | 0.170 / 0.341 / 0.112–0.399 |
| dx12-device-resources | 6.439 / 17.331 / 5.801–83.473 |

Repeated candidate stages are summed per launch (two candidates: Iris Xe and Microsoft Basic Render Driver). For command signatures this table uses the **first three** calls, from initial device-resource setup; raw data also contains later pipeline calls. Expose includes native device creation and capability queries. Selection-and-discard includes debug metadata, selection, `Adapter::new`, and destruction of unused candidates. Default preference requires no sorting. Its ~43 ms suggests unused-candidate teardown is important, but this scope does not isolate the individual release calls; that remains a hypothesis for follow-up.

The detailed cohort’s first native-device creation total was 103.936 ms versus a 37.286 ms cohort median; first device-resource setup was 83.473 ms versus 6.439 ms median. These first-launch effects are included. In earlier hal-only cohorts, descriptor allocation medians were 38.440 ms at 1M and 15.880 ms at 16K; in the later detailed cohort 16K allocation was 1.371 ms. Native timing varies substantially with process/driver/cache state and tracing boundaries. The strongest optimization evidence is the alternating **stock dependency** comparison, not cross-cohort subtraction. After the budget reduction, candidate native-device creation/capability exposure and selection/discard dominate the finer adapter path.

## Application audit and retained optimization

Only one atlas bind group is created and reused across panes and ordinary same-format resizes. It binds the atlas texture/sampler; DX12 also needs sampler indexing bookkeeping. Per-frame instance buffers are vertex buffers, not additional bind groups. Format changes may retire/recreate resources. 16,384 descriptors leaves ample headroom for this resource model and avoids asking DX12 to reserve a million descriptors. It is not a universal budget for future texture-heavy rendering; reassess if the resource model changes. This limit affects only the DX12 backend, so explicit other-backend request limits retain their effective behavior.

No redundant application adapter enumeration, unnecessary requested feature, repeated startup capability query, or explicit startup device-poll synchronization was found. Surface default configuration is queried once per reconfigure; its ~10 µs startup cost is immaterial. wgpu's compatibility and capability queries are dependency-owned correctness work, not removable duplicate app calls. Pipeline creation is ~15 ms combined at the median and remains necessary for current first-frame output. No speculative pipeline-cache, asynchronous initialization or backend change was retained.

## Validation and follow-up

Focused renderer tests passed (53, four ignored); both ignored native release GPU tests were then run explicitly on DX12 with the production device descriptor and passed, including pixel readback for background, glyph, underline and cursor. Existing resize/configuration tests passed. Required completion gates passed: formatting, workspace/all-target check, 672 workspace tests (five ignored), four doctests, Clippy with warnings denied, diff whitespace check. Three explicit `WGPU_BACKEND=dx12` launches passed; diagnostics-off release launch and normal close passed without diagnostic output. Principal startup ordering checks passed throughout. Hardware-failure fallback and interactive resize/minimize acceptance were not newly exercised; policy tests and existing resize tests passed. Other GPUs/OSes were not measured.

**Investigate wgpu DX12 candidate exposure and unused-adapter destruction next, before an OpenGL prototype.** Isolate native release calls, measure hardware/software candidates separately, and consult upstream supported ways to avoid unnecessary candidate work without changing adapter preference, compatible-surface or fallback semantics. Use ETW to distinguish driver/OS work if useful. Do not merely select a hardcoded adapter or discard fallback to chase these timings. An OpenGL prototype can follow if this remaining dependency/native cost has no safe supported improvement; this investigation alone does not justify changing backends.

## Reproduce the internal trace

Run `./scripts/prepare-wgpu-init-overlay.ps1` after Cargo has fetched the pinned sources. It copies cached sources into a new ignored target directory and applies the exact recipe; it never edits registry sources or existing overlays. Temporarily add Cargo `[patch.crates-io]` entries for both `wgpu-hal` and `wgpu-core`, pointing at `target/perf/wgpu-device-init/overlay/<crate>`, build release and save a copy of the executable. Save and restore the original manifest/lockfile bytes before rebuilding the stock application. Local dependency builds may expose existing upstream warnings suppressed for registry builds. Run the saved measurement binary with the existing first-frame script and `TERMINAL_STARTUP_DIAGNOSTICS=1` (the harness sets this). Keep logs scoped to initialization; internal command-signature scopes can also fire during pipeline creation. Never publish the patched dependency build as production.
