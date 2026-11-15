#!/usr/bin/env bash
# Testes do checador de commits e do hook. Roda local, sem repositório: bash .github/scripts/check-commits.test.sh
# Os arquivos de trabalho ficam numa pasta fixa da própria árvore, ignorada pelo git (.test-tmp/), regravados a cada
# rodada; o teste não apaga nada. Saída 0 = todos passaram.
set -u
export LC_ALL=C

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
checker="$here/check-commits.sh"
hook="$root/.githooks/commit-msg"
tmp="$root/.test-tmp/check-commits"
mkdir -p "$tmp" || exit 2

author='12345+usuario@users.noreply.github.com'
other='usuario@example.org'
sob="Signed-off-by: Fulano <$author>"
passed=0
failed=0

msg() { # <nome> <conteúdo com \n>: grava o arquivo da mensagem
  printf '%b' "$2" > "$tmp/$1.txt"
}

expect_pass() { # <nome> [e-mail do autor]
  if bash "$checker" --message "$tmp/$1.txt" --author "${2:-$author}" > "$tmp/out.txt" 2>&1; then
    passed=$((passed + 1)); echo "ok    passa: $1"
  else
    failed=$((failed + 1)); echo "FALHA deveria passar: $1"; sed 's/^/        /' "$tmp/out.txt"
  fi
}

expect_fail() { # <nome> [e-mail do autor]
  if bash "$checker" --message "$tmp/$1.txt" --author "${2:-$author}" > "$tmp/out.txt" 2>&1; then
    failed=$((failed + 1)); echo "FALHA deveria reprovar: $1"
  else
    passed=$((passed + 1)); echo "ok    reprova: $1"
  fi
}

expect_rc() { # <código esperado> <descrição> <mensagem> [toml]: confere o código de saída exato
  local rc=0
  ADS_DEVFLOW_TOML="${4:-$root/defaults/devflow.toml}" bash "$checker" --message "$tmp/$3.txt" --author "$author" > "$tmp/out.txt" 2>&1 || rc=$?
  if [ "$rc" -eq "$1" ]; then
    passed=$((passed + 1)); echo "ok    sai com $1: $2"
  else
    failed=$((failed + 1)); echo "FALHA esperava saída $1 e veio $rc: $2"; sed 's/^/        /' "$tmp/out.txt"
  fi
}

# ---- Passam
msg p-basico "feat(core): add event envelope\n\n$sob\n"
msg p-sem-escopo "fix: handle empty replay\n\n$sob\n"
msg p-breaking "feat(contracts)!: rename task field\n\n$sob\n"
msg p-claude "docs: describe update flow\n\n$sob\nCo-authored-by: Claude Sonnet <noreply@anthropic.com>\n"
msg p-codex "test(pty): cover resize\n\n$sob\nCo-authored-by: Codex <noreply@openai.com>\n"
msg p-50 "feat(core): $(printf 'a%.0s' $(seq 1 38))\n\n$sob\n"
msg p-digito "fix: 3d preview crash\n\n$sob\n"
msg p-crlf "feat: add thing\r\n\r\n$sob\r\n"
printf 'feat: with comments\n\n# a comment\n%s\n# another\n' "$sob" > "$tmp/p-comentario.txt"
expect_pass p-basico
expect_pass p-sem-escopo
expect_pass p-breaking
expect_pass p-claude
expect_pass p-codex
expect_pass p-50
expect_pass p-digito
expect_pass p-crlf
if bash "$checker" --message "$tmp/p-comentario.txt" --author "$author" --strip-comments > /dev/null 2>&1; then
  passed=$((passed + 1)); echo "ok    passa: p-comentario (com --strip-comments)"
else
  failed=$((failed + 1)); echo "FALHA deveria passar: p-comentario"
fi

