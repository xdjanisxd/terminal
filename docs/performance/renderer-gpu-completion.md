# Renderer/GPU completion investigation

Branch: `perf/renderer-gpu-completion`. No commit or push.

## Plan

1. Trace dirty-state scheduling, projection, glyph/instance preparation, uploads, encoding, submission, acquisition, presentation and resize synchronization. Confirm locked wgpu APIs.
2. Extend opt-in throughput aggregates; separate encoding/submit/acquire/present/reconfigure and CPU frame total. Add bounded sampled completion callbacks and nonblocking polls; keep normal rendering unchanged. Add deterministic bookkeeping tests.
3. Collect three Windows x86_64 release runs for existing finite, short, long, ANSI, continuous and input workloads with default, DX12 and Vulkan backends where supported. Compare baseline and diagnostics overhead; include idle/resize observations when tooling permits.
4. Identify bottlenecks and pacing effects from measured evidence. Optimize only a safe material bottleneck; otherwise retain diagnostics/reporting.
5. Perform native acceptance, all required validation, reconcile task documentation and report measurements and limitations.

## Status

- Pipeline tracing, opt-in diagnostics, repeated release measurements, automated validation and manual Windows x86_64 native acceptance are complete. The user reported native acceptance passed on 2026-10-01. No renderer optimization was justified; only diagnostics/reporting and the diagnostic workload fixture are retained.
- Existing harness continuous/input workloads are finite 32 MiB streams, not indefinite-duration tests.

## Pipeline and timing boundaries

1. PTY parsing mutates terminal-owned state. The app coalesces invalidation with its existing frame scheduler and requests a redraw. The redraw handler applies pending resize and drains notified PTY output within the unchanged 64-event/4-ms fairness budget, then computes the visible pane rectangles and UI overlay. These app operations are outside renderer CPU time.
2. Renderer entry polls completion callbacks only when throughput diagnostics are enabled. Every visible pane projects the immutable terminal viewport into render data, including selection, search, cursor, scrollbar and UI overlay. Projection scans the visible grid even when only a few cells changed.
3. A nonzero surface is configured if needed; otherwise the frame is skipped. Surface acquisition occurs BEFORE instance generation. Acquisition can synchronize with surface availability. Lazy draw-resource creation and texture-view creation follow acquisition.
4. Each pane regenerates its rectangle/glyph instances in retained vectors. Nonblank text uses the bounded shaped-string cache, then the atlas lookup. Atlas misses rasterize through the CPU bitmap cache and write atlas textures. Thus instance generation includes font/atlas work and atlas texture-write calls; it is not solely geometry arithmetic.
5. Instance-buffer preparation compares current instances to retained previous instances, creates/grows buffers if necessary, writes changed contiguous spans (or merged spans when fragmented), and retains a CPU copy. Upload time includes this CPU comparison/copy and queue.write_buffer calls. Bytes are requested write volume, not measured bus traffic.
6. Each pane creates an encoder, records the clear/load render pass and instanced rectangle/glyph/overlay draws, and finishes the encoder. Encoding and queue.submit are now separate scopes. There is one queue submission per rendered pane, not necessarily one per window frame.
7. After the final pane submit, the diagnostic sampler may register a completion callback. The window receives pre_present_notify, then surface.present is called. Present-call duration measures only the API call. The existing legacy render_present scope spans acquisition through present; CPU-frame scope additionally includes projection, diagnostic poll, resource setup and any post-present recovery.
8. Suboptimal textures are presented before reconfiguration. Lost/outdated/other acquisition errors reconfigure and schedule existing recovery; timeout skips; out-of-memory exits. Zero-size surfaces skip configuration. Resize is applied before redraw; DPI changes recompute font metrics/grid through existing paths. Configuration can internally wait for GPU idle and is measured separately, including configuration outside redraw.

Normal rendering already has some Instant scopes for lifecycle/legacy aggregates. New frame/present clocks and completion channels exist only under TERMINAL_THROUGHPUT_DIAGNOSTICS=1; encode/submit clocks also preserve existing TERMINAL_RENDERER_DIAGNOSTICS tracing. Normal release emits no new messages, registers no callbacks and performs no new device polls. No terminal state, drawing, backend preference, present mode or scheduling policy was optimized.

