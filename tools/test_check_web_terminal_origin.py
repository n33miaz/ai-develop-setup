import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check_web_terminal_origin.py").resolve()
ORIGIN = "http://127.0.0.1:5173"


class CheckWebTerminalOrigin(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        scratch = Path(__file__).resolve().parents[1] / ".test-tmp"
        scratch.mkdir(exist_ok=True)
        root = Path(tempfile.mkdtemp(prefix="terminal-origin-", dir=scratch))
        cls.script = root / "tools" / SCRIPT.name
        cls.defaults = root / "defaults" / "daemon.toml"
        cls.config = root / "config.toml"
        cls.script.parent.mkdir()
        cls.defaults.parent.mkdir()
        shutil.copyfile(SCRIPT, cls.script)

    def setUp(self):
        self.defaults.write_text(f'ui_base_url = "{ORIGIN}"', encoding="utf-8")
        self.config.write_text("", encoding="utf-8")

    def run_check(self, *args):
        return subprocess.run(
            [sys.executable, "-I", self.script, *args], capture_output=True, text=True
        )

    def assert_error(self, message, *args):
        result = self.run_check(*args)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("Traceback", result.stderr)
        self.assertIn(message, result.stderr)

    def test_argument_count(self):
        for args in ([], [self.config], [self.config, ORIGIN, "extra"]):
            with self.subTest(args=args):
                self.assert_error("Usage: check_web_terminal_origin.py <config_path> <expected_origin>", *args)

    def test_missing_config(self):
        self.assert_error("Could not read the daemon TOML", self.config.with_name("missing.toml"), ORIGIN)

    def test_invalid_config(self):
        self.config.write_text("ui_base_url = [", encoding="utf-8")
        self.assert_error("Could not read the daemon TOML", self.config, ORIGIN)

    def test_missing_default_origin(self):
        self.defaults.write_text("", encoding="utf-8")
        self.assert_error("ui_base_url", self.config, ORIGIN)

    def test_default_origin(self):
        result = self.run_check(self.config, ORIGIN)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_matching_configured_origin(self):
        self.defaults.write_text('ui_base_url = "http://localhost:5174"', encoding="utf-8")
        for origin in (ORIGIN, ORIGIN + "/"):
            with self.subTest(origin=origin):
                self.config.write_text(f'ui_base_url = "{origin}"', encoding="utf-8")
                result = self.run_check(self.config, ORIGIN)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_mismatched_origin(self):
        self.config.write_text('ui_base_url = "http://localhost:5174"', encoding="utf-8")
        self.assert_error(f"Set ui_base_url to {ORIGIN}", self.config, ORIGIN)
