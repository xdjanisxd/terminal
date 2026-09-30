# Windows PowerShell native spawn investigation

## Conclusion

On Windows x86_64, PowerShell's extra native spawn latency is overwhelmingly **inside CreateProcessW with the ConPTY attribute attached**. Local backend preparation takes about 0.2 ms. No production optimization is justified by these measurements, and none was implemented. Profiles and shell integration remain enabled as configured.

This resolves the API boundary left open by [PTY startup profiling](PTY_STARTUP.md). It does not identify the internal Windows/ConHost/PowerShell mechanism; ETW would be needed for that attribution. A slow API return must not be equated with a Terminal bug or attributed exclusively to profile execution.

## Exact boundary and ownership

Project entry: PortablePtyBackend::spawn_observed in crates/terminal-pty/src/lib.rs. The lowest project-controlled invocation is portable-pty 0.9.0's slave.spawn_command(command). Its Windows implementation calls win/conpty.rs → win/psuedocon.rs → CreateProcessW.

Before child-spawn-requested:

1. App discovers the configured executable, prepares existing shell integration arguments and creates the configuration.
2. openpty creates input/output pipes, loads the ConPTY API and calls CreatePseudoConsole with the initial cell dimensions. Existing ConPTY flags are preserved (inherit cursor, resize quirk, Win32 input mode).
3. The master reader is duplicated (DuplicateHandle); the writer is moved out. Both operations take the backend mutex.
4. CommandBuilder clones inherited environment into its normalized map, applies arguments, working directory and explicit environment overrides.

The child-spawn-requested → child-spawned interval contains:

1. Acquire the shared ConPTY backend mutex (uncontended before workers exist).
2. Prepare STARTUPINFOEXW and a one-entry process attribute list: allocation, InitializeProcThreadAttributeList and UpdateProcThreadAttribute with the existing pseudoconsole handle.
3. Resolve the executable using the backend PATH/PATHEXT search and filesystem existence checks, even for the supplied absolute path; construct/quote the command line and convert executable/arguments to UTF-16. Reconstruct an OsString for potential error reporting.
4. Resolve/validate configured cwd and USERPROFILE fallback; convert the selected directory to UTF-16.
5. Serialize the inherited/overridden environment map into the sorted, double-NUL terminated UTF-16 Windows block.
6. Call CreateProcessW once with EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT, inherit-handles=false and the existing pseudoconsole attribute. The attachment occurs as part of this call.
7. Check its result, wrap process/thread handles, close the temporary thread handle, delete the attribute list and release temporary buffers; construct WinChild and box it for the caller.
8. In the diagnostic overlay only, write buffered stamps after backend creation returns. That logging is included in the outer observed boundary, but excluded from CreateProcessW and native pre/post intervals.

There is no additional ConPTY attachment/finalization call, process wait, shell readiness wait, profile execution by Terminal, reader thread startup or rendering inside the boundary. PortablePtySession is assembled after child-spawned. Two std-only worker threads then start. First bytes can include ConPTY control traffic; they do not mean prompt or visible shell text. App integration/prompt markers and first rendered shell text remain separate existing observations.

## Method

Measured 2026-09-30 on Windows 11 Pro 10.0.26200 x86_64, PowerShell 7.6.6, Rust 1.98.1. Seven launches per scenario, interleaved within each batch, release builds. No concurrent Cargo validation during measurement. Figures below are medians of **per-launch deltas**, in milliseconds; ranges are min–max, not confidence intervals. No timing assertions were added to unit tests.

The existing app observer and scripts/measure-pty-startup.ps1 are reused unchanged. Configured launches preserve normal settings; explicit profile and -NoProfile workspace scenarios retain the embedded integration hook and -NoExit. No default/profile setting is changed.

scripts/prepare-native-spawn-trace.py copies cached portable-pty 0.9.0 into a fresh target directory and generates a Cargo command-line patch configuration. Exact unique source anchors fail if the backend changes. This is a diagnostic overlay, not a vendored dependency or production patch. Instant stamps are buffered on the spawning thread and written after the backend spawn returns. Native logs use a separate .log.native file to avoid colliding with the app's persistent writer. App/probe and native stamps have different zero points; the summarizer never subtracts between origins. The initial shared-file batch was discarded for fine-grained analysis because its records could be overwritten.

