# Linux stability and resource cleanup

Measured 2026-10-26 on `test/linux-stability-resources`, starting at integrated
master `1b2abf5`. The initial worktree was clean. No production runtime code,
dependencies, Windows code, or macOS code changed.

## Scope and method

The audit reused the existing PTY ownership and blocked-write fixes, workspace
lifecycle tests, renderer GPU tests, scrollback bounds, and Linux release
baseline. It found no reproducible production leak or lifecycle defect.

The opt-in [Linux resource runner](../scripts/stress_linux_resources.py) samples
the running terminal through `/proc`: RSS, private/anonymous/file memory, CPU,
threads, FDs, and direct children. It writes isolated temporary homes and XDG
directories. KWin scripts target only each launched PID. A bounded cleanup path
signals only that terminal's observed child tree if the run fails.

Run the extended campaign from the repository root on Linux with KDE/KWin:

```sh
python3 scripts/stress_linux_resources.py --cycles 2 --idle-seconds 60
```

This runs two one-pane, three-pane, and three-tab create/close cycles, with a
60-second idle sample per process. It then writes 32 MiB of Unicode-heavy PTY
output in two passes and samples for another 60 idle seconds. The runner stores
JSON summaries and per-case startup logs under `target/perf/linux-resource-stress/`.
The retained [two-cycle results](measurements/linux-stability-resources/two-cycles-per-layout.json)
and [extended-idle results](measurements/linux-stability-resources/extended-idle-and-unicode.json)
include actual RSS, CPU, thread, FD, child, and shutdown measurements. Separate
[PTY shutdown](measurements/linux-stability-resources/pty-shutdown.json),
[native close](measurements/linux-stability-resources/native-close.json), and
[visible throughput](measurements/linux-stability-resources/visible-throughput.json)
records preserve the other scenario outputs.

Reuse the existing native close and blocked-write probes with:

```sh
python3 docs/measurements/blocked-pty-shutdown/native_close.py
cargo build -p terminal-pty
rustc --edition=2024 docs/measurements/blocked-pty-shutdown/probe.rs \
  -L dependency=target/debug/deps \
  --extern terminal_pty=target/debug/libterminal_pty.rlib \
  -o /tmp/linux-stability-resources-pty-probe
/tmp/linux-stability-resources-pty-probe normal 1
/tmp/linux-stability-resources-pty-probe blocked 1
/tmp/linux-stability-resources-pty-probe full 1
/tmp/linux-stability-resources-pty-probe blocked 3
```

## Measured results

The previous [Linux baseline](measurements/linux-performance-baseline/README.md)
used three 10-second samples. This campaign used 60-second samples. Results are
host observations, not universal limits.

| Layout | Earlier idle RSS median | This run before / peak / final RSS | This run threads / FDs | Idle CPU median / p95 |
| --- | ---: | ---: | ---: | ---: |
| One pane | 95,072 KiB | 92,688 / 94,376 / 94,376 KiB | 10 / 49 | 0% / 1.99% |
| Three panes | 98,812 KiB | 94,880 / 96,592 / 96,592 KiB | 14 / 59 | 0% / 1.99% |
| Three tabs | 95,984 KiB | 93,812 / 95,500 / 95,500 KiB | 14 / 59 | 0% / 1.99% |

Idle p95 is one scheduler tick at 500 ms sample intervals. It does not show
sustained 2% CPU. Final RSS stayed below the earlier medians; the three-tab
reading also fell below its earlier run range. Each window closed in 134 ms
for one pane and 230–233 ms for three sessions. The terminal PID and all one
or three shell PIDs disappeared after every run.

The Unicode scenario used a median 104.8% of one logical CPU while output was
active, then 0% median CPU during idle. It started at 91,768 KiB RSS and 49,528
KiB private memory. Two 16 MiB passes peaked at 151,256 KiB RSS and 109,016 KiB
private memory.
The first and second pass checkpoints were 151,248 and 151,256 KiB RSS, an 8 KiB
difference. After 60 idle seconds, final memory remained 151,256 KiB. The final
scrollback was at its 10,000-row limit. This stable second-pass plateau matches
bounded scrollback plus allocator/high-water retention; it does not show
continued growth. RSS alone cannot separate retained cells from freed pages
kept by the allocator.

