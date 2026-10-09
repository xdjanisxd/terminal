"""Exercise launcher escaping, ownership boundaries, and user install lifecycle."""

import os
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import install_linux as installer


@unittest.skipUnless(sys.platform == "linux", "Linux installer")
class InstallationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.home = root / 'home space "quote" $dollar `tick` \\slash'
        self.home.mkdir()
        self.data = root / "alternate data"
        self.binary = root / "binary"
        self.binary.write_text('#!/bin/sh\nprintf "%s" "$PWD" > "$HOME/launched"\n')
        self.binary.chmod(0o755)
        self.environment = patch.dict(os.environ, {"HOME": str(self.home),
                                                  "XDG_DATA_HOME": str(self.data)})
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.refresh = patch.object(installer, "refresh")
        self.refresh.start()
        self.addCleanup(self.refresh.stop)

    def install(self):
        installer.install("install", self.binary)

    def test_launcher_and_lifecycle_preserve_user_files(self):
        self.install()
        paths = installer.destinations(self.home, self.data)
        unrelated = paths[2].parent / "unrelated.png"
        unrelated.write_bytes(b"keep")
        config = self.home / ".config/terminal/config.toml"
        config.parent.mkdir(parents=True)
        config.write_text("# keep config")
        if shutil.which("desktop-file-validate"):
            subprocess.run(["desktop-file-validate", str(paths[1])], check=True)
        if shutil.which("gio"):
            environment = os.environ.copy()
            environment["PATH"] = "/usr/bin:/bin"
            subprocess.run(["gio", "launch", str(paths[1])], cwd="/",
                           env=environment, check=True)
            # gio starts asynchronously; wait for its concrete launch marker.
            import time
            for _ in range(100):
                if (self.home / "launched").exists():
                    break
                time.sleep(0.01)
            self.assertEqual((self.home / "launched").read_text(), str(self.home))
        self.binary.write_text("#!/bin/sh\nexit 0\n")
        self.install()
        self.assertEqual(paths[0].read_bytes(), self.binary.read_bytes())
        installer.install("uninstall")
        self.assertFalse(any(path.exists() for path in paths))
        self.assertEqual(unrelated.read_bytes(), b"keep")
        self.assertEqual(config.read_text(), "# keep config")
        installer.install("uninstall")

    def test_conflict_stops_before_any_removal_or_update(self):
        self.install()
        paths = installer.destinations(self.home, self.data)
        paths[-1].write_bytes(b"user modified")
        for action in ("install", "uninstall"):
            with self.assertRaisesRegex(ValueError, "modified"):
                installer.install(action, self.binary)
        self.assertTrue(all(path.exists() for path in paths))

    def test_unrelated_executable_and_symlink_are_preserved(self):
        target = self.home / ".local/bin/terminal"
        target.parent.mkdir(parents=True)
        target.write_bytes(b"unrelated")
        with self.assertRaisesRegex(ValueError, "unrelated"):
            self.install()
        target.unlink()
        target.symlink_to(self.binary)
        with self.assertRaisesRegex(ValueError, "unrelated"):
            self.install()
        self.assertTrue(target.is_symlink())

    def test_invalid_ownership_record_cannot_authorize_symlink_removal(self):
        self.install()
        paths = installer.destinations(self.home, self.data)
        record = self.data / "terminal/desktop-install.json"
        owned = json.loads(record.read_text())
        paths[0].unlink()
        paths[0].symlink_to(self.binary)
        owned[str(paths[0])] = None
        record.write_text(json.dumps(owned))
        with self.assertRaisesRegex(ValueError, "invalid installation ownership"):
            installer.install("uninstall")
        self.assertTrue(paths[0].is_symlink())
        self.assertTrue(paths[1].exists())

    def test_missing_binary_has_no_installation_side_effects(self):
        with self.assertRaisesRegex(ValueError, "missing executable"):
            installer.install("install", self.home / "missing")
        self.assertFalse(self.data.exists())

    def test_relative_xdg_is_ignored(self):
        with patch.dict(os.environ, {"XDG_DATA_HOME": "relative"}):
            self.install()
            self.assertTrue((self.home / ".local/share/applications" /
                             f"{installer.APP_ID}.desktop").exists())
            installer.install("uninstall")

    def test_forbidden_executable_path_is_rejected_before_writes(self):
        for name in ("a=b", "a%b", "a\nb", "a\rb"):
            with self.assertRaisesRegex(ValueError, "must not contain"):
                installer.exec_value(f"/home/{name}/.local/bin/terminal")


if __name__ == "__main__":
    unittest.main()
