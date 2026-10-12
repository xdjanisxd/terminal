# URL Hint Mode plan

Branch: `feat/url-hint-mode`. No commit or push.

Status: implementation and native verification complete. See
[the behavior and verification report](URL_HINT_MODE.md).

## Goal

Implement Ctrl+Shift+U URL Hint Mode and Ctrl+Click opening for HTTP/HTTPS
URLs in the active pane's visible content. Show compact, theme-aware labels
beside URL occurrences, accept only A S D F J K L hint characters, filter by
prefix, open immediately on completion, and support Backspace and Escape.
Keep assignments stable throughout a session, prevent hint input reaching
the PTY, respect explicit configured bindings, preserve terminal selection
and mouse reporting, and verify on native Linux KDE Wayland.

## Existing integration points

- `crates/terminal-core` owns cells, occupancy, wrap metadata, primary and
  alternate screens, viewport offsets, selection, and terminal input encoding.
  Add only narrowly needed read access to visible row/wrap information here.
- `crates/app/src/main.rs` owns command dispatch, modal input, pane routing,
  mouse handling, and browser-open dispatch. `search.rs` provides an existing
  example of app-owned text matching with terminal cell coordinates.
- `crates/platform/src/lib.rs::open_url` already uses `ShellExecuteW` on
  Windows and separate process arguments for `open`/`xdg-open` elsewhere.
  Reuse this project-owned boundary; never construct a shell command.
- `crates/renderer` already resolves semantic colors, pane geometry, glyphs,
  scale, and alpha composition. Reuse that infrastructure for local labels.
- `crates/app/src/tests/linux_palette.rs` and existing GPU alpha tests provide
  native KDE Wayland verification patterns.

## Plan

1. **Trace input, viewport, and rendering boundaries.** Confirm configured
   binding precedence, normal keyboard selection, active-pane routing,
   primary-screen viewport rows, wrap metadata, mouse reporting, command
   overlays, and scale/resize behavior. Record the baseline before changes.
   Keep parsing and hint state in a small app module; the renderer receives
   presentation data and never interprets terminal semantics.

2. **Detect visible URL occurrences with cell spans.** Extract visible cells
   from the active pane, joining only rows linked by actual soft wraps.
   Preserve a mapping from text offsets to cells, including wide-character
   continuation cells and combining marks. Recognize HTTP/HTTPS schemes,
   require a usable authority, stop at whitespace/control characters, and
   trim sentence punctuation and unmatched closing delimiters while retaining
   balanced URL delimiters. Restrict candidates to visible content; omit
   known clipped continuations at viewport edges instead of opening a guessed
   fragment. Never scan hidden history to complete a URL. Produce one candidate
   per occurrence, including all wrapped spans; repeated text at separate
   positions remains separately selectable. Use these same spans for clicking.

3. **Generate deterministic prefix-free hints.** Order occurrences by visible
   row and column. Use the alphabet `A S D F J K L` exclusively. For 1–7
   candidates, take the corresponding single letters. For larger counts,
   start with seven leaves and repeatedly expand a shortest leaf, breaking
   ties in reverse alphabet order to preserve earlier single letters. Replace
   it with the first `min(7, remaining_count + 1)` alphabet children, increasing
   the leaf count by exactly the number still needed, up to six per expansion.
   Sort final leaves by the specified alphabet and assign them in occurrence
   order. Replacing a leaf with children preserves prefix freedom; expanding
   shortest leaves first bounds depth and avoids a blanket two-letter switch.
   Examples: 7 gives `A S D F J K L`; 8 gives `A S D F J K LA LS`;
   13 gives `A S D F J K LA LS LD LF LJ LK LL`; 49 uses all two-letter
   combinations, including `AJ`, `KS`, `DJ`, and `FL`; 50 expands `LL`
   to `LLA LLS` while retaining the other 48 two-letter hints. Freeze the
   candidate-to-hint mapping for the session.

