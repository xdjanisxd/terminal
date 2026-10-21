# Concurrent native GPU SIGSEGV investigation

Investigated on 2026-10-21 on `fix/resource-cleanup-memory-safety`, continuing
the [resource cleanup audit](RESOURCE_CLEANUP_MEMORY_SAFETY.md). Existing code and
uncommitted changes were preserved. This follow-up changes documentation and
diagnostic artifacts only; no production/test behavior or dependencies changed.

## Result and closure recommendation

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

## Original crash and environment

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

## Bounded reproduction and crash records

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

## Reproduction commands and retained evidence

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
