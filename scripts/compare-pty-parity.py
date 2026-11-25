#!/usr/bin/env python3
"""Replay terminal-bound fixtures in live emulator PTYs and compare raw replies."""

import argparse
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import select
import shutil
import subprocess
import sys
import tempfile
import termios
import time
import tty


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/pty-parity"
ALACRITTY_VERSION = "0.15.1"
ALACRITTY_SHA256 = "3696cbe9e61c835e539f9dacad0abf86a01eb58a386838ba03ca55075d5475e9"

# Match Terminal's stock foreground, background, cursor, and ANSI colors.
ALACRITTY_CONFIG = """
[general]
ipc_socket = false
[colors.primary]
foreground = "#e6e6e6"
background = "#000000"
[colors.cursor]
cursor = "#e6e6e6"
[colors.normal]
black = "#000000"
red = "#cd3131"
green = "#0dbc79"
yellow = "#e5e510"
blue = "#2472c8"
magenta = "#bc3fbc"
cyan = "#11a8cd"
white = "#e5e5e5"
[colors.bright]
black = "#666666"
red = "#f14c4c"
green = "#23d18b"
yellow = "#f5f543"
blue = "#3b8eea"
magenta = "#d670d6"
cyan = "#29b8db"
white = "#ffffff"
"""


@dataclass(frozen=True)
class Fixture:
    name: str
    term: str
    colorterm: str
    writes: list[tuple[str, bytes]]


def read_fixture(path):
    try:
        data = json.loads(path.read_text())
        writes = [(entry["label"], bytes.fromhex(entry["hex"])) for entry in data["writes"]]
        name, term = data["name"], data["term"]
        colorterm = data.get("colorterm", "truecolor")
        if not all(isinstance(value, str) for value in (name, term, colorterm)):
            raise ValueError("name, term and colorterm must be strings")
        if not name or not term or not writes:
            raise ValueError("name, term and writes must be nonempty")
        if any(not isinstance(label, str) or not label or not payload for label, payload in writes):
            raise ValueError("each write needs a nonempty label and hex payload")
    except (KeyError, TypeError, ValueError) as error:
        raise ValueError(f"{path}: invalid fixture: {error}") from error
    return Fixture(name, term, colorterm, writes)


def read_replies(fd, seconds):
    reply = bytearray()
    deadline = time.monotonic() + seconds
    while (remaining := deadline - time.monotonic()) > 0:
        if not select.select([fd], [], [], remaining)[0]:
            break
        chunk = os.read(fd, 65536)
        if not chunk:
            raise OSError("PTY input closed during replay")
        reply.extend(chunk)
    return reply.hex()


def pty_child(fixture_path, result_path, seconds):
    fixture = read_fixture(fixture_path)
    fd = sys.stdin.fileno()
    output = sys.stdout.buffer
    if not os.isatty(fd) or not os.isatty(output.fileno()):
        raise OSError("replay helper needs a live PTY on stdin and stdout")
    term = os.environ.get("TERM")
    colorterm = os.environ.get("COLORTERM")
    if (term, colorterm) != (fixture.term, fixture.colorterm):
        raise ValueError(f"unexpected PTY environment: TERM={term!r} COLORTERM={colorterm!r}")
    columns, rows = os.get_terminal_size(fd)
    settings = termios.tcgetattr(fd)
    tty.setraw(fd)
    try:
        replies = []
        for _, payload in fixture.writes:
            output.write(payload)
            output.flush()
            replies.append(read_replies(fd, seconds))
    finally:
        termios.tcsetattr(fd, termios.TCSANOW, settings)
    result_path.write_text(json.dumps({"replies": replies, "columns": columns, "rows": rows}))


def replay(executable, name, fixture_path, fixture, directory, seconds, grid=None):
    config = directory / f"{name}.toml"
    result = directory / f"{name}.json"
    # Both launch the same command. env applies the fixture settings inside
    # the child PTY even when an emulator chooses its own default TERM.
    helper = [
        shutil.which("env"),
        f"TERM={fixture.term}",
        f"COLORTERM={fixture.colorterm}",
        sys.executable, "-I", str(Path(__file__).resolve()),
        "--pty-child", str(fixture_path), str(result), str(seconds),
    ]
    env = dict(os.environ, XDG_CONFIG_HOME=str(directory / "config"))
    command = [executable]
    if name == "terminal":
        config.write_text("")
        env["TERMINAL_CONFIG"] = str(config)
    else:
        columns, rows = grid
        config.write_text(f"{ALACRITTY_CONFIG}\n[window.dimensions]\ncolumns = {columns}\nlines = {rows}\n")
        command += ["--config-file", str(config)]
    command += ["-e", *helper]
    log = directory / f"{name}.log"
    with log.open("wb") as stream, subprocess.Popen(
        command, env=env, stdin=subprocess.DEVNULL, stdout=stream, stderr=subprocess.STDOUT
    ) as process:
        try:
            status = process.wait(timeout=30 + seconds * len(fixture.writes))
        except subprocess.TimeoutExpired as error:
            process.kill()
            raise RuntimeError(f"{name} timed out replaying {fixture.name}: {log.read_text(errors='replace')}") from error
    observed = json.loads(result.read_text()) if result.is_file() else {"error": "replay helper produced no result"}
    if status or "error" in observed:
        error = observed.get("error", "replay process failed after producing a result")
        raise RuntimeError(f"{name} exited {status} replaying {fixture.name}: {error}\n{log.read_text(errors='replace')}")
    return observed


