# Blocked PTY write shutdown

Investigated on Linux x86_64, Debian kernel 6.12.113, Rust 1.99.0, on 2026-10-22.
Branch `fix/blocked-pty-shutdown` started clean at freshly fetched master
`b5676a0f0fbc9e7c591fbb9b12901ee54df1b358`. No commits or pushes were made.

## Root cause and ownership

The controller owns `PtyWorker`, its bounded command sender, event receiver, and
session-thread join handle. The session thread owns `PortablePtySession` and
starts a separate reader thread. Both queues have capacity eight. The reader
can block reading or sending output; the session thread processes writes,
resizes, termination, and child-status observation sequentially.

`PortablePtySession::write` called blocking `write_all`. With a raw-mode child
that stops consuming input, the kernel PTY input queue fills and that call never
returns. The session thread cannot process the queued termination command or
release session ownership. `shutdown_and_join` waits for that same thread; with
a full command queue it can first block enqueueing termination. The session's
Drop cleanup cannot run while that thread owns the session. Termination also
dropped portable-pty's Unix writer before killing the child; its destructor can
block writing newline/EOT to the same full input queue.

The baseline probe confirms both worker threads in kernel I/O waits and the
controller blocked in join. Shutdown exceeded two seconds. A separate diagnostic
then killed the child and still observed no completion for another two seconds.
The supervisor killed the probe and adopted/reaped its child. Killing a child
alone is therefore insufficient evidence that a pending Linux PTY write will
return. The retained [original reproduction](measurements/resource-cleanup-memory-safety/blocked_write.rs)
and prior [resource cleanup](RESOURCE_CLEANUP_MEMORY_SAFETY.md) and
[startup/shutdown](STARTUP_SHUTDOWN_RELIABILITY.md) investigations were reviewed;
their Drop-leak and disconnected-channel spin fixes remain intact.

## Change

`terminal-pty/src/unix_io.rs` owns Unix descriptor cloning, nonblocking I/O, EOF
normalization, and cancellation. It replaces the Unix portable-pty reader/writer
wrappers with owned file descriptors. A successful read/write makes its ordinary
system call. Only `WouldBlock` enters an indefinite `poll(..., -1)` readiness
wait, watching the PTY and a cancellation descriptor. This is an event-driven
kernel wait, without periodic polling or a timeout. Interrupted waits retry;
Unix PTY `EIO` still means read EOF.

The cancellation handle closes the sole peer of a Unix socket pair. Hangup
remains visible to both I/O owners, including waiters entering after cancellation.
Its mutex protects idempotent cancellation only during shutdown. Normal I/O
does not lock it, inspect an atomic cancellation flag, or allocate cancellation
state. Session construction adds two descriptors and fixed setup allocations.
No permanent thread, periodic timer, dependency package, or queue is added;
the already-used libc dependency now applies to Unix instead of macOS alone.
Readiness waiting can add system calls after `WouldBlock`; no claim of identical
syscall count or measured throughput is made.

`PtySession::take_io_cancellation` transfers an optional project-owned capability.
The worker discards unread events, sets the existing disconnected/stop flag,
cancels blocked I/O, then requests termination and joins. Reusing that flag lets
the session loop terminate before processing remaining queued input. The command
sender stays alive until joining finishes, preserving the prior spin fix.
Cancellation failures are returned after joining, with join failure taking
precedence. The existing two threads remain joined on this shutdown path.

Termination kills the child before releasing the writer. Unix uses ordinary
file Drop, which never injects an EOT write. Child termination still uses
portable-pty's existing SIGHUP grace and forced-kill fallback; lifecycle status
observation still reaps children. No process-tree policy is added. Natural exit
with `join` still drains output, EOF, and exit independently. Explicit shutdown
already discards output. Startup order, direct-command argv/cwd, renderer behavior,
and application ownership stay with their existing layers.

