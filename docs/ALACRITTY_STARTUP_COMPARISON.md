# Terminal / Alacritty startup comparison

Measured 2026-10-05 on Windows x86_64, branch `perf/alacritty-cold-start-compare`. This comparison and attribution investigation is **completed with deferred validation**, at the user's direction. **Independent cold-start comparison was NOT performed. Manual first-frame/background/focus comparison is deferred.** No runtime optimization, Alacritty patch, commit, or push was performed.

**Warm/uncontrolled-cache result:** in seven matched pairs per main scenario, Alacritty's window became visible a median **40.7–47.8 ms earlier**. The difference occurs before shell spawning: graphics/font initialization and presentation/visibility work account for the pre-shell critical path. The evidence does **not** isolate a backend-only DX12/OpenGL difference. PowerShell costs hundreds of milliseconds in both applications, with additional variation and a measurable advantage for Alacritty in the no-profile prompt probe. Independent cold-start comparison was **NOT performed**; these observations establish neither application's relative speed in true cold-start conditions. Manual visual acceptance is deferred.

## Environment and inputs

| Item | Recorded value |
| --- | --- |
| OS | Microsoft Windows 11 Pro, 10.0.26300, build 26300, 64-bit |
| CPU | Intel Core i5-1250P, 12 cores / 16 logical processors |
| GPU | Intel Iris Xe Graphics; driver 32.0.101.7088 |
| Boot | 2026-10-05 10:58:12.5 +03:00 |
| Primary batch | 09:35:39–09:40:09 UTC, approximately 38–42 minutes after boot; exploratory launches preceded it |
| Terminal | Commit `a598f0a841c6e340814d167607c8ca288ee6e56b`; native release build, `cargo build --release -p terminal-app -q` |
| Terminal executable | `C:\Users\gifted\Documents\Projects\terminal\target\release\terminal.exe`, 9,576,960 bytes |
| Rust / Cargo | rustc 1.98.1 (48a229cea 2026-09-01); cargo 1.98.1 (797e8a9bc 2026-08-05) |
| Alacritty | 0.17.0 (94e7c88), installed release executable |
| Alacritty executable | `C:\Program Files\Alacritty\alacritty.exe`, 6,003,200 bytes |
| cmd | `C:\WINDOWS\system32\cmd.exe` |
| PowerShell | 7.6.6; `C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.6.0_x64__8wekyb3d8bbwe\pwsh.exe` |
| Working directory | `C:\Users\gifted\Documents\Projects\terminal` for both apps and all shells |
| Terminal graphics | winit 0.30.13, wgpu 25; default Windows DX12-first selection, no `WGPU_BACKEND` override. Separate diagnostic probe confirmed `backend=Dx12`, Intel Iris Xe, same driver |
| Alacritty graphics | winit / glutin; logs confirmed WGL, hardware-accelerated OpenGL 3.3, Intel Iris Xe, same driver |

Binary SHA-256:

- Terminal: `C697AACF0E1E91648F0E19A32F124A717B0AF45D3B6850568773A8B0ED35711F`
- Alacritty: `5AD70DDC5C2A2BFE84084A2C4C73558E9360914F035A902D7A69277BE7249F1F`

The active Terminal configuration is `%APPDATA%\terminal\config.toml`: **cmd.exe, no arguments**, JetBrainsMono Nerd Font Mono, size 16, black background, one local-shell pane. The active Alacritty configuration is `%APPDATA%\alacritty\alacritty.toml`: **pwsh.exe -NoLogo**, JetBrainsMono Nerd Font Bold, 12 pt, background `#0d0f18`, padding 10. Comparing those defaults directly would confound shell, font, and theme differences. Their hashes and executable provenance are in [environment JSON](measurements/alacritty-startup-environment.json). Both active files and both binaries were unchanged when rechecked after measurement.

The harness used separate generated configs: the same Mono Regular family, Terminal size 16 logical pixels / Alacritty 12 pt at logged scale factor 1, black background, one pane, and the same working directory. Alacritty logged an 800×600 client area, 9×21 cells. Terminal retains its default winit window size and application chrome; exact text-grid/decoration equivalence was not measured. Profiles and inherited user environment were preserved, except per-pair `TEMP`/`TMP` directories and measurement variables. Both applications in a pair receive the same temp path. Parent/terminal-injected environment can still differ; no claim of byte-identical child environments is made.

