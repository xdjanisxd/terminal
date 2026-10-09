# Native Linux restoration evidence

Collected on 2026-10-09, Debian 13 / KDE Wayland with XWayland release windows.
Bash 5.2.37 and Zsh 5.9 used the normal user's startup configuration.

For each shell, `ui-save-reopen-pane0.txt` and `pane1.txt` contain the three
observed startup-command results from the successful UI verification:

1. Initial launch with distinct pane roots.
2. Reopen the saved `NativeBash` or `NativeZsh` workspace.
3. Change pane 1 to `~` and pane 2 to `~/codes/terminal`, save `ChangedBash` or
   `ChangedZsh`, then reopen it.

Each operation appended exactly one line per pane. Later prompts and focus
changes produced no additional lines during this sequence. The corresponding
`ui-save-reopen-workspaces.toml` files show both saved definitions, the 40/60
split, focused second pane, CWD values, and explicit commands. These files
are evidence fixtures; their absolute paths are machine-specific.

The final release build was subsequently launched in fresh isolated fixture
directories. `final-launch-pane*.txt` shows one command execution per pane
in its configured root. A final UI reopen replay was inconclusive because
desktop focus changed during automation; it is not counted as a successful
reopen. Deterministic native PTY save/reopen regressions passed on the final
source. No desktop screenshots are included.

`validation.txt` records all Rust gates and the final release binary hash.
The all-target suite retains the repository's platform-dependent ignored
tests; it is not a claim of native Windows execution.
