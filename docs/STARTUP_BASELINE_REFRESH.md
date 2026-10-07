# Windows x86_64 startup baseline refresh

Measured 2026-10-07 on `perf/startup-baseline-refresh`, revision `cb11a16144792a130c7d4d4a9c479e152aedbf02`. This is the consolidated current release baseline. No production code, dependency, optimization, user configuration, or backend policy changed. No commit or push.

## Collection and provenance

- `cargo build --release -p terminal-app -q`; Rust 1.98.1, x86_64-pc-windows-msvc. Binary SHA-256: `795034AF943127F862F628A08E5B56025943CC434948AAEB88EC3C287AE6B2EB`.
- Windows 11 Pro build 26300, Intel i5-1250P, Iris Xe, driver 32.0.101.7088. Every measured trace selected DX12 / Intel with descriptor limit 16,384; no backend override or temporary dependency diagnostic overlay.
- Fifteen fresh-process runs per scenario; two unreported warmups per scenario. Scenario order rotates each cycle. Caches are warm/uncontrolled: no eviction, reboot, or cold-start acceptance. Collection ran without concurrent build or validation jobs.
- Existing startup diagnostics enabled, renderer diagnostics disabled. Timestamps start at Rust main entry, excluding OS executable loading. API presentation markers mean submission to the window system, not compositor scanout or proof of visible pixels.
- Isolated complete copies of the active config preserve JetBrainsMono Nerd Font Mono size 16, black background, one LocalShell pane, bindings and workspace settings. Working/project directory is this repository. The actual active config selects cmd; “configured-pwsh” means the same configuration with only shell selection replaced by profile-enabled pwsh, not a claim that the active file selected pwsh.
- cmd.exe has no arguments (AutoRun/Clink enabled). PowerShell 7.6.6 uses the installed executable recorded in environment.json: no-profile has only `-NoProfile`; configured-pwsh has no user arguments. Application policy appends its normal `-NoExit -Command <powershell_osc7.ps1>` integration to both; no `-NoLogo` was added. Profiles run normally except in no-profile.
- The harness waits for both prompt-ready and first-shell-output-rendered, requires one spawned shell and verifies first-present → show → PTY ordering. It inventories descendants only after the measured interval, closes the app and verifies termination. All 51 launches exited normally, code 0, without forced cleanup; each recorded two descendants. Executable and source config hashes were unchanged afterward.
- An initial pilot's no-profile shell exited without a prompt; it was rejected rather than included. A repeat pilot and the final independent three-scenario harness smoke passed. No automatic retry or outlier exclusion occurred in the main 45-run batch. The pilot failure's cause was not established.

Reproduce with a freshly built release binary and a fresh output directory:

```powershell
cargo build --release -p terminal-app -q
./scripts/measure-startup-baseline.ps1 -Runs 15 -Warmups 2 -OutputDirectory target/perf/startup-baseline-refresh-new
```

[Raw milestones](perf/startup-baseline-refresh/milestones.csv), [cost scopes](perf/startup-baseline-refresh/costs.csv), [per-run intervals](perf/startup-baseline-refresh/intervals.csv), [environment](perf/startup-baseline-refresh/environment.json), [cleanup](perf/startup-baseline-refresh/cleanup.csv), [all measured/warmup traces](perf/startup-baseline-refresh/traces.txt), and [individual trace hashes](perf/startup-baseline-refresh/trace-manifest.json) are retained. Isolated scenario TOMLs are retained alongside them. Exploratory/smoke output stays under target and is excluded from this evidence set.

## Current baseline

Every cell is **median (minimum–maximum) milliseconds**, n=15 for its shell. These are elapsed milestones from main entry, not durations. “Application started” is the first diagnostic observation, not OS process creation.

