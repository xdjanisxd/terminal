# ANSI/VT compatibility v2

This bounded audit started from freshly fetched `master` (`be963f2`) on
`feat/terminal-compatibility-v2`, with a clean worktree. See the
[plan and scope](TERMINAL_COMPATIBILITY_V2_PLAN.md). No commit or push was made.

## Audit and priorities

The audit traced `parser.rs`, `state.rs`, screen-local state, replies, and the
existing tests. It reviewed the previous compatibility polish, bounded Unicode
grapheme work, reset/recovery audit, and Kitty protocol implementation.

| Area | Existing behavior / decision |
| --- | --- |
| Parsing and recovery | Incremental vte adapter; bounded OSC/CSI state; split UTF-8 recovery, CAN/SUB cancellation and saturated OSC rejection already implemented. Reused unchanged. |
| Cursor, editing, modes | CUP/HVP/CHA/VPA, relative movement, IL/DL/ICH/DCH/ECH, margins, origin, wrap, insert, screen modes 47/1047/1049, DECSC/DECRC, mouse/focus/paste, and reset paths already implemented. |
| Rendition and Unicode | Indexed/RGB semicolon and colon colors, basic underline, italic/faint/conceal/strike, bounded graphemes and whole-cell reflow/copy already implemented. No renderer changes needed. |
| Replies and keyboards | DA1/DA2, DSR, origin-relative ANSI CPR, bounded reply FIFO, per-screen Kitty stacks and legacy encoding already implemented. No new identity/capability claims or keyboard encoders. |
| **P1: DEC graphics** | Ignored G0/G1 designations printed `lqqk` as letters. Installed `xterm-256color` terminfo advertises `smacs=ESC ( 0`, `rmacs=ESC ( B`, and the identity `acsc` map. This is a concrete ncurses/TUI output gap. Implemented. |
| **P1: ED3** | Installed `clear` emits `CSI 3 J`; it previously left retained history available. Implemented without clearing the live grid. |
| **P2: VT/FF** | C0 VT/FF were ignored despite their specified LF-equivalent behavior. Implemented with the existing LF operation. |
| P2: REP, CHT/CBT | Installed terminfo advertises REP and CBT. Real remaining gaps, deferred to keep this pass bounded; REP also needs an explicit Unicode/grapheme repeat policy. |
| P2: modern queries | DECRQM/private CPR, DECRQSS and XTGETTCAP remain unsupported. No reproduced application blocker justified expanding bounded reply/DCS parsing in this pass. |
| P3: other protocols | Horizontal margins, selective erase/protection, keypad mode/routing, extra cursor-save protocols, G2/G3/NRCS and other character sets, richer underline and simultaneous bold+faint remain deferred. Their application impact needs separate evidence. |

This is supported protocol coverage, not full Vim/Neovim/tmux or ANSI/VT
conformance. Existing shell/editor/multiplexer byte fixtures remain the baseline.
The shell fixture's stale claim that OSC title was unsupported was corrected,
and it now asserts the already-supported title.

## Completed behavior

- `ESC ( 0/B` and `ESC ) 0/B` designate DEC Special Graphics or ASCII for
  G0/G1; SI/SO invoke G0/G1. Both default to ASCII. Unsupported sets, extra
  intermediates and canceled designations leave designation unchanged.
- Only ASCII `_` through `~` is translated; all other scalars use the existing
  Unicode path. The DEC blank maps to space; control pictures, scan lines and
  box drawing map to Unicode characters. Cells, selection/copy, history and
  reflow contain those Unicode values. The renderer owns no translation.
- Runtime character sets are terminal-global, like rendition/origin. The
  existing per-screen DECSC slots save both designations and invocation;
  DECRC/1049 restore them. DECSTR resets runtime sets and the active saved
  slot; RIS resets all. Existing delayed-wrap restore policy is unchanged.
- Non-private, single-parameter ED3 erases active Primary history and returns
  its viewport to live. It advances the history origin so live selection
  coordinates retain their identity. Selections touching deleted history are
  cleared; live selections, grid, cursor, rendition, saved cursor, pending
  wrap, margins, modes and replies are preserved. Alternate has no saved
  lines, so ED3 there leaves inactive Primary history alone. ED2 still erases
  the live display only. Private/malformed/extra/canceled forms are ignored.
- VT (`0x0b`) and FF (`0x0c`) call the same LF operation, including existing
  region, column, scrollback and pending-wrap behavior.

