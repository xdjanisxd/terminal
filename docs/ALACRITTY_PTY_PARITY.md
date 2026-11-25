# Alacritty PTY parity

This task compares terminal replies requested by small Codex, Neovim, and tmux PTY traces. It also records separate local-shell, SSH, tmux, and terminfo checks. Fixtures retain protocol requests and a short control-sequence probe, with rendered text and user data omitted.

## Reproduce

Use an isolated PTY to capture each application's requests.

```sh
python3 scripts/capture-pty-parity.py --seconds 1.5
```

The capturer uses a 100×30 PTY, `TERM=xterm-256color`, `COLORTERM=truecolor`, clean per-run HOME/Codex state, and a controlling TTY. It strips inherited authentication, token, API-key, credential, and secret variables. A fixture records exact request bytes, PTY metadata, raw-output digest, and a compact inventory of relevant control sequences. Screen text and arbitrary OSC content are excluded. `regression.json` contains synthetic regression probes.

Build with `cargo build --release -p terminal-app`. Run the replay in a Linux graphical session with Python 3.10 or later and the pinned Alacritty binary installed.

```sh
python3 scripts/compare-pty-parity.py --terminal /absolute/path/to/target/release/terminal
```

The comparison prints one table per fixture, including exact request/reply bytes and missing or different replies. It uses Alacritty 0.15.1 (Debian 0.15.1-3), SHA-256 `3696cbe9e61c835e539f9dacad0abf86a01eb58a386838ba03ca55075d5475e9`. Both PTYs were 80×29 during the final replay. The tested Terminal binary had SHA-256 `927ce89c77dc0cb70cdd2b5aba36f2b90945772fee084caf82f3ecfa2b244ac0`.

## Captured requests and support decisions

The three application traces contain 25 writes, including Kitty setup. Codex accounts for 8 writes, Neovim for 13, and tmux for 4. Captures used Codex CLI 0.172.0, Neovim 0.12.6, and tmux 3.5a. Counts and ordering can vary with application timing. The files in `tests/fixtures/pty-parity/` are the retained captures used for the final comparison.

