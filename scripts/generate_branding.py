"""Regenerate committed icons from the canonical RGBA PNG (Pillow pinned nearby)."""

import argparse
import binascii
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


def validate_png_chunks(data: bytes, label: str) -> None:
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError(f"invalid PNG signature: {label}")
    chunks = []
    offset = 8
    while offset + 12 <= len(data):
        length = struct.unpack(">I", data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        end = offset + 12 + length
        if end > len(data):
            raise RuntimeError(f"invalid PNG chunk layout: {label}")
        checksum = struct.unpack(">I", data[end - 4:end])[0]
        if binascii.crc32(data[offset + 4:end - 4]) & 0xFFFFFFFF != checksum:
            raise RuntimeError(f"invalid PNG chunk checksum: {label}")
        chunks.append(kind)
        offset = end
        if kind == b"IEND":
            break
    if offset != len(data) or chunks != [b"IHDR", b"IDAT", b"IEND"]:
        raise RuntimeError(f"PNG must contain only IHDR, IDAT, and IEND chunks: {label}")


def scaled_images(source: Image.Image) -> dict[int, Image.Image]:
    return {size: source.resize((size, size), Image.Resampling.LANCZOS)
            for size in sorted({*SIZES, 1024})}


def variants(images: dict[int, Image.Image]) -> dict[Path, bytes]:
    files = {
        ASSETS / "linux" / "share" / "icons" / "hicolor" /
        f"{size}x{size}" / "apps" / f"{APP_ID}.png": png(images[size])
        for size in SIZES
    }
    output = io.BytesIO()
    images[1024].save(output, format="ICO", sizes=[(size, size) for size in SIZES if size <= 256])
    files[ASSETS / "windows" / "terminal.ico"] = output.getvalue()
    chunks = []
    for kind, size in ICNS_SIZES.items():
        data = png(images[size])
        chunks.append(kind + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    files[ASSETS / "macos" / "terminal.icns"] = b"icns" + struct.pack(">I", len(body) + 8) + body
    return files


def check_png(path: Path, expected: Image.Image) -> None:
    label = str(path.relative_to(ROOT))
    if not path.is_file():
        raise RuntimeError(f"missing branding asset: {label}")
    validate_png_chunks(path.read_bytes(), label)
    with Image.open(path) as actual:
        actual.load()
        if actual.format != "PNG":
            raise RuntimeError(f"{label}: format is {actual.format}, expected PNG")
        if actual.mode != "RGBA":
            raise RuntimeError(f"{label}: mode is {actual.mode}, expected RGBA")
        if actual.size != expected.size:
            raise RuntimeError(f"{label}: dimensions are {actual.size}, expected {expected.size}")
        if actual.info:
            raise RuntimeError(f"{label}: unexpected PNG metadata: {sorted(actual.info)}")
        if actual.tobytes() != expected.tobytes():
            raise RuntimeError(f"{label}: decoded RGBA pixels differ from canonical source")


def check_generated_assets(images: dict[int, Image.Image]) -> None:
    linux_icons = ASSETS / "linux" / "share" / "icons" / "hicolor"
    expected_paths = {
        linux_icons / f"{size}x{size}" / "apps" / f"{APP_ID}.png"
        for size in SIZES
    }
    actual_paths = {path for path in linux_icons.rglob("*") if path.is_file()}
    if actual_paths != expected_paths:
        missing = sorted(path.relative_to(ROOT).as_posix()
                         for path in expected_paths - actual_paths)
        unexpected = sorted(path.relative_to(ROOT).as_posix()
                            for path in actual_paths - expected_paths)
        raise RuntimeError(f"Linux hicolor paths differ; missing={missing}, unexpected={unexpected}")
    for size in SIZES:
        check_png(linux_icons / f"{size}x{size}" / "apps" / f"{APP_ID}.png",
                 images[size])

    windows_path = ASSETS / "windows" / "terminal.ico"
    if not windows_path.is_file():
        raise RuntimeError(f"missing branding asset: {windows_path.relative_to(ROOT)}")
    with Image.open(windows_path) as icon:
        expected_sizes = {(size, size) for size in SIZES if size <= 256}
        if icon.format != "ICO":
            raise RuntimeError(f"{windows_path.relative_to(ROOT)}: format is {icon.format}, expected ICO")
        if icon.ico.sizes() != expected_sizes:
            raise RuntimeError(f"{windows_path.relative_to(ROOT)}: embedded sizes are "
                               f"{sorted(icon.ico.sizes())}, expected {sorted(expected_sizes)}")
        for size in sorted(SIZES):
            if size <= 256:
                embedded = icon.ico.getimage((size, size))
                label = f"{windows_path.relative_to(ROOT)} ({size}x{size})"
                if embedded.size != (size, size):
                    raise RuntimeError(f"{label}: dimensions are {embedded.size}")
                if embedded.info:
                    raise RuntimeError(f"{label}: unexpected image metadata: {sorted(embedded.info)}")
                if embedded.convert("RGBA").tobytes() != images[size].tobytes():
                    raise RuntimeError(f"{label}: decoded RGBA pixels differ from canonical source")

    macos_path = ASSETS / "macos" / "terminal.icns"
    if not macos_path.is_file():
        raise RuntimeError(f"missing branding asset: {macos_path.relative_to(ROOT)}")
    data = macos_path.read_bytes()
    if len(data) < 8 or data[:4] != b"icns":
        raise RuntimeError(f"{macos_path.relative_to(ROOT)}: invalid ICNS signature/header")
    declared_size = struct.unpack(">I", data[4:8])[0]
    if declared_size != len(data):
        raise RuntimeError(f"{macos_path.relative_to(ROOT)}: declared length {declared_size}, "
                           f"actual length {len(data)}")
    offset = 8
    chunks = {}
    while offset < len(data):
        kind = data[offset:offset + 4]
        length = struct.unpack(">I", data[offset + 4:offset + 8])[0]
        if length < 8 or offset + length > len(data) or kind in chunks:
            raise RuntimeError("invalid macOS ICNS chunk layout")
        chunks[kind] = data[offset + 8:offset + length]
        offset += length
    if offset != len(data) or set(chunks) != set(ICNS_SIZES):
        missing = sorted(kind.decode("ascii", errors="replace")
                         for kind in set(ICNS_SIZES) - set(chunks))
        unexpected = sorted(kind.decode("ascii", errors="replace")
                            for kind in set(chunks) - set(ICNS_SIZES))
        raise RuntimeError(f"{macos_path.relative_to(ROOT)}: ICNS representations differ; "
                           f"missing={missing}, unexpected={unexpected}, trailing_bytes={len(data) - offset}")
    for kind, size in ICNS_SIZES.items():
        with Image.open(io.BytesIO(chunks[kind])) as embedded:
            validate_png_chunks(chunks[kind], f"{macos_path.relative_to(ROOT)} ({kind.decode('ascii')})")
            embedded.load()
            label = f"{macos_path.relative_to(ROOT)} ({kind.decode('ascii')})"
            if embedded.format != "PNG":
                raise RuntimeError(f"{label}: format is {embedded.format}, expected PNG")
            if embedded.mode != "RGBA":
                raise RuntimeError(f"{label}: mode is {embedded.mode}, expected RGBA")
            if embedded.size != (size, size):
                raise RuntimeError(f"{label}: dimensions are {embedded.size}, expected {(size, size)}")
            if embedded.info:
                raise RuntimeError(f"{label}: unexpected PNG metadata: {sorted(embedded.info)}")
            if embedded.tobytes() != images[size].tobytes():
                raise RuntimeError(f"{label}: decoded RGBA pixels differ from canonical source")


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
        validate_png_chunks(source_path.read_bytes(), str(source_path.relative_to(ROOT)))
        if source.info:
            raise ValueError("canonical source must not contain image metadata or a color profile")
        images = scaled_images(source)
        files = variants(images)
    if not args.check:
        for path, data in files.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    if args.check:
        check_generated_assets(images)
    validate_metadata()
    print(f"{'Checked' if args.check else 'Generated'} {len(files)} platform icon assets")


if __name__ == "__main__":
    main()
