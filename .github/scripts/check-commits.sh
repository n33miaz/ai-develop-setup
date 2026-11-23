#!/usr/bin/env bash
# Valida mensagens de commit. Uma implementação só, dois modos (o hook local e o CI usam o mesmo conjunto de regras):
#
#   check-commits.sh <base> <head> [--branch <nome>]
#       Modo CI: valida cada commit (sem merges) do intervalo base..head, lendo mensagem e autor do git.
#   check-commits.sh --message <arquivo> --author <e-mail> [--strip-comments] [--branch <nome>]
#       Modo mensagem: valida um arquivo de mensagem. É o que `.githooks/commit-msg` chama.
#       O hook passa --merge-subject <assunto> quando há um merge de develop em andamento.
#   Com --branch, o nome da branch passa pela mesma regra de texto público do assunto (termos de processo e códigos
#   internos de organização, ver devflow.toml).
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
  cfg_check_enum ai_attribution executor agent-author none
  local key raw email identity_re='^"[^"]+"$' emails=""
  for key in coauthor.owner agent.claude agent.codex; do
    cfg_has "$key" || [ "$(cfg_str ai_attribution executor)" = agent-author ] || continue
    raw="$(cfg_raw "$key")"
    if ! [[ "$raw" =~ $identity_re ]] || ! email="$(identity_email "$(cfg_str "$key")")"; then
      cfg_fail "$key" 'esperado "Nome <e-mail>".'
    elif in_list "$email" "$emails"; then
      cfg_fail "$key" "o e-mail precisa identificar uma conta distinta."
    else
      emails="$emails"$'
'"$email"
    fi
  done
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
  cfg_check_list commits.process_terms 0 '^[a-z0-9]+$'
  cfg_check_list commits.internal_code_patterns 0 regex
  cfg_check_enum signature.mode warn block off
  if [ "$cfg_errors" -gt 0 ]; then
    echo "Configuração inválida em $cfg: $cfg_errors problema(s). Nenhuma mensagem foi analisada." >&2
    exit 2
  fi
}

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

# Lista do arquivo, ou o padrão embutido quando a chave não existe (lista vazia no arquivo desliga a regra).
default_process_terms=$'round\nrounds\nfinding\nfindings\nrodada\nachado\nachados'
default_code_patterns=$'(^|[^a-z0-9])[a-z]{2}-[0-9]{2}([^a-z0-9]|$)\n(^|[^a-z0-9])t-?[0-9]{2,3}([^a-z0-9]|$)\n(^|[^a-z0-9])[em][0-9]{1,2}([^a-z0-9]|$)\n(^|[^a-z0-9])f[0-9]([^a-z0-9]|$)'
cfg_list_or() { # <chave> <padrão, um por linha>
  if cfg_has "$1"; then cfg_arr "$1"; else printf '%s\n' "$2"; fi
}

# check_public_text <rótulo> <o que é> <texto> <caracteres que formam palavra, para tr>
# Texto público descreve assunto e resultado: sem termos do processo interno nem códigos internos de organização.
check_public_text() {
  local label="$1" what="$2" text keep="$4" word p
  text="$(lower "$3")"
  local terms
  terms="$(cfg_list_or commits.process_terms "$default_process_terms")"
  while IFS= read -r word; do
    [ -z "$word" ] && continue
    if in_list "$word" "$terms"; then
      report "$label" "$what tem o termo de processo \"$word\"; descreva o assunto e o resultado (lista em commits.process_terms)."
    fi
  done < <(printf '%s\n' "$text" | tr -cs "$keep" '\n')
  while IFS= read -r p; do
    [ -z "$p" ] && continue
    if printf '%s\n' "$text" | grep -Eiq -- "$p"; then
      report "$label" "$what tem um código interno de organização (padrão de commits.internal_code_patterns)."
    fi
  done < <(cfg_list_or commits.internal_code_patterns "$default_code_patterns")
}

