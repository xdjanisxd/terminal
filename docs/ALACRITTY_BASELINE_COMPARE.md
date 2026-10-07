# Current-baseline Terminal versus Alacritty startup

Measured 2026-10-07 on Windows x86_64, branch `perf/alacritty-baseline-compare`, repository revision `87e0325959c8babd0b17d4b1971b3773d8d5745e`.

The current startup gap does **not justify an OpenGL startup prototype**. In 15 warm alternating pairs per shell, Terminal's visibility flag was typically 17–22 ms later. cmd readiness was comparable; the matched instrumented PowerShell readiness gap was 68–83 ms. A substantial part of the PowerShell difference occurs after shell process creation and is not established as renderer cost. Investigate the existing startup/spawn/integration boundary before changing backend strategy. No optimization, cold/reboot measurement, commit or push was performed.

## Authoritative Terminal reference

[STARTUP_BASELINE_REFRESH.md](STARTUP_BASELINE_REFRESH.md) remains authoritative for normal LocalShell startup. Its medians from **Rust main entry**, n=15, are:

| Endpoint | cmd.exe | pwsh -NoProfile | configured pwsh |
| --- | ---: | ---: | ---: |
| First API frame presentation | 134.071 | 133.377 | 132.524 |
| Window-shown marker | 157.050 | 155.226 | 155.381 |
| Prompt-ready OSC marker | 301.906 | 970.571 | 1047.238 |
| First shell content rendered | 238.316 | 1005.384 | 1068.370 |

The exact authoritative Terminal release executable was reused, SHA-256 `795034AF943127F862F628A08E5B56025943CC434948AAEB88EC3C287AE6B2EB`. It was not rebuilt. This comparison's supplemental external/encoded measurements must not replace those normal-launch values or be described as an optimization.

## Controlled collection

- Same Windows 11 Pro build 26300, Intel i5-1250P, Iris Xe driver 32.0.101.7088. Terminal selected DX12; Alacritty 0.17.0 (94e7c88) selected WGL/OpenGL 3.3. Alacritty executable SHA-256: `5AD70DDC5C2A2BFE84084A2C4C73558E9360914F035A902D7A69277BE7249F1F`.
- **Warm/repeated fresh-process runs only**, unchanged/uncontrolled caches. Two excluded warmup pairs per shell, then 15 measured pairs; scenario order rotates, app order alternates. No concurrent build or validation during either measured cohort. Every measured launch is retained; no retries or outlier exclusion.
- Same resolved cmd and PowerShell 7.6.6 binaries, cwd, paired TEMP/TMP, inherited environment, configured cmd AutoRun/Clink and PowerShell profiles. Temporary CLINK_PATH supplements configured scripts. All 51 main/warmup pairs had byte-identical actual shell command lines. Source binary/config/hook and profile hashes remained unchanged.
- Isolated Terminal copies preserve the entire active config and use one explicit Command pane for the shared probe. Alacritty uses an isolated config. Both have JetBrainsMono Nerd Font Mono Regular, 16 logical px / Alacritty 12 pt, black background, zero padding, client 800×600 at DPI 96. Window decoration, text grid and renderer behavior are not identical.
- cmd.exe has no arguments and preserves AutoRun. The probe timestamps its first Clink onbeginedit callback via file last-write UTC; that means beginning interactive input, not the native Terminal OSC marker or visible prompt pixels.
- PowerShell uses identical `-NoProfile -NoExit -EncodedCommand` or `-NoExit -EncodedCommand` arguments. The encoded script contains the current normal integration hook plus a wrapper which invokes the original integrated prompt and timestamps before returning prompt text. No -NoLogo. Profiles are enabled in normal-hook. These are instrumented configured-shell launches; the authoritative baseline instead uses LocalShell policy with `-Command`. The argument form materially affects spawn timing.
- Launcher stopwatch starts immediately before Process.Start. OS process CreationDate/StartTime and shared probe UTC timestamps are mapped to launcher UTC; measured clock drift was at most 0.050 ms. Window checks request 2 ms sleeps but Windows scheduling produced polling gaps up to 33.417 ms. The first observed visible flag is an upper-bound observation with polling uncertainty, not a pixel timestamp.
- Native Terminal startup diagnostics and Alacritty -vvv --print-events are enabled in the main cohort. Process inventory occurs after the measured endpoints. All 102 launches exited code 0, no forced termination or surviving recorded direct shell/conhost children. All dimensions/DPI passed. An initial smoke's post-hash check had a path-normalization error; it was fixed and a separate preflight passed. Smoke data is excluded.

## Equivalent external observations

Milliseconds from the shared launcher epoch; visible/readiness cells are **median (minimum–maximum)**. Every row has n=15. Full ranges for all metrics are retained in summary.csv. OS shell creation is a process-creation timestamp, not completion of CreateProcess/ConPTY startup.

