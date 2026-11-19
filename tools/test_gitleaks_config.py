import json
import os
import re
import shutil
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / ".gitleaks.toml"
# The samples are assembled at runtime so the repository scan never sees them as literals.
RFC_SAMPLE = "dGhlIHNhbXBsZSBub25jZQ" + "=="
ULID = "01HZX3" + "QK7RMB9TNVW2CJ5D8FGA"
SYNTHETIC = "q8Vt2LmX" + "9rZp4KwN7sYb3HdJ"


def gitleaks_binary():
    return os.environ.get("GITLEAKS_BIN") or shutil.which("gitleaks")


class GitleaksConfigTest(unittest.TestCase):
    def test_websocket_sample_exception_matches_only_the_extracted_value(self):
        allowlist = tomllib.loads(CONFIG.read_text(encoding="utf-8"))["allowlist"]
        sample = RFC_SAMPLE
        other = "synthetic-value"

        def ignored(secret, line):
            candidate = secret if allowlist["regexTarget"] == "secret" else line
            return any(re.search(pattern, candidate) for pattern in allowlist["regexes"])

        line = f"Sec-WebSocket-Key: {sample} api_key={other}"
        self.assertTrue(ignored(sample, line))
        self.assertFalse(ignored(other, line))
        self.assertFalse(ignored(sample + other, line))


@unittest.skipUnless(
    gitleaks_binary() or os.environ.get("GITLEAKS_REQUIRED") == "1",
    "gitleaks not installed; the secrets CI job sets GITLEAKS_REQUIRED=1 and runs this",
)
class GitleaksBinaryTest(unittest.TestCase):
    """Runs the real scanner over generated samples, with the repository configuration and without it."""

    def scan(self, files, config):
        binary = gitleaks_binary()
        self.assertIsNotNone(binary, "GITLEAKS_REQUIRED=1 but gitleaks is not available")
        with tempfile.TemporaryDirectory() as work:
            source = Path(work) / "samples"
            source.mkdir()
            for name, text in files.items():
                (source / name).write_text(text, encoding="utf-8")
            report = Path(work) / "report.json"
            result = subprocess.run(
                [binary, "dir", str(source), "--config", str(config), "--no-banner",
                 "--exit-code", "0", "--report-format", "json", "--report-path", str(report),
                 "--log-level", "error"],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            findings = json.loads(report.read_text(encoding="utf-8") or "[]")
            return sorted((Path(f["File"]).name, f["Secret"]) for f in findings)

    def scan_with_defaults_only(self, files):
        with tempfile.TemporaryDirectory() as work:
            config = Path(work) / "defaults.toml"
            config.write_text("[extend]\nuseDefault = true\n", encoding="utf-8")
            return self.scan(files, config)

    def test_identifier_exception_hides_only_ulid_idempotency_keys(self):
        files = {
            "command.json": '{"params": {"_meta": {"idempotency_key": "%s", "origin": "local"}}}\n' % ULID,
            "near-miss.json": '{"params": {"_meta": {"idempotency_key": "%s"}}}\n' % (ULID + "Q"),
            "secret.json": '{"api_key": "%s"}\n' % SYNTHETIC,
        }
        # Control: without the exception the default rule flags the identifier.
        control = self.scan_with_defaults_only(files)
        self.assertIn(("command.json", ULID), control)
        self.assertEqual(
            self.scan(files, CONFIG),
            [("near-miss.json", ULID + "Q"), ("secret.json", SYNTHETIC)],
        )

    def test_websocket_sample_exception_keeps_other_secrets_on_the_same_line(self):
        files = {"handshake.txt": f"Sec-WebSocket-Key: {RFC_SAMPLE}\napi_key = \"{SYNTHETIC}\" Sec-WebSocket-Key: {RFC_SAMPLE}\n"}
        self.assertEqual(self.scan(files, CONFIG), [("handshake.txt", SYNTHETIC)])


if __name__ == "__main__":
    unittest.main()
