"""Deterministic diagnostic parser/overlay guards; no performance thresholds."""
import importlib.util
from pathlib import Path
import unittest
import tempfile
import os
import sys
sys.dont_write_bytecode = True


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


trace = load("trace", "prepare-native-spawn-trace.py")
summary = load("summary", "summarize-native-spawn.py")


class TraceTests(unittest.TestCase):
    def test_backend_anchor_must_be_unique(self):
        self.assertEqual(trace.replace_once("before anchor after", "anchor", "stamp"), "before stamp after")
        for source in ("missing", "anchor anchor"):
            with self.assertRaises(ValueError):
                trace.replace_once(source, "anchor", "stamp")

    def test_generated_windows_boundary_order_and_single_spawn(self):
        registry = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "registry/src"
        sources = list(registry.glob("*/portable-pty-0.9.0"))
        if len(sources) != 1:
            self.skipTest("requires the cached portable-pty 0.9.0 used by the diagnostic build")
        original = (sources[0] / "src/win/psuedocon.rs").read_bytes()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "trace"
            trace.prepare(sources[0], output)
            generated = (output / "portable-pty/src/win/psuedocon.rs").read_text(encoding="utf-8")
            milestones = [
                'let mut environment = cmd.environment_block();',
                'super::trace("environment-utf16-end");',
                'super::trace("create-process-begin");',
                'CreateProcessW(',
                'super::trace("create-process-end");',
                'if res == 0 {',
                'super::trace("handles-owned");',
            ]
            positions = [generated.index(stage) for stage in milestones]
            self.assertEqual(positions, sorted(positions))
            self.assertEqual(generated.count("CreateProcessW("), 1)
            with self.assertRaises(FileExistsError):
                trace.prepare(sources[0], output)
        self.assertEqual((sources[0] / "src/win/psuedocon.rs").read_bytes(), original)

    def test_formats_and_independent_clock_origins(self):
        values = summary.durations("""terminal-startup stage=child-spawn-requested session=PaneId(1) elapsed_us=10000
terminal-startup stage=child-spawned session=PaneId(1) elapsed_us=15000
native-startup elapsed_us=200 stage=create-process-begin
native-startup elapsed_us=4200 stage=create-process-end
ordinary elapsed_us=0 stage=start-begin
ordinary elapsed_us=3000 stage=start-end""")
        self.assertEqual(values["spawn boundary"], 5)
        self.assertEqual(values["CreateProcessW"], 4)
        self.assertEqual(values["ordinary start"], 3)

    def test_incomplete_pair_is_not_zero(self):
        self.assertEqual(summary.durations("native-startup elapsed_us=200 stage=create-process-begin"), {})

    def test_rejects_duplicate_or_reversed_milestones(self):
        for text in (
            "native-startup elapsed_us=1 stage=create-process-begin\nnative-startup elapsed_us=2 stage=create-process-begin",
            "native-startup elapsed_us=2 stage=create-process-begin\nnative-startup elapsed_us=1 stage=create-process-end",
        ):
            with self.assertRaises(ValueError):
                summary.durations(text)


if __name__ == "__main__":
    unittest.main()
