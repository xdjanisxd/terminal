#!/usr/bin/env python3
"""Capture minimal, deterministic app-to-terminal query traces from local PTYs."""

from __future__ import annotations

import argparse
import collections
import fcntl
import hashlib
import os
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time
import uuid
import json
from pathlib import Path


CSI = re.compile(rb"\x1b\[([0-?]*[ -/]*)([@-~])")
OSC = re.compile(rb"\x1b\](.*?)(?:\x07|\x1b\\)", re.DOTALL)
DCS = re.compile(rb"\x1bP(.*?)(?:\x07|\x1b\\)", re.DOTALL)


def queries(data: bytes) -> list[tuple[str, bytes]]:
    """Return only app-emitted terminal queries, preserving order and repeats."""
    found: list[tuple[int, str, bytes]] = []
    csi_matches = list(CSI.finditer(data))
    for match in csi_matches:
        params, final = match.group(1), match.group(2)
        body = match.group(0)
        if final in (b"c", b"n") or (final == b"p" and b"$" in params) or (final == b"u" and params == b"?"):
            found.append((match.start(), "CSI query", body))
        elif final == b"u" and params == b">7":
            found.append((match.start(), "Kitty keyboard flags setup", body))
    for match in OSC.finditer(data):
        payload = match.group(1)
        if re.match(rb"(?:4;[0-9]+;\?|10;\?|11;\?|12;\?)", payload):
            found.append((match.start(), "OSC color query", match.group(0)))
    for match in DCS.finditer(data):
        if match.group(1).startswith((b"+q", b"$q")):
            body = match.group(0)
            label = "DCS capability query"
            if match.group(1) == b"$qm":
                preceding = [item for item in csi_matches if item.end() <= match.start()][-2:]
                if (len(preceding) == 2 and preceding[-1].end() == match.start()
                        and preceding[-1].group(0) == b"\x1b[4:3m"
                        and preceding[-2].end() == preceding[-1].start()
                        and preceding[-2].group(0) == b"\x1b[0m"):
                    body = data[preceding[-2].start():match.end()]
                    label = "DECRQSS rendition probe with exact SGR context"
            found.append((match.start(), label, body))
    found.sort(key=lambda item: item[0])
    return [(label, body) for _, label, body in found]


def relevant_controls(data: bytes) -> dict[str, int]:
    counts: collections.Counter[str] = collections.Counter()
    for match in CSI.finditer(data):
        params, final = match.group(1), match.group(2)
        if (final in (b"c", b"n") or (final == b"p" and b"$" in params)
                or (final == b"u" and params in (b"?", b">7"))):
            counts[f"CSI:{match.group(0).decode('ascii', errors='replace')}"] += 1
        if final == b"m":
            parts = re.findall(rb"(?:^|[;:])(?:4(?::[0-5])?|58(?::[^;:]*)?|59)(?=$|[;:])", params)
            for part in parts:
                counts[f"SGR:{part.decode('ascii')}m"] += 1
        if b"1007" in params and final in (b"h", b"l"):
            counts[f"DEC mode:{params.decode('ascii', errors='replace')}{final.decode()}"] += 1
    for match in OSC.finditer(data):
        payload = match.group(1)
        if re.match(rb"(?:4;[0-9]+;\?|10;\?|11;\?|12;\?|104(?:;[0-9]+)?)", payload):
            key = re.match(rb"(?:4;[0-9]+;\?|10;\?|11;\?|12;\?|104(?:;[0-9]+)?)", payload).group(0)
            counts[f"OSC:{key.decode('ascii', errors='replace')}"] += 1
    for match in DCS.finditer(data):
        if match.group(1).startswith((b"+q", b"$q")):
            counts["DCS:capability query"] += 1
    return dict(sorted(counts.items()))


