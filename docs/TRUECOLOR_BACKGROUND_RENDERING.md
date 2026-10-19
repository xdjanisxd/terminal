# Color and background rendering correction

Branch: `fix/truecolor-background-rendering`. No commit or push.

Status: implemented and validated on KDE Wayland; standard Rust checks and ten explicit native tests passed.

## Root causes

Two independent defects reproduced on native KDE Wayland, before transparency was enabled.

1. **Background-color erase was missing.** Terminal advertises `xterm-256color`, whose terminfo includes `bce`. Neovim uses background-colored erase operations to clear row suffixes and highlight regions. `TerminalState` instead filled erases, insert/delete gaps, and exposed scrolling rows with `Cell::default()`, discarding the active background. Printed spaces retained Neovim's RGB background; adjacent erased spaces resolved to the terminal theme background. The renderer then faithfully drew inconsistent cell state.

   The minimal reproduction is `SGR 48;2;32;32;32`, painted spaces, and `CSI K`: painted and erased areas must share the active background. The [ncurses terminfo documentation](https://invisible-island.net/ncurses/man/terminfo.5.html#h3-Color-Handling) defines `bce` as erasing with the current background. [Alacritty 0.15.1](https://github.com/alacritty/alacritty/blob/v0.15.1/alacritty_terminal/src/term/mod.rs) uses the current background for erases and newly exposed cells. Erasure carries the background without inheriting foreground, reverse, bold, underline, hyperlinks, or combining marks.

2. **Encoded RGB was treated as linear RGB.** Configured hex colors, ANSI colors, and terminal RGB were normalized by dividing byte channels by 255, then passed unchanged to GPU instances and clears. The actual surface was `Bgra8UnormSrgb`; it encoded those already encoded channels again on storage. This raised dark backgrounds and desaturated the perceived text/background relationship. The existing GPU test repeated that mistaken encoding in its expected values, so it could not catch the defect.

   The fix treats renderer `Rgba` channels as straight-alpha encoded sRGB and decodes RGB at the GPU boundary for sRGB attachments. Alpha remains coverage. This follows the [sRGB transfer function](https://www.w3.org/TR/css-color-4/#color-conversion-code) and wgpu's sRGB texture contract. Non-sRGB attachments retain their existing channel path. Both normal and empty-frame clears now follow the appropriate render-target versus compositor storage premultiplication contract.

## Controlled evidence

Baseline revision: `faeeef5ebd9c6df75befa575e8089f3d8bb73896`. Environment: KWin 6.3.6 / Qt 6.8.2, native Wayland, Vulkan on Intel Iris Xe (ADL GT2), Mesa 25.0.7. Alacritty 0.15.1, Neovim 0.10.4, Vim 9.1. The controlled font was the installed JetBrainsMono Nerd Font Mono Regular/Bold pair, 16 logical pixels (Alacritty 12 points), with Terminal bold intensity `1.0`. Opaque comparisons used opacity `1.0`, blur off, foreground `#d0d0d0`, and default background `#05070b`.

The real installed Neovim configuration was used unchanged: oxocarbon with `termguicolors`, dark background, `Normal` background `#161616`, `CursorLine` `#202020`, `Visual` `#393939`, `Search` `#444444`, and `IncSearch` `#78a9ff`. A separate minimal Vim/Neovim configuration used the same highlight colors. The fixture file and editor PTY were held at 80 columns by 24 rows; capture windows were allowed to settle and sized to contain that viewport.

Normal capability environments differ: Terminal used `TERM=xterm-256color` and inherited an empty `COLORTERM`; Alacritty used `TERM=alacritty`, `COLORTERM=truecolor`. Both terminfo entries advertise `bce`. The original Neovim transcripts each contained 44 `CSI K` and two `CSI J` operations, with repeated explicit `#161616` backgrounds. They were not byte-identical because of capability negotiation. Terminal's lack of OSC 10/11 color-query replies remains a separate existing capability limitation; the explicit highlight values in this reproduction do not depend on those replies.

To isolate rendering, a captured 10,388-byte Neovim frame was replayed unchanged in baseline Terminal, corrected Terminal, and Alacritty. All three replay files exactly matched the source. The row/cell discrepancy and wrong color values persisted in baseline replay and disappeared in corrected replay. This rules out differing application output as the cause of these reproduced defects.

Measured opaque flat pixels from native window captures:

| Requested color | Baseline Terminal | Corrected Terminal | Alacritty |
| --- | --- | --- | --- |
| Default `#05070b` | `#262e3b` | `#05070b` | `#05070b` |
| Neovim Normal `#161616` | `#535353`, with default-color gaps | `#161616`, continuous | `#161616`, continuous |
| `#202020` | `#636363` | `#202020` | `#202020` |
| `#393939` | `#828282` | `#393939` | `#393939` |
| `#444444` | `#8d8d8d` | `#444444` | `#444444` |
| `#78a9ff` | `#b6d5ff` | `#78a9ff` | `#78a9ff` |

The initial report that direct RGB backgrounds looked approximately correct did not hold quantitatively on this checkout's actual sRGB surface. Background erase and double encoding were separately observable; neither was caused by blur. Text coverage and font rasterization differ between terminals, so these flat-pixel results are not a claim of identical glyph-edge pixels.

Before/after evidence: [baseline RGB/erase fixture](measurements/truecolor-background-rendering/before-fixture.png), [corrected fixture](measurements/truecolor-background-rendering/after-fixture.png); identical Neovim replay in [baseline](measurements/truecolor-background-rendering/replay-before-replay.png), [corrected Terminal](measurements/truecolor-background-rendering/replay-candidate-replay.png), and [Alacritty](measurements/truecolor-background-rendering/replay-alacritty-replay.png). [Build/config identities](measurements/truecolor-background-rendering/environment.json) and [pixel measurements](measurements/truecolor-background-rendering/pixel-evidence.json) accompany the captures.

## Implementation

- Terminal-core constructs a nonprinting blank cell carrying only the current background, and passes it through erase, character/line editing, LF/IND/NEL/RI, explicit scrolling, and autowrap scrolling. Partial wide-character cleanup uses that background. Moved cells and scrollback retain their original complete attributes. Reset, newly allocated screens, and resize retain their existing initialization behavior.
- Renderer projection still resolves default/indexed/RGB colors, reverse video, selection, and opacity in its existing order. No terminal semantics moved into the renderer.
- GPU drawing decodes RGB for rectangle, glyph, decoration, cursor, scrollbar, title-bar, and UI instances on sRGB attachments, together with the background clear. The existing transparent intermediate/composition pass remains in place. There is no extra opaque pass, surface-format override, palette adjustment, new dependency, or font/atlas change.
- Theme/configuration RGB and ANSI indexes keep their meaning. Opacity still applies to default and explicit terminal backgrounds; foreground, selection, cursor, and UI retain their existing alpha policy. Existing native Bold and DPI paths are unchanged.

The user's original Terminal theme differs from Alacritty defaults, and its configured bold intensity was `0.90`. Those settings can independently change appearance; the comparison aligned them rather than compensating for them in code. Controlled Zsh checks used the same syntax-highlighting plugin/styles and matched ANSI palettes. Neovim-specific behavior was not added.

## Regression and standard validation

Before implementation, three new terminal-core tests failed on lost indexed/RGB backgrounds, resets, and partial-wide erasure. The corrected GPU expectation also failed: at opacity `0.5`, an sRGB target stored `[62,75,85,128]` for a background whose expected stored channels were approximately `[26,38,51,128]`. The [core failure log](measurements/truecolor-background-rendering/bce-before.txt) and [GPU failure log](measurements/truecolor-background-rendering/gpu-before.txt) are retained with the evidence.

Coverage now includes five focused core tests across 20 erase/edit/scroll operations, default/ANSI/indexed/RGB backgrounds, primary/alternate screens, reverse and text styles, SGR `0`/`49`, partial wide cells, hyperlink/combining cleanup, and history preservation. A projection regression checks identical painted/erased backgrounds at opacity `0.0`, `0.85`, and `1.0`. Existing tests that asserted the incorrect canonical-default fills were updated to assert unstyled blanks with the active background.

Native GPU readback extends existing tests with exact reported RGB swatches, solid foreground/underline RGB and alpha, ordinary versus direct/intermediate empty clears, native Bold/reset coverage, UI and alpha preservation. Flat-color assertions use independent encoded values on both `Rgba8Unorm` and `Rgba8UnormSrgb`, at opacity `0.0`, `0.5`, `0.85`, and `1.0`.

All standard commands passed with cached dependencies (`--offline` on Cargo build/check/test/Clippy commands):

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p terminal-app
git diff --check
```

The workspace run passed 746 tests, with 23 native/environment-dependent tests ignored; all four doctests passed. One intermediate run hit `Pty(NotRunning)` in the unchanged PTY worker test. Its targeted rerun passed all eight tests, and the subsequent full workspace run passed. No PTY code was changed.

## Native acceptance

All ten explicit native tests passed, each in its own Cargo process. The six renderer tests cover pipeline validation, variable-weight atlas separation, background/glyph/underline/cursor drawing, encoded RGB/opacity/empty clears, Command Palette alpha, and URL-hint alpha. The four app tests cover blur reload/recreation/shutdown, Command Palette, Tab Picker, and Bold reload/font-size/recreation. Both panel tests completed 24 phases covering dark/light themes, opacity `0.0`/`0.5`/`0.85`/`1.0`, blur toggling, filtering/navigation/activation, split/tab commands, small windows, maximize/restore, and repeated reopen. Their optional fractional-monitor phases were not taken; DPI editor/fixture comparisons were performed separately.

An initial concurrent GPU batch failed because the optional JetBrains Mono variable fixture was not configured, then exited with SIGSEGV. The final native run exposed the existing repository font fixture through a temporary Fontconfig file and ran GPU tests in separate processes; all six passed, including the unchanged native Bold atlas test. No driver or font workaround was added to the application.

| Comparison | Result |
| --- | --- |
| Identical Neovim frame, opaque/blur off | Baseline failure reproduced; corrected RGB and backgrounds agree with Alacritty |
| Live original Neovim configuration and minimal configuration | Continuous Normal/CursorLine backgrounds; no configuration edits |
| Vim and controlled Zsh syntax highlighting | Opaque and opacity-0.85 windows captured sequentially and visually compared; foreground/background and Bold/reset behavior retained |
| ANSI 16/256 | All 256 background entries sampled at their expected grid coordinates match configured palette/cube/grayscale RGB in both terminals |
| Reported four RGB swatches, reverse/reset/erase fixture | Exact opaque values; painted and erased regions agree |
| Opacity `0.85`, blur off/on | Captured over a fixed colored pattern in both terminals; transparency and actual KDE blur observed |
| Opacity `0.0` | Background disappearance captured; foreground retained; GPU alpha checks pass |
| DPI 100%, 125%, 200% | Native Terminal/Alacritty editor and RGB fixtures captured; Terminal diagnostics confirm scales `1`, `1.25`, `2` |
| Selection, UI, cursor and lifecycle | Existing projection tests, six GPU tests, panel tests, blur/Bold lifecycle tests pass |

For policy parity, Alacritty used `transparent_background_colors=true` and `draw_bold_text_with_bright_colors=false`, with a matched 16-color palette. Alacritty normally applies transparency only to default backgrounds; Terminal's documented policy applies it to explicit cell backgrounds as well. That intentional policy difference was preserved, not treated as a bug. See [Alacritty's configuration documentation](https://alacritty.org/config-alacritty.html#colors).

Compositor evidence over the fixed pattern: Terminal [transparent](measurements/truecolor-background-rendering/pattern-transparent-candidate.png) / [blurred](measurements/truecolor-background-rendering/pattern-blur-candidate.png), and Alacritty [transparent](measurements/truecolor-background-rendering/pattern-transparent-alacritty.png) / [blurred](measurements/truecolor-background-rendering/pattern-blur-alacritty.png). Window-only captures can omit backdrop composition, so these are cropped desktop captures restricted to test windows and the pattern. DPI evidence: [125%](measurements/truecolor-background-rendering/dpi-1.25-candidate-nvim-config.png), [200%](measurements/truecolor-background-rendering/dpi-2-final-candidate-nvim-config.png). Transparent Vim and shell evidence: [Vim](measurements/truecolor-background-rendering/transparent-candidate-vim.png), [Zsh](measurements/truecolor-background-rendering/transparent-candidate-zsh.png), with the matching Alacritty captures alongside them. ANSI evidence: [Terminal](measurements/truecolor-background-rendering/ansi-candidate-ansi.png), [Alacritty](measurements/truecolor-background-rendering/ansi-alacritty-ansi.png).

[Native results](measurements/truecolor-background-rendering/native-summary.txt) and [standard results](measurements/truecolor-background-rendering/standard-summary.txt) are retained. See the [evidence README](measurements/truecolor-background-rendering/README.md) for reproduction. Large raw logs, original desktop captures, and the preserved baseline binary remain in `/tmp/truecolor-background-rendering/`. Test windows/processes exited, temporary KWin scripts were unloaded, both monitors were verified back at scale `1.0`, and original Neovim configuration hashes remained unchanged.

## Remaining limits

- Windows, macOS, X11/XWayland, other compositors, and other GPUs were not tested in this task. Native results apply to this KDE Wayland/Intel setup; offscreen GPU tests additionally cover sRGB and non-sRGB target storage.
- Glyph-edge antialiasing/linear-light blending can differ from Alacritty's rendering. This change restores RGB meanings and consistent backgrounds, without forcing identical font rasterization or changing font weight to mimic another terminal.
- Runtime OSC palette/default-color changes and OSC 10/11 queries were not implemented. They are not needed to reproduce or correct the explicit-color/erase defects demonstrated by identical replay.
