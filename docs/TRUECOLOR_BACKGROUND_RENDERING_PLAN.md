# Truecolor and background rendering: goal and plan

Branch: `fix/truecolor-background-rendering`.
Status: implemented and validated. Both root causes, before/after comparisons, standard Rust checks, native acceptance, and remaining platform limits are recorded in [the report](TRUECOLOR_BACKGROUND_RENDERING.md). Do not commit or push.

## Goal

Investigate and fix color and background rendering inconsistencies between Terminal and Alacritty, particularly Neovim on KDE Wayland. Establish the root cause with controlled evidence before changing implementation. Apply the smallest standards-compliant fix in the responsible layer while preserving explicit RGB values, ANSI palette semantics, text, selection, cursor, UI, configurable opacity, KDE blur, native Bold, and DPI behavior.

## Observations to reproduce

- The same Neovim configuration reportedly produces unexpected row/cell background differences in Terminal and less vibrant text than Alacritty.
- Direct RGB backgrounds `#202020`, `#393939`, `#444444`, and `#78a9ff` reportedly render approximately correctly in both terminals.
- The largest reported differences involve default backgrounds, text appearance, and Neovim highlight regions.
- These are user observations, not yet independently reproduced. They do not establish a parser, color-space, alpha, or theme defect.

## Starting points verified in this checkout

- `crates/terminal-core/src/cell.rs` distinguishes default, indexed, and RGB colors and stores rendition attributes separately. `parser.rs` handles extended colors, default-color resets, reverse video, and erase dispatch; `state.rs` and `grid.rs` own resulting terminal state.
- `crates/renderer/src/snapshot.rs` resolves cell colors against the render theme, swaps foreground/background for inverse cells, applies background opacity, then selection colors, and omits some blank cells. Audit both semantic color identity and resolved values along this path.
- `crates/renderer/src/lib.rs`, `gpu.rs`, and `composition.wgsl` contain surface-format, clear, blending, transparency, and sRGB composition paths. Existing GPU tests include opaque rendering and background-opacity/sRGB premultiplication checks.
- Existing terminal-core tests include `truecolor.rs`, `xterm_256color.rs`, `rendition.rs`, and `ncurses_rendition.rs`. Extend relevant coverage rather than creating a parallel test framework.
- The requested branch is already checked out, and the worktree was clean before adding this plan.

## Plan

1. **Capture a reproducible, opaque baseline.** Record Terminal revision/build, Alacritty and Neovim versions, KDE/KWin/session, actual Wayland window backend, GPU/backend, display color settings, scale, font files/faces/size, palette, default foreground/background, and effective configuration. Preserve a pre-fix binary and exact reproduction inputs. Use the same Neovim configuration, plugins, file, working directory, viewport, actions, and highlight state in both terminals. Start with opacity `1.0`, blur disabled, and no compositor opacity overrides. Align default colors, ANSI palette, font, scale, and applicable bold/dim policies explicitly; record settings that cannot be matched. Retain both the original user configuration and a minimal controlled reproduction.

2. **Establish whether application output is equivalent.** Record `TERM`, `COLORTERM`, terminfo capabilities including background-color erase, startup environment, Neovim `termguicolors`/`background`, effective highlights and their sources, and any relevant terminal capability/default-color queries and replies. Capture PTY output and terminal replies for deterministic runs through existing infrastructure where practical. Account for capability negotiation and dynamic data when comparing streams; do not assume matching screenshots or configuration means identical bytes. Replay one captured output stream into both terminals at identical dimensions to isolate interpretation/rendering from application behavior. Compare the terminals' normal advertised environments first; use a common capability environment only as an additional controlled experiment. Distinguish GUI RGB highlights, indexed fallbacks, inherited/default backgrounds, reverse attributes, shell highlighting, and genuine terminal defects.

3. **Reduce differences to terminal semantics.** Build minimal byte fixtures for default foreground/background, SGR `0`, `39`, `49`, `7`/`27`, normal/bold/faint/reset transitions, ANSI 16/256 colors, and supported 24-bit encodings. Include the four reported RGB backgrounds and explicit RGB equal to the configured default. Compare colored spaces, untouched cells, end-of-line/display erasure, erase characters, insertion/deletion, scrolling, alternate-screen entry/exit, wide cells, and redraw/resize where relevant to the reproduction. Trace parser input through current rendition, stored cells, erase/fill attributes, and render projection; record attributes and resolved colors for specific mismatching coordinates. Check default-color queries/changes/resets if observed in the captured session. Keep terminal semantics in terminal-core. Reduce each confirmed failure to the smallest sequence and identify the first incorrect state transition or projection.

