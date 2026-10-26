#!/usr/bin/env python3
"""Repeat existing Linux release microbenchmarks and optionally check regressions."""

import argparse
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
from datetime import datetime, timezone


ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "docs/measurements/linux-performance-baseline/release-cpu.json"
DEFAULT_OUTPUT = ROOT / "target/perf/linux-performance-check.json"
COMMANDS = {
    "state_baselines": ["cargo", "bench", "-q", "-p", "terminal-core", "--bench", "state_baselines"],
    "projection_baselines": ["cargo", "bench", "-q", "-p", "terminal-renderer", "--bench", "projection_baselines"],
}
GATES = {
    "parse_plain_state": 15,
    "parse_escape_state": 15,
    "resize_one_column_unicode_pair": 30,
    "resize_columns_pair_history_2000": 25,
    "resize_columns_pair_history_10000": 15,
    "visible_projection_80x30": 50,
    "output_echo_plus_projection": 50,
    "three_pane_layout_plus_projection": 25,
}
MEASUREMENT = re.compile(
    r"^([a-z0-9_]+): operations=(\d+) elapsed_ns=(\d+) ns_per_operation=([\d.]+)$",
    re.MULTILINE,
)


def percentile(values, percent):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(percent * len(ordered)) - 1)]


def summary(values):
    median = statistics.median(values)
    mad = statistics.median(abs(value - median) for value in values)
    return {
        "samples_ns_per_operation": values,
        "median_ns_per_operation": median,
        "p95_ns_per_operation": percentile(values, 0.95),
        "mad_ns_per_operation": mad,
        "cv_percent": statistics.pstdev(values) / statistics.mean(values) * 100,
    }


def cpu_model():
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unknown"


def environment():
    os_release = {}
    try:
        for line in Path("/etc/os-release").read_text().splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                os_release[key] = value.strip('"')
    except OSError:
        pass
    rustc = subprocess.run(["rustc", "-Vv"], cwd=ROOT, check=True, text=True, capture_output=True).stdout
    cargo = subprocess.run(["cargo", "-V"], cwd=ROOT, check=True, text=True, capture_output=True).stdout.strip()
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, text=True, capture_output=True).stdout.strip()
    return {
        "os": os_release.get("PRETTY_NAME", platform.platform()),
        "kernel": platform.release(),
        "architecture": platform.machine(),
        "cpu_model": cpu_model(),
        "rustc": rustc.splitlines()[0],
        "rustc_details": rustc.strip(),
        "cargo": cargo,
        "revision": revision,
        "profile": "Cargo bench release profile",
        "cpu_affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "load_average": list(os.getloadavg()) if hasattr(os, "getloadavg") else None,
    }


def run_benchmarks(runs):
    samples = {}
    for command in COMMANDS.values():
        for _ in range(runs):
            result = subprocess.run(command, cwd=ROOT, check=True, text=True, capture_output=True)
            for name, _operations, _elapsed, value in MEASUREMENT.findall(result.stdout + result.stderr):
                samples.setdefault(name, []).append(float(value))
    if not samples:
        raise RuntimeError("No benchmark records found; inspect cargo bench output.")
    if any(len(values) != runs for values in samples.values()):
        raise RuntimeError(f"Incomplete repeated benchmark samples: { {name: len(values) for name, values in samples.items()} }")
    return {name: summary(values) for name, values in samples.items()}


def compare(current, baseline, current_environment, baseline_environment):
    for key in ("os", "kernel", "architecture", "cpu_model", "rustc", "cargo", "cpu_affinity"):
        if current_environment.get(key) != baseline_environment.get(key):
            raise RuntimeError(
                f"Environment mismatch for {key}: baseline={baseline_environment.get(key)!r}, "
                f"current={current_environment.get(key)!r}; rerun without --check for a new baseline."
            )
    failures = []
    for name, minimum_percent in GATES.items():
        before = baseline["benchmarks"].get(name)
        after = current.get(name)
        if before is None or after is None:
            raise RuntimeError(f"Missing gated benchmark {name}.")
        floor = before["median_ns_per_operation"] * minimum_percent / 100
        noise = 3 * before["mad_ns_per_operation"]
        limit = before["median_ns_per_operation"] + max(floor, noise)
        if after["median_ns_per_operation"] > limit:
            failures.append((name, after["median_ns_per_operation"], limit))
    return failures


def self_test():
    assert percentile([4, 1, 3, 2], 0.95) == 4
    assert summary([1.0, 2.0, 3.0])["median_ns_per_operation"] == 2.0
    assert summary([1.0, 2.0, 3.0])["mad_ns_per_operation"] == 1.0
    print("benchmark runner self-check passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--check", action="store_true", help="compare medians with the checked-in baseline")
    parser.add_argument("--baseline", type=Path, default=BASELINE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.runs < 3:
        parser.error("--runs must be at least 3")
    if sys.platform != "linux":
        parser.error("Linux benchmarks require Linux")

    report = {
        "schema": 1,
        "measured_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "runs": args.runs,
        "environment": environment(),
        "benchmarks": run_benchmarks(args.runs),
        "gate_thresholds_percent": GATES,
    }
    report["environment_after"] = environment()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"measurements: {args.output}")
    for name, result in report["benchmarks"].items():
        print(
            f"{name}: median={result['median_ns_per_operation']:.1f} ns/op "
            f"p95={result['p95_ns_per_operation']:.1f} cv={result['cv_percent']:.1f}%"
        )
    if args.check:
        baseline = json.loads(args.baseline.read_text())
        failures = compare(report["benchmarks"], baseline, report["environment"], baseline["environment"])
        if failures:
            for name, measured, limit in failures:
                print(f"REGRESSION {name}: median={measured:.1f} ns/op limit={limit:.1f} ns/op", file=sys.stderr)
            return 1
        print("no gated regressions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
