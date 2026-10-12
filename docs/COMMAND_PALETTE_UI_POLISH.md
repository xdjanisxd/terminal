# Command Palette UI polish

Implemented on `feat/command-palette-ui-polish`. No commit or push.

## Presentation

Ctrl+Shift+P opens a centered panel below the custom title bar, spanning the
window rather than the focused split pane. It is limited to 540 logical pixels
and eight visible results, with 13px typography matching the title bar, compact
padding, a restrained border, and a semantic selection highlight. Navigation
keeps the selected command visible. Long labels truncate with an ellipsis, wide
characters and combining marks keep their cell widths, and long queries keep
the insertion end visible. Small windows reduce the result count and omit
secondary header/footer text when needed.

The palette remains keyboard driven: filtering, Up/Down, Enter, Escape, project
actions, and command dispatch stay in the application. Content mouse events
cannot select terminal text, scroll the terminal, or change the focused pane
while the palette is open. Title-bar controls retain their existing subtle
hover behavior. Opening cancels terminal selection/scrollbar drags and releases
any tracked mouse press; closing preserves pane focus.

## Theme, opacity, and blur

The panel uses resolved semantic UI background, foreground, selected, muted,
and border colors. Explicit theme overrides remain authoritative. Secondary
text and borders derive from the UI foreground/background when not overridden,
so they follow light and dark themes. Theme changes apply while the palette is
open without resetting its query or selection. The same semantic secondary
text defaults also keep the custom title bar consistent with light themes.
Independent scrollbar defaults remain unchanged.

The surrounding terminal's background alpha remains exactly the renderer's
effective window opacity. Dimming mixes terminal colors 14% toward the terminal
background without adding an overlay or changing cell/background alpha. Cursor
and scrollbar presentation are suppressed behind the modal panel; terminal
state is untouched. Foreground coverage is independent of background opacity.

Only the panel has a readability floor: its local background coverage is at
least 94%. It blends over the existing scene, with a subtle selected-row tint;
it does not make the entire window opaque. At window opacity 1, the existing
opaque rendering path remains in use. No palette action changes the native blur
request, window effects, compositor settings, or alpha-mode fallback.

## Regression coverage

- App tests cover palette query/selection/focus retention through valid and
  invalid reload, drag cancellation, filtering, Escape, Enter dispatch, project
  commands, and tab creation/renaming.
- Renderer tests cover dark/light semantic colors, opacity boundaries, unchanged
  dimmed cell/background alpha, bounded/scaled geometry, selected-result
  visibility, and wide/combining text clipping.
- The ignored GPU readback test renders the actual palette at opacity 0, 0.5,
  0.85, and 1 in both linear and sRGB targets. It checks unchanged surrounding
  alpha, colored-cell alpha, and local panel composition/foreground coverage.
- The ignored KDE native test uses real Wayland windows, renderer, app handlers,
  configuration reload, and a checkerboard background. It checks the default
  shortcut mapping, empty-result Enter, execution of split/new-tab commands,
  focus restoration, content mouse isolation, title-bar hover, resize/maximize,
  and an available fractional-scale output. Scripted interaction uses app
  handlers; unrelated live user input is excluded from the test sequence.

## Native acceptance and reproduction

Verified on 2026-10-12 with KWin 6.3.6, Qt 6.8.2, Mesa 25.0.7, Intel Iris Xe
(ADL GT2), winit Wayland, and wgpu Vulkan. The native run passed all 26 phases
and exited cleanly. Each capture waits for the app to present its changed
configuration. Surface alpha mode was `PreMultiplied` below opacity 1 and
`Auto` at opacity 1.

