# Next Linux Release Plan

## Scope and evidence

This plan uses the integrated `master` snapshot at `61de4cf` (2026-10-28) and current repository docs, especially `CURRENT_STATE.md`, `ROADMAP.md`, `TERMINAL_COMPATIBILITY.md`, `LINUX_UX_COMPATIBILITY_AUDIT.md`, and Linux release reports. The checked-out branch is `planning/linux-next-features`; no implementation changes are included.

Complexity: S = focused change, M = bounded cross-layer work, L = broad protocol or platform work. Runtime impact describes expected steady-state impact; all new work must retain event-driven idle behavior.

## Current feature inventory

| Area | Implemented | Partial or limited |
|---|---|---|
| Terminal semantics | Incremental VTE parsing; cursor, erase, margins, scroll, insert/delete, DA1/DA2 and DSR/CPR subset; DEC origin, reset/save state; ANSI/DEC graphics G0/G1; 16/indexed/truecolor SGR, bold/faint/italic, conceal, strike and basic underline | Many CSI/DEC/OSC/DCS semantics absent; selective erase and horizontal margins absent; native italic faces, blink, extended underline styles/colors absent |
| Unicode and history | Combining and bounded grapheme/ZWJ support; wide-cell invariants; bounded Primary scrollback and viewport | No text/history reflow on resize; grapheme shaping intentionally bounded |
| Input and selection | Mode-aware cursor/function/key input, Kitty keyboard reporting, bracketed paste, terminal mouse reporting, selection and Windows clipboard, OSC 52 write policy denied by default | Clipboard integration outside Windows deferred; broad physical-key and editor acceptance incomplete |
| UI and workflows | Tabs, splits, pane focus, command palette, configurable bindings, searchable tab picker, Primary search, URL hint mode, OSC 8 HTTP(S) links, saved workspace templates, startup commands | Runtime session/process restoration is intentionally absent; configured font changes require restart |
| Rendering and window | wgpu GPU rendering, damage-driven redraw, DPI/resize, bounded glyph cache, font fallback, themes, opacity, scrollbar, Linux native window integration | Measured Linux input-to-present latency and other-adapter coverage open; synthetic italic can clip; blur deferred |
| Configuration and operations | Validated TOML, partial hot reload, shell config, project workspaces, Linux packaging/install support, opt-in startup/throughput diagnostics | Release documentation/asset consistency had known defects; verify against current release workflow before publication |
| Lifecycle and performance | Event-driven bounded PTY queues, output drain budget, Linux cancellation before worker join, bounded history/cache, opt-in benchmarks | Windows cancellation and macOS PTY behavior remain open; Linux interactive application coverage incomplete |

## Gaps and limitations

1. **Linux application compatibility evidence:** the 2026-10-28 audit covers Vim/Neovim real-PTY execution, tmux input acceptance, and native KDE interaction. It explicitly does not prove physical keyboard delivery or rendered-screen correctness for tmux. Broader reproducible visual and key-path coverage is the clearest immediate gap.
2. **Keyboard and mouse edge behavior:** core protocols and app routing exist, but acceptance across physical key events, configurable overrides, mouse reporting, selection, and alternate-screen transitions is incomplete. Fix only reproduced failures.
3. **Terminal protocol coverage:** remaining CSI/SGR/OSC/DCS semantics are known omissions, not blanket defects. Select additions from a failing real application or compatibility fixture; do not pursue parity by count.
4. **Linux clipboard:** clipboard support beyond Windows is documented as deferred. It is useful for Linux workflows but needs desktop session/backend behavior and end-to-end selection/paste proof.
5. **Rendering latency evidence:** Linux GPU rendering and Wayland smoke exist, but input-to-present latency and adapter-specific behavior are unmeasured. Keep diagnostics opt-in and out of production hot paths.
6. **Scrollback resize:** history remains at capture-time widths. Reflow exists as a separate documented capability, but resize-history semantics remain limited; only reconsider after a concrete usability issue or bounded design review.
7. **Release quality:** the v0.2.2 post-release report records release-note/README mismatches and absence of a checksum sidecar, plus a blocked PTY-close issue in that validation context. Treat these as release-gate follow-ups requiring current confirmation, not as assumed present defects or feature work.

## Prioritized recommendations

