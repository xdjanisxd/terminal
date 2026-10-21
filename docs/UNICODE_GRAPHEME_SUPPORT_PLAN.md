# Unicode grapheme support plan

Base: `e042ac631523e65c54b11432f335a9d00594d5ca` (remote master verified
2026-10-21). The task branch already existed at that commit with a clean tree.

## Findings

- Core prints scalars using `UnicodeWidthChar`; a cell stores a base and eight
  inline `char` attachments. Only width-zero scalars attach, so modifiers,
  flags, and positive-width ZWJ components become separate terminal cells.
- The incremental VTE parser already retains split UTF-8. Core must retain
  grapheme continuity independently of PTY chunk boundaries.
- Wide lead/continuation cells, selection, history and reflow already transport
  complete inline cell payloads. Preserve that ownership and bounded storage.
- Renderer projection assembles the payload, but font fallback chooses a face
  per scalar and can split a cluster before Swash shapes it. GPU placement also
  needs to respect advances if a cluster produces multiple glyphs.

## Implementation

1. Record release core, projection and GPU workload baselines before changes.
2. Reuse inline cell attachments for general grapheme tails without increasing
   `Cell` size or introducing per-cell heap allocations. Keep the existing
   nine-scalar total bound and expose accurately named accessors alongside the
   legacy combining-mark accessor.
3. Use `unicode-segmentation` (already in Cargo.lock) for extended grapheme
   boundaries and the existing `unicode-width` string implementation for cluster
   width. Keep ASCII away from segmentation and stack-buffer UTF-8 conversion.
4. Append incrementally before delayed wrap. Correct width changes, insertion,
   right-margin relocation, single-column terminals, cursor and continuation
   invariants. End positive-width joining after explicit terminal edits/movement.
5. Select fallback faces for whole graphemes, then retain existing Swash shaping,
   bounded caches and color glyph rasterization. Preserve relative glyph placement.
6. Cover representative sequences, every byte split, errors/overflow, edits,
   widths/margins, selection/copy, scrolling/reflow/history eviction and rendering.
7. Compare identical release workloads, investigate material regressions, then
   run formatting, workspace check/tests, Clippy and release build. Run opt-in
   font/GPU checks where available; distinguish software GPU from native display.
8. Document bounds, unsupported sequences, benchmark evidence, roadmap status
   and Linux/Windows manual acceptance. Do not commit or push.

## Acceptance boundaries

Clusters beyond the fixed payload limit and clusters whose width cannot be
represented in one or two terminal columns receive explicit bounded errors.
No normalization, bidi paragraph layout or cross-cell script shaping is added.
Font availability remains a rendering prerequisite; native OS acceptance must
be reported separately from offscreen tests.
