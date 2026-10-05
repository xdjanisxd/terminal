# Minimal first-frame startup audit

Branch: `perf/minimal-first-frame`. Investigation date: 2026-10-05. No production work was deferred, no renderer backend changed, and no concurrency was added. Retained changes are opt-in monotonic diagnostic scopes, a release sampling harness, and this evidence report. Visual/input acceptance and independently cache-cold launches remain pending.

## Pipeline and ownership

Ordering follows the actual call path, not the names of the older milestones. In particular, **renderer-ready precedes shader/pipeline/atlas construction**: these resources are created lazily inside the first redraw. First present means return from the window-system presentation API, not observed compositor scanout. Window-shown means completion of visibility and focus requests.

1. OS loader/CRT initialization precedes Rust `main` and is outside the clock. `app::main` starts an `Instant`, collects arguments, handles the separate throughput mode, initializes opt-in diagnostics, parses CLI options, creates the winit event loop/proxy, then constructs `Application`. Help returns before event loop, config, window, fonts, GPU or shell work.
2. `Application::default` creates the default workspace, active pane, terminal's initial 80×24 primary/alternate screens, parser, frame/lifecycle state, empty interaction state and default config. It discovers config and Saved Workspace paths from environment/storage-directory rules. CLI overrides select the startup config/workspace file and optional project root.
3. `reload_config` checks/reads/parses/validates TOML (or defaults), constructs the workspace model exactly once, creates inactive pane runtimes, applies roots/session definitions/theme/font configuration, and retains last valid defaults on invalid config. `terminal-config` owns validation; `terminal-workspace` owns tab/layout/session metadata; `terminal-core` owns grid semantics. The app starts its **existing** config watcher before entering the event loop; its initial file fingerprint is on that existing thread.
4. On winit resume, the app builds branding/window attributes and creates the native window hidden. Winit supplies physical size and scale factor. Renderer construction resolves the configured font and calculates DPI-aware metrics before deriving grid dimensions.
5. `renderer::FontSystem` creates a fontdb database and enumerates installed fonts, resolves the primary family, sorts fallback face candidates, constructs shaping/scaling contexts and empty bounded caches, then reads primary-font metrics. Installed-font enumeration is shared by primary resolution and fallback candidates; it is not a separate optional fallback scan. Fallback character lookup, shaping and raster cache population happen on actual text use.
6. `renderer::initialize_gpu` creates a wgpu instance and compatible window surface, requests an adapter then device/queue. Windows tries DX12 first and preserves the existing all-backend fallback when no environment override applies. The selected adapter supplies the default surface configuration; renderer construction configures the surface. The app sets the configured render theme before any redraw.
7. The app sizes terminal grids for all pane runtimes from window size, pane geometry and font metrics; synchronizes fullscreen state/title; rearms damage and requests redraw. There are no PTYs yet to resize. Startup also invokes redraw directly because hidden windows need not receive native paint events.
8. `redraw_window` applies any pending size, drains pending output (none on ordinary startup), assembles the active tab's pane rectangles/focus/cursor, selection/search/scrollbar inputs and optional overlays, then calls `Renderer::redraw_panes`. Renderer projection produces the actual configured initial grid/UI data.
9. The renderer acquires a surface frame, lazily creates `DrawResources` (WGSL shader/layout, shared rectangle pipeline, glyph pipeline, atlas texture/view/sampler/bind group), generates instances, uploads/allocates nonempty instance buffers, encodes render passes, submits commands to the queue, and calls `present`. No separate selection/search/scrollbar pipelines or uniform-buffer setup exist: rectangles share one pipeline; projection is CPU-side. Multiple visible panes have their own encoding/submission calls; cost summaries aggregate repeated scopes per launch.
10. A successful outcome consumes the existing one-time presentation gate, emits `first-frame-presented`, then calls `set_visible(true)` and `focus_window`, emits `window-shown`, and only afterward starts shell/session preparation, integration/environment construction, PTY creation, spawn and worker setup for all panes. PowerShell startup commands remain gated by the existing prompt/readiness protocol; direct command sessions retain their own path. Later output invalidates frames normally.

Relevant implementation: [app lifecycle](../crates/app/src/main.rs), [renderer construction/rendering](../crates/renderer/src/lib.rs), [font setup](../crates/renderer/src/font.rs), [draw resources](../crates/renderer/src/gpu.rs), [diagnostic scopes](../crates/renderer/src/startup.rs). Architecture and ownership are unchanged; no ADR is needed.

