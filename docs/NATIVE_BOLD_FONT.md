# Native Bold font rendering

Implemented on `feat/native-bold-font`; no commit or push. The [plan](NATIVE_BOLD_FONT_PLAN.md) describes the original scope. Evidence is retained under [measurements/native-bold-font](measurements/native-bold-font/).

## Implementation

`crates/renderer/src/font.rs` resolves Bold companions through the existing fontdb database using the actual selected face's family names, style, stretch, and Bold weight request. It rejects unrelated families, mismatched style/stretch, proportional companions for a monospaced face, unusable face data, and nearest regular-face substitutes. Bold or heavier primary faces remain unchanged; a SemiBold primary can still resolve a heavier native Bold companion. A nearest native face of at least SemiBold weight is accepted when exact Bold is absent.

`gpu.rs` passes SGR 1 weight to shaping before atlas lookup. Shaping uses the selected face's own glyph IDs and offsets. Unicode characters retain the existing regular-face coverage resolution, then use that face's usable Bold companion when it covers the character. Missing Bold characters stay on the resolved regular/fallback face. Primary Bold is classified as primary text, not Unicode fallback.

Native weighted faces receive no outline synthesis. If no native companion covers a character, regular outlines receive `pixels_per_em / 48` emboldening, half the previous strength. Already weighted faces receive no extra synthesis. This fallback remains distinct but preserves counters more reasonably at small sizes.

Shaped-text cache keys now include requested weight. Companion resolution is cached per regular face within one font database. CPU bitmap and GPU atlas keys already include actual face identity, glyph ID, physical pixels-per-em, and requested weight; they continue to separate regular, native Bold, and synthesized output. Cache capacities and eviction behavior remain bounded. DPI changes invalidate the current shaping epoch and select correctly sized CPU/atlas entries; valid entries can be reused on a return to the same size.

Cell metrics, baseline anchoring, grid placement, clipping, foreground color projection, ANSI semantics, and terminal attributes retain their existing owners and behavior. A font-family change still requires restart, as documented in [configuration](CONFIG.md): a fresh renderer creates a new database, CPU caches, and draw resources/atlas, so database-local IDs cannot alias an old font's entries. Runtime font-size changes retain physical-size cache separation. No dependencies or platform-specific code were added.

## Native visual results

Linux KDE Wayland, JetBrainsMono Nerd Font Mono, logical size 16, opacity 1, blur disabled. The baseline was built from `1fde87726d9ec9adb1b00599bbb5bb905c6bd65f`; the candidate was built from this branch's uncommitted implementation. Both used the same test inputs and native application. See `build-identities.txt` for binary hashes and environment metadata.

| Workload | Result | Evidence |
| --- | --- | --- |
| Regular / Bold / SGR 22 reset | Native Bold has visibly lighter strokes and more open counters than synthesized baseline; regular/reset text stays aligned. Accents, combining marks, CJK, and Nerd Font symbols remain visible. | [baseline](measurements/native-bold-font/baseline-ansi.png), [candidate](measurements/native-bold-font/candidate-ansi.png) |
| ANSI, indexed, RGB foregrounds | Red/green/blue/cyan retain their hues across weight changes; indexed red and RGB cyan remain distinct. | Same ANSI captures; snapshot color/reset tests and native GPU test |
| Zsh 5.9 syntax highlighting | Typed `echo` and invalid-command tokens show native Bold with preserved green/red highlighting; string highlighting remains yellow. | [baseline](measurements/native-bold-font/baseline-zsh.png), [candidate](measurements/native-bold-font/candidate-zsh.png) |
| MTR 0.95 | Interactive loopback display, headings and numeric columns remain aligned; Bold headers lose the excessively heavy baseline appearance. | [baseline](measurements/native-bold-font/baseline-mtr.png), [candidate](measurements/native-bold-font/candidate-mtr.png) |
| Vim 9.1 | Explicit Bold syntax group with RGB foreground preserves Unicode and cell positions in the editor. | [baseline](measurements/native-bold-font/baseline-vim.png), [candidate](measurements/native-bold-font/candidate-vim.png) |
| Neovim 0.10.4 | Same Bold group remains readable with correct color, background, cursor, and status line. | [baseline](measurements/native-bold-font/baseline-nvim.png), [candidate](measurements/native-bold-font/candidate-nvim.png) |
| Live DPI changes | One running ANSI fixture traversed 1x → 1.25x → 2x → 1x, with readable native Bold, correct baseline/grid scaling, and no stale bitmaps after returning. Diagnostic log confirms all scale events. Both KDE outputs were restored to 1x. | `dpi-transitions.log`, `dpi-0-1.png`, `dpi-1-1.25.png`, `dpi-2-2.png`, `dpi-3-1.png` |
| No native Bold | Installed Noto Mono has only a regular face. It stays in that family for supported characters and receives restrained synthesis; Bold remains distinguishable without changing row spacing or colors. | [fallback capture](measurements/native-bold-font/fallback-ansi.png), `fallback.log` |

