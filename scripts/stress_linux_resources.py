#!/usr/bin/env python3
"""Measure Linux terminal resources during bounded lifecycle and PTY stress."""

import argparse
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target/release/terminal"


def pane(command=None):
    session = ', session = "local_shell"'
    if command is not None:
        session = f", session = {{ command = {{ program = \"python3\", args = [\"-u\", \"-c\", {json.dumps(command, ensure_ascii=False)}] }} }}"
    return f"{{ kind = \"pane\"{session} }}"


def workspace(kind, command=None):
    if kind == "three_panes":
        node = pane(command)
        for _ in range(2):
            node = f'{{ kind = "split", axis = "vertical", first_share = 500000, first = {node}, second = {pane(command)} }}'
        return f'[workspace]\nactive_tab = 0\n[[workspace.tabs]]\ntitle = "Stress panes"\nlayout = {node}\n'
    count = 3 if kind == "three_tabs" else 1
    tabs = "".join(
        f'[[workspace.tabs]]\ntitle = "Stress tab {index + 1}"\nlayout = {pane(command)}\n'
        for index in range(count)
    )
    return f"[workspace]\nactive_tab = 0\n{tabs}"


def dbus(*args):
    return subprocess.run(
        ["qdbus6", "org.kde.KWin", *args], check=True, text=True, capture_output=True
    ).stdout.strip()


def process_sample(pid, previous=None):
    root = Path(f"/proc/{pid}")
    status = dict(
        (line.split(":", 1)[0], int(line.split()[1]))
        for line in (root / "status").read_text().splitlines()
        if line.startswith(("VmRSS:", "RssAnon:", "RssFile:", "Threads:"))
    )
    stat = (root / "stat").read_text().rsplit(")", 1)[1].split()
    ticks = int(stat[11]) + int(stat[12])
    private = 0
    for line in (root / "smaps_rollup").read_text().splitlines():
        if line.startswith(("Private_Clean:", "Private_Dirty:", "Private_Hugetlb:")):
            private += int(line.split()[1])
    now = time.monotonic()
    cpu = None
    if previous is not None:
        cpu = max(0.0, ticks - previous[0]) / os.sysconf("SC_CLK_TCK") / (now - previous[1]) * 100
    children_file = root / "task" / str(pid) / "children"
    children = [int(child) for child in children_file.read_text().split()]
    return {
        "rss_kib": status["VmRSS"],
        "private_kib": private,
        "anon_kib": status["RssAnon"],
        "file_kib": status["RssFile"],
        "threads": status["Threads"],
        "fds": len(list((root / "fd").iterdir())),
        "children": children,
        "cpu_percent": cpu,
        "_ticks": ticks,
        "_time": now,
    }


def public_sample(sample):
    return {key: value for key, value in sample.items() if not key.startswith("_")}


def close_window(process, case, label):
    plugin = f"terminal-stress-{process.pid}-{label}"
    script = case / f"close-{label}.js"
    script.write_text(
        f"function close(w) {{ if (w.pid === {process.pid}) w.closeWindow(); }} "
        "workspace.windowAdded.connect(close); "
        "for (const w of workspace.windowList()) close(w);\n"
    )
    script_id = dbus("/Scripting", "org.kde.kwin.Scripting.loadScript", str(script), plugin)
    started = time.monotonic()
    try:
        run_error = None
        try:
            dbus(f"/Scripting/Script{script_id}", "org.kde.kwin.Script.run")
        except subprocess.CalledProcessError as error:
            run_error = error
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired as error:
            detail = run_error.stderr.strip() if run_error else "KWin returned success"
            raise RuntimeError(f"window close did not exit terminal: {detail}") from error
        if process.returncode != 0:
            raise RuntimeError(f"terminal exited with {process.returncode}")
        if run_error is not None:
            print(f"KWin close reply after clean app exit: {run_error.stderr.strip()}", file=sys.stderr)
        return (time.monotonic() - started) * 1000
    finally:
        try:
            dbus("/Scripting", "org.kde.kwin.Scripting.unloadScript", plugin)
        except subprocess.CalledProcessError:
            pass