| Shell / app | OS app creation | Process.Start return | HWND visible flag | OS shell creation | Shared readiness |
| --- | ---: | ---: | ---: | ---: | ---: |
| cmd / terminal | 1.3 | 4.2 | 153.6 (145.0–188.7) | 179.0 | 314.6 (296.7–354.6) |
| cmd / alacritty | 2.0 | 5.9 | 137.3 (117.9–167.2) | 154.6 | 326.4 (295.0–376.5) |
| no-profile-hook / terminal | 1.4 | 5.1 | 162.4 (148.9–188.5) | 214.5 | 748.3 (725.6–831.0) |
| no-profile-hook / alacritty | 2.1 | 6.6 | 137.3 (115.0–218.0) | 186.1 | 668.8 (587.5–776.0) |
| normal-hook / terminal | 1.7 | 4.3 | 163.3 (150.7–174.4) | 213.3 | 829.7 (796.8–862.8) |
| normal-hook / alacritty | 2.2 | 6.9 | 143.8 (120.1–167.6) | 191.0 | 751.7 (717.8–904.5) |

Paired Terminal-minus-Alacritty differences, **median (minimum–maximum)**; positive means Terminal later. These are medians of same-cycle differences, not differences between independently summarized medians. Interval rows are computed within each run first; their medians must not be added to explain another median.

| Shell | Visibility | OS shell creation | Shared readiness | Shell creation → readiness interval |
| --- | ---: | ---: | ---: | ---: |
| cmd | 21.5 (5.3–57.6) | 27.7 (-53.0–57.0) | -10.4 (-56.1–34.9) | -31.2 (-41.4–-3.1) |
| no-profile-hook | 22.3 (-65.0–57.4) | 22.9 (-55.4–60.4) | 82.6 (-6.4–142.6) | 63.5 (-29.8–96.2) |
| normal-hook | 17.4 (-0.1–38.4) | 23.5 (-25.7–38.2) | 67.8 (-75.1–135.0) | 43.4 (-49.4–105.2) |

cmd's Terminal readiness advantage is small relative to between-run variation. PowerShell's no-profile gap is positive in 14/15 pairs, configured gap in 14/15; individual negative differences remain in the ranges. Process launch itself differs by less than a millisecond at the paired median. PowerShell shell creation is roughly 23 ms later in Terminal, while the post-creation readiness interval accounts for a further paired median difference of 43–64 ms. That interval includes unfinished host-side spawn, ConPTY transport, shell initialization and probe work; it is not pure shell CPU time.

## Endpoint equivalence and missing observations

| Requested endpoint | Comparable observation | Limits / unavailable counterpart |
| --- | --- | --- |
| Process launch | Same external pre-Process.Start epoch, OS creation, and Process.Start return | Return includes launcher/API overhead; OS creation differs from Rust main or Alacritty logger initialization |
| First visible/presented frame | IsWindowVisible flag observed identically | No first visible pixel/scanout measurement for either app; flag can precede content. Terminal has an API presentation marker; Alacritty RedrawRequested is only a request, not presentation |
| Shell spawn | OS child CreationDate for same shell command line | Creation precedes native spawn return. Terminal child-spawned and Alacritty Initialisation complete bracket different work |
| First shell output | Terminal first PTY bytes, first text processed and first content rendered | Alacritty first Terminal(Wakeup) is a proxy, not established first bytes/text/output. Equivalent first-output measurement unavailable |
| Prompt ready | Same per-shell callback/probe and epoch in both apps | PowerShell function return and cmd begin-edit callback differ from each other and from displayed prompt pixels. Neither proves prompt rendering |

For native attribution only, the instrumented Terminal main-relative first-present medians were 136.0/139.3/141.2 ms; first PTY bytes 188.2/298.7/297.2 ms; first content rendered 222.9/763.3/837.0 ms, in cmd/no-profile/normal order. cmd explicit Command lacks the normal LocalShell OSC readiness injection; its shared Clink probe supplies the comparable observation. Alacritty logger-relative first-redraw-request was 149.1/207.2/210.6 ms and Wakeup proxy 159.8/218.7/220.8 ms. **Do not subtract these native elapsed values across apps**, or use redraw/wakeup as frame/output equivalents.

## Gap ownership

