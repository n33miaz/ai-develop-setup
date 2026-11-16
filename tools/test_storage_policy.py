"""Check storage documentation and CI obligations without creating fixtures."""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class StoragePolicyTests(unittest.TestCase):
    def test_build_dependencies_are_documented(self):
        adr = next((ROOT / "docs/adr").glob("0021*")).read_text(encoding="utf-8")
        for term in ("pkg-config", "vcpkg", "compilação", "inertes", "bundled"):
            self.assertIn(term, adr)

    def test_storage_decision_has_reversal_and_cross_reference(self):
        adr = next((ROOT / "docs/adr").glob("0021*")).read_text(encoding="utf-8")
        self.assertIn("## Condição de reversão", adr)
        self.assertIn("synchronous=FULL", adr)
        previous = next((ROOT / "docs/adr").glob("0004*")).read_text(encoding="utf-8")
        self.assertIn("[ADR 0021]", previous)
        unsafe = (ROOT / "docs/unsafe.md").read_text(encoding="utf-8")
        self.assertLess(unsafe.index("| `ads-storage`"), unsafe.index("| demais"))

    def test_change_summary_includes_storage(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        summary = next(line for line in workflow.splitlines() if 'echo "shell-check roda:' in line)
        self.assertIn("storage-windows roda: $storage", summary)


if __name__ == "__main__":
    unittest.main()