| Scenario | Exact shell arguments in both apps |
| --- | --- |
| cmd | none; existing cmd AutoRun is enabled |
| no-profile | `pwsh.exe -NoProfile -NoLogo` |
| normal | `pwsh.exe -NoLogo`; installed normal profiles are enabled |
| no-profile-hook | `pwsh.exe -NoProfile -NoLogo -NoExit -EncodedCommand <identical hook/probe>` |
| normal-hook | `pwsh.exe -NoLogo -NoExit -EncodedCommand <identical hook/probe>` |

The first three are the primary same-shell comparisons. Explicit Terminal command sessions avoid its automatic PowerShell integration, preserving those exact arguments. The supplemental hook scenarios install the existing Terminal OSC7/OSC133 integration in **both** apps and write one UTC timestamp after the original prompt function returns, before returning prompt text. They measure prompt-function readiness, not visible prompt presentation. They include normal profile behavior without quoting differences; they are not interchangeable with the bare scenarios. Terminal's ordinary local-shell PowerShell integration is therefore represented separately, rather than silently added to only one app.

cmd AutoRun contains `%USERPROFILE%\cmd\init.cmd` and Clink injection from `C:\Program Files (x86)\clink\clink.bat`. This is the same configured cmd in both apps, not an isolated `/d` cmd benchmark. Git Bash was not added.

## Methodology and cache classification

The [measurement harness](../scripts/compare-startup.ps1) launches one release app at a time, alternates Terminal/Alacritty order on successive runs, and collects seven runs per app per scenario: **70 primary launches**. No compiler or validation ran during this batch. No primary sample was dropped, including the pronounced later slowdowns in both apps. A brief CPU observation did not establish their cause; desktop/background contention and cache/driver effects remain uncontrolled.

These are **fresh-process launches with warm/uncontrolled caches**, a reproducible approximation to app restart, not true cold starts. No file-cache eviction, driver/shader-cache deletion, standby-list flushing, or reboot loop was performed. New temp directories prevent stale logs; they do not make application binaries/fonts/drivers cache-cold. The launcher primes its .NET/Win32 path with `cmd /d /c exit`, which also warms cmd's process image. Even each scenario's first recorded sample follows earlier app and shell launches. `first-observed-uncontrolled-cache` in CSV is an observation label, not a cold-cache assertion.

To obtain independently cache-cold evidence, use a planned ordinary reboot and one app/scenario launch before prior launches/builds, then repeat on independent boots with app order alternated. Record boot time and background load. Such a result is only reboot-cold; persistent driver/shader caches can survive. Repeated reboots were not requested or needed for this reproducible warm comparison. **There is no true-cold-versus-warm speedup estimate in this report.**

Terminal's `elapsed_us` is converted to milliseconds. Its clock starts at Rust main entry, excluding OS loading. Alacritty's `[...s]` clock starts when its logger is initialized, also excluding OS loading and preceding startup work. Absolute native timestamps across apps have different origins; use external timing for end-to-end comparisons and native intervals for attribution. Existing baseline records such as `window-created=17230 us` mean 17.23 ms, not 17,230 ms.

External time starts immediately before `Process.Start`. OS process creation occurred 1.2–6.8 ms later; `Start` returned in 3.0–41.7 ms for Terminal and 3.6–17.5 ms for Alacritty. A Win32 `EnumWindows` probe includes hidden process-owned windows and filters tiny helper windows (client dimensions must exceed 100×100). `IsWindowVisible` measures the Windows visibility flag, not compositor scanout, occlusion, or correct pixels. `WaitForInputIdle(0)` measures message-queue input-idle availability, not shell/GPU readiness. Polling requests a 2 ms sleep, but maximum observed loop gaps were roughly 20–94 ms: timestamps are first observations, not exact native transition times. Foreground HWND ownership is recorded separately.

Alacritty used existing `-vvv --print-events` and `debug.persistent_logging=true`; stdout/stderr were asynchronously drained. No Alacritty source patch was necessary. Initial exploration was discarded because it exposed launcher JIT overhead, concurrent validation, raw multiline PowerShell argument parsing, and reused-PID log-file collisions. The retained batch primes the launcher, uses identical UTF-16 encoded hook commands, and isolates `TEMP` per pair. Alacritty's `ALACRITTY_LOG` is an output environment variable, not a configurable log destination. Canonical logs contain no log-creation errors. Exploratory directories are excluded from the committed measurements.

