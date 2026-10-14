# Configurable bold intensity

Implemented on `feat/configurable-bold-intensity`. No commit or push.

## Result

Optional `[font].bold_intensity` accepts finite values in `0.75..=1.25`, default
`1.0`. Omission and `1.0` retain the existing native Bold selection, outlining,
and missing-Bold fallback. Font configuration remains restart-required; renderer
recreation uses the latest valid configuration, like font family and size.

Nondefault values control actual font weight through the existing fontdb/Swash
stack. Variable `wght` axes receive a clamped target in both shaping and
rasterization. Static families select the nearest compatible installed native
Semibold-or-heavier face. There is no increased synthetic emboldening, bitmap
dilation, repeated drawing, or alpha adjustment. Unsupported fonts retain their
existing rendering and issue a bounded diagnostic. See [configuration details](CONFIG.md#font)
for the reference weight, ties, clamping, and limitations.

## Capability and implementation findings

- Swash 0.2.10 exposes axis bounds and normalization, plus variation coordinates
  on both shaping and scaling builders. The licensed JetBrains Mono variable
  fixture has `wght` 100–800, default 400. Tests demonstrated distinct native
  outlines and stable monospace advances before adding configuration.
- Installed JetBrainsMono Nerd Font Mono has real 600/700/800 faces. Native
  tests select Semibold/Bold/ExtraBold for `0.75`/`1.0`/`1.25`, and verify those
  outlines receive no extra synthesis. Metadata fixtures independently check
  compatibility, invalid data, tie behavior, and missing-character fallback.
- The native Bold metadata weight is the reference. A variable family without
  a separate native Bold face uses reference 700 for nondefault values while
  `1.0` preserves its existing fallback. This case need not transition
  continuously through `1.0`; static families also change discretely.
- A shaped glyph carries its effective weight. CPU bitmap and GPU atlas keys
  distinguish weight instances as well as face, glyph, physical size, and bold
  state. Intensity is fixed for each renderer. Consuming font initialization
  clears relevant CPU caches; renderer recreation creates fresh GPU resources.
  Primary-face metrics and baseline remain independent of the bold request.
- Swash scaler contexts retain variation coordinates. Passing an empty
  `variations` iterator left a previous bold coordinate active for subsequently
  rasterized regular glyphs. The implementation supplies explicit normalized
  coordinates for every run and bitmap, including regular defaults. A focused
  regression reproduces the incorrect regular bitmap before the fix and checks
  it against fresh rasterization afterward. Both builders use the same
  normalization and default values for unrelated axes.
- A requested static weight without a particular character falls back to the
  original native Bold companion before regular synthesis. It remains marked
  as a primary-family face rather than an unrelated Unicode fallback.

## Verification

The user subsequently instructed that no testing be performed for this task.
No further tests or validation commands are being run. The results below describe
work completed before that instruction; documentation/evidence assembly does not
represent another validation run.

The seven standard gates passed: formatting, workspace/all-targets check,
workspace/all-targets tests, workspace doctests, Clippy with `-D warnings`,
release terminal-app build, and `git diff --check`. Commands and exit codes are
recorded in [rust-gates.json](measurements/configurable-bold-intensity/rust-gates.json).
Ordinary tests cover validated configuration, defaults and example parity,
native/default bitmap equivalence, unsupported fonts, variable weight effects
and clamping, regular/Bold/reset behavior, shaping and bitmap cache separation,
missing-character coverage, metrics, baselines, and multiple sizes/scales.

Explicit native checks passed:

| Check | Evidence |
| --- | --- |
| Installed native 600/700/800 selection, grid, Unicode and no synthesis | [native font log](measurements/configurable-bold-intensity/static-native.log) |
| Variable instances occupy distinct GPU atlas slots and retain their entries | [GPU log](measurements/configurable-bold-intensity/gpu.log) |
| Real CJK fallback bitmaps remain nonblank after preceding glyph workloads, at 16/20/32 physical pixels and all three intensities | [fallback log](measurements/configurable-bold-intensity/variable-fallback.log) |
| Live config reload retains the active intensity; invalid input retains the last valid config; size increase/reset preserves metrics; recreation uses the new intensity | [KDE Wayland lifecycle log](measurements/configurable-bold-intensity/lifecycle.log) |
| Same running window traverses 1x → 1.25x → 2x → 1x for both static and variable weights; return images are identical | [comparison](measurements/configurable-bold-intensity/dpi-comparison.txt), [static log](measurements/configurable-bold-intensity/dpi-static.log), [variable log](measurements/configurable-bold-intensity/dpi-variable.log) |

The KDE Wayland matrix includes 16 logical pixels on dark backgrounds at
100%, 125%, and 200%; 12 logical pixels on light backgrounds at 100%; and 20
logical pixels on light backgrounds at 200%. Each supported family is captured
at `0.75`, `1.0`, and `1.25`. Samples include counters, accents/combining marks,
box drawing, CJK fallback, Nerd Font symbols, ANSI/indexed/RGB colors, and
normal/Bold/reset rows. Counters remain open and baseline/grid placement stays
consistent in the inspected captures. Existing glyph clipping remains governed
by the cell grid; intensity adds no outline distortion or layout expansion.

Paired original-HEAD and candidate screenshots are entirely pixel-identical for
omission and explicit `1.0` at all three scales with the native static family.
The variable family additionally matches the original at 125%. For nondefault
values, regular/reset/regular-color rows are identical while the Bold row
changes, on both dark and light matrices. Regular-only Noto Mono is entirely
unchanged at all three intensities. See [pixel comparisons](measurements/configurable-bold-intensity/pixel-comparison.txt).

Capture targeting was checked separately from rendering. Early captures affected
by focus, popup overlays, or fixture lifetime were rejected and replaced. The
fractional variable-font baseline was rechecked with a live window-only capture;
its CJK output matches the original. Saved accepted evidence includes no other
application content. Both KDE displays were restored to their original 100% scale.

## Visual evidence

Each comparison sheet contains `0.75`, `1.0`, then `1.25` from top to bottom.
The variable-font 125% row links the three accepted captures separately.
Each panel contains regular, Bold, then reset text. Full accepted source images,
logs, scripts, and font/source hashes are retained under
[measurements/configurable-bold-intensity](measurements/configurable-bold-intensity/).

| Size / scale / background | Static native faces | Variable weight axis |
| --- | --- | --- |
| 16px / 100% / dark | [comparison](measurements/configurable-bold-intensity/1-static-16-dark.png) | [comparison](measurements/configurable-bold-intensity/1-variable-16-dark.png) |
| 16px / 125% / dark | [comparison](measurements/configurable-bold-intensity/1.25-static-16-dark.png) | [0.75](measurements/configurable-bold-intensity/1.25-variable-16-dark-0.75.png), [1.0](measurements/configurable-bold-intensity/1.25-variable-16-dark-1.0.png), [1.25](measurements/configurable-bold-intensity/1.25-variable-16-dark-1.25.png) |
| 16px / 200% / dark | [comparison](measurements/configurable-bold-intensity/2-static-16-dark.png) | [comparison](measurements/configurable-bold-intensity/2-variable-16-dark.png) |
| 12px / 100% / light | [comparison](measurements/configurable-bold-intensity/1-static-12-light.png) | [comparison](measurements/configurable-bold-intensity/1-variable-12-light.png) |
| 20px / 200% / light | [comparison](measurements/configurable-bold-intensity/2-static-20-light.png) | [comparison](measurements/configurable-bold-intensity/2-variable-20-light.png) |

Baseline revision: `33ce5703040bd9025dd74d875a3f5c0ce2a5d6bf`. The baseline was
built from an archive of HEAD; candidate and baseline binaries were kept
separately. Harness sources retain their `/tmp/bold-intensity-evidence` paths
as validation provenance, not a supported application interface. Reproduction
requires the baseline/candidate binaries, installed native fonts, KWin,
Spectacle, ImageMagick, and the licensed variable fixture through fontconfig.

## Limitations

Visual/runtime acceptance covers this Linux KDE Wayland host and the recorded
fonts, sizes, and scales. Windows, macOS, other compositors, arbitrary fonts,
and custom/named variation instances were not visually validated. The shared
Rust path introduces no platform-specific weight implementation or dependency.
Fonts without sufficient native weights may ignore an intensity change; axis
clamping and static selection do not promise uniform perceived thickness across
families. Actual weight-dependent advances and overhangs are font-defined;
terminal cell metrics and baseline remain fixed to the regular primary face.
