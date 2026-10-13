# Native Bold font faces: goal and plan

Branch: `feat/native-bold-font`. Status: implemented and validated on native KDE Wayland; see [findings and evidence](NATIVE_BOLD_FONT.md). Do not commit or push.

## Goal

Replace synthesized SGR 1 glyph outlines with matching native Bold font faces whenever available. Make JetBrainsMono Nerd Font Mono bold text visibly balanced on KDE Wayland while preserving regular text, foreground colors, ANSI semantics, attributes, Unicode coverage, baseline, cell spacing, and DPI behavior. Keep a restrained fallback for families or characters without a usable native Bold face, with correct cache separation and invalidation.

## Findings from initial inspection

- `crates/renderer/src/font.rs` owns fontdb discovery, primary-face selection, Swash shaping/rasterization, character fallback, and bounded CPU caches. Linux system-monospace resolution additionally uses fontconfig through `fc-match`; explicitly configured families use fontdb.
- Shaping currently resolves regular/fallback faces without a weight parameter. Its cache is keyed by text, with a current pixels-per-em epoch. Native weight must participate in shaping and its cache key; glyph IDs cannot safely be transferred between faces.
- Rasterization currently applies `Render::embolden(pixels_per_em / 24.0)` when bold is requested. CPU glyph keys already include face ID, glyph ID, physical size, and bold.
- `crates/renderer/src/gpu.rs` shapes each rendered cell, then uses a GPU atlas key containing face identity, glyph ID, physical size, and bold. The actual selected face and synthesis mode must remain represented consistently through shaping, rasterization, and atlas lookup.
- Cell metrics derive from the primary regular face. `Renderer::set_scale_factor` and `set_font_size` update those metrics. Trace renderer/font replacement and retained draw resources before deciding whether additional invalidation is needed.
- `snapshot.rs` projects bold independently of foreground color. Existing font and GPU tests cover synthesized weight separation and bold/reset color behavior; adapt or extend them for native faces rather than assuming heavier pixel coverage always proves correctness.

## Plan

1. **Finish the architecture and baseline audit.** Trace font selection, fallback runs, shaping, CPU caches, atlas keys, frame reuse, font replacement, size changes, and DPI transitions. Check all consumers of shaping, including UI text. Capture comparable regular/bold samples from the current release build using JetBrainsMono Nerd Font Mono, with font files, size, scale, theme, and window geometry recorded. Keep baseline and candidate artifacts identifiable.

2. **Resolve matching native faces using the existing font stack.** Resolve Bold from the actual regular face's family metadata using fontdb weight selection, preserving style/stretch and monospace suitability for the primary terminal family. Validate the returned family, weight, and usable font data instead of accepting a query's nearest regular substitute as native Bold. Handle already-bold primary selections without applying extra weight. Inspect installed JetBrainsMono Nerd Font Mono naming and collection indices. Use existing discovery and platform interfaces; avoid dependencies and Linux-only selection rules in the shared path. Evaluate variable-font weight support explicitly and record any unsupported case.

3. **Select weight before shaping.** Thread the requested bold state through character resolution and shaping. Shape with the chosen native face, retaining its own glyph IDs, cluster ranges, and offsets. If that face lacks a character, preserve existing Unicode coverage: try an appropriate native Bold companion for the resolved regular fallback, then use that regular face with the fallback policy. Keep fallback selection deterministic and cache it by the information that actually changes the result. Keep primary native Bold distinct from Unicode fallback in glyph metadata. Continue positioning all glyphs against the regular-face grid and baseline.

4. **Apply a restrained synthesis fallback.** Rasterize native Bold outlines with no emboldening. When no usable native counterpart exists, synthesize a smaller, size-aware weight on the resolved regular outline; select and document the strength from paired captures at normal and fractional/high DPI. Preserve cell geometry and shaping advances, and avoid additional synthesis on a face already carrying bold weight. Inspect thin strokes, counters, box drawing, Nerd Font symbols, combining marks, and fallback Unicode before accepting the policy.

5. **Separate caches and verify lifecycle behavior.** Include weight in shaping and character/face-resolution caches as needed. Ensure CPU bitmap and GPU atlas keys identify the selected face, glyph, physical size, and synthesis mode, so regular, native Bold, and synthesized fallback cannot collide. Preserve bounded cache behavior. Clear or replace relevant caches on font changes, including atlas/frame state if a database-local face ID can be reused. Verify DPI/size transitions use the correct physical-size entries and redraw even when rounded cell dimensions coincide; returning to a previous scale may reuse only entries valid for that same font and rendering mode.

6. **Add focused correctness tests.** Use deterministic regular/Bold fixtures with suitable redistribution licensing, preferably existing project resources where practical. Test exact native-face selection, family mismatch/no-Bold fallback, already-bold faces, native glyph IDs, missing characters and Unicode fallback, regular/bold/regular cache reuse, atlas separation, eviction, and font/size/DPI transitions. Check primary cell metrics and baseline remain unchanged by SGR 1. Preserve existing snapshot and native GPU coverage for foreground colors, SGR 22/reset, indexed/RGB colors, inverse, faint, underline, and other attributes. Test the shared selection path without relying on this host's installed fonts.

7. **Perform KDE Wayland visual acceptance.** Compare baseline/candidate regular and Bold samples side by side using JetBrainsMono Nerd Font Mono under identical settings. Include ASCII, punctuation, counters, accents/combining sequences, wide Unicode, Nerd Font symbols, and fallback characters. Inspect baseline/alignment, line wrapping, clipping, cell spacing, and regular/bold/reset alternation. Exercise 1x and available fractional/high-DPI settings plus live size/DPI transitions. Test Zsh syntax highlighting, MTR, Vim, and Neovim in the native application, including normal/alternate-screen use, colors, redraw, and return to the shell. Capture task-specific screenshots/logs; record each application separately. Confirm native face use from face metadata and visually judge whether excessive thickness is resolved. Exercise a family without Bold as a separate fallback sample.

8. **Run standard Rust validation and report.** Run focused renderer/core/app tests first, then `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo test --workspace --doc`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --release`, and `git diff --check`, adjusting to any repository-specific gates discovered during the audit. Run relevant ignored native GPU tests explicitly and distinguish them from normal test results. Review shared code for Windows/macOS regressions; claim runtime coverage only for platforms actually exercised. Update rendering documentation and produce a findings report with implemented selection/caching behavior, fallback strength, exact visual results, evidence locations, and unresolved limitations. Do not commit or push.

## Acceptance gates

- SGR 1 uses the matching usable native Bold face, without synthesized extra weight; absent faces/characters receive the documented restrained fallback.
- Regular/Bold/reset and font/DPI changes never reuse an incorrect shaping result, bitmap, atlas entry, or frame.
- Regular text, grid metrics, baseline anchoring, Unicode coverage, ANSI foreground colors, and terminal attributes remain correct.
- KDE Wayland evidence covers JetBrainsMono Nerd Font Mono and each requested application, and shows balanced bold thickness under the tested settings.
- Focused and standard Rust checks pass. Missing native/platform coverage and variable-font limitations are reported explicitly.

## Expected scope

Primary code: `crates/renderer/src/font.rs` and `crates/renderer/src/gpu.rs`. Lifecycle integration in `crates/renderer/src/lib.rs` only where the audit shows it is necessary. Focused fixtures/tests and task-specific native evidence accompany the change. Terminal semantics remain owned by `terminal-core`; configuration and platform architecture stay in their existing owners.
