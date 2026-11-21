"""Refuse production code that lists the whole process environment.

The daemon and the core read parent values only by the names on the approved list
(`process_env`). Listing every variable and filtering afterwards would keep the
same output, so the behaviour tests cannot see it; this check reads the sources.
Test code is skipped: inline `#[cfg(test)] mod x { ... }` blocks and the files of
out-of-line `#[cfg(test)] mod x;` modules (with their submodule folders).
"""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCANNED = ("crates/daemon/src", "crates/core/src", "crates/launcher/src", "crates/storage/src")

# Ways to list the environment instead of asking for one name.
FORBIDDEN = (
    re.compile(r"\benv\s*::\s*(vars|vars_os|Vars|VarsOs)\b"),
    re.compile(r"\buse\s+std\s*::\s*env\s*::\s*\{[^}]*\b(vars|vars_os|Vars|VarsOs)\b"),
    re.compile(r"\buse\s+std\s*::\s*env\s+as\s+\w+"),
    re.compile(r"\buse\s+std\s*::\s*env\s*::\s*(?:\*|\{[^}]*\*)"),
    re.compile(r"\buse\s+(?:::\s*)?std\s*::\s*env\s*::\s*\{[^}]*\bself\s+as\b"),
    # The same through a braced group of `std` (`use std::{fs, env as e}`, `use std::{env::*}`).
    re.compile(
        r"\buse\s+(?:::\s*)?std\s*::\s*\{[^;]*?\benv\s*(?:as\b|::\s*(?:\*|\{[^}]*(?:\*|\bself\s+as\b|"
        r"\b(?:vars|vars_os|Vars|VarsOs)\b)|(?:vars|vars_os|Vars|VarsOs)\b))"
    ),
    re.compile(r"\bGetEnvironmentStrings\w*"),
    re.compile(r"\benviron\b"),
)

CFG_TEST = re.compile(r"#\[\s*cfg\s*\(\s*test\s*\)\s*\]")
PATH_ATTR = re.compile(r'#\[\s*path\s*=\s*"([^"]+)"\s*\]')
MOD_DECL = re.compile(r"(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*([;{])")


def blank_comments_and_literals(source):
    """Replace comments, strings and char literals with spaces, keeping offsets and lines."""
    out = list(source)
    i, n = 0, len(source)

    def blank(start, end):
        for k in range(start, min(end, n)):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        if source.startswith("//", i):
            end = source.find("\n", i)
            end = n if end < 0 else end
            blank(i, end)
            i = end
        elif source.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if source.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif source.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif (raw := re.match(r'b?r(#*)"', source[i:i + 260])) and (i == 0 or not (source[i - 1].isalnum() or source[i - 1] == "_")):
            close = '"' + raw.group(1)
            end = source.find(close, i + raw.end())
            end = n if end < 0 else end + len(close)
            blank(i, end)
            i = end
        elif source[i] == '"':
            j = i + 1
            while j < n and source[j] != '"':
                j += 2 if source[j] == "\\" else 1
            blank(i, j + 1)
            i = j + 1
        elif source[i] == "'":
            char = re.match(r"'(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'])'", source[i:i + 16], re.S)
            if char:
                blank(i, i + char.end())
                i += char.end()
            else:
                i += 1  # lifetime or label
        else:
            i += 1
    return "".join(out)


def block_end(code, open_brace):
    depth = 0
    for k in range(open_brace, len(code)):
        if code[k] == "{":
            depth += 1
        elif code[k] == "}":
            depth -= 1
            if depth == 0:
                return k + 1
    return len(code)


def module_dir(path):
    """Folder that holds the files of `path`'s child modules."""
    if path.name in ("mod.rs", "lib.rs", "main.rs"):
        return path.parent
    return path.parent / path.stem


def test_modules(path, code, source=None):
    """Return (inline test spans, files of out-of-line test modules) for one file.

    `code` has literals blanked; `source` (same offsets) is read only for `#[path]`.
    """
    source = code if source is None else source
    spans, files = [], []
    for attr in CFG_TEST.finditer(code):
        rest = code[attr.end():]
        lead = re.match(r"(\s*#\[[^\]]*\])*\s*", rest)
        decl = MOD_DECL.match(rest, lead.end())
        if not decl:
            continue
        if decl.group(2) == "{":
            spans.append((attr.start(), block_end(code, attr.end() + decl.end() - 1)))
            continue
        custom = PATH_ATTR.search(source[attr.end():attr.end() + lead.end()])
        if custom:
            target = path.parent / custom.group(1)
        else:
            base = module_dir(path) / decl.group(1)
            target = base.with_suffix(".rs") if base.with_suffix(".rs").exists() else base / "mod.rs"
        files.append(target.resolve())
        files.append(module_dir(target).resolve())
    return spans, files


