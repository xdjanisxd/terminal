# Resource cleanup and memory safety audit

Audited 2026-10-21 from freshly fetched master `7d82ca1`, on
`fix/resource-cleanup-memory-safety`. The initial working tree was clean and the
existing task branch already pointed at master. No commits or pushes were made.

## Verified defect and fix

`PortablePtySession` documented a backend Drop safety net but implemented none.
Rust child-handle destruction neither terminates nor reaps a live child. Normal
application sessions use `PtyWorker`, which explicitly terminates and joins them;
this defect affects direct adapter owners and unwinding before worker ownership.
It is not evidence of a leak during ordinary tab closure.

The adapter now checks the child during Drop, kills and waits for a live child,
and skips killing a child already observed to have exited. It does not wait after
a failed kill. Cleanup recovers access to a poisoned child mutex without locking:
Drop has exclusive ownership and uses `Mutex::get_mut`.

Child cleanup precedes field destruction. In particular, portable-pty's Unix
writer destructor writes newline/EOT, which can block against a non-reading
child. The safety net must terminate the child before dropping that writer.
Explicit worker termination and its ordering remain unchanged.

The regression in `crates/terminal-pty/src/lib.rs` checks live, already exited,
and failed-kill cases, including writer-drop ordering. It failed before the fix
with zero kill/wait calls where one of each was required. Production changes add
no polling, timers, threads, dependencies, allocations, or lock acquisitions;
the only added work is during adapter destruction. Existing queue capacities,
caches, input/output paths, startup, and rendering are unchanged.

## Ownership and bounds reviewed

| Area | Evidence and decision |
| --- | --- |
| PTY worker | Owns one session thread and one blocking-reader thread. Eight queued commands and eight events; output chunks are at most 1 KiB. Shutdown discards unread events, sends termination, and joins both threads. Reader-acquisition errors explicitly terminate the consumed session. No queue change warranted. |
| Portable adapter | Spawn-error locals own and release PTY handles. Session owns child, master, writer, and initial reader; reader ownership transfers once. Fixed the missing direct-session destructor. |
| Tab/split closure | App removes runtime ownership after workspace removal/focus transfer. Removed workers terminate/join. Existing repeated real-worker test exercises 12 rounds of tab/split creation and closure. |
| Workspace replacement | Saved-workspace replacement drops active/inactive workers, clears old exit identities, and creates fresh runtimes. Initial `load_workspace` is used only before live startup. Existing restoration and exit-race tests passed. |
| Application shutdown | `shutdown_sessions` blocks restart, drops workers and startup metadata; Linux releases renderer/window before display teardown. One config watcher belongs to the process lifetime, not individual panes; it is detached and has no explicit join. |
| Font caches | Bitmap LRU: 256 entries; shaped-text LRU: 512 entries; character/grapheme fallback caches clear at 512. Grapheme fallback keys have the core scalar bound. Bold companion/instance maps are bounded by faces in the fixed font database. Scale changes clear shaped-text identity; glyph/atlas keys include physical size and face/weight/style. |
| GPU atlas | One 1024 x 1024 RGBA texture (4 MiB), 256 fixed 64 x 64 slots. Slot reuse removes the prior entry and LRU key. No forced eviction or allocation changes warranted. |
| GPU buffers and composition | Instance buffers retain geometric high-water capacity. CPU scratch/previous-frame vectors retain capacity intentionally. Composition target replacement drops previous owned views/bindings; ordinary same-format resize retains pipelines/atlas. Actual driver retirement is asynchronous and was not measured. |
| Renderer recreation | Suspend drops renderer ownership; resume reconstructs it. Surface errors use existing reconfiguration, with draw-resource reset only on format change. Font family/intensity reload remains restart-required; size/DPI changes reuse the renderer. No ownership cycle or project-owned unsafe GPU reference was found. |

Entry limits are not universal byte limits: shaped UI strings and retained buffer
capacity depend on content and the largest workload. This audit does not prove an
absolute process-memory ceiling or the absence of every memory safety defect.

## Before/after Linux measurements

The [probe and raw logs](measurements/resource-cleanup-memory-safety/) use 40 actual
PTY sessions. Each Python child reports readiness, ignores SIGHUP, and sleeps
without reading input. The caller retains the output reader across session Drop
so closing the final master cannot hide the missing explicit child cleanup.
The Python supervisor is a Linux child subreaper and kills/reaps baseline leaks
after measuring them. These are short native Linux PTY runs in the command
sandbox, not a desktop application's multi-hour RSS soak.

| Measurement, after 40 direct session drops | Master | Fixed |
| --- | ---: | ---: |
| Child processes remaining | 40 | 0 |
| Sum of remaining child RSS | 376,004 KiB | 0 KiB |
| Parent FDs, start/end | 4 / 4 | 4 / 4 |
| Parent threads, start/end | 1 / 1 | 1 / 1 |
| Parent RSS at end | 2,808 KiB | 2,800 KiB |
| Total spawn/drop duration | 584 ms | 8,639 ms |

The RSS sum counts shared pages once per child; it is not unique physical memory.
Parent RSS differences are allocator/run variation, not a claimed memory saving.
The duration increase is necessary cleanup: the existing backend allows about
200 ms for a child ignoring SIGHUP before force-killing it. The baseline simply
abandons children. An explicit terminate-and-observe-exit control left zero
children, four FDs and one thread both before and after (8,822 / 8,584 ms for 40
cycles). These samples are not a performance benchmark or a shutdown deadline.

To reproduce from the repository root on each revision, retain the baseline
executable before building the fixed revision:

