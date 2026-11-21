"""Tests for the terminal host startup check."""

import io
import unittest
from threading import Event, Timer
from unittest.mock import Mock

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