Previously available throughput aggregates covered projection, generation, upload/preparation, a combined encoding-plus-submit scope and acquisition-through-present. Lifecycle tracing had acquisition and reconfigure events but no aggregate completion or CPU-frame totals. This patch separates the combined scope and adds aggregates, outcome/work counters and sampled completion bookkeeping; GPU execution, scanout and total frames-in-flight remain unavailable.

## Completion semantics

Cargo.lock resolves wgpu **25.0.2** (workspace requirement 25.0.0). Its local locked Queue source documents on_submitted_work_done as completion of earlier submitted GPU work; callback delivery requires submit, instance polling or device polling. Device.poll(PollType::Poll) is nonblocking; no PollType::Wait is used by these diagnostics. Surface.configure explicitly documents internal idle synchronization.

The sampler registers on the first and every sixteenth submitted frame, with at most one outstanding callback and a capacity-one channel. It times from just before the last pane's submit to callback execution. Callback delivery therefore includes submit/driver time, prior queue backlog, GPU execution and delayed delivery until a later submit/nonblocking poll. It is an **upper bound on queue completion latency**, not an isolated GPU-duration measurement. Earlier panes are covered by queue ordering but the timer starts at the last pane. Reset replaces the channel so late callbacks cannot contaminate a new workload. Reporting does one nonblocking poll; remaining samples are reported pending and never waited for.

The pending gauge counts sampled callbacks only. It does not measure all in-flight frames. No timestamp queries or backend-specific instrumentation were added: these runs cannot accurately distinguish short GPU execution from callback delivery delay. They measure no compositor latency, display refresh, scanout or input-to-photon latency.

## Diagnostics and reproducibility

Existing command: cargo build --release -p terminal-app --bin terminal --example throughput-workload. Run scripts/measure-throughput.ps1 -Runs 3 -Label renderer-gpu-default; repeat with existing WGPU_BACKEND=dx12 and WGPU_BACKEND=vulkan overrides and distinct labels. The default Windows adapter preference/fallback is unchanged. Raw parser/visible logs live under target/perf/throughput/<label> and are not committed.

The baseline was built and run before instrumentation. The diagnostic build uses the same release profile, 80 columns and 31 rows on Windows x86_64, Intel Iris Xe Graphics, driver 32.0.101.7088, Rust 1.98.1. Series run sequentially; they are not randomized or a controlled thermal/power experiment. All numbers are wall-clock scopes, potentially affected by driver scheduling and host activity. Aggregate stage totals divided by attempted frames are per-run means; medians in the result tables are medians of those run means, not individual-frame percentiles. Frame intervals are intervals between successful app-observed presents. Largest gap includes workload inactivity when applicable.

Finite/long/ANSI emit about 8 MiB, short emits about 1 MiB, continuous/input emit about 32 MiB. Continuous is bounded sustained output rather than indefinite streaming. Input injects a harness probe and records dispatch/child acknowledgement; it is not manual typing or Ctrl+C acceptance. The additional idle helper writes a marker, deliberately waits three seconds without output, and writes a final marker; this delay belongs solely to the test workload.

## Release results

The following are medians of three run means. All stage columns, completion bounds and frame intervals are **milliseconds**. MB/s is decimal visible-output throughput. CPU/frame includes diagnostic polling and driver calls, so it is CPU-side wall time rather than pure CPU execution. Default selected DX12. Explicit DX12 used Mailbox with maximum frame latency 2; Vulkan used Immediate with latency 2. The present-mode difference confounds a backend-only comparison.

