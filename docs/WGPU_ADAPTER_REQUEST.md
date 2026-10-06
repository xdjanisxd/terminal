# Wgpu adapter request on Windows x86_64

Branch: `perf/wgpu-adapter-request`. Investigation completed 2026-10-06; no commit or push.

## Result

DX12 candidate native-device creation dominates. The Intel adapter's D3D12CreateDevice call takes a 31.365 ms median within a 36.192 ms internal request. An unused Microsoft software candidate costs 3.296 ms to create its device and 0.713 ms to destroy the entire unused candidate (0.702 ms inside the native-device reference release). Feature/capability queries and packaging are small. There is no justified low-risk application optimization: retain only this report, measurements, completed plan, and opt-in temporary dependency diagnostic recipe/script. Renderer code, backend policy, descriptor budget (16,384), manifests and lockfile are unchanged.

Move to shutdown reliability. A separate wgpu task would be justified only with upstream support for lightweight/early candidate selection or a controlled driver/ETW investigation of D3D12CreateDevice. The roughly 4 ms software-candidate creation/destruction is a potential upstream opportunity, not evidence that an application shortcut is safe. A backend switch is outside this task.

## Exact path and ownership

1. `crates/renderer/src/lib.rs:629` initialize_gpu builds InstanceDescriptor::from_env_or_default and checks Backends::from_env. On Windows without an override, gpu_backend_attempts (line 597) prepares DX12-only, then the original descriptor as a fallback. The second descriptor is just data: its instance/backends are not initialized after first-attempt success.
2. Instance::new and create_surface precede request_adapter. Request options are default power preference (None), force_fallback_adapter=false, compatible_surface=Some(surface). Native wgpu does synchronous enumeration/selection inside the request call; polling its ready future is negligible.
3. Pinned wgpu 25.0.2 forwards through wgpu-core's Instance::request_adapter (`src/instance.rs:423`). It enumerates each active backend, filters candidates for surface support, performs preference/metadata selection, takes the first adapter, destroys the unused Vec iterator, then wraps the selected HAL adapter. With None preference, enumeration order is preserved.
4. wgpu-hal DX12 Instance::enumerate_adapters (`src/dx12/instance.rs:138`) obtains raw DXGI adapters, exposes every candidate with Adapter::expose, then collects. Adapter::expose (`src/dx12/adapter.rs:53`) calls D3D12CreateDevice at feature level 11.0 before querying feature levels, description/architecture/driver metadata, D3D12 options, formats, limits and features. It returns the HAL adapter plus packaged capabilities. Surface compatibility checks happen after both candidates have been exposed.
5. Both Intel Iris Xe (integrated GPU) and Microsoft Basic Render Driver (CPU) were exposed in every one of the 15 buffered requests. Thus two native D3D12 devices were created before selection. Intel was selected in every launch. The unused software HAL adapter and its native device reference were destroyed before request_adapter returned. The Adapter Drop body alone does not cover field destruction; the overlay additionally times the actual owned ID3D12Device reference release.
6. The later Adapter::open (HAL adapter.rs:630) clones the selected adapter's existing device reference and creates the command queue and wgpu Device resources. It does not call D3D12CreateDevice again. Final selected-device shutdown is outside this request breakdown.

Successful requests used one backend enumeration and application attempt=0 only. No Vulkan/GL/fallback backend was probed. On a surface/adapter/device failure, the existing second attempt uses the original descriptor and may probe DX12 again alongside other enabled backends. Eliminating that retry or filtering CPU candidates would change fallback semantics and is not justified by successful-startup measurements. Explicit WGPU_BACKEND overrides continue to use one descriptor; the explicit dx12 smoke records override=true and selects Intel DX12. Invalid/other override handling remains wgpu-owned and unchanged.

The public wgpu enumerate_adapters API still exposes candidates; it cannot enumerate raw DXGI metadata and cheaply instantiate only the final candidate. Unsafe HAL interop would move selection/lifetime responsibilities into the app. No such change was made.

## Timings

