# Truecolor/background evidence

See [the report](../../TRUECOLOR_BACKGROUND_RENDERING.md) for root causes, results,
configuration differences, and platform limits. Captures were collected on KDE
Wayland with Intel Iris Xe/Mesa. No commit or push was performed.

`environment.json` records revision, versions, binary/config/frame hashes, surface
format, alpha modes, and observed display scales. `pixel-evidence.json` records
native flat-color counts and the 256-entry palette coordinate checks. The JSON
counts include window decorations/shadows; only the requested flat regions are
used for the color conclusions. `neovim-highlights.json` records the live effective
highlight values. `source-frame.ansi` is the identical captured Neovim frame used
in all three replay comparisons.

The baseline has two faults: encoded RGB is encoded again by the sRGB target,
and erased/exposed cells lose the active background. Corrected captures preserve
both explicit RGB values and backgrounds across painted/erased regions. Compare
`replay-before-replay.png`, `replay-candidate-replay.png`, and
`replay-alacritty-replay.png`. Pattern images are crops of compositor screenshots;
window-only capture does not necessarily show the compositor's backdrop/blur.

## Reproduction

`compare.py` uses Python's standard library, installed Terminal/Alacritty,
Vim/Neovim/Zsh, Spectacle, and optionally KWin's scripting interface. It generates
isolated config/input files in `/tmp/truecolor-background-rendering` (override with
`TRUECOLOR_EVIDENCE`) and leaves the user's configuration unchanged. `nvim-config`
intentionally uses the installed Neovim configuration; `nvim-min` and `vim` use a
generated minimal fixture. The script launches native GUI windows and captures
them; run it in an authorized KDE session.

From the repository root, after building the candidate:

```sh
mkdir -p /tmp/truecolor-background-rendering
cp docs/measurements/truecolor-background-rendering/source-frame.ansi /tmp/truecolor-background-rendering/source-frame.ansi
python3 docs/measurements/truecolor-background-rendering/compare.py candidate candidate fixture nvim-min nvim-config vim zsh ansi replay
python3 docs/measurements/truecolor-background-rendering/compare.py reference alacritty fixture nvim-min nvim-config vim zsh ansi replay
```

A preserved baseline binary lives at `/tmp/truecolor-background-rendering/terminal-before`
in the original acceptance environment. `compare.py baseline before ...` selects
it. For a new environment, build revision
`faeeef5ebd9c6df75befa575e8089f3d8bb73896` separately and place that binary there;
do not replace it with the corrected candidate.

For transparency, set `TRUECOLOR_OPACITY=0.85` and `TRUECOLOR_BLUR=false`/`true`.
`TRUECOLOR_POSITION=x,y` uses a temporary KWin script to place only the launched
window, then unloads it. `TRUECOLOR_DESKTOP=1` additionally captures the desktop;
review/crop unrelated desktop content before retaining evidence. Compare over a
fixed backdrop and record actual compositor scale/settings. Display-scale changes
are deliberately outside this single-run capture script.

The native tests in `native-summary.txt` were each invoked with
`cargo test -p terminal-renderer TEST --offline -- --ignored --nocapture --test-threads=1`
or the corresponding `terminal-app` command. The atlas test requires the existing
`crates/renderer/tests/fixtures/JetBrainsMono-variable.ttf` to be visible through
Fontconfig (a temporary config included `/etc/fonts/fonts.conf`, added that fixture
directory, and used a cache under the evidence directory). Standard validation
commands and the initial failing core/GPU evidence are in the accompanying text
files. Full local logs remain under the temporary evidence directory.
