#!/usr/bin/env python3
"""Alternate preserved optimized test executables; retain raw output and CSV."""
import argparse
import csv
import os
from pathlib import Path
import re
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("baseline", type=Path)
parser.add_argument("candidate", type=Path)
parser.add_argument("--runs", type=int, default=9)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
env = os.environ.copy()
for key in ("TERMINAL_RENDERER_DIAGNOSTICS", "TERMINAL_STARTUP_DIAGNOSTICS", "TERMINAL_STARTUP_LOG", "TERMINAL_THROUGHPUT_DIAGNOSTICS"):
    env.pop(key, None)
rows = []
for pair in range(args.runs):
    variants = [("baseline", args.baseline), ("candidate", args.candidate)]
    if pair % 2:
        variants.reverse()
    for variant, executable in variants:
        # A fresh wrapper keeps child rusage (including peak RSS) per process.
        wrapper = (
            "import resource,subprocess,sys; "
            "r=subprocess.run(sys.argv[1:]); "
            "u=resource.getrusage(resource.RUSAGE_CHILDREN); "
            "print(f'process user_s={u.ru_utime} system_s={u.ru_stime} maxrss_kib={u.ru_maxrss}'); "
            "sys.exit(r.returncode)"
        )
        result = subprocess.run(
            [sys.executable, "-c", wrapper, str(executable.resolve()), "renderer_workload_baselines",
             "--ignored", "--nocapture", "--test-threads=1"],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        log = result.stdout
        (args.output / f"{pair:02}-{variant}.txt").write_text(log)
        result.check_returncode()
        if 'device_type: IntegratedGpu' not in log:
            raise RuntimeError("Expected native integrated GPU; inspect the adapter before comparing")
        for line in log.splitlines():
            if line.startswith("workload="):
                row = dict(re.findall(r"(\w+)=(\S+)", line))
                rows.append({"pair": pair, "variant": variant, **row})
        print(f"pair={pair} variant={variant} complete", flush=True)
with (args.output / "samples.csv").open("w", newline="") as output:
    writer = csv.DictWriter(output, fieldnames=list(rows[0]))
    writer.writeheader()
    writer.writerows(rows)
