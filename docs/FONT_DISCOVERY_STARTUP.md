# Font discovery startup investigation

Branch: `perf/font-discovery-startup`. Base revision: `aade3952813c9dc3261bffb17b06849fcbd79561`. Windows x86_64 release, 2026-10-06. No commit or push.

The allocation-reducing fallback-sort experiment was reverted. It consistently saved about 1 ms in sorting, but did not establish a meaningful reduction in total CPU font preparation. Discovery's file opening/mapping/cleanup remains dominant. Retained changes are opt-in diagnostic scopes, executable selection in the measurement harness, and these findings. Font selection, fallback order, file/data ownership, metrics, shaping, rasterization and renderer/backend behavior remain unchanged.

## Method and evidence

Hardware: Windows 11 Pro build 26300, Intel Core i5-1250P, Intel Iris Xe Graphics driver 32.0.101.7088. Rust 1.98.1, `x86_64-pc-windows-msvc`. Default backend selection with no `WGPU_BACKEND` override. Default app sizing/DPI configuration and a single `cmd.exe /d /k` workspace. Caches were not cleared; each launch used a fresh process. These are warm-cache process-start measurements, not independently cold launches or compositor scanout timings.

There are 120 valid sequential launches: 15 baseline and 15 candidate for default generic monospace, 15 each with explicitly configured Consolas, ten alternating baseline/candidate pairs with dependency profiling, ten alternating on/off pairs of the baseline binary to assess profiling overhead, and ten alternating baseline/candidate pairs with dependency profiling disabled. No builds/tests overlapped measured launches. All reached the shell-output-rendered marker and closed without forced cleanup. Every valid sample, including tails, is retained.

An additional initial 15-run batch is excluded from comparisons because its dependency report used stderr and delayed startup by about 60 ms. It was replaced with a single separate report-file write. This is an instrumentation exclusion, not removal of slow samples. The excluded batch's CSV and metadata are retained separately.

The temporary `fontdb` 0.24.0 overlay aggregated per-operation timers/counters without per-file logging. Its `discovery` clock stops before the report write; the outer project scope includes that write. Ten alternating on/off pairs measured a median discovery difference of **+2.227 ms**, range −2.728 to +5.261 ms, and CPU preparation difference of +2.421 ms. Attribution timings therefore include instrumentation perturbation; do not subtract those medians from unrelated batches or treat them as production constants. Both candidate variants used identical profiling. The final comparison disabled dependency timers and report output, retaining only the ordinary project startup scopes. Disabled overlay guards still exist in both saved experiment binaries, so this is not an uninstrumented shipping benchmark.

Evidence: [all valid per-run stages and counts](measurements/font-discovery-startup/runs.csv), [summaries and paired differences](measurements/font-discovery-startup/summary.json), [host, revision and executable hashes](measurements/font-discovery-startup/environment.json), [default workspace](measurements/font-discovery-startup/default-workspace.toml), [Consolas workspace](measurements/font-discovery-startup/consolas-workspace.toml), and [temporary attribution patch](measurements/font-discovery-startup/fontdb-attribution.patch). Original launch logs remain under ignored `target/font-discovery/`; the archived CSV contains the stage timings and counters. The attribution patch is evidence, not an applied dependency change.

## Startup path and breakdown

`Renderer::new_with_font_settings` starts fonts, loads `FontSystem`, calculates cell metrics, then starts GPU initialization. On Windows, `fontdb::load_system_fonts` scans the system Fonts directory and the user's Local and Roaming Windows Fonts directories. It does not use a registry/DirectWrite font query here. Directory traversal filters font extensions, opens each accepted file, memory-maps it, parses collection-face metadata, inserts face records with file-backed sources, then unmaps/closes the file. The database retains metadata and paths, not those discovery mappings.

After discovery, exactly one family query selects the primary face and the existing monospace validation applies. The renderer collects all other face IDs, sorts by PostScript name then the **lexicographic formatted ID**, creates contexts/empty caches, and maps the primary face again for Swash metrics. Runtime character fallback selects from that same ordered list and caches results on demand.

The following medians are from the corrected 15-run default baseline. Nested inclusive scopes must not be added together. Residuals are calculated per run before taking their medians, rather than subtracting medians.