| Native check | Outcome |
| --- | --- |
| Dark/light × opacity 0, 0.5, 0.85, 1 × blur false/true | Passed; themed panel remained readable and surrounding transparency remained visible. |
| Theme/effects reload while open | Passed; nonempty query and selection retained, native frames presented each configuration. |
| Invalid reload | Passed; last valid theme/effects retained. |
| Navigation, empty-result Enter, Escape, repeated reopening | Passed. |
| Split and new-tab command execution | Passed; expected workspace changes and palette closure. |
| Content clicks/wheel, focus loss/return, title-bar hover | Passed through app event handlers on the native window; pane focus, selection, and viewport remained stable. |
| 320×190 resize, maximize, 800×600 restore | Passed; configured renderer dimensions asserted and screenshots inspected. |
| Move to 1.2-scale output and reload light theme | Passed; native scale event and 720×600 buffer confirmed, layout inspected. |
| GPU alpha readback at four opacities, linear/sRGB targets | Passed; surrounding alpha unchanged and local panel coverage/foreground verified. |

Scoped captures show [dark with blur](measurements/command-palette-ui-polish/dark-085-blur.png),
[light with blur](measurements/command-palette-ui-polish/light-085-blur.png),
[zero opacity without blur](measurements/command-palette-ui-polish/dark-0-sharp.png),
[zero opacity with blur](measurements/command-palette-ui-polish/dark-0-blur.png),
[light at zero opacity](measurements/command-palette-ui-polish/light-0-sharp.png),
[small geometry](measurements/command-palette-ui-polish/small-window.png),
[scale 1.2 dark](measurements/command-palette-ui-polish/fractional-scale-blur.png),
and [scale 1.2 light](measurements/command-palette-ui-polish/fractional-scale-light.png).
In the same 300×180 background patch at opacity 0.85, aggregate 8-bit RGB
standard deviation fell from 4.02 [without blur](measurements/command-palette-ui-polish/dark-085-sharp.png)
to 0.27 with blur. This measures this checkerboard fixture, not general blur
strength. The [filtered native evidence](measurements/command-palette-ui-polish/native-evidence.txt)
records blur requests, alpha modes, geometry, scale changes, and test outcomes.

Workspace tests, workspace/all-target Clippy with warnings denied, formatting,
diff checks, release build, and both explicitly invoked native tests passed.

The fixture uses temporary KWin scripts restricted to its process's windows.
Geometry changes come from KWin configure events, rather than pre-updating
winit's stored client size. GPU owners are released before the Wayland event
loop, including on a test assertion failure. Captures focus the fixture window;
the scripted sequence ignores unrelated live user input. No desktop compositor
settings are changed.

Run the tests alone in a native KDE Wayland session with a GPU:

```sh
cargo test -p terminal-app native_wayland_palette_theme_opacity_and_interaction -- --ignored --nocapture
cargo test -p terminal-renderer native_gpu_palette_preserves_surrounding_alpha -- --ignored --nocapture
```

The native fixture requires `qdbus6`; optional screenshot evidence also requires
Spectacle and ImageMagick. Set `TERMINAL_PALETTE_TEST_SCREENSHOTS` to an output
directory. Screenshots capture the compositor and are cropped to the fixture
window. Set `TERMINAL_PALETTE_CAPTURE_SCALE` to the full-desktop screenshot's
backing scale (2 in the tested mixed-scale session). `WAYLAND_DEBUG=1` and
`TERMINAL_RENDERER_DIAGNOSTICS=1` provide protocol and renderer evidence.

## Limits

Blur remains compositor policy and requires supported native alpha composition;
unsupported surfaces retain the existing opaque fallback. The panel's local
94% readability floor intentionally differs from the surrounding terminal's
opacity. Arbitrary low-contrast explicit color overrides can still reduce
readability. Extremely small windows that cannot fit one UI row cannot display
a useful palette, although keyboard dismissal remains available.

Native acceptance is scoped to the available KDE Wayland session. Windows,
macOS, other compositors, and successor-only blur protocols were not tested.
The palette has no result-row mouse interaction, matching its existing keyboard
behavior; title-bar mouse interaction remains available.
