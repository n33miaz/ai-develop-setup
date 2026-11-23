"""Keep Windows path filters in sync with workspace dependencies."""

import re
import shlex
import textwrap
import unittest

from check_deps import ROOT, load_metadata, relative_dir


WINDOWS_MODULE = re.compile(r"(?:^|/)(?:win|windows)(?:\.rs$|/)")
UNSAFE_CODE = re.compile(r"\bunsafe\s*(?:\{|fn\b|impl\b|extern\b)")
RUN_KEY = re.compile(r"^( *)(- )?run:(?: +(.*))?$")
ASSIGNMENT = re.compile(r"^[A-Za-z_]\w*=")
SHELL_TOKEN = re.compile(
    r"(?P<separator>\n|[;&|]+)|[ \t]+|\\\n|#[^\n]*|"
    r"""(?P<word>(?:\\[\s\S]|'[^'\n]*'|"(?:\\[^\n]|[^"\\\n])*"|[^ \t\n;&|<>()'"\\\x60])+)""",
)


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


def windows_jobs(workflow):
    """The text of each job of `workflow` that runs on Windows."""
    jobs = re.split(r"^  [\w-]+:\n", workflow.split("\njobs:\n", 1)[1], flags=re.M)
    return [job for job in jobs if "    runs-on: windows-latest\n" in job]


def run_commands(job):
    """Read plain and block run scalars from conventional Windows job steps."""
    lines = job.splitlines()
    at = 0
    while at < len(lines):
        key = RUN_KEY.match(lines[at])
        at += 1
        if not key or not key[3]:
            continue
        value = key[3].strip()
        block = re.fullmatch(r"([|>])[-+]?(?:\s+#.*)?", value)
        indent = len(key[1]) + len(key[2] or "")
        body = []
        while at < len(lines):
            line = lines[at]
            width = len(line) - len(line.lstrip(" "))
            if line.strip() and width <= indent:
                break
            body.append(line)
            at += 1
        if not block:
            if value.startswith(("|", ">", "'", '"', "*", "&", "!", "{", "[", "%", "@", "`")):
                raise ValueError(f"unsupported YAML run scalar: {value}")
            if any(line.strip() and not line.lstrip().startswith("#") for line in body):
                raise ValueError("unsupported YAML run scalar: multiline plain text")
            yield from simple_commands(re.split(r"\s+#", value, maxsplit=1)[0])
            continue

        body = textwrap.dedent("\n".join(body)).split("\n")
        if block[1] == ">":
            if any(line.startswith((" ", "\t")) for line in body):
                raise ValueError("unsupported YAML run scalar: indented content in folded block")
            # Each empty line in a folded block preserves one line break.
            script = re.sub(
                r"\n+", lambda breaks: " " if len(breaks[0]) == 1 else breaks[0][1:],
                "\n".join(body).strip("\n"),
            )
            yield from simple_commands(script)
        else:
            yield from simple_commands("\n".join(body))


def simple_commands(script):
    """Tokenize simple shell commands before removing their quotes."""
    command = []
    at = 0
    while at < len(script):
        token = SHELL_TOKEN.match(script, at)
        if not token:
            raise ValueError(f"unsupported shell syntax in YAML run: {script[at:]!r}")
        at = token.end()
        if token.lastgroup == "separator":
            if command:
                yield command
                command = []
        elif token.lastgroup == "word":
            command.extend(shlex.split(token[0].replace("\\\n", ""), comments=False))
    if command:
        yield command


def cargo_commands(job):
    """`(subcommand, packages)` of each Cargo command `job` executes: `cargo` has to be the
    program, after any `NAME=value` assignments, never an argument of another one."""
    found = []
    for words in run_commands(job):
        while words and ASSIGNMENT.match(words[0]):
            words = words[1:]
        if not words or words[0] != "cargo":
            continue
        args = words[1:]
        if args and args[0].startswith("+"):
            args = args[1:]
        if not args or not re.fullmatch(r"\w+", args[0]):
            continue
        if "--" in args:
            args = args[: args.index("--")]
        packages = {args[i + 1] for i in range(1, len(args) - 1) if args[i] == "-p"}
        found.append((args[0], packages))
    return found


