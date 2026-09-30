"""Summarize per-launch monotonic durations, never subtract batch medians."""
from collections import defaultdict
from pathlib import Path
import re
import statistics
import sys

PAIRS = {
    "app preparation": ("app", "session-preparation-begin", "pty-create-begin"),
    "shell discovery": ("app", "shell-discovery-begin", "shell-discovery-end"),
    "integration preparation": ("app", "shell-integration-preparation-begin", "shell-integration-preparation-end"),
    "reader/writer acquisition": ("app", "initial-size-sent", "environment-build-begin"),
    "return to prompt": ("app", "child-spawned", "prompt-ready"),
    "PTY creation": ("app", "pty-create-begin", "pty-created"),
    "builder": ("app", "environment-build-begin", "environment-build-end"),
    "spawn boundary": ("app", "child-spawn-requested", "child-spawned"),
    "return to first bytes": ("app", "child-spawned", "first-pty-bytes"),
    "return to worker ready": ("app", "child-spawned", "pty-worker-ready"),
    "first shell output": ("app", "child-spawn-requested", "first-shell-output-rendered"),
    "prompt": ("app", "child-spawn-requested", "prompt-ready"),
    "input pipe": ("native", "input-pipe-begin", "input-pipe-end"),
    "output pipe": ("native", "output-pipe-begin", "output-pipe-end"),
    "pseudoconsole": ("native", "pseudoconsole-begin", "pseudoconsole-end"),
    "lock": ("native", "spawn-lock-begin", "spawn-lock-acquired"),
    "attributes": ("native", "attributes-begin", "attributes-end"),
    "resolution": ("native", "executable-resolution-begin", "executable-resolution-end"),
    "quoting UTF16": ("native", "quoting-utf16-begin", "quoting-utf16-end"),
    "cwd": ("native", "cwd-begin", "cwd-end"),
    "environment UTF16": ("native", "environment-utf16-begin", "environment-utf16-end"),
    "CreateProcessW": ("native", "create-process-begin", "create-process-end"),
    "pre CreateProcessW": ("native", "spawn-lock-begin", "create-process-begin"),
    "post CreateProcessW": ("native", "create-process-end", "backend-spawn-end"),
    "ordinary start": ("ordinary", "start-begin", "start-end"),
    "ordinary exit": ("ordinary", "start-begin", "exit"),
}


def durations(text):
    records = {}
    for line in text.splitlines():
        elapsed = re.search(r"\belapsed_us=(\d+)\b", line)
        stage = re.search(r"\bstage=([\w-]+)", line)
        if not elapsed or not stage:
            continue
        origin = "native" if line.startswith("native-startup ") else "ordinary" if line.startswith("ordinary ") else "app"
        key = origin, stage[1]
        if key in records:
            raise ValueError(f"Duplicate stage (single-pane logs required): {key}")
        records[key] = int(elapsed[1])
    result = {}
    for label, (origin, begin, end) in PAIRS.items():
        if (origin, begin) in records and (origin, end) in records:
            delta = records[origin, end] - records[origin, begin]
            if delta < 0:
                raise ValueError(f"Non-monotonic milestones: {label}")
            result[label] = delta / 1000
    if "spawn boundary" in result and "CreateProcessW" in result:
        result["boundary excluding CreateProcessW"] = result["spawn boundary"] - result["CreateProcessW"]
    return result


def summarize(directory):
    batches = defaultdict(lambda: defaultdict(list))
    for path in sorted(directory.glob("*.log")):
        scenario = path.stem.rsplit("-", 1)[0]
        text = path.read_text(encoding="utf-8-sig")
        native = Path(str(path) + ".native")
        if native.exists():
            text += "\n" + native.read_text(encoding="utf-8")
        for label, value in durations(text).items():
            batches[scenario][label].append(value)
    for scenario, values in sorted(batches.items()):
        print(scenario)
        for label, samples in values.items():
            print(f"  {label}: {statistics.median(samples):.3f} ({min(samples):.3f}–{max(samples):.3f}) ms n={len(samples)}")


if __name__ == "__main__":
    for argument in sys.argv[1:]:
        print(argument)
        summarize(Path(argument))
