# PTY and shell startup latency

## Result

No latency optimization was implemented. Measurements justify keeping the current deterministic startup sequence. App-owned discovery, argument preparation and environment construction are submillisecond; native PTY creation takes about 8 ms. PowerShell's native spawn boundary takes about 370 ms, followed by substantially more shell initialization. The spawn boundary remains unresolved between native process/ConPTY work and shell-specific interaction; it must not be described as entirely profile-owned.

Default shell selection, profiles, PSReadLine, integration, startup commands and workspace behavior are unchanged. This task adds opt-in diagnostics, ordering tests and a reproducible release measurement harness.

## Diagnostic semantics

Enable `TERMINAL_STARTUP_DIAGNOSTICS=1`; optionally set `TERMINAL_STARTUP_LOG`. Existing renderer records and new app/PTY records use monotonic elapsed microseconds from Rust main entry. New session records include the pane ID. Normal startup emits no timing records, reads no diagnostic clocks, and installs no diagnostic PTY callback.

| Stage | Meaning |
| --- | --- |
| first-frame-presented | Successful renderer presentation of the initial frame, before PTY startup. Submission is not compositor scanout. |
| workspace-restore-begin/end | Initial configured workspace model construction. These do not measure a later palette-driven Saved Workspace file load. |
| shell-discovery-begin/end | App local-shell executable selection. |
| shell-integration-preparation-begin/end | Embedded integration argument construction before spawn. |
| session-preparation-begin, session-config-ready | App session configuration preparation. |
| pty-create-begin, pty-created | Native backend openpty boundary. |
| initial-size-sent | Successful openpty with the requested initial grid dimensions, not an extra resize. |
| environment-build-begin/end | CommandBuilder construction with inherited environment and overrides. Native environment-block serialization remains inside the spawn boundary. |
| child-spawn-requested, child-spawned | Native backend spawn call and successful return. This includes backend command-line, cwd and environment preparation and OS process creation. |
| pty-worker-ready | Session worker successfully created after spawn. |
| first-pty-bytes | Reader's first nonempty read, before app event processing. Control sequences count. EOF/errors do not. |
| first-output-processed | First PTY output processed by the app. |
| first-shell-text-processed | First processed output producing a non-whitespace terminal cell. Control-only output does not count. |
| first-shell-output-rendered | First successful presentation after shell text was processed in that visible pane. |
| shell-integration-begin/end | Diagnostic OSC emitted at the beginning/end of the embedded PowerShell hook. Record timestamps are app receipt times. End also includes shell_duration_us measured locally with Stopwatch. |
| prompt-ready, shell-ready | Existing OSC 133 prompt version first becomes nonzero. This is the readiness condition used for startup-command dispatch; it does not certify that PSReadLine has finished all interactive initialization. |
| startup-command-dispatched | Command successfully queued to the PTY after the existing prompt condition. This is not command completion. |
| session-spawn-failed, pty-worker-failed | Failure observations; existing error reporting remains in force. |

The diagnostic OSC parser is bounded and accepts split chunks. Diagnostics never gate output, spawn or readiness. Shells without the existing prompt integration have no prompt-ready/shell-ready marker. No new wait requires a marker to arrive.

## Native method

Windows 11 Pro 10.0.26200, x86_64, Intel Core i5-1250P, DX12 renderer; native release binary. PowerShell 7.6.6 came from the WindowsApps installation. A separate normal console probe found PSReadLine 2.4.5 loaded and an existing 69-byte current-user/current-host profile. Profile contents were not inspected. This probe is not proof of module state inside every measured interactive shell.

Seven fresh launches per scenario, interleaved in fixed scenario order with validation idle:

1. Normal configured startup: existing config, one local-shell pane.
2. Explicit resolved PowerShell with normal profiles and the same embedded hook.
3. The same explicit PowerShell with -NoProfile, hook preserved.
4. System cmd.exe in a command pane.

Warm launches; no cold-cache or load normalization. The explicit-profile comparison controls executable discovery. No user config was edited. Raw logs are ignored local artifacts under `target/perf/pty-startup/{configured,profile,no-profile,cmd}-{1..7}.log`. Earlier exploratory samples are excluded.

