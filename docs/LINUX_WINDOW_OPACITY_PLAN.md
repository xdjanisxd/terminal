# Linux window background opacity

Branch: `feat/linux-window-opacity`. Do not commit or push.

## Goal

Add optional `[window] opacity = 0.85` configuration on Linux, starting with KDE
Wayland. Default to `1.0`, reject values outside `0.0..=1.0`, and change only
terminal background opacity. Preserve theme RGB values, foreground/glyph quality,
cursor, selection, UI, tab/pane layout, DPI/resize behavior, and Windows/macOS
behavior. Keep blur and custom title bars out of scope.

Status: implemented and validated. Native KDE Wayland and XWayland composition
were observed. See [results and remaining coverage limits](LINUX_WINDOW_OPACITY.md).

## Baseline implementation inspected before changes

- `crates/config/src/lib.rs` uses optional raw fields, typed defaults, explicit
  validation, and `ConfigError`; unknown fields are rejected. `Config` currently
  derives `Eq`, which needs consideration when adding a numeric opacity field.
- `crates/app/src/main.rs` creates native windows using `branding::window_attributes`,
  initializes the renderer, resolves theme colors, and reloads configuration while
  retaining the last valid configuration on errors.
- `crates/renderer/src/lib.rs` obtains the default wgpu surface configuration on
  initialization and recovery. Transparency requires deliberate capability-based
  alpha selection, including resize and surface recreation.
- `crates/renderer/src/snapshot.rs` resolves cell colors, inverse video, selection,
  and overlays. `gpu.rs` clears to the projected background, skips redundant cell
  backgrounds, and uses alpha blending; `shader.wgsl` applies glyph coverage.
  Audit all clear paths, pane backgrounds, and decoration drawing before changes.

## Plan

1. **Establish the baseline and alpha contract.** Inspect the complete clear,
   rectangle, glyph, pane, UI, and empty-frame paths. Check the installed winit
   and wgpu versions' transparency and composite alpha contracts. Capture an
   opaque native baseline and available Wayland/X11 backends. Define handling of
   default and explicit ANSI backgrounds and inverse video before implementing;
   selection, search highlights, cursor, and UI must remain opaque. Preserve
   terminal semantics in terminal-core and keep platform policy behind
   project-owned interfaces.

2. **Add validated configuration.** Introduce optional raw `[window]` settings
   and typed window configuration with opacity defaulting to `1.0`. Reject
   non-finite numbers, out-of-range values, invalid types, and unknown fields
   through existing validation. Choose a simple numeric representation that
   preserves adequate precision and accounts for existing equality consumers.
   Do not clamp invalid input or alter theme RGB configuration.

3. **Enable Linux composition.** Request transparent native window composition
   through winit on Linux where needed. Select a supported wgpu composite alpha
   mode explicitly for transparency, preferring a representation compatible
   with rendering. Distinguish requested opacity from effective support; use a
   documented opaque fallback if correct composition is unavailable. Preserve
   the established opaque path for `1.0` where practical and all Windows/macOS
   paths. Avoid extra device initialization, per-frame capability queries, or
   format/present-mode changes without a demonstrated need.

4. **Apply background opacity correctly.** Carry opacity separately from theme
   RGB values and terminal semantic state. Apply it only after resolving cell
   roles so inverse video cannot make foreground text translucent. Match clear
   colors, rectangle blending, output RGB representation, and output alpha to
   the chosen surface mode. Avoid applying opacity twice when cell backgrounds
   cover the surface clear. Preserve glyph coverage/rasterization and opaque
   foreground, cursor, selection, decorations, and UI. Include padding, split
   panes, empty frames, and overlays in the audit.

5. **Integrate lifecycle behavior.** Apply the setting before the first visible
   frame. Preserve resize, zero-size suspension, DPI, lost/outdated surface
   recovery, and window recreation. Investigate native transparency toggling
   before choosing live reload versus restart requirements; document any
   restart requirement rather than rebuilding windows or panes as a workaround.
   Invalid reloads must keep the last valid configuration. Keep opaque UI
   defaults independent of terminal background alpha.

6. **Add focused tests and run Rust validation.** Test absent/empty window
   settings, `0.0`, `0.85`, `1.0`, out-of-range values, NaN/infinity, invalid types,
   unknown fields, and reload rejection. Test supported/unsupported alpha-mode
   selection, opacity-1 equivalence, terminal versus foreground/UI alpha,
   inverse video, explicit backgrounds, selection, and pane projection. Extend
   existing native GPU readback tests where appropriate to verify transparent
   clears and blended pixels with a nonblack background, catching premultiplication
   and double-blending errors. Run:

   ```sh
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   cargo test --workspace --all-targets
   cargo test --workspace --doc
   cargo clippy --workspace --all-targets -- -D warnings
   cargo build --release
   git diff --check
   ```

7. **Perform native Linux acceptance.** Test the release binary on KDE Wayland
   and X11/XWayland where available, recording compositor/session, actual window
   backend, GPU/backend, surface alpha mode, and effective opacity. Place the
   terminal over a colorful background and compare omitted opacity, `1.0`,
   `0.85`, and `0.0`. Check text edges, cursor styles, selections, ANSI backgrounds,
   inverse video, search, palette, tabs, splits, pane zoom, padding, resize,
   maximize/restore, minimize/restore, and available DPI transitions. Compare
   opaque startup timings using existing diagnostics, without claiming changes
   within measurement noise. Record unavailable environments and untested
   behavior honestly; automated pixel tests alone do not establish compositor
   support.

8. **Document and report.** Update `config.example.toml` and `docs/CONFIG.md`
   with defaults, range, Linux scope, reload/restart policy, and fallback behavior.
   Record implementation, Rust validation, native observations, unsupported
   modes, and remaining limitations in `docs/LINUX_WINDOW_OPACITY.md`. Update
   the current-task handoff when implementation begins. Audit each requirement
   before completing the goal; do not commit or push.

## Acceptance and blur handoff

- Omitted opacity and `1.0` preserve current appearance and behavior.
- `0.85` reveals content behind terminal backgrounds without reducing foreground,
  selection, cursor, or UI opacity; `0.0` retains those visible elements.
- Invalid values use existing config diagnostics and reload retention behavior.
- Native KDE Wayland composition is verified, with X11/XWayland results or
  explicit availability limits reported separately.
- Unsupported compositor/wgpu combinations receive documented limitations,
  without fragile workarounds.
- The blur handoff documents the established native transparency and surface
  alpha contract. Blur requires separate compositor support and region policy;
  opacity alone neither requests nor guarantees blur. No blur protocol,
  compositor rule, or custom title bar is introduced by this task.
