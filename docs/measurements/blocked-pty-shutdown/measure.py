"""Bounded Linux before/after probe runner; only cleans processes it launched."""
import ctypes
import json
import os
from pathlib import Path
import re
import selectors
import signal
import statistics
import subprocess
import sys
import time


def run(binary, label, mode, panes, sample, output_directory):
    marker = output_directory / f"{label}-{mode}-{panes}-{sample}.marker"
    marker.unlink(missing_ok=True)
    process = subprocess.Popen(
        [binary, mode, str(panes)], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, env=os.environ | {"PTY_PROBE_GRACEFUL_MARKER": str(marker)},
    )
    timed_out = False
    prefix = b""
    shutdown_started = False
    try:
        # Start the recovery watchdog at the shutdown marker, excluding startup.
        # Read the descriptor directly so text buffering cannot hide ready lines.
        deadline = time.monotonic() + 5
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while not re.search(rb"^shutdown-start .*\n", prefix, re.M):
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not selector.select(remaining):
                    raise subprocess.TimeoutExpired(process.args, 5)
                chunk = os.read(process.stdout.fileno(), 4096)
                if not chunk:
                    break
                prefix += chunk
        shutdown_started = bool(re.search(rb"^shutdown-start .*\n", prefix, re.M))
        output, error = process.communicate(timeout=2)
        output = prefix.decode() + output
    except subprocess.TimeoutExpired as timeout:
        timed_out = True
        partial = timeout.stdout or b""
        if isinstance(partial, bytes):
            partial = partial.decode()
        partial = prefix.decode() + partial
        children = [int(pid) for pid in re.findall(r"^child=(\d+)$", partial, re.M)]
        assert len(children) == panes, partial
        # A stuck baseline cannot reap its children. This runner adopts them
        # with PR_SET_CHILD_SUBREAPER, kills them, and waits after killing it.
        for pid in children:
            os.kill(pid, signal.SIGKILL)
        process.kill()
        output, error = process.communicate(timeout=5)
        output = prefix.decode() + output
        for pid in children:
            os.waitpid(pid, 0)
            assert not Path(f"/proc/{pid}").exists()
    assert shutdown_started, ("startup failed", output, error)
    expected_timeout = label == "before" and mode in ("blocked", "full", "ignore-hup")
    assert timed_out == expected_timeout, (label, mode, output, error)
    if not timed_out:
        assert process.returncode == 0, (output, error)
        assert "children_gone=" in output, output
        if mode == "graceful":
            assert marker.read_text() == "hup", "graceful SIGHUP handler did not finish"
    (output_directory / f"{label}-{mode}-{panes}-{sample}.log").write_text(output + error)
    duration = re.search(r"elapsed_us=(\d+)", output)
    record = {
        "revision": label, "mode": mode, "panes": panes, "sample": sample,
        "timeout_ms": 2000 if timed_out else None,
        "shutdown_ms": int(duration[1]) / 1000 if duration else None,
        "watchdog_cleanup": timed_out,
    }
    print(json.dumps(record), flush=True)
    return record


def main():
    before, after, destination = sys.argv[1:]
    destination = Path(destination).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0
    records = []
    for mode, panes, count in [("normal", 1, 20), ("graceful", 1, 5), ("blocked", 1, 3), ("full", 1, 3), ("ignore-hup", 1, 3), ("blocked", 3, 3)]:
        for sample in range(count):
            for label, binary in [("before", before), ("after", after)]:
                records.append(run(binary, label, mode, panes, sample, destination))
    summary = []
    for label, mode, panes in sorted({(record["revision"], record["mode"], record["panes"]) for record in records}):
        samples = [record for record in records if (record["revision"], record["mode"], record["panes"]) == (label, mode, panes)]
        values = [record["shutdown_ms"] for record in samples if record["shutdown_ms"] is not None]
        summary.append({"revision": label, "mode": mode, "panes": panes, "samples": len(samples), "timeouts": sum(record["watchdog_cleanup"] for record in samples), "median_ms": statistics.median(values) if values else None, "min_ms": min(values) if values else None, "max_ms": max(values) if values else None})
    (destination / "results.json").write_text(json.dumps({"summary": summary, "samples": records}, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
