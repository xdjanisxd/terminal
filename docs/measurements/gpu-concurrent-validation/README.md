# Linux Vulkan concurrency evidence, 2026-10-23

See [the investigation](../../GPU_CONCURRENT_VALIDATION.md) for conclusions,
coverage boundaries and the Linux acceptance checklist. `results.json` retains
exact executable arguments, diagnostic environment overrides, statuses and elapsed
times. Negative return code `-11` means SIGSEGV. Timings are not benchmarks.

The baseline executable was copied before rebuilding the guarded tests. Its hash
and the two loader hashes are in `binary-hashes.txt`. GDB files are raw debugger
output. Per-run logs preserve test assertions and adapter identity. Loader/layer
excerpts identify their full raw logs under `/tmp/gpu-concurrent-validation/`;
full cores, source archives, build trees and desktop screenshots are not in Git.
The broad ignored-test prerequisite failure is retained explicitly.

## Isolated upstream loader comparison

These commands built the comparison loader without installing anything or
changing Cargo dependencies. Downloads came from official tagged upstream
repositories; archive checksums are retained here.

```sh
curl -fL https://github.com/KhronosGroup/Vulkan-Loader/archive/refs/tags/v1.4.345.tar.gz \
  -o /tmp/gpu-concurrent-validation/loader-1.4.345.tar.gz
curl -fL https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v1.4.345.tar.gz \
  -o /tmp/gpu-concurrent-validation/headers-1.4.345.tar.gz
tar -xzf /tmp/gpu-concurrent-validation/loader-1.4.345.tar.gz -C /tmp/gpu-concurrent-validation
tar -xzf /tmp/gpu-concurrent-validation/headers-1.4.345.tar.gz -C /tmp/gpu-concurrent-validation
cmake -S /tmp/gpu-concurrent-validation/Vulkan-Headers-1.4.345 \
  -B /tmp/gpu-concurrent-validation/headers-build \
  -DCMAKE_INSTALL_PREFIX=/tmp/gpu-concurrent-validation/headers-install
cmake --install /tmp/gpu-concurrent-validation/headers-build
cmake -S /tmp/gpu-concurrent-validation/Vulkan-Loader-1.4.345 \
  -B /tmp/gpu-concurrent-validation/loader-build \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo \
  -DCMAKE_PREFIX_PATH=/tmp/gpu-concurrent-validation/headers-install \
  -DBUILD_TESTS=OFF -DBUILD_WSI_XCB_SUPPORT=OFF \
  -DBUILD_WSI_XLIB_SUPPORT=OFF -DBUILD_WSI_XLIB_XRANDR_SUPPORT=OFF
cmake --build /tmp/gpu-concurrent-validation/loader-build -j4
LD_LIBRARY_PATH=/tmp/gpu-concurrent-validation/loader-build/loader \
  FONTCONFIG_FILE=/tmp/gpu-concurrent-validation/fonts.conf RUST_BACKTRACE=1 \
  timeout 90s /tmp/gpu-concurrent-validation/renderer-baseline \
  native_gpu_ --ignored --nocapture --test-threads=7
LD_LIBRARY_PATH=/tmp/gpu-concurrent-validation/loader-build/loader \
  timeout 90s /tmp/gpu-concurrent-validation/vulkan-probe 7
```

`-S`/`-B` select source/build directories. The install prefix stays under `/tmp`.
`CMAKE_PREFIX_PATH` supplies matching headers. `RelWithDebInfo` keeps symbols.
X11 support was disabled because development headers are absent; Wayland support
remained enabled. `BUILD_TESTS=OFF` builds the loader, not its upstream suite.
`-j4` limits build parallelism. `LD_LIBRARY_PATH` selects the temporary loader for
one process; `LD_DEBUG=libs` on the captured probe confirmed the actual loaded path
in `fixed-loader-path.txt`. This comparison does not qualify the temporary build
for system installation or test its X11 behavior.

## Temporary native validation layer

The Debian package was downloaded and extracted, not installed:

```sh
cd /tmp/gpu-concurrent-validation
apt-get download vulkan-validationlayers
dpkg-deb -x vulkan-validationlayers_1.4.309.0-1_amd64.deb validation
```

From the repository root, after building renderer tests and creating the fontconfig
file as described in the investigation:

```sh
VK_LAYER_PATH=/tmp/gpu-concurrent-validation/validation/usr/share/vulkan/explicit_layer.d \
LD_LIBRARY_PATH=/tmp/gpu-concurrent-validation/validation/usr/lib/x86_64-linux-gnu \
VK_LAYER_SETTINGS_PATH=/tmp/gpu-concurrent-validation \
VK_LOADER_DEBUG=error,warn,layer \
FONTCONFIG_FILE=/tmp/gpu-concurrent-validation/fonts.conf RUST_BACKTRACE=1 \
timeout 90s target/debug/deps/terminal_renderer-5c29bd990de9e307 \
  native_gpu_ renderer_workload_baselines --ignored --nocapture --test-threads=8
```

Copy this directory's `vk_layer_settings.txt` into `/tmp/gpu-concurrent-validation/`
first. It requests direct warning/error stdout logging, independent of Rust's log
subscriber. `VK_LAYER_PATH` finds the manifest; `LD_LIBRARY_PATH` finds its shared
library and still uses the system Vulkan loader. `VK_LAYER_SETTINGS_PATH` finds
the settings file. Loader output confirms Khronos layer insertion. No VUID was
reported; GPU-assisted and synchronization validation were not enabled.

## Native Wayland acceptance

Run the existing GUI test alone; it opens windows and can capture full desktops:

```sh
WGPU_BACKEND=vulkan FONTCONFIG_FILE=/tmp/gpu-concurrent-validation/fonts.conf \
TERMINAL_RENDERER_DIAGNOSTICS=1 TERMINAL_STARTUP_DIAGNOSTICS=1 \
TERMINAL_BLUR_TEST_SCREENSHOTS=/tmp/gpu-concurrent-validation/native-screenshots \
timeout 60s target/debug/deps/terminal-b58a4779d952aebb \
  native_wayland_blur_reload_recreation_and_shutdown \
  --ignored --nocapture --test-threads=1
TERMINAL_RENDERER_DIAGNOSTICS=1 TERMINAL_STARTUP_DIAGNOSTICS=1 \
WGPU_BACKEND=vulkan FONTCONFIG_FILE=/tmp/gpu-concurrent-validation/fonts.conf \
timeout 30s target/release/terminal -e /bin/sh -c \
  'printf "\033[2J\033[HNative Vulkan release acceptance\n\033[38;2;240;120;40mTruecolor text\033[0m\nBold: \033[1mABC\033[0m Emoji: 🚀\n"; sleep 4'
```

`WGPU_BACKEND=vulkan` is honored by production initialization. The diagnostic
variables enable existing opt-in logging; the screenshot variable belongs only to
the acceptance test. `-e` executes the given command directly; the temporary
shell delay keeps its sample visible before normal exit. The final GUI screenshot
run was performed alone, after an initial capture overlapped the release window.
