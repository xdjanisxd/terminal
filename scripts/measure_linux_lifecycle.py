#!/usr/bin/env python3
"""Measure release startup, PTY first output, idle CPU/RSS, and pane/tab counts."""

import argparse
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import shutil
import statistics
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target/release/terminal"
READY_COMMAND = "printf 'READY\\n'; exec sleep 300"


def pane():
    return f'{{ kind = "pane", session = {{ command = {{ program = "/bin/sh", args = ["-c", {json.dumps(READY_COMMAND)}] }} }} }}'


def layout(count):
    node = pane()
    for _ in range(count - 1):
        node = f'{{ kind = "split", axis = "vertical", first_share = 500000, first = {node}, second = {pane()} }}'
    return node


def workspace(count, tabs):
    if tabs:
        body = "".join(
            f'[[workspace.tabs]]\ntitle = "Perf tab {index + 1}"\nlayout = {pane()}\n'
            for index in range(count)
        )
    else:
        body = f'[[workspace.tabs]]\ntitle = "Perf panes"\nlayout = {layout(count)}\n'
    return '[workspace]\nactive_tab = 0\n' + body


def qdbus(*args):
    result = subprocess.run(
        ["qdbus6", "org.kde.KWin", *args], text=True, capture_output=True
    )
    if result.returncode:
        raise subprocess.CalledProcessError(
            result.returncode, result.args, result.stdout, result.stderr
        )
    return result.stdout.strip()


def process_ticks(pid):
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    return int(fields[11]) + int(fields[12])


def rss_kib(pid):
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        if line.startswith("VmRSS:"):
            return int(line.split()[1])
    raise RuntimeError(f"VmRSS missing for process {pid}")


def percentile(values, fraction):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(fraction * len(ordered)) - 1)]


def stage_values(text, stage):
    return [int(value) / 1000 for value in re.findall(rf"stage={re.escape(stage)}(?:\s+session=[^ ]+)? elapsed_us=(\d+)", text)]


def wait_for_stage(process, log, stage, deadline):
    pattern = re.compile(rf"stage={re.escape(stage)}(?:\s+session=[^ ]+)? elapsed_us=\d+")
    while time.monotonic() < deadline:
        text = log.read_text(errors="replace") if log.exists() else ""
        if pattern.search(text):
            return text
        if process.poll() is not None:
            raise RuntimeError(f"terminal exited {process.returncode} before {stage}: {text}")
        time.sleep(0.02)
    raise TimeoutError(f"terminal did not reach {stage}")


def close_window(process, directory, sample):
    script = directory / f"close-{sample}.js"
    plugin = f"terminal-perf-{process.pid}-{sample}"
    script.write_text(
        f"function close(w) {{ if (w.pid === {process.pid}) w.closeWindow(); }} "
        "workspace.windowAdded.connect(close); "
        "for (const w of workspace.windowList()) close(w);\n"
    )
    script_id = qdbus("/Scripting", "org.kde.kwin.Scripting.loadScript", str(script), plugin)
    if not script_id.isdigit():
        raise RuntimeError(f"KWin did not return a script id: {script_id!r}")
    run_error = None
    started = time.monotonic()
    try:
        try:
            qdbus(f"/Scripting/Script{script_id}", "org.kde.kwin.Script.run")
        except subprocess.CalledProcessError as error:
            run_error = error
        process.wait(timeout=30)
        if process.returncode != 0:
            raise RuntimeError(f"terminal exited with {process.returncode}")
        if run_error:
            print(f"KWin run reply after clean app exit: {run_error.stderr.strip()}", file=sys.stderr)
        return (time.monotonic() - started) * 1000
    finally:
        try:
            qdbus("/Scripting", "org.kde.kwin.Scripting.unloadScript", plugin)
        except subprocess.CalledProcessError:
            pass


