import contextlib
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release  # noqa: E402

CONFIG = {
    "enabled": "on",
    "base_version": "0.1.0",
    "channel": "dev",
    "tag_prefix": "v",
    "keep": 3,
    "prune": "apply",
    "wait_attempts": 6,
    "wait_interval_seconds": 5,
}
SHA = "abc1234def5678abc1234def5678abc1234def56"


def sha_of(tag):
    return hashlib.sha1(tag.encode()).hexdigest()


def pre(tag, prerelease=True, draft=False, release_id=None):
    if release_id is None:
        release_id = int(sha_of(tag)[:6], 16) + 1
    return {"id": release_id, "tagName": tag, "isPrerelease": prerelease, "isDraft": draft}


def dev(*pairs):
    return ["v0.1.0-dev.%s.%d" % (date, n) for date, n in pairs]


class ShippedConfigTest(unittest.TestCase):
    def test_shipped_defaults_are_valid(self):
        config = release.load_config()
        self.assertEqual(config["base_version"], "0.1.0")
        self.assertEqual(config["keep"], 10)


class ValidateConfigTest(unittest.TestCase):
    def bad(self, **change):
        with self.assertRaises(release.ConfigError):
            release.validate_config(dict(CONFIG, **change))

    def test_rejects_wrong_values(self):
        self.bad(enabled=True)
        self.bad(base_version="0.1")
        self.bad(base_version="01.0.0")
        self.bad(base_version="0.1.0-dev")
        self.bad(channel="Dev")
        self.bad(channel="1dev")
        self.bad(channel="de.v")
        self.bad(tag_prefix="release-")
        self.bad(keep=0)
        self.bad(keep=101)
        self.bad(keep=True)
        self.bad(keep="10")
        self.bad(prune="yes")
        for invalid in (0, 11, True, "3"):
            self.bad(wait_attempts=invalid)
        for invalid in (0, 16, False, 1.5):
            self.bad(wait_interval_seconds=invalid)

    def test_rejects_unknown_and_missing_keys(self):
        with self.assertRaises(release.ConfigError):
            release.validate_config(dict(CONFIG, extra=1))
        partial = dict(CONFIG)
        del partial["keep"]
        with self.assertRaises(release.ConfigError):
            release.validate_config(partial)

    def test_unreadable_file_is_a_config_error(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "release.toml"
            path.write_text("enabled = ", encoding="utf-8")
            with self.assertRaises(release.ConfigError):
                release.load_config(path)


class NextReleaseTest(unittest.TestCase):
    def nxt(self, releases=(), tags=(), merged=(), points_at=(), date="20261120", sha=SHA):
        return release.next_release(CONFIG, sha, date, list(releases), list(tags), list(merged), list(points_at))

    def test_first_release_of_the_day(self):
        result = self.nxt()
        self.assertEqual(result["publish"], "yes")
        self.assertEqual(result["tag"], "v0.1.0-dev.20261120.1")
        self.assertEqual(result["version"], "0.1.0-dev.20261120.1+abc1234")
        self.assertEqual(result["previous"], "")

    def test_counts_within_the_day_over_all_tags(self):
        tags = dev(("20261120", 1), ("20261120", 9), ("20261120", 10), ("20261119", 42)) + [
            "v0.1.0-rc.20261120.50",
            "v0.0.9-dev.20261120.70",
            "0.1.0-dev.20261120.80",
            "v0.1.0-dev.20261120.81-extra",
            "dev-20261120-abc1234",
        ]
        self.assertEqual(self.nxt(tags=tags)["tag"], "v0.1.0-dev.20261120.11")

    def test_new_day_restarts_the_counter(self):
        self.assertEqual(self.nxt(tags=dev(("20261119", 4)))["tag"], "v0.1.0-dev.20261120.1")

    def test_previous_is_the_newest_reachable_published_prerelease(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        releases = [pre(tags[0]), pre(tags[1], draft=True), pre(tags[2])]
        merged = tags[:2] + [tags[3], "v0.1.0-rc.20261121.1", "v9.0.0"]
        result = self.nxt(releases=releases, tags=tags, merged=merged)
        self.assertEqual(result["tag"], "v0.1.0-dev.20261120.5")
        self.assertEqual(result["previous"], "v0.1.0-dev.20261120.1")

    def test_commit_with_a_published_prerelease_is_not_published_again(self):
        tag = "v0.1.0-dev.20261120.2"
        result = self.nxt(releases=[pre(tag)], tags=[tag], points_at=["v0.0.1", tag])
        self.assertEqual(result, {"publish": "no", "existing": tag})

    def test_tags_without_published_release_do_not_block(self):
        tag = "v0.1.0-dev.20261120.2"
        for releases in ([], [pre(tag, draft=True)], [pre(tag, prerelease=False)]):
            result = self.nxt(releases=releases, tags=[tag], points_at=[tag, "v0.1.0-rc.20261120.1"])
            self.assertEqual(result["publish"], "yes")
            self.assertEqual(result["tag"], "v0.1.0-dev.20261120.3")

    def test_rejects_bad_commit_and_date(self):
        for sha in ("ABC1234", "abc12", "zzzzzzz", ""):
            with self.assertRaises(ValueError):
                self.nxt(sha=sha)
        for date in ("2026112", "20261340", "2026-11-20"):
            with self.assertRaises(ValueError):
                self.nxt(date=date)


class PrunePlanTest(unittest.TestCase):
    def plan(self, releases, refs=None, require="", require_sha=""):
        if refs is None:
            refs = [(item["tagName"], sha_of(item["tagName"])) for item in releases if "tagName" in item]
        return release.prune_plan(CONFIG, releases, refs, require, require_sha)

    def test_keeps_the_newest_by_version_order(self):
        tags = dev(("20261120", 10), ("20261120", 9), ("20261119", 1), ("20261120", 2), ("20261118", 5))
        targets, orphans = self.plan([pre(tag, release_id=n) for n, tag in enumerate(tags, 1)])
        self.assertEqual(
            targets,
            [
                (3, "v0.1.0-dev.20261119.1", sha_of("v0.1.0-dev.20261119.1")),
                (5, "v0.1.0-dev.20261118.5", sha_of("v0.1.0-dev.20261118.5")),
            ],
        )
        self.assertEqual(orphans, [])

    def test_never_touches_stable_draft_or_foreign_releases(self):
        keep = dev(("20261120", 3), ("20261120", 4), ("20261120", 5))
        old = dev(("20261120", 1))
        guarded = [
            pre("v0.1.0-dev.20261120.2", prerelease=False),
            pre("v0.1.0-dev.20261119.1", draft=True),
            pre("v0.1.0"),
            pre("v0.1.0-rc.20260101.1"),
            pre("something-else"),
            {"id": 9, "isPrerelease": True, "isDraft": False},
        ]
        targets, orphans = self.plan([pre(tag) for tag in keep + old] + guarded)
        self.assertEqual([tag for _, tag, _ in targets], old)
        self.assertEqual(orphans, [])

    def test_excludes_releases_with_non_boolean_flags(self):
        old, *keep = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        kept_releases = [pre(tag) for tag in keep]
        for flag, candidate in (
            ("isPrerelease", pre(old, prerelease=1, release_id=7)),
            ("isDraft", pre(old, draft=0, release_id=7)),
        ):
            with self.subTest(flag=flag):
                targets, _ = self.plan([candidate, *kept_releases])
                self.assertEqual(targets, [])

    def test_tags_without_release_are_only_reported(self):
        keep = dev(("20261120", 3), ("20261120", 4), ("20261120", 5))
        refs = [(tag, sha_of(tag)) for tag in keep + dev(("20261120", 1), ("20261120", 9)) + ["v0.0.1"]]
        targets, orphans = self.plan([pre(tag) for tag in keep], refs=refs)
        self.assertEqual(targets, [])
        self.assertEqual(orphans, ["v0.1.0-dev.20261120.9", "v0.1.0-dev.20261120.1"])

    def test_release_whose_tag_is_gone_is_still_a_target(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        refs = [(tag, sha_of(tag)) for tag in tags[1:]]
        releases = [pre(tags[0], release_id=7)] + [pre(tag) for tag in tags[1:]]
        targets, _ = self.plan(releases, refs=refs)
        self.assertEqual(targets, [(7, tags[0], "-")])

    def test_nothing_to_prune_at_or_below_keep(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3))
        self.assertEqual(self.plan([pre(tag) for tag in tags]), ([], []))

    def test_required_tag_absent_from_listing_reports_channel_count(self):
        tag, missing = dev(("20261120", 1), ("20261120", 2))
        releases = [pre(tag), pre("v0.1.0-rc.20261120.1")]
        with self.assertRaises(ValueError) as error:
            self.plan(releases, refs=[(missing, SHA)], require=missing, require_sha=SHA)
        self.assertIn(missing, str(error.exception))
        self.assertIn("tag ausente da listagem", str(error.exception))
        self.assertIn("releases do canal lidas: 1", str(error.exception))

    def test_required_tag_outside_kept_reports_listed_flags(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        for draft, prerelease, flags in (
            (False, True, "draft=false, prerelease=true"),
            (True, False, "draft=true, prerelease=false"),
            ("invalid\nflag", None, "draft=desconhecido, prerelease=desconhecido"),
        ):
            with self.subTest(draft=draft, prerelease=prerelease):
                releases = [pre(tags[0], draft=draft, prerelease=prerelease)] + [pre(tag) for tag in tags[1:]]
                with self.assertRaises(ValueError) as error:
                    self.plan(releases, require=tags[0], require_sha=sha_of(tags[0]))
                message = str(error.exception)
                self.assertIn(tags[0], message)
                self.assertIn(f"tag presente na listagem ({flags})", message)
                self.assertIn("releases do canal lidas: 4", message)
                self.assertNotIn("\n", message)

    def test_required_release_must_be_published_kept_and_on_its_commit(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        new = tags[3]
        releases = [pre(tag) for tag in tags]
        with self.assertRaises(ValueError):
            self.plan([pre(tag) for tag in tags[:3]] + [pre(new, draft=True)], require=new, require_sha=sha_of(new))
        with self.assertRaises(ValueError):
            self.plan(releases, require=new, require_sha=SHA)
        missing_ref = [(tag, sha_of(tag)) for tag in tags[:3]]
        with self.assertRaises(ValueError):
            self.plan(releases, refs=missing_ref, require=new, require_sha=sha_of(new))
        targets, _ = self.plan(releases, require=new, require_sha=sha_of(new))
        self.assertEqual([tag for _, tag, _ in targets], [tags[0]])

    def test_rejects_release_without_numeric_id(self):
        for bad in ("7", None):
            item = pre(dev(("20261120", 1))[0])
            item["id"] = bad
            with self.assertRaises(ValueError):
                self.plan([item])

    def test_rejects_published_dev_release_with_boolean_id(self):
        oldest, *kept = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        for release_id in (False, True):
            with self.subTest(release_id=release_id):
                releases = [pre(oldest, release_id=release_id)] + [pre(tag) for tag in kept]
                with self.assertRaises(ValueError) as error:
                    self.plan(releases)
                self.assertEqual(str(error.exception), f"release {oldest} sem id numérico")


WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "prerelease.yml"


def workflow_jobs():
    """Corpo de cada job do workflow, pelo recuo de duas colunas sob `jobs:`."""
    text = WORKFLOW.read_text(encoding="utf-8")
    header, body = text.split("\njobs:\n", 1)
    jobs, name = {}, None
    for line in body.splitlines():
        if line.startswith("  ") and not line.startswith("   ") and line.rstrip().endswith(":"):
            name = line.strip()[:-1]
            jobs[name] = []
        elif name:
            jobs[name].append(line)
    return header, {key: "\n".join(lines) for key, lines in jobs.items()}


class WorkflowContractTest(unittest.TestCase):
    def setUp(self):
        self.header, self.jobs = workflow_jobs()

    def test_jobs_are_the_expected_ones(self):
        self.assertEqual(list(self.jobs), ["plan", "publish", "prune-plan", "prune"])

    def test_write_permission_only_where_nothing_from_the_repository_runs(self):
        self.assertIn("permissions:\n  contents: read", self.header)
        for name, body in self.jobs.items():
            writes = "contents: write" in body
            self.assertEqual(writes, name in ("publish", "prune"), name)
            if writes:
                self.assertNotIn("actions/checkout", body, name)
                self.assertNotIn("python", body, name)

    def test_only_green_develop_pushes_are_planned(self):
        plan = self.jobs["plan"]
        for condition in (
            "github.event.workflow_run.conclusion == 'success'",
            "github.event.workflow_run.event == 'push'",
            "github.event.workflow_run.head_branch == 'develop'",
            "github.event.workflow_run.head_repository.full_name == github.repository",
        ):
            self.assertIn(condition, plan)

    def test_release_is_a_prerelease_that_is_never_latest(self):
        publish = self.jobs["publish"]
        self.assertIn("needs.plan.outputs.enabled == 'on' && needs.plan.outputs.publish == 'yes'", publish)
        create = publish[publish.index("gh release create") :]
        for flag in ("--verify-tag", "--prerelease", "--latest=false", "--generate-notes"):
            self.assertIn(flag, create.split("\n\n")[0])
        self.assertLess(publish.index('-f "ref=refs/tags/$TAG"'), publish.index("gh release create"))

    def test_deletions_only_run_from_a_non_empty_applied_plan(self):
        self.assertIn("needs.prune-plan.result == 'success'", self.jobs["prune"])
        self.assertIn("needs.prune-plan.outputs.targets != ''", self.jobs["prune"])
        plan = self.jobs["prune-plan"]
        self.assertIn("needs.plan.outputs.prune != 'off'", plan)
        self.assertIn('if [ "$MODE" != "apply" ]; then', plan)
        self.assertIn(": > targets.txt", plan[plan.index('if [ "$MODE" != "apply" ]; then') :])
        for name in ("plan", "publish", "prune-plan"):
            self.assertNotIn("-X DELETE", self.jobs[name], name)


class PruneWorkflowTest(unittest.TestCase):
    tag = "v0.1.0-dev.20261120.11"

    def run_step(self, tag_after, list_after, new=True, sha=SHA, listed_id=11):
        _, jobs = workflow_jobs()
        step = textwrap.dedent(jobs["prune-plan"].split("        run: |\n", 1)[1])
        doubles = r'''
gh() {
    case "$*" in
        "api repos/example/project/releases/tags/$NEW_TAG --jq .id")
            echo tag >> calls
            if (( $(grep -c '^tag$' calls) >= TAG_AFTER )); then echo 11; return; fi
            echo 'gh: Not Found (HTTP 404)' >&2
            return 1 ;;
        "api --paginate repos/example/project/releases?per_page=100 --jq "*)
            echo list >> calls
            cat previous.jsonl
            if (( $(grep -c '^list$' calls) >= LIST_AFTER )); then cat new.jsonl; fi ;;
        "api --paginate repos/example/project/git/matching-refs/tags/ --jq "*)
            echo refs >> calls
            if [[ -n "$NEW_TAG" ]]; then echo "$NEW_TAG $REF_SHA"; fi ;;
        *) echo "unexpected gh call: $*" >&2; return 1 ;;
    esac
}
sleep() { echo "sleep $1" >> calls; }
python3() { "$TEST_PYTHON" "$@"; }
'''
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / "tools").mkdir()
            (folder / "defaults").mkdir()
            shutil.copy(release.__file__, folder / "tools")
            config = dict(CONFIG, keep=10)
            (folder / "defaults" / "release.toml").write_text(
                "\n".join(f"{key} = {json.dumps(value)}" for key, value in config.items())
            )
            previous = [pre(f"v0.1.0-dev.20261120.{n}", release_id=n) for n in range(1, 11)]
            (folder / "previous.jsonl").write_text("".join(json.dumps(row) + "\n" for row in previous))
            (folder / "new.jsonl").write_text(json.dumps(pre(self.tag, release_id=listed_id)) + "\n")
            output = folder / "output"
            output.touch()
            env = dict(
                os.environ, REPO="example/project", NEW_TAG=self.tag if new else "", HEAD_SHA=SHA,
                REF_SHA=sha, KEEP="10", MODE="apply", WAIT_ATTEMPTS="3", WAIT_INTERVAL_SECONDS="2",
                TAG_AFTER=str(tag_after), LIST_AFTER=str(list_after), GITHUB_OUTPUT="output",
                GITHUB_STEP_SUMMARY="summary", TEST_PYTHON=Path(sys.executable).as_posix(),
                PYTHONIOENCODING="utf-8",
            )
            bash = shutil.which("bash")
            if os.name == "nt":
                git_bash = Path(shutil.which("git")).resolve().parents[1] / "bin" / "bash.exe"
                if git_bash.is_file():
                    bash = str(git_bash)
            result = subprocess.run(
                [bash, "-c", doubles + step], cwd=folder, env=env, capture_output=True, encoding="utf-8",
            )
            self.assertTrue((folder / "calls").exists(), result.stderr)
            return result, (folder / "calls").read_text().splitlines(), output.read_text()

    def test_waits_for_tag_then_for_the_planners_listing(self):
        result, calls, output = self.run_step(tag_after=2, list_after=2)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, ["tag", "sleep 2", "tag", "list", "sleep 2", "tag", "list", "refs"])
        self.assertIn("delete 1 v0.1.0-dev.20261120.1 -", output)

    def test_exhaustion_blocks_pruning(self):
        for tag_after, list_after, expected_calls in (
            (4, 1, ["tag", "sleep 2", "tag", "sleep 2", "tag"]),
            (1, 4, ["tag", "list", "sleep 2", "tag", "list", "sleep 2", "tag", "list"]),
        ):
            with self.subTest(tag_after=tag_after, list_after=list_after):
                result, calls, output = self.run_step(tag_after, list_after)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("poda bloqueada", result.stdout)
                self.assertEqual(calls, expected_calls)
                self.assertEqual(output, "")

    def test_same_tag_with_different_release_ids_exhausts_wait(self):
        result, calls, output = self.run_step(tag_after=1, list_after=1, listed_id=12)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("poda bloqueada", result.stdout)
        self.assertEqual(calls, ["tag", "list", "sleep 2", "tag", "list", "sleep 2", "tag", "list"])
        self.assertEqual(output, "")

    def test_visible_release_on_wrong_commit_blocks_pruning(self):
        with mock.patch.dict(os.environ, PYTHONUTF8="0", PYTHONIOENCODING="cp1252"):
            result, _, output = self.run_step(1, 1, sha="f" * 40)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(f"não aponta para {SHA}", result.stderr)
        self.assertEqual(output, "")

    def test_without_new_tag_lists_once_and_plans(self):
        result, calls, output = self.run_step(4, 1, new=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, ["list", "refs"])
        self.assertIn("delete 1 v0.1.0-dev.20261120.1 -", output)


class CommandLineTest(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.count = 0

    def file(self, lines):
        self.count += 1
        path = Path(self.folder.name) / ("input-%d" % self.count)
        path.write_text("".join(line + "\n" for line in lines), encoding="utf-8")
        return str(path)

    def run_main(self, argv):
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.object(release, "load_config", return_value=dict(CONFIG)), contextlib.redirect_stdout(
            out
        ), contextlib.redirect_stderr(err):
            code = release.main(argv)
        return code, out.getvalue(), err.getvalue()

    def test_next_prints_outputs(self):
        tags = self.file(dev(("20261120", 1)))
        releases = self.file([json.dumps(pre("v0.1.0-dev.20261120.1"))])
        argv = ["next", "--sha", SHA, "--date", "20261120", "--releases", releases, "--tags", tags]
        argv += ["--merged", tags, "--points-at", self.file([])]
        code, out, _ = self.run_main(argv)
        self.assertEqual(code, 0)
        self.assertEqual(
            out.splitlines(),
            [
                "publish=yes",
                "tag=v0.1.0-dev.20261120.2",
                "version=0.1.0-dev.20261120.2+abc1234",
                "previous=v0.1.0-dev.20261120.1",
            ],
        )

    def test_prune_prints_targets_then_orphans(self):
        tags = dev(*[("20261120", n) for n in range(1, 5)])
        releases = self.file([json.dumps(pre(tag, release_id=n)) for n, tag in enumerate(tags, 1)])
        refs = self.file(["%s %s" % (tag, sha_of(tag)) for tag in tags + ["v0.1.0-dev.20261119.1"]])
        code, out, _ = self.run_main(["prune", "--releases", releases, "--remote-tags", refs])
        self.assertEqual(code, 0)
        self.assertEqual(
            out.splitlines(),
            ["delete 1 v0.1.0-dev.20261120.1 %s" % sha_of(tags[0]), "orphan v0.1.0-dev.20261119.1"],
        )

    def test_bad_input_exits_one(self):
        cases = ((['["not", "an object"]'], []), ([], ["v0.1.0-dev.20261120.1 nothex"]))
        for releases, refs in cases:
            argv = ["prune", "--releases", self.file(releases), "--remote-tags", self.file(refs)]
            code, out, err = self.run_main(argv)
            self.assertEqual((code, out), (1, ""))
            self.assertIn("entrada inválida", err)

    def test_missing_file_exits_one(self):
        missing = str(Path(self.folder.name) / "missing")
        code, _, _ = self.run_main(["prune", "--releases", missing, "--remote-tags", missing])
        self.assertEqual(code, 1)

    def test_bad_config_exits_two(self):
        err = io.StringIO()
        broken = mock.patch.object(release, "load_config", side_effect=release.ConfigError("x"))
        with broken, contextlib.redirect_stderr(err):
            self.assertEqual(release.main(["config"]), 2)


if __name__ == "__main__":
    unittest.main()