| Priority | Task and goal | Acceptance criteria | Complexity | Steady-state performance / regression risk |
|---|---|---|---|---|
| P0 | **Linux release integrity gate.** Make packaged Linux guidance, assets, checksums, and install lifecycle agree. | Build/install a release candidate in isolated HOME/XDG dirs; verify archive contents, executable/version, checksum sidecar with `sha256sum -c`, desktop entry/icons, upgrade/uninstall preservation, and every linked guide path. Release notes state actual platform coverage. | M | No runtime cost. Low application risk; packaging/release workflow risk. |
| P1 | **Linux interaction acceptance harness.** Close the physical-input and visual evidence gap for existing features. | On KDE Wayland and X11 where available, record repeatable checks for physical Ctrl/Alt/function keys, configured binding precedence, selection/copy/paste, mouse reporting, resize, alternate-screen entry/exit, search/URL gestures, and tab/pane workflows. Capture screenshots/raw PTY where semantics matter; compare an identical workload with a reference terminal for any claimed compatibility fix. | M | No product runtime cost. Medium manual/environment risk. |
| P1 | **Linux desktop clipboard integration.** Add native clipboard read/write behind existing platform-owned interface. | Linux copy/paste Unicode and multiline text on Wayland and X11; paste remains bracketed when enabled; OSC 52 policy remains enforced; selection survives pane focus changes; no shell/global environment mutation. Exercise clipboard ownership loss and unavailable backend errors. | M | No idle cost; work only on user clipboard actions. Medium platform/backend and input regression risk. |
| P2 | **Reproduced terminal compatibility fixes only.** Use harness findings to select the smallest missing CSI/OSC behavior needed by an identified shell/editor/TUI. | Each behavior has a minimized byte fixture, terminal-core semantic regression, real-PTY Linux application check, and documented supported subset. No renderer-owned terminal semantics. | S–M per issue | Typically zero idle cost; parser-path cost must be benchmarked for hot printable paths. Medium semantic regression risk. |
| P2 | **Linux input-to-present baseline.** Establish a repeatable baseline before considering renderer/input optimization. | Opt-in release workload records host, compositor, adapter/backend, workload, median/p95 and variability; compare current build to baseline under equivalent conditions. Instrumentation is disabled by default and adds no production synchronization or polling. | S–M | Zero default runtime cost. Low code risk; measurements host-specific. |

### Proposed cycle sequence

1. Begin with P0 release integrity and P1 interaction acceptance, since they validate the shipped Linux experience and existing behavior.
2. Implement P1 clipboard only after Wayland/X11 backend ownership and acceptance paths are clear; retain the current platform interface and avoid a new dependency unless native APIs require it.
3. Select at most one or two P2 compatibility fixes from observed failures. Keep fixes in `terminal-core` or the existing platform/app owner, as appropriate.
4. Capture P2 latency baseline before proposing any optimization. End cycle with focused regressions, Linux release build, and documented manual/platform coverage.

## Defer or avoid

- **Plugin systems, scripting runtimes, SSH manager, AI, IDE/file/Git/Docker surfaces:** outside lightweight terminal scope; high maintenance and security surface.
- **Runtime session/process restoration:** explicitly outside current workspace-template model; adds persistence and process-recovery complexity.
- **Terminal feature parity sweep:** broad protocol expansion without a reproduced application need risks semantic regressions and expands maintenance indefinitely.
- **Blink and animated UI:** known deferred compatibility polish; adds recurring wakeups/redraws and idle CPU/battery cost. Reopen only for a demonstrated compatibility need with visibility-aware scheduling.
- **Background blur:** compositor-specific, already deferred; opacity exists. Avoid platform-specific complexity without clear user value and reliable cross-compositor behavior.
- **Always-on polling, timers, permanent threads, telemetry, or production diagnostics:** conflict with low idle CPU/RAM goals. Keep measurement opt-in and event-driven.
- **Large renderer/font architecture rewrites or eager GPU/resource initialization:** prior measurements show tradeoffs and no justified Linux benefit. Require paired Linux measurements first.
- **Unbounded Unicode shaping or wholesale CSI/OSC/DCS implementation:** complexity and regression surface exceed next-cycle value absent concrete compatibility failures.
- **Scrollback reflow redesign:** defer until a confirmed issue justifies its memory/semantic tradeoffs; preserve current documented capture-width behavior meanwhile.

## Decision rule

Accept work only when it closes a demonstrated Linux user problem or a release-integrity gap, has a bounded acceptance test, preserves cross-platform ownership boundaries, and does not add idle work. Complexity estimates are planning estimates, not commitments; verify runtime impact with paired measurements for hot-path or rendering changes.