The standalone spawn_probe example uses the same backend, initial 24×80 size and worker, without the renderer/event loop. It buffers its observer records rather than doing I/O inside preparation. It stops after the first output chunk, then terminates/joins its child; it does not claim prompt readiness or implement a terminal query responder. The trivial existing pty_test_helper outputs a marker and exits (exit 0). Scripts also compare ordinary redirected .NET Process.Start without ConPTY using cmd /c exit and pwsh [-NoProfile] -Command exit, separating Start return from exit. These are independent boundary comparisons, not identical interactive-shell workloads.

## Full app: fine-grained diagnostic batch

Raw logs: target/perf/windows-spawn/app-v2 (seven complete logs plus native companions per scenario).

| Interval | cmd | configured pwsh | pwsh with profiles | pwsh -NoProfile |
|---|---:|---:|---:|---:|
| App preparation | 0.042 | 0.262 | 0.043 | 0.044 |
| PTY creation | 7.237 | 6.859 | 7.182 | 7.733 |
| Input pipe | 0.037 | 0.037 | 0.038 | 0.043 |
| Output pipe | 0.005 | 0.004 | 0.004 | 0.005 |
| Pseudoconsole creation | 7.167 | 6.802 | 7.110 | 7.638 |
| Reader duplication/writer acquisition, including observer overhead | 0.018 | 0.015 | 0.015 | 0.016 |
| Environment clone/normalize and builder | 0.391 | 0.346 | 0.313 | 0.293 |
| Spawn lock | 0.000 | 0.000 | 0.001 | 0.000 |
| Attribute list | 0.003 | 0.003 | 0.002 | 0.002 |
| Executable resolution/filesystem checks | 0.083 | 0.080 | 0.077 | 0.071 |
| Quoting and executable/argument UTF-16 | 0.001 | 0.012 | 0.013 | 0.012 |
| Cwd resolution/UTF-16 | 0.028 | 0.036 | 0.029 | 0.029 |
| Environment UTF-16 serialization | 0.055 | 0.027 | 0.038 | 0.038 |
| Total native preparation before CreateProcessW | 0.175 | 0.181 | 0.186 | 0.156 |
| **CreateProcessW** | **7.595** | **351.144** | **352.686** | **343.237** |
| Native return/handle ownership/cleanup | 0.019 | 0.013 | 0.013 | 0.015 |
| Whole observed spawn boundary | 8.250 | 351.756 | 353.285 | 343.828 |
| Per-launch boundary excluding CreateProcessW, including diagnostic flush | 0.655 | 0.612 | 0.593 | 0.625 |
| Child-spawned → worker ready | 0.091 | 0.083 | 0.078 | 0.080 |
| Child-spawned → first bytes | 13.748 | 11.108 | 10.576 | 10.775 |
| Child-spawned → prompt | — | 494.444 | 488.137 | 405.065 |
| Spawn request → prompt | — | 854.025 | 836.900 | 745.840 |
| Spawn request → first rendered shell text | 58.946 | 867.153 | 859.518 | 769.331 |

CreateProcessW ranges: cmd 6.083–8.023; configured 335.408–370.054; profiles 321.236–372.582; -NoProfile 330.910–392.295. These ranges overlap substantially between PowerShell scenarios. Profiles have a clearer cost after the API return.

Configured app discovery costs 0.188 ms (0.172–0.287), integration argument preparation 0.010 ms (0.010–0.021). Explicit startup commands bypass configured shell discovery. Resolution, PATH scanning, metadata, environment normalization, quoting, duplicate handles, locks and worker startup are all immaterial relative to PowerShell. The ~7 ms ConPTY system call is common to both shells. The roughly 0.6 ms outer residual is an upper bound containing diagnostic file I/O, not a demonstrated optimization opportunity. Component medians must not be added/subtracted to infer aggregate medians.

## Normal dependency verification

After restoring Cargo.lock and rebuilding without the diagnostic overlay, the unchanged production release was measured seven times per scenario (target/perf/windows-spawn/production-final):

| Scenario | Native spawn boundary median (range) | Spawn request → rendered shell text median |
|---|---:|---:|
| cmd | 6.515 (6.278–11.759) | 57.667 |
| configured pwsh | 357.051 (317.845–364.651) | 873.978 |
| pwsh with profiles | 347.293 (328.509–706.019) | 860.193 |
| pwsh -NoProfile | 337.973 (326.776–366.575) | 773.373 |