def stop_owned_tree(pid):
    """Stop only the terminal and descendants discovered through /proc."""
    descendants = []
    pending = [pid]
    while pending:
        current = pending.pop()
        children = Path(f"/proc/{current}/task/{current}/children")
        if children.exists():
            found = [int(child) for child in children.read_text().split()]
            descendants.extend(found)
            pending.extend(found)
    for child in descendants:
        try:
            os.kill(child, 15)
        except ProcessLookupError:
            pass
    try:
        os.kill(pid, 15)
    except ProcessLookupError:
        pass


def launch(binary, case, kind, command=None):
    for name in ("home", "config", "cache", "tmp"):
        (case / name).mkdir()
    config = case / "workspace.toml"
    config.write_text(workspace(kind, command))
    env = os.environ | {
        "HOME": str(case / "home"),
        "XDG_CONFIG_HOME": str(case / "config"),
        "XDG_CACHE_HOME": str(case / "cache"),
        "TMPDIR": str(case / "tmp"),
        "TERMINAL_STARTUP_DIAGNOSTICS": "1",
        "TERMINAL_STARTUP_LOG": str(case / "terminal.log"),
    }
    log = (case / "terminal.log").open("w")
    process = subprocess.Popen(
        [str(binary), "--workspace", str(config)], cwd=ROOT, env=env,
        stdout=subprocess.DEVNULL, stderr=log,
    )
    return process, log


def run_cycle(binary, output, kind, cycle, idle_seconds, command=None, marker=None):
    case = output / f"{kind}-{cycle}"
    case.mkdir()
    process, log = launch(binary, case, kind, command)
    samples = []
    previous = None
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"terminal exited before ready: {log.name}")
            if marker is None:
                if "first-shell-output-rendered" in Path(log.name).read_text(errors="replace"):
                    break
            elif marker.exists():
                break
            time.sleep(0.02)
        else:
            raise TimeoutError("terminal or PTY workload did not become ready")

        if marker is not None:
            initial = process_sample(process.pid)
            samples.append(initial)
            previous = (initial["_ticks"], initial["_time"])
            marker.with_suffix(".start").touch()
        done_marker = marker.with_suffix(".done") if marker is not None else None
        idle_started = time.monotonic() if marker is None else None
        sampling_deadline = time.monotonic() + idle_seconds + (120 if marker is not None else 0)
        active_cpu = []
        idle_cpu = []
        checkpoints = []
        while True:
            now = time.monotonic()
            if marker is not None and done_marker.exists() and idle_started is None:
                idle_started = now
            if idle_started is not None and now - idle_started >= idle_seconds:
                break
            if now >= sampling_deadline:
                raise TimeoutError("bounded PTY output did not finish")
            sample = process_sample(process.pid, previous)
            samples.append(sample)
            previous = (sample["_ticks"], sample["_time"])
            if sample["cpu_percent"] is not None:
                (idle_cpu if marker is None or done_marker.exists() else active_cpu).append(sample["cpu_percent"])
            if marker is not None:
                for pass_number in (1, 2):
                    checkpoint = marker.with_name(f"{marker.stem}.pass-{pass_number}.done")
                    if checkpoint.exists() and len(checkpoints) < pass_number:
                        checkpoints.append({
                            "pass": pass_number,
                            "rss_kib": sample["rss_kib"],
                            "private_kib": sample["private_kib"],
                        })
            time.sleep(0.5)

        final = samples[-1]
        close_ms = close_window(process, case, cycle)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline and any(Path(f"/proc/{pid}").exists() for pid in final["children"]):
            time.sleep(0.02)
        survivors = [pid for pid in final["children"] if Path(f"/proc/{pid}").exists()]
        if survivors:
            raise RuntimeError(f"child processes survived terminal close: {survivors}")
        record = {
            "scenario": kind,
            "cycle": cycle,
            "samples": len(samples),
            "idle_seconds": idle_seconds,
            "before": public_sample(samples[0]),
            "peak": {
                "rss_kib": max(sample["rss_kib"] for sample in samples),
                "private_kib": max(sample["private_kib"] for sample in samples),
                "threads": max(sample["threads"] for sample in samples),
                "fds": max(sample["fds"] for sample in samples),
                "children": max(len(sample["children"]) for sample in samples),
            },
            "final_before_close": public_sample(final),
            "unicode_pass_checkpoints": checkpoints,
            "activity_cpu_percent_median": statistics.median(active_cpu) if active_cpu else None,
            "idle_cpu_percent_median": statistics.median(idle_cpu) if idle_cpu else None,
            "idle_cpu_percent_p95": sorted(idle_cpu)[math.ceil(0.95 * len(idle_cpu)) - 1] if idle_cpu else None,
            "close_to_exit_ms": close_ms,
            "process_after_close": not Path(f"/proc/{process.pid}").exists(),
            "children_after_close": survivors,
        }
        return record
    finally:
        if process.poll() is None:
            stop_owned_tree(process.pid)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                stop_owned_tree(process.pid)
                process.kill()
                process.wait(timeout=5)
        log.close()
        shutil.rmtree(case / "cache", ignore_errors=True)


