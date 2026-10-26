# M6 compatibility, fuzzing, and performance baselines

These are reproducible workloads and coverage boundaries for later compatibility and performance work. Timings are observations, not test assertions or release budgets.

## Compatibility coverage

`crates/terminal-core/tests/compatibility_fixtures.rs` replays representative byte streams through the incremental parser. The fixtures cover a colored shell prompt redraw with OSC title traffic, a full-screen editor entering and leaving the alternate screen, multiplexer cursor/margin/query traffic, a shell editing and replacing an input line, and a TUI updating progress rows through cursor addressing and erasure. Inputs are split at small byte boundaries, including one byte at a time. They assert visible cells, selected attributes, cursor state, modes, and reply order where relevant. They validate implemented behavior; they are not a claim of compatibility with complete applications or every terminal protocol.

`compatibility_v2.rs` adds DEC G0/G1 graphics, save/reset boundaries, ED3
history/selection preservation, and VT/FF regression coverage. An opt-in Linux
test replays real `tput` and `clear` output; see [v2 evidence](TERMINAL_COMPATIBILITY_V2.md).

`crates/terminal-core/tests/deterministic_inputs.rs` runs fixed pseudo-random byte streams at three parser chunk sizes and fixed state-operation streams covering scrollback, selection, and resize. It checks cursor bounds, scrollback and reply limits, viewport bounds, and visible-cell addressability. These tests run on stable Rust in CI.

Known protocol gaps include remaining CSI/SGR styles and modes, most OSC/DCS semantics, tertiary DA replies, and further alternate-screen behavior. Clipboard support outside Windows and broader real-application integration remain deferred. See [CURRENT_STATE.md](CURRENT_STATE.md) for the implemented slice and [TERMINAL_COMPATIBILITY.md](TERMINAL_COMPATIBILITY.md) for the compatibility polish validation.

## Fuzzing

The independent `fuzz/` Cargo workspace keeps libFuzzer and nightly-only tooling out of ordinary workspace builds and CI. Its targets are:

| Target | Input and invariant |
| --- | --- |
| `parser_bytes` | At most 4 KiB of arbitrary PTY bytes, replayed at 1-, 2-, 7-, 32-byte, input-derived, and variable chunk boundaries. This covers incomplete and cancelled CSI/OSC/DCS sequences, split UTF-8, and grapheme edits. Cursor, scrollback, viewport, reply-queue, and visible-cell bounds must hold after input, including semantic errors. |
| `state_actions` | At most 4 KiB of operations selecting index/reverse-index, page and row scrolling, selection, text reads, resize up to 32 × 16, Primary/Alternate switches, reset, and ASCII/CJK/combining/emoji printing. Cursor, history, and viewport bounds must hold after every action; stored row and wrap extents must fit dimensions, and every stored cell must be addressable at the end. |

The targets are split by ownership: `parser_bytes` feeds wire bytes to the incremental parser and semantics, while `state_actions` drives state transitions directly so resize/reflow and screen ownership are reached without needing a matching escape stream. Reviewed initial inputs live in `fuzz/seeds/<target>/`; libFuzzer's generated corpus, crash artifacts, and coverage stay ignored under `fuzz/`. Parser seeds cover malformed parameters, OSC/DCS cancellation, ST/BEL termination, split UTF-8 graphemes, and prior DEC graphics/alternate-screen/ED3/reset regressions. State seeds cover screen switches, resize, scrolling, selection, reset, truncated action records, and the minimized wide-cell reflow regression.

