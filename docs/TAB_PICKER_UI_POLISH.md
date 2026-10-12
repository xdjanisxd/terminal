# Tab Picker UI polish

Implemented on `feat/tab-picker-ui-polish`. No commit or push.

## Visual changes and shared presentation

Ctrl+Shift+Space now uses the Command Palette's `PalettePanel` drawing path
instead of replacing terminal rows with an opaque `TextOverlay`. Both panels
share the 540-logical-pixel width cap, eight-result viewport, 13px title-bar
typography, compact padding, semantic border, selected-row highlight, and
window-wide placement below the custom title bar. Navigation keeps the selected
result visible. Small windows reduce visible rows and truncate secondary text.
Long titles use the existing Unicode-width-aware ellipsis clipping, including
wide characters and combining marks.

The panel accepts app-provided title, placeholder, empty-state, footer, and
optional entry accent columns. The picker supplies tab-specific copy and retains
animated unseen-output indicators in the semantic accent color. Filtering,
title resolution, activity tracking, tab actions, and selection stay in the app;
the renderer owns only presentation. No framework or dependency was added.
Command Palette styling and unrelated overlays remain unchanged.

Both panels share modal input cleanup and mouse guards: opening releases tracked
terminal mouse presses and cancels selection/scrollbar drags; content pointer
events cannot change terminal selection, scrollback, or pane focus while open.
Title-bar controls remain available. Existing picker Up/Down, `j`/`k`, filtering,
Backspace, Enter, and Escape handling is retained. Enter on an empty picker
closes without switching tabs, preserving its prior behavior.

## Theme, opacity, and blur

The picker now uses the same resolved semantic UI colors as the Command Palette,
including live dark/light reload and explicit overrides. Reload retains the
query, selected entry, tab/pane focus, and last valid configuration on errors.

Surrounding terminal dimming mixes RGB 14% toward the current theme background
without adding an overlay or changing surface/cell alpha. Text coverage remains
independent of window background opacity. The shared local panel retains the
94% readability floor; it does not make the surrounding window opaque.
The existing compositor blur requests, premultiplied composition, title-bar
integration, and opaque fallback are preserved.

## Verification

- Formatting, workspace/all-target check and tests, workspace/all-target Clippy
  with warnings denied, diff checks, and the release application build passed.
  Workspace test output records 697 passing tests, zero failures, and ten ignored
  tests; native checks below were invoked separately.
- Existing app tests still cover picker title resolution, filtering, Up/Down,
  `j`/`k`, Escape, tab switching, and activity animation. New coverage checks
  modal cleanup, live theme/effects reload with query/selection/focus retained,
  invalid reload, and empty-result Enter.
- Renderer coverage compares picker and command geometry/rectangles, verifies
  semantic background/selection and activity accents, and checks Unicode title
  clipping and bounds at scales 1, 1.2, and 2 in dark/light themes at opacity
  0, 0.5, 0.85, and 1. Existing alpha-preserving dimming tests passed.
- The explicitly invoked native GPU readback test passed at all four opacities
  in linear and sRGB targets, checking surrounding alpha and local composition.

Native KDE Wayland verification ran on 2026-10-12 with KWin 6.3.6, Qt 6.8.2,
Mesa 25.0.7, Intel Iris Xe (ADL GT2), winit Wayland, and wgpu Vulkan. The shared
fixture passed all 26 phases for the picker and all 26 phases for the Command
Palette. Interaction is scripted through actual app handlers on native windows;
unrelated live user input is excluded. No compositor settings were changed.

| Native check | Outcome |
| --- | --- |
| Dark/light × opacity 0, 0.5, 0.85, 1 × blur off/on | Passed; screenshots inspected for readable panel, transparency, and visible blur. |
| Live theme/effects and invalid reload | Passed; nonempty query and selected result retained, last valid configuration preserved. |
| Thirty tabs, long Unicode titles, navigation, filtering and selection | Passed; selected result visible and titles ellipsized. |
| Empty-result Enter, Escape, reopening, tab switching | Passed; expected closure, active title, and focus assertions. |
| Split panes, content mouse/wheel, focus loss/return, title-bar hover | Passed; pane focus, terminal selection, and viewport remained stable. |
| 320×190 resize, maximize/restore | Passed; renderer size assertions and screenshots inspected. |
| Move from scale 1 to the 1.2-scale output; live light-theme reload | Passed; fractional-scale protocol event, 720×600 buffer, and screenshots inspected. |
| Command Palette regression run | Passed all 26 phases after sharing presentation. |

Evidence includes [dark with blur](measurements/tab-picker-ui-polish/dark-085-blur.png),
[light with blur](measurements/tab-picker-ui-polish/light-085-blur.png),
[zero opacity without blur](measurements/tab-picker-ui-polish/dark-0-sharp.png),
[zero opacity with blur](measurements/tab-picker-ui-polish/dark-0-blur.png),
[light at zero opacity](measurements/tab-picker-ui-polish/light-0-sharp.png),
[empty results](measurements/tab-picker-ui-polish/no-matches.png),
[small window](measurements/tab-picker-ui-polish/small-window.png),
[fractional-scale dark](measurements/tab-picker-ui-polish/fractional-scale-blur.png),
and [fractional-scale light](measurements/tab-picker-ui-polish/fractional-scale-light.png).
In the same 300×180 background patch at opacity 0.85, aggregate 8-bit RGB
standard deviation fell from 4.02
[without blur](measurements/tab-picker-ui-polish/dark-085-sharp.png) to 0.28
with blur. This quantifies the checkerboard fixture, not general blur strength.
[Filtered native evidence](measurements/tab-picker-ui-polish/native-evidence.txt)
records effects, alpha modes, scale/geometry, and test results.

Run each native test alone in a KDE Wayland session with a GPU:

```sh
cargo test -p terminal-app native_wayland_tab_picker_theme_opacity_and_interaction -- --ignored --nocapture
cargo test -p terminal-app native_wayland_palette_theme_opacity_and_interaction -- --ignored --nocapture
cargo test -p terminal-renderer native_gpu_palette_preserves_surrounding_alpha -- --ignored --nocapture
```

The shared fixture requires `qdbus6`. Optional screenshots require Spectacle and
ImageMagick; set `TERMINAL_PALETTE_TEST_SCREENSHOTS` to a separate directory for
each run. `TERMINAL_PALETTE_CAPTURE_SCALE=2` was used for this mixed-scale desktop.
`WAYLAND_DEBUG=1` and `TERMINAL_RENDERER_DIAGNOSTICS=1` supply diagnostic evidence.

## Remaining limitations

Blur remains compositor policy and requires supported alpha composition.
The local 94% panel coverage intentionally differs from surrounding opacity.
Low-contrast explicit color overrides can reduce readability. Extremely small
windows may fit only the query, or no panel row; keyboard dismissal remains
available. Result rows remain keyboard driven.

Native verification covers KDE Wayland at scale 1 and 1.2. Scale 2 is covered by
renderer geometry tests, not a native scale-2 output. Windows, macOS, and other
compositors were not verified.
