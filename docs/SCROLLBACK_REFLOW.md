# Scrollback reflow

Primary screen column changes repack history and live cells into the new
width. Recorded soft wraps join into logical lines; hard line breaks remain.
The transformation copies complete `Cell` values, retaining colors, styles,
OSC 8 targets, explicit spaces and ordered combining marks. A wide pair moves
together. The unused right-edge space before a wrapped wide glyph is omitted
using the recorded wrap width.

`terminal-core/src/reflow.rs` packs cells and maps a small set of anchors;
`screens.rs` owns the history/grid boundary, eviction and cursor policy.
Selection remains terminal-owned. App resize batching, PTY sizing and renderer
projection consume the resulting state through their existing interfaces.
The renderer does not reflow text.

## Positions and screens

- The live cursor is mapped as an insertion boundary, including delayed wrap.
  A boundary at the new right edge leaves the cursor on the final cell with
  delayed wrap pending. Further printing continues after the retained text.
- Primary's saved cursor follows its corresponding position and is clamped
  into the new live grid. Restore retains its existing policy of cancelling
  delayed wrap and restoring rendition/origin mode.
- Mouse, word, line and keyboard selection endpoints follow retained cells.
  Copy joins soft wraps, preserves spaces inside a logical line and keeps hard
  breaks. Keyboard copy excludes erased end padding before hard breaks while
  retaining explicit spaces. Mouse copy still trims trailing spaces at hard
  line/selection ends.
  Fully evicted selections disappear; partially retained selections clip to
  the retained range. Dragging can continue from mapped anchors.
- A scrolled viewport follows the old top row's first cell. After widening,
  that cell may land inside the new top row because scrolling remains vertical
  and row-based. Live view continues following the live bottom.
- Height shrink moves top rows into history as needed to keep the cursor
  visible; height growth pulls recent history back into the grid. This path
  copies the live rectangle without scanning history. Selection's absolute
  history coordinates remain stable during row transfer.
- Alternate retains rectangular clipping and an independent cursor/saved slot,
  never gains history, and never joins or rewraps TUI rows. Its surviving
  selection coordinates are clamped. Inactive Primary still reflows during an
  Alternate resize, including the saved position used by DEC 1049 restore.
- Changed sizes reset both screens' vertical margins and resize shared tab
  stops. An unchanged size preserves all state without allocating.

## Bounds, small dimensions and limitations

History retains at most 10,000 **physical rows after reflow**. Narrowing can
therefore evict older text even when no new output arrives. Oldest rows are
evicted deterministically; a partially retained soft line is not reconstructed
from missing data. Widening cannot recover evicted text.

The streaming builder drains old history incrementally and retains at most
the history capacity plus the new screen height and
`max(old columns, old rows)` scratch rows. It reuses evicted output buffers
during large expansions. No flattened copy of the entire transcript or
per-cell position map is built. Ordinary cell runs copy in slices; only
packing boundaries and one-column wide glyphs need special handling. Compact
historical rows omit default erased padding; viewport access to omitted cells
returns `None`, which clients treat as a default blank. Explicit spaces and
non-default attributes are retained.

One-column grids retain a wide scalar as a single cell with private width
metadata, keeping its attributes, link and combining marks. Reflow restores a
normal wide pair once two columns are available. New wide output at one column
uses the same representation. Existing Unicode limitations still apply: this
does not add grapheme clustering or ZWJ composition.

When shrinking with content below the cursor, keeping both the cursor and all
lower rows visible is impossible. Rows below the cursor that cannot fit are
clipped from the bottom, matching the rectangular resize policy for TUIs.
Unused blank rows below the cursor absorb expansion before it scrolls history.
Saved positions moved out of the live area clamp to the screen boundary.

## Automated verification

`crates/terminal-core/tests/reflow.rs` covers hard breaks and blank lines,
explicit spaces, wide gaps, combining marks, styles/links, live and saved
cursors, both selection directions and keyboard selection, viewport anchors,
height transfers, one-column output, eviction, inactive Primary/Alternate
isolation, and deterministic repeated resize sequences. Renderer snapshot
coverage verifies that mapped selection, Unicode payloads and cursor positions
reach projection. Existing screen/history, URL visibility and scrollbar tests
now assert the reflow contract.

From the repository root:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --offline
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo build --workspace --release --offline
cargo bench -p terminal-core --bench state_baselines --offline
cargo bench -p terminal-renderer --bench projection_baselines --offline
```

`--workspace` covers every crate; `--all-targets` includes test/benchmark
compilation; `--offline` uses cached dependencies; `--release` enables optimized
builds. `-p` selects a package and `--bench` selects its benchmark. The `--`
passes `-D warnings` to Clippy so warnings fail validation. Formatting's
`--check` reports differences without editing files.

Resize benchmarks measure 80↔60 columns, 30↔29 rows and unchanged dimensions
with empty, partial and nearly full history, plus a repeated Unicode 80↔1
column case. Fixtures are populated outside the timed loop. Measurements are
core CPU costs, not native pointer-to-frame latency or GPU/compositor timings.
See [PERFORMANCE.md](PERFORMANCE.md) for measured results.

## Manual acceptance: Linux and Windows

These visual/interactive checks remain pending on both platforms. Automated
core and snapshot checks do not substitute for compositor or ConPTY acceptance.

Build the release binary using the command above, then launch
`target/release/terminal` on Linux or `target\release\terminal.exe` on Windows.
Record OS, shell, display scale, font and approximate starting grid size.

For Linux Bash/Zsh, generate mixed long and hard-broken output:

```sh
python3 -c 'for i in range(200): print(f"\x1b[1;32m{i:03} \x1b[0m" + "ab界e\u0301  " * 18 + " END\nHARD\n")'
```

For Windows PowerShell:

```powershell
$esc = [char]27
$sample = "ab$([char]0x754c)e$([char]0x301)  "
1..200 | ForEach-Object { "${esc}[1;32m$_ ${esc}[0m" + ($sample * 18) + " END`nHARD`n" }
```

On each platform:

1. Shrink/grow repeatedly, change height independently, and use the smallest
   supported window. Check ordered text, `END`/`HARD` boundaries and blank
   lines; confirm wide and accented characters and colors stay intact.
2. Page up to a numbered historical line, resize repeatedly, and confirm its
   text remains at the top row (possibly at a different column after widening).
   Page down and confirm live view/cursor follow output.
3. Select across wrapped text in both directions, copy before/after resize,
   and compare the pasted strings. Include the two internal spaces, a wide
   glyph and its accent. Repeat with word/line selection and Shift+arrows;
   resume dragging after resize and check highlights match copied text.
4. Type a long command at the prompt, shrink/grow, then type, Backspace and
   execute it. Check insertion position and the shell's resize repaint. Repeat
   with a line ending exactly at the right edge.
5. Run Vim/Neovim (and a fullscreen monitor where available), resize in both
   directions, then exit. Confirm the TUI redraws correctly, Primary output is
   retained/reflowed, and the prompt cursor/rendition are usable. On Windows
   repeat with PowerShell and cmd through ConPTY.
6. Generate over 10,000 rows, scroll near the oldest retained content, and
   narrow repeatedly. Confirm bounded eviction, no stale selection, and no
   crash. Repeat in split panes, including output in an unfocused pane.

Compare equivalent resize gestures on the same machine/scale with diagnostics
disabled. If recording pacing, use the existing renderer diagnostics described
in [PERFORMANCE.md](PERFORMANCE.md), preserving core reflow time separately
from PTY repaint, surface configuration and presentation.
