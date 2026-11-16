# 0006. URN namespace for contracts

Status: aceita em 14/11/2026

## Contexto
Todo contrato e todo evento carregam um identificador. O nome comercial do produto ainda vai mudar, e o identificador não pode mudar junto.

## Decisão
"(a) urn:ads:<tipo>:<nome>:<versão>".

## Consequências
- O identificador não contém nome de fornecedor de IA nem o nome comercial.
- A versão no fim é a versão maior do contrato; mudança incompatível cria identificador novo, nunca altera o antigo.
- O namespace não é registrado oficialmente; isso só importaria se os contratos virassem padrão público.

## Condição de reversão
Nenhuma depois do primeiro evento gravado: trocar o namespace exige migrar tudo o que já foi armazenado. Só se reabre por colisão comprovada com outro uso público de `urn:ads`.