| Milestone | cmd.exe | pwsh -NoProfile | configured pwsh |
| --- | ---: | ---: | ---: |
| application-started | 0.333 (0.275–0.545) | 0.297 (0.262–0.490) | 0.327 (0.263–0.609) |
| window-created | 20.711 (17.539–23.183) | 20.589 (17.365–27.215) | 20.292 (18.832–24.848) |
| fonts-started | 20.894 (17.692–23.563) | 20.603 (17.377–27.228) | 20.304 (18.845–24.869) |
| fonts-ready | 48.626 (45.854–55.383) | 48.204 (44.563–59.788) | 47.863 (46.204–57.581) |
| gpu-started | 48.637 (45.864–55.396) | 48.214 (44.588–59.800) | 47.873 (46.216–57.593) |
| gpu-ready | 113.669 (108.143–127.941) | 113.761 (107.041–129.919) | 112.773 (109.829–127.456) |
| renderer-ready | 118.671 (112.711–133.344) | 118.522 (111.349–135.811) | 117.917 (114.227–136.452) |
| first-frame-presented | 134.071 (126.919–148.584) | 133.377 (126.835–150.953) | 132.524 (129.319–154.644) |
| window-shown | 157.050 (146.234–187.131) | 155.226 (147.812–184.407) | 155.381 (150.170–192.300) |
| pty-started | 157.103 (146.320–187.193) | 155.287 (147.867–184.464) | 155.442 (150.254–192.363) |
| child-spawn-requested | 165.076 (154.194–196.948) | 163.216 (155.740–192.386) | 163.618 (158.037–201.817) |
| child-spawned | 173.489 (161.811–205.549) | 521.396 (507.023–572.657) | 524.365 (505.965–579.516) |
| first-pty-bytes | 187.653 (176.380–227.822) | 531.965 (517.830–583.726) | 535.633 (518.641–596.965) |
| prompt-ready | 301.906 (291.942–365.799) | 970.571 (946.866–1028.693) | 1047.238 (1016.847–1120.615) |
| first-shell-text-processed | 205.479 (195.123–246.784) | 985.454 (961.508–1044.993) | 1056.820 (1032.353–1140.742) |
| first-shell-output-rendered | 238.316 (225.460–283.651) | 1005.384 (981.913–1063.957) | 1068.370 (1046.153–1153.283) |
| pty-spawn-complete | 173.597 (161.910–205.716) | 521.498 (507.137–572.775) | 524.481 (506.074–579.633) |

First PTY bytes can be ConPTY control sequences. First-shell-text-processed means the first non-whitespace screen content, which may be a banner. First-shell-output-rendered is the first presentation after such content. **It does not guarantee that the prompt itself has been presented.** cmd renders initial output at 238.316 ms but reaches its prompt marker at 301.906 ms. PowerShell's marker is emitted by the prompt function before its returned prompt text is processed. The harness continues to wait for both milestones even if rendered output occurs first.

## Cost ownership and remaining contributors

Compute each duration by subtracting two timestamps from the **same run**, then summarize those durations. Never subtract independently summarized medians. Ranges retain every main sample. Nested cost scopes are inclusive; neither their medians nor the rows below should all be added together.

| Interval | cmd.exe | pwsh -NoProfile | configured pwsh |
| --- | ---: | ---: | ---: |
| Terminal before fonts | 20.615 (17.398–23.196) | 20.338 (17.086–26.848) | 19.978 (18.564–24.520) |
| CPU fonts | 28.162 (26.289–32.516) | 28.835 (26.038–36.113) | 27.867 (26.861–32.712) |
| GPU/wgpu init | 65.439 (60.931–72.545) | 64.802 (61.933–74.232) | 65.636 (62.037–79.717) |
| renderer completion | 4.818 (4.315–5.403) | 4.514 (4.173–6.586) | 4.765 (4.149–8.996) |
| renderer-ready to first present | 15.240 (14.208–16.753) | 15.060 (13.901–19.634) | 15.304 (13.674–18.192) |
| first present to show | 22.275 (19.315–38.547) | 22.655 (18.420–33.454) | 22.865 (19.950–37.656) |
| PTY preparation | 7.973 (7.361–9.755) | 7.873 (7.433–9.068) | 8.265 (7.581–9.454) |
| process spawn | 7.711 (6.791–8.601) | 355.628 (346.596–382.852) | 358.984 (345.803–395.680) |
| spawn to first bytes | 14.589 (12.035–22.273) | 11.069 (10.271–13.712) | 11.367 (10.068–19.838) |
| shell/integration to prompt | 133.888 (120.493–160.250) | 443.380 (427.095–462.546) | 523.022 (501.209–563.748) |
| bytes to prompt | 121.143 (107.364–137.977) | 432.698 (415.726–451.166) | 512.171 (490.291–553.680) |
| spawn to shell-text | 35.788 (21.634–41.254) | 463.882 (437.614–482.291) | 535.175 (520.213–574.031) |
| text to render | 31.352 (29.026–39.043) | 20.118 (18.336–22.480) | 12.287 (10.430–19.817) |

