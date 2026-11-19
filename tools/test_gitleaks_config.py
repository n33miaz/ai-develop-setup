import re
import tomllib
import unittest
from pathlib import Path


class GitleaksConfigTest(unittest.TestCase):
    def test_websocket_sample_exception_matches_only_the_extracted_value(self):
        config_path = Path(__file__).resolve().parents[1] / ".gitleaks.toml"
        allowlist = tomllib.loads(config_path.read_text(encoding="utf-8"))["allowlist"]
        sample = "dGhlIHNhbXBsZSBub25jZQ" + "=="
        other = "synthetic-value"

        def ignored(secret, line):
            candidate = secret if allowlist["regexTarget"] == "secret" else line
            return any(re.search(pattern, candidate) for pattern in allowlist["regexes"])

        line = f"Sec-WebSocket-Key: {sample} api_key={other}"
        self.assertTrue(ignored(sample, line))
        self.assertFalse(ignored(other, line))
        self.assertFalse(ignored(sample + other, line))


if __name__ == "__main__":
    unittest.main()