# ---- Reprovam
msg f-sem-signoff "feat: add thing\n"
msg f-tipo "update(core): add thing\n\n$sob\n"
msg f-maiuscula "feat: Add thing\n\n$sob\n"
msg f-tipo-maiusculo "Feat: add thing\n\n$sob\n"
msg f-ponto "feat: add thing.\n\n$sob\n"
msg f-51 "feat(core): $(printf 'a%.0s' $(seq 1 39))\n\n$sob\n"
msg f-corpo "feat: add thing\n\nThis explains the change in prose.\n\n$sob\n"
msg f-corpo-colado "feat: add thing\nmore text\n\n$sob\n"
msg f-sem-branco "feat: add thing\n$sob\n"
msg f-dois-coautores "feat: add thing\n\n$sob\nCo-authored-by: Claude <noreply@anthropic.com>\nCo-authored-by: Codex <noreply@openai.com>\n"
msg f-coautor-desconhecido "feat: add thing\n\n$sob\nCo-authored-by: Someone <someone@example.org>\n"
msg f-trailer-desconhecido "feat: add thing\n\n$sob\nReviewed-by: Fulano <$author>\n"
msg f-signoff-outro-email "feat: add thing\n\nSigned-off-by: Fulano <$other>\n"
msg f-gerado "feat: add thing\n\n$sob\nGenerated with SomeTool\n"
msg f-codex-auto "feat: add thing\n\n$sob\nCo-authored-by: x <chatgpt-codex-connector@example.org>\n"
msg f-acento "feat: adicionar relé\n\n$sob\n"
msg f-sem-espaco "feat:add thing\n\n$sob\n"
msg f-vazia "\n\n"
msg f-so-assunto-sem-trailer "feat: add thing\n"
msg f-signoff-sem-email "feat: add thing\n\nSigned-off-by: Fulano\n"
expect_fail f-sem-signoff
expect_fail f-tipo
expect_fail f-maiuscula
expect_fail f-tipo-maiusculo
expect_fail f-ponto
expect_fail f-51
expect_fail f-corpo
expect_fail f-corpo-colado
expect_fail f-sem-branco
expect_fail f-dois-coautores
expect_fail f-coautor-desconhecido
expect_fail f-trailer-desconhecido
expect_fail f-signoff-outro-email "$other"
expect_fail f-gerado
expect_fail f-codex-auto
expect_fail f-acento
expect_fail f-sem-espaco
expect_fail f-vazia
expect_fail f-so-assunto-sem-trailer
expect_fail f-signoff-sem-email
# autor fora do padrão noreply (mensagem correta, e-mail errado)
msg f-autor "feat: add thing\n\nSigned-off-by: Fulano <$other>\n"
expect_fail f-autor "$other"

# ---- Minúsculas na descrição inteira (não só na primeira letra); o código de saída é 1, não 2
msg f-maiuscula-meio "feat: add UPPERCASE words\n\n$sob\n"
msg f-maiuscula-fim "feat: add thinG\n\n$sob\n"
msg f-sigla "docs: describe the API\n\n$sob\n"
msg f-identificador "fix(core): handle planHash\n\n$sob\n"
msg f-maiuscula-apos-digito "fix: 3D preview crash\n\n$sob\n"
msg p-minusculas-com-simbolos "fix(ci): keep utf-8, pty-host/1 and m1 ids\n\n$sob\n"
expect_rc 1 "maiúscula no meio da descrição" f-maiuscula-meio
expect_rc 1 "maiúscula no fim da descrição" f-maiuscula-fim
expect_rc 1 "sigla em maiúsculas" f-sigla
expect_rc 1 "identificador com maiúscula" f-identificador
expect_rc 1 "maiúscula depois de dígito" f-maiuscula-apos-digito
expect_rc 1 "maiúscula na primeira letra" f-maiuscula
expect_rc 0 "siglas, dígitos e símbolos em minúsculas" p-minusculas-com-simbolos

# ---- Parâmetros: o arquivo de configuração manda (P8)
sed -e 's/^ai_attribution = .*/ai_attribution = "none"/' "$root/defaults/devflow.toml" > "$tmp/none.toml"
if ADS_DEVFLOW_TOML="$tmp/none.toml" bash "$checker" --message "$tmp/p-claude.txt" --author "$author" > /dev/null 2>&1; then
  failed=$((failed + 1)); echo "FALHA ai_attribution=none deveria barrar coautor"
else
  passed=$((passed + 1)); echo "ok    reprova: coautor com ai_attribution=none"
fi
sed -e 's/^commits.enabled = .*/commits.enabled = false/' "$root/defaults/devflow.toml" > "$tmp/off.toml"
if ADS_DEVFLOW_TOML="$tmp/off.toml" bash "$checker" --message "$tmp/f-tipo.txt" --author "$author" > /dev/null 2>&1; then
  passed=$((passed + 1)); echo "ok    passa: commits.enabled=false desliga a checagem"
else
  failed=$((failed + 1)); echo "FALHA commits.enabled=false deveria desligar"
fi
sed -e 's/^commits.subject_max = .*/commits.subject_max = 20/' "$root/defaults/devflow.toml" > "$tmp/curto.toml"
if ADS_DEVFLOW_TOML="$tmp/curto.toml" bash "$checker" --message "$tmp/p-basico.txt" --author "$author" > /dev/null 2>&1; then
  failed=$((failed + 1)); echo "FALHA subject_max=20 deveria reprovar"
else
  passed=$((passed + 1)); echo "ok    reprova: subject_max vem do arquivo"
fi
if ADS_DEVFLOW_TOML="$tmp/nao-existe.toml" bash "$checker" --message "$tmp/p-basico.txt" --author "$author" > /dev/null 2>&1; then
  failed=$((failed + 1)); echo "FALHA configuração ausente deveria falhar fechado"
