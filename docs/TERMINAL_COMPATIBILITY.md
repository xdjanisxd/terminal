# Terminal compatibility polish

This pass keeps protocol semantics in `terminal-core` and projects resolved attributes in `terminal-renderer`. The working tree was clean on `feat/terminal-compatibility-polish` before edits. Plan Mode could not be activated in the available session; inspection and the following bounded plan preceded implementation.

1. Trace existing parsing, cell attributes, saved cursors, margins, resets, and renderer caches.
2. Add visible italic/faint, atomic colon colors, bounded missing attributes, DECOM, and explicit reset dispatch.
3. Test split input, malformed payloads, screen/history/selection invariants, and raster/GPU style identity.
4. Run workspace checks, tests, Clippy, GPU readback, release builds, and before/after benchmarks; record deliberate deferrals.

## Implemented behavior

| Area | Behavior |
| --- | --- |
| Italic | SGR 3/23 projects italic state. Swash shears outlines by 0.2 around the baseline with unchanged shaping advances, font metrics, existing cell clipping, native Bold selection, and variable weights. CPU bitmap and GPU atlas keys include italic state. Upright shaping cache entries can be shared because this transform is applied during rasterization. |
| Faint | SGR 2 multiplies resolved foreground sRGB channels by 0.66 after inverse resolution and before selection colors. Alpha and backgrounds are preserved. Existing ordered/exclusive normal/bold/faint state and SGR 22 are preserved. |
| Colon colors | Foreground/background accept `38/48:5:n`, `38/48:2:r:g:b`, and `38/48:2::r:g:b`; explicit zero color-space is also accepted. Components must fit bytes. Nonzero color spaces and malformed/extra subparameters are atomic no-ops. Existing semicolon colors retain grouped validation. |
| Additional attributes | SGR 8/28 conceal/reveal and 9/29 strike-through/reset survive cell snapshots, edits, history, and saved rendition. Conceal suppresses glyphs and decorations while retaining text for selection/copy. Strike-through uses existing rectangle rendering, including on spaces. `4:0/1` maps to the existing single underline. Deferred SGR 58 semicolon color groups consume payload without changing other styles. |
| DECOM | `CSI ? 6 h/l` homes and cancels delayed wrap. CUP/HVP/VPA rows become relative to the active top margin; vertical cursor movements clamp to margins. DECSTBM homes to the selected origin. ANSI CPR rows become origin-relative. Existing origin-disabled behavior remains. |
| Saved cursor | `ESC 7/8` routes to per-screen saved cursor operations. Slots additionally save origin mode; restore uses physical coordinates clamped to current margins/dimensions, restores rendition/origin, and cancels pending wrap. `?1049` uses the same slots. Uninitialized restore remains a no-op. |
| RIS | `ESC c` clears both buffers, Primary history/viewport, selection, saved cursors, hyperlink state, keyboard reporting stacks, and pending clipboard requests. It selects Primary and restores startup rendition, modes, tabs, and margins. Dimensions, already-generated replies, and shell title/directory metadata with version counters are retained. |
| DECSTR | `CSI ! p` (also default zero parameter) preserves both buffers, history/viewport, selection, tabs, and active cursor position. It resets rendition, terminal modes, cursor-key mode, current hyperlink, active margins/pending wrap, and active saved cursor to home/default. Inactive saved state and other input reporting requests remain. Auto-wrap follows this emulator's enabled startup default; the VT510 manual lists a different default. |

`TerminalState::set_cursor_position` remains an absolute checked physical setter for application/state callers. Origin mode is terminal-global; margins, cursors, and saved slots remain screen-local. Buffer switching preserves the established `?47`, `?1047`, and `?1049` contracts. Resize retains the existing full-height margin reset, history widths, and cursor clamping policy.

Projection skips undecorated, unselected default-background blanks before resolving their colors. The later resolved-background check still handles explicit colors equal to the theme. This retains sparse draw data and avoids an observed projection slowdown from the additional style handling. Core `Cell` storage remains 72 bytes on this x86_64 build.

