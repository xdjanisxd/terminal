# Windows font file overhead

On `perf/windows-font-file-overhead`, no production optimization is retained. Startup scans each installed font file once to construct the metadata database used by family resolution and fallback. There is no redundant app scan to remove. The remaining small extra operations belong to memmap2; bypassing them is not justified by their cost or behavior implications. No font library, cache, runtime source, manifest or lockfile content changes remain. No commit or push.

## Why 657 files are touched

`FontSystem::load_system` in `crates/renderer/src/font.rs` creates an empty `fontdb::Database`, then calls `load_system_fonts()` exactly once, before resolving the configured family. fontdb 0.24.0 scans `%SYSTEMROOT%/Fonts` and the Local and Roaming per-user Microsoft/Windows/Fonts directories. This Windows path is directory scanning, not DirectWrite enumeration or a registry-backed metadata query.

For each supported font extension, fontdb opens the file, maps it using memmap2 0.9.11, parses all collection faces and stores face metadata plus its file path. The temporary map and original file are then dropped. On this host every profiled launch recorded:

| Observation | Count |
|---|---:|
| Directory reads | 3 |
| Directory entry checks | 852 |
| System font paths | 656 |
| User font paths | 1 |
| Unique opened/mapped files | 657 |
| Duplicate paths within discovery | 0 |
| Parsed faces | 724 |

Collection files explain more faces than files. Five launches with the existing explicit Consolas workspace also produced these exact counts. Family selection occurs after discovery, so specifying a family does not narrow the scan. This count describes discovery, not all later process file activity: primary metrics may reopen the selected file, and glyph-time loading happens later.

The app owns the decision to eagerly construct a complete database. fontdb owns directory enumeration, metadata probing, opening and mapping every font. memmap2 owns the Windows mapping/handle lifecycle. Startup fallback preparation collects and orders face IDs already in the database; it does not independently scan files or test every font for each glyph. Database creation and family lookup are tiny compared with populating it.

## Dominant operations

The ten split-attribution runs (paired-on) produced the following medians. Times are elapsed boundary measurements in milliseconds, not CPU samples. Nested rows overlap and must not be added to their parent rows; individual medians also need not add to a median of per-run sums.

| Boundary | Median ms |
|---|---:|
| File open | 12.087 |
| Map, including metadata length and library work | 10.865 |
| Unmap and file release together | 11.519 |
| Per-run sum: open + map + release | 34.470 |
| Metadata parsing, 724 faces | 4.123 |
| Map drop (within release) | 6.357 |
| Original file close (within release) | 5.046 |
| File length/metadata (within map) | 3.296 |
| Actual CreateFileMappingW (within map) | 4.443 |
| MapViewOfFile (within map) | 1.137 |
| UnmapViewOfFile (within map drop) | 5.157 |
| DuplicateHandle | 0.339 |
| Close mapping handle | 0.458 |
| Close duplicate file handle | 0.305 |
| Write-protection probe | 0.256 |
| Execute-protection probe | 0.214 |

File open is the largest individual top-level boundary; release is nearly as costly, with both UnmapViewOfFile and closing the original File contributing substantially. Mapping also includes a relatively expensive file-length metadata query and actual section creation. The two protection probes each run 657 times in addition to the 657 real mappings; no VirtualProtect calls were observed. These probes establish supported protections for later map conversions, rather than repeating font discovery. Removing them would alter library behavior for roughly half a millisecond of measured work. Handle duplication and its release are also small and library-owned.

WPR `-start FileIO -start CPU -filemode` failed with “Failed to enable the policy to profile system performance” (0xc5585011). No trace was started. These measurements do not identify kernel stacks, security-filter/antivirus overhead, page-fault causes or physical disk I/O. A touched/mapped file is not proof of a disk read. Source and API attribution establish where elapsed cost appears, but not its kernel cause.

## Release measurements

55 fresh processes ran sequentially on Windows 11 Pro build 26300, x86_64 MSVC, i5-1250P / Iris Xe driver 32.0.101.7088, Rust 1.98.1. No cache flush or wgpu backend override was applied; no builds/tests overlapped launches. Every process reached rendered shell output and exited normally with no forced cleanup. Full per-run stages and executable hashes are archived under [measurements/windows-font-file-overhead](measurements/windows-font-file-overhead/README.md).

| Batch | Runs | CPU font preparation median [range] ms | Fonts-start to first-present median [range] ms |
|---|---:|---:|---:|
| Normal baseline | 10 | 40.542 [30.534–50.586] | 192.106 [142.530–258.106] |
| Attribution v1, unsplit release | 10 | 35.400 [32.349–50.543] | 159.511 [148.252–235.147] |
| Split profiler on | 10 | 47.960 [37.362–60.872] | 211.887 [164.113–233.576] |
| Same overlay, profiler off | 10 | 46.600 [35.012–51.772] | 226.567 [159.166–245.851] |
| Explicit Consolas, profiler on | 5 | 41.135 [33.681–55.217] | 170.506 [163.780–216.132] |
| Restored normal dependencies | 10 | 34.613 [30.275–50.279] | 162.092 [142.815–236.919] |

CPU font preparation is the per-run sum of `font-family-preparation` and `font-metrics`; fonts-start to first-present uses the existing startup marker through the first present call, not scanout.

The baseline/final comparison is a no-production-change check, not an optimization result. Both builds use normal registry dependencies; their binary hashes differ after rebuilding, and are recorded. CPU and first-present ranges overlap broadly, and GPU preparation moved from 116.786 to 99.157 ms despite unchanged production code. Batch drift prevents attributing the lower final median to a change.

The earlier investigation's ~27 ms CPU and ~20.4 ms file-operation medians belong to a different session. Here unsplit attribution measured 26.433 ms file operations [24.073–36.313], versus split attribution 34.470 [28.054–44.023]. Do not blend these batches or treat instrumentation as free. Alternating ten on/off pairs measured median profiler-on minus off CPU preparation +1.803 ms [−12.575–19.568] and discovery +1.289 ms [−12.916–17.407]. First-present differences were noisy (median −14.279 ms [−61.390–62.442]). Profiler output is written after its internal discovery timer but inside the app discovery scope; timers/path tracking also perturb elapsed time. The overlay's disabled branch still performs guard checks and is not a pristine baseline.

## Decision and next task

No production candidate was introduced. Temporary diagnostic dependency overrides were restored; only this report, completed plan, opt-in attribution patches, summary tool and measurements remain. The prior fallback-sort optimization remains reverted.

Narrowing discovery to the configured family would remove the available fallback metadata and could change fallback/glyph selection. Lazy discovery would move work into rendering and require preserving complete ordering and selection semantics. Retaining all mapping handles would move release work and retain resources without eliminating the initial scan; it is not a demonstrated startup win. No already-known complete metadata exists in the app to reuse, and there were no repeated discovery paths. A native metadata service or persistent cache would be a larger strategy change unsupported by these measurements. Configured family, fallback, glyph selection and DPI/font sizing remain unchanged because production code is unchanged.

The next font-specific task should capture ETW FileIO/CPU stacks on a host with profiling permission, under explicit warm/cold cache conditions, focusing on File::open, UnmapViewOfFile and original handle close. This can distinguish OS/security-filter cost before proposing a metadata strategy. For overall startup work, wgpu instance/adapter/device setup remains the larger target (final median 99.157 ms versus 34.613 ms CPU fonts).

Validation results are recorded in the measurement README after completion.