| Stage | Median ms | Meaning/count per launch |
| --- | ---: | --- |
| CPU font preparation | 27.029 | Inclusive `FontSystem::load_system` |
| Database construction | 0.001 | One empty database |
| System discovery, outer scope | 25.789 | Includes dependency report write |
| Dependency discovery | 25.460 | Inclusive traversal/loading/metadata work |
| File open | 7.304 | 657 files |
| Memory mapping | 6.332 | 657 files |
| Unmap and file close | 6.745 | 657 files |
| Face metadata parsing | 2.496 | 724 faces: raw tables, names, OS/2 and post metadata |
| Other file work | 1.256 | Collection handling, insertion, paths and timer overhead; residual |
| Directory open | 0.137 | Three `read_dir` calls; excludes iterator traversal |
| Entry type/canonicalization | 0.027 | 852 entries |
| Other discovery work | 1.055 | Iterator traversal, paths/extensions/environment and timer overhead; residual |
| Generic monospace lookup | 0.012 | One database query; enclosing resolution 0.034 ms |
| Configured Consolas lookup | 0.010 | One query; separate 15-run configured baseline |
| Primary monospace validation | <0.001 | Existing face check; microsecond timer rounded to zero |
| Fallback candidate collection | 0.008 | 723 non-primary IDs |
| Fallback sort | 1.188 | No glyph/support discovery here |
| Context/cache setup | 0.004 | Empty caches; no startup cache lookup/population |
| Primary face access/metrics | 0.105 | Separate from preparation; includes reopen/remap and Swash access |
| Preparation plus metrics | 27.116 | Median of per-run sums |

The file-open/map/release medians total approximately **20.4 ms**. These operations dominate font discovery; metadata parsing is about 2.5 ms. Family resolution and cache setup are too small to explain the historical 30 ms CPU cost. No repeated startup family query or repeated system database scan was found. The primary file is reopened for metrics, but that entire operation costs only about 0.1 ms. Reusing it would not remove discovery's hundreds of file operations. Glyph fallback/cache lookups and glyph rasterization are not part of this empty-frame preparation.

## Candidate and before/after

The candidate compared borrowed PostScript names and formatted IDs only when names tied, replacing repeated name clones and ID formatting inside `sort_by_key`. It retained the same comparison rule, including lexical ID ties, and did not defer work or change font-data lifetimes. Focused font tests passed (18 passed, two existing ignored). No new equivalence or native visual claims are made for this rejected candidate.

All values below are medians in milliseconds; “candidate” describes the rejected experiment, not the final worktree.

| Batch | CPU preparation baseline → candidate | Fonts-start → first-present baseline → candidate |
| --- | --- | --- |
| Default, 15 each, detailed profiling | 27.029 → 29.205 | 122.854 → 122.399 |
| Consolas, 15 each, detailed profiling | 26.928 → 25.305 | 117.577 → 118.102 |
| Default, ten alternating pairs, detailed profiling | 26.474 → 24.733 | 117.391 → 116.992 |
| Default, ten alternating pairs, dependency profiling disabled | 27.277 → 26.211 | 127.953 → 123.346 |

For the final ten pairs, the **median of paired CPU differences** was −0.229 ms, range −3.988 to +5.891 ms. The median paired sort difference was −1.051 ms, range −1.148 to −0.951 ms: a consistent local reduction. The median paired fonts-to-present difference was −4.532 ms, range −22.100 to +5.175 ms. The difference between batch medians and the median paired difference is intentional; they answer different questions.

Discovery variation obscured the small sort saving. First-present variation also includes GPU/driver work and exceeds the roughly 1 ms attributable CPU reduction. The separately configured batch's first-present median regressed by 0.525 ms. Consequently this experiment does not justify claiming a stable multi-millisecond startup improvement from sorting. The local saving was judged marginal for this task, and the optimization was fully reverted. No work was shifted to first text, and no first-text latency benefit is claimed.

## Final state and next task

No font libraries, persistent caches, discovery deferral, unsafe ownership changes, or production sorting changes remain. `Cargo.lock` is restored; normal builds use the registry dependency. The retained project scopes have no clock reads/output when startup diagnostics are disabled. The measurement harness can select a saved executable with `-Executable`, making alternating comparisons reproducible.

The remaining font bottleneck is the file-backed metadata discovery pass, not family lookup, metrics or startup cache construction. Reducing fallback discovery would require preserving all configured-family matches, collection indices and the exact later fallback ordering/coverage; skipping files or changing discovery order is not justified by this experiment. Keeping every mapped file changes resource use and file-mutation semantics, while unsafe shared mappings are unnecessary for the tiny metrics cost.

The evidence-supported next task is a focused Windows native trace of `File::open`, mapping and unmapping/close during this metadata pass, separating filesystem/security/driver overhead from page faults. Establish a quiet coarse-timing baseline and controlled cache conditions first. Only then assess a project-owned metadata discovery strategy with explicit fallback/collection equivalence; library replacement or persistent caches need their own measured justification. Overall startup is still dominated by wgpu instance/adapter/device+queue: 79.723 ms median in the final coarse baseline (73.339–96.072 ms).

## Validation

Final formatting, workspace all-target checks/tests, documentation tests, Clippy with warnings denied, and whitespace checks passed. The release binary was rebuilt with the normal registry dependency; a final native startup smoke reached first shell output and closed normally without forced cleanup. The candidate's focused font tests also passed before it was reverted.

Native measurement markers establish submitted frames and normal shutdown, not subjective visual/DPI acceptance or physical scanout. Since the candidate was reverted and no production font behavior changed, new manual visual acceptance is not claimed.
