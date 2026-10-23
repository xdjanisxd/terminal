# Linux concurrent Vulkan GPU validation

## Current result (2026-10-23)

The crash is an **external Vulkan-loader concurrency failure**, with the
strongest supported mechanism being the old loader's inconsistent locking of
its global instance/ICD/device lists during debug-object naming. No terminal
renderer ownership defect or shared test GPU state was found. The exact racing
write was not instrumented locally, so this is not a claim that every possible
renderer race has been disproved.

The unchanged baseline executable crashed in **3/3 concurrent runs** with Debian's
Vulkan loader 1.4.309. It passed **3/3 concurrent runs** with an independently built
upstream loader 1.4.345 selected only through `LD_LIBRARY_PATH`. The independent
safe-wgpu probe shows the same failure and also passes with the newer loader.
No system library, Cargo dependency, or production renderer behavior was changed.

This matches the maintainer's diagnosis in
[Vulkan-Loader #1863](https://github.com/KhronosGroup/Vulkan-Loader/issues/1863)
and the locking changes in
[Vulkan-Loader #1866](https://github.com/KhronosGroup/Vulkan-Loader/pull/1866).
The upstream change replaces the separate global-instance-list lock with a
consistent recursive global lock and adds threaded debug-naming tests. Our A/B
comparison tests release versions, not that single patch in isolation.

**Linux GPU tests now hold one test-only mutex for each complete test lifetime.**
All seven `native_gpu_*` tests and the opt-in `renderer_workload_baselines` test
use it before creating an instance and retain it through resource destruction.
Every rendering assertion, format/opacity/DPI case, label, debug flag and workload
remains unchanged. Existing opt-in tests remain opt-in; no new ignore or skip was
added. A failed assertion still fails its test; poison recovery lets independent
later tests run. CPU tests remain parallel.

The guard is compiled only with `cfg(all(test, target_os = "linux"))`. It adds
**zero production runtime synchronization, polling, threads, timers or GPU waits**.
Windows/macOS-specific code is unchanged. Test execution takes serial wall time;
no application performance improvement or regression is claimed.

## Baseline and affected environment

- Branch: `fix/gpu-concurrent-validation`, initially clean and already at fetched
  `origin/master`, `ae0f50dcaca1b06d5e84725435bbec8ea8382488`.
- Debian Linux x86_64, kernel `6.12.113+deb13-amd64`, KDE Wayland, 16 logical CPUs.
- Intel Iris Xe (ADL GT2), vendor/device `8086:46a6`, Vulkan backend; Mesa
  `25.0.7-2+deb13u1`, system `libvulkan1 1.4.309.0-1`.
- wgpu/wgpu-core/wgpu-hal 25.0.2, ash 0.38.0+1.3.281, Rust 1.99.0
  (`b940084d7`, LLVM 23.1.1); debug test builds.
- Default layer: `VK_LAYER_MESA_device_select`; Intel nullhw and Mesa overlay
  manifests are available but were not requested. Default driver discovery sees
  Intel, llvmpipe and additional ICD manifests without matching hardware.
- No pre-existing WGPU/Vulkan/test-thread overrides. Baseline runs have no
  Khronos validation layer installed. A matching Debian validation-layer package
  was later downloaded and unpacked under `/tmp`, without installing it.
- Fontconfig includes system fonts plus licensed repository JetBrains Mono
  fixtures; system Noto Color Emoji supplies emoji. Font prerequisites were met
  for every GPU comparison.
- The sandbox exposes only llvmpipe. Native results below used host GPU access;
  one successful sandbox serial run is recorded separately and is not native
  Intel evidence.

[Raw evidence and run records](measurements/gpu-concurrent-validation/) include
adapter details, commands, statuses, timings, GDB stacks and hashes. Baseline
executable SHA-256:
`872899b04bf78db4bb3ea8526caaf404984787554dda2ac9ed21d5681e369267`.
A copy remains at `/tmp/gpu-concurrent-validation/renderer-baseline`; the target
executable was subsequently rebuilt with the guard.

## Controlled comparisons

All GPU runs completed within a 90-second per-process bound. A return code of
`-11` in `results.json` is Python's representation of SIGSEGV (shell status 139),
not a timeout. No GPU comparison required omitting a failing assertion.

| Configuration | Runs | Result |
| --- | ---: | --- |
| Baseline, system loader, `--test-threads=1` | 2 | 7/7 passed each |
| Baseline, system loader, `--test-threads=7` | 3 | SIGSEGV each |
| Baseline, Mesa selection layer disabled only | 1 | SIGSEGV |
| Baseline, Intel ICD selected only | 3 | 7/7 passed each |
| Historical standalone wgpu probe, default discovery | 1 | SIGSEGV |
| Historical probe, selection layer disabled only | 1 | SIGSEGV |
| Historical probe, Intel ICD selected only | 1 | Passed |
| Historical probe, Intel ICD plus no selection layer | 1 | Passed; local raw log retained |
| Vulkan-only probe, one / seven workers | 1 each | Passed / SIGSEGV |
| Unchanged baseline, isolated loader 1.4.345, seven threads | 3 | 7/7 passed each |
| Vulkan-only probe, loader 1.4.345, seven workers | 1 | Passed; loaded library path captured |
| Guarded tests, system loader, one thread | 2 | 7/7 passed each |
| Guarded tests, system loader, seven libtest threads | 5 | 7/7 passed each; GPU work serialized |
| Guarded tests plus workload benchmark, eight libtest threads | 1 | 8/8 passed |
| Same eight tests with Khronos validation 1.4.309 | 1 | 8/8 passed; no reported VUID diagnostics |

Disabling `VK_LAYER_MESA_device_select` alone does not prevent the crash. Selecting
only Intel avoids it in these samples, which supports a loader enumeration/list
interaction rather than a demonstrated Intel shader/rendering defect. Neither
observation establishes universal safety for another driver or concurrency pattern.
The newer-loader runs retain actual concurrent GPU validation independently of
the new guard. The standalone probe remains unguarded for future upstream checks.

## Actionable crash evidence

The baseline GDB run faults on LWP 3247638 in
`native_gpu_background_opacity_and_srgb_premultiplication`, creating a named
`DescriptorSetLayout` from `DrawResources::new` at opacity 0.5:

```text
libvulkan.so.1 + 0x2d03c
libvulkan.so.1 + 0x2d408
libvulkan.so.1 + 0x18ea7
ash::...::debug_utils::Device::set_debug_utils_object_name
wgpu_hal::vulkan::DeviceShared::set_object_name<DescriptorSetLayout>
wgpu_hal::vulkan::device::create_bind_group_layout
terminal_renderer::gpu::DrawResources::new
render_native_frame_with_overlay -> native_gpu_background_opacity_and_srgb_premultiplication
```

The faulting instruction dereferences `rdx = 1`. Disassembly and matching upstream
1.4.309 source identify the path as `loader_get_icd_and_device`, which scans global
instance/ICD/device lists. Simultaneously, LWP 3247640 is in `dlclose` beneath
`vkEnumeratePhysicalDevices` / wgpu adapter enumeration. Loader tracing records
removal of unused Intel hasvk, virtio, gfxstream, nouveau and Radeon ICDs.
The loader's `unload_drivers_without_physical_devices` mutates the ICD list while
the naming traversal uses a different lock. This is a credible local mechanism,
corroborated by upstream's reported synchronization defect; no local TSAN or
write watchpoint establishes which single mutation won this race.

The independent probe faults at loader `+0x2d066 -> +0x2d408 -> +0x18ea7`, while
creating a named buffer. Other probe threads include concurrent `vkCreateDevice`.
It links only std, wgpu and pollster, with no terminal/font/atlas/PTY code or unsafe
Rust. The controlled Vulkan-only variant accepts a worker count, uses four cycles
and 64 named buffer/encoder/submission operations per cycle, and waits for device
completion. All worker panics propagate through thread joins.

wgpu's debug-build `InstanceFlags::default()` enables DEBUG and VALIDATION. DEBUG
requests `VK_EXT_debug_utils`, causing resource labels to reach the loader.
VALIDATION requests the Khronos layer only when available. Tests construct
independent instances/devices via safe APIs; helper calls recreate them between
format/opacity cases. No project-owned raw HAL operations, global GPU resources,
or unsafe renderer blocks were found. Disabling debug naming or validation was
not used as a fix.

## Reproduce and validate

Run from the repository root. The captured fontconfig file contains this
checkout's absolute fixture path; regenerate it if the checkout moves:

```sh
mkdir -p /tmp/gpu-concurrent-validation
cat > /tmp/gpu-concurrent-validation/fonts.conf <<EOF
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <include>/etc/fonts/fonts.conf</include>
  <dir>$PWD/crates/renderer/tests/fixtures</dir>
  <cachedir>/tmp/gpu-concurrent-validation/font-cache</cachedir>
</fontconfig>
EOF
cargo test -p terminal-renderer --lib --offline --no-run
FONTCONFIG_FILE=/tmp/gpu-concurrent-validation/fonts.conf RUST_BACKTRACE=1 \
  timeout 90s target/debug/deps/terminal_renderer-5c29bd990de9e307 \
  native_gpu_ --ignored --nocapture --test-threads=1
FONTCONFIG_FILE=/tmp/gpu-concurrent-validation/fonts.conf RUST_BACKTRACE=1 \
  timeout 90s target/debug/deps/terminal_renderer-5c29bd990de9e307 \
  native_gpu_ renderer_workload_baselines --ignored --nocapture --test-threads=8
```

`--offline` uses cached Cargo dependencies. `--no-run` builds and prints the test
executable; use that printed filename if its hash differs. `--ignored` selects
existing opt-in tests; both positional filters select all eight GPU entry points.
`--nocapture` retains adapter/readback diagnostics. `--test-threads=1` is the
serial control; `8` tests the guarded harness under contention. `RUST_BACKTRACE=1`
records Rust panic stacks; native faults require GDB. `timeout 90s` bounds a run.
Do not interpret these guarded runs as proof of raw in-process Vulkan concurrency.

To reproduce the original suite locally, substitute the preserved baseline copy
and use only `native_gpu_ --ignored --nocapture --test-threads=7`. On a fresh
checkout, build baseline commit `ae0f50d` separately; do not remove the guard or
reset unrelated work to manufacture a baseline.

The standalone Vulkan-only reproduction remains independent of the guarded tests:

```sh
rustc --edition=2024 -g \
  docs/measurements/gpu-concurrent-validation/vulkan_probe.rs \
  --extern wgpu=target/debug/deps/libwgpu-933b4afee7131c3a.rlib \
  --extern pollster=target/debug/deps/libpollster-541c4a4a3b83889d.rlib \
  -L dependency=target/debug/deps -o /tmp/gpu-concurrent-validation/vulkan-probe
timeout 90s /tmp/gpu-concurrent-validation/vulkan-probe 1
timeout 90s /tmp/gpu-concurrent-validation/vulkan-probe 7
NODEVICE_SELECT=1 timeout 90s /tmp/gpu-concurrent-validation/vulkan-probe 7
VK_DRIVER_FILES=/usr/share/vulkan/icd.d/intel_icd.json \
  timeout 90s /tmp/gpu-concurrent-validation/vulkan-probe 7
```

`-g` enables source-level GDB frames. `--extern` and `-L dependency` reuse existing
artifacts, whose filenames are checkout-specific. `1`/`7` set worker counts.
`NODEVICE_SELECT=1` disables the installed Mesa selection layer, as specified in
its manifest. `VK_DRIVER_FILES` restricts ICD discovery; it is a diagnostic
comparison, not the default test policy. Tests using `InstanceDescriptor::default`
do not read `WGPU_BACKEND`; setting it alone does not force their backend. The
probe explicitly selects Vulkan and prints each adapter.

For native stacks, replace the executable and filters as needed:

```sh
FONTCONFIG_FILE=/tmp/gpu-concurrent-validation/fonts.conf timeout 90s \
  gdb -q -batch -ex 'set pagination off' -ex 'set print thread-events off' \
  -ex run -ex 'bt 35' -ex 'info registers' -ex 'x/12i $pc-24' \
  -ex 'thread apply all bt 12' \
  --args /tmp/gpu-concurrent-validation/renderer-baseline \
  native_gpu_ --ignored --nocapture --test-threads=7
VK_LOADER_DEBUG=error,warn,layer,driver \
  timeout 90s /tmp/gpu-concurrent-validation/vulkan-probe 7
```

GDB preserves the faulting thread plus peer stacks; `VK_LOADER_DEBUG` records
layer insertion and ICD discovery/removal. Direct debugger runs report signal
status in GDB output rather than propagating the child SIGSEGV exit code.

The [measurement README](measurements/gpu-concurrent-validation/README.md) records
the exact isolated-loader build and temporary validation-layer setup.

## Final validation and Linux acceptance

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --all-targets --offline`: passed.
- `cargo test --workspace --all-targets --offline`: passed, 895 tests across
  75 test binaries, 31 existing opt-in tests ignored; native GPU runs above are
  additional and explicit.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo build --release --offline`: passed.
- All eight GPU entry points passed together, including the workload benchmark.
  These debug workload times are diagnostics, not release performance measurements.
- The focused Khronos-layer run passed all eight GPU tests. Loader logs prove
  layer insertion, with direct stdout warning/error logging configured and zero
  reported VUIDs. This does not prove absence of every Vulkan misuse.
- An initially overbroad `--ignored` run selected 17 tests: 16 passed, including
  all GPU tests; `native_monospace_control_fonts_report_complete_line_metrics`
  failed because none of Cascadia Mono, Consolas or JetBrains Mono Nerd Font was
  installed. No test was changed or silently skipped to hide this prerequisite.
- Existing native Wayland acceptance passed with `WGPU_BACKEND=vulkan`, including
  opacity/blur reload, selection, palette, surface/window recreation and shutdown.
  A release terminal also presented truecolor, bold and emoji output and exited
  normally after its command. Adapter diagnostics confirm Intel Vulkan. Screenshots
  were visually inspected; full-desktop PNGs remain under `/tmp`, outside Git.

Linux manual acceptance checklist for another machine:

- [x] Confirm native Intel Vulkan adapter rather than llvmpipe in this investigation.
- [x] Present terminal text, truecolor, bold and emoji using the release binary.
- [x] Inspect selection, palette and recreated-window screenshots on KDE Wayland.
- [x] Confirm surface/window recreation and normal shutdown without a GPU crash.
- [ ] Repeat interactive resize/minimize/restore, DPI changes and editor use on the target machine.
- [ ] Repeat on other Linux GPUs/loaders before claiming general portability.
- [ ] Recheck raw concurrency with the standalone probe after the system loader changes.

The guard is an in-process validation strategy, not a system Vulkan repair. It
does not certify concurrent native instance/device lifecycles in production on
old loaders, synchronize other test executables, or establish long-running GPU
stability. Native X11, other Linux drivers, GPU-assisted/synchronization validation,
TSAN and all Windows/macOS execution were outside this Linux investigation.
No commits or pushes were made.

---

## Historical investigation (2026-10-21)

The following records the earlier evidence and decisions before the 2026-10-23
loader A/B comparison and test-only isolation above.

Investigated on 2026-10-21 on `fix/resource-cleanup-memory-safety`, continuing
the [resource cleanup audit](RESOURCE_CLEANUP_MEMORY_SAFETY.md). Existing code and
uncommitted changes were preserved. This follow-up changes documentation and
diagnostic artifacts only; no production/test behavior or dependencies changed.

### Result and closure recommendation

**The original SIGSEGV reproduced. The PTY Drop change is not implicated.**
A standalone safe-wgpu program, with no terminal, font, glyph atlas, or PTY code,
also crashed in the same Vulkan debug-object naming path. This isolates a
failure in the installed upstream graphics stack under concurrent independent
wgpu instances, without requiring project resource ownership code.

The faulting instruction is in the Vulkan loader (`libvulkan.so.1`), not directly
inside Mesa's Intel ICD. The exact root cause within the loader, ICD/implicit
layers, or wgpu integration is **not established**. Calling this a proven Mesa
driver bug would overstate the evidence. No project-owned defect was identified,
so no speculative renderer workaround, global test lock, forced wait/eviction,
dependency update, or new project regression test was added.

It is reasonable to close the **scoped PTY cleanup change** with this documented
external native-validation limitation. Concurrent native GPU validation remains
failing on this machine and must not be marked passed. This does not certify
all long-running stability: the audit's separate blocked-input shutdown finding
and Windows/macOS acceptance limits also remain open.

### Original crash and environment

The original process was PID 2136324, at 2026-10-21 12:18:46 +03. Systemd retained
its core, and GDB resolved the faulting thread, LWP 2136327, to:

```text
libvulkan.so.1 + 0x2d066
libvulkan.so.1 + 0x2d408
libvulkan.so.1 + 0x18ea7
ash::...::debug_utils::Device::set_debug_utils_object_name
wgpu_hal::vulkan::DeviceShared::set_object_name<CommandBuffer>
wgpu_hal::vulkan::command::begin_encoding
wgpu_core::...::Queue::submit
wgpu::Queue::submit
render_native_frame_with_overlay(opacity=0.5, palette=false, hints=false)
  crates/renderer/src/gpu.rs:2166
native_gpu_background_opacity_and_srgb_premultiplication
  crates/renderer/src/gpu.rs:1674
```

The separate `RequestedFamilyUnavailable("JetBrains Mono")` panic was in the
atlas weight test; it was not the SIGSEGV thread. Interleaved stdout/stderr alone
had not identified the faulting test. The font-complete concurrent reproduction
below establishes that this panic is not required to trigger the SIGSEGV.

Environment inspected locally:

- Debian Linux x86_64, kernel `6.12.113+deb13-amd64`, Wayland session; tests are
  offscreen and create no application window.
- Intel Iris Xe Graphics (ADL GT2), selected Vulkan backend; Mesa packages
  `25.0.7-2+deb13u1` and Vulkan loader package `libvulkan1 1.4.309.0-1`.
- `wgpu`, `wgpu-core`, `wgpu-hal` 25.0.2; ash 0.38.0+1.3.281; Rust 1.99.0
  (`b940084d7`, LLVM 23.1.1), debug test build.
- Native test binary `terminal_renderer-5c29bd990de9e307`, unchanged SHA-256
  `872899b04bf78db4bb3ea8526caaf404984787554dda2ac9ed21d5681e369267`.
- Default concurrent libtest execution, seven selected tests, 16 available CPUs.
  Current environment has no `RUST_TEST_THREADS`, `WGPU_BACKEND`, `WGPU_DEBUG`,
  `WGPU_VALIDATION`, `VK_ICD_FILENAMES`, `VK_DRIVER_FILES`, `VK_INSTANCE_LAYERS`,
  `VK_LAYER_PATH`, or `MESA_VK_DEVICE_SELECT` overrides. Original environment
  variables were not archived; the original command, modules and adapter were.
- The original core loaded the Mesa device-select layer and multiple ICDs as
  well as the selected Intel driver. No Khronos validation layer is installed;
  absence of validation messages is not proof of Vulkan correctness.

### Bounded reproduction and crash records

Only four new GPU executions were made, each with a 45-second external timeout.
All completed or crashed before that limit; status 139 means SIGSEGV, not timeout.

| Run | Result | Faulting test / path |
| --- | --- | --- |
| Original configuration, default fonts, concurrent | SIGSEGV, status 139, PID 2149936 | Palette test; naming an upload staging `Buffer`, loader +0x2d03c -> +0x2d408 -> +0x18ea7 |
| Font prerequisite satisfied, serial | 7 passed, status 0 | All native GPU tests, including atlas and emoji |
| Font prerequisite satisfied, concurrent | SIGSEGV, status 139, PID 2150514 | Emoji DPI/warm-cache test; naming a `CommandBuffer`, loader +0x2d05a -> +0x2d408 -> +0x18ea7 |
| Standalone wgpu concurrency probe | SIGSEGV, status 139 | Creating a named `Buffer`; same loader +0x2d05a -> +0x2d408 -> +0x18ea7 path |

An older journal/core record from **2026-10-19 17:12:14 +03**, PID 207923, also
shows the background-opacity test crashing at loader +0x2d066 -> +0x2d408 ->
+0x18ea7 while naming a `DescriptorSetLayout`. It predates the PTY Drop edit.
Its older test binary differs, so this is corroborating historical evidence,
not a controlled before/after measurement.

The renderer dependency graph (`cargo tree -p terminal-renderer --edges normal,dev`)
contains no `terminal-pty`; the changed destructor cannot execute in this binary.
Renderer sources also contain no project-owned unsafe/raw-HAL handle operations.
Tests use independent local instances/devices/resources through safe wgpu APIs.
The inspected wgpu HAL retains its instance through `Arc<InstanceShared>` in
`DeviceShared`. No missing project-owned lifetime retention was found at the
crashing creation/submission sites. These observations plus the independent
probe exclude the PTY change and show that project rendering resources are not
necessary to trigger this failure; they do not prove all renderer lifetimes.

The standalone probe creates seven threads, each with four bounded cycles of
independent instance/device creation, 64 named buffers/encoders/submissions, and
an explicit device completion wait. It uses only std, wgpu and pollster, with
no unsafe code. GDB places its crash at its own `Device::create_buffer` call,
then wgpu HAL's object naming and the same loader path. No terminal crate is
linked into the probe.

### Reproduction commands and retained evidence

The [measurement directory](measurements/resource-cleanup-memory-safety/gpu-investigation/)
contains each run's raw output, focused GDB/journal stack excerpts, and the
standalone probe source. Full local GDB/core metadata remains under
`/tmp/resource-cleanup-audit/gpu-investigation/`; systemd retains the cores.
Core files themselves are not added to the repository.

The exact existing test executable was reused rather than rebuilt between runs:

```sh
RUST_BACKTRACE=1 timeout 45s target/debug/deps/terminal_renderer-5c29bd990de9e307 \
  native_gpu_ --ignored --nocapture
FONTCONFIG_FILE=/tmp/resource-cleanup-audit/fonts.conf RUST_BACKTRACE=1 \
  timeout 45s target/debug/deps/terminal_renderer-5c29bd990de9e307 \
  native_gpu_ --ignored --nocapture --test-threads=1
FONTCONFIG_FILE=/tmp/resource-cleanup-audit/fonts.conf RUST_BACKTRACE=1 \
  timeout 45s target/debug/deps/terminal_renderer-5c29bd990de9e307 \
  native_gpu_ --ignored --nocapture
```

`--ignored` selects opt-in GPU tests, `--nocapture` preserves diagnostics,
`--test-threads=1` is the serial control, and `RUST_BACKTRACE=1` reveals ordinary
Rust panics; native SIGSEGV frames come from the core/GDB. `timeout 45s` bounds
each attempt without changing libtest concurrency. The isolated fontconfig file
includes `/etc/fonts/fonts.conf` and the repository's `crates/renderer/tests/fixtures`
directory containing the licensed JetBrains Mono fixture.

The standalone probe was compiled against existing artifacts without fetching
or changing dependencies:

```sh
rustc --edition=2024 -g \
  docs/measurements/resource-cleanup-memory-safety/gpu-investigation/wgpu_concurrency_probe.rs \
  --extern wgpu=target/debug/deps/libwgpu-933b4afee7131c3a.rlib \
  --extern pollster=target/debug/deps/libpollster-541c4a4a3b83889d.rlib \
  -L dependency=target/debug/deps -o /tmp/wgpu-concurrency-probe
timeout 45s /tmp/wgpu-concurrency-probe
```

Artifact hashes/paths are specific to this checkout. `--extern` selects the
already installed dependency artifacts, `-L dependency` resolves their existing
dependencies, and `-g` permits resolving probe source locations in GDB.

No repeated full workspace suite or release build was warranted for this
documentation-only follow-up. Link and whitespace checks were run. No other
driver, ICD, backend, dependency version, Windows/macOS system, or production
multi-window scenario was tested. Serial execution is an observed validation
workaround, not a production fix or evidence that concurrent use is safe.