```sh
cargo build -p terminal-pty
rustc --edition=2024 docs/measurements/resource-cleanup-memory-safety/probe.rs \
  --extern terminal_pty=target/debug/libterminal_pty.rlib \
  -L dependency=target/debug/deps -o /tmp/resource-drop-probe
python3 docs/measurements/resource-cleanup-memory-safety/run_probe.py /tmp/resource-drop-probe
python3 docs/measurements/resource-cleanup-memory-safety/run_probe.py /tmp/resource-drop-probe explicit
```

`--extern` selects the built adapter, `-L dependency` resolves its existing Rust
dependencies, and `explicit` selects the already-terminated control. The probe
requires Linux `/proc`, Python 3, and subreaper support. FD snapshots include the
temporary directory-enumeration FD consistently in both revisions.

## Open stability findings

The later [blocked PTY shutdown investigation](BLOCKED_PTY_SHUTDOWN.md) fixes
Unix I/O cancellation and verifies Linux recovery for finding 1 below. This
section retains the original observations; Windows cancellation remains open.

1. **Blocked input can still hang worker shutdown.** A raw-mode child that never
   reads input, followed by an 8 MiB queued write, blocked shutdown for two seconds.
   Killing the child externally still did not release the session thread within
   another five seconds in this run. The controller waited in a futex and the
   session thread in `wait_woken`; the reader had finished. The supervisor then
   killed the probe and reaped the child. The session thread performs synchronous
   `write_all`; it cannot process the queued termination command while blocked.
   A full command queue can also block termination enqueueing. The new Drop
   safety net cannot execute until the blocked session thread releases ownership.
   [Source and raw diagnostics](measurements/resource-cleanup-memory-safety/blocked-write.log)
   are retained. No change is made here: cancellable input needs a separate
   platform/I/O design that preserves byte ordering and normal shutdown behavior;
   replacing blocking I/O or adding hot-path state speculatively violates this
   task's minimal-change/performance constraints.
2. **Concurrent opt-in GPU tests crash in the installed Vulkan stack.** The
   [focused follow-up](GPU_CONCURRENT_VALIDATION.md) resolved the original core
   to the background-opacity test inside Vulkan debug-object naming. Both new
   concurrent runs crashed, including one with the font prerequisite satisfied;
   a serial run passed all seven tests. A standalone safe-wgpu probe with no
   project rendering or PTY code reproduced the same loader path, and an older
   core predates the PTY change. The PTY Drop edit is excluded; the exact upstream
   loader/driver-layer/wgpu cause remains unresolved. No project renderer change
   is justified by this evidence. Concurrent validation remains failing.
3. **Process-tree cleanup is not guaranteed.** The adapter terminates the direct
   child. Descendants retaining a slave PTY may delay reader EOF/join. This was a
   source-review risk, not a measured leak in the Bash/Zsh acceptance matrix.
   Failed kill/wait and forced process termination cannot guarantee cleanup.

These findings mean long-running stability is not fully verified. The later
Linux blocked-input fix does not resolve Windows cancellation or the concurrent
GPU limitation.

## Validation and limits

Final production revision passed formatting, workspace/all-target checks,
workspace/all-target tests (889 passed, 31 ignored), four doctests, Clippy with
warnings denied, release workspace build, and whitespace checks. The PTY package
passed 31 tests, including the new three-case destructor regression. Full raw
validation logs are local under `/tmp/resource-cleanup-audit/final-*`.

Native desktop Linux Bash/Zsh application-exit tests passed all 12 scenarios:
zero/nonzero exit, split, tabs, mixed layouts, and immediate startup exit. They
verify child reaping, survivor behavior, integration-file cleanup, and unchanged
saved-workspace bytes. Seven GPU regressions passed across serial/separate runs,
covering pipelines, glyph/atlas weight identity, backgrounds, cursor, emoji DPI,
warm caches, transparency, palette, and hints. Initial failures are retained in
the measurement directory rather than being hidden by successful reruns.

No Windows/macOS toolchain or desktop was available (only the Linux Rust target
is installed). ConPTY handles, Windows process termination, DirectX resource
retirement, and macOS process/Metal behavior require native acceptance. No Miri,
sanitizers, VRAM accounting, multi-hour soak, automated suspend/resume stress, or
interactive font/resize resource plateau measurement was performed.

## Manual acceptance checklist

All unchecked items remain manual work; they are not implied by automated tests.

| Check | Linux | Windows |
| --- | --- | --- |
| Repeated tabs/splits: close foreground/background panes, retain survivor and focus, then close app | [ ] Track `/proc/PID/fd`, task count, child PIDs and RSS over repeated batches | [ ] Track process/thread/handle counts and working set; no residual CMD/PowerShell children |
| Repeated saved-workspace replacement, including startup/exit races | [ ] Old shell PIDs and integration directories disappear; saved bytes preserved | [ ] Old ConPTY/session handles and children disappear; saved bytes preserved |
| Zoom/reset font size, resize, maximize, minimize/restore, suspend/resume | [ ] Text/selection/colors remain correct; memory/resources plateau after warm-up | [ ] Check DPI and DirectX dedicated/shared GPU memory as well as rendering |
| Long-running shell, editor, Unicode output, scrollback and unique glyph churn | [ ] Capture RSS/FD/thread/child samples for several hours; distinguish history/cache warm-up | [ ] Capture working set/private bytes/handles and GPU memory for several hours |
| Natural shell exit and native window close during output/startup | [x] Bash/Zsh natural-exit matrix; [ ] interactive window-close stress on final build | [ ] CMD/PowerShell, early close, busy output; verify process exit and no leftovers |
| Input backpressure and descendants retaining PTY | [ ] Reproduce/document current blocked-input failure before approving any cancellation redesign | [ ] Test a non-reading ConPTY child and descendants; document behavior |

On macOS repeat the same checks with shell PID/open-file/thread counts, resident
memory, and Metal resources; none were run in this environment.
