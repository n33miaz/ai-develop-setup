"""Numera e poda os pré-lançamentos de desenvolvimento (ADR 0025).

Só calcula, a partir de inventários em arquivo; quem cria e apaga releases e tags é o workflow `prerelease.yml`.

  python3 tools/release.py config
  python3 tools/release.py next --sha S [--date AAAAMMDD] --tags T --merged M --points-at P
  python3 tools/release.py prune --releases R --tags T [--require TAG]

`--tags`, `--merged` e `--points-at` têm um nome de tag por linha (todas as tags; as alcançáveis a partir do commit;
as que apontam para ele). `--releases` tem um objeto JSON por linha: {"tagName", "isPrerelease", "isDraft"}.
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


def channel_tags(config, tags):
    """(chave de ordem SemVer, tag) das tags do canal; a ordem é a de precedência do SemVer."""
    pattern = tag_pattern(config)
    found = []
    for tag in tags:
        match = pattern.match(tag)
        if match:
            major, minor, patch, date, number = match.groups()
            found.append(((int(major), int(minor), int(patch), date, int(number)), tag))
    return found


def parse_date(text):
    if not re.fullmatch(r"[0-9]{8}", text or ""):
        raise ValueError("data deve ser AAAAMMDD")
    datetime.datetime.strptime(text, "%Y%m%d")
    return text


def next_release(config, sha, date, tags, merged, points_at):
    """O que publicar para `sha`: nada se ele já tem tag do canal; senão a próxima tag do dia na versão base.

    A tag anterior (ponto de partida das notas) é a mais nova do canal alcançável a partir do commit."""
    if not SHA.match(sha):
        raise ValueError("commit deve ser hexadecimal minúsculo de 7 a 40 caracteres")
    date = parse_date(date)
    existing = channel_tags(config, points_at)
    if existing:
        return {"publish": "no", "existing": max(existing)[1]}
    base = tuple(int(part) for part in config["base_version"].split("."))
    same_day = [key[4] for key, _ in channel_tags(config, tags) if key[:3] == base and key[3] == date]
    number = max(same_day, default=0) + 1
    prerelease = "%s-%s.%s.%d" % (config["base_version"], config["channel"], date, number)
    reachable = channel_tags(config, merged)
    return {
        "publish": "yes",
        "tag": config["tag_prefix"] + prerelease,
        "version": "%s+%s" % (prerelease, sha[:7]),
        "previous": max(reachable)[1] if reachable else "",
    }


def prune_plan(config, releases, tags, require=""):
    """Releases e tags a apagar para sobrarem só os `keep` pré-lançamentos publicados mais novos do canal.

    Nunca toca rascunho, release estável, release ou tag de outro canal, nem os mantidos. Tags do canal sem
    release (sobras de uma poda interrompida) também saem, desde que fora dos mantidos."""
    pattern = tag_pattern(config)
    published, protected = [], set()
    for release in releases:
        tag = release.get("tagName")
        if not isinstance(tag, str) or not pattern.match(tag):
            continue
        if release.get("isPrerelease") is True and release.get("isDraft") is False:
            published.append(tag)
        else:
            protected.add(tag)
    ordered = [tag for _, tag in sorted(channel_tags(config, published), reverse=True)]
    kept = set(ordered[: config["keep"]])
    if require and require not in kept:
        raise ValueError("a release %s não está publicada entre as mais novas do canal" % require)
    delete_releases = ordered[config["keep"] :]
    delete_tags = sorted(
        (tag for _, tag in channel_tags(config, tags) if tag not in kept and tag not in protected),
        key=lambda tag: channel_tags(config, [tag])[0][0],
        reverse=True,
    )
    return delete_releases, delete_tags


def read_lines(path):
    with open(path, encoding="utf-8") as handle:
        return [line.strip() for line in handle if line.strip()]


def read_releases(path):
    releases = []
    for line in read_lines(path):
        item = json.loads(line)
        if not isinstance(item, dict):
            raise ValueError("cada linha de releases deve ser um objeto JSON")
        releases.append(item)
    return releases


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("config")
    nxt = sub.add_parser("next")
    nxt.add_argument("--sha", required=True)
    nxt.add_argument("--date", default=datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d"))
    nxt.add_argument("--tags", required=True)
    nxt.add_argument("--merged", required=True)
    nxt.add_argument("--points-at", required=True)
    prune = sub.add_parser("prune")
    prune.add_argument("--releases", required=True)
    prune.add_argument("--tags", required=True)
    prune.add_argument("--require", default="")
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
            result = next_release(
                config,
                args.sha,
                args.date,
                read_lines(args.tags),
                read_lines(args.merged),
                read_lines(args.points_at),
            )
            for key, value in result.items():
                print("%s=%s" % (key, value))
        else:
            releases, tags = prune_plan(config, read_releases(args.releases), read_lines(args.tags), args.require)
            for tag in releases:
                print("release %s" % tag)
            for tag in tags:
                print("tag %s" % tag)
    except (OSError, ValueError) as error:
        print("entrada inválida: %s" % error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
