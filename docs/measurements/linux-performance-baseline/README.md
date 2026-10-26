# Linux release performance baseline

Baseline captured 2026-10-26 on `master` / `perf/baseline-regression` at
`82838573f4ce8cba454ca75bbd0e45357e95965a`. It uses the existing fixed-work
benches plus opt-in measurement scripts. It adds no production instrumentation
or dependencies.

## Host and measurement conditions

- Debian GNU/Linux 13.7, Linux `6.12.113+deb13-amd64`, x86_64.
- Intel Core i5-1250P; available CPU affinity and before/after load averages are
  recorded in [release-cpu.json](release-cpu.json). The host was not pinned to
  one CPU and frequency scaling remained enabled.
- Intel Iris Xe Graphics (ADL GT2), Vulkan, Intel Mesa driver
  `25.0.7-2+deb13u1`.
- KDE Plasma/KWin 6.3.6, Qt 6.8.2, Wayland. Displays were 1920×1080 at scale
  1.0, with 60 Hz internal and 75 Hz external outputs.
- Rust 1.99.0, Cargo 1.99.0. All code measurements use release builds.
- No `WGPU_BACKEND` override. No shell profile was loaded by the lifecycle
  harness: each child was `/bin/sh -c 'printf READY; exec sleep 300'` with an
  isolated temporary home and XDG directories.
- App processes were fresh. System page and global font caches were not
  cleared. Each lifecycle sample used a fresh user-level XDG cache, so
  per-user font/driver cache state was reset; the OS file cache stayed warm.
  Load averages were recorded because the host was not otherwise isolated.

Raw samples and per-run startup logs live beside this report. The small-sample
p95 uses nearest-rank selection, so p95 equals the maximum for three, five, or
seven runs. These are host baselines, not universal performance promises.

## CPU parser, state, resize, and projection

`benchmark_linux.py` repeated both existing harnesses seven times. Each
operation and fixture is documented in [BASELINES.md](../../BASELINES.md).
The table reports median, run-level p95, and coefficient of variation (CV).

| Workload | Median | p95 | CV |
| --- | ---: | ---: | ---: |
| Plain parser/state feed | 8.31 µs/op | 8.69 µs/op | 1.6% |
| Escape-heavy parser/state feed | 4.11 µs/op | 4.27 µs/op | 1.8% |
| Scrollback page-up/down pair | 3.9 ns/pair | 4.5 ns/pair | 5.3% |
| Selection extension | 4.0 ns/update | 4.7 ns/update | 7.0% |
| Resize width 80↔1, wide Unicode fixture | 477.7 µs/pair | 717.4 µs/pair | 16.9% |
| Column resize pair, empty history | 66.0 µs/pair | 72.7 µs/pair | 7.3% |
| Column resize pair, about 2,000 history rows | 1.35 ms/pair | 1.79 ms/pair | 13.2% |
| Column resize pair, about 10,000 history rows | 9.41 ms/pair | 9.80 ms/pair | 2.3% |
| Height resize pair, about 10,000 history rows | 31.8 µs/pair | 34.4 µs/pair | 3.6% |
| Visible projection, 80×30 | 21.9 µs/op | 29.1 µs/op | 14.7% |
| Output echo plus projection | 20.7 µs/op | 21.3 µs/op | 4.3% |
| Three-pane layout plus projection | 25.1 µs/op | 26.1 µs/op | 3.9% |

Nanosecond navigation results are below useful wall-clock gate resolution.
Reflow timings include two resizes per pair. The 10,000-row fixture starts
near the configured history cap.

### Comparison with the earlier reflow report

I rebuilt the earlier scrollback-reflow commit
`2f8ae3794070038fb8987a0acf6d192b3013bf52` from an archive and ran seven
interleaved pairs of the same release benchmark binaries, alternating old and
current order. Raw output is
[reflow-commit-interleaved.txt](samples/reflow-commit-interleaved.txt).