- **Terminal-owned orchestration/CPU font path:** approximately 20 ms from the application marker to font work and 28 ms preparing CPU fonts. Config, event loop, workspace model and window creation are in the first interval; font discovery dominates the second. These scopes include synchronous dependency/OS work, not pure application CPU instruction time.
- **Renderer/wgpu:** approximately 65 ms GPU initialization plus 4–5 ms renderer completion and 15 ms to first presentation. The renderer-ready-to-present interval includes lazy pipeline/atlas work. A further 22–23 ms elapses from first presentation to the window-shown marker; attribution to compositor pixels is unavailable. GPU API times do not measure GPU execution completion.
- **PTY/process:** about 8 ms preparation, then about 8 ms cmd spawn or 356–359 ms PowerShell spawn. This is the existing project spawn boundary; the [Windows spawn investigation](WINDOWS_POWERSHELL_SPAWN.md) found CreateProcessW with ConPTY dominated it. The current batch has no dependency overlay and does not re-prove internal Windows attribution. Some child initialization overlaps this synchronous spawn interval; do not label it exclusively profile cost or Terminal CPU overhead.
- **Shell/profile:** child-spawned to prompt is 133.888 ms cmd, 443.380 ms no-profile, 523.022 ms configured pwsh. Profiles, PowerShell initialization, PSReadLine, prompt work, integration and observation/dispatch are mixed in this interval. The within-cycle difference of configured versus no-profile post-spawn-to-prompt durations is 79.743 ms median (45.173–123.905). This is an observed profile-enabled scenario difference, not an isolated profiler attribution. NoProfile still carries most of the PowerShell delay.
- **Output processing/presentation:** first text to rendered output is about 31 ms cmd, 20 ms no-profile and 12 ms configured pwsh. This includes event scheduling/dispatch and rendering; it is not a measured parser-only cost. Received integration begin→end is 16.675 ms (14.372–20.129) no-profile and 8.411 ms (7.507–17.190) configured; it is nested inside shell startup and includes observation effects.

The biggest contributors to PowerShell readiness remain process spawn and post-spawn shell initialization. For the pre-shell first-frame path, GPU initialization is largest, then font discovery and app/window setup. Fairness under sustained large output and shutdown reliability are not startup speed claims; this batch verifies cleanup and marker collection only.

Selected existing CPU/API scopes, same units and sample count:

| Scope | cmd.exe | pwsh -NoProfile | configured pwsh |
| --- | ---: | ---: | ---: |
| system-font-discovery | 26.747 (24.979–30.931) | 27.575 (24.778–34.843) | 26.647 (25.502–31.175) |
| font-family-preparation | 28.036 (26.189–32.276) | 28.741 (25.945–36.026) | 27.779 (26.755–32.550) |
| wgpu-instance | 6.045 (5.462–8.017) | 6.351 (5.471–7.435) | 6.257 (5.375–7.515) |
| adapter-request-call | 33.286 (29.887–36.206) | 33.092 (30.594–38.994) | 32.842 (31.074–41.586) |
| device-request-call | 24.976 (23.385–28.372) | 25.295 (24.215–27.682) | 25.538 (23.749–31.409) |
| surface-configure | 4.680 (4.189–5.247) | 4.378 (4.024–6.411) | 4.629 (4.026–8.767) |
| rectangle-pipeline | 5.116 (4.649–5.701) | 5.150 (4.581–5.834) | 5.088 (4.772–6.622) |
| glyph-pipeline | 5.810 (5.272–6.711) | 5.724 (4.982–7.440) | 5.869 (4.872–8.075) |
| glyph-atlas | 1.595 (1.347–2.024) | 1.580 (1.406–2.423) | 1.688 (1.335–2.192) |

Font-family-preparation contains discovery; GPU-initialize contains instance/adapter/device work; pipeline creation happens later. Do not sum nested rows.

## Historical comparisons

### Most recent stock renderer cohort (2026-10-06)