def capture(name: str, argv: list[str], exit_bytes: bytes, seconds: float) -> dict:
    executable = shutil.which(argv[0])
    if not executable:
        raise RuntimeError(f"required executable not found: {argv[0]}")
    with tempfile.TemporaryDirectory(prefix=f"pty-parity-{name}-") as home:
        env = {key: value for key, value in os.environ.items()
               if not re.search(r"(?:AUTH|TOKEN|CREDENTIAL|API[_-]?KEY|SECRET)", key, re.I)
               and key not in {"TMUX", "CODEX_HOME", "OPENAI_BASE_URL"}}
        env.update({"TERM": "xterm-256color", "COLORTERM": "truecolor", "HOME": home,
                    "CODEX_HOME": os.path.join(home, ".codex"), "TMPDIR": home,
                    "TMUX_TMPDIR": home, "LC_ALL": "C.UTF-8"})
        Path(env["CODEX_HOME"]).mkdir()
        socket_name = f"pty-parity-{uuid.uuid4().hex[:12]}"
        actual_argv = [executable, *argv[1:]]
        if name == "tmux":
            actual_argv[actual_argv.index("pty-parity-capture")] = socket_name
        pid, master = pty.fork()
        if pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
            os.chdir(home)
            os.execvpe(executable, actual_argv, env)
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
        flags = fcntl.fcntl(master, fcntl.F_GETFL)
        fcntl.fcntl(master, fcntl.F_SETFL, flags | os.O_NONBLOCK)
        raw = bytearray()
        deadline = time.monotonic() + seconds
        exited = False
        try:
            while time.monotonic() < deadline and not exited:
                try:
                    ready, _, _ = select.select([master], [], [], min(0.1, deadline-time.monotonic()))
                    if ready:
                        chunk = os.read(master, 65536)
                        raw.extend(chunk)
                except (BlockingIOError, OSError):
                    pass
                try:
                    waited, _ = os.waitpid(pid, os.WNOHANG)
                    exited = waited == pid
                except ChildProcessError:
                    exited = True
            if not exited:
                try:
                    os.write(master, exit_bytes)
                except OSError:
                    pass
                end = time.monotonic() + 0.5
                while time.monotonic() < end and not exited:
                    try:
                        ready, _, _ = select.select([master], [], [], 0.05)
                        if ready:
                            raw.extend(os.read(master, 65536))
                    except (BlockingIOError, OSError):
                        pass
                    waited, _ = os.waitpid(pid, os.WNOHANG)
                    exited = waited == pid
                if not exited:
                    os.killpg(pid, signal.SIGTERM)
                    grace = time.monotonic() + 0.5
                    while time.monotonic() < grace and not exited:
                        try:
                            waited, _ = os.waitpid(pid, os.WNOHANG)
                            exited = waited == pid
                        except ChildProcessError:
                            exited = True
                        if not exited:
                            time.sleep(0.02)
                    if not exited:
                        os.killpg(pid, signal.SIGKILL)
                        try:
                            os.waitpid(pid, 0)
                        except ChildProcessError:
                            pass
        finally:
            os.close(master)
            if not exited:
                try:
                    os.killpg(pid, signal.SIGKILL)
                    os.waitpid(pid, 0)
                except (ProcessLookupError, ChildProcessError):
                    pass
        if name == "tmux":
            subprocess.run(["tmux", "-L", socket_name, "kill-server"], env=env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        found = queries(bytes(raw))
        return {
            "name": name,
            "term": "xterm-256color",
            "colorterm": "truecolor",
            "provenance": {
                "argv": actual_argv,
                "exit_input_hex": exit_bytes.hex(),
                "pty_size": "100x30",
                "capture_seconds": seconds,
                "raw_output_bytes": len(raw),
                "raw_output_sha256": hashlib.sha256(raw).hexdigest(),
                "query_extraction": "lossless matching CSI DA/DSR/DECRQM and OSC color queries; screen output omitted",
                "control_sequence_extraction": "counts of relevant unique CSI/OSC/DCS terminal-query and gated feature controls; arbitrary screen content omitted",
            },
            "observed_controls": relevant_controls(bytes(raw)),
            "writes": [{"hex": data.hex(), "label": f"{label} #{i+1}"}
                       for i, (label, data) in enumerate(found)],
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=Path("tests/fixtures/pty-parity"))
    parser.add_argument("--seconds", type=float, default=1.5)
    args = parser.parse_args()
    specs = [
        ("codex", ["codex"], b"\x03"),
        ("neovim", ["nvim", "-u", "NONE", "-i", "NONE", "--noplugin"], b":qa!\r"),
        ("tmux", ["tmux", "-f", "/dev/null", "-L", "pty-parity-capture", "new-session", "-s", "fixture", "bash", "--noprofile", "--norc"], b"exit\r"),
    ]
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for name, argv, exit_bytes in specs:
        result = capture(name, argv, exit_bytes, args.seconds)
        target = args.output_dir / f"{name}.json"
        target.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"{name}: {len(result['writes'])} query writes -> {target}")


if __name__ == "__main__":
    main()
