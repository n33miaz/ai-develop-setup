# 0001. Proprietary, source-available license

Status: aceita em 14/11/2026

## Contexto
O repositório é público e precisa de um arquivo LICENSE no primeiro commit. A licença define quem pode usar o código, quais bibliotecas podem entrar e se serviços grátis de assinatura para código aberto ficam disponíveis.

## Decisão
"(a) Copiar o texto do [outro projeto do autor], trocando o nome do produto" (o nome do projeto foi omitido da citação), isto é, a licença proprietária de código à vista usada lá: o código pode ser lido e avaliado; usar, copiar, modificar ou distribuir exige licença escrita.

## Consequências
- Dependências: MIT, Apache-2.0, BSD, ISC, Zlib e Unicode passam; GPL e AGPL nunca entram; MPL-2.0 e LGPL só por exceção registrada em ADR. O portão é o `cargo-deny` no CI.
- Programas de assinatura grátis que exigem licença aberta ficam fora.
- O DCO continua sendo cobrado (ADR 0012), mas contribuição externa fica fechada até existir um contrato de contribuição.
- Ferramenta GPL só por subprocesso, cada uma com ADR própria.

## Condição de reversão
Reavaliar quando o produto for oferecido a terceiros. Trocar para licença aberta é possível a qualquer momento; o inverso não desfaz o que já foi publicado.