def binary_path(value):
    result = shutil.which(value)
    if result is None:
        raise ValueError(f"executable not found: {value}")
    return Path(result).resolve()


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def compare(terminal, alacritty):
    if terminal == alacritty:
        return "same" if terminal else "neither replied"
    if not terminal:
        return "missing in Terminal"
    if not alacritty:
        return "missing in Alacritty"
    return "different"


def table_cell(value):
    return str(value).replace("|", r"\|").replace("\n", r"\n").replace("\r", r"\r")


def print_table(fixture, terminal, alacritty):
    sizes = [f"{case['columns']}×{case['rows']}" for case in (terminal, alacritty)]
    print(f"\n## {fixture.name} (TERM={fixture.term}, COLORTERM={fixture.colorterm})\n")
    print(f"PTY grid (columns × rows): Terminal {sizes[0]}, Alacritty {sizes[1]}\n")
    print("| Write | Request (hex) | Terminal reply (hex) | Alacritty reply (hex) | Comparison |")
    print("| --- | --- | --- | --- | --- |")
    differs = False
    for (label, payload), left, right in zip(fixture.writes, terminal["replies"], alacritty["replies"], strict=True):
        differs |= left != right
        cells = (label, payload.hex(), left or "—", right or "—", compare(left, right))
        print("| " + " | ".join(map(table_cell, cells)) + " |")
    return differs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--terminal", required=True, help="path to the Terminal executable")
    parser.add_argument("--alacritty", default="alacritty", help="path to the Alacritty executable")
    parser.add_argument("--alacritty-version", default=ALACRITTY_VERSION, help="expected Alacritty release")
    parser.add_argument("--alacritty-sha256", default=ALACRITTY_SHA256, help="expected Alacritty executable SHA-256")
    parser.add_argument("--reply-timeout", type=float, default=0.5, help="seconds of input capture per write (default: 0.5)")
    parser.add_argument("--fail-on-difference", action="store_true", help="exit 1 for unequal replies")
    parser.add_argument("fixtures", nargs="*", type=Path, help=f"fixtures to replay (default: {FIXTURES.relative_to(ROOT)}/*.json)")
    args = parser.parse_args()
    if not math.isfinite(args.reply_timeout) or args.reply_timeout <= 0:
        parser.error("--reply-timeout must be a positive finite number")
    if not shutil.which("env"):
        parser.error("env must be available on PATH")
    paths = [path.resolve() for path in (args.fixtures or sorted(FIXTURES.glob("*.json")))]
    if not paths:
        parser.error("no fixtures found")
    fixtures = [read_fixture(path) for path in paths]
    terminal, alacritty = map(binary_path, (args.terminal, args.alacritty))
    version = subprocess.check_output([alacritty, "--version"], text=True, timeout=10).strip()
    sha256 = digest(alacritty)
    if version.split()[:2] != ["alacritty", args.alacritty_version] or sha256 != args.alacritty_sha256.lower():
        raise ValueError(f"Alacritty pin mismatch: {version}, SHA-256 {sha256}")
    print(f"Terminal: `{terminal}` SHA-256 `{digest(terminal)}`")
    print(f"\nReference: `{version}` at `{alacritty}` SHA-256 `{sha256}`")
    print(f"\nReply window: {args.reply_timeout:g} s per write.")
    differs = False
    with tempfile.TemporaryDirectory(prefix="pty-parity-") as directory:
        for index, fixture in enumerate(fixtures):
            case = Path(directory) / str(index)
            case.mkdir()
            path = case / "fixture.json"
            path.write_text(json.dumps({
                "name": fixture.name,
                "term": fixture.term,
                "colorterm": fixture.colorterm,
                "writes": [{"label": label, "hex": payload.hex()} for label, payload in fixture.writes],
            }))
            left = replay(terminal, "terminal", path, fixture, case, args.reply_timeout)
            right = replay(
                alacritty, "alacritty", path, fixture, case, args.reply_timeout,
                (left["columns"], left["rows"]),
            )
            differs |= print_table(fixture, left, right)
    return int(args.fail_on_difference and differs)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--pty-child"]:
        source, destination, seconds = sys.argv[2:]
        result_path = Path(destination)
        try:
            pty_child(Path(source), result_path, float(seconds))
        except (OSError, ValueError) as error:
            result_path.write_text(json.dumps({"error": str(error)}))
            sys.exit(2)
    else:
        try:
            sys.exit(main())
        except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
            sys.exit(f"pty parity: {error}")
