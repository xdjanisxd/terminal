# Windows font file overhead: goal and plan

Branch: `perf/windows-font-file-overhead`. No commit or push.

Goal: trace Windows font startup file open/map/release overhead and retain only a measured, low-risk improvement preserving configured-family, fallback, glyph-selection and DPI/font-sizing semantics. If no safe improvement is justified, retain findings only.

1. Trace the renderer discovery call through fontdb and memmap2; distinguish enumeration, metadata parsing, fallback discovery and repeated opens.
2. Add opt-in temporary dependency instrumentation for unique paths and Windows operation boundaries, keeping normal dependencies unchanged.
3. Measure sequential Windows x86_64 release launches: normal baseline, attribution, paired profiler on/off, explicit configured family, restored normal build.
4. Evaluate redundant work and metadata/handle reuse. Do not replace libraries or add persistent caches without compelling evidence.
5. Retain a production change only with meaningful measurements and preserved semantics; otherwise restore dependencies and archive evidence.
6. Run repository completion checks and report ownership, dominant costs, timings, limitations and the next investigation.

Completed: source/API attribution and 55 valid launches. No production candidate was justified. Temporary dependency overlays were removed from the normal build. Results: [Windows font file overhead](WINDOWS_FONT_FILE_OVERHEAD.md). Kernel-stack attribution remains limited by the host's WPR policy.
