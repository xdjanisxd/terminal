# URL Hint Mode verification evidence

Verified on Linux KDE Wayland on 2026-10-12. Branch: `feat/url-hint-mode`.
No commit or push. See [behavior and limitations](../../URL_HINT_MODE.md).

## Rust gates

[Recorded commands and exit codes](rust-gates.json) show successful formatting,
workspace/all-target checks, workspace/all-target tests, Clippy with warnings
denied, release application build, and diff whitespace checks. The workspace
test run passed 710 tests; 13 environment-dependent tests were ignored by the
normal run. Relevant ignored checks were run explicitly below.

## Native Wayland matrix

The app test `native_wayland_url_hints_theme_opacity_and_interaction` passed at
100%, 125%, and 200% display scaling. Each run cycles dark/light themes,
opacity 0/0.85/1, and blur off/on. It exercises filtering, Backspace, Escape,
default-browser dispatch by hint completion and Ctrl+Click, Ctrl-drag selection,
alternate-screen protection, active split-pane labels, focus/scroll/resize
cancellation, and mouse-reporting exclusion. Display scaling was restored;
`kscreen-doctor -o` confirmed both connected displays at scale 1 afterward.
The fixture excludes capture-induced focus changes and live user input;
focus cancellation is tested explicitly through the production event handler.
Initial fractional-scale geometry is allowed to settle before capture.

| Scale | Recorded output | Selected capture |
| --- | --- | --- |
| 100% | [Native log](native-100.txt) | [Dark, opacity 0.85, blur](dark-085-blur.png) |
| 125% | [Native log](native-125.txt) | [Light, opacity 0.85, blur](fractional-light-085-blur.png) |
| 200% | [Native log](native-200.txt) | [L-prefix filtering](scale-200-filtered-L.png) |

Additional captures: [transparent dark](dark-0-sharp.png),
[light at opacity 0.85](light-085-blur.png),
[opaque light](light-1-blur.png), and
[labels restricted to the active split pane](active-split-pane.png).

## Additional native checks

- [GPU readback output](native-gpu-alpha.txt): `cargo test -p terminal-renderer native_gpu_url_hints_preserve_surrounding_alpha -- --ignored --nocapture`
  passed. Real GPU readback covered opacity 0/0.85/1 in linear and sRGB formats,
  checking unchanged surrounding alpha and the label readability floor.
- [Vim/Neovim output](native-vim-neovim.txt): `cargo test -p terminal-app native_url_hints_preserve_vim_and_neovim_input -- --ignored --nocapture`
  passed. Installed `/usr/bin/vim` and `/usr/bin/nvim` entered their real
  alternate screens, stayed excluded from URL gestures, accepted normal editing
  and quit input, and returned to Primary through a real PTY.

Native keyboard acceptance is scripted through production handlers; hardware
keyboard injection and an actual Codex interactive session were not exercised.
Browser checks establish dispatch through the default native handler, rather
than eventual web-page loading. Windows/macOS native verification was not run.
