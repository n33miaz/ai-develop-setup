"""Tests for the desktop smoke check."""

import io
import unittest
from pathlib import Path
from subprocess import TimeoutExpired
from threading import Event, Timer
from unittest.mock import Mock, call, patch

import smoke_desktop
from smoke_desktop import stop, wait_for_host_banner


class StopProcessTests(unittest.TestCase):
    def test_terminates_and_waits_when_process_responds_normally(self):
        process = Mock()
        process.poll.return_value = None

        stop(process)

        process.terminate.assert_called_once_with()
        process.wait.assert_called_once_with(timeout=10)
        process.kill.assert_not_called()

    def test_kills_and_waits_again_when_terminate_times_out(self):
        process = Mock()
        process.poll.return_value = None
        process.wait.side_effect = [TimeoutExpired("ads-shell.exe", 10), 0]

        stop(process)

        process.terminate.assert_called_once_with()
        process.kill.assert_called_once_with()
        self.assertEqual(process.wait.call_args_list, [call(timeout=10), call(timeout=10)])


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
    def test_host_banner_failure_propagates_and_cleans_up_host(self):
        host = Mock(stdout=io.BytesIO())
        host.poll.return_value = None
        failure = SystemExit("terminal host startup timed out")
        host_path = Path("target/debug").resolve() / "ads-ptyhost.exe"

        with (
            patch.object(smoke_desktop, "Popen", return_value=host) as popen,
            patch.object(smoke_desktop, "wait_for_host_banner", side_effect=failure) as wait,
        ):
            with self.assertRaises(SystemExit) as raised:
                smoke_desktop.main()

        self.assertIs(raised.exception, failure)
        popen.assert_called_once_with([host_path], stdout=smoke_desktop.PIPE)
        wait.assert_called_once_with(host.stdout)
        host.terminate.assert_called_once_with()
        host.wait.assert_called_once_with(timeout=10)
        self.assertTrue(host.stdout.closed)

    def test_rejects_nonzero_window_exit_with_background_processes_running(self):
        host = Mock(stdout=io.BytesIO(b"ads-ptyhost ready\n"))
        daemon = Mock(stdout=io.BytesIO())
        for process in (host, daemon):
            process.poll.return_value = None
        window = Mock()
        window.poll.return_value = 1
        window.wait.return_value = 1

        with patch.object(smoke_desktop, "Popen", side_effect=[host, daemon, window]):
            with self.assertRaisesRegex(
                SystemExit,
                r"^desktop window failed to relay the harness\.list reply$",
            ):
                smoke_desktop.main()

        self.assertEqual(window.wait.call_args_list, [call(timeout=60), call(timeout=10)])
        window.terminate.assert_not_called()
        for process in (daemon, host):
            process.terminate.assert_called_once_with()
            process.wait.assert_called_once_with(timeout=10)
            self.assertTrue(process.stdout.closed)

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

    def test_window_timeout_explains_the_stage_and_cleans_up(self):
        host = Mock(stdout=io.BytesIO(b"ads-ptyhost ready\n"))
        daemon = Mock(stdout=io.BytesIO())
        window = Mock()
        for process in (host, daemon, window):
            process.poll.return_value = None
        window.wait.side_effect = [TimeoutExpired("ads-shell.exe", 60), 0]

        with patch.object(smoke_desktop, "Popen", side_effect=[host, daemon, window]):
            with self.assertRaisesRegex(
                SystemExit,
                r"desktop window timed out after 60 seconds waiting for the harness\.list reply",
            ):
                smoke_desktop.main()

        self.assertEqual(window.wait.call_args_list, [call(timeout=60), call(timeout=10)])
        for process in (window, daemon, host):
            process.terminate.assert_called_once_with()
        for process in (daemon, host):
            process.wait.assert_called_once_with(timeout=10)
        self.assertTrue(daemon.stdout.closed)
        self.assertTrue(host.stdout.closed)
