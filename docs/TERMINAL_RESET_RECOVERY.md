# Terminal reset and malformed-input recovery

The audit started from clean `fix/terminal-reset-recovery` at `009e2bb`, equal to fetched `origin/master`. Existing reset policies from the compatibility, cursor-style, and Kitty keyboard changes remain intact. No reset-state or PTY lifecycle rewrite was needed.

## Confirmed bugs and fixes

- **Lost output after malformed split UTF-8:** `TerminalParser::advance` can report zero bytes consumed when it clears a UTF-8 prefix from an earlier chunk. The app previously stopped processing that PTY chunk, dropping its valid remainder, including RIS/DECSTR. The app now retries the same unconsumed byte. The parser clears the prefix before returning, so the retry progresses; no retry queue or new state is needed. Positive-consumption errors still skip exactly the consumed bytes. Existing error diagnostics and reply FIFO handling are retained.
- **Canceled OSC committed state:** vte 0.15 invokes `osc_dispatch` on CAN/SUB, before calling `execute`. Ignoring those controls in `execute` was too late: canceled titles, working directories, hyperlinks, clipboard requests, and prompt markers had already been applied. The existing per-byte adapter now supplies the dispatching byte to its performer, which rejects canceled OSC callbacks before semantic effects.
- **Truncated OSC committed state:** vte's bounded raw buffer and parameter array silently truncate payloads without an overflow flag. Long OSC 7/8 payloads could therefore install incomplete directories or hyperlinks; excess semicolon fields could install a truncated title. OSC dispatch now rejects saturated storage before applying any payload.

Failing regressions preceded production edits: canceled OSC installed `canceled` as the title; a 4,096-byte directory installed its truncated prefix; app RIS/DECSTR tests lost their following text. The raw core failure log is `/tmp/reset-recovery-core-before.log` for this run.

## Preserved reset policy

| State | DECSTR (`CSI ! p`, default zero accepted) | RIS (`ESC c`) |
| --- | --- | --- |
| Active buffer/cells/cursor | Preserve | Select blank Primary, home cursor; blank Alternate too |
| Primary history, viewport, selection | Preserve | Clear |
| Dimensions | Preserve | Preserve |
| Rendition/current hyperlink | Reset; existing linked cells remain linked | Reset and clear link targets |
| Terminal modes | Startup defaults; auto-wrap enabled | Startup defaults |
| Configured cursor shape/blink | Restore latest configured default | Restore latest configured default |
| Cursor-key mode | Normal | Normal |
| Mouse/focus/paste/modifyOtherKeys reporting | Preserve | Reset |
| Kitty reporting | Clear both screen stacks; invalidate held events | Clear both screen stacks; invalidate held events |
| Margins/pending wrap/saved cursor | Reset active screen only; saved slot becomes home/default | Reset both screens; saved slots uninitialized |
| Tabs | Preserve | Default eight-column stops |
| Shell title/directory/version counters, generated replies | Preserve | Preserve |
| Pending OSC 52 clipboard write | Preserve | Clear |

Inactive screen-local saved state remains independent under DECSTR. Switching through `?47`, `?1047`, and `?1049` retains its existing semantics. Core owns these policies; the renderer and PTY lifecycle are unchanged.

## Recovery boundaries and performance

Normal BEL and ESC-terminated OSC behavior is retained. CAN (`0x18`) and SUB (`0x1a`) abort payloads. Existing vte ESC handling also lets a new escape sequence interrupt a string; no timeout or heuristic terminates incomplete input. A genuinely incomplete string can consume subsequent printable bytes until termination/cancellation, as required by the parser's existing framing. Send CAN followed by RIS to explicitly resynchronize a disposable session.

OSC accepts at most 1,023 stored bytes, including command bytes but excluding semicolon separators, and at most 15 fields including the command. Exactly 1,024 stored bytes or 16 fields are rejected conservatively because the callback cannot distinguish exact fit from truncation. Existing tighter per-command limits, including 512 decoded OSC 52 bytes and 512 title bytes, still apply. The field-capacity constant mirrors vte 0.15 and must be reviewed on dependency updates; boundary regressions cover it.

The fix adds no heap allocation, persistent buffering, background work, timer, or thread. OSC validation scans at most 15 parameter lengths only when OSC dispatches. The existing byte loop supplies one byte of callback context; it does not add a scan or semantic processing for plain text. App recovery changes only its error path. Existing queues, parameter storage, history and combining-mark bounds remain in force. Existing diagnostic logging on malformed input remains synchronous and can be expensive under a sustained invalid stream.