App teardown, explicit pane removal, and saved-workspace replacement all drop
their existing worker owners. They therefore use the corrected shutdown path
without application edits. Direct commands and shell panes use that same path.

## Regression and measurements

The real-PTY tests fill a raw PTY input queue while retaining a non-reading slave.
They cancel pending write and empty-read waits without closing that slave, and
cover cancellation before waiting and repeated cancellation after I/O owners
are gone. Scripted worker tests cover a full command queue, discarded queued
input, cancellation before termination, reader/session destruction, repeated
shutdown, and cancellation errors after cleanup. Session tests check kill before
writer destruction, termination failure, Drop retry, and no wait after kill
failure. Existing reader-acquisition failure, backpressure, FIFO, output-drain,
exit-status, and lifecycle tests also pass.

The same [probe source](measurements/blocked-pty-shutdown/probe.rs) was compiled
against baseline source extracted from HEAD and the candidate, using identical
debug-build dependency versions from the workspace lockfile. Samples alternate
before/after. Linux `/proc` confirms both threads are in I/O waits before blocked
shutdown; no sleep guesses when the write has blocked. Each blocked input is
8 MiB. Timings start immediately before `shutdown_and_join`, excluding startup.

| Scenario | Samples per revision | Before | After median (range), ms |
| --- | ---: | --- | ---: |
| Normal shutdown, one pane | 20 | 50.706 ms median (50.508–51.066) | 50.725 (50.590–50.913) |
| Graceful SIGHUP handler, one pane | 5 | 50.786 ms median | 50.693 (50.610–50.776) |
| Blocked write, one pane | 3 | All exceeded 2,000 ms | 51.413 (51.232–51.444) |
| Blocked write plus full command queue | 3 | All exceeded 2,000 ms | 51.199 (50.931–51.208) |
| Blocked write, SIGHUP ignored | 3 | All exceeded 2,000 ms | 222.298 (222.283–222.471) |
| Three blocked panes, sequential close | 3 | All exceeded 2,000 ms | 154.795 (154.521–155.831) |

Normal shutdown differs by 0.019 ms between medians in these uncontrolled warm
samples; this does not establish a statistically significant performance change.
The graceful handler writes an external marker before exiting; every before/after
sample observed that marker. Ignoring SIGHUP exercises the existing forced-kill
fallback. These observations are not hard shutdown deadlines.

Every completed candidate probe returns to four descriptors and one thread, with
all direct child PIDs absent, including zombies. Three active panes use seven
threads and 19 descriptors; baseline uses seven threads and 13 descriptors.
The six additional descriptors are the two cancellation endpoints per pane.
After joins, the harness allows up to one second for `/proc` task entries to
disappear, separately from the measured join duration. The runner uses Linux
subreaper support to kill and reap baseline timeouts; candidate samples need no
watchdog cleanup. Raw samples and numeric results are retained under
[measurements](measurements/blocked-pty-shutdown/).

Reproduce using the same source and built rlibs:

```sh
rustc --edition=2024 docs/measurements/blocked-pty-shutdown/probe.rs \
  -L dependency=target/debug/deps \
  --extern terminal_pty=target/debug/libterminal_pty.rlib \
  -o /tmp/blocked-pty-shutdown/probe-after
python3 docs/measurements/blocked-pty-shutdown/measure.py \
  /tmp/blocked-pty-shutdown/probe-before \
  /tmp/blocked-pty-shutdown/probe-after /tmp/blocked-pty-shutdown/measurements
```

`--edition` selects the Rust edition, `-L dependency` resolves dependencies,
`--extern` selects the revision's adapter rlib, and `-o` selects the executable.
Build the baseline from `b5676a0` in an isolated directory with the workspace
lockfile; do not reset a working checkout. The runner requires Linux `/proc`,
Python 3, and subreaper support. It uses two-second watchdogs for known baseline
hangs and cleans only its own probe processes and reported child PIDs.

