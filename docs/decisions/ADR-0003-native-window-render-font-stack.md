# Native Window, GPU Rendering, and Font Stack

Status: Accepted

## Context

A terminal grid needs consistent low-latency rendering, per-cell styling, DPI handling, font fallback, and bounded glyph caching across Windows, Linux, and macOS without a browser or large GUI framework.

## Decision

Use `winit` for windows and input, `wgpu` for GPU rendering, `fontdb` for font discovery, `swash` for shaping/rasterization, and a project-owned bounded glyph atlas/cache. Add these dependencies only when their owning implementation begins.

On Windows, try DX12 first, then fall back to wgpu's normal backend selection if GPU initialization fails. Honor an explicit `WGPU_BACKEND` override. Other platforms retain wgpu's normal backend selection. A native Intel Iris Xe resize trace showed a shorter per-configuration stall with DX12 than Vulkan, and the user confirmed the visible resize lag and temporary text scaling disappeared with DX12.

## Alternatives

- Native text widgets: inconsistent metrics and insufficient terminal-grid control.
- Electron/WebView, React, Qt, or another large GUI framework: outside product constraints and heavier than required.
- Direct platform GPU APIs: maximum control but duplicated cross-platform implementation.
- OpenGL: simpler surface area but less consistent backend strategy.

## Consequences

One native architecture can serve all target platforms with direct control over the terminal grid. GPU initialization, surface loss, adapter compatibility, DPI changes, and atlas behavior require explicit testing. Windows backend fallback preserves startup on hosts where DX12 initialization fails; visual resize behavior on those fallback backends may differ. Exact platform support floors will be derived from selected dependency versions, SDK requirements, and validation evidence as required by ADR-0008.