def unicode_command(marker):
    code = (
        "import os,time\n"
        f"open({str(marker)!r},'wb').close()\n"
        f"start={str(marker.with_suffix('.start'))!r}\n"
        "while not os.path.exists(start): time.sleep(.01)\n"
        "block=('界🙂e\\u0301 العربية 漢字 🇹🇷\\r\\n'*64).encode()\n"
        "limit=16*1024*1024\n"
        "for pass_number in (1,2):\n"
        " total=0\n"
        " while total<limit:\n"
        "  total+=os.write(1,block[:min(len(block),limit-total)])\n"
        f" open({str(marker.parent / (marker.stem + '.pass-1.done'))!r} if pass_number==1 else {str(marker.parent / (marker.stem + '.pass-2.done'))!r},'wb').close()\n"
        f"open({str(marker.with_suffix('.done'))!r},'wb').close()\n"
        "os.execv('/bin/sleep',['sleep','300'])\n"
    )
    return code


def self_test():
    assert workspace("three_panes").count('kind = "pane"') == 3
    assert workspace("three_tabs").count("[[workspace.tabs]]") == 3
    assert "16*1024*1024" in unicode_command(Path("/tmp/ready"))
    print("Linux resource stress runner self-check passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--cycles", type=int, default=3)
    parser.add_argument("--idle-seconds", type=int, default=30)
    parser.add_argument("--output", type=Path, default=ROOT / "target/perf/linux-resource-stress")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if sys.platform != "linux":
        parser.error("Linux resource stress requires Linux")
    if args.cycles < 1 or args.idle_seconds < 5:
        parser.error("use at least one cycle and five idle seconds")
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"terminal binary not found: {binary}")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    records = []
    for kind in ("one_pane", "three_panes", "three_tabs"):
        for cycle in range(1, args.cycles + 1):
            records.append(run_cycle(binary, args.output, kind, cycle, args.idle_seconds))
            print(json.dumps(records[-1]), flush=True)
    marker = args.output / "unicode-ready"
    marker.unlink(missing_ok=True)
    records.append(run_cycle(
        binary, args.output, "unicode_output", 1, args.idle_seconds,
        unicode_command(marker), marker,
    ))
    print(json.dumps(records[-1]), flush=True)
    result = {
        "measured_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "host": platform.platform(),
        "cpu_model": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")), "unknown"),
        "cycles_per_layout": args.cycles,
        "idle_seconds_per_run": args.idle_seconds,
        "unicode_output_bytes": 32 * 1024 * 1024,
        "binary": str(binary),
        "samples": records,
    }
    path = args.output / "results.json"
    path.write_text(json.dumps(result, indent=2) + "\n")
    print(f"results: {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