## Validation

- PTY package: 36 tests passed, including five new regression tests.
- Workspace/all-target tests: 895 passed, 31 ignored, zero failures; four doctests passed.
- Formatting, workspace/all-target check, Clippy with warnings denied, release
  workspace build, and whitespace checks passed.
- Native Linux application shell-exit matrix: all 12 Bash/Zsh cases passed,
  including single zero/nonzero exits, splits, tabs, mixed panes, and startup exit;
  direct child reaping, survivors, integration cleanup, and saved bytes checked.
- Native Linux direct-command tests: three tests passed, covering `-e`,
  `--execute`, `--`, exact argv/cwd, missing executable, and Vim/Neovim Unicode paths.
- Native release window-close matrix: eight Bash/Zsh cases passed, one/three tabs,
  early/prompt-ready close. KWin targeted only the launched PID. Every app
  received `CloseRequested`, exited zero, removed integration files, and preserved
  saved bytes. All recorded direct shell PIDs were gone. Early close can precede
  PID markers, so those cases do not enumerate every startup child. First frame
  preceded PTY startup in all eight launches. Close submission through process
  exit was 129.73–587.59 ms; these warm samples are not controlled timing proof.

Full local validation logs are under `/tmp/blocked-pty-shutdown/`. Native GUI
tests ran outside the sandbox with KDE Wayland and the GPU. The workspace run
used a temporary fontconfig file including repository font fixtures. Ignored
native tests were selected explicitly for the shell/direct-command matrices;
unrelated concurrent GPU tests were not rerun.

## Limits and manual acceptance

Windows still uses portable-pty's existing blocking I/O: this change does not
implement ConPTY write cancellation or claim to fix a blocked Windows write.
macOS uses the new Unix implementation but was neither cross-compiled nor run;
only the Linux target is installed. Windows needs a separate native mechanism
behind the same cancellation capability and native proof. Custom backends with
the default absent cancellation capability retain their existing behavior.

`terminate` remains a queued FIFO operation; `join` waits for natural/previously
requested exit. Use `shutdown_and_join` or worker Drop to cancel blocked I/O.
OS kill/wait failures still have no bounded cleanup guarantee. Direct children
are owned and reaped; arbitrary detached descendants are not managed. Reader
cancellation now permits explicit worker shutdown even if a descendant retains
the slave, but it does not terminate that descendant. Panic/unwinding and OS
thread-creation failure behavior were not redesigned. No general throughput,
long soak, Windows, or macOS performance claim is made.

Linux manual acceptance, Bash and Zsh:

- [ ] Confirm first frame, immediate typing, focus, and shell startup behavior.
- [ ] Paste large input into a raw non-reading child, then close the pane/window;
  repeat across tabs, both split orientations, and saved-workspace replacement.
- [ ] Repeat with SIGHUP ignored and during output backpressure; check application
  exit, absent direct child PIDs, stable FD/thread counts, and integration cleanup.
- [ ] Rapidly close during startup; race `exit 0`/`exit 23` with sibling close;
  confirm surviving panes, saved bytes, direct editor argv/cwd, and final exit.

Windows manual acceptance, cmd and PowerShell with/without profiles:

- [ ] Run `scripts/test-shutdown.ps1 -Runs 3` on an interactive desktop after a
  release build; retain actual-HWND targeting and process-descendant logs.
- [ ] Reproduce a non-reading ConPTY child with large input and a full command
  queue. Treat recovery as unverified until native cancellation is implemented.
- [ ] Repeat tabs/splits, rapid startup close, saved-workspace replacement,
  natural exits, direct commands, and missing-program diagnostics.
- [ ] Check shell/conhost cleanup, first frame, typing, focus, integration files,
  and unchanged saved workspace bytes after natural exit.

macOS acceptance requires the Unix cancellation regressions, real shell/direct
command lifecycle tests, and the same visual/window-close/cleanup checks.
