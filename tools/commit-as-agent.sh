#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ] || [ -z "${2:-}" ]; then
  echo "uso: bash tools/commit-as-agent.sh <claude|codex> <assunto>" >&2
  exit 2
fi
who="$1"
case "$who" in
  claude|codex) ;;
  *) echo "ferramenta desconhecida: $who (use claude ou codex)" >&2; exit 2 ;;
esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
cfg="${ADS_DEVFLOW_TOML:-$root/defaults/devflow.toml}"
. "$root/.github/scripts/lib-devflow.sh"
bash "$root/.github/scripts/check-commits.sh" --check-config
if [ "$(cfg_str ai_attribution executor)" != "agent-author" ]; then
  echo "commit-as-agent: requer ai_attribution = agent-author." >&2
  exit 2
fi
if [ "$(cfg_str commits.enabled true)" = "false" ]; then
  echo "commit-as-agent: habilite a checagem para o hook acrescentar a coautoria e o DCO." >&2
  exit 2
fi
if [ "$(git config --get core.hooksPath || true)" != ".githooks" ]; then
  echo "commit-as-agent: ative o hook com git config core.hooksPath .githooks." >&2
  exit 2
fi

agent="$(cfg_str "agent.$who")"
email="$(identity_email "$agent")"
name="${agent% <*}"
export GIT_AUTHOR_NAME="$name" GIT_AUTHOR_EMAIL="$email"
export GIT_COMMITTER_NAME="$name" GIT_COMMITTER_EMAIL="$email"
export ADS_AI_AUTHOR="$who"
exec git commit -m "$2"
