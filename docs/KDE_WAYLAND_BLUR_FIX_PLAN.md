# Plasma 6.7+ blur compatibility fix

Branch: `fix/kde-wayland-background-blur`, from `master` at `85bfd22`.

Status: implemented and validated, with baseline failures recorded in
[the implementation report](KDE_WAYLAND_BLUR_FIX.md). No push or merge performed.

## Investigation and decision

winit 0.30.13 sends only `org_kde_kwin_blur_manager` requests. Plasma 6.7
removed that interface in favor of `ext_background_effect_manager_v1`.
Transparency is independent and already works.

As checked on 2026-10-31, upstream's latest stable release is 0.30.13;
0.31.0-beta.4 is a prerelease. Commit
[`c4afadb`](https://github.com/rust-windowing/winit/commit/c4afadbfabf7b1e7989b40b493db1a4c7bd8ff4e)
implements the successor in the 0.31 architecture. The
[release notes](https://github.com/rust-windowing/winit/releases) document
trait-based windows/event loops, removal of generic user events, pointer event
changes, and new surface lifecycle callbacks. This application depends on all
of those APIs. Migrating is a separate cross-platform project, not a safe
dependency bump for this bug fix. A permanent fork/backport is also undesirable.

Use the existing `window_effects` abstraction for a removable Linux-only
adapter with official `wayland-client`, `wayland-backend`, and
`wayland-protocols` bindings. These packages are already locked transitively;
no custom XML, generated bindings, second socket, or renderer changes are needed.
Retain winit's legacy blur path when the successor is absent. Replace the
adapter with upstream `Window::set_blur` after a separately validated stable
winit upgrade.

## Implementation plan

1. Give each window an effect owner, keeping its window and display alive.
2. Lazily discover optional globals on the existing display when blur is enabled.
3. Track advertised blur capability and manager removal/reappearance through a
   private event queue, dispatched without blocking in the existing app loop.
4. Set an explicit whole-surface region, clipped by the compositor, covering
   future resize/scale changes. Apply changes through the next normal present.
5. Destroy effects before their surface/display owners; preserve configuration,
   opacity, legacy Wayland, X11, Windows, and macOS paths.

## Validation plan

Add regression coverage for successor requests, disable/re-enable, capability
changes and missing globals. Extend native lifecycle coverage for resize and
effect ownership. Run repository formatting, check, tests, Clippy, and release
build gates. Run native checks if the available compositor permits them;
record unavailable environments explicitly. Do not push or merge.