## Classification

A = required before a correct first frame in the current design. B = can follow first frame without changing that frame, but not necessarily worth moving. C = initialize on first use (already lazy unless stated). D = negligible in this sample; changing it is unjustified. A/D indicates correctness requires the operation despite negligible cost.

| Operation | Class | Dependency / decision |
| --- | --- | --- |
| Process initialization / event loop / proxy | A | Required to own and create the native window; pre-main loader cost is unmeasured. |
| CLI parsing | A/D | Determines help, workspace and root paths before resources are created. |
| Config path discovery / typed defaults / config parsing | A/D | Determines first theme/font/layout, shell and session policy. Path/default cost is included in app setup. |
| Workspace model, active tab/pane metadata and layout | A/D | Required focus, title, initial geometry and session definitions. |
| Inactive tab terminal buffers and grid resizing | B | Not rendered initially; metadata must remain. Combined model/runtime creation is tiny here; no isolated meaningful saving proved, no deferral retained. First-use/runtime invariants would need tests before a move. |
| Initial terminal state / parser / active and visible pane grids | A/D | Correct cursor/background/geometry and subsequent output handling. |
| Window creation, branding, native size/scale/DPI, title/fullscreen state | A | Native surface and metrics depend on them. No placement/visibility changes. |
| Font request / primary family resolution / primary metrics | A | Correct configured face and DPI-aware grid before present. |
| System font database enumeration | A | Current primary-family lookup and fallback data depend on the same database. Splitting discovery requires separate investigation, not merely moving a call. |
| Fallback candidate ordering and empty font-cache/context setup | A/D | Existing shaping/fallback ownership is ready on any first-frame text; sorting ~1.4 ms is not a justified lifecycle change. |
| Actual fallback character lookup, glyph shaping/raster and cache population | C | Already performed only for non-space text that is rendered. No eager glyph warmup was found. |
| wgpu instance, adapter, device/queue, surface creation/configuration | A | Required for actual rendering/present; backend/fallback preserved. |
| Rectangle shader/layout/pipeline and instance buffers | A | Actual first background/cursor/pane borders and any selection/scrollbar need these shared resources. Buffers are already allocated only for nonempty data. |
| Glyph pipeline/atlas | A for text; conditional B for a proven blank frame | Blank normal launch does not draw glyphs, but supported visual-smoke initial content does. A conditional move must maintain nonblank startup and readiness, and avoid a first-text compilation stall. ~9 ms combined is a future experiment, not a proven safe saving; retained eager readiness. |
| Uniform buffers | — | No independent uniform initialization exists in this path. |
| Selection/search state and highlight resources | C/D | Empty optional state initially; interaction builds actual data. Shared rectangle pipeline cannot be deferred as a selection-only resource. |
| Scrollbar state/resources | A/D or C | Geometry derives from current history/viewport for each rendered pane; empty history has no thumb. Drag/hover state is empty. No independent pipeline. |
| Tab/pane UI and overlays | A/D | Initial focus border/title/layout are real. Pickers/palette/search/rename state are lazy. No persistent graphical tab strip is built. |
| Saved Workspace file metadata | C | Only its path is discovered on startup. The saved-store file/picker loads on explicit action. CLI --workspace loads a startup TOML recipe, not the saved-store file. Saved templates' commands/CWDs are used on reopening. |
| Shell configuration/session definitions | A/D | Parse with config and workspace; only preparation/spawn need await show. |
| Shell discovery, integration preparation, environment and PTY/session spawn | B, already after show | Existing ordering stays unchanged; no startup command/integration dependency was moved. |
| Existing config-watcher thread launch | B | Could start after show but would alter observation timing; combined launch/event dispatch median 0.184 ms, range 0.170–9.777 ms, not isolated causation. No new thread or move. |
| Diagnostics initialization/bookkeeping | D | Opt-in; normal startup emits no timing. Disabled scopes read no clock and allocate no sink. Existing renderer/throughput diagnostics remain separate. |
| Redraw scheduling / first projection / encoding / submit / present | A | Actual configured frame must be submitted, not an empty placeholder. |
| Native show/focus | After present, required for visibility | Must remain after successful frame; cannot improve visible startup merely by excluding this cost. |

## Measurement method and evidence

