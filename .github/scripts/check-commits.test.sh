#!/usr/bin/env bash
# Testes do checador de commits e do hook. Roda local, sem repositório: bash .github/scripts/check-commits.test.sh
# Os arquivos de trabalho ficam numa pasta fixa da própria árvore, ignorada pelo git (.test-tmp/), regravados a cada
# execução; o teste não apaga nada. Saída 0 = todos passaram.
set -u
export LC_ALL=C

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
checker="$here/check-commits.sh"
hook="$root/.githooks/commit-msg"
tmp="$root/.test-tmp/check-commits"
mkdir -p "$tmp" || exit 2

agent_cfg="$root/defaults/devflow.toml"
legacy_cfg="$tmp/executor.toml"
sed 's/^ai_attribution = .*/ai_attribution = "executor"/' "$agent_cfg" > "$legacy_cfg" || exit 2
export ADS_DEVFLOW_TOML="$legacy_cfg"

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
  ADS_DEVFLOW_TOML="${4:-$legacy_cfg}" bash "$checker" --message "$tmp/$3.txt" --author "$author" > "$tmp/out.txt" 2>&1 || rc=$?
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
msg p-minusculas-com-simbolos "fix(ci): keep utf-8, pty-host/1 and v2 ids\n\n$sob\n"
expect_rc 1 "maiúscula no meio da descrição" f-maiuscula-meio
expect_rc 1 "maiúscula no fim da descrição" f-maiuscula-fim
expect_rc 1 "sigla em maiúsculas" f-sigla
expect_rc 1 "identificador com maiúscula" f-identificador
expect_rc 1 "maiúscula depois de dígito" f-maiuscula-apos-digito
expect_rc 1 "maiúscula na primeira letra" f-maiuscula
expect_rc 0 "siglas, dígitos e símbolos em minúsculas" p-minusculas-com-simbolos

# ---- Texto público: termos de processo e códigos internos no assunto (código 1)
msg p-round-trip "test(contracts): cover the round-trip of v2\n\n$sob\n"
msg p-legitimos "fix: keep utf8, x86_64, e2e, f32 and issue 42\n\n$sob\n"
msg p-palavra-parecida "docs: describe the roundtable and surround\n\n$sob\n"
expect_rc 0 "round-trip é uma palavra só no assunto" p-round-trip
expect_rc 0 "utf8, x86_64, e2e, f32 e número de issue passam" p-legitimos
expect_rc 0 "termo só como parte de outra palavra passa" p-palavra-parecida
for t in round rounds finding findings rodada achado achados; do
  msg "f-termo-$t" "fix(core): close the gate $t\n\n$sob\n"
  expect_rc 1 "termo de processo \"$t\" no assunto" "f-termo-$t"
done
msg f-codigo-marco "feat(contracts): add m1 event families\n\n$sob\n"
msg f-codigo-cartao "fix(core): close ad-17 in the gate\n\n$sob\n"
msg f-codigo-tarefa "fix(core): close t-012 in the gate\n\n$sob\n"
msg f-codigo-tarefa-curto "fix(core): close t12 in the gate\n\n$sob\n"
msg f-codigo-etapa "feat(ui): finish e1 screens\n\n$sob\n"
msg f-codigo-fase "chore: start f0 setup\n\n$sob\n"
expect_rc 1 "código de marco (m1) no assunto" f-codigo-marco
expect_rc 1 "código de cartão (ad-17) no assunto" f-codigo-cartao
expect_rc 1 "código de tarefa (t-012) no assunto" f-codigo-tarefa
expect_rc 1 "código de tarefa (t12) no assunto" f-codigo-tarefa-curto
expect_rc 1 "código de etapa (e1) no assunto" f-codigo-etapa
expect_rc 1 "código de fase (f0) no assunto" f-codigo-fase
# sem diferenciar maiúsculas: com a regra de minúsculas desligada, a maiúscula não escapa
sed -e 's/^commits.subject_lowercase = .*/commits.subject_lowercase = false/' "$legacy_cfg" > "$tmp/livre2.toml"
msg f-termo-maiusculo "fix(core): close gate Review Findings\n\n$sob\n"
msg f-codigo-maiusculo "fix(core): close AD-17 and M1\n\n$sob\n"
expect_rc 1 "termo de processo em maiúsculas" f-termo-maiusculo "$tmp/livre2.toml"
expect_rc 1 "código interno em maiúsculas" f-codigo-maiusculo "$tmp/livre2.toml"
# as listas vêm do arquivo: vazias desligam; chave ausente cai no padrão embutido
sed -e 's/^commits.process_terms = .*/commits.process_terms = []/' -e 's/^commits.internal_code_patterns = .*/commits.internal_code_patterns = []/' "$legacy_cfg" > "$tmp/sem-texto.toml"
expect_rc 0 "process_terms vazia desliga os termos" f-termo-findings "$tmp/sem-texto.toml"
expect_rc 0 "internal_code_patterns vazia desliga os códigos" f-codigo-marco "$tmp/sem-texto.toml"
sed -e '/^commits.process_terms = /d' -e '/^commits.internal_code_patterns = /d' "$legacy_cfg" > "$tmp/sem-chave.toml"
expect_rc 1 "sem a chave, os termos do padrão embutido valem" f-termo-rodada "$tmp/sem-chave.toml"
expect_rc 1 "sem a chave, os códigos do padrão embutido valem" f-codigo-cartao "$tmp/sem-chave.toml"
sed -e 's/^commits.process_terms = .*/commits.process_terms = ["sprint"]/' "$legacy_cfg" > "$tmp/outro-termo.toml"
msg f-termo-sprint "feat(ui): finish sprint screens\n\n$sob\n"
expect_rc 1 "termo vindo do arquivo" f-termo-sprint "$tmp/outro-termo.toml"
expect_rc 0 "termo fora da lista do arquivo passa" f-termo-rodada "$tmp/outro-termo.toml"

