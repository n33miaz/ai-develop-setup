#!/usr/bin/env bash
# Valida mensagens de commit. Uma implementação só, dois modos (o hook local e o CI usam o mesmo conjunto de regras):
#
#   check-commits.sh <base> <head>
#       Modo CI: valida cada commit (sem merges) do intervalo base..head, lendo mensagem e autor do git.
#   check-commits.sh --message <arquivo> --author <e-mail> [--strip-comments]
#       Modo mensagem: valida um arquivo de mensagem. É o que `.githooks/commit-msg` chama.
#
# As regras e seus valores ficam em defaults/devflow.toml (P8); o arquivo pode ser trocado por ADS_DEVFLOW_TOML.
# Formato exigido: assunto de uma linha (Conventional Commits, inglês, minúsculo), linha em branco e trailers.
# Sem corpo descritivo. Saída 0 = tudo certo; 1 = há problema; 2 = erro de uso ou de configuração.
set -u
export LC_ALL=C

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cfg="${ADS_DEVFLOW_TOML:-$here/../../defaults/devflow.toml}"
# shellcheck source=lib-devflow.sh
. "$here/lib-devflow.sh"

if [ ! -r "$cfg" ]; then
  echo "erro: arquivo de parâmetros não encontrado: $cfg" >&2
  exit 2
fi

errors=0
report() { # <rótulo> <texto>
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    echo "::error::$1: $2"
  else
    echo "erro: $1: $2" >&2
  fi
  errors=$((errors + 1))
}

lower() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }

in_list() { # <item> <lista, um por linha>: sem diferença de maiúscula
  local want it
  want="$(lower "$1")"
  while IFS= read -r it; do
    [ "$(lower "$it")" = "$want" ] && return 0
  done <<< "$2"
  return 1
}

