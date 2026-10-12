# URL Hint Mode

Branch: `feat/url-hint-mode`. No commit or push.

## Detection and opening

Ctrl+Shift+U (`open_url_hints`) detects HTTP/HTTPS URL text in the active
pane's visible Primary viewport, including visible scrollback. It joins only
soft-wrapped rows and preserves cell spans for Unicode, wide characters, and
combining marks. Hard line breaks remain boundaries. Sentence punctuation
and unmatched closing delimiters are trimmed; balanced parentheses/brackets
remain part of the URL. Separate occurrences of identical text receive
separate hints, while one wrapped occurrence receives one hint.

Detection never reads hidden history to complete a target. Known clipped
continuations at viewport edges are omitted. HTTP/HTTPS targets require a
usable authority; credential-bearing authorities, control/whitespace
characters, invalid ports, and malformed IPv6 literals are rejected.
The opener rechecks policy and calls the existing `terminal-platform`
interface: `xdg-open` on Linux, `open` on macOS, and `ShellExecuteW` on Windows.
Process arguments are passed literally; no shell command is constructed.
Launch failures reported by this interface appear as an app notice.

## Hints

The only hint characters are `A S D F J K L`, in that order. Occurrences are
ordered by visible row and column. Up to seven occurrences receive the first
single-character hints. Above seven, repeatedly replace a shortest hint leaf
with alphabet children, using later leaves first for equal depths. Add only
the children needed, up to seven per expansion. This preserves earlier short
hints, minimizes maximum depth, and maintains prefix freedom because a parent
ceases to be a complete hint when it gains children.

- 7 URLs: `A S D F J K L`.
- 8 URLs: `A S D F J K LA LS`.
- 13 URLs: `A S D F J K LA LS LD LF LJ LK LL`.
- 49 URLs: all two-character combinations, including `AJ`, `KS`, `DJ`, `FL`.
- 50 URLs: the other 48 two-character hints plus `LLA LLS`.
- Further expansion supports three, four, or more characters as needed.

Assignments are frozen for a session. Filtering never renumbers candidates.

## Input and lifecycle

Typing a hint character, case insensitive, filters the labels by the accumulated
prefix. Completing a hint opens immediately and ends the mode. Backspace removes
one prefix character; Escape cancels. Invalid characters/prefixes are ignored.
Hint keystrokes stay out of the PTY, and held hint keys cannot leak repeat events
after opening. Explicit configured chords, including repeats, take precedence and leave the mode
when dispatching their configured action. Custom binding lists retain their
existing replacement semantics. Normal terminal keyboard selection is unchanged.

Ctrl+left-click opens a detected URL on release, including any wrapped span.
It requires the same occurrence under the pointer and Control still held.
Movement exceeding three physical pixels cancels opening and keeps the ordinary
selection-drag path. Ctrl+Shift+click on explicit OSC 8 targets retains its
existing behavior.

Both new gestures stay inactive on Alternate screens and while application
mouse reporting is enabled. The unavailable activation binding falls through
to existing terminal key encoding; explicitly configured replacement actions
still take precedence. No application names are inferred. Hint Mode
cancels on pane/tab changes, focus loss, scrolling, terminal dimension changes,
or detected target/span changes. Pure geometry/scale updates with an unchanged
grid reposition labels without reassignment. Opening with no URLs does not
capture keyboard input.

## Presentation

Small local labels use the live semantic selected foreground/background and
accent colors, with 11px logical typography. Their background has a local 94%
readability floor. Surrounding terminal cells are not dimmed, and their alpha is
unchanged. Labels prefer space after a URL's first visible span, fall back beside
its start at pane edges, and remain clipped to the active pane below the custom
title bar. There is no popup or separate list. Existing native opacity, blur,
and title-bar paths remain in use.

## Verification

Focused tests cover URL parsing, Unicode cell spans, soft wraps/hard breaks,
punctuation, viewport clipping, duplicate occurrences, safe opener policy and
failure, deterministic hint generation, prefix freedom for counts 0–399 plus a 10,000-target, five-character case,
filtering/backtracking, configured binding precedence, invalidated sessions,
and input isolation against a real PTY. Core tests cover viewport wrap metadata
through history and screen switching. A resized-history regression verifies
that horizontally clipped URLs are omitted without reading hidden text. Renderer tests cover pane bounds, scale,
semantic colors, and local alpha.

Native KDE Wayland acceptance exercises the real application/renderer and
production keyboard/mouse handlers at 100%, 125%, and 200% display scaling.
It covers dark/light themes, opacity 0/0.85/1, blur on/off, filtered labels,
Backspace/Escape, default-browser dispatch by hint and Ctrl+Click, Ctrl-drag
selection, alternate-screen exclusion, active split-pane scope, and focus,
scroll, resize, and mouse-reporting transitions. Display scaling is restored
after the runs. Capture-related focus changes and unrelated live user input
are isolated by the fixture; focus cancellation is exercised explicitly.

Native GPU readback exercises hint rendering at opacity 0/0.85/1 on both
linear and sRGB output formats, proving unchanged surrounding alpha and
the local readability floor. Real Vim and Neovim PTY acceptance confirms
their alternate-screen guard and normal editing/quit behavior.

Run the native checks individually:

```sh
cargo test -p terminal-app native_wayland_url_hints_theme_opacity_and_interaction -- --ignored --nocapture
cargo test -p terminal-app native_url_hints_preserve_vim_and_neovim_input -- --ignored --nocapture
cargo test -p terminal-renderer native_gpu_url_hints_preserve_surrounding_alpha -- --ignored --nocapture
```

Evidence: [native captures and logs](measurements/url-hint-mode/README.md).
Required Rust checks are `cargo fmt --all -- --check`,
`cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`.

## Remaining limitations

- Detection uses conservative text heuristics, not a complete RFC URI parser.
  Ambiguous trailing sentence punctuation may also be valid URL data.
  Clipped URLs are omitted rather than reconstructed from hidden content.
- Target-changing output cancels the session; rapidly updating terminal content
  may require reactivation. Newly visible targets do not join an existing session.
- The new gestures intentionally do not operate inside Alternate screens or
  mouse-reporting TUIs. Actual Codex interactive sessions were not driven by
  the automated acceptance fixture; preservation relies on the same input guards
  and unchanged normal input paths.
- Extremely dense or narrow layouts may place a label over the URL start or
  overlap neighboring labels when adjacent placement is impossible.
- Linux/macOS process launch reports successful spawning, not eventual page
  loading or browser errors. Windows/macOS native behavior was not exercised
  in this Linux session. Native input acceptance is scripted through production
  handlers; it does not synthesize hardware keyboard events.
