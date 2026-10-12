# Tab Picker UI polish plan

Branch: `feat/tab-picker-ui-polish`. No commit or push.

Status: implementation and validation complete. See
[the implementation and native verification report](TAB_PICKER_UI_POLISH.md).

## Goal

Polish Ctrl+Shift+Space to match the completed Command Palette: a compact,
theme-aware panel consistent with the custom title bar, shared spacing,
typography, borders, selection, and dimensions. Preserve window opacity,
compositor blur, live dark/light changes, tab listing/filtering, keyboard
navigation, selection, Enter/Escape, focus, and background activity indicators.
Truncate long titles cleanly and retain correct layout at different DPI scales.
Use the existing renderer without unrelated UI changes or a new framework.

## Implementation findings

- `crates/app/src/main.rs` builds tab entries with `tab_picker_label`, handles
  tab-picker input separately, and presents them through `tab_picker_overlay`.
  The picker shares the app's `Palette` filtering/selection model with commands.
  Its existing `j`/`k` navigation and Enter behavior must remain intact.
- The picker uses `TextOverlay`, which replaces terminal rows with semantic UI
  colors at full background coverage. Theme resolution already supplies those
  colors; the black appearance should not be treated as evidence that a literal
  black picker color needs replacing. Replace this opaque presentation path.
- `crates/renderer/src/palette.rs` owns `PalettePanel`, bounded layout, semantic
  colors, ellipsis clipping, selection visibility, and alpha-preserving dimming.
  Its title, search placeholder, empty-state text, and footer are command-specific.
- `crates/renderer/src/lib.rs` renders that panel across the window below the
  title bar, using 13px UI typography. The existing layout caps width at 540
  logical pixels and visible results at eight, adapting to small windows.
- Palette dimming tints terminal RGB 14% toward the theme background without
  changing alpha. Only the local panel uses a 94% readability floor. Reuse these
  policies rather than changing global window opacity or native blur requests.
- App redraw precedence currently favors workspace/rename/search overlays,
  then the Command Palette, then the Tab Picker. Mouse isolation currently
  checks command-palette state; inspect equivalent picker focus/input handling
  before routing both through the same visual panel.
- Existing picker tests cover titles, filtering, navigation, switching, and
  animated unseen activity. Existing renderer/GPU tests and
  `crates/app/src/tests/linux_palette.rs` supply composition and KDE fixtures.
  The reference behavior and prior evidence are documented in
  [Command Palette UI polish](COMMAND_PALETTE_UI_POLISH.md).

## Plan

1. **Confirm the baseline and integration boundaries.** Trace picker opening,
   filtering, selection, activity animation, redraw precedence, focus, mouse
   routing, configuration reload, and scale changes. Capture the picker and
   Command Palette on KDE Wayland for comparison. Keep tab actions in the app
   and terminal semantics in `terminal-core`.

2. **Share the existing panel presentation.** Extend `PalettePanel` narrowly
   with the presentation information needed for both callers: title, query
   placeholder, empty state, footer, and activity/accent data where necessary.
   Prefer a small explicit presentation variant or fields based on actual
   needs; avoid a general UI framework. Replace `tab_picker_overlay` with a
   panel builder and preserve overlay precedence. Reuse the same drawing path,
   dimensions, padding, font metrics, border, selection highlight, and viewport
   scrolling. Keep command appearance unchanged. Retain tab title resolution
   and animated activity indicators without encoding tab semantics in renderer
   code. Use picker-specific copy such as “Search tabs…” and “Enter Select”.

3. **Integrate theme, effects, and input.** Resolve colors from the current
   semantic theme on each render so live reload applies without resetting query
   or selection. Reuse the palette's local readability floor and alpha-preserving
   dimming across panes. Preserve native opacity/blur configuration and the
   existing opaque fallback. Check opening/closing, drag state, focus restoration,
   and terminal mouse isolation for the picker, sharing only the small modal
   guards or cleanup needed. Preserve shortcuts, Up/Down and `j`/`k`, filtering,
   Backspace, tab selection, and current Enter/Escape behavior including empty
   results. Leave unrelated overlays and title-bar styling unchanged.

4. **Add focused regression coverage.** Adapt existing picker presentation tests
   to the shared panel. Cover correct labels/activity accents, filtering and
   selected-tab dispatch, empty results, Enter/Escape, focus retention, and live
   theme/effects reload with query/selection retained. Extend renderer coverage
   for picker presentation, selected-result visibility, long Unicode titles
   (wide characters and combining marks), narrow windows, and integer/fractional
   scale geometry. Reuse existing dark/light and alpha tests; add coverage only
   for changed paths or distinct picker invariants. Keep Command Palette tests
   passing to establish that sharing preserves its appearance and behavior.

5. **Run Rust validation.** Run focused app/renderer tests first, then the
   repository's normal checks:

   ```sh
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   cargo test --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   git diff --check
   ```

   Build the native application and explicitly run the existing ignored GPU
   alpha readback test. Extend or reuse the KDE fixture for picker-specific
   phases without duplicating its window/capture infrastructure unnecessarily.

6. **Verify visually on KDE Wayland.** Compare both panels with a detailed
   desktop background in dark/light themes at opacity 0, 0.5, 0.85, and 1, with
   blur enabled and disabled. Reload theme/opacity/blur while the picker is open.
   Inspect panel readability, borders, selection, title-bar alignment, surrounding
   transparency, and actual blur in compositor captures; protocol success alone
   does not establish visual blur. Exercise many tabs, long titles, filtering,
   navigation, tab switching, empty results, Enter/Escape, repeated reopening,
   activity animation, focus loss/return, mouse isolation, split panes, small
   windows, resize/maximize/restore, and available integer/fractional DPI scales.
   Record screenshots, environment details, assertions, and unavailable checks.

7. **Document and report outcomes.** Record visual changes, shared presentation
   code, theme/opacity behavior, Rust checks, native KDE evidence, and remaining
   limitations in a concise implementation report. Update meaningful usage
   documentation if behavior requires it. Do not commit or push.

## Acceptance criteria

- Both panels share practical styling/layout and match dimensions, spacing,
  typography, semantic panel/border colors, and selection highlighting.
- The picker uses no opaque full-window black overlay; surrounding alpha stays
  at effective window opacity and compositor blur remains configured as before.
- Live theme/effects reload updates an open picker without losing its query,
  selection, or focus. Explicit semantic color overrides remain authoritative.
- Listing, filtering, navigation, switching, Enter/Escape, focus, and existing
  activity indicators remain functional. Long titles truncate within the panel;
  selected rows remain visible at supported sizes and scales.
- Focused regressions and normal Rust checks have recorded outcomes. Native
  KDE Wayland captures establish visual consistency, transparency, and blur.
- No unrelated UI changes, new framework, commit, or push.

## Known boundaries to report

The shared panel's 94% local readability floor intentionally differs from
surrounding window opacity. Blur remains compositor-controlled and unsupported
surfaces retain the existing fallback. Extremely small windows may not fit a
panel row; dismissal must still work. Record any unavailable DPI outputs,
unverified platforms, or low-contrast explicit theme overrides honestly. Prior
Command Palette verification is a reference, not verification of this task.
