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


def pre(tag, prerelease=True):
    return {"tagName": tag, "isPrerelease": prerelease}


class ShippedConfigTest(unittest.TestCase):
    def test_shipped_defaults_are_valid(self):
        config = release.load_config()
        self.assertEqual(config["base_version"], "0.1.0")
        self.assertEqual(config["keep"], 10)


class ValidateConfigTest(unittest.TestCase):
    def bad(self, **change):
        raw = dict(CONFIG, **change)
        with self.assertRaises(release.ConfigError):
            release.validate_config(raw)

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
    def test_first_release_of_the_day(self):
        result = release.next_release(CONFIG, [], SHA, "20261120")
        self.assertEqual(result["tag"], "v0.1.0-dev.20261120.1")
        self.assertEqual(result["version"], "0.1.0-dev.20261120.1+abc1234")
        self.assertEqual(result["previous"], "")

    def test_counts_within_the_day_and_ignores_other_tags(self):
        tags = [
            "v0.1.0-dev.20261120.1",
            "v0.1.0-dev.20261120.9",
            "v0.1.0-dev.20261120.10",
            "v0.1.0-dev.20261119.42",
            "v0.1.0-rc.20261120.50",
            "v0.0.9-dev.20261120.70",
            "0.1.0-dev.20261120.80",
            "v0.1.0-dev.20261120.81-extra",
            "dev-20261120-abc1234",
        ]
        result = release.next_release(CONFIG, tags, SHA, "20261120")
        self.assertEqual(result["tag"], "v0.1.0-dev.20261120.11")
        self.assertEqual(result["previous"], "v0.1.0-dev.20261120.10")

    def test_new_day_restarts_the_counter(self):
        tags = ["v0.1.0-dev.20261119.4"]
        result = release.next_release(CONFIG, tags, SHA, "20261120")
        self.assertEqual(result["tag"], "v0.1.0-dev.20261120.1")
        self.assertEqual(result["previous"], "v0.1.0-dev.20261119.4")

    def test_rejects_bad_commit_and_date(self):
        for sha in ("ABC1234", "abc12", "zzzzzzz", ""):
            with self.assertRaises(ValueError):
                release.next_release(CONFIG, [], sha, "20261120")
        for date in ("2026112", "20261340", "2026-11-20"):
            with self.assertRaises(ValueError):
                release.next_release(CONFIG, [], SHA, date)


class PrunePlanTest(unittest.TestCase):
    def test_keeps_the_newest_by_version_order(self):
        releases = [
            pre("v0.1.0-dev.20261120.10"),
            pre("v0.1.0-dev.20261120.9"),
            pre("v0.1.0-dev.20261119.1"),
            pre("v0.1.0-dev.20261120.2"),
            pre("v0.1.0-dev.20261118.5"),
        ]
        plan = release.prune_plan(CONFIG, releases, "v0.1.0-dev.20261120.10")
        self.assertEqual(plan, ["v0.1.0-dev.20261119.1", "v0.1.0-dev.20261118.5"])

    def test_never_touches_stable_or_foreign_releases(self):
        releases = [pre("v0.1.0-dev.2026112%d.1" % day) for day in range(10)]
        releases += [
            pre("v0.0.1-dev.20260101.1", prerelease=False),
            pre("v0.1.0"),
            pre("v0.1.0-rc.20260101.1"),
            pre("something-else"),
            {"isPrerelease": True},
        ]
        plan = release.prune_plan(CONFIG, releases, "")
        self.assertEqual(len(plan), 7)
        self.assertTrue(all("-dev.2026112" in tag for tag in plan))

    def test_nothing_to_prune_at_or_below_keep(self):
        releases = [pre("v0.1.0-dev.20261120.%d" % n) for n in (1, 2, 3)]
        self.assertEqual(release.prune_plan(CONFIG, releases, ""), [])

    def test_refuses_when_the_new_tag_would_be_pruned(self):
        releases = [pre("v0.1.0-dev.20261120.%d" % n) for n in (1, 2, 3, 4)]
        with self.assertRaises(ValueError):
            release.prune_plan(CONFIG, releases, "v0.1.0-dev.20261120.1")


class CommandLineTest(unittest.TestCase):
    def run_main(self, argv, stdin=""):
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.object(release, "load_config", return_value=dict(CONFIG)), mock.patch(
            "sys.stdin", io.StringIO(stdin)
        ), contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = release.main(argv)
        return code, out.getvalue(), err.getvalue()

    def test_next_prints_outputs(self):
        code, out, _ = self.run_main(["next", "--sha", SHA, "--date", "20261120"], "v0.1.0-dev.20261120.1\n")
        self.assertEqual(code, 0)
        self.assertIn("tag=v0.1.0-dev.20261120.2\n", out)
        self.assertIn("version=0.1.0-dev.20261120.2+abc1234\n", out)

    def test_prune_prints_one_tag_per_line(self):
        releases = [pre("v0.1.0-dev.20261120.%d" % n) for n in range(1, 6)]
        code, out, _ = self.run_main(["prune"], json.dumps(releases))
        self.assertEqual(code, 0)
        self.assertEqual(out.splitlines(), ["v0.1.0-dev.20261120.2", "v0.1.0-dev.20261120.1"])

    def test_bad_input_exits_one(self):
        code, out, err = self.run_main(["prune"], '{"not": "a list"}')
        self.assertEqual((code, out), (1, ""))
        self.assertIn("entrada inválida", err)

    def test_bad_config_exits_two(self):
        err = io.StringIO()
        broken = mock.patch.object(release, "load_config", side_effect=release.ConfigError("x"))
        with broken, contextlib.redirect_stderr(err):
            self.assertEqual(release.main(["config"]), 2)


if __name__ == "__main__":
    unittest.main()
