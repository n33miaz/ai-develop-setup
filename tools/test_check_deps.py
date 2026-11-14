import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_deps  # noqa: E402

ROOT = "/repo"


def package(name, folder, deps=()):
    return {
        "name": name,
        "manifest_path": "%s/%s/Cargo.toml" % (ROOT, folder),
        "dependencies": [{"name": d, "kind": k} for d, k in deps],
    }


MATRIX = {
    "a-contracts": {"path": "crates/contracts", "allow": []},
    "a-core": {
        "path": "crates/core",
        "allow": ["a-contracts"],
        "forbid_external": ["rusqlite"],
    },
    "a-storage": {"path": "crates/storage", "allow": ["a-contracts"]},
}


def metadata(*packages):
    return {"workspace_root": ROOT, "packages": list(packages)}


def graph(core_deps=()):
    return metadata(
        package("a-contracts", "crates/contracts"),
        package("a-core", "crates/core", core_deps),
        package("a-storage", "crates/storage", [("a-contracts", None)]),
    )


ALL = {"normal", "build", "dev"}


class CheckGraph(unittest.TestCase):
    def test_allowed_edges_pass(self):
        meta = graph([("a-contracts", None), ("serde", None)])
        self.assertEqual(check_deps.check_graph(meta, ALL, MATRIX), [])

    def test_internal_edge_outside_the_matrix_fails(self):
        meta = graph([("a-storage", None)])
        problems = check_deps.check_graph(meta, ALL, MATRIX)
        self.assertEqual(len(problems), 1)
        self.assertIn("a-core -> a-storage", problems[0])

    def test_dev_dependency_counts_by_default(self):
        meta = graph([("a-storage", "dev")])
        self.assertEqual(len(check_deps.check_graph(meta, ALL, MATRIX)), 1)
        self.assertEqual(check_deps.check_graph(meta, {"normal", "build"}, MATRIX), [])

    def test_forbidden_external_library_fails(self):
        meta = graph([("rusqlite", None)])
        problems = check_deps.check_graph(meta, ALL, MATRIX)
        self.assertEqual(len(problems), 1)
        self.assertIn("forbidden external", problems[0])

    def test_crate_missing_from_the_matrix_fails(self):
        meta = graph()
        meta["packages"].append(package("a-new", "crates/new"))
        problems = check_deps.check_graph(meta, ALL, MATRIX)
        self.assertEqual(len(problems), 1)
        self.assertIn("a-new", problems[0])

    def test_matrix_entry_without_crate_fails(self):
        meta = metadata(package("a-contracts", "crates/contracts"))
        problems = check_deps.check_graph(meta, ALL, {k: MATRIX[k] for k in ("a-contracts", "a-core")})
        self.assertEqual(len(problems), 1)
        self.assertIn("a-core", problems[0])

    def test_moved_crate_fails(self):
        meta = metadata(
            package("a-contracts", "crates/moved"),
        )
        problems = check_deps.check_graph(meta, ALL, {"a-contracts": MATRIX["a-contracts"]})
        self.assertEqual(len(problems), 1)
        self.assertIn("crates/moved", problems[0])


class Docs(unittest.TestCase):
    def test_doc_is_deterministic_and_lists_every_crate(self):
        text = check_deps.render_doc(ALL, MATRIX)
        self.assertEqual(text, check_deps.render_doc(ALL, MATRIX))
        for name in MATRIX:
            self.assertIn("`%s`" % name, text)

    def test_modules_map_flags_missing_crate(self):
        self.assertEqual(check_deps.check_modules_map(MATRIX, "`a-contracts` `a-core` `a-storage`"), [])
        self.assertEqual(len(check_deps.check_modules_map(MATRIX, "`a-contracts` `a-core`")), 1)


class RealRepository(unittest.TestCase):
    def test_matrix_file_is_valid(self):
        kinds, crates = check_deps.load_matrix(check_deps.MATRIX)
        self.assertTrue(kinds <= check_deps.KINDS)
        for name, rule in crates.items():
            for dep in rule.get("allow", []):
                self.assertIn(dep, crates, "%s allows unknown crate %s" % (name, dep))
            self.assertNotIn(name, rule.get("allow", []))


if __name__ == "__main__":
    unittest.main()
