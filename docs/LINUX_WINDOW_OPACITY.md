# Linux window background opacity

Implemented on `feat/linux-window-opacity`. No commit or push. Native acceptance
performed on 2026-10-09 in KDE Wayland, including an XWayland window.

## Configuration and behavior

```toml
[window]
opacity = 0.85
```

The default is `1.0`. Values must be finite numbers in `0.0..=1.0`; existing
configuration diagnostics reject invalid values, types, and unknown fields.
Invalid reloads retain the last valid configuration. Linux supports live reload.
Windows and macOS accept the setting but continue rendering with opacity `1.0`.

Opacity changes the default terminal background, padding, and explicit ANSI/RGB
cell backgrounds, including backgrounds resolved from inverse video. Foreground,
glyph coverage, cursor, selection, search highlights, and UI colors retain their
existing alpha values. Some existing cursor/UI colors are already translucent;
the setting does not multiply their alpha. Theme RGB values are unchanged.

## Implementation

- Configuration owns a separate `WindowConfig`, with validation before conversion
  to `f32`. `Config` now uses `PartialEq` rather than `Eq` for its float field.
- Linux windows request alpha-capable composition at creation, even at opacity
  `1.0`, so X11 can subsequently reload opacity without recreating the window.
  Windows/macOS window attributes remain unchanged.
- Renderer initialization receives opacity before the first surface configure.
  Surface reconfiguration selects explicit `PreMultiplied` alpha when opacity is
  below `1.0` and the surface advertises support. Otherwise it renders opaquely
  and reports the limitation. `Auto`, `Inherit`, and `PostMultiplied` are not
  treated as a known premultiplied contract. The opaque path retains its existing
  default alpha mode and direct rendering.
- Renderer projection applies background alpha after inverse color resolution
  and before selection/UI overrides. Terminal semantics remain in terminal-core.
- Translucent cell backgrounds replace the clear rather than blending over it,
  avoiding accumulated alpha. Glyphs and decorations retain their existing
  blending/rasterization. An intermediate target preserves linear blending;
  a final pass converts premultiplied linear RGB to premultiplied stored RGB for
  sRGB surfaces. Linear surfaces use direct premultiplied values. Empty-frame
  clears follow the same alpha contract.
- Extra pipelines are lazy and cached. The intermediate texture is reused until
  its size changes; transparency costs an additional full-window texture and
  composition pass. Opacity `1.0` avoids those resources and that pass. Resize,
  DPI, and surface recovery continue through the existing lifecycle.

## Validation

All passed against the final implementation:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p terminal-renderer native_gpu_ -- --ignored --nocapture
cargo build --release
git diff --check
```

The workspace run passed 682 tests, with six environment-dependent tests ignored.
All four doctests passed. The three native GPU tests were run explicitly and
passed on Vulkan / Intel Iris Xe (ADL GT2), using Intel Mesa. Focused coverage
includes validation boundaries/non-finite input, reload retention, background
versus foreground alpha, inverse colors, selection/UI overlays, opacity-1
projection equivalence, and supported/unsupported alpha modes. GPU readback
checks nonblack clears, replacement backgrounds, glyph coverage, and stored
premultiplication at `0.0`, `0.5`, `0.85`, and `1.0` on linear and sRGB targets.

## Native Linux results

The release binary used Vulkan / Intel Iris Xe with Mesa. Renderer raw-window
handle diagnostics verified actual Wayland and X11 backends rather than inferring
them from session environment. Both surfaces supported `PreMultiplied`.

| Check | KDE native Wayland | X11 through XWayland in KDE |
| --- | --- | --- |
| Omitted setting and `1.0` | Captured client pixels exactly match the pre-change release | Opaque |
| `0.85` / `0.0` | Compositor transparency observed; text/cursor remain visible | Compositor transparency observed |
| ANSI background / inverse / Unicode glyphs | Visually checked | Visually checked |
| Live reload and rejected NaN | Passed, last valid opacity retained | Passed, last valid opacity retained |
| Resize, maximize/restore, minimize/restore | Passed | Passed |
| Display transition | Scale `1.0` to `1.2`, grid/glyph resize observed | Global XWayland scale about `1.198`, moved between displays |
| Selection / palette / split / new tab / tab picker | Keyboard automation unavailable | Passed using X11 input; selection and UI remained opaque |

The pre-change baseline was built from `git archive HEAD` in a separate temporary
directory. An unobstructed 1596-by-1173 client-area capture containing ANSI colors,
inverse video, Unicode glyphs, cursor, and background was byte-identical between
baseline, omitted opacity, and explicit `1.0`. This is fixture-level evidence,
not a claim about every possible frame. Opaque startup diagnostics were sampled
for baseline and new release;
these individual runs are not a performance benchmark. The implementation avoids
additional opaque pipelines, texture allocation, composition passes, and an
extra startup reconfigure. No performance improvement is claimed.

Temporary logs/screenshots and harness are in `/tmp/opacity-acceptance` and
`/tmp/opacity_acceptance.py`; validation logs are `/tmp/opacity-*.log`. Desktop
screenshots are local acceptance evidence and are not included in the repository.

## Remaining limitations

- A compositor must honor per-pixel alpha. Only advertised premultiplied surface
  support is enabled; unsupported modes receive an opaque fallback. Compositor
  policy can still override the result. Other compositor/GPU combinations were
  not tested, and no separate native Xorg session was available.
- KDE's fake-input protocol was unavailable in this session. Native Wayland
  keyboard selection/overlay acceptance remains unverified. Search, pane zoom,
  all cursor styles, forced surface-loss recovery, and suspend/resume were not
  individually exercised in this native acceptance run; existing Rust coverage
  and the shared rendering path provide regression coverage, not native proof.
- Windows/macOS changes are gated to preserve existing behavior, but native
  Windows/macOS builds and visual tests were unavailable on this Linux host.
- Closing the native Wayland fixture produced `Bad file descriptor` followed by
  process exit `-11`. The unchanged baseline reproduced the same shutdown error.
  XWayland fixtures exited normally. The shutdown issue remains unresolved and
  has not been attributed to opacity.
- Eight-bit intermediate storage can introduce small rounding differences on
  translucent antialiased pixels; GPU readback allows a two-byte tolerance.

## Blur handoff

No blur protocol, compositor rule, or custom title bar was added. A later blur
task can use the established alpha-capable window and premultiplied surface
contract, but must separately negotiate compositor support and define blur
regions. Terminal padding and cell backgrounds are translucent; selection,
search highlights, and UI should retain their existing appearance. Opacity alone
neither requests nor guarantees blur. Preserve the opaque fallback and the
direct opacity-1 path when adding compositor-specific behavior.
