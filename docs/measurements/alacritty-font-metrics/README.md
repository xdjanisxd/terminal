# Matched Terminal and Alacritty font rendering

Terminal 0.2.2 and Alacritty 0.15.1 displayed the same 80×24 sample on a nested
KWin 6.3.6 Wayland output at scales 1 and 1.25. Both used opaque #101010
backgrounds, #ececec foregrounds, 16 logical px per em (12 pt in Alacritty at
96 logical DPI) and JetBrainsMono Nerd Font Mono. Alacritty used the font's
box-drawing glyphs. The windows were resized to match the grid independently,
and captures were aligned from the blue bar at the start of the first row.

| Display scale | Physical px per em | Terminal and Alacritty cell pitch | Terminal baseline | Capital A ink within each style's row |
| --- | ---: | ---: | ---: | ---: |
| 100% | 16 | 10×22 px | 17 px | y=5–16 |
| 125% | 20 | 12×27 px | 21 px | y=7–20 |

Ink measurements use a maximum channel value of at least 48/255 and at most
three levels of difference between channels. The five outer and dividing
box edges had no gaps at either scale. Capital A reached the same ending row
in all four styles. The earlier Terminal used the same grid and upright
files, and applied a 0.2 shear to Regular and Bold glyphs for italic lines.
Alacritty opened the four distinct faces in [the font manifest](fonts.sha256).
The baseline Terminal also opened the two italic files during discovery and
used the upright files for shaping and rasterization.

Each image shows Terminal before the change, Terminal with native italic, and
Alacritty, from left to right.

- [100% display scale](scale-100.png)
- [125% display scale](scale-125.png)

The candidate used the same four TTF files at both scales. Its upright and
bold text rows were pixel-identical to the baseline. The cap and box-edge
measurements above also held after the change.

| Style | 100% before → after | 125% before → after |
| --- | ---: | ---: |
| Italic | 0.878 → 0.927 | 0.888 → 0.944 |
| Bold italic | 0.913 → 0.949 | 0.903 → 0.952 |

The scores are Dice overlap with Alacritty's thresholded ink mask for the
entire sample row, using the best translation within ±1 px horizontally and
±2 px vertically. The best alignment moved from one pixel left to zero
translation for both italic styles at both scales.

## Reproduce the sample

Use an 80×24 disposable pane at each scale. Run `python3 sample.py` from this
directory. The four text rows are normal, bold, italic and bold italic. The
bottom panel uses the font's box-drawing glyphs. Press Enter to restore the
cursor and return to the shell.

Use this Terminal configuration.

```toml
[font]
family = "JetBrainsMono Nerd Font Mono"
size = 16
bold_intensity = 1.0

[window]
opacity = 1.0
blur = false

[theme]
foreground = "#ececec"
background = "#101010"
```

Use the following Alacritty configuration.

```toml
[window]
dimensions = { columns = 80, lines = 24 }
opacity = 1.0
decorations = "None"

[font]
size = 12
normal = { family = "JetBrainsMono Nerd Font Mono", style = "Regular" }
bold = { family = "JetBrainsMono Nerd Font Mono", style = "Bold" }
italic = { family = "JetBrainsMono Nerd Font Mono", style = "Italic" }
bold_italic = { family = "JetBrainsMono Nerd Font Mono", style = "Bold Italic" }
builtin_box_drawing = false

[colors.primary]
foreground = "#ececec"
background = "#101010"
```

Use `stty size` to confirm the grid and `fc-match` with each family and style
to identify the files. Terminal's `TERMINAL_RENDERER_DIAGNOSTICS=1` prints the
cell metrics and display scale. Alacritty's `-vv` output includes the chosen
face styles, cell pitch and PTY dimensions.
