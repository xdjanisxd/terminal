# PTY startup overlap investigation

Branch: perf/startup-pty-overlap. No commit or push.

1. Complete: trace finalized config/workspace, font/DPI/grid, GPU/first frame, PTY/
   worker ownership, readiness and failure dependencies.
2. Complete: collect valid Windows x86_64 release n=5 baseline per cmd/NoProfile/
   normal configured pwsh; exclude failed alias launches.
3. Complete: establish exact CPU-font metrics as earliest safe data point; no
   provisional size, new runtime/thread pool or terminal ownership change.
4. Complete: implement bounded worker-factory experiment and deterministic focused
   tests; preserve first-frame drain gate and existing readiness condition.
5. Complete: collect n=5 comparison per shell. Reject and revert all production/test
   changes due unproven first-frame preservation (PowerShell medians regressed)
   and unresolved native cleanup. Retain report and reproducible harness changes.
6. Complete: all six final checks passed and release executable rebuilt from the
   reverted source; report and final handoff retain explicit measurement limitations.
   Native visual/focus/sizing/failure acceptance remains a checklist for any future
   overlap candidate; there is no retained production overlap to accept.

See STARTUP_PTY_OVERLAP_FINDINGS.md for exact timings, evidence and limitations.