References: [xterm controls](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html),
[DEC VT100 DECSC and graphics](https://oldmanprogrammer.net/webapps/vt100/doc/vt100-ug/chapter3.html),
[DEC graphics table](https://www.zx.net.nz/computers/dec/vt220/doc/vt220-rm/table2-4.html),
[DECSTR defaults](https://www.zx.net.nz/computers/dec/vt220/doc/vt220-rm/table4-10.html).

## Validation

- 13 new deterministic regressions in `compatibility_v2.rs`: all byte splits
  of designation/shift and ED3 sequences, full graphics mapping, Unicode,
  canceled/malformed/unsupported input, per-screen saved slots, 1049 resize,
  resets, wrap/history/copy/reflow, ED3 live/deleted selection identity,
  repeated erase, cursor/rendition/reply preservation, and VT/FF vs LF on
  both screens, one-row screens and restricted margins.
- One additional allocation regression checks graphics, cursor save/restore
  and ED3, including nonempty history: zero allocations after initialization.
  Existing ASCII/grapheme and Kitty allocation tests also pass. Protocol state
  is three fixed bytes; no buffers, dependencies, threads or timers were added.
- `cargo check --workspace --all-targets --offline`,
  `cargo test --workspace --all-targets --offline` (887 passed, 31 ignored),
  `cargo test --workspace --doc --offline` (4 passed),
  `cargo clippy --workspace --all-targets --offline -- -D warnings`, and
  `cargo build --release -p terminal-app --bin terminal --offline` pass.
  `cargo fmt --all --check` and `git diff --check` also pass.
- Opt-in Linux `native_linux_terminfo_acs_and_clear_replay` passes with actual
  `tput`/`clear` output through the incremental parser. It requires installed
  `xterm-256color` terminfo and asserts ED3 output, graphics cells and cleared
  history. Run with `cargo test -p terminal-core --test compatibility_v2
  native_linux_terminfo --offline -- --ignored --nocapture`.
- Native KDE Wayland Vim and Neovim exact Unicode/space-path launch and exit
  checks pass. The real Neovim PTY Kitty test passes distinct Tab/Ctrl+I and
  Enter/Ctrl+M behavior and restoration to Primary legacy mode. These checks
  establish startup/protocol/lifecycle behavior, not visual acceptance of all
  application features.
- The opt-in Linux GPU readback test for backgrounds, glyphs, underlines and
  cursor passes with the available adapter. This does not establish native
  box-drawing appearance, other adapters or renderer throughput.

Raw logs and saved before/after release executables are in
`/tmp/terminal-compatibility-v2/`.

## Performance

Existing state and Unicode release benchmarks ran before edits. Saved before
and after executables were measured sequentially, with one warmup and 11
alternating retained runs per executable, without concurrent build/test load.
Unicode workloads have five inner samples per run. [Raw run metrics, ranges,
toolchain and host](measurements/terminal-compatibility-v2/benchmarks.json) are retained.

| Workload | Before median ns/op | After median ns/op | Change |
| --- | ---: | ---: | ---: |
| ASCII parser | 2,193.9 | 2,166.0 | -1.3% |
| Plain parser/state | 8,136.4 | 8,227.5 | +1.1% |
| Escape parser/state | 4,039.3 | 4,076.8 | +0.9% |
| Combining | 133.1 | 128.7 | -3.3% |
| ZWJ | 311.8 | 317.0 | +1.7% |
| Flag | 193.8 | 201.7 | +4.1% |
| Keycap | 247.5 | 241.3 | -2.5% |
| Resize columns, 10k history | 7,936,032.6 | 7,921,179.4 | -0.2% |
| Unchanged resize | 1.2 | 1.6 | +0.4 ns |
| Scrollback page pair | 3.9 | 3.9 | 0% |
| Selection extend | 4.0 | 4.0 | 0% |

Parser workload ranges overlap; ASCII throughput shows no observed regression.
The unchanged-resize microbenchmark has a measurable 0.4 ns increase and flag
parsing has a 4.1% median increase; no claim of universal performance equality
is made. Cell size stays 72 bytes; TerminalState grows from 1,280 to 1,296
bytes on this x86_64 build because saved/runtime character-set state affects
layout. Renderer code and cell representation are unchanged. These benches
exclude PTY scheduling, GPU presentation and input latency; rendering throughput
and end-to-end latency were not independently benchmarked in this pass.

## Manual acceptance checklist

Use a disposable pane; ED3 intentionally destroys its retained Primary history.
On Linux run these in Bash/Zsh. On Windows use PowerShell with `py -3` in
place of `python3` for the direct-byte samples.

1. Emit a DEC box and a SI/SO row:
   `python3 -c "import sys; sys.stdout.write('\x1b(0lqqk\r\nx  x\r\nmqqj\x1b(B\r\n\x1b)0\x0elqqk\x0fq\r\n')"`.
   Expect box corners/lines, then `┌──┐q`. Copy them as Unicode, resize, scroll
   through history, and repeat with CJK/combining/emoji text.
2. Populate history, scroll up and select across historical rows; emit
   `python3 -c "import sys; sys.stdout.write('\x1b[3J')"`.
   Expect empty history, live viewport, invalidated historical selection and
   unchanged live cells/cursor. Repeat with a live-only selection: it survives.
3. Run ncurses `clear`; verify both display and retained history clear. Enter
   Vim/Neovim or a pager alternate screen, leave it, and verify Primary output,
   cursor, Unicode and legacy/Kitty input recover. Check graphics save/restore
   and DECSTR/RIS in a separate disposable pane.
4. Exercise tmux splits, ncurses borders, mouse, paste and editor exit. Compare
   identical bytes in a reference terminal. tmux visual/interactive acceptance
   has not been completed by this audit.
5. Repeat on Linux KDE Wayland at 100%, 125% and 200%, light/dark and
   opaque/transparent settings; inspect box continuity, fallback glyphs,
   clipping, selection/copy and post-resize recovery.
6. Windows: repeat direct-byte probes and PowerShell prompt editing through
   ConPTY, Vim/Neovim entry/exit, Kitty/legacy keys, paste/mouse, saved cursor,
   resize/history and DX12 rendering at multiple scales. WSL/tmux is optional
   if installed. Native Windows, ConPTY, physical-key and visual verification
   remain pending; no Windows execution or cross-compilation was performed.