| Workload | Earlier commit median | Current median | Change |
| --- | ---: | ---: | ---: |
| Plain parser/state feed | 8.12 µs/op | 8.04 µs/op | −0.9% |
| Escape-heavy parser/state feed | 3.97 µs/op | 4.06 µs/op | +2.2% |
| 2,000-row column resize pair | 1.14 ms/pair | 1.19 ms/pair | +4.8% |
| 10,000-row column resize pair | 8.19 ms/pair | 8.42 ms/pair | +2.8% |
| Unicode 80↔1 resize pair | 477 µs/pair | 462 µs/pair | −3.2% |

The 2,000-row case had 9.3% old and 6.0% current CV; Unicode resize had about
23% CV on both revisions. Neither reflow delta crosses its gate. The intervening
wide-cell fix extends the trimmed reflow row by one continuation cell when
needed for correctness; the interleaved run found no meaningful slowdown from
it. These paired absolute medians differ from the separately captured release
baseline because CPU frequency/load varied; use the interleaved deltas for
revision attribution. The older single-run report's 7.00 ms result for 10,000 rows was not
reproduced by the repeated earlier-commit median (8.19 ms). No confirmed
regression was found, so no optimization was made.

## Large PTY output

The existing `throughput-workload` example feeds an 8 MiB fixed block through
the parser. Seven release runs measured **8.96 MB/s median** (elapsed p95
951 ms). The app's existing `--throughput-run finite` workload sent 8,519,680
bytes through a PTY and rendered them with the real Wayland surface. It measured
**7.43 MB/s median**, **1,147 ms median / 1,167 ms p95**, and 66–70 frames per
run. Parser time was 978 ms median / 992 ms p95. PTY pending queues drained to
zero; peak queue was 9–10 chunks (9–10 KiB). Frame interval p50 was 18.17 ms
median; per-run frame p95 was 18.51 ms median / 18.69 ms p95.

Renderer totals per presented frame in that app workload were 35 µs projection,
1.32 ms instance generation, 48 µs upload, 221 µs queue submission, and 2.07 ms
render/present median. These are opt-in diagnostic aggregates and include
driver/compositor effects. Full samples are in
[visible-pty-throughput.txt](samples/visible-pty-throughput.txt) and
[parser-throughput.txt](samples/parser-throughput.txt).

## Renderer and GPU work

The ignored release test `renderer_workload_baselines` warms each fixed workload,
measures 100 offscreen frames, and waits for queued GPU work. The five-run
median / p95 values below are per outer frame in microseconds. Three-pane values
include three pane draws per outer frame.

| Workload | Projection | Instance generation | Upload | Queue submission | Wall time with GPU wait |
| --- | ---: | ---: | ---: | ---: | ---: |
| Sparse 80×30 | 28.5 / 36.8 | 86.5 / 110.4 | 3.3 / 4.4 | 64.6 / 95.8 | 857 / 1,230 |
| Dense 160×50 | 143.5 / 179.0 | 683.4 / 911.9 | 40.9 / 53.0 | 94.5 / 137.0 | 2,255 / 2,622 |
| Unicode 80×30 | 23.7 / 30.5 | 25.2 / 30.5 | 0.8 / 1.2 | 41.1 / 71.3 | 514 / 792 |
| Three panes | 79.6 / 86.9 | 622.5 / 679.4 | 14.6 / 16.2 | 126.4 / 149.7 | 1,563 / 1,625 |

Each cell is `median / p95` in µs. Raw adapter details and samples are in
[gpu-vulkan-intel-iris-xe.txt](samples/gpu-vulkan-intel-iris-xe.txt). This test
does not measure surface acquire/present. `wall time with GPU wait` is a
driver-backed wall-time proxy, not a hardware timestamp of GPU execution. The
app PTY run above supplies surface and compositor timing.

## Startup, PTY, idle, tabs, panes, and normal shutdown

`measure_linux_lifecycle.py` ran three ten-second idle samples for each layout.
It logs `first-frame-presented` before showing the window; `window-shown`
follows immediately. Startup p95 is nearest-rank across three launches.

