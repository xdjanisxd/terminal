# Terminal branding

Display name: **Terminal**. Executable: `terminal` (`terminal.exe` on Windows).
Stable application/icon identifier: `io.github.xdjanisxd.terminal`.
Cargo packages and portable archive names retain their existing names.

- `source/terminal.png`: the single canonical 1024×1024 RGBA, lossless icon. The current terminal-prompt tile is an original project-owned **placeholder**, distributed under the repository MIT license. Replace this file with final project-owned artwork to update all platforms.
- `windows/terminal.ico`: generated 16, 24, 32, 48, 64, 128, and 256 pixel images. `terminal-app` embeds resource ID 1 and Windows version metadata through `winresource`; winit loads it for window/taskbar use. MSVC builds use the Windows SDK resource compiler, on both x86_64 and ARM64. The MSI reuses the embedded application icon. No post-build editing is needed.
- `linux/share/applications/io.github.xdjanisxd.terminal.desktop` and `linux/share/icons/hicolor/{size}x{size}/apps/io.github.xdjanisxd.terminal.png`: freedesktop assets reused by portable packaging and the user installer. Window Wayland app ID and X11 class match the desktop entry. Normal builds and archive extraction do not register them. `scripts/install_linux.py` installs the binary, icons, and an absolute-path launcher without requiring user PATH changes; see [Linux installation](../../docs/LINUX_DESKTOP_INTEGRATION.md).
- `macos/terminal.icns`: generated PNG-backed ICNS entries including standard and Retina resolutions up to 1024 pixels. `macos/Info.plist.in` is a future bundle metadata template: substitute `@VERSION@` and `@BUILD_VERSION@`, put the binary in `Contents/MacOS/terminal`, and the icon in `Contents/Resources/terminal.icns`. No app bundle is built here; a bare macOS executable does not acquire a Finder/Dock application icon from an adjacent ICNS file.

## Regeneration

Use Python 3.14 and the pinned Pillow dependency in an isolated environment:

```sh
python -m venv target/branding-tools
# Activate the environment (Scripts/activate on Windows, bin/activate on Unix).
python -m pip install -r scripts/branding-requirements.txt
python scripts/generate_branding.py
python scripts/generate_branding.py --check
```

The generator reads only the canonical PNG, scales with Pillow's pinned Lanczos resampler, and writes ICO, PNG, and ICNS variants without timestamps or GUI tools. Platform assets are committed; normal Rust builds need neither Python nor Pillow. PNG compression libraries can emit different IDAT bytes on different operating systems, so `--check` compares decoded RGBA pixels and validates exact dimensions, format, embedded resolutions, and absence of color profiles or metadata. It does not rewrite assets and produces the same pass/fail result across CI platforms. It also checks the Linux hicolor paths, desktop identity, and bundle metadata. `--create-placeholder` exists only to recreate the initial artwork when the source is absent; it refuses to overwrite any source.

Windows portable packaging checks every embedded icon image against the committed ICO before launching the extracted executable with `--help`. Linux portable archives include only the Linux share tree. Windows and macOS portable archives need no loose icon files. Installers and packages reuse these assets and identifiers; signing remains separate work.
