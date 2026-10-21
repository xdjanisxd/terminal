import ctypes, os, pathlib, signal, subprocess, sys

# Reap baseline leaks instead of leaving them to the desktop or container init.
assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0
run = subprocess.run([sys.argv[1], *sys.argv[2:]], text=True, capture_output=True)
print(run.stdout, end="")
print(run.stderr, end="", file=sys.stderr)
reaped = 0
resident_kib = 0
for line in run.stdout.splitlines():
    if line.startswith("child="):
        try:
            status = pathlib.Path(f"/proc/{int(line.split('=')[1])}/status").read_text()
        except FileNotFoundError:
            continue
        for field in status.splitlines():
            if field.startswith("VmRSS:"):
                resident_kib += int(field.split()[1])
print(f"surviving_child_rss_sum_kib={resident_kib}")
for line in run.stdout.splitlines():
    if line.startswith("child="):
        pid = int(line.split("=")[1])
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            os.waitpid(pid, 0)
            reaped += 1
        except ChildProcessError:
            pass
print(f"probe_cleanup_reaped={reaped}")
sys.exit(run.returncode)
