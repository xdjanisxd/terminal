# Linux clipboard paste and TERM verification

Date: 2026-10-08. Branch: `fix/linux-paste-term`. No commit or push.
The [plan](LINUX_PASTE_TERM_PLAN.md) is complete.

## Root causes and trace

Keyboard presses arrive through winit's `KeyboardInput` and `ModifiersChanged`
events. Overlay handling runs first, followed by `configured_command`, then
terminal key encoding. Ctrl+Shift+V already resolved to `Command::Paste` with
built-in defaults. The failure was in `terminal-platform`: every non-Windows
clipboard read and write returned `Unsupported: clipboard unavailable`.
Consequently no paste bytes reached the PTY. Shift+Insert additionally lacked
a default paste binding and reached the ordinary Insert-key encoder instead.

The native user's existing config has an explicit replacement binding list,
including Ctrl+Shift+C/V but no Shift+Insert. That list remains authoritative:
the new Linux default does not silently add a binding to existing explicit
lists. To enable this alias in such a config, add:

```toml
[[bindings]]
key = "Shift+Insert"
command = "paste"
```

Paste now follows the existing route: clipboard text -> `paste_text` ->
`encode_paste` using the active terminal's modes -> `write_terminal_input` ->
the active `PtyWorker`. `activate_pane` swaps the parser, terminal state, and
PTY together, so bracketed-paste modes and writes remain pane-specific.

PTY spawning previously constructed portable-pty's `CommandBuilder`, which
copies the inherited environment, and applied explicit spawn overrides. Neither
Terminal nor portable-pty supplied TERM. Bash with no inherited TERM sets its
shell variable to `dumb`; `clear` reports an unset TERM environment and exits 1.
An inherited `TERM=dumb` also persists and makes `clear` exit 1. Both failure
paths were reproduced with `bash --noprofile --norc`, eliminating user startup
files as a necessary cause. The native user config has no shell override or
TERM setting. The current `.bashrc` already exports `TERM=xterm-256color`;
checked Bash/profile startup files contain no TERM=dumb assignment. Release
verification used no-profile/no-rc Bash to test the PTY fix independently of
that existing workaround. This agent's launch environment has
`TERM=xterm-256color`; the
environment of the original failing launch was not available, so missing TERM
and inherited dumb are both verified triggers rather than a claim about that
historical process.

## Changes