else
  passed=$((passed + 1)); echo "ok    reprova: configuração ausente falha fechado"
fi

sed -e 's/^commits.subject_lowercase = .*/commits.subject_lowercase = false/' "$root/defaults/devflow.toml" > "$tmp/livre.toml"
expect_rc 0 "subject_lowercase=false aceita maiúscula no meio" f-maiuscula-meio "$tmp/livre.toml"
expect_rc 0 "subject_lowercase=false aceita maiúscula no começo" f-maiuscula "$tmp/livre.toml"
expect_rc 1 "subject_lowercase=false não solta o tipo" f-tipo-maiusculo "$tmp/livre.toml"

# ---- Configuração inválida: sai com 2 antes de analisar a mensagem (que aqui é válida)
bad_cfg() { # <descrição> <expressão sed sobre o devflow.toml padrão>
  sed -e "$2" "$root/defaults/devflow.toml" > "$tmp/bad.toml"
  expect_rc 2 "configuração: $1" p-basico "$tmp/bad.toml"
}
bad_cfg "subject_max com texto" 's/^commits.subject_max = .*/commits.subject_max = "invalid"/'
bad_cfg "subject_max entre aspas" 's/^commits.subject_max = .*/commits.subject_max = "50"/'
bad_cfg "subject_max zero" 's/^commits.subject_max = .*/commits.subject_max = 0/'
bad_cfg "subject_max negativo" 's/^commits.subject_max = .*/commits.subject_max = -5/'
bad_cfg "subject_max acima do teto" 's/^commits.subject_max = .*/commits.subject_max = 1000/'
bad_cfg "subject_max fracionário" 's/^commits.subject_max = .*/commits.subject_max = 50.5/'
bad_cfg "subject_max vazio" 's/^commits.subject_max = .*/commits.subject_max =/'
bad_cfg "subject_max com expressão" 's/^commits.subject_max = .*/commits.subject_max = a[$(id)]/'
bad_cfg "max_coauthors com texto" 's/^commits.max_coauthors = .*/commits.max_coauthors = many/'
bad_cfg "enabled fora de true/false" 's/^commits.enabled = .*/commits.enabled = yes/'
bad_cfg "enabled entre aspas" 's/^commits.enabled = .*/commits.enabled = "false"/'
bad_cfg "signoff_required numérico" 's/^commits.signoff_required = .*/commits.signoff_required = 1/'
bad_cfg "subject_lowercase fora de true/false" 's/^commits.subject_lowercase = .*/commits.subject_lowercase = sometimes/'
bad_cfg "ai_attribution fora do domínio" 's/^ai_attribution = .*/ai_attribution = "robot"/'
bad_cfg "ai_attribution sem aspas" 's/^ai_attribution = .*/ai_attribution = none/'
bad_cfg "signature.mode fora do domínio" 's/^signature.mode = .*/signature.mode = "strict"/'
bad_cfg "profile fora do domínio" 's/^profile = .*/profile = "fast"/'
bad_cfg "types vazio" 's/^commits.types = .*/commits.types = []/'
bad_cfg "types não é lista" 's/^commits.types = .*/commits.types = "feat"/'
bad_cfg "types com metacaractere de regex" 's/^commits.types = .*/commits.types = ["feat", ".*"]/'
bad_cfg "types com lista aberta" 's/^commits.types = .*/commits.types = ["feat", "fix"/'
bad_cfg "allowed_trailers vazio" 's/^commits.allowed_trailers = .*/commits.allowed_trailers = []/'
bad_cfg "coauthors.allowed sem e-mail" 's/^coauthors.allowed = .*/coauthors.allowed = ["not an email"]/'
bad_cfg "author_email_regex que não compila" "s/^commits.author_email_regex = .*/commits.author_email_regex = '('/"
bad_cfg "blocked_patterns com regex que não compila" "s/^commits.blocked_patterns = .*/commits.blocked_patterns = ['generated', '(']/"
# o erro de configuração vence o desligamento e a mensagem ruim
sed -e 's/^commits.enabled = .*/commits.enabled = false/' -e 's/^commits.subject_max = .*/commits.subject_max = "invalid"/' "$root/defaults/devflow.toml" > "$tmp/bad.toml"
expect_rc 2 "configuração: inválida mesmo com commits.enabled=false" f-tipo "$tmp/bad.toml"
sed -e 's/^signature.mode = .*/signature.mode = "strict"/' "$root/defaults/devflow.toml" > "$tmp/bad.toml"
expect_rc 2 "configuração: inválida vence mensagem fora do padrão" f-tipo "$tmp/bad.toml"
# chave ausente continua caindo no padrão embutido; listas opcionais podem ficar vazias
printf 'commits.subject_max = 50\n' > "$tmp/parcial.toml"
expect_rc 0 "configuração parcial usa os padrões" p-basico "$tmp/parcial.toml"
expect_rc 1 "configuração parcial mantém a regra de minúsculas" f-maiuscula-meio "$tmp/parcial.toml"
sed -e 's/^commits.blocked_patterns = .*/commits.blocked_patterns = []/' -e 's/^coauthors.allowed = .*/coauthors.allowed = []/' "$root/defaults/devflow.toml" > "$tmp/vazias.toml"
expect_rc 0 "listas opcionais vazias são aceitas" p-basico "$tmp/vazias.toml"
expect_rc 1 "coautor fora de lista vazia é recusado" p-claude "$tmp/vazias.toml"

