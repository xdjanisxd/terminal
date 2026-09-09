# Performance

Correctness and architectural clarity take priority over performance complexity.

## Initial goals

- Cold startup ideally below 100 ms where practical; below 150 ms is initially acceptable
- Stable 60 FPS terminal rendering
- Idle CPU usage approaching zero
- Immediate-feeling input
- Bounded and reasonable memory growth
- No busy polling or unnecessary redraws

## Rules

- Treat targets as budgets measured on named hardware and operating systems, not universal claims.
- Establish baselines before adding performance-specific complexity.
- Profile before replacing dependencies, adding threads, introducing `unsafe`, or performing micro-optimizations.
- Bound scrollback, queues, parser payloads, glyph atlases, and frame work.
- Coalesce resize/redraw work and render only when state changes.

## Measurements to add with implementations

- Cold and warm time to first usable frame
- Idle CPU and resident memory
- Input-to-present latency distributions
- Parser throughput for plain and escape-heavy streams
- Sustained-output behavior and queue pressure
- Resize-storm and high-DPI frame cost
- Glyph atlas hit, miss, and eviction rates
- Release binary size

Record the tool, build profile, commit, OS, CPU, GPU, display scale, and sample method with every baseline. Do not create hard regression gates until measurement noise is understood.
