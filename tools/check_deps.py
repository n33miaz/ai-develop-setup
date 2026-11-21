"""Dependency rule between workspace crates (P2).

Reads `cargo metadata --no-deps` and compares it with `defaults/dependency-matrix.toml`.
Also requires every crate to inherit the workspace lints, or to copy them with only
`unsafe_code` relaxed to "deny" (docs/unsafe.md), so a new workspace lint reaches every crate.

  python tools/check_deps.py check        # rule + lints + modules map + generated doc up to date
  python tools/check_deps.py write-doc    # regenerate docs/dependency-matrix.md from the matrix

Exit code 0 when everything holds, 1 on any violation (one `VIOLATION:` line each), 2 on usage errors.
"""

import argparse
import json
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MATRIX = ROOT / "defaults" / "dependency-matrix.toml"
DOC = ROOT / "docs" / "dependency-matrix.md"
MODULES = ROOT / "docs" / "modules.md"
KINDS = {"normal", "build", "dev"}


def load_matrix(path):
    with open(path, "rb") as fh:
        data = tomllib.load(fh)
    kinds = set(data.get("settings", {}).get("kinds", sorted(KINDS)))
    if not kinds <= KINDS:
        raise SystemExit("settings.kinds has an unknown kind: %s" % sorted(kinds - KINDS))
    return kinds, data.get("crates", {})


def load_metadata(path=None):
    if path:
        with open(path, encoding="utf-8") as fh:
            return json.load(fh)
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(out.stdout)


def relative_dir(manifest_path, root):
    folder = Path(manifest_path).resolve().parent
    return folder.relative_to(Path(root).resolve()).as_posix()


def check_graph(metadata, kinds, crates):
    """Return the list of violation messages for a metadata document and a matrix."""
    problems = []
    members = {p["name"]: p for p in metadata["packages"]}
    root = metadata["workspace_root"]

    for name in sorted(set(members) - set(crates)):
        problems.append("%s is a workspace crate with no entry in the matrix" % name)
    for name in sorted(set(crates) - set(members)):
        problems.append("%s is in the matrix but is not a workspace crate" % name)

    for name, pkg in sorted(members.items()):
        rule = crates.get(name)
        if rule is None:
            continue
        declared = rule.get("path")
        actual = relative_dir(pkg["manifest_path"], root)
        if declared is not None and declared != actual:
            problems.append("%s lives in %s but the matrix says %s" % (name, actual, declared))
        allow = set(rule.get("allow", []))
        forbid = set(rule.get("forbid_external", []))
        for dep in pkg["dependencies"]:
            kind = dep.get("kind") or "normal"
            if kind not in kinds:
                continue
            dep_name = dep["name"]
            if dep_name in members:
                if dep_name not in allow:
                    problems.append(
                        "%s -> %s (%s dependency not allowed; allowed: %s)"
                        % (name, dep_name, kind, ", ".join(sorted(allow)) or "nothing internal")
                    )
            elif dep_name in forbid:
                problems.append("%s -> %s (%s dependency on a forbidden external library)" % (name, dep_name, kind))
    return problems


def check_lints(workspace_lints, manifests):
    """Return violations for crates whose `[lints]` differ from the workspace tables.

    `manifests` maps a crate name to its parsed Cargo.toml. A crate either inherits
    (`[lints] workspace = true`) or copies every table, with `unsafe_code` at most relaxed to "deny".
    """
    problems = []
    for name, manifest in sorted(manifests.items()):
        lints = manifest.get("lints")
        if lints == {"workspace": True}:
            continue
        if not isinstance(lints, dict) or "workspace" in lints:
            problems.append("%s neither inherits the workspace lints nor copies them" % name)
            continue
        own = {table: dict(rules) for table, rules in lints.items()}
        expected = {table: dict(rules) for table, rules in workspace_lints.items()}
        unsafe = own.get("rust", {}).pop("unsafe_code", None)
        expected.get("rust", {}).pop("unsafe_code", None)
        if unsafe not in ("deny", "forbid"):
            problems.append("%s sets unsafe_code to %r; only deny or forbid are allowed" % (name, unsafe))
        if own != expected:
            problems.append(
                "%s copies the workspace lints but differs from them; copy [workspace.lints] again" % name
            )
    return problems