Pixel comparison of the complete text-cell rows in the 1x ANSI captures confirms regular text, SGR 22 reset text, and regular color rows are identical to baseline; the Bold row differs as intended. See `pixel-comparison.txt` and its saved PNG decoder for the exact comparison.

Each application exited cleanly after its scripted session; the editor/MTR probes returned to their enclosing PTY fixture. The fixture's nested PTY forwards original application output bytes to the actual terminal renderer, rather than replacing applications with canned ANSI samples. Raw baseline application transcripts are retained. MTR used localhost and a one-second interval. The initial sub-second MTR probe was rejected by MTR's non-root interval restriction; the successful captures use the corrected interval. An initial Zsh scripted-exit timeout was corrected before successful evidence capture.

## Validation

Passed:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo test --workspace --doc`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --release`
- `git diff --check`
- Explicit ignored test `native_jetbrains_bold_uses_distinct_outlines_and_preserves_grid`: selected `JetBrainsMonoNFM-Regular` and `JetBrainsMonoNFM-Bold`, checked native outlines without added synthesis at 1x/1.25x/2x, advances, metrics, Unicode/fallback coverage.
- Explicit ignored test `native_regular_only_family_uses_synthesis_without_family_substitution`: Noto Mono fallback selection and distinct synthesized bitmaps with unchanged advances.
- Explicit ignored test `native_gpu_renders_backgrounds_glyphs_underlines_and_cursor`: GPU rendering, weight/reset atlas separation, and foreground/background checks.

Deterministic tests use the licensed existing Tuffy fixture under separate metadata identities to test face selection and cache behavior without depending on installed fonts. They cover invalid data/style rejection, native no-synthesis behavior, regular/Bold shaping cache separation across physical sizes, fallback Unicode coverage, and synthesis cache separation. The installed JetBrains test independently verifies actual distinct font outlines. Existing tests retain cache eviction, glyph-origin/baseline clipping, physical-size keys, inverse/underline behavior, and SGR/color/reset coverage.

Successful command logs, screenshots, transcripts, and the native harness sources are in the evidence directory. The harnesses use `/tmp/native-bold-validation` and were run from `/tmp`; their saved source is task evidence, not a new general-purpose application interface.

## Limitations

Runtime acceptance covers KDE Wayland on this Linux host. Windows and macOS runtime rendering were not exercised; selection changes use shared fontdb/Swash code without new platform branches. The editor probes used clean `-u NONE` configurations and an explicit Bold highlight group, not every user plugin/theme. MTR acceptance covers its interactive loopback display, not remote network behavior.

Native selection covers faces exposed by the current database metadata. This change does not instantiate a variable-font `wght` axis when a font exposes only a regular database face; that case uses the synthesis fallback. Existing per-character Unicode fallback, outline-only rasterization, and terminal-cell clipping are preserved. Font families with unusual native Bold metrics can have different glyph ink shapes, but cell geometry and baseline anchoring remain regular-face based. Visual acceptance is specific to the fonts, sizes, and scales tested here.
