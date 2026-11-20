"""Keep Windows path filters in sync with workspace dependencies."""

import re
import unittest

from check_deps import ROOT, load_metadata, relative_dir


class WindowsPathFiltersTest(unittest.TestCase):
    def test_filters_cover_transitive_workspace_dependencies(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        metadata = load_metadata()
        packages = {pkg["name"]: pkg for pkg in metadata["packages"]}
        jobs = re.split(r"^  [\w-]+:\n", workflow.split("\njobs:\n", 1)[1], flags=re.M)
        windows_jobs = [job for job in jobs if "    runs-on: windows-latest\n" in job]
        names = {re.search(r"^    name: ([\w-]+)$", job, re.M)[1] for job in windows_jobs}
        self.assertEqual(
            names,
            {
                "shell-check",
                "pty-windows",
                "fs-windows",
                "storage-windows",
                "vault-windows",
                "launcher-windows",
            },
        )
        required = re.search(r"^  ci-ok:\n(?:    .*\n)*?    needs: \[([^\]]+)\]", workflow, re.M)
        self.assertIsNotNone(required)
        self.assertLessEqual(names, {name.strip() for name in required[1].split(",")})

        # O lançador precisa de um job Windows que passe o lint e os testes no crate.
        windows_cargo = [
            line for job in windows_jobs for line in job.splitlines() if re.search(r"\bcargo (clippy|test) ", line)
        ]
        for command in ("clippy", "test"):
            with self.subTest(launcher=command):
                self.assertTrue(
                    any(f"cargo {command} " in line and " -p ads-launcher " in f"{line} " for line in windows_cargo),
                    f"ads-launcher has no Windows cargo {command}",
                )

        for job in windows_jobs:
            output = re.search(r"needs\.changes\.outputs\.(\w+) == 'true'", job)
            self.assertIsNotNone(output, job)
            variable = f"{output[1].upper()}_PATHS"
            with self.subTest(filter=variable):
                pattern = re.search(rf"^ +{variable}: '([^']+)'$", workflow, re.M)
                self.assertIsNotNone(pattern)
                path_filter = re.compile(pattern[1])
                cargo_commands = "\n".join(
                    line for line in job.splitlines() if re.match(r"\s*(?:run: )?cargo ", line)
                )
                pending = set(re.findall(r" -p ([\w-]+)", cargo_commands))
                self.assertTrue(pending, f"No Cargo packages found for {variable}")
                visited = set()
                while pending:
                    name = pending.pop()
                    visited.add(name)
                    package = packages[name]
                    folder = relative_dir(package["manifest_path"], metadata["workspace_root"])
                    for path in (f"{folder}/Cargo.toml", f"{folder}/src/lib.rs"):
                        self.assertRegex(path, path_filter, f"{variable} misses {name}")
                    pending.update(
                        dep["name"]
                        for dep in package["dependencies"]
                        if dep.get("path") and dep["name"] in packages and dep["name"] not in visited
                    )
                if output[1] == "launcher":
                    self.assertRegex("defaults/launcher.toml", path_filter)
                if "ads-contracts" in visited:
                    self.assertRegex("contracts/schemas/example.json", path_filter)
