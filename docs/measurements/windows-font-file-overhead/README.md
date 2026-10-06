# Measurement archive

See [findings](../../WINDOWS_FONT_FILE_OVERHEAD.md) and [completed goal/plan](../../WINDOWS_FONT_FILE_OVERHEAD_PLAN.md).

`runs.csv` contains all 55 accepted launches, merging original harness samples, per-stage durations and diagnostic counters. Duration fields ending in `.us`, stage durations and `*_us` are microseconds. Counts represent discovery only. Empty diagnostic columns mean profiling was disabled, not zero operations. `environment.json` records each batch's machine/build settings and executable SHA-256. `summary.json` is derived without excluding outliers. Regenerate it with `python docs/measurements/windows-font-file-overhead/summarize.py`.

Original logs and full path lists remain ignored in `target/windows-font-overhead`. `summarize.py --archive target/windows-font-overhead` regenerates this archive from those logs and asserts 657 unique paths and normal shutdown for every measured launch. Full user-specific paths are deliberately not archived; aggregate system/user/duplicate counts are retained. The ten v1 attribution launches used the same boundaries without splitting map drop from original file close; the retained patches represent the final v2 diagnostics.

## Repeat attribution

Use the normal `scripts/measure-first-frame.ps1` harness in native PowerShell 7. Build and measure sequentially; do not overlap builds, tests or other launches. Caches remain unchanged. A normal baseline requires no dependency override:

```powershell
cargo build -q --release -p terminal-app
./scripts/measure-first-frame.ps1 -Runs 10 -OutputDirectory target/windows-font-overhead/baseline-ps7
```

For diagnostics, copy pristine Cargo registry `fontdb-0.24.0` and `memmap2-0.9.11` into a fresh ignored directory, for example `target/windows-font-overhead/repro/fontdb` and `.../memmap2`. Never patch the registry itself. Normalize `fontdb/src/lib.rs`, `memmap2/src/lib.rs` and `memmap2/src/windows.rs` in the copies to LF before applying the LF patches. Both patches were verified with `git apply --check` against such pristine copies.

```powershell
git apply --directory=target/windows-font-overhead/repro/fontdb docs/measurements/windows-font-file-overhead/fontdb-attribution.patch
git apply --directory=target/windows-font-overhead/repro/memmap2 docs/measurements/windows-font-file-overhead/memmap2-attribution.patch
```

Create an ignored `overlay.toml` with absolute forward-slash paths:

```toml
[patch.crates-io]
fontdb = { path = "C:/absolute/repository/target/windows-font-overhead/repro/fontdb" }
memmap2 = { path = "C:/absolute/repository/target/windows-font-overhead/repro/memmap2" }
```

```powershell
cargo build -q --release -p terminal-app --config target/windows-font-overhead/repro/overlay.toml
$env:TERMINAL_FONTDB_PROFILE = '1'
./scripts/measure-first-frame.ps1 -Runs 10 -OutputDirectory target/windows-font-overhead/repeated-attribution
# Optional explicit-family comparison:
./scripts/measure-first-frame.ps1 -Runs 5 -Workspace docs/measurements/font-discovery-startup/consolas-workspace.toml -OutputDirectory target/windows-font-overhead/repeated-consolas
Remove-Item Env:TERMINAL_FONTDB_PROFILE
```

The harness sets `TERMINAL_STARTUP_DIAGNOSTICS` and a per-run `TERMINAL_STARTUP_LOG`. The overlay adds `.fontdb` aggregate reports and `.paths` discovery path lists alongside that log. Profiling is opt-in with `TERMINAL_FONTDB_PROFILE=1`. Each map timer counts 657 operations; additional protection probes are mapping API attempts, not extra font-file opens. Nested timers overlap. Diagnostics write once at discovery completion and perturb app scope timing; alternating on/off launches estimate that perturbation but show substantial noise.

Afterward rebuild without `--config`, restore the original lockfile content if necessary, verify `git diff -- Cargo.lock Cargo.toml` is empty, and repeat normal measurements. Keep temporary overlays ignored. There is no production optimization in these patches.

## Completed validation

Normal registry release builds succeeded before and after attribution. All six repository completion gates passed: formatting, workspace/all-target check, workspace/all-target tests (524 passed, five existing ignored), four doctests, Clippy with warnings denied and `git diff --check`. No ignored tests were enabled for this investigation. Both archived patches passed applicability checks; summary regeneration succeeded. Final review confirmed no runtime, manifest or lockfile content changes.

Native kernel attribution was unavailable: WPR refused to enable profiling policy (0xc5585011). Warm-cache one-host timings establish operation boundaries and ownership, not disk/antivirus/kernel causation or a production speedup.
