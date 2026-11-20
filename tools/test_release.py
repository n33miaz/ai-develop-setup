import contextlib
import hashlib
import io
import json
import sys
import tempfile
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

    def test_required_release_must_be_published_kept_and_on_its_commit(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        new = tags[3]
        releases = [pre(tag) for tag in tags]
        for bad_require in (tags[0], "v0.1.0-dev.20261120.5"):
            with self.assertRaises(ValueError):
                self.plan(releases, require=bad_require, require_sha=sha_of(bad_require))
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
        for bad in ("7", None, True):
            item = pre(dev(("20261120", 1))[0])
            item["id"] = bad
            with self.assertRaises(ValueError):
                self.plan([item])


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