| Contributor | Evidence | Ownership and implication |
| --- | --- | --- |
| Renderer/window startup | Authoritative Terminal CPU fonts ~28 ms, GPU/wgpu ~65 ms, renderer completion ~4–5 ms, renderer-to-present ~15 ms. Current instrumented font/GPU intervals remain ~27–28 / 64–65 ms. Alacritty scale-factor→glyph-cache-complete proxy ~86–91 ms includes window/context/font work | Terminal/app/font path and wgpu/DX12/driver versus Alacritty/WGL; scopes do not match. A 65 ms Terminal GPU scope is not a 65 ms measured backend gap. External visibility gap is much smaller |
| PTY/process spawn | Equivalent OS shell creation is ~23–28 ms later at paired medians. Current encoded Terminal spawn-return interval is ~113 ms for pwsh, versus authoritative normal -Command ~356–359 ms; Alacritty PTY-dimensions→initialisation proxy ~78–80 ms | App-owned sequencing/arguments plus ConPTY/process APIs/dependencies. Alacritty proxy cannot establish a faster equivalent CreateProcess boundary. Investigate argument sensitivity and return boundaries before optimization |
| Shell initialization/profile | Shared shell-creation→readiness intervals: Terminal cmd/pwsh-no-profile/normal ~136/537/613 ms, Alacritty ~163/484/562 ms. Same-cycle normal-minus-no-profile readiness medians ~70 ms Terminal / ~79 ms Alacritty; authoritative Terminal profile contribution ~80 ms | PowerShell/profile/runtime plus host spawn/transport overlap. Profiles explain much of total latency, but do not explain the inter-app no-profile gap. OS process creation alone cannot isolate ownership of post-creation differences |
| Visibility policy | Terminal presents first, then calls set_visible(true), focus_window(), then logs window-shown. Native present→shown is ~20–24 ms; external flag can appear during those calls. PTY startup follows startup presentation policy. Alacritty may be visible before initialization/redraw completion | Existing application policy is a measurable contributor. A visible flag does not establish a ready frame or prompt. Its exact net gap is not separable from window/renderer work in this observation |
| Measurement/tooling | Distinct native timer origins; encoded vs -Command; probes; 33 ms maximum poll gaps; diagnostic logging; unchanged but uncontrolled caches | Explains why normal Terminal baseline cannot be directly subtracted from instrumented Alacritty numbers. No compositor timing or equivalent Alacritty first-output marker was collected |

A separate **logging-disabled warm sensitivity cohort**, n=3 pairs/shell after one excluded warmup pair, disabled Terminal native diagnostics and Alacritty event/verbose logging while keeping the shared probes. Paired visible/readiness differences were cmd 43.9/8.1 ms, no-profile 32.6/66.5 ms, normal 27.3/89.9 ms. All 24 launches passed cleanup, geometry and hash checks. The PowerShell gap persists; this small later cohort does not quantify logging cost independently from drift and is not merged into the main sample.

## Decision and recommended next task

**No OpenGL prototype is justified by this comparison.** Terminal is close to Alacritty at the observable window and cmd-readiness endpoints. It retains a smaller app startup sequencing/visibility contribution and an unresolved PowerShell spawn/integration gap. This does not prove an app-code defect or a renderer-independent causal bottleneck; it establishes where the next controlled measurement should focus.

Next task: **matched native -Command PowerShell spawn/integration attribution**, preserving this normal baseline and the same profile/no-profile shell inputs in both hosts. Record equivalent process creation and native spawn call/return boundaries, shared prompt readiness, and actual visible content if an observer is available. Separate app sequencing and unfinished CreateProcess/ConPTY work from shell/profile and transport work. Only revisit a backend prototype if equivalent frame observations establish a substantial renderer-owned residual after those contributions are understood. No backend or optimization implementation belongs in the current task.

## Evidence and reproduction

[Completed plan](ALACRITTY_BASELINE_COMPARE_PLAN.md), [environment](perf/alacritty-baseline-compare/environment.json), [external samples](perf/alacritty-baseline-compare/external.csv), [all launches](perf/alacritty-baseline-compare/all-external.csv), [native/external samples](perf/alacritty-baseline-compare/samples.csv), [summary](perf/alacritty-baseline-compare/summary.csv), [paired deltas](perf/alacritty-baseline-compare/paired-deltas.csv), [raw traces](perf/alacritty-baseline-compare/traces.txt), [trace hashes](perf/alacritty-baseline-compare/trace-manifest.json), [shell/process provenance](perf/alacritty-baseline-compare/process-provenance.json), [audit](perf/alacritty-baseline-compare/audit.json), [cleanup](perf/alacritty-baseline-compare/cleanup.csv), and [logging-disabled evidence](perf/alacritty-baseline-compare/quiet/external.csv) are retained. Isolated TOMLs and cmd probe are alongside them. Original individual files, temporary directories and pilots remain under target only.

```powershell
./scripts/compare-startup-baseline.ps1 -Runs 15 -Warmups 2 -OutputDirectory target/perf/alacritty-baseline-compare-new
./scripts/summarize-startup-baseline-comparison.ps1 -InputDirectory target/perf/alacritty-baseline-compare-new
./scripts/compare-startup-baseline.ps1 -Runs 3 -Warmups 1 -QuietLogs -OutputDirectory target/perf/alacritty-baseline-quiet-new
```

Completion validation: preflight and full main/sensitivity collection, counts/arguments/hashes/profiles/geometry/cleanup/clock audit, script parsing and native log marker review, plus repository formatting/check/tests/doctests/Clippy/diff checks. Runtime Rust, Cargo dependencies, user config, architecture and roadmap are unchanged. Unrelated function-key acceptance task remains active.
