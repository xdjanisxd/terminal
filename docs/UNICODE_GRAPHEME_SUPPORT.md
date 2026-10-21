# Unicode grapheme support

Core now stores bounded extended grapheme clusters as one printable lead cell
and, for two-column text, one metadata-only continuation. Combining marks,
VS15/VS16 presentation, emoji modifiers, regional-indicator pairs, keycaps and
ZWJ sequences stay together. Examples include `é`, `⌚︎`, `✈️`, `👍🏽`, `🇺🇸`,
`1️⃣`, `👩‍💻` and `👨‍👩‍👧‍👦`.

## Semantics and architecture

`terminal-core` uses `unicode-segmentation` 1.13.3 for extended boundaries and
the existing `unicode-width` 0.2.2 string rules for one/two-column widths. Both
currently ship Unicode 17 tables. Segmentation follows
[UAX #29](https://www.unicode.org/reports/tr29/tr29-47.html); terminal widths
follow the [width library's documented rules](https://docs.rs/unicode-width/0.2.2/unicode_width/).
No hand-maintained emoji boundary table or new transitive package is added.

Each incoming scalar can extend the last printable cluster before delayed wrap
is resolved. VTE continues to own partial UTF-8 decoding. A PTY chunk boundary
does not finish a grapheme; no flush timeout, pending string or parser buffer
growth is introduced. Incomplete valid input remains immediately displayable
and later components update that cell. SGR changes retain the base cell's
attributes and hyperlink. Explicit cursor movement, edits, screen switches,
mode changes affecting placement and changed-size resize end positive-width joining. Legacy
width-zero attachment to a preceding cell after movement is retained when it
does not change that cell's footprint.

Cluster width can grow or shrink as presentation/modifier components arrive.
Core updates cursor position, delayed wrap and wide occupancy together; insert
mode shifts the extra columns. A growing cluster at the right edge moves intact
to the next line with autowrap. With autowrap disabled, impossible growth returns
`UnsupportedCharacter` and preserves the previous cell. Shrinking does not
rewind an already performed wrap or restore text overwritten by the wider
prefix. One-column terminals retain the existing squeezed-wide representation
so widening later restores the complete cluster.

Column-based ICH/DCH/ECH, erasure and cursor movement keep their existing terminal
semantics. They do not become grapheme-counted editing commands. Intersecting
wide pairs are repaired centrally. Cells transport their complete payload
through scrolling, selection/copy, history eviction and Primary reflow. App
search also consumes the complete payload and maps matches to whole cell spans.

## Rendering and bounds

Projection passes the entire cluster to the renderer. Fallback chooses one font
for a whole grapheme, including all visible components. Explicit emoji
presentation prefers a covering color face when installed. If no face covers
the whole grapheme, the base face shapes it intact; missing glyphs can remain.
Swash handles script shaping, offsets and substitutions. Glyph placement now
uses advances as well as offsets, and multiple color glyphs share the terminal
span proportionally instead of being centered on top of each other.

Swash's nominal charmap does not resolve cmap format 14. After presentation-based
face selection, selectors are excluded from glyph substitution inputs while
original source ranges are retained. This prevents a missing-selector glyph
from interrupting keycap ligatures. Exact font-specific variation glyph lookup
is intentionally unsupported. Existing color formats, intrinsic colors,
opacity/composition and atlas limits remain as documented in
[emoji rendering](EMOJI_FONT_RENDERING.md).

- `Cell` remains `Copy` and **72 bytes** on this x86_64 build. The existing eight
  inline `char` slots now retain general tails: **nine scalars total**, including
  the base. No per-cell core allocation or global grapheme interning is added.
- `TerminalState` grows **1,256 → 1,280 bytes** here: one optional live anchor and
  an ASCII fast-path flag. UTF-8 segmentation scratch space is 40 stack bytes.
- Grid limits and 10,000-row history eviction remain unchanged. Oldest rows
  release their payload directly; there is no separately retained cluster store.
- The shape cache stays at 512 entries, CPU glyph cache at 256, and GPU atlas at
  256 slots. New fallback memoization is capped at 512 short grapheme keys
  (at most 18 KiB of key bytes, plus map overhead). The existing scalar fallback
  map is now capped at 512 too. These caches evict rather than growing with input.
- Ordinary ASCII skips segmentation and UTF-8 scratch conversion. Renderer
  scalar projection remains inline; multi-scalar projection uses the existing
  bounded-cell `String` path. A counting-allocator regression verifies core ASCII
  and representative cluster printing perform zero per-cell allocations.

## Intentional limits

- More than nine scalars in one cluster return the existing
  `PrintError::CombiningMarkOverflow` without changing cell/cursor/grid. This
  includes some valid long RGI emoji, such as certain couples with modifiers.
  Parser consumed-byte/resume handling remains unchanged; later output recovers.
- Cluster widths outside one/two columns receive an explicit unsupported-width
  error. Standalone zero-width input has no independent printable cell; leading
  zero-width prepend characters are not buffered for a later base.
- Conflicting/duplicate presentation-selector chains, malformed emoji and
  arbitrary non-RGI ZWJ ligatures have no promised single-glyph rendering.
  Fonts and the pinned segmentation/width/shaping tables determine their output.
- No normalization, bidi paragraph layout or contextual shaping across terminal
  cells is introduced. Font-specific ideographic/other variation glyphs require
  future cmap format 14 support. Font coverage is required for visible output.
- Joining is not continued across an intervening resize/edit/cursor operation.
  Already stored cluster payloads remain intact under the existing Primary reflow
  and Alternate rectangular-resize policies.

## Validation and measurements

See [raw logs and benchmark comparison](measurements/unicode-grapheme-support/README.md).
Coverage includes 15 focused core regressions, all-byte split fixtures, width
growth/shrink and margins, insert/replace edits, SGR, malformed UTF-8/resume,
screen switching, selection/copy, one-column input/reflow, bounded history
eviction, allocation counting, search, projection and deterministic fallback.

Installed Noto Color Emoji shaping tests verify complete sequence source ranges,
one nonzero color glyph and regular/bold cache reuse. Offscreen GPU acceptance
checks linear/sRGB targets, opacity 1/0.5 and 1×/1.25×/2× DPI, visible ink,
two-column clipping, intrinsic-color tint invariance and selection. All 100
unchanged frames per scenario require zero shape misses, raster calls, buffer
allocations and buffer writes. The GPU here is Mesa **llvmpipe**, not hardware.
The installed Noto family artwork is intentionally grayscale; RGBA/tint/ink
assertions cover it without incorrectly requiring chroma.

## Linux and Windows manual acceptance checklist

These interactive checks remain **pending** on both platforms. Offscreen tests
do not establish compositor, hardware GPU, clipboard or physical-key behavior.

1. Linux: use Bash/Zsh on KDE Wayland and an installed emoji font. Windows: use
   PowerShell/ConPTY with a covering emoji font. Record font names and GPU/backend.
2. Print the sample below using `python3` on Linux or `py -3` on Windows. Check
   combined accents, text/emoji presentation, flags, skin tones, keycaps,
   professions and families. Check following `|` characters align with the
   cluster's one/two-column footprint; font fallback must not split a sequence.
3. Place the same samples at the last two columns, with autowrap enabled/disabled
   and in insert mode. Use shell/editor backspace, delete, progress redraws and
   cursor positioning. Check no orphan continuations or shifted following text.
4. Drag and keyboard-select either half of a wide emoji, copy and paste into a
   Unicode-aware editor, and compare exact scalars. Test search for a full ZWJ
   sequence and an accented cluster. Repeat in scrollback and after eviction.
5. Narrow/widen through a one-column viewport and repeated soft wraps, scroll,
   switch alternate-screen TUIs, and return to the shell. Check payloads and
   selected text survive according to the documented screen resize policies.
6. Check 100%, 125% and 200% scaling, monitor transitions, dark/light themes,
   opacity 1/0.5, selection and bold/italic. Verify unchanged panes remain idle,
   warm frames do not churn glyph resources, and intrinsic emoji colors persist.
7. Repeat representative redraw/copy cases in Vim/Neovim and a multiplexer;
   record application versions and width disagreements rather than compensating
   for application-specific policies in the renderer.

```python
import sys
samples = ["e\u0301", "⌚\ufe0e", "✈\ufe0f", "👍🏽", "🇺🇸", "1️⃣", "👩‍💻", "👨‍👩‍👧‍👦"]
for sample in samples:
    sys.stdout.write(sample + "|" + "\r\n")
sys.stdout.flush()
```