Native Windows x86_64 release, Microsoft Windows 10.0.26300, X64 process/OS. Same local configuration: JetBrainsMono Nerd Font Mono, logical size 16, black background, one local-shell pane. No WGPU_BACKEND or TERMINAL_CONFIG override was present; normal Windows DX12-first path was retained. Seven sequential launches of the unchanged release baseline, then seven of the diagnostic build, with builds/validation idle for those samples. No launch was discarded. Each launch reached first shell text rendering. These are fresh-process, **not independently cache-cold** samples: OS/font/driver/shader caches were not reset. Batches were not interleaved. Consequently there is no causal speedup or regression claim from their difference.

All timings use monotonic Instant CPU-side scopes. Nested scopes are inclusive; sums of medians are not totals. File logging overhead is included in the enclosing intervals. API duration is not GPU completion or visible scanout. The start clock now includes argument collection; the application-started marker follows diagnostics initialization, so the requested marker-to-marker totals exclude that small initialization interval. The prior baseline clock excluded argument collection. Pre-main process loading is outside both.

[Per-launch milestone and scope data](measurements/minimal-first-frame.csv) preserves the samples in milliseconds. Raw logs remain local ignored artifacts: target/minimal-first-frame-baseline/launch-{1..7}.log and target/minimal-first-frame-instrumented/launch-{1..7}.log.

| Requested interval (ms) | Baseline median (min–max) | Diagnostics-only median (min–max) |
| --- | ---: | ---: |
| application-started → window-created | 29.256 (19.346–50.450) | 24.280 (20.234–40.258) |
| window-created → fonts-ready | 32.122 (28.916–68.196) | 38.546 (29.674–44.384) |
| fonts-ready → GPU-ready | 91.694 (81.299–197.208) | 88.101 (82.441–116.277) |
| GPU-ready → renderer-ready | 6.025 (5.149–9.333) | 5.763 (4.748–7.449) |
| renderer-ready → first-frame-presented | 30.314 (16.332–45.137) | 17.961 (17.397–30.435) |
| application-started → first-frame-presented | **179.254 (163.009–369.196)** | **183.501 (167.145–198.846)** |
| first-frame-presented → window-shown | 39.222 (19.183–80.420) | 39.869 (28.492–51.513) |
| window-shown → all PTY spawns complete | 425.656 (397.851–512.262) | 437.600 (414.386–582.661) |
| application-started → prompt-ready | 1246.471 (1070.121–1614.825) | 1407.415 (1249.117–1612.403) |

There is **no production before/after optimization**. Differences above are two diagnostic sampling batches with unchanged production ordering. Prompt timing is informational and does not gate visibility.

| Individual CPU scope (ms), diagnostic build | Median | Min–max |
| --- | ---: | ---: |
| CLI parse | 0.006 | 0.004–0.009 |
| Event-loop creation | 4.845 | 3.615–8.877 |
| Default state | 0.181 | 0.158–0.347 |
| App state + paths (includes default state) | 0.233 | 0.208–0.406 |
| Config read/parse/validate | 0.467 | 0.384–0.604 |
| Workspace model + runtimes | 0.013 | 0.009–0.022 |
| Window + branding | 18.678 | 14.147–22.437 |
| System font enumeration | 30.896 | 27.911–36.461 |
| Primary family resolution | 0.013 | 0.010–0.014 |
| Fallback candidate ordering | 1.426 | 1.331–1.792 |
| Empty font contexts/caches | 0.004 | 0.004–0.011 |
| Font metrics | 0.097 | 0.072–0.128 |
| wgpu instance | 7.238 | 6.184–8.531 |
| Surface creation | 0.032 | 0.030–0.053 |
| Adapter request | 36.475 | 32.985–60.960 |
| Device/queue request | 46.581 | 40.997–48.249 |
| Surface configure | 5.693 | 4.692–7.373 |
| Initial grid resize | 0.130 | 0.127–0.182 |
| Initial UI/pane input assembly | 0.004 | 0.002–0.005 |
| Initial projection | 0.034 | 0.029–0.058 |
| Surface acquisition | 0.066 | 0.053–0.098 |
| Shader and layout | 0.393 | 0.355–0.527 |
| Rectangle pipeline | 6.063 | 5.640–8.194 |
| Glyph pipeline | 7.034 | 6.221–9.041 |
| Glyph atlas | 1.943 | 1.668–2.795 |
| Instance generation | 0.001 | 0.001–0.002 |
| Buffer creation/upload | 0.288 | 0.260–0.511 |
| Render encoding + command finish | 0.737 | 0.603–1.160 |
| Queue submit | 0.507 | 0.369–1.691 |
| First present API | 0.396 | 0.316–0.664 |

