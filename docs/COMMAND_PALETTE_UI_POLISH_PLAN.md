# Command Palette UI polish plan

Branch: `feat/command-palette-ui-polish`. No commit or push.

## Goal

Redesign Ctrl+Shift+P as a compact, minimal palette integrated with Terminal's
semantic theme, window opacity, compositor blur, and custom title bar. Preserve
search/filtering, keyboard navigation, Enter/Escape, command execution, and focus.
Support dark/light themes and live configuration changes, with readable text at
different opacity levels. Add focused regression coverage and verify natively
on KDE Wayland.

## Current implementation

- `crates/app/src/commands.rs` owns filtering and selection; execution remains
  in `Application` in `crates/app/src/main.rs`.
- `Application::palette_overlay` currently emits the generic `TextOverlay`.
  `TerminalRenderData::apply_text_overlay` replaces full terminal rows with UI
  cells whose background alpha does not follow window opacity. The palette is
  projected into the focused pane.
- `RenderTheme::ui` already supplies semantic panel, text, selected, muted,
  accent, and border colors. Inspect configuration resolution before changing
  any defaults; the opaque appearance must not be assumed to come solely from
  a literal black color.
- `crates/renderer/src/title_bar.rs` already uses theme colors, modest typography,
  restrained controls, and an opacity-aware surface background.
- The renderer already has premultiplied alpha composition and an opaque
  fallback. `crates/app/src/window_effects.rs` owns native blur requests.
  Existing blur documentation and native test fixtures provide a starting point.

## Implementation sequence

1. **Trace presentation and interaction.** Review theme resolution, live reload,
   overlay precedence, pane geometry, GPU blending, palette input routing, and
   mouse/focus behavior. Record the current visual baseline on KDE Wayland.
   Decide placement from actual window/pane behavior, including small windows,
   splits, and scale changes. Keep command state and execution in the app.

2. **Add narrowly scoped palette presentation.** Use the existing renderer and
   drawing primitives to give the palette a bounded panel with compact padding,
   clean query/result spacing, restrained typography, a subtle semantic border,
   and a clear selected row. Use existing semantic colors and explicit overrides.
   Match the custom title bar's visual language. Keep long text clipped safely
   and the selected command visible as navigation moves through long lists.
   Apply subtle hover styling wherever palette mouse interaction exists.
   Avoid heavy shadows, oversized elements, and new dependencies/frameworks.

3. **Integrate opacity and dimming.** Respect the renderer's effective window
   opacity and premultiplied composition. Keep dimming restrained and theme-aware,
   without an opaque black layer or unnecessarily increasing background alpha.
   Keep text and selection legible independently of surface transparency; verify
   the panel's contrast policy against both bright and dark desktop backgrounds.
   Preserve native blur requests and title-bar integration while opening,
   navigating, and closing the palette. Leave unrelated overlays unchanged.

4. **Verify live changes and behavior.** Resolve presentation from the current
   theme so dark/light changes and explicit UI color overrides apply while the
   palette is open. Confirm simultaneous theme/opacity/blur reload, last-valid
   configuration handling, Escape/focus restoration, Enter dispatch, empty
   results, and existing keyboard behavior.

5. **Add focused regression tests.** Cover semantic color selection in dark/light
   themes, opacity boundaries and alpha-preserving dimming, selection visibility,
   clipping/small geometry, and palette open/close/input/dispatch invariants.
   Extend existing app/renderer coverage where possible. Verify other overlays
   retain their prior behavior. Use GPU/native tests for composition invariants
   that draw-data tests cannot establish.

6. **Run checks and native acceptance.** Run focused tests, then workspace tests,
   formatting checks, and workspace Clippy. Build the native application and
   inspect it on KDE Wayland using a detailed desktop background. Compare dark
   and light themes at opacity 0.0, 0.5, 0.85, and 1.0 with blur on/off; reload
   theme and effects while open. Exercise filtering, navigation, Enter/Escape,
   execution, focus restoration, repeated reopening, split panes, resizing,
   maximize/restore, and available integer/fractional scaling. Capture scoped
   screenshots and environment details. Native visual inspection must establish
   blur and readability; a successful protocol request alone is insufficient.

7. **Document and report.** Align meaningful UI documentation with the resulting
   behavior. Report visual changes, theme/opacity integration, native KDE evidence,
   and remaining limitations. Distinguish passed checks from unavailable checks.
   Do not commit or push.

## Acceptance criteria and limits

- The palette is compact, theme-aware, visually consistent with the title bar,
  and free of hardcoded black panel/scrim colors or opaque black overlays.
- Opening the palette preserves surrounding terminal/window transparency and
  compositor blur; dimming does not flatten the window into an opaque surface.
- Text, selection, and any existing mouse interaction remain clear in dark/light
  themes, including live changes and low-opacity configurations.
- Existing command and focus behavior remains intact. Terminal semantics stay
  in `terminal-core`; no unrelated renderer changes or UI framework are added.
- Focused regressions and native KDE acceptance have recorded outcomes.
- Blur remains compositor-controlled and alpha support retains the existing
  fallback. Any unreadable extreme-opacity/background combinations or native
  verification unavailable in the current environment must be reported honestly.

Status: implementation and validation complete. See
[the implementation and native acceptance report](COMMAND_PALETTE_UI_POLISH.md)
for visual evidence, regression coverage, check outcomes, and limitations.
