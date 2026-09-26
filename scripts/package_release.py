"""Create and smoke-test one native, portable release archive."""

import argparse
import gzip
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tarfile
import uuid
import zipfile


TARGETS = {
    "x86_64-pc-windows-msvc": ("windows", "x86_64", ".zip"),
    "aarch64-pc-windows-msvc": ("windows", "aarch64", ".zip"),
    "x86_64-unknown-linux-gnu": ("linux", "x86_64", ".tar.gz"),
    "aarch64-unknown-linux-gnu": ("linux", "aarch64", ".tar.gz"),
    "x86_64-apple-darwin": ("macos", "x86_64", ".tar.gz"),
    "aarch64-apple-darwin": ("macos", "aarch64", ".tar.gz"),
}


def native_target(target: str) -> None:
    result = subprocess.run(
        ["rustc", "-vV"], check=True, capture_output=True, text=True
    )
    host = next(line.removeprefix("host: ") for line in result.stdout.splitlines()
                if line.startswith("host: "))
    if host != target:
        raise RuntimeError(f"release smoke requires a native runner: host={host}, target={target}")


def archive_members(binary: Path, windows: bool) -> dict[str, bytes]:
    root = Path(__file__).resolve().parent.parent
    files = {
        "terminal.exe" if windows else "terminal": binary,
        "README.md": root / "README.md",
        "config.example.toml": root / "config.example.toml",
        "LICENSE": root / "LICENSE",
    }
    for name, path in files.items():
        if not path.is_file() or not path.stat().st_size:
            raise FileNotFoundError(f"missing or empty release file: {name}: {path}")
    return {name: path.read_bytes() for name, path in files.items()}


def write_zip(path: Path, files: dict[str, bytes]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED,
                         compresslevel=9) as archive:
        for name, data in files.items():
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = ((0o755 if name == "terminal.exe" else 0o644) << 16)
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED,
                             compresslevel=9)


def write_tar_gz(path: Path, files: dict[str, bytes]) -> None:
    import io

    with path.open("wb") as output:
        with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                for name, data in files.items():
                    info = tarfile.TarInfo(name)
                    info.size = len(data)
                    info.mode = 0o755 if name == "terminal" else 0o644
                    info.mtime = 0
                    archive.addfile(info, io.BytesIO(data))


def smoke_archive(path: Path, files: dict[str, bytes], windows: bool) -> None:
    extracted = path.parent / f".terminal-smoke-{uuid.uuid4().hex}"
    extracted.mkdir()
    try:
        if windows:
            with zipfile.ZipFile(path) as archive:
                if set(archive.namelist()) != set(files):
                    raise RuntimeError("ZIP contents differ from release manifest")
                for name in files:
                    expected_mode = 0o755 if name == "terminal.exe" else 0o644
                    if archive.getinfo(name).external_attr >> 16 != expected_mode:
                        raise RuntimeError(f"incorrect ZIP member mode: {name}")
                    (extracted / name).write_bytes(archive.read(name))
        else:
            with tarfile.open(path, "r:gz") as archive:
                if set(archive.getnames()) != set(files):
                    raise RuntimeError("tar contents differ from release manifest")
                for name in files:
                    entry = archive.getmember(name)
                    expected_mode = 0o755 if name == "terminal" else 0o644
                    if not entry.isfile() or entry.mode != expected_mode:
                        raise RuntimeError(f"incorrect tar member type or mode: {name}")
                    member = archive.extractfile(entry)
                    if member is None:
                        raise RuntimeError(f"missing archive member: {name}")
                    (extracted / name).write_bytes(member.read())
                    (extracted / name).chmod(entry.mode)

        for name, expected in files.items():
            if (extracted / name).read_bytes() != expected:
                raise RuntimeError(f"archive member changed: {name}")
        executable = extracted / ("terminal.exe" if windows else "terminal")
        if not windows and not executable.stat().st_mode & stat.S_IXUSR:
            raise RuntimeError("packaged executable is not executable")
        env = os.environ.copy()
        env.pop("TERMINAL_CONFIG", None)
        env["APPDATA" if windows else "XDG_CONFIG_HOME"] = str(extracted / "no-config")
        result = subprocess.run([str(executable), "--help"], cwd=extracted,
                                env=env, capture_output=True, text=True,
                                timeout=30, check=True)
        if not result.stdout.startswith("Usage: terminal "):
            raise RuntimeError(f"unexpected packaged binary smoke output: {result.stdout!r}")
    finally:
        shutil.rmtree(extracted)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=TARGETS)
    parser.add_argument("--output-dir", type=Path, default=Path("dist"))
    args = parser.parse_args()
    native_target(args.target)
    system, architecture, extension = TARGETS[args.target]
    windows = system == "windows"
    binary = Path("target") / args.target / "release" / (
        "terminal.exe" if windows else "terminal"
    )
    files = archive_members(binary, windows)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    archive = args.output_dir / f"terminal-{system}-{architecture}{extension}"
    if windows:
        write_zip(archive, files)
    else:
        write_tar_gz(archive, files)
    smoke_archive(archive, files, windows)
    print(f"Packaged and smoke-tested {archive}")


if __name__ == "__main__":
    main()
