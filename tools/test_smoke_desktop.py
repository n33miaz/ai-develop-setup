"""Tests for the desktop smoke check."""

import io
import unittest
from subprocess import TimeoutExpired
from threading import Event, Timer
from unittest.mock import Mock, call, patch

from smoke_desktop import main, wait_for_host_banner


class HostBannerTests(unittest.TestCase):
    def test_accepts_host_banner(self):
        wait_for_host_banner(io.BytesIO(b"ads-ptyhost ready\n"))

    def test_rejects_eof_before_banner(self):
        with self.assertRaisesRegex(SystemExit, "did not start"):
            wait_for_host_banner(io.BytesIO())

    def test_times_out_while_the_host_pipe_is_still_open(self):
        release = Event()
        stdout = Mock()
        def readline():
            release.wait()
            return b"ads-ptyhost ready\n"

        stdout.readline.side_effect = readline
        fallback = Timer(1, release.set)
        fallback.start()
        try:
            with self.assertRaisesRegex(SystemExit, "startup timed out"):
                wait_for_host_banner(stdout, timeout=0.01)
        finally:
            release.set()
            fallback.cancel()
            fallback.join()


class DesktopWindowTests(unittest.TestCase):
    def test_window_timeout_explains_the_stage_and_cleans_up(self):
        host = Mock(stdout=io.BytesIO(b"ads-ptyhost ready\n"))
        daemon = Mock(stdout=io.BytesIO())
        window = Mock()
        for process in (host, daemon, window):
            process.poll.return_value = None
        window.wait.side_effect = [TimeoutExpired("ads-shell.exe", 60), 0]

        with patch("smoke_desktop.Popen", side_effect=[host, daemon, window]):
            with self.assertRaisesRegex(
                SystemExit,
                r"desktop window timed out after 60 seconds waiting for the harness\.list reply",
            ):
                main()

        self.assertEqual(window.wait.call_args_list, [call(timeout=60), call(timeout=10)])
        for process in (window, daemon, host):
            process.terminate.assert_called_once_with()
        for process in (daemon, host):
            process.wait.assert_called_once_with(timeout=10)
        self.assertTrue(daemon.stdout.closed)
        self.assertTrue(host.stdout.closed)
