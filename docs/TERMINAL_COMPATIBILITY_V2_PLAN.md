# ANSI/VT compatibility v2

Base: freshly fetched `master` at `be963f2`; existing task branch
`feat/terminal-compatibility-v2`, initially clean. No commits or pushes.
Goal tracking is active. Plan Mode has no activation tool in this session;
this written plan records the investigation, decisions, and validation.

## Plan

1. Audit parser dispatch, mode/reply ownership, screen/saved-cursor state, and
   existing compatibility, reset/recovery, Unicode, and Kitty work.
2. Reproduce a small set of missing behaviors against specifications and local
   application/terminfo output; capture release benchmark executables first.
3. Add focused failing regressions, then implement semantics in terminal-core.
4. Validate split/malformed input, screen isolation, save/restore, reset,
   selection, history, resize/reflow, Unicode, and existing keyboard behavior.
5. Compare alternating before/after release benchmarks, run workspace tests,
   formatting, Clippy, release build, and available Linux PTY/native checks.
6. Update compatibility documentation and roadmap only for finished behavior;
   disclose platform and manual acceptance limits.

## Selected scope

- P1: DEC Special Graphics G0/G1 designation (`ESC ( 0/B`, `ESC ) 0/B`)
  and SI/SO invocation. Local `xterm-256color` terminfo uses `ESC ( 0/B`
  for `smacs/rmacs`; ignored designation currently prints `lqqk` instead of a
  box border. Save designation/invocation in the existing per-screen DECSC
  slot; preserve terminal-global runtime character-set policy and reset it
  on DECSTR/RIS. Unicode must bypass ASCII translation.
- P1: ED3 (`CSI 3 J`), emitted by local `clear`, removes only active Primary
  history. Preserve live grid, cursor, rendition, delayed wrap, saved cursor,
  live selections, and inactive Primary history when Alternate is active.
  Invalidate selections touching removed rows; return viewport to live.
- P2: VT/FF C0 controls alias LF, using the existing line-feed semantics.

Deferred: REP (needs Unicode/grapheme policy), CHT/CBT, DECRQM/private CPR,
DECRQSS/XTGETTCAP, horizontal margins, selective erase, additional character
sets, keypad routing, richer underline and simultaneous bold/faint. Existing
DA1/DA2/DSR/CPR, Kitty, OSC, RGB, grapheme, and reset implementations remain
the integration baseline. No renderer, app routing, config, dependency,
public export, thread, timer, or buffer-size changes are planned.

Scope reaches parser dispatch, TerminalState character-set/reset/save/restore
and history semantics, ScreenSet saved state/history, focused tests, existing
fixtures, and docs. ED3 intentionally destroys requested history; malformed
variants must not do so. No general terminal conformance claim is implied.

References: [xterm controls](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html),
[DEC VT100 guide](https://oldmanprogrammer.net/webapps/vt100/doc/vt100-ug/chapter3.html),
[DEC VT220 reset table](https://www.zx.net.nz/computers/dec/vt220/doc/vt220-rm/table4-10.html).

## Status

- Audit, scope selection, implementation and focused regressions complete.
- Workspace checks/tests/doctests, Clippy and release build pass; release
  before/after benchmark runs and Linux terminfo, native editor and Kitty
  checks complete. GPU readback, final formatting/diff review and evidence
  documentation complete; see [results and acceptance](TERMINAL_COMPATIBILITY_V2.md).
- Native Windows and interactive visual/physical-key acceptance remain pending.