def load_manifests(metadata):
    workspace = Path(metadata["workspace_root"]) / "Cargo.toml"
    with open(workspace, "rb") as fh:
        workspace_lints = tomllib.load(fh).get("workspace", {}).get("lints", {})
    manifests = {}
    for package in metadata["packages"]:
        with open(package["manifest_path"], "rb") as fh:
            manifests[package["name"]] = tomllib.load(fh)
    return workspace_lints, manifests


def check_modules_map(crates, modules_text):
    return [
        "%s has no entry in docs/modules.md" % name
        for name in sorted(crates)
        if "`%s`" % name not in modules_text
    ]


def render_doc(kinds, crates):
    reverse = {name: [] for name in crates}
    for name, rule in crates.items():
        for dep in rule.get("allow", []):
            if dep in reverse:
                reverse[dep].append(name)
    lines = [
        "# Matriz de dependências",
        "",
        "Gerado por `python tools/check_deps.py write-doc` a partir de `defaults/dependency-matrix.toml`. "
        "Não edite à mão: mude a matriz e regenere.",
        "",
        "Regra (P2): cada crate só importa os crates internos da coluna \"Pode depender de\"; "
        "o que não está lá é proibido. O teste lê `cargo metadata` e reprova a diferença; "
        "o CI roda `python tools/check_deps.py check` em todo PR. "
        "Tipos de dependência que contam: %s." % ", ".join("`%s`" % k for k in sorted(kinds)),
        "",
        "| Crate | Pasta | Pode depender de | Quem pode depender dele | Bibliotecas externas proibidas |",
        "|---|---|---|---|---|",
    ]
    for name in sorted(crates):
        rule = crates[name]
        allow = ", ".join("`%s`" % a for a in sorted(rule.get("allow", []))) or "nada interno"
        users = ", ".join("`%s`" % u for u in sorted(reverse[name])) or "ninguém"
        forbid = ", ".join("`%s`" % f for f in sorted(rule.get("forbid_external", []))) or "nenhuma listada"
        lines.append("| `%s` | `%s` | %s | %s | %s |" % (name, rule.get("path", ""), allow, users, forbid))
    lines += [
        "",
        "Para permitir uma dependência nova, edite a matriz no mesmo PR que a introduz e justifique no corpo do PR.",
        "",
    ]
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["check", "write-doc"])
    parser.add_argument("--matrix", type=Path, default=MATRIX)
    parser.add_argument("--metadata", type=Path, help="read this cargo metadata JSON instead of running cargo")
    parser.add_argument("--doc", type=Path, default=DOC)
    parser.add_argument("--modules", type=Path, default=MODULES)
    args = parser.parse_args(argv)

    kinds, crates = load_matrix(args.matrix)
    if args.command == "write-doc":
        args.doc.write_text(render_doc(kinds, crates), encoding="utf-8", newline="\n")
        print("wrote %s" % args.doc)
        return 0

    metadata = load_metadata(args.metadata)
    problems = check_graph(metadata, kinds, crates)
    if not args.metadata:
        problems += check_lints(*load_manifests(metadata))
    problems += check_modules_map(crates, args.modules.read_text(encoding="utf-8"))
    current = args.doc.read_text(encoding="utf-8") if args.doc.exists() else ""
    if current != render_doc(kinds, crates):
        problems.append("%s is out of date; run: python tools/check_deps.py write-doc" % args.doc.name)
    for problem in problems:
        print("VIOLATION: %s" % problem)
    if problems:
        print("dependency rule: %d violation(s)" % len(problems))
        return 1
    print("dependency rule: ok (%d crates)" % len(crates))
    return 0


if __name__ == "__main__":
    sys.exit(main())