def missing_windows_cargo(workflow, crates):
    """`(crate, command)` pairs of `crates` that no Windows job lints (`clippy`) or tests (`test`)."""
    found = {
        (name, command)
        for job in windows_jobs(workflow)
        for command, names in cargo_commands(job)
        for name in names
    }
    return sorted((crate, command) for crate in crates for command in ("clippy", "test") if (crate, command) not in found)


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

    def test_only_executed_commands_count(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        crates = {"ads-launcher", "ads-private-dir"}
        self.assertEqual(missing_windows_cargo(workflow, crates), [])
        active = "-p ads-launcher -p ads-private-dir"
        self.assertEqual(workflow.count(active), 2)
        steps = "\n".join(
            [
                "      # - run: cargo clippy --locked -p ads-private-dir --all-targets -- -D warnings",
                "      - name: Comentados",
                "        run: echo ok # cargo test --locked -p ads-private-dir",
                "      - run: |",
                "          # cargo clippy --locked -p ads-private-dir --all-targets",
                "          echo ok  # cargo test --locked -p ads-private-dir",
                "",
            ]
        )
        anchor = "        run: cargo test --locked -p ads-launcher -j 2\n"
        mutated = workflow.replace(active, "-p ads-launcher")
        self.assertEqual(mutated.count(anchor), 1)
        mutated = mutated.replace(anchor, anchor + steps)
        self.assertEqual(
            missing_windows_cargo(mutated, crates),
            [("ads-private-dir", "clippy"), ("ads-private-dir", "test")],
        )
        job = "\n".join(
            [
                "    steps:",
                "      - run: |",
                "          cargo test --locked \\",
                "            -p ads-private-dir",
                "          echo done && cargo clippy -p ads-launcher # -p ads-private-dir",
            ]
        )
        self.assertEqual(
            cargo_commands(job),
            [("test", {"ads-private-dir"}), ("clippy", {"ads-launcher"})],
        )
        printed = "\n".join(
            [
                "    steps:",
                "      - run: echo cargo test -p ads-private-dir",
                "      - run: |",
                "          echo cargo clippy -p ads-private-dir",
                '          echo "done; cargo test -p ads-private-dir"',
                "          printf '%s' 'x && cargo clippy -p ads-private-dir'",
                "          CARGO_TERM_COLOR=always cargo test -p ads-launcher | tee out.txt",
                "          cargo test -p ads-launcher -- -p ads-private-dir",
            ]
        )
        self.assertEqual(
            cargo_commands(printed),
            [("test", {"ads-launcher"}), ("test", {"ads-launcher"})],
        )

    def test_folded_run_is_one_command(self):
        job = """    steps:
      - run: >-
          cargo test --locked
          -p ads-private-dir

          cargo clippy -p ads-launcher
        """
        self.assertEqual(
            cargo_commands(job), [("test", {"ads-private-dir"}), ("clippy", {"ads-launcher"})]
        )

    def test_shell_quotes_survive_comments_and_operators(self):
        job = """    steps:
      - run: |
          echo "label # text" && cargo test -p ads-private-dir # -p ignored
          echo "&&" cargo clippy -p ignored
          cargo test -p ads-launcher \\
          # -p ignored
        """
        self.assertEqual(
            cargo_commands(job), [("test", {"ads-private-dir"}), ("test", {"ads-launcher"})]
        )

    def test_unsupported_run_scalar_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unsupported YAML run"):
            cargo_commands("      - run: *shared_script")

    def test_filters_cover_transitive_workspace_dependencies(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        metadata = load_metadata()
        packages = {pkg["name"]: pkg for pkg in metadata["packages"]}
        jobs = windows_jobs(workflow)
        names = {re.search(r"^    name: ([\w-]+)$", job, re.M)[1] for job in jobs}
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
        crates = windows_unsafe_crates(packages, metadata) | {"ads-launcher"}
        self.assertEqual(missing_windows_cargo(workflow, crates), [])

        shell_job = next(job for job in jobs if "    name: shell-check\n" in job)
        self.assertRegex(
            shell_job,
            r"(?m)^        run: cargo clippy --locked -p ads-shell --all-targets -j 2 -- -D warnings$",
        )

        for job in jobs:
            output = re.search(r"needs\.changes\.outputs\.(\w+) == 'true'", job)
            self.assertIsNotNone(output, job)
            variable = f"{output[1].upper()}_PATHS"
            with self.subTest(filter=variable):
                pattern = re.search(rf"^ +{variable}: '([^']+)'$", workflow, re.M)
                self.assertIsNotNone(pattern)
                path_filter = re.compile(pattern[1])
                pending = set().union(*(names for _, names in cargo_commands(job)))
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