Reproduce after `cargo build --release -p terminal-app`:

```powershell
.\scripts\measure-pty-startup.ps1 -Runs 7
```

The harness generates isolated workspace files for comparisons, restores diagnostic environment variables, uses bounded external observation, closes only its own processes and fails if required presentation/readiness is absent. Its polling interval is harness-only, not a startup delay. Normal configured shells without prompt integration may lack that marker; cmd is checked using rendered output.

## Milestones

Elapsed milliseconds from main entry; median (minimum–maximum), seven launches. Precision rounded to 0.1 ms.

| Milestone | Configured PowerShell | Explicit profile | NoProfile | cmd |
| --- | ---: | ---: | ---: | ---: |
| First frame | 170.6 (154.2–185.5) | 158.8 (150.9–237.2) | 167.7 (156.4–210.5) | 169.3 (150.6–200.2) |
| PTY begin | 201.4 (174.3–228.3) | 193.7 (171.2–340.9) | 195.2 (184.5–276.9) | 203.9 (171.2–273.7) |
| Spawn requested | 209.4 (181.9–238.3) | 203.1 (179.6–348.8) | 204.5 (193.1–287.6) | 214.2 (179.3–282.7) |
| Child spawned | 576.7 (556.3–635.1) | 576.5 (553.3–756.9) | 574.0 (559.5–654.0) | 222.4 (186.4–293.3) |
| First PTY bytes | 587.3 (568.0–663.1) | 589.4 (563.3–768.6) | 584.2 (572.2–663.2) | 241.1 (199.1–319.5) |
| First shell text processed | 1316.6 (1276.8–1529.9) | 1286.3 (1268.6–1541.1) | 1056.2 (1026.3–1123.4) | 250.6 (216.8–348.0) |
| First shell output rendered | 1335.0 (1288.1–1548.9) | 1309.5 (1281.0–1582.4) | 1065.0 (1034.8–1131.4) | 260.0 (226.1–361.7) |
| Prompt-ready | 1299.9 (1267.7–1508.6) | 1281.5 (1265.3–1530.0) | 1038.1 (1006.1–1108.7) | Unavailable |
| Shell-ready | 1299.9 (1267.7–1508.6) | 1281.6 (1265.3–1530.0) | 1038.1 (1006.1–1108.8) | Unavailable |

Prompt markers can precede visible prompt text. First bytes can be only terminal queries; neither means shell output was rendered.

The prior presentation task measured 208 ms renderer-ready and 231 ms first presentation in another batch. These samples are not a before/after experiment and establish no new causal improvement.

## Timing breakdown and ownership

Durations are computed within each launch before taking medians; subtracting table medians is not equivalent.

| Duration, ms | Configured PowerShell | NoProfile | cmd |
| --- | ---: | ---: | ---: |
| App preparation through PTY begin | 0.290 (0.269–0.670) | 0.048 (0.044–0.096) | 0.044 (0.043–0.074) |
| Native PTY creation | 8.147 (7.105–10.334) | 8.913 (7.502–10.544) | 8.307 (7.709–9.734) |
| CommandBuilder/environment construction | 0.335 (0.288–0.516) | 0.383 (0.297–0.475) | — |
| Native spawn boundary | 374.412 (347.302–435.545) | 367.794 (354.948–399.936) | 8.301 (7.128–13.413) |
| Spawn return to integration begin receipt | 697.474 (661.142–838.932) | 404.535 (379.999–431.872) | Unavailable |
| Integration execution, shell Stopwatch | 7.373 (6.250–7.964) | 15.703 (13.969–16.698) | Unavailable |
| Integration end receipt to prompt-ready | 25.166 (21.702–34.826) | 36.281 (33.860–46.687) | Unavailable |
| First bytes to app processing | 0.197 (0.150–0.389) | 0.214 (0.116–0.659) | — |
| Shell text processed to presentation | 11.265 (9.541–19.024) | 8.976 (8.009–14.851) | — |

Configured shell discovery: 0.214 ms (0.192–0.257). Integration argument construction: 0.011 ms (0.010–0.032). Initial workspace model: 0.059 ms (0.043–0.075). Explicit-profile spawn-to-hook duration was 678.483 ms (667.787–755.816), consistent with the configured-profile case.

