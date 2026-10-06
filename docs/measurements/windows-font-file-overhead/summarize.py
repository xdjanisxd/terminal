"""Archive native harness results; regenerate statistics from the archived CSV."""
import argparse
import csv
import json
import re
import statistics
from collections import defaultdict
from pathlib import Path


def stats(values):
    return {"median": statistics.median(values), "min": min(values), "max": max(values)}


def archive(source, destination):
    rows, environments = [], {}
    batches = [("baseline", source / "baseline-ps7"),
               ("attribution-v1", source / "attribution"),
               ("consolas-attribution", source / "consolas-attribution"),
               ("final-registry", source / "final-registry")]
    batches += [(f"paired-{pair}-{mode}", source / f"paired-{pair}-{mode}")
                for pair in range(1, 11) for mode in ("on", "off")]
    for batch, directory in batches:
        environments[batch] = json.loads((directory / "environment.json").read_text(encoding="utf-8-sig"))
        with (directory / "costs.csv").open(encoding="utf-8-sig", newline="") as stream:
            costs = defaultdict(dict)
            for cost in csv.DictReader(stream):
                costs[cost["run"]][cost["stage"]] = int(cost["duration_us"])
        with (directory / "samples.csv").open(encoding="utf-8-sig", newline="") as stream:
            for sample in csv.DictReader(stream):
                assert sample["forced_cleanup"] == "False" and sample["exit_code"] == "0", sample
                row = {"batch": batch, **sample, **costs[sample["run"]]}
                row["cpu-font-preparation"] = sum(costs[sample["run"]][stage]
                                                  for stage in ("font-family-preparation", "font-metrics"))
                row["gpu-preparation"] = sum(costs[sample["run"]][stage]
                                             for stage in ("wgpu-instance", "adapter-request", "device-request"))
                report = directory / f"run-{sample['run']}.fontdb"
                if report.exists():
                    for match in re.finditer(r"(fontdb|memmap)-profile stage=(\S+) duration_us=(\d+) count=(\d+)", report.read_text()):
                        prefix, stage, duration, count = match.groups()
                        row[f"{prefix}.{stage}.us"] = int(duration)
                        row[f"{prefix}.{stage}.count"] = int(count)
                    paths = (directory / f"run-{sample['run']}.paths").read_text().splitlines()
                    assert len(paths) == 657 and len(set(paths)) == 657
                    row["system-font-paths"] = sum(path.lower().startswith("c:\\windows\\fonts\\") for path in paths)
                    row["user-font-paths"] = len(paths) - row["system-font-paths"]
                    row["file-operations.us"] = sum(row[f"fontdb.{stage}.us"]
                                                    for stage in ("file-open", "file-map", "file-unmap-close"))
                rows.append(row)
    fields = sorted(set().union(*(row.keys() for row in rows)))
    with (destination / "runs.csv").open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fields)
        writer.writeheader()
        writer.writerows(rows)
    (destination / "environment.json").write_text(json.dumps(environments, indent=2) + "\n")


def summarize(destination):
    with (destination / "runs.csv").open(newline="") as stream:
        rows = list(csv.DictReader(stream))
    groups = defaultdict(list)
    for row in rows:
        batch = row["batch"]
        groups["paired-on" if batch.startswith("paired-") and batch.endswith("-on") else
               "paired-off" if batch.startswith("paired-") and batch.endswith("-off") else batch].append(row)
    summary = {}
    excluded = {"run", "exit_code", "descendants", "close_ms"}
    for group, samples in groups.items():
        result = {"runs": len(samples)}
        for field in samples[0]:
            if field not in excluded:
                values = [float(row[field]) for row in samples if row[field] and row[field].replace(".", "", 1).isdigit()]
                if values:
                    result[field] = stats(values)
        summary[group] = result
    paired = {}
    for field in ("cpu-font-preparation", "font-family-preparation", "system-font-discovery", "fonts_to_frame_us"):
        deltas = []
        for pair in range(1, 11):
            on = next(row for row in rows if row["batch"] == f"paired-{pair}-on")
            off = next(row for row in rows if row["batch"] == f"paired-{pair}-off")
            deltas.append(int(on[field]) - int(off[field]))
        paired[field] = stats(deltas)
    summary["profiling-on-minus-off-paired-us"] = paired
    (destination / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="Original ignored native harness directory")
    args = parser.parse_args()
    destination = Path(__file__).resolve().parent
    if args.archive:
        archive(args.archive, destination)
    summarize(destination)
