#!/usr/bin/env bash
# Testes do checador de commits e do hook. Roda local, sem repositório: bash .github/scripts/check-commits.test.sh
# Usa só arquivos temporários (mktemp -d). Saída 0 = todos passaram.
set -u
export LC_ALL=C

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
checker="$here/check-commits.sh"
hook="$root/.githooks/commit-msg"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

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

echo
echo "Resultado: $passed passaram, $failed falharam."
[ "$failed" -eq 0 ]
