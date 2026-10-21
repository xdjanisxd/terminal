# CWD inheritance

Implemented on `feat/cwd-inheritance`, based on master `d4542ae`. The branch
already existed at that commit with a clean tree; fetching `origin/master`
confirmed the same base. No commits or pushes are part of this task.

## Plan and implementation

1. Trace OSC 7 ownership, new-tab/split creation, launch roots, and shell startup.
2. Resolve local URI authority and native path semantics outside the output path.
3. Add runtime inheritance beneath configured roots, preserving workspace data.
4. Cover URI edge cases, precedence, focus, fallback, and real shell launches.
5. Update documentation and roadmap, then run final workspace checks.

`terminal-core` continues to own and track the latest syntactically valid OSC 7
report per pane. Its parser and PTY-output handling are unchanged. The app's
`create_pane_at_root` reads the source runtime before activation changes, and
only resolves inheritance when the new pane has no configured launch root.

The app's `cwd` module decodes the URI and selects a launch directory. The
platform module supplies OS machine names through `gethostname` on Linux/macOS
and `GetComputerNameExW` on Windows. No DNS query, subprocess, environment-host
alias, process-CWD inspection, new watcher, timer, or background polling is used.
The platform crate now directly declares the already locked `libc` dependency
for Linux/macOS and enables the Windows system-information API feature.

Inherited directories live in the existing pane runtime representations,
follow focus changes, and are cleared on restore/shutdown. They do not become
configured roots, so a later report can be inherited by subsequent children.
Saved Workspace snapshots retain the inherited launch directory if a new pane
has not reported a directory yet; later local OSC 7 reports still take priority
for snapshots as before.

## Precedence and fallback

For a newly created tab or split:

1. Explicit project action root, followed by the existing target pane, tab,
   and workspace root resolution. `--project-root` remains the workspace override.
2. The source pane's latest tracked OSC 7 URI, if it describes an existing local
   directory under the platform rules below.
3. No working-directory override: inherit the Terminal process's current
   directory, then allow the user's shell startup files to apply their normal
   behavior.

The source pane's pane-specific root is not copied as configuration to children;
its own launch and Saved Workspace metadata remain intact. New tabs still copy
the active tab's configured root, and splits retain their containing tab/workspace
root behavior. Explicit pane roots on restored panes retain precedence over tab
and workspace roots. Missing explicit roots still produce launch errors.

Only inherited roots are best effort. A missing report, rejected URI, regular
file, failed directory metadata lookup, or deleted directory falls back. The
inherited directory is rechecked immediately before constructing the session's
spawn configuration. If it disappears or becomes inaccessible after that check,
the normal spawn error is reported; there is no retry loop or silent shell
substitution. With no explicit or inherited root, PTY children start in the
Terminal process's current directory. User shell startup files can still change
directory according to their existing behavior.

Shell selection, arguments, environment, configured startup-command readiness
and exactly-once dispatch are unchanged. New panes do not copy the source pane's
startup command or running process. Initial launches, Saved Workspace restore,
and direct `-e`/`--execute` sessions do not acquire active-pane inheritance.

## URI and platform rules

