"""Keep Windows path filters in sync with workspace dependencies."""

import re
import unittest

from check_deps import ROOT, load_metadata, relative_dir


WINDOWS_MODULE = re.compile(r"(?:^|/)(?:win|windows)(?:\.rs$|/)")
UNSAFE_CODE = re.compile(r"\bunsafe\s*(?:\{|fn\b|impl\b|extern\b)")


def windows_unsafe_crates(packages, metadata):
    """Workspace crates with a Windows module (`win.rs`, `windows.rs` or such a folder) that uses `unsafe`."""
    found = set()
    for name, package in packages.items():
        folder = ROOT / relative_dir(package["manifest_path"], metadata["workspace_root"])
        for source in folder.rglob("*.rs"):
            module = source.relative_to(folder).as_posix()
            if WINDOWS_MODULE.search(module) and UNSAFE_CODE.search(source.read_text(encoding="utf-8")):
                found.add(name)
                break
    return found


class WindowsPathFiltersTest(unittest.TestCase):
    def test_windows_unsafe_detection_sees_the_known_modules(self):
        metadata = load_metadata()
        packages = {pkg["name"]: pkg for pkg in metadata["packages"]}
        crates = windows_unsafe_crates(packages, metadata)
        self.assertLessEqual({"ads-private-dir", "ads-adapter-fs", "ads-adapter-vault", "ads-ptyhost"}, crates)
        self.assertNotIn("ads-core", crates)
        for module in ("src/platform/win.rs", "src/sys/windows.rs", "src/windows/handle.rs"):
            self.assertTrue(WINDOWS_MODULE.search(module), module)
        for module in ("src/window.rs", "src/winget.rs", "src/darwin.rs"):
            self.assertFalse(WINDOWS_MODULE.search(module), module)

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

        # O lançador e todo crate com um módulo Windows que usa `unsafe` passam o lint e os testes
        # num job Windows: no Linux esse código nem compila.
        windows_cargo = [
            line for job in windows_jobs for line in job.splitlines() if re.search(r"\bcargo (clippy|test) ", line)
        ]
        for crate in sorted(windows_unsafe_crates(packages, metadata) | {"ads-launcher"}):
            for command in ("clippy", "test"):
                with self.subTest(crate=crate, command=command):
                    self.assertTrue(
                        any(f"cargo {command} " in line and f" -p {crate} " in f"{line} " for line in windows_cargo),
                        f"{crate} has no Windows cargo {command}",
                    )

        shell_job = next(job for job in windows_jobs if "    name: shell-check\n" in job)
        self.assertRegex(
            shell_job,
            r"(?m)^        run: cargo clippy --locked -p ads-shell --all-targets -j 2 -- -D warnings$",
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
                    self.assertRegex(f"{folder}/Cargo.toml", path_filter, f"{variable} misses {name}")
                    for source in (ROOT / folder).rglob("*.rs"):
                        self.assertRegex(source.relative_to(ROOT).as_posix(), path_filter, f"{variable} misses {name}")
                        for embedded in re.findall(
                            r'\binclude_(?:str|bytes)!\s*\(\s*"([^"]+\.toml)"',
                            source.read_text(encoding="utf-8"),
                        ):
                            path = (source.parent / embedded).resolve()
                            if path.is_relative_to(ROOT / "defaults"):
                                self.assertRegex(
                                    path.relative_to(ROOT).as_posix(), path_filter, f"{variable} misses {name}"
                                )
                    pending.update(
                        dep["name"]
                        for dep in package["dependencies"]
                        if dep.get("path") and dep["name"] in packages and dep["name"] not in visited
                    )
                if "ads-contracts" in visited:
                    self.assertRegex("contracts/schemas/example.json", path_filter)
                if variable == "SHELL_PATHS":
                    # A janela empacota a interface e o teste dela roda pelo script de fumaça.
                    for path in ("ui/src/main.ts", "tools/smoke_desktop.py"):
                        self.assertRegex(path, path_filter, f"{variable} misses {path}")
                    self.assertNotRegex("tools/smoke_desktop.py.bak", path_filter)
