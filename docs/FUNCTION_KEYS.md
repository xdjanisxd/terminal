# Function keys

Branch: `feat/function-keys`. No commit or push.

## Goal and plan

Add VT/xterm-compatible F1–F12 input for shells and TUIs while preserving existing keyboard behavior and configured binding precedence.

- [x] Inspect keyboard → configured-command → core encoder → focused PTY. Existing special keys use VT/xterm; DECCKM changes cursor keys only.
- [x] Add core F1–F12 encoding and app logical/physical key mapping.
- [x] Encode Shift/Alt/Ctrl combinations and preserve configured binding precedence without new default shortcuts.
- [x] Test core encoding, app mapping, configuration chords, and cursor-mode/alternate-screen independence.
- [x] Run final Rust validation and build the native executable.
- [ ] Manually verify cmd + Clink, PowerShell and Vim/Neovim, recording versions, keys/modifiers checked and failures.

## Protocol

ESC is byte 0x1B. Spaces below separate bytes for readability; none are transmitted. These are PC-style [xterm sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html).

| Key | Plain | Modified |
| --- | --- | --- |
| F1 | ESC O P | ESC [ 1 ; m P |
| F2 | ESC O Q | ESC [ 1 ; m Q |
| F3 | ESC O R | ESC [ 1 ; m R |
| F4 | ESC O S | ESC [ 1 ; m S |
| F5 | ESC [ 15 ~ | ESC [ 15 ; m ~ |
| F6 | ESC [ 17 ~ | ESC [ 17 ; m ~ |
| F7 | ESC [ 18 ~ | ESC [ 18 ; m ~ |
| F8 | ESC [ 19 ~ | ESC [ 19 ; m ~ |
| F9 | ESC [ 20 ~ | ESC [ 20 ; m ~ |
| F10 | ESC [ 21 ~ | ESC [ 21 ; m ~ |
| F11 | ESC [ 23 ~ | ESC [ 23 ; m ~ |
| F12 | ESC [ 24 ~ | ESC [ 24 ; m ~ |

m = 1 + Shift + 2×Alt + 4×Ctrl: Shift=2, Alt=3, Shift+Alt=4, Ctrl=5, Shift+Ctrl=6, Alt+Ctrl=7, all three=8. Alt adds no extra ESC. Cursor/keypad application modes and screen selection do not change these sequences. Configured bindings match physical keys and exact modifiers before encoding; existing modal overlays retain focus. No default F1–F12 shortcuts are added.

## Tests and validation

Three focused tests cover all twelve plain encodings, all eight modifier combinations in core, cursor-mode/screen independence, logical keys, physical fallback, default pass-through, explicit binding resolution and valid/invalid config chords. Full fmt, workspace check/all-target tests, doc tests, Clippy with warnings denied and diff checks pass. Native PowerShell tests initially failed under sandbox process restrictions and passed with native process permissions. These existing smoke tests do not establish function-key compatibility.

## Manual checklist (pending)

Build with `cargo build -p terminal-app`; launch `target/debug/terminal.exe`. Perform these checks inside its window with terminal input focused. A temporary config selected by `TERMINAL_CONFIG` can set `[shell]`, `program = 'cmd.exe'` or `program = 'pwsh.exe'`, and `args = []`. Restore the environment afterward.

1. **cmd + Clink:** inject Clink if needed with its installed clink.bat. Run `clink echo` (or full-path clink.bat with `echo`). Press F1–F12, then repeat with Shift, Alt, Ctrl and combinations. Record the keys reported; Clink may normalize console input into its own sequences. Ctrl+C exits. Verify an actual configured Clink function-key binding at the prompt. See [Clink key discovery](https://chrisant996.github.io/clink/clink.html#discovering-clink-key-sequences).
2. **PowerShell:** run the loop below and press every function key with the same modifiers. Expect F1–F12 and corresponding modifiers; Escape exits. Also verify a normal PSReadLine function-key binding at the prompt.
3. **Vim/Neovim:** start `vim -Nu NONE -n` or `nvim --clean`; execute the Ex command below. Press each key/modifier combination in Normal mode; expect a message naming it. This exercises the alternate screen. Exit with `:qa!` and repeat a shell probe after returning to the primary screen.
4. **Binding precedence:** in a temporary terminal config add `[[bindings]]`, `key = 'F1'`, `command = 'open_palette'`. F1 must open the palette and not reach the probe. Escape closes it; unbound Shift+F1 must still reach the probe. Remove the temporary binding.

PowerShell probe:

```powershell
while ($true) {
    $key = [Console]::ReadKey($true)
    if ($key.Key -eq 'Escape') { break }
    '{0} {1}' -f $key.Key, $key.Modifiers
}
```

Vim/Neovim mappings (one Ex command):

```vim
for n in range(1, 12) | for mod in ['', 'S-', 'A-', 'C-', 'S-A-', 'S-C-', 'A-C-', 'S-A-C-'] | let k = '<'.mod.'F'.n.'>' | execute 'nnoremap '.k.' :echo '.string(k).'<CR>' | endfor | endfor
```

## Results and limits

Manual results: cmd + Clink **pending**; PowerShell **pending**; Vim/Neovim **pending**. Installed applications were located; no physical-key acceptance run is claimed. Computer Use guidance prohibits automating terminal applications, so human checks are required. The goal is blocked pending human manual-verification results; implementation and automated validation are complete.

F13+ and extended keyboard protocol negotiation are outside this change. OS shortcuts, Fn/media layers and application mappings can affect recognition. Byte encoding alone does not prove application compatibility.
