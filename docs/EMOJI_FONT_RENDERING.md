# Emoji font rendering

## Investigation and root cause

The task branch `fix/emoji-font-rendering` was already present with a clean
worktree at `61033ae`. Fetching `origin master` confirmed that HEAD, master,
and origin/master all identified that commit. No unrelated changes existed.

The investigation traced discovery, fallback, shaping, CPU caching,
rasterization, atlas upload, and fragment output before implementation:

- `fontdb` discovers the installed Noto Color Emoji face. The retained database
  and character fallback cache select it for 🚀 when the primary font lacks it.
- Swash maps 🚀 to glyph 963 on the tested Noto installation. Shaping succeeds.
- The old rasterizer requested only `Source::Outline`. Noto Color Emoji uses
  CBDT/CBLC color bitmaps, so rasterization returned `GlyphNotRasterizable(963)`.
  The draw loop discarded the error and emitted no quad. This reproduces
  independently of window resizing.
- The old CPU bitmap contract, R8 atlas, and fragment shader only carried
  alpha coverage. Enabling bitmap rasterization alone could not preserve color.
- Cache identities already include face, effective weight, glyph, physical
  size, bold, and italic. They did not need a new key or terminal-core changes.

The installed-font regression was run before the implementation and failed
at rasterization with the Noto face selected. It passes with the fix.

## Implementation

Swash now tries color outlines (palette zero), best-fit color bitmaps, outlines,
and alpha bitmaps. The existing scaler resamples bitmap strikes to the requested
physical font size. CPU bitmaps distinguish single-byte masks from straight-alpha
sRGB RGBA. Swash COLR layer output is unpremultiplied before storage; decoded
color PNG bitmaps already provide straight alpha.

The shared bounded atlas stores RGBA8 instead of R8. Monochrome CPU bitmaps stay
single-channel and expand only on atlas insertion; their coverage, foreground
tint, shaping, bearings, baseline, and cache identity remain unchanged. The
shader uses intrinsic RGB for color glyphs and decodes sRGB when the target
requires linear-light blending. Selection changes the cell background while
preserving intrinsic color. Conceal still suppresses glyph drawing.

Color ink is proportionally fitted and centered within its terminal-owned cell
span. Neither fallback advances nor bitmap-strike metrics determine terminal
columns. Terminal-core continues to own Unicode widths, selection text, history,
and reflow. There are no runtime dependencies or configuration additions.

## Validation

- `cargo test --workspace --all-targets --offline`: 780 passed, 25 ignored.
  This also executes the existing benchmark binaries in the test profile.