15 sequential fresh-process Windows x86_64 release launches per cohort; unchanged caches, no concurrent build/tests. Windows 11 Pro build 26300, Core i5-1250P, Intel Iris Xe driver 32.0.101.7088; software adapter driver 10.0.26100.9549. Full environment hashes and rustc identity are retained alongside samples.

Buffered internal timings, milliseconds. Ordinal #1 is Intel and #2 is software for candidate stages; ordinal identifies distinct recorded occurrences rather than a backend ID. Inclusive/nested scopes overlap: do not sum every row or add medians as if paired.

| Scope / occurrence | Median ms | Min–max ms |
| --- | ---: | ---: |
| dxgi-enumerate#1 | 0.256 | 0.225–0.373 |
| native-device-create#1 | 31.365 | 28.721–92.431 |
| feature-queries-and-packaging#1 | 0.289 | 0.232–0.458 |
| candidate-expose#1 | 31.622 | 28.972–92.768 |
| native-device-create#2 | 3.296 | 2.586–4.045 |
| feature-queries-and-packaging#2 | 0.077 | 0.058–0.093 |
| candidate-expose#2 | 3.372 | 2.649–4.147 |
| dx12-enumeration-and-exposure#1 | 35.446 | 32.736–95.657 |
| adapter-drop-body#1 | 0.000 | 0.000–0.000 |
| native-device-release#1 | 0.702 | 0.387–1.286 |
| backend-enumerate-and-expose#1 | 35.462 | 32.751–95.685 |
| candidate-surface-check#1 | 0.006 | 0.006–0.009 |
| candidate-surface-check#2 | 0.000 | 0.000–0.001 |
| selection-metadata#1 | 0.000 | 0.000–0.000 |
| unused-candidates-destroy#1 | 0.713 | 0.400–1.301 |
| selected-adapter-wrap#1 | 0.002 | 0.001–0.005 |
| request-total#1 | 36.192 | 33.544–96.129 |

A per-launch nonoverlapping partition is DXGI enumeration + candidate exposure #1 + exposure #2 + both surface checks + selection metadata + unused-candidate destruction + selected wrap + residual. Residual median is 0.051 ms (0.044–0.073 ms): backend/core iteration, recording and other glue. Each candidate exposure contains native-device creation and feature-query/packaging timings. The latter starts immediately after successful native creation and includes feature-level/metadata/capability queries and packaging; no finer API attribution is necessary to establish the dominant call. Native creation remains a black box for driver/runtime work: this investigation does not attribute its internal stacks.

| Stock/diagnostic cohort (15 each) | Adapter request median ms | Device request median ms | First present median ms |
| --- | ---: | ---: | ---: |
| Initial stock | 36.506 | 26.263 | 145.552 |
| Buffered diagnostic executable, app inclusive | 59.549 | 26.210 | 167.069 |
| Final stock, normal dependencies | 34.394 | 25.756 | 140.454 |

There is no before/after optimization claim. Initial/final stock are identical production behavior; their difference is sequential batch/cache/system drift. Historical descriptor-budget results (device 51.7→31.2 ms; first present 213.6→181.5 ms) came from the previous task and are not a matched baseline for this run. The first initial-stock launch spent 8.037 s in font discovery and presented at 8.956 s; it remains included, as do all other outliers. Stock first-present ranges are 140.872–8956.483 ms initial and 134.351–181.609 ms final.

Diagnostics perturb startup. Immediate per-scope stderr output was rejected after it inflated candidate/query scopes substantially. The retained recipe buffers records in memory during the request, captures request-total, then writes them to redirected stderr. It prevents I/O inside measured internal stages, but printing still delays the application-inclusive request and first frame (about 23 ms in this cohort). Buffered allocation/locking/formatting remains a small uncalibrated internal overhead. Internal 36.192 ms versus stock adapter medians 36.506/34.394 ms supports the attribution; it does not establish exact zero-overhead production costs. HAL and core buffers are flushed separately, so textual interleaving is not chronological across packages. Ordinals remain in-order within each scope. No production overlay is installed.

