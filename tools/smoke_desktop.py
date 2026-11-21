"""Exercise the bundled WebView against the local daemon on Windows CI."""

from pathlib import Path
from queue import Empty, Queue
from threading import Thread
from subprocess import DEVNULL, PIPE, Popen


def stop(process):
    if process.poll() is None:
        process.terminate()
    process.wait(timeout=10)


def wait_for_host_banner(stdout, timeout=10):
    banner = Queue(1)
    Thread(target=lambda: banner.put(stdout.readline()), daemon=True).start()
    try:
        line = banner.get(timeout=timeout)
    except Empty:
        raise SystemExit("terminal host startup timed out") from None
    if not line.startswith(b"ads-ptyhost "):
        raise SystemExit("terminal host did not start")


def main():
    target = Path("target/debug").resolve()
    host = Popen([target / "ads-ptyhost.exe"], stdout=PIPE)
    try:
        wait_for_host_banner(host.stdout)
        daemon = Popen([target / "ads-daemon.exe", "--desktop"], stdout=PIPE)
        try:
            with daemon.stdout:
                shell = Popen(
                    [target / "ads-shell.exe", "960", "640", "--smoke-test"],
                    stdin=daemon.stdout,
                    stdout=DEVNULL,
                )
            try:
                if shell.wait(timeout=60) != 0:
                    raise SystemExit("desktop window failed to relay the harness.list reply")
                if daemon.poll() is not None or host.poll() is not None:
                    raise SystemExit("a background process exited with the window")
            finally:
                stop(shell)
        finally:
            stop(daemon)
    finally:
        stop(host)
        host.stdout.close()


if __name__ == "__main__":
    main()
