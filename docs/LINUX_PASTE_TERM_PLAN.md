# Linux clipboard paste and TERM

Branch: `fix/linux-paste-term`. Do not commit or push.

Goal: fix native Linux clipboard paste and Linux/macOS PTY TERM initialization,
preserving explicit bindings, bracketed paste, child environment overrides,
Windows behavior, and shell integration.

1. Trace keyboard dispatch, clipboard services, active-pane switching, and PTY writes.
2. Add Linux clipboard support through the platform boundary with Wayland data
   control and X11 fallback; preserve clipboard ownership across operations.
3. Add Linux's Shift+Insert default binding through the existing configuration
   mechanism, so replacement bindings remain authoritative.
4. Trace TERM inheritance and shell initialization. Validate xterm-256color's
   relevant terminfo sequences against the parser and input encoder before
   choosing it as a child-only default. Preserve non-dumb inherited TERM and
   explicit shell/spawn overrides, including intentionally configured dumb.
5. Add focused clipboard, binding, pane routing, bracketed paste, TERM, and
   configuration tests.
6. Run fmt, workspace check, all-target tests, doctests, Clippy, and release build.
7. Exercise the native release on Wayland and X11 with Bash, clear, clipboard,
   split panes, and Vim/Neovim; record direct observations and limitations.
8. Audit requirements and report findings in `docs/LINUX_PASTE_TERM.md`.