| Layout | First surface frame median / p95 | First shell output rendered median / p95 | Idle RSS median (run range) | Idle CPU median / sample p95 | KWin close-to-exit median / p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| One pane | 214 / 228 ms | 233 / 240 ms | 95,072 KiB (93,616–95,188) | 0% / 2.00% | 72 / 74 ms |
| Three panes | 197 / 223 ms | 207 / 232 ms | 98,812 KiB (96,324–99,004) | 0% / 2.00% | 173 / 174 ms |
| Three tabs | 218 / 233 ms | 227 / 242 ms | 95,984 KiB (95,944–96,084) | 0% / 2.03% | 174 / 176 ms |

Idle CPU is from `/proc/<pid>/stat` sampled every 500 ms. One scheduler tick
quantizes a sample to about 2%; report the median as zero and treat the p95 as
resolution, not sustained 2% usage. RSS is the terminal process only. Close
timing includes the KWin D-Bus close request and compositor/event-loop shutdown.

For one pane, the startup diagnostics separated the stages as follows:

| Stage | Median / p95 |
| --- | ---: |
| Process start → first surface frame | 214 / 228 ms |
| PTY allocation (`pty-create-begin` → `pty-created`) | 77 / 114 µs |
| Child spawn (`pty-created` → `pty-spawned`) | 1.75 / 1.90 ms |
| Child spawn → first PTY bytes | 2.18 / 2.21 ms |
| First bytes → terminal state updated | 99 / 317 µs |
| State updated → shell output frame presented | 8.91 / 14.02 ms |

The shell startup command is deliberately fixed and profile-free. These numbers
measure PTY creation, `/bin/sh` process startup, first output, and presentation;
they do not include a user's Bash/Zsh profile or plugin initialization. Startup
logs separately attribute system-font discovery (29.1 ms median), total font
family preparation (39.7 ms), wgpu initialization (68.8 ms), rectangle-pipeline
creation (36.7 ms), and glyph-pipeline creation (7.7 ms). Nested costs overlap;
do not sum them. Per-run logs and exact generated workspace configurations are
in [lifecycle-run-20261026](lifecycle-run-20261026/).

## Normal and blocked-write shutdown

The existing Linux PTY shutdown probe was compiled against the current release
`terminal-pty` library. Each scenario ran seven times. The blocked case filled
an 8 MiB write against a child that does not read; the probe confirmed both
worker threads waited in kernel I/O and checked that child processes, file
descriptors, and threads were reaped.

| Scenario | Median | p95 | Range |
| --- | ---: | ---: | ---: |
| Normal, one PTY worker | 50.57 ms | 50.64 ms | 50.35–50.64 ms |
| Blocked write, one worker | 50.59 ms | 50.86 ms | 50.48–50.86 ms |
| Blocked write, three workers | 152.09 ms | 152.16 ms | 151.49–152.16 ms |

Raw output is [pty-shutdown.txt](samples/pty-shutdown.txt); the source probe is
[`probe.rs`](../blocked-pty-shutdown/probe.rs). Shutdown is sequential across
workers, so three-worker time is about three times the one-worker time.

## Opt-in regression check

Run the CPU gate only on the same Linux distribution/kernel, CPU model, Rust and
Cargo versions, architecture, and CPU affinity recorded in the baseline:

```sh
python3 scripts/benchmark_linux.py --check --runs 7
```

The runner compares seven-run medians and requires a rise beyond both three
times the baseline median absolute deviation and the per-stage floor: 15% for
parser and 10,000-row reflow, 25% for 2,000-row reflow and three-pane
projection, 30% for wide-Unicode resize, and 50% for the noisier projection
stages. It reports p95 and CV but does not gate on p95. This flags substantial,
repeatable slowdowns while keeping observed run noise below the gate. The
thresholds are intentionally conservative; a failed gate requires a paired
recheck before attributing a regression.