The policy follows the local-authority and drive-path distinctions in
[RFC 8089](https://www.rfc-editor.org/rfc/rfc8089) and Microsoft's
[Windows path rules](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file).
It intentionally supports a conservative subset of file URIs.

- Accept `file://` case-insensitively with an empty authority, `localhost`, or an
  exact OS-provided machine name, compared case-insensitively. Do not resolve
  remote hosts, IP aliases, short-name guesses, or environment aliases.
- Decode percent escapes once as UTF-8. Reject malformed escapes, control/NUL
  characters, credentials, ports, queries, fragments, and invalid URI characters.
  Encoded spaces, Unicode, `#`, and `?` remain filename characters where the
  platform permits them.
- Linux and macOS accept absolute POSIX paths such as `/home/me/project` and
  `/Users/me/project`. Reject double-leading-slash paths and Windows drive forms.
- Windows requires `/C:/...` URI paths and maps them to `C:/...`. Reject absent
  drive letters, drive-relative paths, UNC/share paths, device namespaces,
  backslashes, alternate-stream colons, and Windows wildcard/reserved characters.
  Final directory validity is checked using the host filesystem.
- Reject a remote SSH authority even if its path happens to exist locally.
  No SSH state, command, or remote filesystem is inherited.

## Limits

OSC 7 is a shell report, not authenticated provenance. An SSH program that emits
an empty authority, `localhost`, or the local machine's name is indistinguishable
from a local report. Normal remote-host reports are rejected; no process-tree or
SSH detection is added. A rejected latest remote report causes fallback rather
than reuse of an older local report. Malformed reports already ignored by
`terminal-core` leave its previous valid report unchanged.

Symlinks, mounted filesystems, and mapped Windows drives may lead to network
storage; this feature rejects remote URI authorities and UNC forms, not such
filesystem indirections. A synchronous metadata check can take time on those
filesystems. It happens during session creation, never on PTY output.

Shells without OSC 7 use the normal fallback. CMD and Fish do not gain reporting
hooks here. macOS accepts POSIX URI semantics but does not gain automatic shell
hooks; users need existing OSC 7 integration. Shell startup files can still
change directory according to existing shell policy. Non-UTF-8 path reports are
unsupported by the existing OSC 7 state contract. Windows/macOS native build and
desktop acceptance remain unverified in this Linux-only environment.

## Validation

Focused coverage includes local/remote authorities, native machine-name matching,
percent-encoded Unicode/spaces, malformed/control/query/fragment input, POSIX/macOS
paths, Windows drive/UNC/device/relative cases, explicit-root precedence, focused
pane selection, runtime/config separation, source-only startup commands, missing
and deleted directories, regular files, and restore isolation. Native Linux
Bash/Zsh PTY tests launch both splits and tabs from real OSC 7 prompt reports.
Final validation on Linux passed:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --all-targets --offline` | Passed |
| `cargo test --workspace --all-targets --offline` | 803 passed, 26 ignored, 0 failed; benchmark targets also completed |
| `cargo test --workspace --doc --offline` | 4 passed, 0 failed |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | Passed |
| `cargo build --locked --release -p terminal-app --bin terminal --offline` | Passed |
| `git diff --check` | Passed |

`--all-targets` includes library, binary, integration-test, and benchmark targets;
`--offline` uses cached dependencies. `--locked` verifies the resulting lockfile
without allowing further updates. Raw logs are retained in
`/tmp/cwd-inheritance-validation/{fmt,check,tests,doctests,clippy,release}.log`.
The focused CWD run passed 13 matching tests before final validation. Native PTY
coverage exercised installed `/bin/bash` and `/bin/zsh` with tabs and splits;
it does not establish desktop UI acceptance. The 26 ignored tests, desktop
manual checks below, and Windows/macOS native execution were not run.

## Manual Linux acceptance (pending desktop execution)

1. Use a temporary config with no project/tab/workspace roots and interactive
   Bash or Zsh. Create and `cd` into `/tmp/cwd inheritance é`; wait for the prompt.
   Open a tab with Ctrl+Shift+T and split with Ctrl+Shift+E and Ctrl+Shift+O.
   Run `pwd` in each child: expect that directory. Change one pane to a second
   directory; focus each source and create children to confirm source isolation.
2. Repeat with a configured workspace root and tab root; children must use those
   roots even after the source changes directory. Use an Open Project tab/split
   action and verify its explicit root wins. Restore a workspace with distinct
   pane roots and startup commands; verify each original root and one command
   execution, with no source startup command copied to new children.
3. In a reporting shell, emit `printf '\033]7;file://remote.invalid/tmp\007'` and
   immediately create a child; expect the normal launch default. To avoid a
   subsequent prompt replacing the report during a controlled probe, emit it
   followed by `sleep 20`, then create the child during that foreground command.
   Also try a deleted reported directory and malformed reports; no unusable
   inherited root should prevent launch. Confirm a deliberately missing explicit
   root still reports its normal launch error.
4. Save/reopen the arranged workspace. Confirm fresh sessions, preserved roots,
   startup-command behavior and focus. Launch `terminal -e /usr/bin/pwd` from a
   different caller directory and confirm caller-relative direct execution.

## Manual Windows acceptance (pending native execution)

1. With PowerShell and no configured roots, `Set-Location` into a local directory
   containing spaces and Unicode, e.g. `C:\Temp\cwd inheritance é`. Wait for the
   prompt; create tabs and both split orientations. `(Get-Location).ProviderPath`
   must match. Repeat after changing/focusing different source panes.
2. Repeat with workspace/tab roots, explicit project actions, and a Saved
   Workspace containing pane roots and one startup command. Verify precedence,
   independent fresh shells, one startup execution and no command copying.
3. From a controlled foreground command, emit remote-host, UNC, `/C:relative`,
   root-relative and deleted-directory reports using `[Console]::Write` with ESC
   and BEL, then pause with `Start-Sleep -Seconds 20` before the next prompt.
   Create children during the pause; expect normal fallback. Try valid
   `file:///C:/...`, localhost and actual machine-name authorities; expect local
   inheritance. Non-filesystem PowerShell providers must not create local paths.
4. Verify configured shell arguments/profiles and CMD behavior are unchanged;
   CMD without OSC 7 uses fallback. Verify `terminal -e cmd /c cd` retains caller
   directory behavior and does not acquire an unrelated pane's directory.