## Terminal native milestones

All entries are **median [minimum, maximum] ms**, n=7. `—` means unavailable. These are elapsed times from Terminal main entry, not stage durations. The complete per-launch stages and cost scopes are in [samples CSV](measurements/alacritty-startup-samples.csv).

| Milestone | cmd | no-profile | normal | no-profile + hook | normal + hook |
| --- | ---: | ---: | ---: | ---: | ---: |
| application-started | 0.3 [0.3, 0.5] | 0.3 [0.3, 0.9] | 0.3 [0.2, 10.4] | 0.4 [0.3, 1.7] | 0.4 [0.3, 0.6] |
| window-created | 26.5 [18.0, 46.1] | 23.8 [20.6, 73.5] | 28.9 [20.8, 83.6] | 29.7 [20.5, 50.7] | 20.3 [18.1, 50.4] |
| fonts-ready | 54.0 [43.8, 106.1] | 53.4 [46.8, 155.8] | 55.5 [48.6, 203.7] | 55.7 [47.5, 140.0] | 49.3 [45.0, 179.3] |
| gpu-ready | 132.9 [117.5, 280.3] | 128.8 [120.1, 395.7] | 128.5 [124.9, 510.4] | 135.5 [119.4, 370.9] | 125.7 [117.9, 361.0] |
| renderer-ready | 137.9 [122.0, 293.8] | 132.6 [124.6, 423.7] | 132.9 [129.5, 527.6] | 140.3 [125.3, 385.3] | 130.3 [121.9, 370.8] |
| first-frame-presented | 153.3 [135.5, 332.0] | 146.9 [138.6, 479.3] | 147.7 [142.7, 566.8] | 154.4 [139.6, 433.7] | 145.2 [135.2, 403.9] |
| window-shown | 177.1 [155.9, 383.6] | 165.8 [159.0, 532.4] | 172.4 [165.6, 621.7] | 182.4 [160.1, 491.4] | 166.7 [157.2, 456.8] |
| pty-started | 177.1 [155.9, 383.6] | 165.9 [159.0, 532.5] | 172.5 [165.6, 621.7] | 182.4 [160.1, 491.4] | 166.7 [157.3, 456.9] |
| child-spawned | 193.0 [170.2, 416.6] | 368.0 [300.5, 862.2] | 344.8 [306.2, 943.4] | 387.5 [301.2, 837.0] | 332.4 [301.6, 746.9] |
| first-pty-bytes | 203.1 [180.5, 453.6] | 390.9 [309.4, 920.9] | 365.5 [316.6, 997.7] | 406.0 [310.9, 917.0] | 341.3 [310.4, 784.6] |
| prompt-ready | — | — | — | 853.8 [707.1, 2444.7] | 848.4 [796.2, 1666.1] |
| first-shell-output-rendered | 246.5 [215.5, 535.0] | 836.1 [649.8, 1782.7] | 872.8 [762.1, 2503.3] | 879.2 [736.9, 2505.1] | 877.6 [823.7, 1714.4] |

`first-pty-bytes` can be control sequences from ConPTY, well before useful shell output. `first-shell-output-rendered` follows processing non-whitespace terminal text and a renderer submission; it can be a banner/profile message, not necessarily a prompt. `prompt-ready` exists only in the matched hook scenarios and observes OSC133A after prompt evaluation. Neither proves prompt pixels reached the display.

## Alacritty native milestones

All entries are **median [minimum, maximum] ms**, n=7, elapsed from logger initialization. Labels describe actual logs rather than invented Terminal equivalents.