4. **Isolate GPU and compositor behavior.** If semantic state and projected colors are correct, trace byte RGB values through normalization, any transfer conversions, clear colors, rectangle/glyph colors, offscreen targets, shaders, blending, and final surface storage. Record actual surface formats and alpha modes rather than inferring them. Compare clear pixels with equivalent cell-background rectangles and pixels under glyphs; check background skipping, premultiplication, double blending, stale frame reuse, damage/redraw, and surface recreation. Use existing native GPU readback tests on supported sRGB and non-sRGB targets to localize differences. Separate flat interior RGB measurements from glyph coverage/edge appearance and from compositor screenshots. Use the opaque baseline to assess color conversion before introducing transparency. Consult primary protocol/API/reference sources for any disputed behavior before selecting a fix.

5. **Gate implementation on documented root cause.** Record the minimal reproduction, relevant bytes, expected standards-based behavior, observed cell/projection/GPU values, and the first failing layer. Alacritty is a comparison reference; configuration differences alone do not establish incorrect semantics. If the discrepancy is entirely configuration/theme/capability behavior, document that evidence and the matching settings instead of forcing a code change. Otherwise implement the smallest generic correction in the owning layer. Preserve explicit RGB and ANSI palette meanings; do not add Neovim-specific handling, palette compensation, brightness knobs, dependencies, or unrelated refactoring. Keep configuration separate from runtime state and platform behavior behind existing project-owned interfaces.

6. **Add focused regression coverage for confirmed bugs.** Demonstrate the failure before the fix and success afterward. Put parsing, default/reset, erase/fill, and stored-attribute tests in terminal-core; put resolved-color, inverse, selection, blank/wide-cell projection tests in the renderer where appropriate. Add GPU readback coverage only where needed to prove the confirmed rendering/composition defect. Assert independent expected values rather than repeating implementation logic. Cover unchanged explicit RGB/ANSI behavior and the implicated interaction with opacity, text, or overlays. Reuse existing native Bold/font and DPI checks where the changed path could affect them.

7. **Validate the corrected behavior on KDE Wayland.** Repeat identical before/after inputs in the preserved baseline binary, candidate, and Alacritty. First compare opaque windows with blur off. Then compare matched transparent settings (including `0.85`, with `0.0` as an edge case where useful), first with blur off and then on where supported. Record each terminal's effective transparency policy for default versus explicit cell backgrounds; do not force undocumented policy equivalence. Use a fixed backdrop/window position for transparency captures. Exercise Neovim highlight regions, Vim, shell highlighting, ANSI 16/256 swatches, RGB swatches, default/reset/reverse/erase fixtures, native Bold, selection, cursor styles, search, palette, tabs/splits/padding, resize, redraw, and surface lifecycle. Check 100%, 125%, and 200% scale where available. Compare flat region pixels quantitatively and inspect text edges/readability separately; do not promise identical font rasterization across terminal implementations. Retain paired screenshots, replay inputs, settings, and logs.

8. **Run standard Rust validation and report.** Run focused regressions, existing affected native GPU/application tests, and the standard commands below. Record ignored native tests separately; a default workspace run does not establish KDE acceptance. Update this plan's status and write `docs/TRUECOLOR_BACKGROUND_RENDERING.md` with root cause and evidence, changed code, regression results, before/after comparisons, and remaining limitations. Store reproducible comparison inputs and compact evidence under `docs/measurements/truecolor-background-rendering/` as useful; keep large temporary captures/builds outside the repository. Update configuration documentation only if the established behavior contract changes. Leave changes uncommitted and unpushed.

   ```sh
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   cargo test --workspace --all-targets
   cargo test --workspace --doc
   cargo clippy --workspace --all-targets -- -D warnings
   cargo build --release -p terminal-app
   git diff --check
   ```

## Acceptance gates

- Each reported discrepancy has a reproducible explanation: confirmed defect, configuration/capability difference, or clearly documented unresolved limitation.
- Every implementation change follows a demonstrated root cause, preserves architectural ownership, and has focused failing-before/passing-after evidence.
- Explicit RGB and ANSI palette meanings remain correct; default/reset/reverse/erase behavior is standards-consistent.
- Opaque and transparent KDE Wayland comparisons demonstrate the corrected behavior without regressions in text, selection, cursor, UI, opacity, blur, native Bold, or tested DPI scales.
- Standard Rust checks pass, with unavailable/failed/ignored coverage reported explicitly. Windows, macOS, X11, other GPUs, and unavailable display scales remain unverified unless exercised directly.
- The final report separates original observations, measured findings, implementation, and remaining limits. No commit or push.
