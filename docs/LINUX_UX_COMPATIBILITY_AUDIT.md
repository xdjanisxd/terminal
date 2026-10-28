# Linux UX and compatibility audit

The 2026-10-28 audit examined keyboard and Kitty input routing, configurable
bindings, selection, clipboard and mouse routing, scrolling, search, terminal
screen transitions, shell integration, workspace layouts, reflow and Unicode.
Search now matches across soft-wrapped Primary rows and keeps hard line breaks
as boundaries. Search remains unavailable on the alternate screen.

Linux workspace tests cover terminal input encodings, selection, reflow,
Unicode, Bash and Zsh shell integration, and workspace behavior. The ignored
real-PTY Vim and Neovim test passed. On KDE Wayland, the native URL interaction
test passed its selection, mouse reporting, resize and alternate-screen guards;
the native tab picker test passed at the host scale. A release-build terminal
launched an isolated tmux 3.5a session; tmux accepted injected input and the
terminal exited successfully. That tmux check did not verify physical keyboard
events or the rendered screen.

## Manual acceptance still needed

- Use physical keyboard shortcuts, configured bindings, and Kitty press,
  repeat and release reporting in a supporting application.
- Copy and paste Unicode text with the desktop clipboard; drag, double-click,
  extend and scroll selections, including while an application captures mouse input.
- Search visible output and scrollback across a soft wrap; navigate matches,
  resize, and verify prompt and highlight placement.
- Exercise interactive Bash, Zsh, Neovim and tmux, including alternate-screen
  entry and exit, scrolling and resize.
- Create, focus and close tabs and splits; resize and reflow Unicode and wide
  glyphs at ordinary and fractional display scales.

These checks were not performed interactively in this audit. Automated tests
and the tmux pane capture do not establish physical input or visual correctness.
No before-and-after CPU or memory benchmark was run for search.
