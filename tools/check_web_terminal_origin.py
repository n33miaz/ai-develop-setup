"""Confere a origem efetiva da interface antes de o iniciador abrir os serviços."""

import sys
import tomllib
from pathlib import Path


def read_config(path: Path) -> dict:
    with path.open("rb") as source:
        return tomllib.load(source)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("Usage: check_web_terminal_origin.py <config_path> <expected_origin>")
    config_path, expected = sys.argv[1:]
    try:
        defaults = read_config(Path(__file__).resolve().parents[1] / "defaults" / "daemon.toml")
        origin = read_config(Path(config_path)).get("ui_base_url", defaults["ui_base_url"])
    except KeyError:
        sys.exit("Missing ui_base_url in defaults/daemon.toml.")
    except (OSError, ValueError):
        sys.exit("Could not read the daemon TOML.")
    if origin not in (expected, expected + "/"):
        sys.exit(f"Set ui_base_url to {expected} for this launcher.")
