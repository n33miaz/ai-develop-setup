# 0005. Column-level encryption at rest

Status: aceita em 14/11/2026

## Contexto
A decisão anterior era cifrar o banco inteiro, com cifra por coluna como plano B já aceito. Cifrar por coluna evita depender de uma variante do SQLite fora do caminho comum.

## Decisão
"(a) SQLite comum + cifra por coluna nos campos sensíveis".

## Consequências
- Texto de terminal, prompts e caminhos são gravados com cifra autenticada; a chave fica no cofre do sistema operacional, nunca em arquivo.
- Datas, tamanhos e identificadores ficam em claro; coluna cifrada não é indexável.
- Teste obrigatório: gravar, reabrir, recusar chave errada e não encontrar uma frase sentinela em claro no arquivo.
- Parâmetro `storage.encryption` documenta a escolha e as alternativas.

## Condição de reversão
Passar ao banco inteiro cifrado se os metadados em claro forem classificados como sensíveis numa revisão de segurança.
