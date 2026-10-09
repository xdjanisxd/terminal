#!/usr/bin/env python3
"""Install or uninstall Terminal for the current Linux user (Python stdlib only)."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

APP_ID = "io.github.xdjanisxd.terminal"
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)


def string_value(value):
    return (str(value).replace("\\", "\\\\").replace("\n", "\\n")
            .replace("\r", "\\r").replace("\t", "\\t").replace(" ", "\\s"))


def exec_value(path):
    value = str(path)
    # '=' is forbidden in the executable path by the Desktop Entry spec.
    # GLib checks the executable before expanding %% in its name. Reject that
    # path rather than installing an entry that GNOME cannot discover or launch.
    if any(character in value for character in "=%\n\r"):
        raise ValueError("HOME must not contain '=', '%', newline, or carriage return")
    value = "".join("\\" + c if c in '\\"`$' else c for c in value)
    return '"' + value.replace("\\", "\\\\").replace("%", "%%") + '"'


def destinations(home, data):
    paths = [home / ".local/bin/terminal",
             data / "applications" / f"{APP_ID}.desktop"]
    paths.extend(data / "icons/hicolor" / f"{size}x{size}/apps" / f"{APP_ID}.png"
                 for size in SIZES)
    return paths


def digest(path):
    if path.is_symlink() or not path.is_file():
        return None
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def atomic_write(path, contents, mode):
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=".terminal-", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(contents)
            os.fchmod(output.fileno(), mode)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def refresh(data):
    commands = [("update-desktop-database", str(data / "applications")),
                ("gtk-update-icon-cache", "-f", "-t", str(data / "icons/hicolor"))]
    if "KDE" in os.environ.get("XDG_CURRENT_DESKTOP", "").split(":"):
        commands.append(("kbuildsycoca6", "--noincremental"))
    for command in commands:
        if shutil.which(command[0]):
            try:
                result = subprocess.run(command, capture_output=True, text=True, timeout=30)
            except (OSError, subprocess.TimeoutExpired):
                result = None
            if result is None or result.returncode:
                print(f"Warning: {command[0]} failed; desktop refresh may require logout/login",
                      file=sys.stderr)


def install(action, binary=None):
    if sys.platform != "linux":
        raise ValueError("this installer supports Linux only")
    home = Path(os.environ.get("HOME", ""))
    if not home.is_absolute() or not home.is_dir():
        raise ValueError("HOME must be an existing absolute directory")
    data = Path(os.environ.get("XDG_DATA_HOME", ""))
    if not data.is_absolute():
        data = home / ".local/share"
    paths = destinations(home, data)
    record = data / "terminal/desktop-install.json"
    if record.is_symlink():
        raise ValueError(f"refusing symlinked installation record: {record}")
    owned = json.loads(record.read_text()) if record.exists() else {}
    if not isinstance(owned, dict) or set(owned) - {str(path) for path in paths}:
        raise ValueError("installation paths changed; uninstall with the original HOME/XDG_DATA_HOME")
    if any(not isinstance(value, str) or len(value) != 64
           or any(c not in "0123456789abcdef" for c in value) for value in owned.values()):
        raise ValueError("invalid installation ownership record")
    # Preflight every destination before changing anything. Never follow leaf
    # symlinks or overwrite/remove files that this installation does not own.
    for path in paths:
        if path.exists() or path.is_symlink():
            if str(path) not in owned or digest(path) != owned[str(path)]:
                raise ValueError(f"refusing unrelated or modified file: {path}")
    if action == "uninstall":
        for path in paths:
            if str(path) in owned and path.exists():
                path.unlink()
        if record.exists():
            record.unlink()
        refresh(data)
        print("Uninstalled Terminal; configuration and Saved Workspaces preserved.")
        return

    root = Path(__file__).resolve().parent
    share = root / "share" if (root / "share").is_dir() else root.parent / "assets/branding/linux/share"
    source = Path(binary) if binary else root / "terminal"
    if not source.is_file() or not os.access(source, os.X_OK):
        raise ValueError(f"missing executable binary: {source}; pass --binary PATH")
    contents = [source.read_bytes()]
    desktop = (share / "applications" / f"{APP_ID}.desktop").read_text()
    desktop = desktop.replace("\nExec=terminal\n", f"\nExec={exec_value(paths[0])}\n")
    desktop = desktop.replace("\nTryExec=terminal\n", f"\nTryExec={string_value(paths[0])}\n")
    desktop += f"Path={string_value(home)}\n"
    contents.append(desktop.encode())
    contents.extend((share / path.relative_to(data)).read_bytes() for path in paths[2:])
    # A replacement binary is renamed into place, so already running instances
    # retain their executable and an interrupted copy cannot truncate it.
    for index, (path, content) in enumerate(zip(paths, contents)):
        atomic_write(path, content, 0o755 if index == 0 else 0o644)
        owned[str(path)] = hashlib.sha256(content).hexdigest()
        atomic_write(record, (json.dumps(owned, indent=2) + "\n").encode(), 0o600)
    refresh(data)
    print(f"Installed Terminal: {paths[0]}\nDesktop entry: {paths[1]}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("install", "uninstall"))
    parser.add_argument("--binary", help="built Terminal executable (source checkout installs)")
    args = parser.parse_args()
    try:
        install(args.action, args.binary)
    except (OSError, ValueError) as error:
        parser.exit(1, f"Error: {error}\n")


if __name__ == "__main__":
    main()
