# Current Task

Goal: finish F1–F12 support and required manual acceptance.

Branch: `feat/function-keys`; no commit/push.

Goal status: blocked pending human manual-verification results.

Implementation and automated validation are complete. Core owns PC-style xterm encoding; app maps keys and preserves explicit config binding precedence. All Shift/Alt/Ctrl combinations are encoded, independently of screen/cursor mode.

Files: core input/lib and encoding tests, app main, config lib, docs CONFIG/CURRENT_STATE/FUNCTION_KEYS, session handoff.

Validation: focused tests, fmt, workspace check/all-target tests/doc tests, Clippy -D warnings, diff check and debug build passed. Native PowerShell tests required native process permissions after sandbox access-denied errors.

Remaining: human cmd + Clink, PowerShell, and Vim/Neovim F1–F12/modifier checks, alternate-screen return and configured binding precedence. No manual results claimed. Computer Use guidance prohibits terminal automation.

Next: run the checklist in docs/FUNCTION_KEYS.md using target/debug/terminal.exe, record results, then complete the goal.
