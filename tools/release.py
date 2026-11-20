"""Numera e poda os pré-lançamentos de desenvolvimento (ADR 0025).

Só calcula: quem cria e apaga releases é o workflow `prerelease.yml`, a partir do que este script imprime.

  python3 tools/release.py config              # valida defaults/release.toml e imprime chave=valor
  python3 tools/release.py next --sha S [--date AAAAMMDD] < tags.txt
  python3 tools/release.py prune --keep-tag T < releases.json

Configuração inválida termina com código 2; entrada inválida, com 1.
"""

import argparse
import datetime
import json
import re
import sys
import tomllib
from pathlib import Path

CONFIG = Path(__file__).resolve().parent.parent / "defaults" / "release.toml"

SEMVER_CORE = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
CHANNEL = re.compile(r"^[a-z][a-z0-9]*$")
SHA = re.compile(r"^[0-9a-f]{7,40}$")
KEYS = {"enabled", "base_version", "channel", "tag_prefix", "keep", "prune"}


class ConfigError(Exception):
    pass


def load_config(path=CONFIG):
    try:
        with open(path, "rb") as handle:
            raw = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ConfigError("não foi possível ler %s: %s" % (path, error)) from error
    return validate_config(raw)


def validate_config(raw):
    unknown = sorted(set(raw) - KEYS)
    if unknown:
        raise ConfigError("chaves desconhecidas: %s" % ", ".join(unknown))
    missing = sorted(KEYS - set(raw))
    if missing:
        raise ConfigError("chaves ausentes: %s" % ", ".join(missing))
    if raw["enabled"] not in ("on", "off"):
        raise ConfigError('enabled deve ser "on" ou "off"')
    if not isinstance(raw["base_version"], str) or not SEMVER_CORE.match(raw["base_version"]):
        raise ConfigError("base_version deve ser MAIOR.MENOR.CORREÇÃO")
    if not isinstance(raw["channel"], str) or not CHANNEL.match(raw["channel"]):
        raise ConfigError("channel deve ter só letras minúsculas e dígitos, começando por letra")
    if raw["tag_prefix"] not in ("", "v"):
        raise ConfigError('tag_prefix deve ser "" ou "v"')
    keep = raw["keep"]
    if isinstance(keep, bool) or not isinstance(keep, int) or not 1 <= keep <= 100:
        raise ConfigError("keep deve ser um inteiro de 1 a 100")
    if raw["prune"] not in ("apply", "dry-run", "off"):
        raise ConfigError('prune deve ser "apply", "dry-run" ou "off"')
    return dict(raw)


def tag_pattern(config):
    """Tags de pré-lançamento deste canal, de qualquer versão base."""
    return re.compile(
        r"^%s(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)-%s\.([0-9]{8})\.([1-9][0-9]*)$"
        % (re.escape(config["tag_prefix"]), re.escape(config["channel"]))
    )


def order_key(match):
    major, minor, patch, date, number = match.groups()
    return (int(major), int(minor), int(patch), date, int(number))


def parse_date(text):
    if not re.fullmatch(r"[0-9]{8}", text or ""):
        raise ValueError("data deve ser AAAAMMDD")
    datetime.datetime.strptime(text, "%Y%m%d")
    return text


def next_release(config, tags, sha, date):
    """Próxima tag, versão completa e a tag anterior do canal (ponto de partida das notas)."""
    if not SHA.match(sha):
        raise ValueError("commit deve ser hexadecimal minúsculo de 7 a 40 caracteres")
    date = parse_date(date)
    pattern = tag_pattern(config)
    ours = [(order_key(m), m.group(0)) for m in map(pattern.match, tags) if m]
    base = tuple(int(part) for part in config["base_version"].split("."))
    same_day = [key[4] for key, _ in ours if key[:3] == base and key[3] == date]
    number = max(same_day, default=0) + 1
    prerelease = "%s-%s.%s.%d" % (config["base_version"], config["channel"], date, number)
    previous = max(ours)[1] if ours else ""
    return {
        "tag": config["tag_prefix"] + prerelease,
        "version": "%s+%s" % (prerelease, sha[:7]),
        "previous": previous,
    }


def prune_plan(config, releases, keep_tag):
    """Tags a apagar: só pré-lançamentos deste canal além dos `keep` mais novos, nunca `keep_tag`."""
    pattern = tag_pattern(config)
    ours = []
    for release in releases:
        tag = release.get("tagName", "")
        match = pattern.match(tag)
        if match and release.get("isPrerelease") is True:
            ours.append((order_key(match), tag))
    ours.sort(reverse=True)
    keep = config["keep"]
    survivors = [tag for _, tag in ours[:keep]]
    if keep_tag and keep_tag in [tag for _, tag in ours] and keep_tag not in survivors:
        raise ValueError("a tag recém-publicada %s não está entre as mais novas" % keep_tag)
    return [tag for _, tag in ours[keep:]]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("config")
    nxt = sub.add_parser("next")
    nxt.add_argument("--sha", required=True)
    nxt.add_argument("--date", default=datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d"))
    prune = sub.add_parser("prune")
    prune.add_argument("--keep-tag", default="")
    args = parser.parse_args(argv)

    try:
        config = load_config()
    except ConfigError as error:
        print("release.toml inválido: %s" % error, file=sys.stderr)
        return 2

    try:
        if args.command == "config":
            for key in ("enabled", "keep", "prune"):
                print("%s=%s" % (key, config[key]))
        elif args.command == "next":
            tags = [line.strip() for line in sys.stdin if line.strip()]
            for key, value in next_release(config, tags, args.sha, args.date).items():
                print("%s=%s" % (key, value))
        else:
            releases = json.load(sys.stdin)
            if not isinstance(releases, list):
                raise ValueError("a entrada deve ser uma lista JSON de releases")
            for tag in prune_plan(config, releases, args.keep_tag):
                print(tag)
    except (ValueError, json.JSONDecodeError) as error:
        print("entrada inválida: %s" % error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