| Milestone | cmd | no-profile | normal | no-profile + hook | normal + hook |
| --- | ---: | ---: | ---: | ---: | ---: |
| welcome | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| scale-factor | 15.6 [11.7, 36.2] | 14.8 [12.1, 37.4] | 15.1 [13.2, 83.0] | 15.0 [12.4, 35.7] | 14.9 [12.8, 57.6] |
| wgl-selected | 68.4 [56.2, 148.9] | 66.0 [53.5, 145.6] | 64.7 [58.0, 230.5] | 68.6 [64.6, 176.5] | 66.9 [61.0, 193.4] |
| loading-font | 78.8 [68.0, 165.4] | 77.0 [61.4, 162.3] | 76.6 [68.3, 252.0] | 81.3 [73.2, 194.9] | 74.7 [68.8, 216.0] |
| renderer-entered | 81.1 [70.4, 171.5] | 79.3 [63.6, 168.6] | 79.1 [71.0, 258.6] | 84.2 [75.9, 201.8] | 77.2 [71.0, 224.2] |
| renderer-selected | 81.4 [70.8, 172.5] | 79.6 [64.0, 169.4] | 79.5 [71.5, 259.5] | 84.7 [76.3, 202.7] | 77.5 [71.4, 225.3] |
| glyph-fill-start | 87.0 [76.4, 187.3] | 85.0 [69.5, 183.4] | 85.1 [77.1, 275.0] | 90.8 [82.5, 219.4] | 83.3 [77.0, 240.6] |
| glyph-fill-done | 99.4 [90.0, 259.4] | 97.4 [82.0, 264.6] | 97.9 [89.6, 363.4] | 103.4 [96.4, 336.2] | 96.8 [90.8, 314.5] |
| pty-dimensions | 118.3 [109.3, 310.4] | 116.4 [103.8, 321.3] | 120.6 [108.7, 414.6] | 131.4 [117.0, 393.5] | 124.0 [110.0, 429.0] |
| conpty-api | 119.7 [110.5, 313.7] | 117.7 [105.3, 324.3] | 122.1 [109.9, 417.7] | 132.8 [118.2, 398.6] | 125.4 [111.3, 432.2] |
| initialisation-complete | 138.7 [125.8, 342.4] | 311.4 [215.7, 818.1] | 238.4 [210.6, 915.9] | 287.3 [254.2, 657.8] | 277.5 [233.5, 632.2] |
| first-redraw-request | 139.4 [131.2, 343.9] | 312.4 [216.7, 818.6] | 238.8 [211.1, 916.3] | 287.7 [254.6, 659.7] | 277.9 [234.1, 633.5] |
| first-pty-wakeup-proxy | 148.9 [141.3, 382.9] | 337.9 [224.6, 828.5] | 255.8 [221.3, 927.2] | 299.5 [264.8, 747.0] | 286.8 [244.9, 681.4] |

Exact application entry, window creation completion, fonts-ready, GPU-ready, first present, window-shown, child-spawned, first raw PTY bytes, and first shell text/prompt **presentation** are unavailable from these stock logs. The scale-factor log occurs after window creation; it is not an exact creation timestamp. `Running on` is within renderer construction, not completion. `Filling glyph cache` follows renderer construction; `Cell size` follows common-glyph preload. `PTY dimensions` precedes PTY setup, while `Initialisation complete` follows it and worker setup. `RedrawRequested` is an event request, not a rendered frame. The first terminal `Wakeup` is an event-loop proxy for processed PTY output; control-only output and other wakeup causes prevent equating it with visible text. Supplemental prompt-function timestamps are provided separately.

