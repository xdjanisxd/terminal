# M6 compatibility, fuzzing, and performance baselines

These are reproducible workloads and coverage boundaries for later compatibility and performance work. Timings are observations, not test assertions or release budgets.

## Compatibility coverage

`crates/terminal-core/tests/compatibility_fixtures.rs` replays representative byte streams through the incremental parser. The fixtures cover a colored shell prompt redraw with deferred OSC title traffic, a full-screen editor entering and leaving the alternate screen, multiplexer cursor/margin/query traffic, a shell editing and replacing an input line, and a TUI updating progress rows through cursor addressing and erasure. Inputs are split at small byte boundaries, including one byte at a time. They assert visible cells, selected attributes, cursor state, modes, and reply order where relevant. They validate implemented behavior; they are not a claim of compatibility with complete applications or every terminal protocol.

`crates/terminal-core/tests/deterministic_inputs.rs` runs fixed pseudo-random byte streams at three parser chunk sizes and fixed state-operation streams covering scrollback, selection, and resize. It checks cursor bounds, scrollback and reply limits, viewport bounds, and visible-cell addressability. These tests run on stable Rust in CI.

Known protocol gaps include remaining CSI/SGR styles and modes, colon-form colors, most OSC/DCS semantics, secondary/tertiary DA replies, origin mode, and further alternate-screen behavior. Clipboard support outside Windows and broader real-application integration remain deferred. See [CURRENT_STATE.md](CURRENT_STATE.md) for the implemented slice.

## Fuzzing

The independent `fuzz/` Cargo workspace keeps libFuzzer and nightly-only tooling out of ordinary workspace builds and CI. Its targets are:

| Target | Input and invariant |
| --- | --- |
| `parser_bytes` | At most 4 KiB of arbitrary PTY bytes, fed in seven-byte chunks. After each chunk, cursor, scrollback, viewport, reply-queue, and visible-cell bounds must hold, even if a semantic operation reports an error. |
| `state_actions` | At most 4 KiB of deterministic operation triples selecting index, page navigation, selection, selected-text read, and bounded resize. Cursor, history, and viewport bounds must hold after every action. |

On a supported Unix-like host, install a nightly toolchain and `cargo-fuzz`, then from the repository root run:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run parser_bytes -- -max_len=4096
cargo +nightly fuzz run state_actions -- -max_len=4096
```

Keep any minimized regression input as a reviewed deterministic fixture or test. Run `cargo test -p terminal-core --test deterministic_inputs --test compatibility_fixtures` for the CI-safe subset. The fuzz binaries were not executed in the initial Windows baseline: `libfuzzer-sys` was unavailable in the local offline registry, and `cargo-fuzz` was not installed. Target compilation and fuzz findings still need a supported host run.

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