The NoProfile comparison demonstrates a substantial profile-associated difference after spawn, but does not isolate profile scripts from module loading, prompt setup or PSReadLine. NoProfile still takes roughly 405 ms before our hook starts. Separately identifying those shell costs requires shell-side profiling. Integration execution is much smaller than the delay before it; disabling it would sacrifice behavior for little demonstrated benefit.

Five additional bare PowerShell -NoProfile -NoExit launches without our hook had a 366.347 ms (352.218–371.887) spawn boundary, similar to integrated NoProfile. This argues against embedded-hook argument generation as the source of that boundary. Their rendered-output median was 730.400 ms (722.337–763.543), but readiness is not equivalent without OSC 133, so this is diagnostic, not a product improvement.

Five redirected .NET ProcessStartInfo probes without ConPTY showed warm ordinary PowerShell process start returns around 9 ms (all-sample median 8.720 ms, range 5.298–599.263 including a cold outlier); process exit including shell initialization had median 357.108 ms (298.785–897.505). cmd start median was 3.174 ms (2.978–19.244). These are different execution conditions: they support investigating the native ConPTY/process boundary, not attributing it specifically to Terminal, PowerShell, security scanning or the OS.

Five configured startup-command launches queued the command immediately after prompt readiness: prompt median 1253.216 ms (1248.138–1274.248), dispatch median 1253.263 ms (1248.195–1274.290). This provides no evidence of an app sequencing delay.

## Hot-path review

- Discovery probes PATH and fallback executable locations per local pane. Measured discovery is about 0.2 ms; caching or changing shell selection is unjustified.
- The embedded integration hook needs no runtime script-file generation/loading or path canonicalization. Argument preparation is about 0.01 ms.
- CommandBuilder builds the inherited environment and applies overrides once. Backend Windows command-line resolution, cwd validation and environment serialization remain inside the native spawn boundary. The installed portable-pty backend uses CreateProcessW with STARTUPINFOEX/ConPTY attributes; available instrumentation does not subdivide that boundary.
- Initial dimensions are passed into openpty. No extra initial resize was introduced. Subsequent worker resizes retain their existing path.
- Spawn remains synchronous after first presentation. Worker output is drained through the existing reader/event path. Observed first-byte processing is submillisecond for the measured single-pane configuration; no measured lock contention justifies speculative concurrency.
- Saved Workspace opening reconstructs fresh sessions through the existing path. The measured configured model construction does not include palette-driven saved-file loading. Multiple synchronous pane spawns could delay earlier panes' app processing; that scenario was not measured here.

No optimization, no before/after effect claimed. The remaining dominant measured costs are PowerShell initialization before integration and its unresolved native spawn boundary. Useful follow-ups are native ETW profiling inside that boundary, shell-side profile/module profiling, and separately measuring multi-pane Saved Workspace restoration.

## Tests and validation

All required Windows x86_64 checks passed:

- cargo fmt --all -- --check
- cargo check --workspace --all-targets
- cargo test --workspace --all-targets: 223 passed, none failed or ignored
- cargo test --workspace --doc
- cargo clippy --workspace --all-targets -- -D warnings
- git diff --check

Focused tests cover optional observer ordering, one spawn request, initial size at PTY creation, exactly one first-byte record, split/bounded integration markers, control-only output, presentation gating, startup-command prompt gating, failed queue retention and successful single dispatch. Existing native contract checks still exercise streaming, resize, EOF, process exit and ConPTY queries. A failed native spawn reports failure without a successful-spawn/first-byte milestone or diagnostic wait.

The final release build and all four harness scenarios passed a final smoke run. A diagnostics-disabled release launch executed a startup command that wrote a marker file; no diagnostic log or timing stderr/stdout was produced. This checks actual command execution beyond queueing.

Native measurements ran only on Windows x86_64. ARM64 and Linux/macOS execution was not available locally; shared interfaces and existing semantics are preserved, but these are not new native platform certifications. New manual Ctrl+C/Ctrl+Arrow or visual scanout acceptance was not performed. No default shell/profile or PSReadLine behavior was changed.
