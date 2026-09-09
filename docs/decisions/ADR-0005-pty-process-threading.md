# PTY Boundary, Process Model, and Threading

Status: Accepted

## Context

Windows ConPTY and Unix PTYs differ, PTY I/O must not block the UI thread, and idle CPU should approach zero without introducing avoidable concurrency complexity.

## Decision

Use `portable-pty` behind a narrow project-owned `terminal-pty` abstraction. Run one application process. Keep the `winit` event loop and rendering on the main thread; use worker threads for blocking PTY reads and child communication. Start with `std::thread` and `std::sync`, bounded communication, and event-loop wakeups. Do not add Tokio or another async runtime without a concrete need and a superseding ADR.

## Alternatives

- Implement ConPTY and Unix PTYs directly at first: more control but significant platform and lifecycle risk.
- Put PTY I/O on the UI thread: simpler but permits stalls.
- Adopt an async runtime immediately: capable but adds runtime and architectural complexity without an established need.

## Consequences

The initial runtime remains understandable and event-driven while sharing a PTY abstraction across platforms. Blocking operations, shutdown ordering, backpressure, child trees, resize, EOF, and exit behavior still require platform-specific tests. Queue capacities and overflow policy remain to be designed with the first session pump.
