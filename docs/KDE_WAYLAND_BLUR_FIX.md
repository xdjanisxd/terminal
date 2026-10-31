# Plasma 6.7+ Wayland blur fix

Implemented on `fix/kde-wayland-background-blur`, based on `85bfd22`.
Validated on 2026-10-31. No push or merge performed.

## Root cause and upstream assessment

Transparency and blur use independent mechanisms. The renderer already submits
translucent backgrounds correctly. winit 0.30.13's `Window::set_blur` only uses
`org_kde_kwin_blur_manager`, which Plasma 6.7 removed. Merely advertising
`ext_background_effect_manager_v1` cannot trigger requests from that winit version.
This matches [Alacritty issue 8987](https://github.com/alacritty/alacritty/issues/8987).

Upstream [commit c4afadb](https://github.com/rust-windowing/winit/commit/c4afadbfabf7b1e7989b40b493db1a4c7bd8ff4e)
adds support in the 0.31 architecture, available in 0.31.0-beta.3 and later.
The [release list](https://github.com/rust-windowing/winit/releases), checked
October 31, lists stable 0.30.13 and prerelease 0.31.0-beta.4. Migrating would
change trait-based window/event-loop ownership, generic user-event delivery,
pointer handling, and surface lifecycle callbacks throughout this application
and its native tests. That cross-platform migration is too broad for this fix.
See [the investigation plan](KDE_WAYLAND_BLUR_FIX_PLAN.md).

## Implementation and compatibility

The existing `crates/app/src/window_effects.rs` abstraction now owns effects
per window. Its Linux-only `window_effects/wayland.rs` uses the official
`wayland-client`, `wayland-backend`, and `wayland-protocols` bindings. These three
packages were already in Cargo.lock; only direct Linux dependency edges were
added, with no package additions or version updates. There is no custom XML,
generated binding source, fork, second display socket, renderer change, or
configuration change.

- Initialization is lazy: the default blur-off path does not discover or bind
  additional globals. On first enable, a guest backend borrows winit's live
  display and surface. Two initial roundtrips discover globals and capabilities.
- The successor takes precedence when advertised. Its `blur` capability must
  be present before creating an effect. Missing capability leaves opacity-only
  rendering. Missing successor uses the existing winit legacy API, which is
  itself a no-op on unsupported compositors.
- A private event queue handles capability changes and manager removal or
  reappearance. `about_to_wait` dispatches pending events without reading or
  blocking; winit remains responsible for reading the shared socket. Effect
  ownership tracks manager object identity, including registry-name reuse.
- The successor's initially empty region means **no blur**, unlike the legacy
  KWin convention. The adapter explicitly adds `(0, 0, i32::MAX, i32::MAX)` in
  surface-local coordinates, following upstream's approach. The compositor
  clips it to the surface, so resize, maximize, and fractional scaling require
  no stale physical-to-logical bounds or additional region objects.
- Regions have copy semantics and are immediately destroyed after use. Changes
  to effects request a redraw; the normal wgpu present supplies the required
  surface commit. The adapter never commits or destroys winit's surface.
- Repeated requests reuse an effect. Disable, lost capability, and removed
  managers release it. Re-enabling or restored capability creates it again.
  GPU renderer recreation retains the same window effect; replacement windows
  receive a new owner with the current configuration.
- The owner retains `Arc<Window>` and `OwnedDisplayHandle` until after all guest
  objects/backend have dropped. Shutdown drops the renderer, then effects, then
  the app window. Foreign-handle operations have local safety comments. Guest
  backend destruction cleans up its local proxies without disconnecting winit.
- X11 never invokes the blur APIs. Windows/macOS compile out the adapter and
  retain their previous no-op behavior. The opacity/alpha fallback, live reload,
  invalid-config retention, and foreground rendering paths are unchanged.

Protocol semantics: [official XML](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-background-effect/ext-background-effect-v1.xml),
also supplied by the locked wayland-protocols package;
[readable protocol reference](https://wayland.app/protocols/ext-background-effect-v1).
Capabilities express support, not a guarantee that compositor policy will
produce a visible effect. Blur strength remains compositor-owned.

This adapter is temporary. Remove its Linux dependencies and implementation
after a separately validated stable winit migration supports both protocols;
retain the application boundary, configuration, and lifecycle acceptance tests.

## Automated validation

Passed:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo build -p terminal-app --release --offline
cargo test --workspace --doc --offline
cargo test -p terminal-app window_effects::wayland --offline
```

Four new headless regression tests use a Unix socket peer to inspect requests
emitted by the official bindings; events go through the production Dispatch
implementations. They cover capability gating (including unknown future bits),
nonempty resize-safe regions, repeated enable, disable/re-enable, capability
loss/restoration, manager removal/replacement including registry-name reuse,
absence and later availability, object cleanup, and borrowed-surface survival.
They do not emulate a compositor's rendering or socket-read scheduling.

The unfiltered `cargo test --workspace --all-targets --offline` fails four
existing shell-prompt tests, both inside and outside the sandbox:

```text
cwd_inheritance_native_bash_and_zsh_report_and_launch
linux_panes_without_startup_commands_remain_idle_and_interactive
linux_saved_workspace_native_cwd_and_startup_commands
shell_exit_during_saved_workspace_restoration_closes_all_restored_workers
```

An untouched `git archive HEAD` baseline in `/tmp/terminal-blur-baseline`
reproduces exactly these failures: 219 app tests pass, four fail, seven are
ignored. The fix has 223 app tests passing (the four additional protocol tests)
and the same failures. No window exists in these failing tests. Skipping only
those four by name lets all remaining workspace targets complete: **910 passed,
31 ignored, zero failed**. No unrelated shell behavior was changed.

## Native verification

KWin 6.8.2, NVIDIA Vulkan driver 615.78.08, Wayland, with integer
wl_output scales 1 and 2. The native protocol trace records fractional preferred
scales 120 and 180 (1.0 and 1.5); the visual matrix also reached its fractional
scale phases. `wayland-info` advertises the successor v1, with no legacy blur manager.

The explicitly run test passed, including clean shutdown:

```sh
WAYLAND_DEBUG=1 cargo test -p terminal-app \
  native_wayland_blur_reload_recreation_and_shutdown --offline -- --ignored --nocapture
```

The test covers initial enable, disable/re-enable, simultaneous opacity changes,
invalid reload retention, resize, selection/palette foreground, GPU renderer
recreation, replacement windows, and shutdown. Native fixture cleanup now also
releases the new effect owner before its window. The
[filtered protocol trace](measurements/kde-wayland-blur-fix/lifecycle-protocol.txt)
shows capabilities(1), get_background_effect, explicit region assignment,
disable destruction, recreation, and final effect/manager destruction.

The existing palette visual matrix captured blur off/on at opacity 0, 0.5,
0.85, and 1, dark/light themes, resize, maximize/restore, and fractional scaling.
The same matrix was run against the unchanged baseline. A fixed 200×160
checkerboard-only crop at `(220,390)` in its 800×600 screenshots gives these
RGB standard deviations:

| Opacity | Baseline blur off/on | Fix blur off | Fix blur on |
| --- | --- | --- | --- |
| 0.0 | 100.000, 57.500, 7.500 | 100.000, 57.500, 7.500 | 4.555, 4.192, 4.087 |
| 0.85 | 15.000, 8.500, 1.000 | 15.000, 8.500, 1.000 | 0.773, 0.647, 0.748 |
| 1.0 | 0, 0, 0 | 0, 0, 0 | 0, 0, 0 |

Evidence at opacity 0.85:
[baseline blur enabled](measurements/kde-wayland-blur-fix/baseline-blur-on.png),
[fix blur disabled](measurements/kde-wayland-blur-fix/blur-off.png),
[fix blur enabled](measurements/kde-wayland-blur-fix/blur-on.png).
These are controlled fixture crops, not desktop captures containing user data.
The numbers quantify this fixture, not a general blur-strength guarantee.

The broader visual harness **does not pass cleanly**: after the captures it
segfaults during teardown, both on the unchanged baseline and with the fix.
A core backtrace enters libnvidia-glcore.so.615.78.08 from
`ash::khr::swapchain::Device::destroy_swapchain` / wgpu Vulkan unconfigure.
This is recorded as an existing limitation, not counted as a passing test and
not addressed by unrelated renderer changes. Logs and the backtrace remain
under `/tmp/terminal-blur-*` for local review.

An X11/Xwayland release smoke launch with blur true and opacity 0.85 exited 0.
Its surface lacks premultiplied alpha, so it reported the existing opaque
fallback; this does not establish visual X11 transparency on this driver.
Legacy-only KWin and a native successor compositor with blur disabled were not
available for new native testing; absence/capability fallback is covered by the
protocol tests, with prior legacy evidence in [the original report](LINUX_WINDOW_BLUR.md).
Windows/macOS targets are not installed locally and were not compiled or run;
their adapter code is excluded by Linux cfg guards. Cross-platform CI remains
necessary before release.