[Wgpu adapter investigation](WGPU_ADAPTER_REQUEST.md), final-stock 15-run cohort, is the latest trustworthy normal-dependency renderer reference on the same hardware/driver/compiler. Its workspace used a direct cmd Command session with `/d /k` and built-in font/config defaults. The new cohort uses the active font/settings and integrated LocalShell, so it is an observational comparison, not a controlled A/B optimization experiment.

| Metric, median ms | Previous final-stock | Current cmd | Observed change |
| --- | ---: | ---: | ---: |
| First-frame-presented | 140.454 (134.351–181.609) | 134.071 (126.919–148.584) | −6.383 ms (−4.5%) |
| GPU-initialize cost | 68.564 (64.805–100.622) | 65.421 (60.914–72.526) | −3.143 ms |
| Adapter-request-call | 34.360 (32.583–66.960) | 33.286 (29.887–36.206) | −1.074 ms |
| Device-request-call | 25.721 (24.332–31.351) | 24.976 (23.385–28.372) | −0.745 ms |
| System-font-discovery | 27.900 (25.647–32.180) | 26.747 (24.979–30.931) | −1.153 ms |
| Font-family-preparation | 29.207 (26.795–33.599) | 28.036 (26.189–32.276) | −1.171 ms |

These ranges overlap. The lower current medians and narrower observed first-frame range are useful baseline observations, not evidence that this task optimized anything. The prior initial-stock font outlier was retained by its own report; it is not the final-stock reference used here. Neither cohort establishes tail guarantees.

### Improvements established by earlier controlled work

[DX12 descriptor-budget investigation](WGPU_DEVICE_INIT.md) used 30 alternating before/after launches per variant: first present 213.571→181.545 ms (−32.026 ms), device-request-call 51.669→31.182 ms (−20.487 ms). That is the trustworthy earlier optimization result; its descriptor limit remains 16,384 in all current traces. Current device-call median is 24.976 ms for cmd, but the further difference is across cohorts and cannot be attributed to a new change. That historical experiment did **not** materially improve p90 first frame (329.824→332.349 ms).

[First-frame ordering work](STARTUP.md) moved shell startup after first present/show and reduced the old first-frame result from about 1481.6 to 230.7 ms while renderer-ready stayed about 210/208 ms. Current traces confirm the ordering for all runs. Those old milestones were a different environment/cohort; the sequencing benefit is real, while they are not a matched numerical reference for this batch. Font profiling and adapter-request profiling established attribution, not retained production speedups.

### Shell baselines: stable costs and a slower spawn regime

The [2026-09-30 production-final spawn cohort](WINDOWS_POWERSHELL_SPAWN.md) had median native spawn cmd 6.515 ms, no-profile 337.973 ms and configured pwsh 357.051 ms. Current values 7.711/355.628/358.984 ms are broadly the same expensive PowerShell regime; there is no material PowerShell spawn improvement versus that investigation.

The **more recent** [2026-10-05 Alacritty comparison](ALACRITTY_STARTUP_COMPARISON.md) retained seven-run Terminal hook cohorts using `-NoLogo -NoExit -EncodedCommand <shared hook/probe>`, direct Command sessions and different timing instrumentation. They are trustworthy observations but not identical launches to the current normal LocalShell `-Command` integration.

| Metric, median ms | Previous no-profile + hook | Current no-profile | Previous normal + hook | Current configured pwsh |
| --- | ---: | ---: | ---: | ---: |
| Spawn-requested→spawned | 150.321 | 355.628 | 150.834 | 358.984 |
| Spawned→prompt-ready | 449.323 | 443.380 | 511.980 | 523.022 |
| Main→prompt-ready | 853.774 | 970.571 | 848.444 | 1047.238 |
| Main→first shell output rendered | 879.197 | 1005.384 | 877.568 | 1068.370 |

Post-spawn shell time did not materially improve; native spawn is roughly 205–208 ms slower than that cohort, despite faster first frame. Prompt/output elapsed totals are correspondingly slower. This report does not hide that difference or claim its cause: command encoding, normal integration, measurement conditions and Windows process/ConPTY behavior differ. The next matched comparison must measure the spawn regime anew. Previous bare PowerShell cohorts had no prompt marker; do not equate their banner rendering with prompt readiness. Previous configured cmd first output was 246.524 ms versus current 238.316 ms, again overlapping cohort ranges; its prompt marker was unavailable.

## Exact endpoints for the next Alacritty comparison

