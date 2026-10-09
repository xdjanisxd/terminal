# KDE Wayland window background blur

Branch: `feat/linux-window-blur`. Do not commit or push.

## Goal

Add optional `[window] blur = true`, defaulting to false, to request
compositor-managed background blur on KDE Wayland. Build on existing Linux
opacity, including live reload. Keep text, cursor, selection, and UI sharp;
preserve Windows/macOS behavior and gracefully retain opacity-only rendering
when the compositor cannot provide blur.

Status: implemented and verified on native KDE Wayland. All eight plan steps
completed; see [the implementation report](LINUX_WINDOW_BLUR.md) for evidence,
fallback results, and compatibility limits. No commit or push performed.

## Initial findings and intended integration

- `terminal-config` owns `WindowConfig` and validates opacity. The app applies
  opacity at renderer creation and in `reload_config`; Linux windows already
  request transparency through `branding::window_attributes`.
- The pinned winit 0.30.13 provides `WindowAttributes::with_blur` and
  `Window::set_blur`. Its Wayland backend binds
  `org_kde_kwin_blur_manager` version 1 when available. Enabling creates and
  commits a surface-associated blur object; disabling unsets and releases it.
  An unavailable manager leaves the window running without blur.
- Prefer this existing safe API over raw Wayland handles, another connection,
  custom protocol bindings, or new dependencies. Keep the project call site in
  a small Linux window-effects module beside the existing winit adapter code.
  Guard runtime calls to Wayland windows; never invoke macOS blur behavior.
- KDE's protocol has no success/effect-active acknowledgement. A requested blur
  state cannot be reported as verified compositor support solely from an API
  call. Native visual verification is necessary.
- Audit current KWin behavior and the newer background-effect protocol before
  finalizing the report. Compatibility with the protocol already supported by
  our pinned winit is the preferred scope; newer protocols alone do not justify
  replacing or extending the windowing stack.

Sources:

- [KDE blur protocol XML](https://github.com/KDE/plasma-wayland-protocols/blob/master/src/protocols/blur.xml)
- [winit 0.30.13 Window::set_blur](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.set_blur)
- Local pinned winit sources: Wayland `types/kwin_blur.rs`,
  `window/state.rs`, `window/mod.rs`, and `state.rs`.

## Plan

1. **Confirm protocol and lifecycle safety.** Trace winit's initial blur
   application, surface commit scheduling, repeated enable/disable behavior,
   resize/scaling behavior, and window destruction. Confirm KWin's default
   full-surface blur region follows surface geometry and applies to the
   background behind the window. Record any limitations involving compositor
   effect disablement or protocol availability changing during a session.
   If safe integration is blocked, document the specific blocker and retain
   opacity-only rendering rather than introducing fragile protocol handling.

2. **Add configuration and focused parsing tests.** Extend typed `WindowConfig`
   with `blur: bool` and raw TOML configuration with `Option<bool>`. Default
   omission to false; accept true/false and reject incorrect types. Test
   omission, explicit values, coexistence with opacity, and invalid values.
   Keep existing opacity range validation and unknown-field handling.

3. **Integrate through the existing window owner.** Add a project-owned helper
   for Linux Wayland blur requests using winit's safe API. Apply the configured
   state to newly created windows before their first visible presentation.
   Keep protocol object ownership, dispatch, and cleanup with winit. Verify
   surface recreation retains or reapplies the desired state appropriately.
   Avoid adding protocol state to the renderer or terminal core.

4. **Connect live reload.** Apply blur changes through the existing config
   reload path on the event-loop thread and ensure a surface commit/redraw
   makes the request visible. Preserve opacity updates and last-valid-config
   behavior on parse errors. Test simultaneous opacity/blur changes, toggles,
   removing settings, reload before window creation, and repeat reloads.
   Keep requested blur independent of opacity; at opacity 1.0 the opaque
   background hides the effect, while lowering opacity reveals it.

5. **Verify rendering and fallback invariants.** Reuse existing background alpha
   composition without new shaders, screenshots, or GPU post-processing.
   Confirm text, cursor, selection, and UI are composited sharply over the
   compositor-blurred background. Unsupported Wayland compositors and X11
   retain existing opacity behavior; Windows/macOS retain existing behavior.
   Do not infer effective support from desktop environment names.

6. **Run focused automated checks.** Run configuration and reload tests,
   existing opacity regression tests, formatting, and workspace compile/test
   checks appropriate to the changed code. Use existing native harnesses where
   useful. Record cross-platform checks actually performed and any unavailable
   targets rather than claiming untested behavior.

7. **Perform native KDE Wayland acceptance.** Record session, KWin version,
   advertised protocol, winit backend, GPU/backend, and surface alpha mode.
   Compare blur false/true at opacity 0.85 against the same detailed background;
   also check opacity 0.0 and 1.0. Capture evidence of background blur and sharp
   foreground text, cursor, selection, ANSI backgrounds, and UI. Exercise live
   toggles, simultaneous opacity reload, invalid reload, resize, minimize and
   restore, available integer/fractional scale transitions, window recreation,
   and shutdown. Check opacity-only fallback in an available unsupported
   environment. Observe compositor-disabled blur if already available; do not
   change system-wide compositor settings. Report unavailable cases explicitly.

8. **Document implementation and results.** Update `config.example.toml` and
   `docs/CONFIG.md`; align the existing opacity report's blur statements. Add
   `docs/LINUX_WINDOW_BLUR.md` describing protocol/integration, code ownership,
   opacity plus blur behavior, live reload, native evidence, fallback, and
   limitations. Distinguish protocol requests from visible compositor effects.

## Completion criteria

- Optional boolean defaults to false and the requested configuration parses.
- KDE Wayland uses compositor blur through the existing winit architecture.
- Existing opacity and live reload remain correct; foreground content stays
  sharp and Windows/macOS behavior stays unchanged.
- Lifecycle safety and fallback are checked with focused tests and native
  evidence, with unavailable verification clearly identified.
- No compositor setting changes, new windowing framework, application-side
  blur workaround, unrelated renderer changes, commit, or push.
