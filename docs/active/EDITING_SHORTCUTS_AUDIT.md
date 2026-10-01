# Editing shortcuts audit and plan

Audited on `feat/editing-shortcuts-polish` before implementation (same HEAD as master).

## Existing behavior

The app resolves overlays, then exact configured physical-key bindings, then PTY input. Custom `[[bindings]]` replaces the entire default list. All requested chords are configurable, but commands cannot currently send arbitrary bytes.

| Key | Unbound behavior before this change |
| --- | --- |
| Delete / Ctrl+Delete | Both send ESC [ 3 ~; modifiers are discarded |
| Backspace / Ctrl+Backspace / Alt+Backspace | All send DEL (0x7f); named key overrides committed text |
| Left / Right | CSI D/C, or SS3 D/C in application cursor mode |
| Ctrl+Left / Ctrl+Right | CSI 1;5 D/C in either cursor mode |
| Shift+Left / Shift+Right | Plain arrow encoding; Shift is discarded |
| Ctrl+Shift+Left / Right | Default PreviousPane / NextPane; unbound sends plain arrow |

Backspace encoding is a platform helper returning DEL on both platforms; there is no separate backspace-byte configuration. Explicit Backspace bindings are supported.

Mouse cell/word/line selection uses one core Selection with inclusive endpoints and absolute history rows. Wide continuation cells normalize to their lead and copying includes combining marks. Copy uses `selected_text`. Screen transitions clear selection. Mouse selection is also available on alternate screen when mouse reporting is off (or Shift overrides reporting). Keyboard selection does not exist. Core currently has no retained soft-wrap row metadata; copy inserts newlines between all rows.

Word selection classifies alphanumerics and `_ . / \\ : ~ @ - $ % +` together, whitespace separately, and other punctuation separately. Ctrl+Left/Right has no local word implementation: the foreground application interprets CSI 1;5 D/C. Thus shell and selection semantics can legitimately differ for identifiers, flags and paths.

Supported application signals are alternate screen, application cursor mode and mouse reporting. Kitty/CSI-u and modifyOtherKeys reporting are not implemented. Primary screen alone cannot establish that a shell owns input (tmux and TUIs can use it).

Interactive PowerShell launches have an existing prompt/OSC integration. PSReadLine Windows defaults bind Ctrl+Delete to KillWord, Ctrl+Backspace and Ctrl+W to BackwardKillWord, and Alt+D to KillWord. GNU readline Emacs defaults use Meta+d and Meta+DEL. User shell bindings and edit mode remain authoritative.

Sources: [PSReadLine bindings](https://github.com/PowerShell/PSReadLine/blob/master/PSReadLine/KeyBindings.cs), [GNU readline killing commands](https://www.gnu.org/software/bash/manual/html_node/Readline-Killing-Commands.html).

## Plan

1. Add core modifier-aware encodings; preserve ordinary keys and application ownership. Use existing shell integration boundaries if native protocol differences require it.
2. Extend the existing Selection with caret boundaries for keyboard movement, retaining mouse endpoint semantics. Reuse word classification. Retain soft-wrap metadata with rows for exact copying.
3. Add selection commands/default bindings, replacing the conflicting pane-arrow defaults (Ctrl+Shift+H/L remain). Commands forward modified arrows when application modes own input. Custom binding lists still replace defaults.
4. Cover encoding, binding precedence, selection contraction/reversal, row/wrap boundaries, Unicode, scrollback and mouse interoperability with deterministic tests.
5. Build release, attempt available native PowerShell/TUI/Linux acceptance, run all requested validation, and document limitations honestly. No commit or push.

Core owns selection and input protocols; app owns bindings and routing; shell policy remains in app/shell. Renderer and PTY lifecycle need no changes. No new dependency or architectural decision is needed.

## Implemented outcome

The implementation, deterministic validation, native protocol probes and manual graphical acceptance are complete. The user reported manual native acceptance passed on 2026-10-01.

- Exact Ctrl+Delete sends Meta+d (ESC d) and exact Ctrl+Backspace sends Ctrl+W (0x17) when shell-style input owns the shortcut. Normal Delete remains CSI 3~ and normal Backspace remains the existing DEL encoding. Alt+Backspace now preserves Meta+DEL. No repeated keys or local buffer editing are used.
- Terminal ownership requires Primary screen, normal cursor mode, no mouse reporting and no requested keyboard reporting. Otherwise selection commands forward modified arrows, Ctrl+Delete sends CSI 3;5~, and Ctrl+Backspace sends BS or the requested modified-key representation. Kitty reporting flags are tracked with screen-local stacks and modifyOtherKeys requests are tracked to avoid consuming application input; complete implementations of those protocols are outside this task.
- Four configurable selection commands replace the former Ctrl+Shift+arrow pane defaults. Existing exact binding precedence and replacement of the entire default set are unchanged. Ctrl+Shift+H/J/K/L pane navigation remains available.
- Keyboard selection uses the existing Selection, absolute scrollback coordinates and existing mouse word classes. Caret endpoints contract and reverse naturally, skip wide continuation cells, preserve attached combining marks, and copy soft-wrapped rows without added newlines or wide-character padding. Mouse selection retains its existing inclusive endpoints and copy behavior.
- Ctrl+Left/Right continues to use shell-defined navigation. Terminal word selection treats identifiers, hyphens, dots and path punctuation in the existing word class as one run. Shell deletion/navigation can differ: GNU Ctrl+W uses whitespace boundaries, while Meta+d and PSReadLine use their own word rules. No second shell word model is introduced.

## Validation and acceptance

Ten deterministic core tests cover character and word extension, contraction/reversal, hard and soft row boundaries, pending wrap, wide characters and attached combining marks, paths/punctuation, history coordinates, mouse interoperability, alternate-screen rejection and reporting-mode lifecycle. App tests cover routing, modifiers, six explicit binding overrides, default-set replacement and shared selection dispatch. Existing regression suites remain passing.

All requested completion gates passed on Windows: fmt, workspace all-target check, workspace all-target tests, workspace doctests (4), warning-denying workspace Clippy and diff check. Four existing benchmark tests remain ignored. A release `terminal` build passed. Validation logs are under ignored `target/editing-validation/`.

Release-built ConPTY probes against PowerShell 7.6.6 / PSReadLine 2.4.5 inspected the real editable buffer using a temporary diagnostic binding. For `one two three`:

| Action and starting position | Result |
| --- | --- |
| Delete at start | `ne two three`, cursor 0 |
| Ctrl+Delete at start | ` two three`, cursor 0 |
| Backspace at end | `one two thre`, cursor 12 |
| Ctrl+Backspace at end | `one two `, cursor 8 |
| Ctrl+Left at end | unchanged text, cursor 8 |
| Ctrl+Right at start | unchanged text, cursor 4 |

The same six protocol checks passed in WSL Arch Linux Bash/readline. Bash Ctrl+Right stopped at position 3 instead of PowerShell's 4, demonstrating shell-owned boundaries. This was a shell PTY probe, not a Linux graphical terminal-app run.

A release ConPTY Neovim smoke observed mouse-reporting activation (the parser still showed Primary screen / normal cursor mode), then sent the modified editing/selection inputs with application ownership active. Deterministic tests verify routing and bytes; this probe did not assert Neovim's editable buffer or graphical result. The temporary Windows probe source was removed.

## Manual native acceptance and deliberate limits

The user reported manual native acceptance passed on 2026-10-01: Ctrl+Delete deletes the right-hand word; Ctrl+Backspace deletes the left-hand word; Shift+Left/Right character selection and Ctrl+Shift+Left/Right word selection work correctly; reversing direction contracts and re-extends selection; highlighting is visually correct; copying keyboard-selected text produces the expected clipboard contents; and Vim/Neovim behavior remains correct without terminal input stealing. This completes the graphical acceptance that the earlier automated launch attempt could not perform because of a tool policy restriction. The normal Delete/Backspace and unchanged Ctrl+Left/Right protocol results are recorded above.

Final review confirmed the diff is limited to shortcut routing, configurable selection commands, core selection/encoding/ownership and required wrap metadata, focused tests and related documentation. Ctrl+Left/Right retain CSI 1;5D / CSI 1;5C and shell-owned navigation. Configured commands still take precedence, require exact modifiers, and a custom binding set still replaces the built-in set. All six completion gates passed; final diff/status checks passed. No commit or push.

Applications using Primary screen without any ownership signal cannot be distinguished from a shell. A custom binding set can omit the selection bindings to forward those arrows. Full Kitty keyboard protocol support, Unicode grapheme clustering beyond the existing cell/combining representation, and scrollback reflow on resize are not added. No shell presets or additional shortcuts were added.