All app scenarios reached rendered shell text; every PowerShell run reached the existing prompt-ready marker. This confirms that the large boundary is also present without native instrumentation. Batch differences and outliers are not an optimization effect. The overlay's sub-millisecond preparation/residual stamps should not be treated as zero-overhead production estimates.

## Independent comparison

Raw logs: target/perf/windows-spawn/native-final.

| Standalone scenario | ConPTY CreateProcessW median (range) | Ordinary Start return median (range) | Ordinary process exit median |
|---|---:|---:|---:|
| cmd | 13.615 (6.957–16.450) | 4.849 (3.937–23.702) | 28.535 |
| pwsh -NoProfile | 667.001 (470.673–710.365) | 8.110 (5.259–591.000) | 255.209 |
| pwsh with profiles | 724.631 (455.628–771.496) | 7.775 (4.369–693.624) | 403.368 |
| trivial helper | 5.223 (4.253–10.091) | — | — |

The standalone ConPTY probe reproduces a large PowerShell delay **inside the Windows call** without app rendering, event-loop processing or shell integration argument construction. Its absolute latency is larger than the app batch; these workloads and launch contexts differ, so this is not evidence of a regression or a quantified rendering effect. Local native preparation is only 0.111 ms for both PowerShell scenarios. Ordinary launches usually return quickly, with large isolated outliers, and exit later as PowerShell initializes. Thus it is inaccurate to claim every ordinary Windows PowerShell spawn always takes 350–700 ms. The observed cost is strongly associated with this ConPTY process-creation path/context; internal causation remains unmeasured.

## Production changes and dominant cost

No production runtime, dependency declaration, default shell, profiles, integration, PSReadLine, OSC 7, startup commands, Saved Workspaces, initial sizing, Ctrl+C/Ctrl+Arrow or first-frame sequencing changed. No process pool, hidden prelaunch, daemon, sleep or platform hack was introduced. Only diagnostic scripts, a manual example and documentation are added. Temporary Cargo.lock overlay changes are reverted and the final release binary is rebuilt with the normal registry dependency.

There is no optimization before/after claim. The dominant native-boundary cost is Windows CreateProcessW with PowerShell attached to ConPTY; the dominant remaining prompt delay also includes hundreds of milliseconds of shell initialization after Terminal regains control. Further Terminal-side micro-optimization is not justified. ETW inside CreateProcessW and shell-side module/profile profiling are the next evidence sources if deeper attribution is desired.

## Reproduction

Use a fresh overlay/output directory on each run (the generator refuses overwrite):

~~~powershell
py scripts/prepare-native-spawn-trace.py --output target/native-spawn-trace-new
cargo --config ./target/native-spawn-trace-new/patch.toml build --release -p terminal-app -p terminal-pty --bin terminal --bin pty_test_helper --example spawn_probe
./scripts/measure-pty-startup.ps1 -Runs 7 -OutputDirectory target/perf/app-new
./scripts/measure-native-spawn.ps1 -Runs 7 -OutputDirectory target/perf/native-new
py scripts/summarize-native-spawn.py target/perf/app-new target/perf/native-new
py scripts/test_native_spawn_trace.py
~~~

Save/reconcile any pre-existing Cargo.lock edits before using the overlay: the patch temporarily removes its registry source/checksum. Restore those two overlay-only changes, then rebuild without --config before distributing or doing final production validation. Normal release startup has no native instrumentation; diagnostic overlay recording is also opt-in via TERMINAL_STARTUP_DIAGNOSTICS=1. Harnesses restore their environment variables and clean up only their own processes. Raw logs are generated/ignored target artifacts; this report retains the findings.

## Validation

Passed: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo test --workspace --doc`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`. The focused PTY contract suite passed all seven tests. Five deterministic Python tests passed, covering trace parsing, missing/duplicate/reversed milestones, exact patch anchors, generated call ordering, single CreateProcessW invocation and preservation of the cached dependency. No timing-threshold assertions were added.

A diagnostics-disabled normal release launch executed a startup command that wrote a marker file; it created no diagnostic log and emitted no stdout/stderr. Repeated release app runs reached rendered output and PowerShell prompt readiness. Native ARM64/Linux/macOS measurements and new manual keyboard/visual acceptance are not available locally. Production behavior is unchanged; this investigation is not a new cross-platform certification.