# check_message <arquivo> <e-mail do autor> <rótulo> <1 = ignorar linhas que começam com #>
check_message() {
  local file="$1" author="$2" label="$3" strip="${4:-0}"
  local -a raw=() lines=()
  local line first=-1 last=-1 i

  local enabled max_subject types allowed_trailers signoff_req signoff_match max_co ai email_re
  enabled="$(cfg_str commits.enabled true)"
  [ "$enabled" = "false" ] && return 0
  max_subject="$(cfg_str commits.subject_max 50)"
  types="$(cfg_arr commits.types)"
  [ -z "$types" ] && types=$'feat\nfix\nchore\nrefactor\ndocs\ntest\nperf\nbuild\nci\nstyle\nrevert'
  allowed_trailers="$(cfg_arr commits.allowed_trailers)"
  [ -z "$allowed_trailers" ] && allowed_trailers=$'Signed-off-by\nCo-authored-by'
  signoff_req="$(cfg_str commits.signoff_required true)"
  signoff_match="$(cfg_str commits.signoff_email_must_match_author true)"
  max_co="$(cfg_str commits.max_coauthors 1)"
  ai="$(cfg_str ai_attribution executor)"
  email_re="$(cfg_str commits.author_email_regex '^[0-9]+\+[A-Za-z0-9-]+@users\.noreply\.github\.com$')"

  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    line="${line%"${line##*[![:space:]]}"}"
    if [ "$strip" = 1 ] && [[ "$line" == \#* ]]; then continue; fi
    raw+=("$line")
  done < "$file"

  for i in "${!raw[@]}"; do
    if [ -n "${raw[$i]}" ]; then
      [ "$first" -lt 0 ] && first=$i
      last=$i
    fi
  done
  if [ "$first" -lt 0 ]; then
    report "$label" "a mensagem está vazia."
    return
  fi
  for ((i = first; i <= last; i++)); do lines+=("${raw[$i]}"); done

  local subject="${lines[0]}"

  # Assunto
  local nonascii='[^ -~]'
  if [[ "$subject" =~ $nonascii ]]; then
    report "$label" "o assunto deve ter só caracteres ASCII imprimíveis (inglês, sem acento)."
  fi
  local type_re
  type_re="$(printf '%s\n' "$types" | paste -sd'|' -)"
  local pat='^('"$type_re"')(\([a-z0-9-]+\))?!?: [a-z0-9]'
  if ! [[ "$subject" =~ $pat ]]; then
    report "$label" "o assunto \"$subject\" não segue tipo(escopo): descrição, em minúsculas."
  fi
  if [ "${#subject}" -gt "$max_subject" ]; then
    report "$label" "o assunto tem ${#subject} caracteres; o máximo é $max_subject."
  fi
  if [[ "$subject" == *. ]]; then
    report "$label" "o assunto não pode terminar em ponto."
  fi

  # Estrutura: assunto, linha em branco, trailers (nada de corpo)
  local -a signoff_emails=() co_emails=()
  local n=${#lines[@]} tok val em
  if [ "$n" -gt 1 ]; then
    if [ -n "${lines[1]}" ]; then
      report "$label" "falta a linha em branco entre o assunto e os trailers (corpo não é permitido)."
    fi
    local trailer_re='^([A-Za-z][A-Za-z0-9-]*):[[:space:]]+(.+)$'
    for ((i = 2; i < n; i++)); do
      line="${lines[$i]}"
      if [ -z "$line" ]; then
        report "$label" "corpo descritivo não é permitido: só o assunto e os trailers."
        break
      fi
      if ! [[ "$line" =~ $trailer_re ]]; then
        report "$label" "a linha \"$line\" não é um trailer; corpo descritivo não é permitido."
        continue
      fi
      tok="${BASH_REMATCH[1]}"
      val="${BASH_REMATCH[2]}"
      if ! in_list "$tok" "$allowed_trailers"; then
        report "$label" "trailer \"$tok\" não está na lista permitida."
        continue
      fi
      em=""
      local email_part='<([^<>]+)>[[:space:]]*$'
      if [[ "$val" =~ $email_part ]]; then em="${BASH_REMATCH[1]}"; fi
      case "$(lower "$tok")" in
        signed-off-by)
          [ -z "$em" ] && report "$label" "Signed-off-by sem e-mail entre < >."
          signoff_emails+=("$em")
          ;;
        co-authored-by)
          [ -z "$em" ] && report "$label" "Co-authored-by sem e-mail entre < >."
          co_emails+=("$em")
          ;;
      esac
    done
  fi

  # DCO
  if [ "$signoff_req" = "true" ]; then
    if [ "${#signoff_emails[@]}" -eq 0 ]; then
      report "$label" "falta o trailer Signed-off-by (DCO)."
    elif [ "$signoff_match" = "true" ]; then
      local ok=0 s
      for s in "${signoff_emails[@]}"; do
        [ "$(lower "$s")" = "$(lower "$author")" ] && ok=1
      done
      [ "$ok" -eq 0 ] && report "$label" "nenhum Signed-off-by usa o e-mail do autor."
    fi
  fi

  # Coautores
  if [ "${#co_emails[@]}" -gt 0 ]; then
    if [ "$ai" = "none" ]; then
      report "$label" "coautor não é permitido com ai_attribution = \"none\"."
    fi
    if [ "${#co_emails[@]}" -gt "$max_co" ]; then
      report "$label" "${#co_emails[@]} Co-authored-by; o máximo é $max_co."
    fi
    local allowed_co c
    allowed_co="$(cfg_arr coauthors.allowed)"
    for c in "${co_emails[@]}"; do
      if ! in_list "$c" "$allowed_co"; then
        report "$label" "coautor com e-mail fora da lista permitida."
      fi
    done
  fi

  # Autor
  if ! [[ "$author" =~ $email_re ]]; then
    report "$label" "o e-mail do autor não está no padrão noreply do GitHub (ID+usuario@users.noreply.github.com)."
  fi

  # Atribuição automática barrada (procura na mensagem inteira; não imprime a linha achada)
  local p
  while IFS= read -r p; do
    [ -z "$p" ] && continue
    if printf '%s\n' "${lines[@]}" | grep -Eiq -- "$p"; then
      report "$label" "linha de atribuição automática de ferramenta barrada (padrão do devflow.toml)."
    fi
  done < <(cfg_arr commits.blocked_patterns)
}

mode="${1:-}"
if [ "$mode" = "--message" ]; then
  file="" author="" strip=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --message) file="${2:-}"; shift 2 ;;
      --author) author="${2:-}"; shift 2 ;;
      --strip-comments) strip=1; shift ;;
      *) echo "uso: $0 --message <arquivo> --author <e-mail> [--strip-comments]" >&2; exit 2 ;;
    esac
  done
  [ -z "$file" ] && { echo "uso: $0 --message <arquivo> --author <e-mail> [--strip-comments]" >&2; exit 2; }
  [ -r "$file" ] || { echo "erro: arquivo ilegível: $file" >&2; exit 2; }
  check_message "$file" "$author" "mensagem" "$strip"
else
  base="${1:-}"
  head="${2:-}"
  if [ -z "$base" ] || [ -z "$head" ]; then
    echo "uso: $0 <base> <head> | --message <arquivo> --author <e-mail> [--strip-comments]" >&2
    exit 2
  fi
  [ "$(cfg_str commits.enabled true)" = "false" ] && { echo "Checagem de commits desligada (commits.enabled = false)."; exit 0; }
  sig_mode="$(cfg_str signature.mode warn)"
  commits="$(git rev-list --reverse --no-merges "${base}..${head}")" || exit 2
  if [ -z "$commits" ]; then
    echo "Nenhum commit para validar."
    exit 0
  fi
  tmp="$(mktemp)"
  trap 'rm -f "$tmp"' EXIT
  for sha in $commits; do
    git show -s --format=%B "$sha" > "$tmp"
    author_email="$(git show -s --format=%ae "$sha")"
    check_message "$tmp" "$author_email" "commit ${sha:0:7}" 0
    if [ "$sig_mode" != "off" ]; then
      sig="$(git show -s --format=%G? "$sha")"
      if [ "$sig" != "G" ] && [ "$sig" != "U" ]; then
        if [ "$sig_mode" = "block" ]; then
          report "commit ${sha:0:7}" "sem assinatura verificada (signature.mode = block)."
        else
          echo "::warning::commit ${sha:0:7}: sem assinatura verificada (signature.mode = warn)."
        fi
      fi
    fi
  done
fi

if [ "$errors" -gt 0 ]; then
  echo "Checagem de commits falhou com $errors problema(s)." >&2
  exit 1
fi
echo "Todos os commits estão no padrão."