Use this report as Terminal's internal attribution baseline, and perform a **new matched warm-process comparison**. Do not reuse main-entry timestamps as external process-start measurements or subtract an old Alacritty median from the new Terminal median.

1. Preserve three scenarios: cmd with no arguments/AutoRun enabled; pwsh no-profile; pwsh normal profiles. Use the same resolved binaries, profile files, cwd, inherited environment, font face/size in equivalent physical units, client dimensions, DPI, theme/padding and window policy in both apps. Hash configs/binaries and record backend/driver. Keep native configured LocalShell results as this baseline; separately label the matched instrumentation scenario when launch arguments change.
2. Use at least 15 alternating app pairs per scenario after two excluded warmups each, rotating scenario order, with fresh log locations and normal cleanup. No concurrent builds, stress tests, cache clearing or reboot acceptance. Retain all valid runs/ranges, report rejected runs and actual observer polling gaps.
3. **Primary external window endpoint:** monotonic timestamp immediately before Process.Start → first observer detection of the app-owned HWND with matching client dimensions and IsWindowVisible=true. Report median/range and paired Terminal-minus-Alacritty deltas. Record Process.Start return separately. This endpoint is a visibility flag observation; neither WaitForInputIdle nor a show log proves correct pixels.
4. **Primary external shell endpoint:** the same launcher timestamp → a shared, identically installed shell prompt-function readiness probe in both apps. For pwsh use identical UTF-16 encoded `-NoExit -EncodedCommand` instrumentation, with only `-NoProfile` differing between shell scenarios; capture an absolute UTC prompt timestamp, calibrate launcher wall/monotonic mapping, and preserve each normal prompt function. Use the same probe placement/transport and overhead in both apps. For cmd, install the same post-initialization prompt probe in both and verify AutoRun/Clink preserves it; an identical OSC marker alone is insufficient unless both apps expose its receive time to the same observer. Until that works, cmd prompt readiness is unavailable cross-app rather than inferred from a banner. Label prompt-function readiness separately from visual prompt presentation.
5. **Visible content endpoint, if needed for user-perceived readiness:** launcher timestamp → externally observed first nonblank shell content and, separately, full prompt pixels, using the same timestamped screen/video observer and detection rule for both apps. Keep detection uncertainty. Existing logs alone cannot provide these cross-app pixel endpoints; do not substitute first PTY bytes or a readiness hook.
6. **Terminal internal attribution to collect again:** main→first-frame-presented, main→window-shown, fonts-started→fonts-ready, gpu-started→gpu-ready, renderer-ready→first-frame-presented, first-frame-presented→window-shown, pty-started→spawn-requested, spawn-requested→spawned, spawned→first-PTY-bytes, spawned→prompt-ready, main→prompt-ready, main→first-shell-output-rendered, text-processed→output-rendered; include adapter/device call scopes. Alacritty logger origin differs from Terminal main; native cross-app absolute timestamps are not directly comparable. Compare native intervals only where both source boundaries are verified equivalent.

Current Terminal reference medians for those internal endpoints are the tables above: first present **134.071/133.377/132.524 ms**, show **157.050/155.226/155.381 ms**, prompt **301.906/970.571/1047.238 ms**, first shell render **238.316/1005.384/1068.370 ms** (cmd/no-profile/configured). No new Alacritty launch or speed ranking was performed in this task.

## Tooling and validation

The consolidated measurement-only harness resolves inconsistencies between existing first-frame and PTY scripts: collect all markers in one run, preserve first occurrence, require prompt plus rendered output for every shell, rotate order, distinguish excluded warmups, restrict one shell, and retain descendant cleanup results/environment/configuration. Cost samples stop at first present so later rendering cannot contaminate first-frame scope summaries. Missing markers or forced cleanup fail collection; no production instrumentation needed repair.

Release build passed. Harness PowerShell parse and all-scenario smoke passed. Full completion checks passed: cargo fmt --all -- --check; cargo check --workspace --all-targets; cargo test --workspace --all-targets; cargo test --workspace --doc; cargo clippy --workspace --all-targets -- -D warnings; git diff --check. Validation logs remain under target/perf/startup-baseline-refresh/validation. Runtime, Cargo files, CURRENT_STATE, ROADMAP and architecture decisions are unchanged. The unrelated function-key active task remains preserved.
