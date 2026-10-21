# Terminal reset and parser recovery plan

Goal: improve reset/recovery correctness without changing established compatibility policies or adding unbounded buffering, background work, timers, or threads.

The working tree was clean on `fix/terminal-reset-recovery` at `009e2bb`, identical to `master` and fetched `origin/master`. The requested branch already existed. Goal tracking is active; the session provides no Plan Mode switch, so this document records the inspection-first plan.

1. Audit RIS/DECSTR, screen-local state, cursor defaults, keyboard state, parser cancellation/error recovery, and PTY lifecycle against previous compatibility fixes.
2. Reproduce suspected bugs with focused regression tests before modifying production code.
3. Implement minimal fixes in the existing owners; retain documented soft/full reset policies and bounded parser storage.
4. Verify chunk boundaries, malformed/incomplete/unsupported controls, Unicode, screens/history/selection, cursor defaults, and keyboard state.
5. Run focused suites, compatibility fixtures, workspace tests/checks, formatting, Clippy, release build, and applicable native checks. Record performance evidence and platform limitations.
6. Update behavior documentation and roadmap only for verified changes. Do not commit or push.

Initial candidates: app recovery stops on a documented zero-consumption UTF-8 error; vte dispatches canceled OSC payloads. These require failing regression evidence before fixes.

Completed: all six steps. Regressions confirmed lost PTY-chunk suffixes, canceled OSC effects, and capacity-truncated OSC effects before production fixes. Existing reset-state policies passed audit and were retained. Workspace/all-target and doctest suites, formatting/check/Clippy/release, release parser samples, compatibility fixtures, and opt-in Linux lifecycle/editor checks passed. See [final behavior, evidence, limits, and manual checklist](TERMINAL_RESET_RECOVERY.md). Windows/physical-key/visual acceptance remains explicitly unverified. No commits or pushes were made.
