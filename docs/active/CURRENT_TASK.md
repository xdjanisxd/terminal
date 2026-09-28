# Current Task

Release blocker: v0.1.1 Windows ARM64 Saved Workspace persistence failure.

- Branch: `master`; do not commit or push.
- Goal: identify the exact filesystem operation returning Windows error 2 on ARM64 and apply the smallest architecture-independent fix.
- Current state: original failure not reproduced on Windows x86_64. Each persistence operation now emits test diagnostics with destination/temp/backup paths, existence, canonical parents, PID, cwd, temp directory, and relevant environment values. Errors retain operation names and raw OS codes.
- Evidence: old test paths used wall-clock timestamps; a local 100,000-call Rust probe yielded 50,792 unique timestamps. This proves allocation can collide, but does not prove the ARM64 failure's cause. Tests now exclusively create and own separate temporary directories; no suite serialization or sleeps.
- Writer inspection: parent creation already precedes temporary creation; temp/backup extension manipulation keeps them beside the destination, including extensionless destinations. No early directory cleanup guard existed. Synced temporary files are now explicitly closed before rename; backup/restore semantics remain intact.
- Files: `crates/app/src/saved_workspaces.rs`, `crates/app/src/main.rs`, this handoff.
- Validation: both original UI tests (within four-test filter) passed 10 release runs; four direct persistence tests passed 10 release runs; all 101 app binary tests passed three release runs. Coverage includes missing parents/files, reload, replacement, backup recovery, deterministic failed-install restoration, spaces/Unicode, extensionless paths, and eight concurrent independent destinations. Formatting, workspace check, workspace all-target tests, doc tests, Clippy, and diff whitespace checks passed on Windows x86_64.
- Remaining: native ARM64 GitHub Actions validation. Canonical-parent diagnostics expose short/long path alias or junction differences if present; no runner-specific alias hypothesis has been established.
- Next step: inspect the instrumented ARM64 failure output for the named operation and its immediately preceding path snapshot. Keep the release blocked until ARM64 passes or that evidence leads to a confirmed fix.