On Linux, install a nightly toolchain and `cargo-fuzz`, then run repeatable 60-second campaigns from `fuzz/`. Use a temporary writable corpus so libFuzzer does not modify reviewed seeds:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
cd fuzz
run_campaign() (
  set -e
  target=$1
  seconds=$2
  corpus=$(mktemp -d)
  trap 'rm -rf "$corpus"' EXIT
  cp -R "seeds/$target/." "$corpus/"
  cargo +nightly fuzz run "$target" "$corpus" -- -max_len=4096 -max_total_time="$seconds" -rss_limit_mb=1024 -timeout=10 -seed=20261026
)
run_campaign parser_bytes 60
run_campaign state_actions 60
```

Use `run_campaign parser_bytes 1800` and `run_campaign state_actions 1800` for 30-minute campaigns. The input cap, 1 GiB RSS limit, and 10-second per-input timeout bound local resource use. Reproduce a saved crash with `cargo +nightly fuzz run <target> <artifact-path>` from `fuzz/`; minimize it with `cargo +nightly fuzz tmin <target> <artifact-path>`. Keep minimized reproductions as reviewed seeds and deterministic terminal-core tests. Run `cargo test -p terminal-core --test deterministic_inputs --test compatibility_fixtures` for the stable-Rust subset.

Linux campaign record, 2026-10-26 (`x86_64-unknown-linux-gnu`, Rust 1.101.0-nightly, `cargo-fuzz` 0.13.2): `parser_bytes` ran 61 seconds and executed 147,709 inputs, reaching 1,808 edge counters at 136 MiB peak RSS. `state_actions` ran 61 seconds and executed 92,136 inputs, reaching 1,242 edge counters at 379 MiB peak RSS. Both finished without crashes or timeouts after fixing the discovered regressions. Edge-counter totals are target-specific, not whole-crate coverage.

## Benchmarks

The two `harness = false` Cargo benches print a fixed operation count, elapsed nanoseconds, and nanoseconds per operation. `std::hint::black_box` protects inputs/results from trivial removal. Run release-profile measurements from the repository root:

```sh
cargo bench -q -p terminal-core --bench state_baselines
cargo bench -q -p terminal-renderer --bench projection_baselines
```

| Stage | Fixed work per operation |
| --- | --- |
| `parse_plain_state` | Feed one 43-byte plain prompt/output sequence into one parser and 80 × 30 state; 2,000 feeds. |
| `parse_escape_state` | Feed an escape-heavy prompt/redraw sequence into one parser and 80 × 30 state; 2,000 feeds. |
| `scrollback_page_pair` | Page up and down in a state prefilled with 2,000 indexed rows; 10,000 pairs. |
| `selection_extend` | Extend an existing selection across the 80 × 30 viewport; 10,000 updates. |
| `visible_projection_80x30` | Build renderer-owned visible data from a populated 2,400-cell state; 500 projections. |
| `output_echo_plus_projection` | Feed a short shell echo and project the same 2,400-cell state; 500 cycles. This approximates only the parser/state/projection portion of an output redraw. |
| `three_pane_layout_plus_projection` | Compute three rectangles in a recursive split and project three populated pane states; 500 cycles. This excludes renderer GPU work. |

Initial single-run sample on Windows 10.0.26200.9550, Intel64 Family 6 Model 154 (Intel Core i5-1250P as identified in the earlier Windows trace), Cargo 1.98.1, `bench` release profile, base revision `4afa2d0`, 2026-09-24:

| Stage | ns/operation |
| --- | ---: |
| `parse_plain_state` | 12,787 |
| `parse_escape_state` | 6,107 |
| `scrollback_page_pair` | 4.2 per pair |
| `selection_extend` | 1.8 |
| `visible_projection_80x30` | 180,136 |
| `output_echo_plus_projection` | 169,262 |
| `three_pane_layout_plus_projection` | 104,403 |

The stages differ in content and pane dimensions; do not rank them as equivalent operations. A second nearby projection run varied by roughly 18–40%, so repeat samples on named hardware before comparing changes. The nanosecond-scale navigation/selection results especially need repeated samples or batching before drawing performance conclusions. No wall-clock assertion is part of normal tests. The operation counts and assertions make the workloads repeatable; the measured durations are environment-sensitive. These benches exclude PTY scheduling, font shaping/rasterization, GPU instance generation/upload, queue submission, present, and input-to-present latency.

## Current performance debt

The prior [PERFORMANCE.md](PERFORMANCE.md) Windows release trace identified full visible-grid projection and CPU instance regeneration on every present, even when instance uploads are empty. PowerShell output may arrive in separate PTY chunks and cause multiple presents for one key. Live resize can still trail the pointer or temporarily scale text; resize-ordering work removed stale-size configurations in one trace but did not establish smooth frame pacing. Damage-aware projection/instance updates, proof-based redraw suppression, and measured input-to-present latency remain future work. This baseline task makes no performance changes and starts no packaging work.

The subsequent [frame-pacing pass](PERFORMANCE.md#sparse-projection-and-pty-wake-follow-through-2026-09-24) reduces projected cells and folds PTY chunks already queued before redraw into that frame. Native Windows release resize smoke passed on an Intel Iris Xe host using the new default DX12 path. Native fuzz execution remains deferred until a suitable host is available.
