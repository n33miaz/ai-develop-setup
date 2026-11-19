#!/usr/bin/env bash
# Leitura de `defaults/devflow.toml` por grep/sed. Usado pelo hook, pelo checador e pelo teste (uma cópia só da leitura).
# Só entende o subconjunto descrito no cabeçalho do próprio devflow.toml.
# Depende de `cfg` apontando para o arquivo (o chamador define).

cfg_raw() {
  local k="${1//./\\.}"
  grep -E "^${k}[[:space:]]*=" "$cfg" | head -n1 | sed -E 's/^[^=]*=[[:space:]]*//; s/[[:space:]]+#.*$//'
}

# cfg_str <chave> [padrão]: texto sem as aspas externas.
cfg_str() {
  local v
  v="$(cfg_raw "$1")"
  v="${v#[\"\']}"
  v="${v%[\"\']}"
  printf '%s' "${v:-${2:-}}"
}

# cfg_arr <chave>: um item por linha, sem aspas.
cfg_arr() {
  cfg_raw "$1" | grep -oE "\"[^\"]*\"|'[^']*'" | sed -E 's/^.(.*).$/\1/'
}

identity_email() {
  local email_part='^([^<>]+) <([^[:space:]<>@]+@[^[:space:]<>@]+)>$'
  [[ "$1" =~ $email_part ]] || return 1
  printf '%s' "${BASH_REMATCH[2]}"
}