## Decision and next investigation

No meaningful **proven safe** defer candidate is retained. Config/state/layout are tiny. Search/pickers/fallback lookup/cache population are already lazy. Shell preparation already follows show. Shared rectangle resources are needed for the initial real frame. Moving glyph pipeline/atlas construction could shift a measurable stall into text rendering or merely delay native show if done between present and show. System-font discovery cannot simply move after present because the configured family and grid metrics depend on it.

GPU setup remains the dominant pre-present stage (~88 ms), especially device and adapter requests. Best next investigation: measure repeatable DX12 adapter/device creation with controlled cold/warm driver-cache conditions and unchanged backend selection; determine what time is driver work versus application requests before proposing a change. Font database discovery (~31 ms) is the next bounded candidate: investigate preserving installed-family/fallback correctness with narrower discovery or validated reuse, without assuming a cache is safe.

No new post-present work or hitch was introduced by a deferral, because none was implemented. **Absence of a visible hitch is not yet manually verified.** Existing synchronous shell spawning blocks the event loop for ~0.4–0.6 s after show; this remains a limitation of the existing lifecycle, not an improvement hidden by this metric. First visible frame requires both present and show, and neither marker proves visible pixels.

The planned Alacritty comparison can proceed as a clearly labeled exploratory fresh-process comparison once the same visual acceptance and measurement definitions are applied to both programs. A defensible cold-start comparison should wait for independently cold samples and manual acceptance; do not report this investigation as a cold-start optimization win.

## Reproduce and complete acceptance

Build with `cargo build --release -p terminal-app`. Run `./scripts/measure-first-frame.ps1 -Runs 7 -OutputDirectory target/first-frame-review`. The harness restores diagnostic environment variables, refuses to overwrite launch logs, closes only its launched processes, waits for shell text, and verifies present → show → PTY-start → spawn-complete ordering. Its polling waits are measurement orchestration, not production delays. `-Workspace` selects a TOML recipe. To obtain independent cache-cold evidence, collect launches after independent Windows restarts (record host/load/cache conditions); the sequential harness alone is insufficient. No system cache manipulation or reboot was performed by this task.

Release smoke also reached first present/show/output in three launches with a nondefault #102030 background, split active tab focused on its second pane, and an inactive tab. This verifies lifecycle markers and session startup, **not pixels or focus**. Saved Workspace reopening remains covered by existing deterministic tests and requires the manual check below.

Native automation is restricted by the computer-use plugin guidance: “Do not automate terminal applications.” Perform this checklist with the release binary, initially with startup diagnostics disabled; enable timing only in a separate measurement run:

- [ ] Fresh launch: no white flash; the first visible client frame has the configured background. Test both the regular config and a nonblack background.
- [ ] Configured font is correct on the first text; no font replacement or late glyph/UI popping. Also run TERMINAL_RENDERER_VISUAL_SMOKE=1 to exercise actual initial glyphs, then unset it.
- [ ] DPI, initial grid, window size and placement match baseline. Launch on each relevant DPI display and move between displays.
- [ ] Type immediately on show and after prompt; input is retained and responsive. Observe the interval after show for any new hitch.
- [ ] Focus, taskbar, minimize/restore behavior match baseline; resize immediately and after prompt.
- [ ] Launch the split/inactive-tab recipe; focus border, pane layout and tab title are correct immediately; switch tabs/panes and type.
- [ ] Open an existing Saved Workspace template with split/tab layout, saved CWDs and a benign saved startup command. Verify layout/focus, OSC 7 CWD update, PowerShell hook/custom prompt, command exactly once and shell available afterward. Do not overwrite the original template.
- [ ] Verify explicit configured shell and direct-command startup; confirm --help creates no window and normal startup emits no diagnostic output.
- [ ] Collect several independent cache-cold launches before/after the retained diagnostics build or record why unavailable. Report medians/ranges separately from these fresh-process samples.

Validation passed on Windows x86_64: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets` (650 passed, four existing ignored), `cargo test --workspace --doc` (four passed), and `cargo clippy --workspace --all-targets -- -D warnings`. Release build, `--help`, and diagnostics-disabled silent startup passed. Two deterministic tests cover disabled diagnostic scopes and closing the scope measurement window; no timing thresholds or lifecycle changes were added. Final `git diff --check` is recorded in the handoff. The Goal is blocked on manual acceptance and independent cache-cold evidence; no commit or push was made.