def run_sample(binary, count, tabs, duration, sample, output):
    label = f"{'tabs' if tabs else 'panes'}-{count}-{sample}"
    case = output / label
    case.mkdir(parents=True)
    load_before = list(os.getloadavg())
    for name in ("home", "config", "cache", "tmp"):
        (case / name).mkdir()
    config = case / "workspace.toml"
    config.write_text(workspace(count, tabs))
    log = case / "startup.log"
    env = os.environ | {
        "HOME": str(case / "home"),
        "XDG_CONFIG_HOME": str(case / "config"),
        "XDG_CACHE_HOME": str(case / "cache"),
        "TMPDIR": str(case / "tmp"),
        "TERMINAL_STARTUP_DIAGNOSTICS": "1",
        "TERMINAL_STARTUP_LOG": str(log),
    }
    started = time.monotonic()
    process = subprocess.Popen(
        [str(binary), "--workspace", str(config)], cwd=ROOT, env=env,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True,
    )
    closed = False
    try:
        text = wait_for_stage(process, log, "first-frame-presented", started + 30)
        text = wait_for_stage(process, log, "first-shell-output-rendered", time.monotonic() + 30)
        # Let the just-created shell and its first frame settle before idle sampling.
        time.sleep(2)
        hz = os.sysconf("SC_CLK_TCK")
        previous_ticks = process_ticks(process.pid)
        previous_time = time.monotonic()
        cpu = []
        rss = []
        sample_deadline = previous_time + duration
        while time.monotonic() < sample_deadline:
            remaining = sample_deadline - time.monotonic()
            if remaining <= 0:
                break
            time.sleep(min(0.5, remaining))
            now = time.monotonic()
            ticks = process_ticks(process.pid)
            cpu.append((ticks - previous_ticks) / hz / (now - previous_time) * 100)
            rss.append(rss_kib(process.pid))
            previous_ticks, previous_time = ticks, now
        milestones = {
            name: stage_values(text, name)
            for name in (
                "first-frame-presented", "pty-spawn-complete",
                "first-shell-text-processed", "first-shell-output-rendered",
            )
        }
        close_ms = close_window(process, case, sample)
        closed = True
        return {
            "scenario": "tabs" if tabs else "panes",
            "count": count,
            "sample": sample,
            "idle_seconds": duration,
            "close_to_exit_ms": close_ms,
        "load_average_before": load_before,
            "startup_milestones_ms_from_process_start": milestones,
            "idle_cpu_percent_median": statistics.median(cpu),
            "idle_cpu_percent_p95": percentile(cpu, 0.95),
            "idle_rss_kib_median": statistics.median(rss),
            "idle_rss_kib_p95": percentile(rss, 0.95),
            "idle_rss_kib_range": [min(rss), max(rss)],
            "sample_count": len(cpu),
        "load_average_after": list(os.getloadavg()),
        }
    finally:
        if process.poll() is None:
            if not closed:
                try:
                    close_window(process, case, sample)
                except Exception:
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=5)
        shutil.rmtree(case / "cache", ignore_errors=True)


def self_test():
    assert 'axis = "vertical"' in workspace(3, False)
    assert workspace(3, True).count("[[workspace.tabs]]") == 3
    assert percentile([1, 2, 3, 4], 0.95) == 4
    print("Linux lifecycle runner self-check passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--seconds", type=int, default=20, help="idle sampling seconds per process")
    parser.add_argument("--output", type=Path, default=ROOT / "target/perf/linux-lifecycle")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if sys.platform != "linux":
        parser.error("Linux lifecycle measurements require Linux")
    if args.runs < 3 or args.seconds < 5:
        parser.error("use at least three runs and five idle seconds")
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"release binary not found: {binary}")
    args.output.mkdir(parents=True, exist_ok=True)
    records = []
    for tabs, count in ((False, 1), (False, 3), (True, 3)):
        for sample in range(1, args.runs + 1):
            record = run_sample(binary, count, tabs, args.seconds, sample, args.output)
            records.append(record)
            print(json.dumps(record), flush=True)
    result = {
        "measured_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "host": platform.platform(),
        "cpu_model": next(
            (line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
            "unknown",
        ),
        "runs": args.runs,
        "seconds_per_idle_sample": args.seconds,
        "binary": str(binary),
        "samples": records,
    }
    result_path = args.output / "results.json"
    result_path.write_text(json.dumps(result, indent=2) + "\n")
    print(f"results: {result_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