def production_findings(roots):
    sources = sorted(p for root in roots for p in Path(root).rglob("*.rs"))
    texts = {p: p.read_text(encoding="utf-8") for p in sources}
    codes = {p: blank_comments_and_literals(t) for p, t in texts.items()}
    skipped_files, spans = [], {}
    for path, code in codes.items():
        spans[path], files = test_modules(path, code, texts[path])
        skipped_files.extend(files)

    def is_test_file(path):
        resolved = path.resolve()
        return any(resolved == f or f in resolved.parents for f in skipped_files)

    findings = []
    for path, code in codes.items():
        if is_test_file(path):
            continue
        for pattern in FORBIDDEN:
            for hit in pattern.finditer(code):
                if any(a <= hit.start() < b for a, b in spans[path]):
                    continue
                line = code.count("\n", 0, hit.start()) + 1
                findings.append(f"{path.relative_to(ROOT).as_posix()}:{line}: {hit.group(0)}")
    return findings


class EnvEnumerationTests(unittest.TestCase):
    def test_production_code_does_not_list_the_environment(self):
        findings = production_findings(ROOT / root for root in SCANNED)
        self.assertEqual(findings, [], "read parent values by name (process_env), never list them all")

    def test_detector_sees_listing_outside_tests_only(self):
        code = blank_comments_and_literals(
            'fn a() { let _ = std::env::vars_os(); }\n'
            '// std::env::vars()\n'
            'const S: &str = "env::vars";\n'
            '#[cfg(test)]\nmod tests { fn b() { let c = \'{\'; std::env::vars(); } }\n'
            'fn d() { for _ in env::vars() {} }\n'
        )
        spans, _ = test_modules(Path("x.rs"), code)
        hits = [m.start() for p in FORBIDDEN for m in p.finditer(code)
                if not any(a <= m.start() < b for a, b in spans)]
        lines = sorted(code.count("\n", 0, h) + 1 for h in hits)
        self.assertEqual(lines, [1, 6])

    def test_detector_sees_imported_names(self):
        code = blank_comments_and_literals("use std::env::{var, vars_os};\n")
        self.assertTrue(any(p.search(code) for p in FORBIDDEN))
        code = blank_comments_and_literals("use std::env::{var, var_os};\n")
        self.assertFalse(any(p.search(code) for p in FORBIDDEN))

    def test_detector_rejects_environment_alias_and_glob_imports(self):
        for code in ("use std::env as e; e::vars()", "use std::env::*; vars_os()",
                     "use std::env::{var, *}; vars_os()"):
            with self.subTest(code=code):
                self.assertTrue(any(pattern.search(code) for pattern in FORBIDDEN))

    def test_detector_reads_braced_std_imports(self):
        for code in ("use std::{env as e}; e::vars()", "use std::{fs, env as e};",
                     "use std::{\n    env as e,\n};", "use ::std::{env as e};",
                     "use std::{env::*};", "use std::{env::{var, *}};", "use std::{env::{vars_os}};",
                     "use std::{fs, env::vars};", "use std::{env::{self as e}};",
                     "use std::env::{self as e};"):
            with self.subTest(code=code):
                code = blank_comments_and_literals(code)
                self.assertTrue(any(pattern.search(code) for pattern in FORBIDDEN))
        for code in ("use std::{env, fs};", "use std::{env::{var, var_os}};",
                     "use std::{env::var, fs::*};", "use std::{fs::*, env::var};",
                     "use std::{env::{self}, fs};", "use std::{environment as e};"):
            with self.subTest(code=code):
                code = blank_comments_and_literals(code)
                self.assertFalse(any(pattern.search(code) for pattern in FORBIDDEN))

    def test_out_of_line_test_modules_are_skipped(self):
        launch_mod = ROOT / "crates/core/src/tool_launch/mod.rs"
        _, files = test_modules(launch_mod, blank_comments_and_literals(launch_mod.read_text(encoding="utf-8")))
        self.assertIn((ROOT / "crates/core/src/tool_launch/tests.rs").resolve(), files)
        self.assertIn((ROOT / "crates/core/src/tool_launch/tests").resolve(), files)
        queue = ROOT / "crates/core/src/queue.rs"
        text = queue.read_text(encoding="utf-8")
        _, files = test_modules(queue, blank_comments_and_literals(text), text)
        self.assertIn((ROOT / "crates/core/src/queue_tests.rs").resolve(), files)


if __name__ == "__main__":
    unittest.main()
