# MTR color compatibility investigation

Verified on Debian KDE Wayland, 2026-10-13, branch `fix/mtr-color-compatibility`.

## Root cause

MTR's headers and shortcut letters request **bold default foreground**, not an
explicit ANSI color. Konsole's Breeze scheme has a separate blue intense default
foreground (`ForegroundIntense=61,174,233`). Terminal's configured default
foreground is white. Those colors need not match for terminal compatibility.

There was also a rendering omission: `terminal-core` parsed and retained SGR 1,
but the renderer discarded text intensity. The user confirmed fixing bold while
preserving colors after reviewing this distinction.

Sources:

- [MTR v0.95 curses implementation](https://github.com/traviscross/mtr/blob/v0.95/ui/curses.c):
  `mtr_curses_redraw` applies `A_BOLD` to the title, shortcut letters, and headers.
- [Konsole Breeze scheme](https://github.com/KDE/konsole/blob/master/data/color-schemes/Breeze.colorscheme):
  separate normal and intense default foreground colors.

## Native evidence

Both native windows ran `/usr/bin/mtr -n 127.0.0.1` through the same Python PTY
capture proxy, with a fixed 100-column, 24-row inner PTY. The binary was Debian
`mtr-tiny 0.95-1.1+b1`, SHA-256
`3fcafd92e3cf357d74430df2af316710d8e1b8c659162ec251fbca90bc0b9669`.
Konsole was `4:25.04.2-1`; ncurses was `6.5+20250216-2`.

The native screenshot reproduced blue emphasized text in Konsole and white text
in Terminal. Before and after the fix, both captures have the same ordered SGR
sequence list. Differences elsewhere in the output are timestamps and live ping
statistics. The shortcut rendition is:

```text
Keys:  ESC(B ESC[0;1m H ESC(B ESC[m elp
```

The only SGR forms in these MTR captures are reset (`CSI m`), default foreground
and background (`CSI 39;49 m`), and reset plus bold (`CSI 0;1 m`). There are no
explicit ANSI, indexed, or RGB foreground selections for the observed elements.
No terminal capability or color query appears in MTR's initialization/output;
the captures contain no terminal replies. Thus missing query replies cannot
account for this reproduction.

Both environments use `TERM=xterm-256color`, byte-identical `infocmp -1` output,
`LANG=en_US.UTF-8`, and `LC_ALL=C.UTF-8`. Neither has `TERMINFO`, `TERMINFO_DIRS`,
`TERMCAP`, or `NCURSES_NO_UTF8_ACS` overrides. Konsole adds `COLORTERM=truecolor`
and `COLORFGBG=15;0`; Terminal's values are empty/unset. These differences did not
change the emitted SGR sequence list. Ncurses initialization emits alternate
screen entry, charset selection, margins, mode resets, and default colors in
both runs. No PTY environment, terminfo, or terminal-reply change is warranted.

## Implementation

The renderer projects the existing core bold attribute into draw cells. Its
existing Swash outline rasterizer emboldens the resolved primary or fallback
glyph by `pixels_per_em / 24`, preserving shaping advances and cell metrics.
Both the CPU bitmap cache and GPU atlas distinguish normal and bold glyphs.
App UI overlays retain their existing normal weight.

The implementation is platform independent and introduces no dependencies.
It changes neither the palette nor theme nor terminal color semantics. In
particular, SGR 1 does not remap ANSI colors 0–7 to 8–15. MTR now has visible bold
emphasis while retaining the configured white foreground; Konsole's blue is a
theme choice.

## Verification

- Core regression tests exercise the captured ncurses charset/bold/reset pattern
  at every byte split, preserve default colors, and separate color-pair changes
  from intensity resets.
- Renderer projection tests cover bold/reset, default, ANSI, and RGB foregrounds.
- Rasterization tests verify increased coverage, unchanged advances, fallback
  glyphs and combining marks, 16/32-pixel sizes, and separate cached weights.
- GPU readback compares normal/bold/normal glyphs and checks increased coverage
  plus restoration of the original bitmap after reset.
- Native MTR comparison and ANSI fixtures confirm bold emphasis, reset, red,
  green, blue, cyan, indexed colors, and RGB colors in actual KDE Wayland windows.
- Native Vim and Neovim execution tests open a Unicode/space-containing path,
  record the exact path from inside each editor, and exit successfully. These
  checks do not cover interactive editing or saving a changed buffer.
- Standard validation: formatting, workspace/all-target checks and tests,
  documentation tests, Clippy with warnings denied, release build, and diff checks.

Raw local evidence is in `/tmp/mtr-color-evidence`; validation logs are
`/tmp/mtr-cargo-{check,tests,doc,clippy,release}.log` and
`/tmp/mtr-editors-native.log`. These temporary artifacts are not committed.

## Limits

Native verification covers Debian KDE Wayland at display scale 1, MTR's default
interactive display against loopback, and the installed Vim/Neovim versions.
Windows and macOS were not exercised natively. Bold uses synthesized outline
weight rather than selecting a separate bold font face. Faint and italic visual
rendering remain outside this change. Exact Konsole theme color matching is not
an acceptance criterion; the palette and theme are preserved.
