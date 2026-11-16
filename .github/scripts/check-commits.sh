#!/usr/bin/env bash
# Valida mensagens de commit. Uma implementação só, dois modos (o hook local e o CI usam o mesmo conjunto de regras):
#
#   check-commits.sh <base> <head>
#       Modo CI: valida cada commit (sem merges) do intervalo base..head, lendo mensagem e autor do git.
#   check-commits.sh --message <arquivo> --author <e-mail> [--strip-comments]
#       Modo mensagem: valida um arquivo de mensagem. É o que `.githooks/commit-msg` chama.
#   check-commits.sh --check-config
#       Só valida o arquivo de parâmetros, sem ler mensagem nenhuma: 0 = válido; 2 = inválido. O hook chama este modo
#       antes de qualquer outra coisa, para o erro de configuração vencer também o desligamento da checagem.
#
# As regras e seus valores ficam em defaults/devflow.toml (P8); o arquivo pode ser trocado por ADS_DEVFLOW_TOML.
# Formato exigido: assunto de uma linha (Conventional Commits, inglês, minúsculo), linha em branco e trailers.
# Sem corpo descritivo. Saída 0 = tudo certo; 1 = há problema; 2 = erro de uso ou de configuração.
# A configuração é validada inteira (tipo e domínio de cada chave) antes de qualquer mensagem ser lida.
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

# ---- Validação da configuração (sai com 2 antes de olhar qualquer mensagem) -------------------------------
# Confere o valor cru de cada chave presente; chave ausente continua caindo no padrão embutido.
# O texto da configuração nunca é avaliado: só comparado com padrões fixos (os valores entram em regex e em testes).
cfg_errors=0
cfg_fail() { # <chave> <texto>
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    echo "::error::configuração: $1: $2"
  else
    echo "erro de configuração: $1: $2" >&2
  fi
  cfg_errors=$((cfg_errors + 1))
}

cfg_has() { # <chave>: a chave está no arquivo?
  local k="${1//./\\.}"
  grep -Eq "^${k}[[:space:]]*=" "$cfg"
}

cfg_check_bool() { # <chave>
  cfg_has "$1" || return 0
  case "$(cfg_raw "$1")" in
    true | false) ;;
    *) cfg_fail "$1" "esperado true ou false, sem aspas." ;;
  esac
}

cfg_check_int() { # <chave> <mínimo> <máximo, até 999>
  cfg_has "$1" || return 0
  local v
  v="$(cfg_raw "$1")"
  local int_re='^(0|[1-9][0-9]{0,2})$'
  if ! [[ "$v" =~ $int_re ]] || [ "$v" -lt "$2" ] || [ "$v" -gt "$3" ]; then
    cfg_fail "$1" "esperado número inteiro de $2 a $3, sem aspas."
  fi
}

cfg_check_enum() { # <chave> <valor>...
  cfg_has "$1" || return 0
  local k="$1" v opt
  v="$(cfg_raw "$k")"
  shift
  for opt in "$@"; do
    [ "$v" = "\"$opt\"" ] && return 0
  done
  cfg_fail "$k" "esperado um destes textos entre aspas: $*."
}

regex_ok() { # <regex estendida>: compila?
  printf '' | grep -Eq -- "$1" 2> /dev/null
  [ $? -lt 2 ]
}

cfg_check_regex() { # <chave>
  cfg_has "$1" || return 0
  local v
  local quoted_re="^('[^']+'|\"[^\"]+\")\$"
  v="$(cfg_raw "$1")"
  if ! [[ "$v" =~ $quoted_re ]]; then
    cfg_fail "$1" "esperado um texto não vazio entre aspas."
  elif ! regex_ok "$(cfg_str "$1")"; then
    cfg_fail "$1" "a expressão regular não compila."
  fi
}

cfg_check_list() { # <chave> <0 = pode ser vazia; 1 = precisa de item> <regex de cada item | "regex">
  cfg_has "$1" || return 0
  local v item n=0
  v="$(cfg_raw "$1")"
  local item_re="(\"[^\"]*\"|'[^']*')"
  local list_re="^\[[[:space:]]*(${item_re}([[:space:]]*,[[:space:]]*${item_re})*)?[[:space:]]*\]$"
  if ! [[ "$v" =~ $list_re ]]; then
    cfg_fail "$1" "esperada uma lista de textos entre aspas, numa linha só."
    return 0
  fi
  while IFS= read -r item; do
    n=$((n + 1))
    if [ "$3" = "regex" ]; then
      if [ -z "$item" ] || ! regex_ok "$item"; then
        cfg_fail "$1" "o item $n não é uma expressão regular válida."
      fi
    elif ! [[ "$item" =~ $3 ]]; then
      cfg_fail "$1" "o item $n está fora do formato aceito."
    fi
  done < <(cfg_arr "$1")
  if [ "$2" = 1 ] && [ "$n" -eq 0 ]; then
    cfg_fail "$1" "a lista precisa de pelo menos um item."
  fi
}

validate_config() {
  cfg_check_enum profile express full
  cfg_check_bool workflow.issue_per_task
  cfg_check_enum ai_attribution executor none
  cfg_check_list coauthors.allowed 0 '^[^[:space:]<>@]+@[^[:space:]<>@]+$'
  cfg_check_int commits.max_coauthors 0 99
  cfg_check_bool commits.enabled
  cfg_check_int commits.subject_max 1 999
  cfg_check_bool commits.subject_lowercase
  cfg_check_list commits.types 1 '^[a-z]+$'
  cfg_check_list commits.allowed_trailers 1 '^[A-Za-z][A-Za-z0-9-]*$'
  cfg_check_bool commits.signoff_required
  cfg_check_bool commits.signoff_email_must_match_author
  cfg_check_bool commits.signoff_autoadd
  cfg_check_regex commits.author_email_regex
  cfg_check_list commits.blocked_patterns 0 regex
  cfg_check_enum signature.mode warn block off
  if [ "$cfg_errors" -gt 0 ]; then
    echo "Configuração inválida em $cfg: $cfg_errors problema(s). Nenhuma mensagem foi analisada." >&2
    exit 2
  fi
}
validate_config
[ "${1:-}" = "--check-config" ] && exit 0

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

  local enabled max_subject lowercase types allowed_trailers signoff_req signoff_match max_co ai email_re
  enabled="$(cfg_str commits.enabled true)"
  [ "$enabled" = "false" ] && return 0
  max_subject="$(cfg_str commits.subject_max 50)"
  lowercase="$(cfg_str commits.subject_lowercase true)"
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
  local first_re='[a-z0-9]'
  [ "$lowercase" = "false" ] && first_re='[A-Za-z0-9]'
  local pat='^('"$type_re"')(\([a-z0-9-]+\))?!?: '"$first_re"
  local upper='[A-Z]'
  if ! [[ "$subject" =~ $pat ]]; then
    report "$label" "o assunto \"$subject\" não segue tipo(escopo): descrição, em minúsculas."
  elif [ "$lowercase" != "false" ] && [[ "${subject#*: }" =~ $upper ]]; then
    # Vale para a descrição inteira, sem exceção: siglas e identificadores também vão em minúsculas.
    report "$label" "a descrição do assunto \"$subject\" tem letra maiúscula; use só minúsculas (siglas e nomes também)."
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
    echo "uso: $0 <base> <head> | --message <arquivo> --author <e-mail> [--strip-comments] | --check-config" >&2
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
