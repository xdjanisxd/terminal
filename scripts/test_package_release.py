"""Check platform manifests and nested Linux archive extraction without a native binary."""

from pathlib import Path
import os
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import package_release as release


class PackagingTests(unittest.TestCase):
    def test_platform_assets_and_linux_round_trip(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "terminal"
            binary.write_bytes(b"test executable")
            linux = release.archive_members(binary, "linux")
            app_id = "io.github.xdjanisxd.terminal"
            desktop = f"share/applications/{app_id}.desktop"
            icons = {
                f"share/icons/hicolor/{size}x{size}/apps/{app_id}.png"
                for size in (16, 24, 32, 48, 64, 128, 256, 512)
            }
            self.assertIn(desktop, linux)
            self.assertEqual(set(linux) - {"terminal", "README.md", "config.example.toml", "LICENSE"},
                             {desktop, *icons})
            for platform in ("windows", "macos"):
                members = release.archive_members(binary, platform)
                self.assertEqual(len(members), 4)
                self.assertFalse(any(name.startswith("share/") for name in members))

            archive = root / "terminal-linux-x86_64.tar.gz"
            release.write_tar_gz(archive, linux)
            first = archive.read_bytes()
            release.write_tar_gz(archive, linux)
            self.assertEqual(archive.read_bytes(), first)
            with tarfile.open(archive, "r:gz") as contents:
                self.assertEqual(set(contents.getnames()), set(linux))
                self.assertEqual(contents.getmember("terminal").mode, 0o755)
                self.assertEqual(contents.extractfile(desktop).read(), linux[desktop])
                for icon in icons:
                    self.assertEqual(contents.extractfile(icon).read(), linux[icon])
            if os.name == "nt":
                # Windows chmod cannot represent POSIX execute permissions.
                # Unix CI additionally exercises the nested smoke extraction.
                return

            def smoke_command(command, **options):
                extracted = Path(options["cwd"])
                self.assertEqual((extracted / desktop).read_bytes(), linux[desktop])
                for icon in icons:
                    self.assertEqual((extracted / icon).read_bytes(), linux[icon])
                self.assertEqual(Path(command[0]).name, "terminal")
                self.assertEqual(command[1:], ["--help"])
                return subprocess.CompletedProcess(command, 0, "Usage: terminal [OPTIONS]\n")

            with patch.object(release.subprocess, "run", side_effect=smoke_command) as run:
                release.smoke_archive(archive, linux, False)
                run.assert_called_once()
            self.assertFalse(list(root.glob(".terminal-smoke-*")))


if __name__ == "__main__":
    unittest.main()