No fixed wall-clock gate is set for GUI startup, throughput, idle CPU/RSS,
normal shutdown, or GPU frames. Their values depend on cache state, display
refresh, driver, compositor, and system load. Repeat with the same command and
environment, then compare medians, p95, and raw samples.

## Reproduction commands

Build the release app and throughput helper:

```sh
cargo build --release -p terminal-app --bin terminal --example throughput-workload
```

Measure or gate the existing CPU microbenchmarks:

```sh
python3 scripts/benchmark_linux.py --runs 7 --output target/perf/linux-cpu.json
python3 scripts/benchmark_linux.py --check --runs 7
```

Repeat the Wayland lifecycle, three-pane, three-tab, idle CPU/RSS, startup, and
normal-close measurements. This requires an active KDE/KWin Wayland session and
`qdbus6`:

```sh
python3 scripts/measure_linux_lifecycle.py --runs 3 --seconds 10 --output target/perf/linux-lifecycle
```

Repeat the visible 8.5 MB PTY workload and parser-only workload seven times:

```sh
for run in 1 2 3 4 5 6 7; do TERMINAL_THROUGHPUT_DIAGNOSTICS=1 target/release/terminal --throughput-run finite; done
for run in 1 2 3 4 5 6 7; do target/release/examples/throughput-workload parser finite; done
```

Repeat the offscreen GPU benchmark on the selected hardware adapter:

```sh
for run in 1 2 3 4 5; do cargo test -p terminal-renderer --release renderer_workload_baselines -- --ignored --nocapture; done
```

Recreate the historical reflow comparison from the earlier source commit:

```sh
mkdir -p /tmp/terminal-reflow-base
git archive 2f8ae3794070038fb8987a0acf6d192b3013bf52 | tar -x -C /tmp/terminal-reflow-base
for run in 1 2 3 4 5 6 7; do if [ "$((run % 2))" -eq 1 ]; then cargo bench --manifest-path /tmp/terminal-reflow-base/Cargo.toml --offline -q -p terminal-core --bench state_baselines; cargo bench -q -p terminal-core --bench state_baselines; else cargo bench -q -p terminal-core --bench state_baselines; cargo bench --manifest-path /tmp/terminal-reflow-base/Cargo.toml --offline -q -p terminal-core --bench state_baselines; fi; done
```

Build and run the real blocked-write shutdown probe:

```sh
cargo build --release -p terminal-pty
mkdir -p target/perf
rustc --edition=2024 docs/measurements/blocked-pty-shutdown/probe.rs -L dependency=target/release/deps --extern terminal_pty=target/release/libterminal_pty.rlib -o target/perf/pty-shutdown-probe
for run in 1 2 3 4 5 6 7; do target/perf/pty-shutdown-probe normal 1; target/perf/pty-shutdown-probe blocked 1; target/perf/pty-shutdown-probe blocked 3; done
```

`TERMINAL_THROUGHPUT_DIAGNOSTICS=1` and `TERMINAL_STARTUP_DIAGNOSTICS=1` are
opt-in. They affect only measurement runs; production defaults and hot paths
are unchanged.

## Linux acceptance checklist

The automated baseline verified startup/presentation, first PTY output, an
8.5 MB PTY stream, parser/reflow/Unicode fixtures, one and three panes, three
tabs, idle resource samples, and normal/blocked PTY shutdown. Complete these
visual checks on Linux after changes to terminal rendering, reflow, or input:

- [ ] Type into the prompt and confirm the first visible frame and shell output.
- [ ] Run a large output command; confirm no missing text and scroll through
      retained history with PageUp/PageDown.
- [ ] Resize repeatedly while output contains CJK wide characters and combining
      text; confirm glyphs and cursor remain aligned.
- [ ] Close one pane and a multi-pane window during idle and active PTY output;
      confirm clean exit and no surviving child process.

This run did not automate PageUp/PageDown or pointer-driven live window resize.
The CPU reflow bench checks state correctness/performance; it does not replace
the visible resize check. No production files or runtime instrumentation
changed.
