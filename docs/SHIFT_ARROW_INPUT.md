# Shift horizontal arrow input

Branch: `feat/shift-arrow-input`, recreated directly on `feat/editing-shortcuts-polish` (`99ed3fa`). No selection-only cherry-pick, commit, or push.

## Goal and plan

Make Shift+Left/Right usable by terminal applications while retaining normal terminal selection and explicit binding precedence.

1. Inspect selection, input modes, encoding, and configured-command dispatch (done).
2. Recreate the branch on the complete editing-shortcuts prerequisite (done).
3. Preserve Shift and Ctrl+Shift horizontal-arrow bytes using its existing core modifier encoder (done).
4. Test normal character/word selection, application-mode ownership, both screens/cursor modes, and configured bindings (done).
5. Run final Rust validation and build the native executable (done; results below).
6. Receive the user's physical-key Codex/TUI results and record compatibility (done; user reported success on 2026-10-06).

Goal complete: automated validation and user-reported physical-key acceptance passed.

## Routing and preserved behavior

Overlays retain their existing handling. Configured commands are resolved before terminal key encoding; an explicit copy/paste or other non-selection command wins in application modes too. Configured selection commands still apply the ownership rule below. A custom bindings list replaces defaults.

Default Shift+Left/Right selection commands stay app-owned only on the Primary screen with normal cursor-key mode, mouse tracking off, and no keyboard-reporting request. Ctrl+Shift+Left/Right uses the same rule for word selection. Alternate screen, application cursor mode (DECCKM), mouse tracking, or a nonzero Kitty-style keyboard-reporting / modifyOtherKeys request causes these selection commands to forward through the PTY. An unbound chord reaches the normal input encoder directly.

| Key | Forwarded bytes |
| --- | --- |
| Shift+Left | `\x1b[1;2D` |
| Shift+Right | `\x1b[1;2C` |
| Ctrl+Shift+Left | `\x1b[1;6D` |
| Ctrl+Shift+Right | `\x1b[1;6C` |

Modified CSI sequences are identical in normal and application cursor modes. Modifier values follow the [xterm reference](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html). The prerequisite's general `encode_modified_cursor_key` preserves the original Shift-arrow encoding changes without adding a redundant Shift-only public helper.

Normal-screen character/word selection, contraction and reversal, shared selection rendering/copying, mouse selection and Shift+click remain inherited from the prerequisite. Its word-deletion behavior is also retained as part of the authorized base. This branch adds focused regression coverage without changing unrelated keyboard behavior.

## Automated evidence

New tests cover all four default selection chords on normal screen, selection yielding to alternate screen / DECCKM / mouse tracking / keyboard-reporting requests, explicit bindings in those modes, and exact Shift/Ctrl+Shift bytes on both screens and in both cursor modes. Existing core selection tests cover contraction, reversal, Unicode and wrapped rows.

Validation passed: focused app/core tests, `cargo fmt --all -- --check`, `cargo check --workspace --all-targets -q`, `cargo test --workspace --all-targets -q`, `cargo test --workspace --doc -q` (four doctests), `cargo clippy --workspace --all-targets -- -D warnings`, `git diff --check`, and `cargo build -q -p terminal-app --bin terminal`. Four existing ignored tests remain ignored. Branch ancestry was verified. Physical-key compatibility is separate from automated byte/routing evidence.

## Physical-key verification — passed

On 2026-10-06 the user reported: "Manual physical-key verification passed." This closes the requested manual acceptance for Codex follow-up input, Vim/Neovim or another TUI, normal terminal selection, and configured binding precedence. Application versions and individual observations were not supplied; this is user-reported acceptance, not agent-performed UI verification.

The verification checklist is retained below for future regression checks with `target/debug/terminal.exe`.

1. In an ordinary shell, type `abc def`; check Shift+Left/Right grows and contracts character selection, Ctrl+Shift+Left/Right selects words, reversal works, highlights match copied text, and mouse selection / Shift+click still work.
2. In Codex follow-up input, type a disposable draft such as `abc def`. Press Shift+Left repeatedly, then Shift+Right; confirm composer selection grows/shrinks. Replace selected text, then cancel the draft. Check plain arrows and Ctrl+Left/Right as well.
3. In `vim -Nu NONE -n` or `nvim --clean`, set `:nnoremap <S-Left> :echo "SHIFT_LEFT"<CR>` and `:nnoremap <S-Right> :echo "SHIFT_RIGHT"<CR>`. Press each physical chord and check its message. Quit and check normal shell selection again.
4. With a disposable config, bind Shift arrows to `copy` / `paste`; check those explicit commands win in the TUI too. Use harmless clipboard text. Custom bindings replace defaults.

## Compatibility limits

A Primary-screen application that requests none of the ownership signals remains subject to default terminal selection. Raw input mode and process identity alone are not observable through this routing rule. Bracketed paste alone does not transfer ownership because ordinary shells use it too. Applications must recognize xterm modified cursor sequences. Keyboard-reporting requests affect ownership, but this branch does not implement or advertise full Kitty keyboard protocol support. The user's physical-key acceptance covers the tested environment; broader application/version compatibility is not established.