| Backend | Workload | MB/s | CPU/frame ms | Projection | Generation | Upload | Encoding | Submit | Acquire | Present | Completion upper bound | Frame p50 | Largest gap |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default | finite | 6.722 | 0.954 | 0.034 | 0.548 | 0.018 | 0.083 | 0.076 | 0.021 | 0.129 | 8.478 | 8.950 | 35.612 |
| default | short | 0.747 | 0.710 | 0.021 | 0.018 | 0.175 | 0.069 | 0.069 | 0.021 | 0.133 | 10.114 | 10.313 | 19.303 |
| default | long | 7.785 | 1.220 | 0.037 | 0.462 | 0.264 | 0.067 | 0.071 | 0.019 | 0.123 | 7.940 | 9.376 | 17.252 |
| default | ansi | 7.101 | 0.589 | 0.021 | 0.079 | 0.100 | 0.074 | 0.072 | 0.021 | 0.121 | 7.119 | 8.629 | 17.139 |
| default | continuous | 7.083 | 0.797 | 0.033 | 0.426 | 0.015 | 0.082 | 0.072 | 0.022 | 0.100 | 8.437 | 8.954 | 35.116 |
| default | input | 7.172 | 0.776 | 0.032 | 0.426 | 0.017 | 0.075 | 0.067 | 0.020 | 0.094 | 8.442 | 8.935 | 34.919 |
| dx12 | finite | 6.749 | 0.999 | 0.034 | 0.571 | 0.018 | 0.085 | 0.077 | 0.022 | 0.144 | 8.565 | 8.981 | 37.293 |
| dx12 | short | 0.744 | 0.748 | 0.021 | 0.019 | 0.180 | 0.071 | 0.072 | 0.023 | 0.129 | 10.084 | 10.533 | 18.608 |
| dx12 | long | 7.851 | 1.192 | 0.039 | 0.462 | 0.268 | 0.068 | 0.070 | 0.019 | 0.094 | 8.138 | 9.394 | 17.413 |
| dx12 | ansi | 6.735 | 0.626 | 0.021 | 0.080 | 0.131 | 0.084 | 0.079 | 0.023 | 0.114 | 8.520 | 8.783 | 16.814 |
| dx12 | continuous | 7.160 | 0.758 | 0.032 | 0.427 | 0.014 | 0.075 | 0.067 | 0.020 | 0.101 | 8.447 | 8.913 | 36.940 |
| dx12 | input | 7.131 | 0.766 | 0.032 | 0.425 | 0.017 | 0.076 | 0.069 | 0.020 | 0.093 | 8.476 | 8.932 | 34.716 |
| vulkan | finite | 6.734 | 0.742 | 0.041 | 0.493 | 0.012 | 0.066 | 0.051 | 0.012 | 0.011 | 8.550 | 8.977 | 32.274 |
| vulkan | short | 0.770 | 0.254 | 0.019 | 0.021 | 0.036 | 0.052 | 0.052 | 0.013 | 0.015 | 10.046 | 10.173 | 16.630 |
| vulkan | long | 8.151 | 0.743 | 0.041 | 0.470 | 0.057 | 0.053 | 0.052 | 0.012 | 0.010 | 8.656 | 8.988 | 17.747 |
| vulkan | ansi | 6.815 | 0.232 | 0.020 | 0.068 | 0.005 | 0.050 | 0.044 | 0.009 | 0.009 | 3.503 | 3.139 | 16.753 |
| vulkan | continuous | 7.242 | 0.629 | 0.040 | 0.406 | 0.012 | 0.063 | 0.048 | 0.012 | 0.011 | 8.413 | 8.876 | 25.454 |
| vulkan | input | 7.228 | 0.632 | 0.040 | 0.411 | 0.012 | 0.063 | 0.049 | 0.012 | 0.011 | 8.391 | 8.873 | 28.408 |

All 54 instrumented output runs completed. Attempted, rendered and presented counts matched; skipped, suboptimal, surface errors, recovery frames and poll errors were zero. No surface reconfiguration occurred after workload-stat reset in these fixed-size runs. Zero reconfiguration time here is **not** evidence of cheap resize.

### Baseline versus diagnostics

