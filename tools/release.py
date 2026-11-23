"""Numera e poda os pré-lançamentos de desenvolvimento (ADR 0025).

Só calcula, a partir de inventários em arquivo, e roda só em jobs de leitura; quem cria e apaga releases e tags é o
workflow `prerelease.yml`, em jobs que não executam código do repositório.

  python3 tools/release.py config
  python3 tools/release.py next --sha S [--date AAAAMMDD] --releases R --tags T --merged M --points-at P
  python3 tools/release.py prune --releases R --remote-tags RT [--require TAG --require-sha S]

`--tags`, `--merged` e `--points-at` têm um nome de tag por linha (todas; alcançáveis a partir do commit; apontando
para ele). `--remote-tags` tem "nome commit" por linha. `--releases` tem um objeto JSON por linha:
{"id", "tagName", "isPrerelease", "isDraft"}. Configuração inválida termina com código 2; entrada inválida, com 1.
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


def published_tags(config, releases):
    """Tags do canal com pré-lançamento publicado (nem rascunho, nem estável)."""
    pattern = tag_pattern(config)
    return {
        release["tagName"]
        for release in releases
        if isinstance(release.get("tagName"), str)
        and pattern.match(release["tagName"])
        and release.get("isPrerelease") is True
        and release.get("isDraft") is False
    }


def next_release(config, sha, date, releases, tags, merged, points_at):
    """O que publicar para `sha`: nada se ele já tem pré-lançamento publicado; senão a próxima tag livre do dia.

    `N` é o maior sufixo do dia na versão base entre todas as tags, mais um, para nunca colidir com uma existente.
    A base das notas é o pré-lançamento publicado mais novo alcançável a partir do commit."""
    if not SHA.match(sha):
        raise ValueError("commit deve ser hexadecimal minúsculo de 7 a 40 caracteres")
    date = parse_date(date)
    published = published_tags(config, releases)
    existing = [item for item in channel_tags(config, points_at) if item[1] in published]
    if existing:
        return {"publish": "no", "existing": max(existing)[1]}
    base = tuple(int(part) for part in config["base_version"].split("."))
    same_day = [key[4] for key, _ in channel_tags(config, tags) if key[:3] == base and key[3] == date]
    number = max(same_day, default=0) + 1
    prerelease = "%s-%s.%s.%d" % (config["base_version"], config["channel"], date, number)
    reachable = [item for item in channel_tags(config, merged) if item[1] in published]
    return {
        "publish": "yes",
        "tag": config["tag_prefix"] + prerelease,
        "version": "%s+%s" % (prerelease, sha[:7]),
        "previous": max(reachable)[1] if reachable else "",
    }


def flag_text(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    return "desconhecido"


def prune_plan(config, releases, remote_tags, require="", require_sha=""):
    """Alvos da poda: (id da release, tag, commit da tag ou "-") dos pré-lançamentos publicados do canal além dos
    `keep` mais novos, mais as tags do canal sem release, só relatadas.

    Nunca entra rascunho, release estável ou de outro canal. A recém-publicada tem de estar entre as mantidas, e a
    tag dela tem de apontar para o commit esperado."""
    pattern = tag_pattern(config)
    refs = dict(remote_tags)
    by_tag = {}
    channel_count = 0
    required_flags = set()
    for release in releases:
        tag = release.get("tagName")
        if require and tag == require:
            draft = flag_text(release.get("isDraft"))
            prerelease = flag_text(release.get("isPrerelease"))
            required_flags.add(f"draft={draft}, prerelease={prerelease}")
        if not isinstance(tag, str) or not pattern.match(tag):
            continue
        channel_count += 1
        if release.get("isPrerelease") is True and release.get("isDraft") is False:
            if not isinstance(release.get("id"), int) or isinstance(release.get("id"), bool):
                raise ValueError("release %s sem id numérico" % tag)
            by_tag[tag] = release["id"]
    ordered = [tag for _, tag in sorted(channel_tags(config, by_tag), reverse=True)]
    kept = ordered[: config["keep"]]
    if require:
        if require not in kept:
            presence = "tag ausente da listagem"
            if required_flags:
                presence = "tag presente na listagem (%s)" % " / ".join(sorted(required_flags))
            raise ValueError(
                f"a release {require} não está publicada entre as mais novas do canal "
                f"({presence}, releases do canal lidas: {channel_count})"
            )
        if refs.get(require) != require_sha:
            raise ValueError("a tag %s não aponta para %s" % (require, require_sha or "o commit esperado"))
    targets = [(by_tag[tag], tag, refs.get(tag, "-")) for tag in ordered[config["keep"] :]]
    with_release = {tag for release in releases if isinstance((tag := release.get("tagName")), str)}
    orphans = sorted(
        (item for item in channel_tags(config, refs) if item[1] not in with_release), reverse=True
    )
    return targets, [tag for _, tag in orphans]


def read_lines(path):
    with open(path, encoding="utf-8") as handle:
        return [line.strip() for line in handle if line.strip()]


def read_remote_tags(path):
    pairs = []
    for line in read_lines(path):
        parts = line.split()
        if len(parts) != 2 or not re.fullmatch(r"[0-9a-f]{40}", parts[1]):
            raise ValueError("cada linha de tags remotas deve ser \"nome commit\"")
        pairs.append((parts[0], parts[1]))
    return pairs


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
    nxt.add_argument("--releases", required=True)
    nxt.add_argument("--tags", required=True)
    nxt.add_argument("--merged", required=True)
    nxt.add_argument("--points-at", required=True)
    prune = sub.add_parser("prune")
    prune.add_argument("--releases", required=True)
    prune.add_argument("--remote-tags", required=True)
    prune.add_argument("--require", default="")
    prune.add_argument("--require-sha", default="")
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
                read_releases(args.releases),
                read_lines(args.tags),
                read_lines(args.merged),
                read_lines(args.points_at),
            )
            for key, value in result.items():
                print("%s=%s" % (key, value))
        else:
            targets, orphans = prune_plan(
                config,
                read_releases(args.releases),
                read_remote_tags(args.remote_tags),
                args.require,
                args.require_sha,
            )
            for release_id, tag, sha in targets:
                print("delete %d %s %s" % (release_id, tag, sha))
            for tag in orphans:
                print("orphan %s" % tag)
    except (OSError, ValueError) as error:
        print("entrada inválida: %s" % error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
