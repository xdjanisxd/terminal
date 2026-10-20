#!/usr/bin/env python3
"""Print median per-logical-frame CPU stages and paired generation changes."""
import csv
from pathlib import Path
import statistics
import sys

rows = list(csv.DictReader(Path(sys.argv[1]).open()))
print("| Workload | Generation before µs | After µs | Change | Faster pairs | Wall before µs | After µs |")
print("| --- | ---: | ---: | ---: | ---: | ---: | ---: |")
for name in dict.fromkeys(row["workload"] for row in rows):
    groups = [[row for row in rows if row["workload"] == name and row["variant"] == variant]
              for variant in ("baseline", "candidate")]
    def med(group, field):
        return statistics.median(int(row[field]) / int(row["frames"]) / 1000 for row in group)
    before, after = [med(group, "generation_ns") for group in groups]
    pairs = {row["pair"]: int(row["generation_ns"]) for row in groups[0]}
    faster = sum(int(row["generation_ns"]) < pairs[row["pair"]] for row in groups[1])
    walls = [med(group, "wall_ns") for group in groups]
    print(f"| {name} | {before:.2f} | {after:.2f} | {(after/before-1)*100:.1f}% | {faster}/{len(pairs)} | {walls[0]:.2f} | {walls[1]:.2f} |")
