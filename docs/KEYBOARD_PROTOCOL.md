# Kitty keyboard protocol

Terminal supports Kitty's [keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) while retaining legacy VT/xterm input when an application has not enabled it. Terminal semantics and encoding live in `terminal-core`; the app preserves modal and configured-shortcut ownership before forwarding input to the focused PTY.

## Negotiation and flags

Applications negotiate with CSI-u. `CSI > flags u` pushes a request, `CSI < count u` pops the requested number of saved entries (default one), and `CSI = flags ; mode u` sets, adds, or removes flags with mode 1, 2, or 3. `CSI ? u` queries the current flags and receives the protocol's CSI-u response. Stacks retain at most 16 saved entries plus the current flags independently on Primary and Alternate screens; when a push exceeds the bound, the oldest saved entry is evicted. Switching screens preserves each screen's stack. DECSTR and RIS reset keyboard protocol state.

Supported progressive-enhancement flags are:

| Flag | Behavior |
| --- | --- |
| 1 | Disambiguate ambiguous legacy escape-code encodings. |
| 2 | Report the event type, including repeat and release when requested. |
| 4 | Report alternate key codes (shifted and base-layout) alongside the main key where available. |
| 8 | Report all keys as escape codes, including text-producing and modifier-key presses. This also disambiguates keys. |
| 16 | Report associated text in the key event; this enhancement is defined with flag 8. |

Flags combine. For example, flags `11` (`1|2|8`) request disambiguation, event types, and escape-code reporting for all keys. The key encoder uses inline storage for events up to 64 bytes before handing bytes to the existing PTY write queue; longer composed text can use heap storage and is not truncated. Legacy encoding remains unchanged while no Kitty request is active.

Normal application pops restore the previous flags. A terminal reset clears the stack. When a foreground app exits normally it is expected to pop its request; if it crashes or is killed without cleanup, the terminal cannot observe process ownership or infer that exit, so reset is the recovery path. Nested applications should push and pop their own requests.

## Key identity and event limits

The encoder distinguishes physical/logical key identity from committed text, so Tab and Ctrl+I, and Enter and Ctrl+M, can be represented separately under CSI-u. Repeat and release events are emitted when the request includes event reporting. Without flag 8, UTF-8 text is sent without a key event, and Enter, Tab, and Backspace do not receive release events even when flag 2 is set; this keeps recovery typing available if an application exits without resetting protocol state. App overlays and configured shortcuts retain ownership of the keys they consume.

Winit exposes Shift, Alt, Ctrl, and Super modifier state. It does not expose startup Caps Lock or Num Lock masks, nor Hyper/Meta modifier state; named Hyper/Meta keys may be reported as key presses, but modifier bits are not guessed. Unknown native keys and platform-specific key identity can therefore have different coverage across Linux and Windows. Held-event routing uses fixed storage for 64 simultaneously forwarded keys and 64 consumed keys; excess forwarded presses are still encoded but cannot be paired with a release.

## Automated evidence

Regression tests cover negotiation, ordered queries, unknown flags, nested requests and overflow, independent screens, resets, key identities, modifier combinations, alternate identities, associated Unicode text, and event routing. An allocation-counting test covers repeated core negotiation, queries, and common reports; the PTY worker test checks short inline and long input FIFO ordering.

Final Linux validation passed: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets --offline`, `cargo test --workspace --all-targets --offline` (824 passed; 27 environment-dependent tests ignored), `cargo test --workspace --doc --offline` (4 passed), `cargo clippy --workspace --all-targets --offline -- -D warnings`, and the release build of `terminal-app`. No Windows target was available locally. Latency and idle resource usage were not benchmarked; the implementation adds no idle work, timers, polling, or threads.

A Linux real-PTY test with Neovim 0.10.4 negotiated Kitty mode and distinguished Tab, Ctrl+I, Enter, and Ctrl+M using four separate mappings. It also checks alternate-screen exit and restoration to legacy mode. This is editor/PTY evidence, not physical-key Wayland acceptance. Run it with `cargo test -p terminal-app kitty_neovim_native --offline -- --ignored --nocapture`; it requires `/usr/bin/nvim` and writes raw session evidence under the temporary directory.

## Manual acceptance — pending

Run these checks in a Linux KDE Wayland build and a native Windows build. Record OS, terminal build, Neovim/Helix versions, observed bytes/behavior, and any failures. Automated encoder tests do not replace native acceptance.

1. In an ordinary shell, type text and use arrows, Tab, Enter, Ctrl+C, configured shortcuts, and paste. Confirm the preexisting shell behavior remains intact when CSI-u is not requested.
2. In Neovim and Helix, verify modified keys and key identity. Use a temporary mapping or key-display plugin to compare Tab with Ctrl+I and Enter with Ctrl+M; also check Ctrl+Shift, Alt, and Super where the OS/window manager permits them.
3. Use a raw CSI-u probe that pushes flags `11` (disambiguation, event types, and all-key escape codes), then press, hold/repeat, and release letter, Tab, Enter, Backspace, and modified keys. Confirm the exception for UTF-8 text and Enter/Tab/Backspace release reporting without flag 8, then pop with `CSI < u`; ordinary shell input should return to legacy encoding.
4. From a probe, push a second request, enter and leave an alternate-screen editor, then pop. Verify each screen restores its own request. Check DECSTR and RIS clear protocol state. A normally exiting probe should pop; forcibly terminate a probe with a request active and verify reset recovers legacy input.
5. Repeat shortcut checks in Neovim and Helix: app-owned configured shortcuts and modal controls must continue to work, while unconsumed input reaches the application.

Native Linux KDE Wayland and Windows acceptance is pending. Compatibility limits include missing startup lock-state masks and Hyper/Meta modifier state in the public winit event API, and keys with no platform-independent identity.