| Capability | Capture and replay evidence | Decision |
| --- | --- | --- |
| OSC 4 palette query/set/reset | No OSC 4 query appeared in the three startup traces. The synthetic fixture sends a set, query, OSC 104 indexed reset, then another query. Terminal replies with its configured palette both times. Alacritty reports the set color before reset and its palette after reset. Existing configured-palette behavior also has `indexed_color_queries` coverage. | Preserve Terminal's configured-palette behavior and regression coverage. |
| OSC 10/11 default foreground/background | Codex, Neovim, and tmux query OSC 10/11. Replies matched Alacritty byte-for-byte, including RGB values and BEL/ST terminators. In the synthetic probe Terminal keeps configured colors after OSC 10/11 set writes and RIS. Alacritty honors the writes and keeps those colors after RIS. | Preserve OSC 10/11 query behavior and configured colors across reset. |
| DEC mode 1007 alternate scroll | Codex requests `CSI ?1007$p`. Terminal reports reset (`;2$y`). Alacritty reports set (`;1$y`) in the initial state. Explicit set/query and reset/query replies match, and `alternate_scroll.rs` covers these transitions. The X11 wheel check sent no arrow input while disabled, then six CSI A sequences for wheel-up while enabled. With mouse reporting active, wheel input produced SGR mouse reports. | Keep Terminal's reset default and current 1007 implementation. |
| Kitty keyboard flags | Codex and Neovim query `CSI ?u`; replies matched (`CSI ?7u` for Codex's flags setup and `CSI ?0u` for Neovim). Codex's `CSI >7u` push setup is retained as its own write. | Existing query behavior is adequate for these traces. |
| DEC mode 2026 synchronized output | Both emulators returned `;2$y` in the final replay. In live Neovim sessions, the baseline Terminal build elicited zero synchronized redraw pairs, the fixed build five, and Alacritty four. The emulators also matched for idle queries, queries buffered until timeout or batch end, and queries sent after batch end. | Add the exact DEC 2026 state query to unblock Neovim. Keep the established update boundaries. |
| Other Neovim DECRQM queries (`?69`, `?2027`, `?2031`, `?2048`) | Alacritty returns status `;0$y`. Terminal is silent. The queried modes are not implemented as Terminal state. | Keep unsupported mode replies silent until Terminal owns those states. |
| DECRQSS rendition (`DCS $qm`) and SGR 4:3 | Neovim emits the exact grouped probe `CSI 0m`, `CSI 4:3m`, `DCS $qm ST`. Both terminals are silent. The trace leaves Neovim's visible use of the queried underline style unconfirmed. | Defer DECRQSS and extended underline implementation. |
| OSC 12 cursor color | No request appeared in the captured traces. | Keep OSC 12 out of scope for this evidence set. |
| SGR 58/59 underline color | No request appeared. The observed SGR 4:3 preceded a DECRQSS probe. | Keep SGR 58/59 out of scope for this evidence set. |
| DA1 | All three apps query DA1. Terminal returns `CSI ?1;0c`. Alacritty returns `CSI ?6c`. | Recorded parity difference. The existing Terminal reply is retained unless a protocol decision changes it. |
| DA2 | tmux queries DA2. Terminal returns `CSI >0;0;0c`. Alacritty returns `CSI >0;2500;1c`. | Recorded parity difference. The current fixed Terminal reply remains in use. |
| DSR | Neovim and Codex request status/cursor position. Replies matched for the captured screen dimensions. | Existing DSR behavior is adequate for these cases. |

The OSC 4, OSC 10/11, and DEC 1007 checks in `regression.json` are compatibility sentinels. They are labeled synthetic and document behavior independently from application demand.

## TERM and terminfo checks

The live acceptance matrix used the installed `xterm-256color` database and a temporary compiled `terminal-roadmap-test` alias whose source was the system `xterm-256color` entry.

| Environment | Result |
| --- | --- |
| Terminal local Bash (`--noprofile --norc -ic`) with parent TERM unset | PTY received `xterm-256color`. `infocmp`, `tput colors` (256), `tput civis`, and `tput setaf 123` passed. |
| Terminal local Zsh (`-f -ic`) with parent TERM unset | PTY received `xterm-256color`. the same ncurses probes passed. |
| tmux 3.5a inside Terminal, `/dev/null` config and fresh socket | Client received `xterm-256color`. tmux advertised `bpaste,ccolour,clipboard,cstyle,focus,title`. The pane received `tmux-256color`. the same ncurses probes passed. |
| OpenSSH client to an isolated local non-root `sshd -i`, `ssh -tt`, fresh host/client keys | The remote PTY received `xterm-256color`. Probes passed with and without the candidate entry. |
| Temporary `terminal-roadmap-test` with its entry installed | `infocmp`, `tput colors`, cursor visibility, and indexed color passed locally and remotely. |
| Same candidate TERM on a remote host without its entry | `infocmp` found no entry. `tput colors` and `tput civis` exited 3. |

Keep `xterm-256color` as Terminal's default TERM. The private `terminal-roadmap-test` alias passed on hosts with its entry. SSH lookup failed on the host without the entry, as expected. This task establishes no benefit for a dedicated entry that outweighs its deployment requirement.

### Manual repeat

Launch Terminal with the parent `TERM` unset, then check `echo "$TERM"`, `infocmp "$TERM"`, `tput colors`, `tput civis`, and `tput setaf 123` in Bash and Zsh. Start a tmux client with a fresh socket and `/dev/null` configuration. Check the outer TERM with `tmux display-message -p '#{client_termname}'`, then run the same checks in the `tmux-256color` pane.

Compile a temporary alias of the installed entry in a private scratch directory.

```sh
scratch=$(mktemp -d)
infocmp -x xterm-256color | sed 's/^xterm-256color|/terminal-roadmap-test|/' > "$scratch/candidate.ti"
tic -x -o "$scratch/with-entry" "$scratch/candidate.ti"
TERMINFO="$scratch/with-entry" TERM=terminal-roadmap-test infocmp terminal-roadmap-test
TERMINFO="$scratch/with-entry" TERM=terminal-roadmap-test tput colors
```

Run local shell and tmux checks with `TERMINFO="$scratch/with-entry"` in their environment. For SSH, use `ssh -tt host`. In the remote shell, export `TERM=terminal-roadmap-test` and run `infocmp "$TERM"`, `tput colors`, and `tput civis`. Repeat with the candidate database installed at the remote scratch path and with `TERMINFO` unset to observe the missing-entry result. The acceptance run used an unprivileged local `sshd -i` with fresh keys behind an SSH `ProxyCommand`. It supplied `TERMINFO` pointing at the private database path. All generated keys, database files, and server configuration lived under one temporary scratch directory. Removing that directory removes the candidate alias and test setup without changing the system database.

## Limits

This is byte-level startup parity for a small set of app traces, plus manual local-shell/SSH/tmux checks. It establishes no pixel-level rendering parity or behavior across all app versions, remote hosts, terminal sizes, and timing schedules. Neither emulator replied to DECRQSS. The number of mode 2026 redraws depends on timing.
