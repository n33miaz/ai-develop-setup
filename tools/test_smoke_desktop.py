"""Tests for the desktop smoke check."""

import io
import unittest
from threading import Event, Timer
from unittest.mock import Mock, call, patch

import smoke_desktop
from smoke_desktop import wait_for_host_banner


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


class DesktopSmokeTests(unittest.TestCase):
    def test_rejects_background_process_exit_after_successful_window_reply(self):
        for exited in ("daemon", "host"):
            with self.subTest(exited=exited):
                host = Mock(stdout=io.BytesIO(b"ads-ptyhost ready\n"))
                host.poll.return_value = None
                daemon = Mock(stdout=io.BytesIO())
                daemon.poll.return_value = None
                background = {"host": host, "daemon": daemon}

                def wait_for_window(*, timeout):
                    if timeout == 60:
                        background[exited].poll.return_value = 1
                    return 0

                window = Mock()
                window.poll.return_value = 0
                window.wait.side_effect = wait_for_window

                with patch.object(smoke_desktop, "Popen", side_effect=[host, daemon, window]):
                    with self.assertRaisesRegex(SystemExit, "a background process exited with the window"):
                        smoke_desktop.main()

                self.assertEqual(window.wait.call_args_list, [call(timeout=60), call(timeout=10)])
                window.terminate.assert_not_called()
                for name, process in background.items():
                    process.wait.assert_called_once_with(timeout=10)
                    self.assertTrue(process.stdout.closed)
                    if name == exited:
                        process.terminate.assert_not_called()
                    else:
                        process.terminate.assert_called_once_with()