Protocol references: [xterm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html), DEC [DECOM](https://vt100.net/docs/vt510-rm/DECOM.html), [DECSTR](https://vt100.net/docs/vt510-rm/DECSTR.html), and [RIS](https://vt100.net/docs/vt510-rm/RIS.html). The emulator reset boundaries above are explicit policy for the supported model; hardware communication/NVR behavior does not apply.

## Validation

The later [reset/recovery audit](TERMINAL_RESET_RECOVERY.md) preserves these reset policies and adds malformed split UTF-8 recovery, rejection of canceled/saturated OSC callbacks, cursor-default/Kitty reset regressions, and further Linux lifecycle/editor evidence. Both reset types restore configured cursor defaults and clear both Kitty reporting stacks, invalidating held events.

- Focused `terminal-core` and `terminal-renderer` suites pass, including 14 additional regression tests. Existing tests asserting colon colors were unsupported were updated for the newly implemented forms.
- Regression coverage includes every byte split in representative sequences; malformed/private reset and origin forms; origin homing, addressing, CPR, save/restore, resize, one-row screens, region scrolling, and alternate-screen round trips; reset preservation/clearing boundaries; dim/inverse/opacity/selection colors; wide/combining text and history; italic/bold/size/cache isolation.
- `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build --release -p terminal-app` pass. The all-target test run passed 760 tests with 23 environment-dependent tests ignored. Four compile-fail ownership doctests also pass through `cargo test --workspace`.
- Six opt-in Linux wgpu tests pass, including GPU readback on linear/sRGB targets, opacity, palette/hint surrounding alpha, style pixel differences, and weight/italic atlas separation. A native variable-font Unicode fallback rasterization test also passes. These use `FONTCONFIG_FILE=/tmp/compat-fonts.conf` including the repository font fixtures; the first atlas run without that setup failed because JetBrains Mono was unavailable. No font was installed or system configuration changed.
- Native Linux window/compositor interaction, Windows execution, other adapters, and full application acceptance were not exercised in this pass. GPU tests may select any available wgpu adapter; no hardware-wide claim follows.

### Benchmarks

Both existing release benchmarks ran before and after changes. Saved pre-change executables were compared with current executables in alternating order, with one warmup and 11 retained samples per executable. Host: Linux x86_64, Intel Core i5-1250P. [Raw samples, ranges, toolchain, and base revision](measurements/terminal-compatibility-polish/benchmarks.json) are retained.

| Workload | Before median ns/op | After median ns/op | Change |
| --- | ---: | ---: | ---: |
| Plain parser/state | 11,017.5 | 10,937.3 | -0.7% |
| Escape parser/state | 5,186.6 | 5,196.4 | +0.2% |
| Scrollback page pair | 3.9 | 3.9 | 0% |
| Selection extend | 4.0 | 4.0 | 0% |
| Visible 80×30 projection | 19,345.9 | 17,643.7 | -8.8% |
| Echo plus projection | 18,640.8 | 16,062.6 | -13.8% |
| Three-pane layout/projection | 21,175.2 | 20,312.1 | -4.1% |

No material slowdown was observed in these workloads after the blank skip adjustment. The ranges overlap and timings vary by host load; these results do not establish end-to-end performance. Existing benches exclude PTY scheduling, font work, GPU upload/present, and input latency. No timing thresholds were added.

## Deliberately deferred

- Native Italic/Bold Italic companion discovery and variable italic/slant axes. Synthetic italics retain existing fixed cell clipping; edge overhang can be clipped, including at small sizes or fractional DPI.
- Blink, double/curly/dotted underlines, underline colors, overline, independent simultaneous bold+faint, non-RGB color spaces, optional color tolerances, and mixed semicolon/colon color payload forms.
- DECSCA/selective erase, horizontal margins, additional cursor-save protocols, private CPR/query variants, DEC graphics character sets, and broader OSC/DCS/CSI coverage. Existing saved-cursor policy still cancels delayed wrap and does not save every historical DEC state field.
- Full grapheme/ZWJ shaping, fuzz execution, Windows native verification, other GPUs, and measured input-to-present latency.

## Recommended manual checks

Use a fresh disposable pane for reset checks. On Linux use Bash/Zsh and Python 3 (`python3`); on Windows use PowerShell and Python (`py -3`, or `python`). `sys.stdout.write` emits the control bytes directly on either platform.

1. Emit the following sample (substitute the platform's Python command). Check visible italic/faint, normal resets, strike-through, hidden text spacing, underline, bright/indexed/RGB colors, CJK, and combining marks:

   ```sh
   python3 -c "import sys; sys.stdout.write('\x1b[0mNormal | \x1b[3mItalic | \x1b[1mBold italic | \x1b[22;2mFaint italic | \x1b[0mReset\r\n\x1b[4;9mUnderline + strike\x1b[0m | \x1b[8mhidden\x1b[28m visible\r\n\x1b[38:2::120:169:255mRGB | \x1b[38:5:196mIndexed | \x1b[93mBright\x1b[0m | \x1b[3m界 e\u0301\x1b[0m\r\n'); sys.stdout.flush()"
   ```

2. Repeat at 100%, 125%, and 200% display scale, several font sizes, opaque/transparent backgrounds, light/dark themes, and after resizing. Compare identical bytes with a reference terminal. Check normal/bold rendering, spacing, clipping, selection/copy, history navigation, and app overlays.
3. In a pane at least eight rows tall, emit the next sequence. `TOP` and `BOTTOM` should remain outside the region; `REGION` appears at screen row 3. Cursor save/restore should place `SAVED` at the same origin-relative row. The final sequences restore full-screen origin and margins:

   ```sh
   python3 -c "import sys; sys.stdout.write('\x1b[2J\x1b[HTOP\x1b[8;1HBOTTOM\x1b[3;6r\x1b[?6hREGION\x1b7\x1b[99B\x1b8SAVED\x1b[?6l\x1b[r\x1b[9;1H'); sys.stdout.flush()"
   ```

4. Populate history, enter an editor/TUI alternate screen, then emit `\x1b[!p` from a controlled program. Check retained content/cursor/history/tabs, normal rendition, visible cursor, and default margins. In a fresh pane emit `\x1bc`; check blank Primary, empty history, default modes/tabs, and no stale selection. Do not run reset sequences inside valuable interactive sessions.
5. Exercise Vim/Neovim, a pager, a multiplexer, shell prompt highlighting, mouse reporting, paste, tabs/splits, and selection. Confirm alternate-screen exit restores Primary output/cursor/rendition, restricted scrolling does not enter history, and existing truecolor/background erasure remains correct.
6. On Windows also check PowerShell prompt redraw, ConPTY output, DX12 rendering, display-scale transitions, and font-size changes. On Linux check the normal Wayland/X11 session, compositor opacity/blur, font fallback, and fractional scaling. These are recommended acceptance checks, not claimed results.
