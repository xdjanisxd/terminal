"""Regenerate committed icons from the canonical RGBA PNG (Pillow pinned nearby)."""

import argparse
import configparser
import io
from pathlib import Path
import struct
import plistlib

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets" / "branding"
APP_ID = "io.github.xdjanisxd.terminal"
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
ICNS_SIZES = {
    b"icp4": 16, b"icp5": 32, b"icp6": 64, b"ic07": 128,
    b"ic08": 256, b"ic09": 512, b"ic10": 1024,
    b"ic11": 32, b"ic12": 64, b"ic13": 256, b"ic14": 512,
}


def png(image: Image.Image) -> bytes:
    output = io.BytesIO()
    image.save(output, format="PNG", optimize=False, compress_level=9)
    return output.getvalue()


def variants(source: Image.Image) -> dict[Path, bytes]:
    images = {size: source.resize((size, size), Image.Resampling.LANCZOS)
              for size in {*SIZES, 1024}}
    files = {
        ASSETS / "linux" / "share" / "icons" / "hicolor" /
        f"{size}x{size}" / "apps" / f"{APP_ID}.png": png(images[size])
        for size in SIZES
    }
    output = io.BytesIO()
    source.save(output, format="ICO", sizes=[(size, size) for size in SIZES if size <= 256])
    files[ASSETS / "windows" / "terminal.ico"] = output.getvalue()
    chunks = []
    for kind, size in ICNS_SIZES.items():
        data = png(images[size])
        chunks.append(kind + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    files[ASSETS / "macos" / "terminal.icns"] = b"icns" + struct.pack(">I", len(body) + 8) + body
    return files


def placeholder() -> Image.Image:
    # Original project-owned art: a dark terminal tile with a mint prompt.
    image = Image.new("RGBA", (1024, 1024))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((64, 64, 960, 960), radius=180, fill="#17232fff")
    draw.rounded_rectangle((128, 128, 896, 256), radius=64, fill="#2d4052ff")
    for x in (192, 272, 352):
        draw.ellipse((x - 20, 172, x + 20, 212), fill="#90a4b8ff")
    draw.line([(248, 392), (432, 560), (248, 728)], fill="#68e6b4ff", width=64,
              joint="curve")
    draw.rounded_rectangle((500, 696, 784, 760), radius=16, fill="#e6edf3ff")
    return image


def validate_metadata() -> None:
    desktop = configparser.ConfigParser(interpolation=None, strict=True)
    desktop.optionxform = str
    desktop.read(ASSETS / "linux/share/applications" / f"{APP_ID}.desktop", encoding="utf-8")
    entry = desktop["Desktop Entry"]
    expected = {"Name": "Terminal", "Exec": "terminal", "TryExec": "terminal",
                "Type": "Application", "Terminal": "false", "Icon": APP_ID,
                "Categories": "System;TerminalEmulator;", "StartupWMClass": APP_ID}
    for key, value in expected.items():
        if entry.get(key) != value:
            raise ValueError(f"unexpected desktop entry value: {key}")
    with (ASSETS / "macos/Info.plist.in").open("rb") as source:
        metadata = plistlib.load(source)
    for key, value in {"CFBundleName": "Terminal", "CFBundleDisplayName": "Terminal",
                       "CFBundleExecutable": "terminal", "CFBundleIdentifier": APP_ID,
                       "CFBundleIconFile": "terminal.icns", "CFBundlePackageType": "APPL"}.items():
        if metadata.get(key) != value:
            raise ValueError(f"unexpected bundle metadata: {key}")
    with Image.open(ASSETS / "windows/terminal.ico") as icon:
        if icon.ico.sizes() != {(size, size) for size in SIZES if size <= 256}:
            raise ValueError("missing ICO resolutions")
        for size in icon.ico.sizes():
            icon.ico.getimage(size).load()
    with Image.open(ASSETS / "macos/terminal.icns") as icon:
        for size in icon.info["sizes"]:
            icon.icns.getimage(size).load()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if variants need regeneration")
    parser.add_argument("--create-placeholder", action="store_true",
                        help="create the initial source; refuses to overwrite existing artwork")
    args = parser.parse_args()
    source_path = ASSETS / "source" / "terminal.png"
    if args.create_placeholder:
        if args.check or source_path.exists():
            parser.error("placeholder creation requires a missing source and no --check")
        source_path.parent.mkdir(parents=True, exist_ok=True)
        source_path.write_bytes(png(placeholder()))
    with Image.open(source_path) as source:
        source.load()
        if source.format != "PNG" or source.mode != "RGBA" or source.size != (1024, 1024):
            raise ValueError("canonical source must be a 1024x1024 RGBA PNG")
        files = variants(source)
    for path, data in files.items():
        if args.check:
            if not path.is_file() or path.read_bytes() != data:
                raise RuntimeError(f"stale branding asset: {path.relative_to(ROOT)}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    validate_metadata()
    print(f"{'Checked' if args.check else 'Generated'} {len(files)} platform icon assets")


if __name__ == "__main__":
    main()