Every measured cohort reached first presentation and rendered shell output, exited normally with code 0, and needed no forced cleanup. This does not replace a shutdown reliability investigation under stress.

## Retained evidence and reproduction

- [Completed goal and plan](WGPU_ADAPTER_REQUEST_PLAN.md)
- [Pinned overlay recipe](perf/wgpu-adapter-request/native-overlay.json) and [preparation script](../scripts/prepare-wgpu-adapter-overlay.ps1)
- [Internal per-launch durations](perf/wgpu-adapter-request/native-costs.csv) and [buffered native traces](perf/wgpu-adapter-request/native-traces.txt)
- `docs/perf/wgpu-adapter-request/{stock,buffered,final-stock}-{samples.csv,costs.csv,environment.json}` retain all application samples/environment data.

Use a fresh output directory; the preparation script rejects overwrite and anything outside repository target. Registry sources are copied, never edited. Build patching is command-local, never placed in Cargo.toml or user Cargo config. Save Cargo.lock before the patched build and restore it afterward, including on failure.

```powershell
cargo build --release -q -p terminal-app
# Save/measure target/release/terminal.exe as the stock control before overlay build.
./scripts/prepare-wgpu-adapter-overlay.ps1
$measurementRoot = Join-Path $PWD 'target/perf/wgpu-adapter-request'
Copy-Item -LiteralPath Cargo.lock -Destination (Join-Path $measurementRoot 'saved-Cargo.lock')
$overlayPath = (Join-Path $measurementRoot 'overlay').Replace('\','/')
$patchConfig = Join-Path $measurementRoot 'patch.toml'
@"
[patch.crates-io]
wgpu-hal = { path = '$overlayPath/wgpu-hal' }
wgpu-core = { path = '$overlayPath/wgpu-core' }
"@ | Set-Content -LiteralPath $patchConfig
try {
    cargo --config $patchConfig build --release -q -p terminal-app
    if ($LASTEXITCODE -ne 0) { throw 'Diagnostic build failed' }
    Copy-Item -LiteralPath target/release/terminal.exe -Destination (Join-Path $measurementRoot 'diagnostic.exe')
} finally {
    Copy-Item -LiteralPath (Join-Path $measurementRoot 'saved-Cargo.lock') -Destination Cargo.lock
}
./scripts/measure-first-frame.ps1 -Runs 15 -Executable (Join-Path $measurementRoot 'diagnostic.exe') -OutputDirectory (Join-Path $measurementRoot 'measurements')
# Restore the normal executable too, before native correctness checks/delivery.
cargo build --release -q -p terminal-app
```

TERMINAL_STARTUP_DIAGNOSTICS=1 enables overlay timings (the launch harness sets it). Without it no timing records are emitted. The copied HAL device wrapper preserves field destruction order and times release of its owned native reference; the core rewrite explicitly destroys the unused iterator at the same point as the original temporary. These are diagnostic changes only. No backend, adapter preference, features or limits are modified.

## Validation and limits

Both stock native release DX12 tests passed: native_gpu_validates_terminal_pipelines and native_gpu_renders_backgrounds_glyphs_underlines_and_cursor. Explicit DX12 override smoke passed and normal release dependency build was restored. Full repository gates passed: cargo fmt --all -- --check; cargo check --workspace --all-targets; cargo test --workspace --all-targets (672 passed, five existing ignored); cargo test --workspace --doc (four doctests passed); cargo clippy --workspace --all-targets -- -D warnings; git diff --check. Logs are under target/perf/wgpu-adapter-request/validation. Script syntax parsing passed, and a freshly prepared retained recipe reproduced all four measured source files after newline normalization. Dependency audit confirms Cargo.toml/Cargo.lock unchanged and the stock release binary rebuilt.

No hardware-failure fallback or new interactive visual/input acceptance was exercised. Existing backend policy tests cover descriptor ordering/override preservation; production source is unchanged. Measurements cover one host/driver/cache sequence and presented-frame markers, not scanout. Earlier immediate-output experiment and the stale package-named executable failed launch are excluded from retained cohorts; neither is used for attribution.
