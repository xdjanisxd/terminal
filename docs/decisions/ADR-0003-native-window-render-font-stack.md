# Native Window, GPU Rendering, and Font Stack

Status: Accepted

## Context

A terminal grid needs consistent low-latency rendering, per-cell styling, DPI handling, font fallback, and bounded glyph caching across Windows, Linux, and macOS without a browser or large GUI framework.

## Decision

Use `winit` for windows and input, `wgpu` for GPU rendering, `fontdb` for font discovery, `swash` for shaping/rasterization, and a project-owned bounded glyph atlas/cache. Add these dependencies only when their owning implementation begins.

## Alternatives

- Native text widgets: inconsistent metrics and insufficient terminal-grid control.
- Electron/WebView, React, Qt, or another large GUI framework: outside product constraints and heavier than required.
- Direct platform GPU APIs: maximum control but duplicated cross-platform implementation.
- OpenGL: simpler surface area but less consistent backend strategy.

## Consequences

One native architecture can serve all target platforms with direct control over the terminal grid. GPU initialization, surface loss, adapter compatibility, DPI changes, and atlas behavior require explicit testing. Backend/fallback policy remains open. Exact platform support floors will be derived from selected dependency versions, SDK requirements, and validation evidence as required by ADR-0008.
