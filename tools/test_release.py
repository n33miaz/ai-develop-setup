import contextlib
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


def pre(tag, prerelease=True, draft=False):
    return {"tagName": tag, "isPrerelease": prerelease, "isDraft": draft}


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
    def nxt(self, tags=(), merged=(), points_at=(), date="20261120", sha=SHA):
        return release.next_release(CONFIG, sha, date, list(tags), list(merged), list(points_at))

    def test_first_release_of_the_day(self):
        result = self.nxt()
        self.assertEqual(result["publish"], "yes")
        self.assertEqual(result["tag"], "v0.1.0-dev.20261120.1")
        self.assertEqual(result["version"], "0.1.0-dev.20261120.1+abc1234")
        self.assertEqual(result["previous"], "")

    def test_counts_within_the_day_and_ignores_other_tags(self):
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

    def test_previous_is_the_newest_reachable_channel_tag(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3))
        merged = dev(("20261120", 1), ("20261120", 2)) + ["v0.1.0-rc.20261121.1", "v9.0.0"]
        result = self.nxt(tags=tags, merged=merged)
        self.assertEqual(result["tag"], "v0.1.0-dev.20261120.4")
        self.assertEqual(result["previous"], "v0.1.0-dev.20261120.2")

    def test_commit_with_a_channel_tag_is_not_published_again(self):
        result = self.nxt(points_at=["v0.0.1", "v0.1.0-dev.20261120.2"])
        self.assertEqual(result, {"publish": "no", "existing": "v0.1.0-dev.20261120.2"})

    def test_other_tags_on_the_commit_do_not_block(self):
        self.assertEqual(self.nxt(points_at=["v0.0.1", "v0.1.0-rc.20261120.1"])["publish"], "yes")

    def test_rejects_bad_commit_and_date(self):
        for sha in ("ABC1234", "abc12", "zzzzzzz", ""):
            with self.assertRaises(ValueError):
                self.nxt(sha=sha)
        for date in ("2026112", "20261340", "2026-11-20"):
            with self.assertRaises(ValueError):
                self.nxt(date=date)


class PrunePlanTest(unittest.TestCase):
    def plan(self, releases, tags=None, require=""):
        if tags is None:
            tags = [item["tagName"] for item in releases if "tagName" in item]
        return release.prune_plan(CONFIG, releases, tags, require)

    def test_keeps_the_newest_by_version_order(self):
        tags = dev(("20261120", 10), ("20261120", 9), ("20261119", 1), ("20261120", 2), ("20261118", 5))
        releases, delete_tags = self.plan([pre(tag) for tag in tags])
        expected = ["v0.1.0-dev.20261119.1", "v0.1.0-dev.20261118.5"]
        self.assertEqual(releases, expected)
        self.assertEqual(delete_tags, expected)

    def test_never_touches_stable_draft_or_foreign_releases(self):
        keep = dev(("20261120", 3), ("20261120", 4), ("20261120", 5))
        old = dev(("20261120", 1))
        guarded = [
            pre("v0.1.0-dev.20261120.2", prerelease=False),
            pre("v0.1.0-dev.20261119.1", draft=True),
            pre("v0.1.0"),
            pre("v0.1.0-rc.20260101.1"),
            pre("something-else"),
            {"isPrerelease": True, "isDraft": False},
        ]
        releases, delete_tags = self.plan([pre(tag) for tag in keep + old] + guarded)
        self.assertEqual(releases, old)
        self.assertEqual(delete_tags, old)

    def test_orphan_channel_tags_beyond_the_kept_are_removed(self):
        keep = dev(("20261120", 3), ("20261120", 4), ("20261120", 5))
        tags = keep + dev(("20261120", 1), ("20261120", 9)) + ["v0.0.1", "v0.1.0-rc.20261120.1"]
        releases, delete_tags = self.plan([pre(tag) for tag in keep], tags=tags)
        self.assertEqual(releases, [])
        self.assertEqual(delete_tags, ["v0.1.0-dev.20261120.9", "v0.1.0-dev.20261120.1"])

    def test_nothing_to_prune_at_or_below_keep(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3))
        self.assertEqual(self.plan([pre(tag) for tag in tags]), ([], []))

    def test_required_release_must_be_published_and_kept(self):
        tags = dev(("20261120", 1), ("20261120", 2), ("20261120", 3), ("20261120", 4))
        with self.assertRaises(ValueError):
            self.plan([pre(tag) for tag in tags], require="v0.1.0-dev.20261120.1")
        with self.assertRaises(ValueError):
            self.plan([pre(tag) for tag in tags], require="v0.1.0-dev.20261120.5")
        drafted = [pre(tag) for tag in tags[:3]] + [pre(tags[3], draft=True)]
        with self.assertRaises(ValueError):
            self.plan(drafted, require=tags[3])
        self.assertEqual(self.plan([pre(tag) for tag in tags], require=tags[3])[0], [tags[0]])


class CommandLineTest(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)

    def file(self, name, lines):
        path = Path(self.folder.name) / name
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
        tags = self.file("tags", dev(("20261120", 1)))
        empty = self.file("empty", [])
        argv = ["next", "--sha", SHA, "--date", "20261120", "--tags", tags, "--merged", tags, "--points-at", empty]
        code, out, _ = self.run_main(argv)
        self.assertEqual(code, 0)
        self.assertIn("publish=yes\n", out)
        self.assertIn("tag=v0.1.0-dev.20261120.2\n", out)
        self.assertIn("version=0.1.0-dev.20261120.2+abc1234\n", out)
        self.assertIn("previous=v0.1.0-dev.20261120.1\n", out)

    def test_prune_prints_releases_then_tags(self):
        tags = dev(*[("20261120", n) for n in range(1, 6)])
        releases = self.file("releases", [json.dumps(pre(tag)) for tag in tags])
        code, out, _ = self.run_main(["prune", "--releases", releases, "--tags", self.file("tags", tags)])
        self.assertEqual(code, 0)
        self.assertEqual(
            out.splitlines(),
            [
                "release v0.1.0-dev.20261120.2",
                "release v0.1.0-dev.20261120.1",
                "tag v0.1.0-dev.20261120.2",
                "tag v0.1.0-dev.20261120.1",
            ],
        )

    def test_bad_input_exits_one(self):
        releases = self.file("releases", ['["not", "an object"]'])
        code, out, err = self.run_main(["prune", "--releases", releases, "--tags", self.file("tags", [])])
        self.assertEqual((code, out), (1, ""))
        self.assertIn("entrada inválida", err)

    def test_missing_file_exits_one(self):
        missing = str(Path(self.folder.name) / "missing")
        code, _, _ = self.run_main(["prune", "--releases", missing, "--tags", missing])
        self.assertEqual(code, 1)

    def test_bad_config_exits_two(self):
        err = io.StringIO()
        broken = mock.patch.object(release, "load_config", side_effect=release.ConfigError("x"))
        with broken, contextlib.redirect_stderr(err):
            self.assertEqual(release.main(["config"]), 2)


if __name__ == "__main__":
    unittest.main()