The existing visible PTY throughput workload also ran: 8,519,680 bytes over
2.478 seconds, with 10,000 scrollback rows, zero pending PTY bytes, and a peak
queue of 10 chunks / 10,240 bytes. The repeated blocked-write probe ran normal,
8 MiB blocked, full command queue, and three-worker shutdown cases. One worker
returned in 50.8–51.1 ms; three blocked workers returned in 153.8 ms. Its
`/proc` snapshots returned from 9 FDs / 3 threads to 4 FDs / 1 thread for one
worker, and from 19 FDs / 7 threads to 4 FDs / 1 thread for three workers.
Every direct child PID was reaped.

KWin native close coverage ran eight Bash/Zsh cases across one/three panes and
early/ready shutdown. All shells exited, integration files were removed, and
saved workspace bytes stayed unchanged. Close-to-exit ranged 428–592 ms in that
matrix. App tests also ran 12 real tab/split create/close rounds and a new
Linux-only test that replaced a three-shell workspace 12 times, reaping all 36
observed shell PIDs.

## Renderer and ownership findings

The source audit found no additional leak. Existing owners remain bounded:
PTY workers join their reader/session threads; workspace replacement drops old
workers; scrollback caps at 10,000 rows; font caches and glyph atlas use fixed
entry/slot limits; GPU buffers may retain high-water capacity until renderer
destruction. The earlier [resource cleanup audit](RESOURCE_CLEANUP_MEMORY_SAFETY.md)
and [blocked PTY shutdown report](BLOCKED_PTY_SHUTDOWN.md) document those
ownership contracts and the existing fixes.

KWin reports Intel Iris Xe with Mesa 25.0.7. `vulkaninfo --summary` sees that
device and llvmpipe. Seven opt-in renderer GPU tests passed serially using the
repository JetBrains Mono fixture; those offscreen tests selected llvmpipe.
This host exposes no installed `intel_gpu_top` or per-process VRAM counter, so
the campaign cannot quantify driver memory retirement. The real-window runs
prove process/FD/thread teardown, not a byte-level GPU allocation count.

## Validation

- `cargo test --workspace --all-targets --offline`: 898 passed, 31 ignored.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo build --release --workspace --all-targets --offline`: passed.
- `cargo test -p terminal-pty --offline`: 36 passed.
- `cargo test -p terminal-renderer native_gpu_ -- --ignored --nocapture --test-threads=1` with fixture `FONTCONFIG_FILE`: 7 passed.
- `python3 scripts/stress_linux_resources.py --self-test`: passed.
- `python3 scripts/measure_linux_lifecycle.py --self-test`: passed.
- Visible PTY throughput, 60-second idle/resource campaigns, native KWin close,
  blocked PTY shutdown, 12-round tab/split closure, and 12-round workspace
  replacement all executed on Linux.

No confirmed defect required a production fix. Production performance impact is
zero: changes are limited to one opt-in script, a test-only lifecycle stress
case, and recorded evidence.

## Limits and manual acceptance

KWin-driven interactive window resize/reflow was not automated. The app's
resize/reflow unit and benchmark coverage ran in the full workspace suite, but
that does not prove repeated native-window resize behavior. Saved-workspace
replacement was stressed through the app test harness, not through repeated
interactive picker actions. GPU memory is not measured per process. Soaks were
bounded to 60 idle seconds and 32 MiB Unicode output, not hours.

Manual Linux acceptance checklist:

- [ ] Repeat tab/split creation and closure in the built desktop app while
  tracking `/proc/PID/{status,fd,task}` and child PIDs across several hours.
- [ ] Switch tabs and replace saved workspaces repeatedly; confirm old shell
  PIDs, worker threads, and integration files disappear after every switch.
- [ ] Resize the window repeatedly during Unicode output, then verify text,
  selection, scrollback, focus, and memory plateau after warm-up.
- [ ] Repeat suspend/resume, maximize/minimize, and DPI changes while checking
  renderer recovery and system GPU memory telemetry.
- [ ] Close the application during sustained output and with PTY input blocked;
  confirm bounded shutdown and no remaining child processes.
