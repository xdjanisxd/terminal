# Configurable bold intensity: goal and plan

Branch: `feat/configurable-bold-intensity`.
Status: implemented. Goal and plan completed; recorded standard and KDE Wayland evidence was collected before the user instructed that no further testing be performed. Testing and validation are now stopped. Do not commit or push.

## Goal

Provide optional control over actual bold font weight while preserving native Bold rendering at the default, regular text, Unicode coverage, terminal semantics, cell metrics, baseline alignment, DPI behavior, and cache correctness. Ship a setting only after demonstrating meaningful weight control through supported font APIs. Do not implement intensity by amplifying synthetic emboldening or modifying bitmap coverage.

## Verified starting points

- `crates/renderer/src/font.rs` selects same-family native Bold companions with style/stretch and coverage checks, shapes using the selected face, and rasterizes native Bold without extra emboldening. Missing native Bold uses the existing restrained synthesis fallback.
- The installed Swash 0.2.10 source exposes variation-axis metadata, including minimum/default/maximum values, and `variations`/`normalized_coords` on both shaping and scaling builders. Neither renderer builder currently supplies variation coordinates. This establishes API availability, not a successful variable-font rendering result.
- CPU bitmap and GPU atlas keys include face, glyph, size, and bold state, but no variation instance. Shaping caches also require review before introducing variable weights.
- Cell metrics come from the primary face. Weight changes must retain that grid and its baseline, while shaping/rasterization agree on the selected instance.
- `reload_config` does not replace an active font renderer. `[font]` family and size currently require restart; the new setting should follow that lifecycle unless a separate font-reload change is explicitly undertaken.

## Setting contract selected after the capability gate

- Optional `[font].bold_intensity`, with omission and `1.0` preserving the existing rendering path exactly.
- Validated range: finite `0.75..=1.25`. This bounds the user control, not a promise that every font provides the full range.
- For a usable variable `wght` axis, derive a target from the native Bold face's metadata weight multiplied by the intensity, then clamp to the font's supported axis range. A variable family without a separate Bold face uses reference weight 700 for nondefault values; `1.0` retains its existing fallback. The transition through `1.0` in that case need not be continuous.
- Use identical effective variation coordinates for shaping and rasterization; retain all unrelated axes. Apply this only to SGR Bold runs and their eligible fallback faces.
- Without a usable weight axis, choose the nearest compatible installed Semibold-or-heavier native face, or retain the current fallback if native Bold is absent. Static adjustment is discrete. Ties favor proximity to original Bold, then stable face identity. Nondefault intensity never increases synthesis. Unsupported or unchanged static selection produces a diagnostic once per resolved regular face per renderer.
- If this ratio cannot be defined predictably or verified, propose an explicit `bold_weight` setting with documented OpenType weight units, native/default behavior, and supported-face/axis selection instead. If neither contract can meaningfully control actual font weight, do not add a setting; report the limitation and recommended font choice.

## Plan

1. **Audit and capture the baseline.** Trace primary and fallback selection, variable/named-instance metadata, shaping, hinting/rasterization, CPU caches, GPU atlas and retained frame reuse, initialization, config handling, size shortcuts, and DPI transitions. Record exact font files, face indices, sizes, scales, and native Bold baseline captures. Reuse the existing fontdb/Swash stack and project-owned platform interfaces.

2. **Prove actual weight control before configuration changes.** Use a redistributable monospace variable-font fixture with a usable `wght` axis. Demonstrate distinct native outlines at lighter/native/heavier weights through both builders, inspect hinting and instance handling, and verify preserved grid/baseline and readable counters. Check a static Regular/Bold pair, a font without Bold, and fallback Unicode. Determine whether compatible static weight selection offers a better contract. Record the chosen reference weight, axis clamping, unsupported-font behavior, and final validated range. Stop before adding configuration if the proof fails and present the alternative. The JetBrains Mono variable fixture passed the API proof; installed JetBrains Mono Nerd Font faces passed discrete 600/700/800 selection.

3. **Add the validated configuration and renderer plumbing.** Keep parsing/defaults/errors in `terminal-config`; pass the resolved request through existing app/renderer initialization without moving terminal semantics into the renderer. Preserve omission/`1.0` behavior exactly. Reject wrong types, nonfinite values, and out-of-range values using existing whole-config validation rules. Match current restart-required font behavior, including invalid reload retaining the last valid config and removal restoring defaults for subsequent startup.

4. **Implement supported weight instances and cache identity.** Resolve weight per selected face before shaping. Carry effective variation identity with shaped glyphs through rasterization and CPU/GPU keys; include it in shaping and face-resolution caches where relevant. Keep caches bounded and ensure size/DPI changes and renderer recreation cannot reuse stale instances. Preserve regular-face metrics, baseline positioning, fallback coverage, ANSI colors, and existing synthesis strength. Do not introduce outline dilation, repeated draws, alpha manipulation, or extra emboldening on native Bold.

5. **Add focused tests.** Cover omitted/explicit default, boundaries, invalid inputs, and example-config parity; byte-identical default native Bold bitmaps; variable weight effects and clamping; agreement between shaping and rasterization; static/no-axis/missing-Bold behavior; Unicode fallback and combining marks; regular/Bold/reset transitions; distinct cached instances and repeated cache hits; font initialization/recreation and restart-required reload behavior; sizes and 100%, 125%, and 200% DPI. Use real outline fixtures rather than relabeled metadata to prove weight changes. Add GPU coverage where needed to expose atlas collisions or clipping.

6. **Document the finalized contract.** Update `docs/CONFIG.md`, `config.example.toml`, and relevant native Bold documentation with units, default, range, reference weight, clamping, font requirements, static-font limitations, fallback policy, and restart behavior. Record implementation findings and evidence in a task-specific document; avoid promising continuous control for static fonts.

7. **Run standard and native validation (stopped by subsequent user instruction).** Run focused config/renderer/app regressions, then `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo test --workspace --doc`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --release -p terminal-app`, and `git diff --check`. Visually compare baseline/default/lighter/heavier on KDE Wayland at 100%, 125%, and 200%, multiple logical sizes, and light/dark backgrounds. Inspect thin strokes, counters, punctuation, box drawing, Nerd Font symbols, combining marks, Unicode fallback, clipping, and normal/Bold/reset alignment. Exercise restart, size shortcuts, and DPI changes. Retain paired screenshots and logs with exact fonts/settings and report any unavailable coverage honestly.

## Acceptance gates

- A supported font demonstrates actual native weight changes; unsupported fonts have explicit, predictable behavior.
- Omission and `1.0` retain existing native Bold rendering without added synthesis.
- Weight changes preserve terminal grid, baseline, Unicode coverage, colors, and scaling; shaping/CPU/GPU caches cannot mix instances.
- Configuration behavior and documentation match the existing font lifecycle.
- Focused and standard checks pass, and task-specific KDE Wayland comparisons support the quality claim. No commit or push.