| Workload | Baseline MB/s | Diagnostics MB/s | Difference | Parse / elapsed | CPU renderer / elapsed | Redraw / rendered / presented / terminal-data |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| finite | 6.275 | 6.722 | 7.1% | 83.4% | 10.4% | 136 / 136 / 136 / 135 |
| short | 0.703 | 0.747 | 6.3% | 89.5% | 6.9% | 135 / 135 / 135 / 134 |
| long | 7.449 | 7.785 | 4.5% | 80.4% | 13.0% | 129 / 129 / 129 / 128 |
| ansi | 6.437 | 7.101 | 10.3% | 86.2% | 8.1% | 152 / 152 / 152 / 151 |
| continuous | 5.649 | 7.083 | 25.4% | 87.1% | 8.8% | 524 / 524 / 524 / 523 |
| input | 5.890 | 7.172 | 21.8% | 87.5% | 8.6% | 520 / 520 / 520 / 519 |

This is a historical before/after comparison of instrumentation, **not an optimization result**. Sequential host runs improved throughput by 4.5–25.4% without changing render work. That variation prevents attributing improvements to this patch or quantifying tiny diagnostic overhead from this comparison. One baseline finite run completed with zero terminal-data frames and no interval percentiles; it remains in the raw logs and is excluded from pacing conclusions. No baseline stage/completion data existed.

Diagnostic polling itself was material enough to expose separately: representative default runs spent 6–26 ms cumulatively polling versus 83–405 ms total renderer time. Callback delivery clustered around the 8–10 ms frame cadence, which is consistent with delayed callback observation; it does **not** establish 8–10 ms of GPU execution. The pending sample count was zero at report time in these runs; actual total frames in flight are unknown.

### Mostly idle

Three default-backend idle runs each presented six frames (five terminal-data frames) during about 3.04 s. Their renderer CPU totals were 9.916–11.859 ms, with two initial buffer allocations, four writes and 1,456 uploaded instance bytes. The roughly 2.98 s largest gap is intentional workload inactivity. This supports damage-driven idle behavior; the one completion sample per run (9.030–10.564 ms) cannot characterize idle GPU duration. This short idle fixture does not characterize every cursor-blink, overlay-animation or background-tab case.

## Interpretation and hot paths

- **Finite / continuous / output-with-input:** parsing dominates wall time (83–88%). Renderer CPU accounts for about 9–10%. Within rendering, repeated instance generation dominates: about 0.43–0.55 ms/frame versus 0.03 ms projection, 0.07–0.08 ms submit and 0.09–0.13 ms present. Cache misses are scarce: one default continuous run made 989,148 shape calls/atlas lookups but only 62 shape misses and raster calls. Repeated lookup and instance rebuilding deserve a controlled follow-up; rasterization is not the steady-state dominant stage here.
- **Short lines:** parser/mutation work accounts for 89.5%, renderer about 6.9%. The known full-grid scroll-copy cost remains the strongest overall bottleneck. Generation is only 0.018 ms/frame. DX12 upload/write preparation averages 0.175 ms/frame despite just 6,272 cumulative instance bytes in a representative run; driver/API overhead needs more controlled profiling before changing buffer logic.
- **Long lines:** parser work remains dominant (80.4%). Renderer share rises to 13%, with generation 0.462 ms/frame and upload preparation 0.264 ms/frame on default. A representative run uploaded 3.415 MB in 224 writes; this includes CPU changed-span comparison/copy and write_buffer call cost, not measured GPU transfer time.
- **ANSI-heavy:** parser work accounts for 86.2%, renderer about 8.1%. Vulkan lowered CPU calls and increased frame frequency (median interval 3.139 ms versus default 8.629 ms), while throughput did not consistently improve. More FPS alone is not a success criterion. This repeated small ANSI pattern has only six shape/raster misses in a representative default run and does not cover every ANSI layout.
- **Idle:** no sustained rendering was observed during the deliberate quiet interval. Startup font/atlas work dominates these few frames rather than steady-state rendering.

