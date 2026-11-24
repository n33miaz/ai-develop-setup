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


WORKSPACE_LINTS = {
    "rust": {"unsafe_code": "forbid"},
    "clippy": {"dbg_macro": "warn", "todo": "warn"},
}


def copied(**changes):
    lints = {table: dict(rules) for table, rules in WORKSPACE_LINTS.items()}
    lints["rust"]["unsafe_code"] = "deny"
    for table, rules in changes.items():
        lints[table] = rules
    return {"lints": lints}


class CheckLints(unittest.TestCase):
    def test_inherited_and_faithful_copies_pass(self):
        manifests = {"a-core": {"lints": {"workspace": True}}, "a-shell": copied()}
        self.assertEqual(check_deps.check_lints(WORKSPACE_LINTS, manifests), [])

    def test_copy_missing_a_workspace_lint_fails(self):
        manifests = {"a-shell": copied(clippy={"dbg_macro": "warn"})}
        problems = check_deps.check_lints(WORKSPACE_LINTS, manifests)
        self.assertEqual(len(problems), 1)
        self.assertIn("a-shell", problems[0])

    def test_copy_missing_a_workspace_table_fails(self):
        manifests = {"a-shell": {"lints": {"rust": {"unsafe_code": "deny"}}}}
        self.assertEqual(len(check_deps.check_lints(WORKSPACE_LINTS, manifests)), 1)

    def test_unsafe_code_cannot_be_relaxed_below_deny(self):
        manifests = {"a-shell": copied(rust={"unsafe_code": "allow"})}
        self.assertEqual(len(check_deps.check_lints(WORKSPACE_LINTS, manifests)), 1)

    def test_crate_without_lints_fails(self):
        self.assertEqual(len(check_deps.check_lints(WORKSPACE_LINTS, {"a-core": {}})), 1)


class RealRepository(unittest.TestCase):
    def test_launcher_cannot_depend_on_core(self):
        _, crates = check_deps.load_matrix(check_deps.MATRIX)
        self.assertNotIn("ads-core", crates["ads-launcher"]["allow"])

    def test_every_crate_follows_the_workspace_lints(self):
        metadata = {
            "workspace_root": str(check_deps.ROOT),
            "packages": [
                {"name": path.parent.name, "manifest_path": str(path)}
                for path in check_deps.ROOT.glob("crates/**/Cargo.toml")
                if "target" not in path.parts and "node_modules" not in path.parts
            ],
        }
        self.assertEqual(check_deps.check_lints(*check_deps.load_manifests(metadata)), [])


    def test_matrix_file_is_valid(self):
        kinds, crates = check_deps.load_matrix(check_deps.MATRIX)
        self.assertTrue(kinds <= check_deps.KINDS)
        for name, rule in crates.items():
            for dep in rule.get("allow", []):
                self.assertIn(dep, crates, "%s allows unknown crate %s" % (name, dep))
            self.assertNotIn(name, rule.get("allow", []))

    def test_launcher_paths_covers_all_internal_deps(self):
        """LAUNCHER_PATHS in ci.yml must cover every internal dep of ads-launcher.

        ads-launcher allows ads-contracts, ads-child-env and ads-private-dir
        (defaults/dependency-matrix.toml). The CI trigger pattern must include the
        source tree for each of those crates plus the top-level contracts/ folder
        (JSON Schemas used by ads-contracts). Without crates/contracts/ and
        contracts/ in LAUNCHER_PATHS the launcher-windows job is not triggered when
        only contracts change, violating the dependency contract.
        """
        import re
        import tomllib

        ci_path = check_deps.ROOT / ".github" / "workflows" / "ci.yml"
        ci_text = ci_path.read_text(encoding="utf-8")

        # Extract the value of LAUNCHER_PATHS from the workflow file.
        m = re.search(r"LAUNCHER_PATHS:\s*'([^']+)'", ci_text)
        self.assertIsNotNone(m, "LAUNCHER_PATHS not found in %s" % ci_path)
        launcher_pattern = m.group(1)

        _, crates = check_deps.load_matrix(check_deps.MATRIX)
        launcher_allow = crates.get("ads-launcher", {}).get("allow", [])

        # Build the set of source paths that must be covered: crate folder + contracts/.
        required_paths = []
        for dep in launcher_allow:
            dep_path = crates[dep]["path"]  # e.g. "crates/contracts"
            required_paths.append(dep_path + "/")
        # The top-level contracts/ folder holds the JSON Schemas consumed by ads-contracts.
        required_paths.append("contracts/")

        missing = []
        for path in required_paths:
            if not re.search(launcher_pattern, path):
                missing.append(path)

        self.assertEqual(
            missing,
            [],
            "LAUNCHER_PATHS does not cover these dependency paths: %s\n"
            "Add them to LAUNCHER_PATHS in .github/workflows/ci.yml." % missing,
        )


if __name__ == "__main__":
    unittest.main()