The marker interpretation comes from the matching official revision: [window context](https://github.com/alacritty/alacritty/blob/94e7c88/alacritty/src/window_context.rs), [display initialization](https://github.com/alacritty/alacritty/blob/94e7c88/alacritty/src/display/mod.rs), [Windows ConPTY](https://github.com/alacritty/alacritty/blob/94e7c88/alacritty_terminal/src/tty/windows/conpty.rs), [logging](https://github.com/alacritty/alacritty/blob/94e7c88/alacritty/src/logging.rs), and [PTY event loop](https://github.com/alacritty/alacritty/blob/94e7c88/alacritty_terminal/src/event_loop.rs). The installed binary's emitted version matches that revision.

## External same-shell comparison

Median [range] ms from external launch request, n=7. Raw launcher measurements are in [external CSV](measurements/alacritty-startup-external.csv).

| Scenario | App | Window exists | Window visible | Input idle |
| --- | --- | ---: | ---: | ---: |
| cmd | terminal | 43.9 [31.7, 99.8] | 182.5 [156.8, 372.6] | 229.0 [187.4, 464.0] |
| cmd | alacritty | 43.8 [28.3, 80.8] | 126.0 [122.2, 316.4] | 173.4 [154.5, 409.4] |
| no-profile | terminal | 48.1 [32.6, 149.1] | 173.0 [152.2, 572.1] | 219.8 [200.5, 685.1] |
| no-profile | alacritty | 41.8 [24.8, 77.8] | 132.9 [112.7, 324.6] | 180.5 [160.0, 432.6] |
| normal | terminal | 50.8 [40.4, 153.1] | 176.0 [162.0, 642.4] | 222.6 [212.3, 759.0] |
| normal | alacritty | 38.5 [27.0, 166.9] | 134.0 [116.2, 464.1] | 181.8 [162.4, 604.3] |
| no-profile-hook | terminal | 49.1 [42.2, 128.5] | 168.1 [161.2, 515.7] | 243.3 [210.7, 613.0] |
| no-profile-hook | alacritty | 37.1 [30.6, 110.7] | 138.4 [123.4, 420.0] | 185.3 [173.8, 528.2] |
| normal-hook | terminal | 43.2 [32.5, 217.4] | 168.8 [154.3, 574.1] | 221.3 [205.4, 667.7] |
| normal-hook | alacritty | 30.6 [23.2, 110.3] | 130.6 [117.5, 393.1] | 186.2 [163.8, 561.4] |

| Scenario | Median paired Terminal − Alacritty visibility difference [range], ms |
| --- | ---: |
| cmd | 45.5 [34.1, 90.6] |
| no-profile | 40.7 [-26.7, 247.5] |
| normal | 47.8 [28.0, 487.5] |
| no-profile-hook | 43.9 [18.8, 95.7] |
| normal-hook | 38.6 [-50.8, 181.0] |

A difference of app medians is not necessarily the median of pairwise differences. These pairs are sequential, not simultaneous, and fixed scenario order/background changes remain confounders. Positive values indicate Terminal was later. Exact first correct-frame presentation cannot be reliably measured by this harness.

A separate three-pair-per-main-scenario control disabled Alacritty trace/event logging, preserving Terminal startup diagnostics. Its external raw data is in [quiet control CSV](measurements/alacritty-startup-quiet-external.csv).

| Scenario | Terminal visible | Alacritty visible |
| --- | ---: | ---: |
| cmd | 194.4 [179.5, 229.5] | 133.8 [124.5, 172.0] |
| no-profile | 156.4 [153.0, 227.1] | 145.0 [143.6, 146.3] |
| normal | 185.3 [173.6, 215.3] | 132.3 [120.2, 133.9] |

The visibility advantage persists without Alacritty trace logging. This small later control does not establish a causal millisecond logging correction; its host/cache state differs from the primary batch. Terminal diagnostic-file overhead was not separately estimated.

## Renderer, font, and application attribution

Terminal creates a hidden winit window, initializes its font system, creates the wgpu instance/surface/adapter/device/queue, configures the surface, and builds shared rectangle/glyph resources. It then renders the empty themed grid, presents, shows/focuses, and only then spawns PTYs. Adapter/device calls are synchronous on this startup path. Alacritty creates a hidden winit window, chooses glutin WGL configuration/context, initializes crossfont/glyph metrics, creates/makes-current its GL surface, constructs its renderer, preloads common glyphs, clears/swaps/finishes, shows, and then initializes the terminal/PTY.

The cmd scenario minimizes PowerShell-related noise for the following duration table (n=7):

| App | cmd interval (not elapsed milestone) | Median [range], ms |
| --- | --- | ---: |
| terminal | interval-font | 27.7 [25.7, 64.6] |
| terminal | cost-system-font-discovery | 26.3 [24.5, 61.7] |
| terminal | interval-gpu | 78.8 [73.7, 174.3] |
| terminal | cost-adapter-request | 32.4 [30.7, 89.0] |
| terminal | cost-device-request | 39.1 [35.7, 98.8] |
| terminal | interval-font-through-renderer | 112.6 [103.9, 252.3] |
| terminal | interval-renderer-to-present | 15.2 [13.5, 38.2] |
| terminal | interval-present-to-show | 22.7 [20.4, 51.6] |
| alacritty | interval-window-context-proxy | 63.4 [52.4, 129.2] |
| alacritty | interval-font-surface-proxy | 2.4 [2.3, 6.1] |
| alacritty | interval-renderer-proxy | 6.0 [5.9, 15.8] |
| alacritty | interval-glyph-fill | 13.1 [12.5, 72.1] |
| alacritty | interval-window-through-glyphs | 84.8 [74.4, 223.2] |
| alacritty | interval-glyphs-to-pty | 20.2 [17.2, 51.1] |

Alacritty's scale-to-font-loading interval includes GL display/config/context work **and rasterizer initialization before the loading log**. Its font/surface interval contains font loading/metrics and surface activation. Its renderer interval starts after renderer construction has already entered. Therefore comparing Terminal's 78.8 ms `gpu-started → gpu-ready` directly with Alacritty's 6.0 ms renderer proxy would be invalid.

The broader pre-presentation graphics/font bundles are 112.6 ms for Terminal (fonts-started → renderer-ready) versus 84.8 ms for Alacritty (window scale log → cell-size log), a **27.8 ms difference of medians**. These boundaries are approximate: Alacritty includes context/rasterizer and glyph preload; Terminal's pipelines are still created on its subsequent first-render path. Terminal's rectangle/glyph pipeline creation medians are 5.3/5.8 ms within the 15.2 ms renderer-ready → first-present interval. The composite evidence is consistent with a material tens-of-milliseconds graphics/font initialization difference, but cannot assign that difference specifically to the API, driver, fonts, or resource strategy.

Terminal's font discovery alone is 26.3 ms [24.5, 61.7], almost all of its 27.7 ms font interval. Wgpu adapter/device requests are 32.4/39.1 ms medians; instance creation is 6.1 ms. Other app work is small: CLI 0.4 ms, config 0.2 ms, event-loop creation 3.5 ms, window creation/branding 13.1 ms, state/path initialization 0.2 ms. Scoped durations can be nested; medians are not additive per-launch accounting.

The 22.7 ms Terminal present-to-show interval includes native visibility/focus requests; Alacritty's glyph-done-to-PTY-dimensions interval is 20.2 ms and contains clear/swap/finish/show plus intervening app work. These are not exact equivalent visibility durations. Window existence itself is similar in the cmd external medians (43.9/43.8 ms). A simpler Alacritty application path and different resource/presentation policy are plausible contributors; neither has a separately timed complete counterpart.

## PTY and shell attribution

Median [range] ms, n=7. Terminal has actual child-spawn/first-byte milestones; Alacritty's PTY interval includes setup plus worker initialization, so it is not a child-spawn-only timer.

| Scenario | Terminal PTY creation | Terminal spawn call | Terminal spawn → first bytes | Alacritty mixed PTY setup proxy |
| --- | ---: | ---: | ---: | ---: |
| cmd | 7.8 [6.9, 16.0] | 6.7 [6.5, 16.0] | 10.3 [9.8, 36.9] | 23.6 [16.3, 32.0] |
| no-profile | 6.9 [6.4, 17.9] | 157.0 [128.7, 357.6] | 11.5 [8.4, 58.7] | 151.6 [99.3, 705.9] |
| normal | 7.7 [6.8, 16.4] | 164.2 [128.7, 364.0] | 10.6 [9.8, 54.4] | 113.8 [100.1, 795.6] |
| no-profile-hook | 7.8 [6.5, 17.6] | 150.3 [132.4, 326.9] | 12.3 [8.8, 80.0] | 169.8 [96.5, 264.4] |
| normal-hook | 8.1 [7.0, 14.1] | 150.8 [136.3, 275.0] | 9.6 [8.7, 37.8] | 157.5 [122.5, 203.3] |

Terminal's cmd spawn call is 6.7 ms; PowerShell spawn calls are roughly 150–164 ms. Its PTY creation itself remains roughly 7–8 ms. First PTY bytes arrive about 10–12 ms after spawn returns, but first printable output is much later: no-profile 360.8 ms [332.5, 1120.7] and normal 458.8 ms [443.1, 1525.5] after spawn. The distinction prevents blaming a renderer for the entire time until PowerShell text appears. Alacritty's comparable broad PTY setup also expands substantially with PowerShell; stock logs do not expose an exact child-spawn interval. Both use ConPTY, but Terminal uses portable-pty and Alacritty its own Windows path; handle setup, environment variables, and timing boundaries differ.

| Matched prompt-function probe | Terminal external readiness | Alacritty external readiness |
| --- | ---: | ---: |
| no-profile-hook | 879.4 [720.7, 2517.2] | 760.9 [689.3, 2300.1] |
| normal-hook | 871.7 [809.4, 1831.1] | 834.4 [743.5, 2624.4] |

The matched probe's **paired** prompt-function difference is 103.8 ms [-21.2, 217.2] for no-profile and 35.1 ms [-793.3, 83.0] for normal profiles. Thus identical shell input does not imply identical measured startup: the no-profile sample set does show a typical additional Terminal delay beyond its earlier visibility gap. This cannot be reduced to profile scripts, since profiles are disabled in that scenario. The normal scenario gives no consistent large Terminal-specific shell penalty.

The investigation eliminated mismatched executable/arguments/cwd, asymmetric integration, and multiline argument quoting as explanations. Both receive the same inherited parent environment apart from app-specific integration; neither config supplies extra shell environment. ConPTY libraries differ, and stock Alacritty logs lack the spawn/first-byte/prompt markers needed to split its residual shell difference exactly. No child-environment dump or ETW process trace was collected, so environment/platform ownership of that residual is **unresolved**, not assumed identical. The substantial shared PowerShell/process startup remains the largest contributor to time until a prompt; it does not explain the window-visibility gap because spawning follows show in both apps. The probe adds integration/file-I/O work to both apps and is not a bare-shell visual-prompt measurement.

## Visibility and perceived startup

Source ordering establishes that **both apps show before shell output**, after submitting a configured empty background frame. Neither waits for PowerShell's prompt to show. Terminal explicitly requests Windows focus after show. Alacritty's explicit `focus_window` call at this point is macOS-only. The external foreground probe observed Terminal in all 35 primary launches, Alacritty in only 2 of 35; this is a real session observation, not proof that Alacritty never gains focus or that source policy alone caused it. Foreground restrictions, existing desktop/window-manager state, and polling can affect the result.

UI automation/desktop pixel capture was unavailable. The visibility flag and renderer submission cannot establish immediate correct background, absence of a stale/white frame, or perceived focus. **Manual first-frame/background/focus comparison is deferred**, using the same generated configs and scenarios when this follow-up is resumed:

1. Observe from launch through prompt: first visible client pixels should be the configured black background; record any white/default/stale frame.
2. Record whether an empty terminal frame appears before the prompt and whether it stays responsive while PowerShell loads.
3. Confirm foreground focus and immediate typed input without clicking; note existing app/window-manager interference.
4. Compare cmd, no-profile pwsh, and normal pwsh separately; record first visible text versus first usable prompt.
5. If recording video, report its frame rate and camera/display latency; do not identify a native show flag or redraw request as a scanned-out frame.

## Gap classification and next task

| Bucket | Evidence and limit |
| --- | --- |
| A — renderer/GPU | Largest Terminal startup duration: about 74–79 ms median GPU init; another roughly 11 ms first-frame pipeline creation. Alacritty's full GL context cost is not isolated. Graphics/font bundles differ by about 28 ms in cmd, not a backend-only estimate. |
| B — fonts | Terminal discovers system fonts for about 26 ms per fresh process. Alacritty font work spans multiple mixed intervals, including rasterizer work before its font log; exact excess unavailable. This is a concrete Terminal critical-path cost. |
| C — app initialization | Terminal config/state costs are sub-ms; event loop ~3.5 ms, window/branding ~13 ms. No evidence of hundreds of ms in workspace/config parsing. Alacritty's counterpart is not fully instrumented. |
| D — PTY/process spawn | cmd spawn ~7 ms versus PowerShell ~150–164 ms in Terminal; Alacritty's mixed PTY setup similarly expands. Both occur after show, so cannot cause the pre-shell visibility gap. Exact inter-app spawn delta unavailable. |
| E — shell-owned startup | Hundreds of ms from PowerShell spawn to useful text/prompt in both; broad overlapping ranges. Matched no-profile prompt probe retains ~104 ms paired Terminal disadvantage; environment/ConPTY/startup ownership of the residual is unresolved. |
| F — presentation/visibility | Terminal presents, then spends ~23 ms until show/focus completion. Alacritty also clears/swaps before show. The external paired visibility advantage is ~41–48 ms for main scenarios; pixel/focus acceptance pending. |
| G — artifacts/cache | Warm caches, 20–94 ms polling gaps, different native origins, PID log collisions in discarded pilots, and later host slowdowns. All clean primary samples retained. True cold numbers unavailable. |

**Dominant measured warm-start gap:** the pre-shell graphics/font/presentation critical path, not PowerShell's wait before showing a window. A unique dominant *sub-bucket* cannot be established with stock Alacritty markers. The prompt gap has additional shell/platform uncertainty and must not be folded into renderer attribution. No attribution or relative performance conclusion here depends on uncollected cold-start data.

**OpenGL prototype: not justified by this evidence.** Alacritty is earlier overall, but GL context setup is itself substantial, mixed markers are not backend-equivalent, and both caches and presentation behavior remain confounded. This comparison supports no backend change or durable architecture decision.

**Exactly one recommended next optimization task:** reduce Terminal's full system-font discovery on the first-frame path, while preserving configured-face and fallback correctness. Use the existing 26 ms discovery scope as the baseline and require matched before/after release measurements and independent cold evidence before claiming a cold-start win. This is a bounded measured cost, not a promised 26 ms net saving. No such optimization was implemented here.

## Validation and retained evidence

Windows x86_64 release build succeeded. All required validation passed:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets` (quiet output)
- `cargo test --workspace --all-targets`, including native smoke tests
- `cargo test --workspace --doc` (4 passed)
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`

Rust runtime/config/dependency files were not changed. Only measurement scripts, this report/evidence, and startup/handoff documentation changed. ARM64 is out of scope. No commit or push.

The retained [samples CSV](measurements/alacritty-startup-samples.csv), [summary CSV](measurements/alacritty-startup-summary.csv), [external CSV](measurements/alacritty-startup-external.csv), [quiet control CSV](measurements/alacritty-startup-quiet-external.csv), and [environment JSON](measurements/alacritty-startup-environment.json) permit numeric review without loading entire logs. Native logs/configs remain local ignored artifacts in `target/startup-comparison-clean`; the control is in `target/startup-comparison-quiet-control`.

Reproduce in **PowerShell 7**, with a new output directory each time:

```powershell
cargo build --release -p terminal-app -q
./scripts/compare-startup.ps1 -Runs 7 -OutputDirectory target/startup-comparison-repro
./scripts/summarize-startup-comparison.ps1 -InputDirectory target/startup-comparison-repro
./scripts/compare-startup.ps1 -Runs 3 -Scenarios @('cmd','no-profile','normal') -QuietLogs -OutputDirectory target/startup-comparison-repro-quiet
```

The scripts close only the app instance they launch, with measured-process tree cleanup if graceful close times out. Do not run builds/validation during timing collection. Defaults do not evict caches. The investigation is closed as **completed with deferred validation**: the warm comparison and normal validation are retained, independent cold-start comparison was **NOT performed**, and pixel-level perceived-startup acceptance is deferred. This closure does not validate the deferred claims.

### Deferred follow-up: independent first-launch and manual acceptance

Follow-up status: **deferred**, not an active blocker for this closed investigation. On a future separately authorized session, collect independent first-launch evidence with recorded boot/cache provenance and complete the manual first-frame/background/focus checklist above. Preserve these warm-start samples as the existing baseline; report new evidence separately before making any true-cold or pixel-level comparison. The reproduction recipe below prepares that follow-up and does not imply it has been performed.

The harness supports selecting one application and skipping its cmd process-launch primer. Prepare/build before a planned ordinary boot, then run only one app/scenario before launching either terminal. For example:

```powershell
./scripts/compare-startup.ps1 -Runs 1 -Applications terminal -Scenarios cmd -SkipLauncherPrime -OutputDirectory target/startup-first-launch-terminal-cmd
./scripts/summarize-startup-comparison.ps1 -InputDirectory target/startup-first-launch-terminal-cmd
```

On a different independent boot, select `-Applications alacritty` and a new output directory, with the same scenario. Repeat across independent boots, alternating apps and recording boot time/background activity. The script does not reboot or evict caches. `-SkipLauncherPrime` leaves process-launch JIT overhead unprimed; external elapsed times include that overhead, while native intervals do not. Starting this collector in PowerShell itself loads PowerShell, so a PowerShell-shell result is **not shell-image-cold** even if the terminal app is its first launch. Windows prefetch and persistent driver caches can also warm first-launch paths. Preserve the uncontrolled-cache CSV labels and annotate the actual methodology; never relabel first launch as proven cache-cold solely from these switches. Existing 70/18-launch tables are unchanged.