# ---- Texto público: nome da branch (o hífen separa palavras)
expect_branch() { # <código esperado> <branch>
  local rc=0
  bash "$checker" --message "$tmp/p-basico.txt" --author "$author" --branch "$2" > "$tmp/out.txt" 2>&1 || rc=$?
  if [ "$rc" -eq "$1" ]; then
    passed=$((passed + 1)); echo "ok    branch sai com $1: $2"
  else
    failed=$((failed + 1)); echo "FALHA branch esperava saída $1 e veio $rc: $2"; sed 's/^/        /' "$tmp/out.txt"
  fi
}
expect_branch 0 "feature/52-public-text-rule"
expect_branch 0 "fix/27-pty-host-responsiveness"
expect_branch 0 "feature/14-ui-contract-types"
expect_branch 0 ""
expect_branch 1 "feature/34-gate-review-round-2"
expect_branch 1 "feature/36-fs-executor-findings"
expect_branch 1 "feature/12-ad-17-gate"
expect_branch 1 "feature/9-m1-contracts"
expect_branch 1 "feature/20-T-012-gaps"

# ---- Parâmetros: o arquivo de configuração manda (P8)
sed -e 's/^ai_attribution = .*/ai_attribution = "none"/' "$legacy_cfg" > "$tmp/none.toml"
if ADS_DEVFLOW_TOML="$tmp/none.toml" bash "$checker" --message "$tmp/p-claude.txt" --author "$author" > /dev/null 2>&1; then
  failed=$((failed + 1)); echo "FALHA ai_attribution=none deveria barrar coautor"
else
  passed=$((passed + 1)); echo "ok    reprova: coautor com ai_attribution=none"
fi
sed -e 's/^commits.enabled = .*/commits.enabled = false/' "$legacy_cfg" > "$tmp/off.toml"
if ADS_DEVFLOW_TOML="$tmp/off.toml" bash "$checker" --message "$tmp/f-tipo.txt" --author "$author" > /dev/null 2>&1; then
  passed=$((passed + 1)); echo "ok    passa: commits.enabled=false desliga a checagem"
else
  failed=$((failed + 1)); echo "FALHA commits.enabled=false deveria desligar"
fi
sed -e 's/^commits.subject_max = .*/commits.subject_max = 20/' "$legacy_cfg" > "$tmp/curto.toml"
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

sed -e 's/^commits.subject_lowercase = .*/commits.subject_lowercase = false/' "$legacy_cfg" > "$tmp/livre.toml"
expect_rc 0 "subject_lowercase=false aceita maiúscula no meio" f-maiuscula-meio "$tmp/livre.toml"
expect_rc 0 "subject_lowercase=false aceita maiúscula no começo" f-maiuscula "$tmp/livre.toml"
expect_rc 1 "subject_lowercase=false não solta o tipo" f-tipo-maiusculo "$tmp/livre.toml"