in_list() { # <item> <lista, um por linha>: sem diferença de maiúscula
  local want it
  want="$(lower "$1")"
  while IFS= read -r it; do
    [ "$(lower "$it")" = "$want" ] && return 0
  done <<< "$2"
  return 1
}

validate_config
[ "${1:-}" = "--check-config" ] && exit 0

# check_message <arquivo> <e-mail do autor> <rótulo> <1 = ignorar linhas que começam com #> [assunto padrão do merge]
check_message() {
  local file="$1" author="$2" label="$3" strip="${4:-0}" merge_subject="${5:-}"
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
  if [ "$subject" != "$merge_subject" ]; then
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
  fi
  if [[ "$subject" == *. ]]; then
    report "$label" "o assunto não pode terminar em ponto."
  fi
  # No assunto o hífen une a palavra ("round-trip" é uma palavra só, e não "round").
  check_public_text "$label" "o assunto" "$subject" 'a-z0-9-'

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

  if [ "$ai" = "agent-author" ]; then
    local owner agents
    owner="$(identity_email "$(cfg_str coauthor.owner)")"
    agents="$(identity_email "$(cfg_str agent.claude)")"$'
'"$(identity_email "$(cfg_str agent.codex)")"
    if in_list "$author" "$agents"; then
      if [ "${#co_emails[@]}" -ne 1 ] || ! in_list "${co_emails[0]:-}" "$owner"; then
        report "$label" "commit de agente exige exatamente um Co-authored-by do mantenedor."
      fi
    elif in_list "$author" "$owner"; then
      if [ "${#co_emails[@]}" -ne 0 ]; then
        report "$label" "commit do mantenedor não pode ter coautor no modo agent-author."
      fi
    else
      report "$label" "autor fora das contas configuradas em agent.* e coauthor.owner."
    fi
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

# check_branch <nome>: o nome da branch segue a mesma regra do assunto; aqui o hífen separa palavras.
check_branch() {
  [ -z "$1" ] && return 0
  [ "$(cfg_str commits.enabled true)" = "false" ] && return 0
  check_public_text "branch" "o nome da branch \"$1\"" "$1" 'a-z0-9'
}

usage="uso: $0 <base> <head> [--branch <nome>] | --message <arquivo> --author <e-mail> [--strip-comments] [--branch <nome>] [--merge-subject <assunto>] | --check-config"
mode="${1:-}"
if [ "$mode" = "--message" ]; then
  file="" author="" strip=0 branch="" merge_subject=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --message) file="${2:-}"; shift 2 ;;
      --author) author="${2:-}"; shift 2 ;;
      --strip-comments) strip=1; shift ;;
      --branch) branch="${2:-}"; shift 2 ;;
      --merge-subject) merge_subject="${2:?$usage}"; shift 2 ;;
      *) echo "$usage" >&2; exit 2 ;;
    esac
  done
  [ -z "$file" ] && { echo "$usage" >&2; exit 2; }
  [ -r "$file" ] || { echo "erro: arquivo ilegível: $file" >&2; exit 2; }
  check_message "$file" "$author" "mensagem" "$strip" "$merge_subject"
  check_branch "$branch"
else
  base="${1:-}"
  head="${2:-}"
  branch=""
  if [ -z "$base" ] || [ -z "$head" ]; then
    echo "$usage" >&2
    exit 2
  fi
  if [ $# -gt 2 ]; then
    if [ "$#" -eq 4 ] && [ "$3" = "--branch" ]; then branch="$4"; else echo "$usage" >&2; exit 2; fi
  fi
  [ "$(cfg_str commits.enabled true)" = "false" ] && { echo "Checagem de commits desligada (commits.enabled = false)."; exit 0; }
  check_branch "$branch"
  sig_mode="$(cfg_str signature.mode warn)"
  commits="$(git rev-list --reverse --no-merges "${base}..${head}")" || exit 2
  if [ -z "$commits" ] && [ "$errors" -eq 0 ]; then
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