- Added text-only arboard 3.6.1 behind `terminal-platform` on Linux/macOS,
  enabling Wayland data-control and X11/XWayland fallback. A persistent,
  serialized backend retains ownership after copy/OSC 52 calls return; failed
  initialization is retried on later calls. No external clipboard utilities
  are required by Terminal. Backend lifetime requirements and fallback follow
  [arboard's documentation](https://github.com/1Password/arboard/blob/master/README.md).
- Added Linux's Shift+Insert alias to configuration defaults. Replacement lists,
  disabled bindings, exact modifiers, and explicit remapping keep precedence.
  Windows and macOS default bindings are unchanged.
- Kept paste on the existing pane-specific bracketed-paste and PTY write path.
  Unix clipboard text is checked against the existing 2 MiB UTF-16 limit before
  encoding/queueing, including a terminating NUL's size.
- Added Linux/macOS TERM initialization at the PTY boundary. Missing, empty,
  or inherited `dumb` becomes `xterm-256color`; other inherited values survive.
  Explicit spawn environment is applied afterwards and always wins.
- Added validated, optional `[shell].env` child-only overrides, so an intentional
  TERM choice, including `dumb` or empty, can be configured explicitly.
  Shell startup and command wrappers can still override TERM afterwards.
  Direct command sessions retain their existing independence from `[shell]`.
- Windows clipboard code, TERM policy, PowerShell arguments/hooks, cmd prompt
  integration, working-directory rules, and PTY transport remain intact.
  No system, parent-process, or user shell/config environment was modified.

## xterm-256color compatibility

Inspected the installed `infocmp xterm-256color` entry before acceptance.
The relevant capabilities match existing parser/encoder support: cursor
positioning and erase, 256-color SGR, alternate-screen save/restore, cursor
mode, editing keys, function keys, and scroll regions. The new core fixture
checks clear, indexed foreground/background colors, and the screen round trip.
Native `clear` output contains supported `ESC[H` and `ESC[2J` sequences and exits
0 with this TERM. Existing input, scrolling, rendition, and screen tests plus
the native Vim exercise provide additional compatibility evidence.

This is a practical compatibility choice for Bash, clear, and Vim, not a claim
that every optional xterm extension is implemented. TERM intentionally chosen
by the user remains their responsibility and is preserved as documented.

## Validation

Rust 1.99.0 was installed only under `/tmp` without changing PATH or shell
profiles globally; system Rust 1.85.1 cannot compile the repository's existing
let-chain syntax. Dependency downloads needed approved network access.
The following gates passed using the isolated toolchain:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
cargo build --locked --release -p terminal-app --bin terminal
git diff --check
```

Cargo validation used `--offline` after dependencies were downloaded.
All-target tests: **672 passed, 0 failed, 5 ignored**. Doctests: **4 passed**.
Four ignored tests predate this change; the new desktop-only clipboard test was
run separately in each backend mode below. The native Bash default test also
passed with TERM removed from the test process and with inherited TERM=dumb.

Focused regressions cover UTF-16 clipboard limits; optional/invalid shell env;
shell integration retaining env overrides; default/remapped/disabled paste
bindings; writes to two real PTYs with independent bracketed-paste modes;
missing/empty/dumb/preserved TERM; explicit empty/dumb overrides; native Bash,
clear, and later shell overrides; and xterm-256color parser compatibility.

## Native Linux release results

Host: Debian 13 x86_64, Linux 6.12.111, KDE Wayland with XWayland,
Bash 5.2.37, Vim 9.1. Results and the tested binary's SHA-256 are recorded in
[results.json](measurements/linux-paste-term/results.json).

| Native path | Result |
| --- | --- |
| Wayland clipboard | Ownership/Unicode round trip and size rejection passed. WAYLAND_DEBUG confirms binding and using `zwlr_data_control_manager_v1`. |
| X11/XWayland clipboard | Same regression passed with WAYLAND_DISPLAY unset. |
| Wayland unavailable -> X11 fallback | Same regression passed with a nonexistent Wayland display and a valid X11 display. |
| Native Wayland release window | Surface creation and GPU frame presentation passed for all TERM cases below. |
| XWayland release Bash | Started from inherited dumb, received xterm-256color; clear exited 0. |
| Ctrl+Shift+V | KDE clipboard -> Terminal -> Bash passed, including accented text, emoji bytes, and CJK text. |
| Multiline/bracketed paste | The pasted shell commands did not execute until Enter; resulting file bytes matched exactly. |
| Shift+Insert | Pasted clipboard text successfully in Bash and after returning from Vim. |
| Active split pane | Pasted commands executed in distinct Bash processes; recording each pane's PID verified routing. |
| Vim | `vim -Nu NONE -n` received a two-line Unicode paste; save/quit preserved exact UTF-8 bytes and restored the Bash screen. |

These were agent-performed native UI acceptance checks using XTest keyboard
events and independent KDE clipboard operations, followed by manual visual
inspection of compositor screenshots. They are not user-reported physical-key
results. Inspected images: [Vim paste](measurements/linux-paste-term/vim-paste.png)
and [Bash after Vim](measurements/linux-paste-term/bash-after.png).
The test scripts restored the clipboard text and closed their own windows.

The final release's native Wayland TERM matrix was:

| Inherited TERM | Explicit shell env | Later shell override | Child TERM | clear exit |
| --- | --- | --- | --- | --- |
| missing | none | none | xterm-256color | 0 |
| dumb | none | none | xterm-256color | 0 |
| screen-256color | none | none | screen-256color | 0 |
| missing | dumb | none | dumb | 1, intentional |
| dumb | vt100 | none | vt100 | 0 |
| dumb | none | vt100 | vt100 | 0 |

## Remaining limitations

- Native keyboard/editor acceptance used XWayland. Native Wayland clipboard,
  release surfaces, rendering, and TERM were exercised, but Wayland-native
  physical-key acceptance was not independently performed.
- A compositor without supported data-control needs working X11/XWayland
  clipboard interoperability. The fallback test covers unavailable Wayland;
  other compositors, seats, and disconnected-display recovery were not tested.
- Neovim was unavailable; Vim fulfilled the editor verification.
- Emoji UTF-8 bytes were pasted and saved correctly, but the selected font/
  renderer displayed the emoji as a blank glyph. This existing rendering
  limitation is outside clipboard/TERM scope.
- Windows/macOS native execution and CI were not available in this Linux session.
  Platform guards preserve Windows's implementation; native cross-platform
  validation remains a limitation of this run.
- Unix retrieval allocates the clipboard contents inside the backend before
  Terminal checks its paste size limit. Clipboard persistence after exit depends
  on a clipboard manager.
- Existing explicit configs gain no shortcuts automatically; the user's current
  binding list needs the Shift+Insert entry above if that alias is desired.