# ---- Configuração inválida: sai com 2 antes de analisar a mensagem (que aqui é válida)
bad_cfg() { # <descrição> <expressão sed sobre o devflow.toml padrão>
  sed -e "$2" "$legacy_cfg" > "$tmp/bad.toml"
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
bad_cfg "process_terms com maiúscula" 's/^commits.process_terms = .*/commits.process_terms = ["Round"]/'
bad_cfg "process_terms com espaço" 's/^commits.process_terms = .*/commits.process_terms = ["review round"]/'
bad_cfg "process_terms não é lista" 's/^commits.process_terms = .*/commits.process_terms = "round"/'
bad_cfg "internal_code_patterns com regex que não compila" "s/^commits.internal_code_patterns = .*/commits.internal_code_patterns = ['[a-z']/"
# o erro de configuração vence o desligamento e a mensagem ruim
sed -e 's/^commits.enabled = .*/commits.enabled = false/' -e 's/^commits.subject_max = .*/commits.subject_max = "invalid"/' "$legacy_cfg" > "$tmp/bad.toml"
expect_rc 2 "configuração: inválida mesmo com commits.enabled=false" f-tipo "$tmp/bad.toml"
sed -e 's/^signature.mode = .*/signature.mode = "strict"/' "$legacy_cfg" > "$tmp/bad.toml"
expect_rc 2 "configuração: inválida vence mensagem fora do padrão" f-tipo "$tmp/bad.toml"
# chave ausente continua caindo no padrão embutido; listas opcionais podem ficar vazias
printf 'commits.subject_max = 50\n' > "$tmp/parcial.toml"
expect_rc 0 "configuração parcial usa os padrões" p-basico "$tmp/parcial.toml"
expect_rc 1 "configuração parcial mantém a regra de minúsculas" f-maiuscula-meio "$tmp/parcial.toml"
sed -e 's/^commits.blocked_patterns = .*/commits.blocked_patterns = []/' -e 's/^coauthors.allowed = .*/coauthors.allowed = []/' "$legacy_cfg" > "$tmp/vazias.toml"
expect_rc 0 "listas opcionais vazias são aceitas" p-basico "$tmp/vazias.toml"
expect_rc 1 "coautor fora de lista vazia é recusado" p-claude "$tmp/vazias.toml"

# ---- Hook (precisa do git instalado, não de repositório)
hook_run() { # <nome> <ADS_AI_AUTHOR> [toml]: roda o hook sobre o arquivo <nome>.txt
  GIT_AUTHOR_NAME="${4:-Fulano}" GIT_AUTHOR_EMAIL="${5:-$author}" \
    GIT_COMMITTER_NAME="${4:-Fulano}" GIT_COMMITTER_EMAIL="${6:-${5:-$author}}" \
    ADS_AI_AUTHOR="$2" ADS_DEVFLOW_TOML="${3:-$legacy_cfg}" bash "$hook" "$tmp/$1.txt" > "$tmp/out.txt" 2>&1
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
sed -e 's/^commits.subject_max = .*/commits.subject_max = "invalid"/' "$legacy_cfg" > "$tmp/bad.toml"
if hook_run h9 claude "$tmp/bad.toml"; then hook_bad "hook deveria recusar configuração inválida"; else hook_ok "hook: recusa commit com configuração inválida"; fi

# ---- Hook com a checagem desligada: a configuração é validada mesmo assim, e a mensagem não é lida nem alterada
hook_off() { # <código esperado> <descrição> <nome> <toml>: código de saída exato e mensagem intacta
  local rc=0
  cp "$tmp/$3.txt" "$tmp/$3.before.txt"
  hook_run "$3" claude "$4" || rc=$?
  if [ "$rc" -eq "$1" ] && cmp -s "$tmp/$3.txt" "$tmp/$3.before.txt"; then
    hook_ok "hook: sai com $1 e não toca na mensagem: $2"
  else
    echo "        esperava saída $1 e veio $rc" >> "$tmp/out.txt"
    hook_bad "hook: $2"
  fi
}
msg h10 "Feat: Bad subject
"
sed -e 's/^commits.enabled = .*/commits.enabled = false/' -e 's/^commits.subject_max = .*/commits.subject_max = "invalid"/' "$legacy_cfg" > "$tmp/bad-off.toml"
hook_off 2 "configuração inválida com commits.enabled=false" h10 "$tmp/bad-off.toml"
sed -e 's/^commits.enabled = .*/commits.enabled = "false"/' "$legacy_cfg" > "$tmp/bad-off.toml"
hook_off 2 "commits.enabled entre aspas não desliga a checagem" h10 "$tmp/bad-off.toml"
hook_off 0 "configuração válida com commits.enabled=false" h10 "$tmp/off.toml"

# ---- Modo --check-config: só a configuração, sem mensagem
check_config() { # <código esperado> <descrição> <toml>
  local rc=0
  ADS_DEVFLOW_TOML="$3" bash "$checker" --check-config > "$tmp/out.txt" 2>&1 || rc=$?
  if [ "$rc" -eq "$1" ]; then
    passed=$((passed + 1)); echo "ok    --check-config sai com $1: $2"
  else
    failed=$((failed + 1)); echo "FALHA --check-config esperava saída $1 e veio $rc: $2"; sed 's/^/        /' "$tmp/out.txt"
  fi
}
check_config 0 "configuração padrão" "$agent_cfg"
check_config 0 "configuração válida com a checagem desligada" "$tmp/off.toml"
check_config 2 "configuração inválida" "$tmp/bad.toml"
check_config 2 "configuração ausente" "$tmp/nao-existe.toml"

# ---- Autoria de agente
cfg="$agent_cfg"
. "$here/lib-devflow.sh"
agent="$(cfg_str agent.codex)"
agent_email="$(identity_email "$agent")"
owner="$(cfg_str coauthor.owner)"
owner_email="$(identity_email "$owner")"
subject="feat(core): add thing"

msg a-valid "$subject\n\nSigned-off-by: $agent\nCo-authored-by: $owner\n"
msg a-missing "$subject\n\nSigned-off-by: $agent\n"
msg a-wrong-coauthor "$subject\n\nSigned-off-by: $agent\nCo-authored-by: Claude <noreply@anthropic.com>\n"
msg a-owner "$subject\n\nSigned-off-by: $owner\n"
msg a-owner-coauthor "$subject\n\nSigned-off-by: $owner\nCo-authored-by: $owner\n"
ADS_DEVFLOW_TOML="$agent_cfg" expect_pass a-valid "$agent_email"
ADS_DEVFLOW_TOML="$agent_cfg" expect_fail a-missing "$agent_email"
ADS_DEVFLOW_TOML="$agent_cfg" expect_fail a-wrong-coauthor "$agent_email"
ADS_DEVFLOW_TOML="$agent_cfg" expect_fail p-basico "$author"
ADS_DEVFLOW_TOML="$agent_cfg" expect_pass a-owner "$owner_email"
ADS_DEVFLOW_TOML="$agent_cfg" expect_fail a-owner-coauthor "$owner_email"

msg ha-valid "$subject\n"
if hook_run ha-valid codex "$agent_cfg" "${agent% <*}" "$agent_email" &&
   grep -Fxq "Co-authored-by: $owner" "$tmp/ha-valid.txt" &&
   grep -Fxq "Signed-off-by: $agent" "$tmp/ha-valid.txt"; then
  hook_ok "hook: acrescenta coautoria do mantenedor e DCO do agente"
else hook_bad "hook: autoria de agente válida"; fi

msg ha-owner "$subject\n"
if hook_run ha-owner "" "$agent_cfg" "${owner% <*}" "$owner_email" &&
   ! grep -qi '^Co-authored-by:' "$tmp/ha-owner.txt"; then
  hook_ok "hook: aceita commit do mantenedor sem coautor"
else hook_bad "hook: autoria do mantenedor"; fi

hook_expect_rc() {
  local expected="$1" description="$2" rc=0
  shift 2
  hook_run "$@" || rc=$?
  if [ "$rc" -eq "$expected" ]; then
    hook_ok "$description"
  else
    hook_bad "$description (esperava $expected, veio $rc)"
  fi
}
hook_expect_rc 0 "hook: aceita trailer do mantenedor já presente" a-valid "" "$agent_cfg" "${agent% <*}" "$agent_email"
hook_expect_rc 1 "hook: exige coautor em commit de agente" a-missing "" "$agent_cfg" "${agent% <*}" "$agent_email"
hook_expect_rc 1 "hook: recusa outro agente como coautor" a-wrong-coauthor codex "$agent_cfg" "${agent% <*}" "$agent_email"
hook_expect_rc 1 "hook: recusa autor desconhecido" p-basico "" "$agent_cfg"
hook_expect_rc 1 "hook: recusa autor diferente de agent.codex" ha-valid codex "$agent_cfg"
hook_expect_rc 1 "hook: recusa committer diferente de agent.codex" ha-valid codex "$agent_cfg" "${agent% <*}" "$agent_email" "$owner_email"

sed '/^agent.codex = /d' "$agent_cfg" > "$tmp/missing-agent.toml"
check_config 2 "agent-author exige identidade do agente" "$tmp/missing-agent.toml"

echo
echo "Resultado: $passed passaram, $failed falharam."
[ "$failed" -eq 0 ]