- `cargo test --workspace --doc --offline`: 4 passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` passed. The renderer
  suite passed again after the final COLR normalization cleanup.
- New deterministic COLRv0 coverage builds a colored layer from the existing
  licensed Tuffy fixture in memory. It verifies RGBA content, repeated-cache
  results, size separation, and byte-identical neighboring monochrome glyphs.
- New projection/selection and history-reflow regressions verify two-column
  emoji occupancy, continuation-cell selection, following-character alignment,
  and text preservation across narrower and wider viewports.
- The ignored installed-Noto regression verifies actual color bitmap decoding
  at 16/20/32/48 physical pixels, cached style separation, unchanged bitmap
  output for synthetic bold/italic, and unchanged primary cell metrics.
- Six ignored GPU regressions passed on both llvmpipe Vulkan and Intel Iris Xe
  Vulkan. They cover existing monochrome text/styles, underline/cursor output,
  palettes, URL hints, opacity, sRGB composition, and the new emoji path.
  The variable-weight atlas fixture test was excluded from this GPU run.
- Emoji GPU readback covers 1x/1.25x/2x physical sizing, RGBA unorm and sRGB
  targets, and opacity 1.0/0.5. It checks visible chromatic pixels and bounded
  two-cell geometry. The strengthened test also compares emoji pixels after
  changing foreground tint; it passed on llvmpipe.
- A release build was opened on KDE Wayland using Intel Iris Xe Vulkan,
  `Bgra8UnormSrgb`, scale 1, and JetBrainsMono Nerd Font Mono at size 20.
  The [native capture](measurements/emoji-font-rendering/linux-wayland.png)
  shows visible 🚀/🌍/🎉, two-cell alignment, Latin/CJK/combining text, and
  regular/bold/italic rows. The process exited successfully.

Run the optional font/GPU tests explicitly on a host with the required fonts:

```sh
cargo test -p terminal-renderer native_color_emoji_rasterization -- --ignored --nocapture
cargo test -p terminal-renderer native_gpu_color_emoji_dpi_and_warm_cache -- --ignored --nocapture
```

`--ignored` runs tests requiring installed fonts or GPU access; `--nocapture`
shows the adapter identity and warm-frame timing output.

## Performance and storage

Atlas dimensions and capacity remain 1024x1024 and 256 slots. GPU storage grows
from 1 MiB to 4 MiB. Each glyph instance grows from 48 to 52 bytes. Masks retain
their original CPU cache footprint; color entries use four bytes per pixel.

Across all twelve emoji GPU scenarios, each of 100 repeated identical frames
performed zero rasterizations, shaping misses, buffer allocations, or instance
buffer writes. Timing measures CPU preparation/submission, not synchronized
display latency. This verifies warm-cache behavior rather than a frame-rate
claim.

The existing release projection benchmark (500 operations per scenario) reported:

| Scenario | Before, microseconds/op | After, microseconds/op |
| --- | ---: | ---: |
| Visible projection 80x30 | 44.87 | 40.30 |
| Output echo plus projection | 36.31 | 35.77 |
| Three-pane layout plus projection | 45.55 | 45.98 |

These single-run smoke measurements show no material projection regression;
they do not establish a statistically significant speed improvement or measure
cold emoji decoding and GPU bandwidth under atlas churn.

## Limits

- Primary-font precedence and deterministic fallback ordering are unchanged.
  For example, 😀 was monochrome in the native capture because the primary
  font already covers it. Emoji presentation selectors do not force a switch
  to a color face.
- This enables the color formats understood by the current Swash version,
  including Noto CBDT/CBLC and COLRv0. It does not add SVG or COLRv1 support.
- Terminal grapheme/ZWJ, variation-selector, flag, and skin-tone sequence
  handling is unchanged. This is not a complete emoji-sequence implementation.
- The existing atlas rejects bitmaps larger than 64x64 physical pixels. Very
  large font sizes or DPI combinations can still exceed that limit for color
  and monochrome glyphs. The tested 1x/1.25x/2x sizes fit.
- Bitmap strikes retain their artwork under bold/italic instead of receiving
  synthetic outline transformations. Intrinsic color is not foreground-tinted.
- Windows/macOS, native fractional-DPI monitor transitions, interactive mouse
  copy/paste, and editor-specific emoji behavior were not manually exercised.

## Linux manual acceptance checklist

The native capture covers the first visual checks at scale 1. Interactive steps
and monitor transitions below remain manual acceptance work:

1. Confirm `fc-match 'Noto Color Emoji'` resolves to the installed font. Restart
   the terminal after installing fonts; discovery occurs at startup.
2. Run `printf 'A🚀B | A🌍B | A🎉B\n|01|23|45|\n|🚀|🌍|🎉|\n'` before resizing.
   Check visible colors, two-column spans, and aligned following characters.
3. Compare regular, bold, italic, ANSI-colored and selected emoji alongside
   ASCII, CJK and combining accents. Emoji should retain intrinsic color;
   neighboring monochrome text should retain its configured rendition.
4. Drag-select across both halves of an emoji and copy/paste it. Verify one
   character is copied and that selection backgrounds cover the entire span.
5. Print enough emoji/text to enter scrollback, narrow and widen the window,
   then inspect history and copy the text. Check for missing/duplicated glyphs,
   orphan continuations, and shifted following characters.
6. Exercise font-size shortcuts and move between 1x, fractional and 2x monitors.
   Check stable cell alignment, refreshed glyphs, and no clipping at tested
   sizes. Repeat with transparency enabled and in Vim/Neovim.