There is no evidence that surface acquisition or the present API call dominates these workloads: default acquire averages 0.019–0.022 ms and present 0.094–0.133 ms. GPU-bound versus callback-delivery/scheduling-bound behavior cannot be resolved from these completion upper bounds. Overall sustained output is primarily parser/terminal-mutation bound, with measurable CPU renderer and driver-call costs. Frame cadence reflects app scheduling, mutation/drain budgets and the surface mode together.

Retained vectors and geometric instance-buffer growth already work: two buffer allocations per representative run, rather than repeated per-frame allocation. Identical scrolling content can generate hundreds of thousands of glyph instances while uploading only a few changed spans. Projection remains a full visible-grid traversal; shape-cache and atlas access still occur for each nonblank cell. No evidence justifies speculative row caching, new threading, present-mode changes, or a default-backend switch. **No renderer optimization was implemented.** No overdraw or GPU pipeline bottleneck was established.

## Pacing and responsiveness

Default medians were 8.6–10.3 ms between app-observed presents, with median per-run largest gaps 17.1–35.6 ms. Explicit DX12 was similar; Vulkan reduced ANSI intervals but did not consistently reduce throughput costs. These are successful-present call intervals, not displayed refresh intervals; neither dropped compositor frames nor scanout can be counted here.

Harness input dispatch ranged 5.35–8.74 ms across instrumented backend runs. Child acknowledgements ranged 25.10–892.99 ms and compete with output processing and child stdout locking; they are not an input-to-photon measurement. The historical baseline acknowledgement range was 5.77–93.52 ms, so the new series does not establish unchanged acknowledgement latency. The fairness code itself is unchanged. Manual acceptance on 2026-10-01 confirmed responsive typing, Ctrl+C under sustained output, tab/pane switching and resize. No numerical resize response times were measured; these observations are qualitative.

## Validation and native acceptance

Windows x86_64 release builds and repeated workloads completed. Required checks passed: cargo fmt --all -- --check; cargo check --workspace --all-targets; cargo test --workspace --all-targets; cargo test --workspace --doc; cargo clippy --workspace --all-targets -- -D warnings; git diff --check. Four existing native/platform tests are ignored; four doctests passed. Two new deterministic tests cover bounded sampling/cadence, collection and reset isolation, using fixed durations rather than timing assertions. Existing render/projection/buffer tests remain unchanged and passing. New polls/callbacks/logs are structurally gated by optional diagnostics; a release launch with diagnostics unset produced empty stdout/stderr during the attempted smoke check, but visual behavior was not observed.

Manual Windows x86_64 native acceptance **passed**, as reported by the user on 2026-10-01 for this renderer/GPU completion profiling task. Direct automated interaction had been blocked by computer-use product policy; the following evidence comes from the user’s native verification:

- Normal typing and cursor behavior were correct.
- Ctrl+C remained responsive under sustained output, and tab/pane switching remained responsive.
- ANSI-heavy output rendered correctly; selection/search highlights remained correct.
- No flicker, stale frames, device-lost errors or surface errors were observed.
- Resize remained responsive during repeated horizontal, vertical and corner operations, maximize and restore. No visible frame-pacing regression was observed during resize.

This completes the native acceptance step. The resize evidence is qualitative: no aggregate resize/reconfiguration capture or numerical response-time measurement was supplied. This acceptance does not add DPI-specific or Linux/macOS native evidence. Final code-diff review confirmed diagnostics and reporting only, plus the opt-in idle workload fixture; rendering algorithms, terminal semantics, event fairness, backend preference and present mode were not optimized or changed.

## Remaining limits and follow-up

Native acceptance is complete; quantitative resize profiling remains a possible follow-up rather than an acceptance blocker. Windows ARM64 is out of scope; Windows builds/tests do not establish Linux/macOS native behavior. Completion is sampled queue latency with delivery delay, not isolated GPU execution; timestamp-query experiments would need opt-in capability detection and delayed readback to separate it without stalling. Surface acquire/submit/present measurements are API wall times. No compositor/display latency was measured. Future CPU experiments should isolate repeated text/atlas lookup and long-line buffer comparison costs, use controlled alternating runs, and preserve unchanged-cell/selection/search/cursor/pane/DPI semantics before considering caching.
