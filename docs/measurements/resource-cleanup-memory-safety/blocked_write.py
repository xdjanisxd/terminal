import ctypes, os, signal, subprocess, pathlib
assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0
process = subprocess.Popen(["/tmp/resource-cleanup-audit/blocked-write"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
line = process.stdout.readline()
print(line, end="", flush=True)
pid = int(line.strip().split("=")[1])
try:
    output, error = process.communicate(timeout=2)
    print("shutdown_blocked_for_2s=False")
except subprocess.TimeoutExpired:
    print("shutdown_blocked_for_2s=True")
    print(pathlib.Path(f"/proc/{pid}/status").read_text().splitlines()[:6])
    os.kill(pid, signal.SIGKILL)
    try:
        output, error = process.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        print("still_blocked_after_child_kill=True")
        for path in pathlib.Path(f"/proc/{process.pid}/task").iterdir():
            print(path.name, (path / "wchan").read_text().strip())
        print(pathlib.Path(f"/proc/{pid}/status").read_text().splitlines()[:6] if pathlib.Path(f"/proc/{pid}").exists() else "child-gone")
        process.kill()
        output, error = process.communicate(timeout=5)
print(output, end="")
print(error, end="")
print(f"probe_exit={process.returncode}")
try:
    os.waitpid(pid, 0)
except ChildProcessError:
    pass
