# Linux Saved Workspace restoration

## Root cause and trace

The workspace store was carrying pane roots and explicit commands correctly.
`Application::snapshot_workspace` obtains each pane's current directory from
terminal-core's OSC 7 metadata, falling back to its configured root. Linux
Bash/Zsh sessions previously had no built-in directory hook, so changing
directory interactively did not update that metadata. Saving could therefore
retain an old root or no root at all. A new PTY without a root starts in home.

`open_saved_workspace` reconstructs the layout and active pane; `pane_launch`
and `spawn_config_for_session` carry the saved root to the PTY configuration.
The backend applies that root before starting the child. Shell startup files
can subsequently change directory, however, so a correct saved root alone
does not guarantee the first prompt appears there. Separately, portable-pty
silently substitutes home when the requested directory does not exist.

Startup commands are pane-local pending state. Both foreground and background
PTY event handlers wait for terminal-core to parse OSC 133 A before queueing
the command. Ordinary Bash/Zsh prompts previously emitted no readiness marker,
so commands remained pending indefinitely. Layout restoration succeeded
independently of these shell metadata failures.

## Fix

The app now supplies private, session-lifetime startup wrappers for supported
interactive Linux Bash/Zsh sessions. They load the user's initialization and
retain visible prompts and existing prompt hooks, then emit URI-encoded OSC 7
CWD and OSC 133 A readiness at each prompt. At the first prompt, an explicit
restored root is reapplied after initialization and preceding prompt hooks.
Subsequent `cd` commands remain effective and are captured when saving.

Bash supports scalar and array `PROMPT_COMMAND`, custom rc files, and `--norc`
bootstrap without sourcing disabled rc files. Zsh preserves user startup stages
and changes to `ZDOTDIR`, handles login initialization, and installs its hook
after user `precmd_functions` assignments. The wrappers never edit user files.
Private directories use mode 0700 and are removed with the session.

Missing Linux roots now produce a launch error before portable-pty can silently
substitute home. Noninteractive Bash/Zsh invocations and unrecognized options
keep their original launch arguments and receive no automatic startup input.
Direct command sessions are not marked for integration.

The existing dispatcher remains responsible for exactly-once delivery: it
requires a parsed prompt marker and consumes pending input only after a
successful PTY queue operation. Repeated prompts and focus changes do not
recreate pending commands. Reopening intentionally creates a new session and
runs its explicit command once again. There is no Codex-specific behavior.

## Windows comparison

CMD already appends OSC 133 readiness to its existing `PROMPT`; PowerShell
already loads normal profiles and wraps its prompt to emit OSC 7 and OSC 133.
Linux now feeds the same terminal parser and pane dispatch path. The CMD and
PowerShell integration code is unchanged, and the Linux mode guard evaluates
to true on other platforms. Windows-specific native checks were not executed
on this Linux machine; existing portable shell-policy tests passed.

## Verification

Native PTY regression tests run Bash 5.2.37 and Zsh 5.9 on Debian 13,
Linux 6.12.111. They exercise Bash normal initialization, `--noprofile --norc`,
custom `--rcfile`, Zsh normal initialization, `-f`, and `-l`. Each restores a
40/60 two-pane layout with pane 2 focused and independent commands. Controlled
startup files delay initialization, change directory, replace prompt hooks,
and relocate Zsh's user startup directory. Paths contain spaces, Unicode,
and URI-significant characters.

The tests verify persisted TOML roots, actual prompt metadata, startup output
in the correct directory, CWD changes followed by save and reopen, background
pane dispatch, focus switching, and no duplicate execution at later prompts.
Panes without startup commands remain idle and accept manually typed commands.
A separate regression rejects missing directories. Existing dispatch tests
cover readiness gating and exact command text, including `codex --no-daemon`;
Codex itself is not a native-test dependency.

Release UI verification uses the normal user's Bash and Zsh configuration,
including Oh My Zsh. Both shells save and reopen distinct two-pane roots,
then save and reopen directories changed to `~` and `~/codes/terminal`.
Append-only per-pane logs confirm one execution per launch/reopen; layout
and focused pane are retained. The final build's fresh launch also passed;
its additional UI reopen replay was inconclusive because desktop focus
changed during automation. Final-source native PTY save/reopen tests passed.
Evidence is in
[the measurement directory](measurements/linux-workspace-restore/README.md).

All normal Rust checks pass: formatting, workspace/all-target checking and
tests, documentation tests, Clippy with warnings denied, and the locked release
build. No commits or pushes were made.

## Limits

- Existing saved entries with absent or stale roots cannot reconstruct lost
  historical directories; change to the desired directory and save again.
- Bash login shells keep their true login startup semantics and need user
  OSC 7/133 hooks. Bash ignores `--rcfile` for login shells, so the built-in
  wrapper deliberately does not emulate login profiles in a non-login shell.
  See the [GNU Bash startup documentation](https://www.gnu.org/software/bash/manual/html_node/Bash-Startup-Files.html).
- Built-in integration recognizes executable names `bash` and `zsh` and a
  narrow option set documented in the README. Fish, shell aliases/wrappers,
  restricted/POSIX modes, and other options are not covered. Unsupported
  shells require their own metadata hooks; there is no readiness timeout that
  injects commands speculatively.
- Startup files that replace the process, disable later Zsh startup stages,
  make prompt hook variables read-only, or remove hooks later can prevent
  reporting. A global `/etc/zshenv` that overrides the wrapper's `ZDOTDIR` can
  bypass integration. The [Zsh startup documentation](https://zsh.sourceforge.io/Doc/Release/Files.html)
  describes these stages.
- Directory existence is checked before launch; a concurrent deletion or
  access change remains a filesystem race. Commands deliberately invoked by
  user startup files are outside the app's pending-command guarantee.
- Native Windows validation was unavailable in this Linux environment.
