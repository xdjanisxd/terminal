# KDE Wayland background blur

Verified on 2026-10-09 on branch `feat/linux-window-blur`.

```toml
[window]
opacity = 0.85
blur = true
```

`blur` is optional, boolean, and defaults to false. This requests compositor
background blur on Linux Wayland without changing foreground rendering or KDE
settings. Unsupported platforms/compositors retain the existing opacity path.

## Protocol and integration

The pinned winit 0.30.13 already implements KDE's
`org_kde_kwin_blur_manager` version 1. Its safe `Window::set_blur` API creates
and commits a blur object for the existing Wayland surface; disabling unsets
and releases it. winit owns the connection, dispatch, and protocol object.
The manager is optional, so its absence is an opacity-only fallback.
See the [KDE protocol](https://github.com/KDE/plasma-wayland-protocols/blob/master/src/protocols/blur.xml)
and [winit API](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.set_blur).

The winit request leaves the blur region empty. KWin interprets this as its
entire client contents rectangle and clips it to that rectangle. Geometry and
scale changes therefore need no application-managed region updates. KWin
processes the background before drawing the window; Terminal's glyphs, cursor,
selection, and UI remain sharp. See
[KWin's blur implementation](https://github.com/KDE/kwin/blob/Plasma/6.4/src/plugins/blur/blur.cpp).
Server decorations and their effects remain compositor policy.

The newer
[ext-background-effect-v1 protocol](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-background-effect/ext-background-effect-v1.xml)
is a successor with capability reporting and different region semantics.
Current [upstream KWin](https://github.com/KDE/kwin/blob/master/src/plugins/blur/blur.cpp)
uses that interface. This implementation uses the legacy protocol supported by
our pinned winit; it does not introduce raw handles, another Wayland connection,
custom protocol bindings, new dependencies, or a new windowing framework.
A compositor exposing only the successor will use opacity-only rendering.

## Implementation and lifecycle

- `terminal-config` owns the typed `WindowConfig.blur` and validates TOML types.
  Opacity's existing finite-number/range validation remains unchanged.
- `crates/app/src/window_effects.rs` isolates the native request. Compile-time
  Linux and runtime Wayland guards prevent invoking blur on Windows, macOS, or
  X11. In particular, macOS's winit blur API is never called.
- The app applies blur immediately after window creation, before rendering.
  Existing windows retain their effect during GPU renderer recreation; new
  windows receive the current configuration again.
- Live reload applies changed blur on the event-loop thread and invalidates the
  frame through the existing reload path, providing the next surface commit.
  Simultaneous opacity changes use the existing renderer setter. Invalid
  configuration retains both last valid values. Omitting/removing settings
  restores opacity 1.0 and blur false; repeated unchanged reloads do nothing.
- Linux shutdown explicitly drops the renderer before the window in
  `ApplicationHandler::exiting`, while the event loop's Wayland connection is
  alive. Native testing exposed an existing Mesa EGL cleanup crash when these
  owners instead outlived the consumed event loop. The corrected ordering
  releases GPU resources, blur, and surface safely; Windows/macOS teardown is
  unchanged.

No renderer, shader, terminal semantics, or alpha composition changes were
needed. Application-side screenshots are used only for optional test evidence,
never to produce the effect.

## Opacity and reload behavior

| Configuration | Observed behavior |
| --- | --- |
| 0.85, blur false | Transparent backgrounds show sharp desktop detail. |
| 0.85, blur true | Background detail is blurred; foreground stays sharp. |
| 0.0, blur true | Cell/padding background is transparent over blurred desktop. |
| 1.0, either blur value | Opaque backgrounds hide the effect; existing opaque path remains. |
| Invalid reload | Previous opacity and blur remain active. |
| Blur omitted | Defaults to false, including live reload. |
| Unsupported/disabled compositor | Existing opacity-only path remains functional. |

Selection and UI retain their existing opaque backgrounds; ANSI backgrounds
retain their existing opacity rules. Blur strength belongs to the compositor.
Transparent rendering still requires a premultiplied surface; the existing
opaque fallback for unsupported alpha modes remains unchanged.

## Native verification

Environment: KDE KWin 6.3.6, Qt 6.8.2, Mesa 25.0.7, Intel Iris Xe (ADL GT2).
Wayland advertised `org_kde_kwin_blur_manager` v1 and the blur effect was active.
Terminal used winit Wayland and wgpu Vulkan, with `PreMultiplied` alpha below
opacity 1 and `Auto` on the opaque path. Displays used scale 1 and 1.2.

| Native check | Result |
| --- | --- |
| Initial 0.85 + blur; false/true toggles | Passed against a checkerboard test window. |
| Opacity 0.0, 0.85, 1.0 and simultaneous reload | Passed; opaque rendering remained unchanged. |
| Invalid reload and omitted settings | Passed; last valid values retained, omission restored defaults. |
| Resize, maximize, minimize/restore | Passed; blur followed client geometry. |
| Move between scale 1 and 1.2, then back | Passed; background and foreground remained correct. |
| Text, cursor, selection, ANSI backgrounds, command palette | Sharp foreground verified visually. |
| Renderer suspension/recreation and new window creation | Native test passed; effect retained/reapplied. |
| Shutdown | Exit 0 after cleanup ordering fix; protocol releases observed. |
| Xwayland | Opacity worked; blur toggles had no visual/protocol effect; exit 0. |
| KWin without blur protocol/effect | Separate private session passed opacity, reload, and clean shutdown. |

The fallback session used its own D-Bus session, virtual Wayland socket,
temporary runtime/config directories, and `blurEnabled=false` in its private
kwinrc. The desktop's compositor settings were not modified.

Checkerboard evidence is cropped to the test window:
[opacity-only 0.85](measurements/linux-window-blur/wayland-false-085.png),
[blur 0.85](measurements/linux-window-blur/wayland-true-085.png),
[opacity-only 0.0](measurements/linux-window-blur/wayland-false-0.png),
[blur 0.0](measurements/linux-window-blur/wayland-true-0.png).
In a fixed 400×400 background patch, RGB standard deviation changed from
approximately (15, 13, 7.5) without blur to (0.48, 0.48, 0.33) with blur at 0.85.
Xwayland's corresponding values were unchanged. This measures this fixture,
not a general compositor blur strength.

The native lifecycle test captures
[selection and ANSI text](measurements/linux-window-blur/selection-blur.png)
and [palette UI](measurements/linux-window-blur/palette-blur.png).
Its [filtered protocol trace](measurements/linux-window-blur/lifecycle-protocol.txt)
records enable, disable, recreation, and release. Suspension is invoked by the
test driver on real native windows; it does not simulate an actual OS suspend.

## Tests and reproduction

Focused parsing tests cover defaults, true/false, coexistence with opacity,
and invalid types. The application reload regression covers simultaneous
changes, invalid reload, defaults, and repeated reloads before window creation.
Existing renderer opacity tests remain unchanged.

Verification passed: `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, and release build of `terminal-app`.
The ignored native test was explicitly run and passed:

```sh
cargo test -p terminal-app native_wayland_blur_reload_recreation_and_shutdown -- --ignored --nocapture
```

Run it alone in a native Wayland session with a GPU. Optionally set
`WAYLAND_DEBUG=1` for protocol evidence and
`TERMINAL_BLUR_TEST_SCREENSHOTS=/tmp/blur-lifecycle` for Spectacle screenshots.
The test checks lifecycle/configuration invariants; advertised support and the
visual blur effect must still be independently inspected.

## Limits and fallback

The legacy protocol supplies no effect-active acknowledgement. An accepted
setting is a request, not a guarantee of visible blur. Missing protocol,
disabled effect, or compositor policy leaves opacity-only rendering; no system
settings are changed. winit binds the optional manager during startup; a manager
first advertised later requires restarting Terminal. Toggling the real desktop's
compositor effect during a running session was not tested.

Only the available Linux target was compiled and run. Windows/macOS behavior is
preserved by compile-time guards and unchanged renderer code, but native or
cross-target verification on those platforms was unavailable. Arbitrary
Wayland compositors and successor-only KWin versions were not tested. There
is no blur strength/region configuration or support-status UI.
