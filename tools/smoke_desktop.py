"""Exercise the bundled WebView against the local daemon on Windows CI."""

from pathlib import Path
from subprocess import DEVNULL, PIPE, Popen


def stop(process):
    if process.poll() is None:
        process.terminate()
    process.wait(timeout=10)


def main():
    target = Path("target/debug").resolve()
    host = Popen([target / "ads-ptyhost.exe"], stdout=PIPE)
    try:
        if not host.stdout.readline().startswith(b"ads-ptyhost "):
            raise SystemExit("terminal host did not start")
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
                    raise SystemExit("desktop window failed to authenticate")
                if daemon.poll() is not None or host.poll() is not None:
                    raise SystemExit("a background process exited with the window")
            finally:
                stop(shell)
        finally:
            stop(daemon)
    finally:
        stop(host)


if __name__ == "__main__":
    main()