Release `state_baselines` executables were saved before/after the core fix and run in alternating order, eight samples each, discarding the first sample of each executable. [Raw parser samples](measurements/terminal-reset-recovery/benchmarks.json) record medians: plain parser/state 8,248.7 → 7,935.2 ns/op; escape parser/state 4,047.9 → 3,988.1 ns/op. No slowdown was observed in these samples; host noise prevents claiming a speedup or end-to-end latency improvement. Benchmarks ran before broad/native validation and are not timing assertions.

## Verification

Focused regressions exercise every split through canceled OSC and malformed UTF-8/reset streams; existing metadata/link preservation; OSC byte/field capacity boundaries with BEL/ST and fragmented input; cancellation from CSI, OSC, DCS, SOS, PM, APC and escape-intermediate states; RIS interruption; 1,536 byte/prefix recovery cases with an explicit finite retry bound; wide/combining cells; repeated soft/full resets after resizing; current configured cursor defaults; and both Kitty screen stacks.

Existing compatibility, state, saved-cursor, screen-switching, scrollback, selection, Unicode, rendition, cursor-style, keyboard and reply tests cover the reset preservation/clearing boundaries above. Native lifecycle checks exercise disposable Bash/Zsh windows; editor checks exercise real Vim/Neovim with Unicode/space paths. Neither establishes physical keyboard behavior, visual reset acceptance, or Windows correctness.

- `cargo test --workspace --all-targets --offline`: 853 tests passed, 28 environment-dependent tests ignored; existing state/projection benchmark targets also completed.
- `cargo test --workspace --offline`: 857 passed including four compile-fail doctests, 28 ignored; no failures.
- Formatting check, workspace/all-target check, Clippy with `-D warnings`, and release application build pass.
- Opt-in Linux native shell-exit validation passes all Bash/Zsh scenarios (single zero/nonzero exit, split, tabs, mixed and startup command); Vim and Neovim exact Unicode/space-path open/exit checks pass. Raw native logs and initial core failure output are retained in [measurements](measurements/terminal-reset-recovery/).
- No Windows host, physical-key acceptance, manual visual reset acceptance, sustained fuzz run, or other GPU/platform execution was performed. Rendering and PTY production code were not changed; lifecycle/editor checks do not imply visual reset correctness.

Protocol framing context: [xterm control-sequence definitions](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html). The existing [project compatibility policy](TERMINAL_COMPATIBILITY.md) defines the reset boundaries rather than claiming complete historical hardware reset emulation.

## Linux and Windows manual acceptance checklist

Use disposable panes, first opaque and then with configured opacity, at normal and fractional scale. On Linux run Python with `python3`; on Windows use `py -3` (or `python`) in PowerShell. Emit bytes with `sys.stdout.buffer.write(...)` and flush, rather than relying on shell escape expansion.

- [ ] **Soft reset:** populate Primary history and Unicode text, select/copy text, customize tab stops, enter Alternate, change rendition/margins/origin/cursor style, hide the cursor, and enable Kitty. Emit `b'\x1b[!p'`. Check retained active screen, cells, cursor position, history/selection/tabs, default rendition/margins/visible configured cursor, cleared Kitty reporting, and retained mouse/focus/paste requests. Exit Alternate and check Primary recovery.
- [ ] **Full reset:** in a fresh pane with the same setup, emit `b'\x1bc'`. Check blank Primary, home cursor, empty history/selection, blank Alternate, startup modes/tabs/margins, configured cursor style, retained shell metadata, and ordinary typing/paste.
- [ ] **Malformed stream:** emit `b'\xe2'`, flush, then emit `b'\x1bcRECOVERED\r\n'`. Check `RECOVERED` survives. Repeat with DECSTR, invalid bytes, and a subsequent shell prompt/editor launch. Chunk boundaries from a real PTY are nondeterministic; automated helper tests provide deterministic split evidence.
- [ ] **Cancellation:** emit `b'\x1b]2;CANCELED\x18RECOVERED\r\n'`. Check the title remains unchanged and text is displayed. Repeat with SUB, an incomplete CSI, and an unsupported DCS. Confirm later valid title/SGR/cursor sequences work.
- [ ] **Compatibility:** exercise Vim/Neovim, pager and multiplexer, alternate-screen exit, wide/combining selection and copy, scrollback after resize, mouse/focus reporting, bracketed paste, shortcuts and Kitty press/release behavior. Reset one split/background pane and confirm other sessions are unaffected.
- [ ] **Linux:** repeat in the supported KDE Wayland session, check Bash/Zsh prompt redraw and configured cursor blink/shape, then exit panes/tabs and verify no orphan PTY/session workers.
- [ ] **Windows:** repeat with PowerShell/ConPTY and DX12, check prompt redraw, resize/DPI transitions, cursor defaults and pane/tab exit cleanup. Windows and physical-key acceptance remain manual and unverified in this Linux run.