# ---- Hook (precisa do git instalado, não de repositório)
hook_run() { # <nome> <ADS_AI_AUTHOR> [toml]: roda o hook sobre o arquivo <nome>.txt
  GIT_AUTHOR_NAME="Fulano" GIT_AUTHOR_EMAIL="$author" GIT_COMMITTER_NAME="Fulano" GIT_COMMITTER_EMAIL="$author" \
    ADS_AI_AUTHOR="$2" ADS_DEVFLOW_TOML="${3:-$root/defaults/devflow.toml}" bash "$hook" "$tmp/$1.txt" > "$tmp/out.txt" 2>&1
}
hook_ok() { passed=$((passed + 1)); echo "ok    $1"; }
hook_bad() { failed=$((failed + 1)); echo "FALHA $1"; sed 's/^/        /' "$tmp/out.txt"; }

msg h1 "feat(core): add thing\n"
if hook_run h1 claude && grep -q '^Co-authored-by: Claude <noreply@anthropic.com>$' "$tmp/h1.txt" && grep -q "^Signed-off-by: Fulano <$author>$" "$tmp/h1.txt"; then
  hook_ok "hook: ADS_AI_AUTHOR=claude acrescenta coautor e sign-off, e a mensagem passa"
else hook_bad "hook claude"; fi

msg h2 "feat(core): add thing\n"
if hook_run h2 "" && ! grep -qi '^Co-authored-by' "$tmp/h2.txt" && grep -q '^Signed-off-by' "$tmp/h2.txt"; then
  hook_ok "hook: ADS_AI_AUTHOR vazia não acrescenta coautor"
else hook_bad "hook vazio"; fi

msg h3 "feat(core): add thing\n"
if hook_run h3 codex && grep -q '^Co-authored-by: Codex <noreply@openai.com>$' "$tmp/h3.txt"; then
  hook_ok "hook: ADS_AI_AUTHOR=codex acrescenta o coautor do Codex"
else hook_bad "hook codex"; fi

msg h4 "feat(core): add thing\n"
if hook_run h4 desconhecido; then hook_bad "hook deveria recusar ADS_AI_AUTHOR desconhecida"; else hook_ok "hook: recusa ADS_AI_AUTHOR sem entrada no devflow.toml"; fi

msg h5 "feat(core): add thing\n\nSigned-off-by: Fulano <$author>\nCo-authored-by: Claude <noreply@anthropic.com>\n"
if hook_run h5 claude && [ "$(grep -ci '^Co-authored-by' "$tmp/h5.txt")" -eq 1 ]; then
  hook_ok "hook: não duplica coautor já presente"
else hook_bad "hook duplicado"; fi

msg h6 "feat(core): add thing\n"
if hook_run h6 claude "$tmp/none.toml" && ! grep -qi '^Co-authored-by' "$tmp/h6.txt"; then
  hook_ok "hook: ai_attribution=none ignora ADS_AI_AUTHOR"
else hook_bad "hook none"; fi

msg h7 "Feat: Bad subject\n"
if hook_run h7 claude; then hook_bad "hook deveria reprovar assunto ruim"; else hook_ok "hook: reprova assunto fora do padrão"; fi

printf 'feat(core): add thing\n\n# Please enter the commit message\n# Lines starting with # are ignored\n' > "$tmp/h8.txt"
if hook_run h8 claude && grep -q '^Co-authored-by' "$tmp/h8.txt"; then
  hook_ok "hook: mensagem com comentários do editor continua válida"
else hook_bad "hook comentários"; fi

msg h9 "feat(core): add thing\n"
sed -e 's/^commits.subject_max = .*/commits.subject_max = "invalid"/' "$root/defaults/devflow.toml" > "$tmp/bad.toml"
if hook_run h9 claude "$tmp/bad.toml"; then hook_bad "hook deveria recusar configuração inválida"; else hook_ok "hook: recusa commit com configuração inválida"; fi

echo
echo "Resultado: $passed passaram, $failed falharam."
[ "$failed" -eq 0 ]