4. **Integrate modal keyboard input and lifecycle.** Add a configurable command
   with default Ctrl+Shift+U activation using existing binding resolution.
   Explicit configured chords take precedence over built-in hint behavior.
   In Hint Mode, normalize letter case, append valid prefix characters, filter
   without reassigning hints, and open once on exact completion. Backspace
   removes one character; Escape cancels. Ignore invalid hint characters
   without PTY writes, and prevent repeat events from opening repeatedly.
   Opening with no candidates leaves no modal input capture. Keep normal
   keyboard selection unchanged outside the mode. Cancel on pane/tab/focus
   changes, conflicting overlays, scrolling, terminal resize/reflow, or
   content changes that invalidate the snapshot. Scale-only changes recompute
   label geometry without changing hints. Prefer conservative cancellation
   over stale targets; document behavior for continuously changing output.

5. **Integrate safe opening and Ctrl+Click.** Validate the HTTP/HTTPS target
   before calling the existing platform opener and report launch errors through
   existing app error handling. Make the opening boundary testable without
   launching a browser. Recognize Ctrl+left-click on a detected span only when
   terminal mouse reporting is off and the primary screen is active. Track
   press/release and drag so a selection gesture cannot launch a browser;
   consume the complete recognized click gesture. Leave other clicks,
   selection gestures, and application mouse-report sequences unchanged.
   Initially disable Hint Mode and URL click interception on alternate screens
   to preserve Vim, Neovim, Codex, and other TUI interactions; do not infer
   application identity from titles or process names.

6. **Render compact labels beside URLs.** Pass labels and cell anchors for the
   active pane through the existing render infrastructure. Use semantic theme
   foreground/background/accent colors and existing font metrics, small padding,
   and a local readability floor if needed. Prefer adjacent free space, handle
   collisions and pane-edge fallback deterministically, and clip labels to the
   pane below the custom title bar. Avoid global dimming or a separate popup.
   Preserve surrounding alpha, native blur, and title-bar behavior. Recompute
   physical geometry from current pane origin, cell metrics, and DPI scale;
   ensure filtered candidates leave no stale labels.

7. **Add focused tests and run repository gates.** Cover wrapped URLs versus
   hard newlines, Unicode/wide/combining cells, delimiters, punctuation,
   viewport clipping, repeated occurrences, inactive panes, and alternate
   screens. Test hint counts at depth boundaries and a broad deterministic
   count range for uniqueness, alphabet membership, exact count, prefix
   freedom, and stable assignment. Test prefix filtering, Backspace, Escape,
   invalid input, repeats, binding precedence, PTY isolation, stale-snapshot
   cancellation, successful/failed opens, HTTP/HTTPS-only validation, click
   spans, drag suppression, and mouse-report passthrough. Cover label bounds,
   dark/light colors, integer/fractional scaling, and surrounding alpha.
   Run focused tests first, then:

   ```sh
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   cargo test --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   git diff --check
   ```

8. **Perform native KDE Wayland acceptance and document results.** Build the
   app and reuse existing native fixtures where practical. Exercise short and
   multi-character hints, filtering/backtracking/cancel, wrapped and Unicode
   URLs, actual default-browser dispatch, Ctrl+Click versus drag selection,
   split-pane scope, scrolling, resizing, and invalidated content. Verify
   Vim/Neovim/Codex interaction and mouse-reporting passthrough. Capture
   dark/light themes, opacity 0/0.85/1, blur on/off, custom title bar, and
   integer/fractional DPI. Run relevant native GPU alpha coverage. Store
   evidence under `docs/measurements/url-hint-mode/`, update meaningful product
   and configuration documentation, and report detection, assignment examples,
   input behavior, theme/opacity integration, native results, and limitations.

## Completion criteria and limitations to track

- All requested interactions work with stable, prefix-free hints and no hint
  keystrokes sent to the PTY; explicit binding precedence remains intact.
- Each visible occurrence has one hint, and clicking any wrapped span opens
  the same validated URL through the platform interface.
- Required checks and native acceptance have recorded outcomes; failures and
  unexercised platforms are reported honestly.
- Explicitly report conservative viewport-edge omissions, cancellation during
  changing output, alternate-screen/mouse-reporting policy, label fallback in
  dense content, and any URL grammar or platform-opening limitations.
- No commit or push.
