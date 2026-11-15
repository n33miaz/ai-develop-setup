# ads-contracts

Tipos Rust de toda mensagem e todo registro, gerados dos JSON Schemas de [`contracts/schemas`](../../contracts/README.md) e válidos por construção. Sem dependência interna: é a base de que os outros crates dependem.

## O que há aqui

| Caminho | Conteúdo |
|---|---|
| `src/generated/` | Um módulo por Schema mais o índice `mod.rs`. **Gerado, não edite**: `npm run gen:rust` em `contracts/` regrava, e o CI reprova se estiver defasado |
| `src/support.rs` | A única parte escrita à mão: o erro `ContractError`, os padrões de texto, o `date-time` e o avaliador das regras entre campos |
| `tests/golden.rs` | Para todo Schema: cada dourado válido é lido no tipo gerado e reescrito no mesmo JSON; cada dourado inválido é recusado |
| `tests/shapes.rs` | O que os dourados não fixam sozinhos: ausente contra `null`, uniões, objeto aberto e validação na construção |
| [`swap-map.md`](swap-map.md) | Qual tipo gerado substitui cada tipo escrito à mão nos crates consumidores, e a diferença de forma |

Os tipos ficam todos na raiz do crate (`ads_contracts::CommonIdV1`). O nome do tipo de um Schema sai do `$id`, com a versão (`urn:ads:event:session.created:1` é `EventSessionCreatedV1`); o de um trecho interno é o nome do Schema mais o caminho (`EventSessionCreatedV1DataCols`).

## Como cada parte do Schema vira tipo

| No Schema | No Rust |
|---|---|
| texto com `pattern`, tamanho, `format` ou `not` | tipo próprio sobre `String`: `new`, `TryFrom<String>` e a leitura validam; `as_str` lê |
| texto sem restrição, booleano | `String`, `bool` |
| `integer` | `i64`; com `minimum: 0`, `u64`; com `minimum: 1`, `NonZeroU64`; com outros limites, tipo próprio que valida |
| `number` | `serde_json::Number` (nunca é NaN e volta ao JSON como veio); com limites, tipo próprio que valida |
| `enum` de textos ou de inteiros | `enum` com a marca do Schema em cada variante |
| `const` | tipo sem dados que só lê e só escreve aquele valor |
| lista | `Vec`; com `minItems`, `maxItems` ou `uniqueItems`, tipo próprio que valida |
| objeto fechado | `struct` com `deny_unknown_fields` |
| objeto aberto com propriedades | `struct` com o campo `extra` para o que o Schema não nomeia |
| objeto sem propriedades | `serde_json::Map`, ou `BTreeMap<String, T>` quando o Schema dá o tipo dos valores |
| obrigatório que aceita `null` | `Option<T>`; a falta do campo é recusada |
| opcional | `Option<T>`; `null` é recusado, a não ser que o Schema o aceite (aí `Option<Option<T>>`, que separa ausente de `null`) |
| `anyOf` | `enum` sem marca, uma variante por alternativa |
| `oneOf` de objetos com um campo de valor fixo | `enum` com esse campo como marca |
| `allOf` de um `$ref` com extensões (todo evento) | um `struct` só, com as propriedades do envelope apertadas pela extensão |
| `if`/`then`/`else`, `oneOf` de `required`, `not` e outras regras entre campos | tipo conferido: `X` guarda `XFields` e só nasce por `X::new(campos)` ou pela leitura, que avaliam as regras do Schema |

Um valor de um tipo gerado é sempre válido pelo Schema: quem tem um `RecordApprovalV1` não precisa conferir de novo. O que o tipo **não** garante é o que o Schema também não exprime (o `stream` de um evento nomear a entidade do `data`, o `plan_hash` bater com o plano, a ordem dos alvos): isso continua sendo regra de contrato de quem produz o valor.

Os tipos são mais estritos que o Schema em dois pontos, de propósito: inteiro escrito com fração (`1.0`) é recusado, e inteiro fora de 64 bits também.

## Acrescentar ou mudar um contrato

O caminho é o de [`contracts/README.md`](../../contracts/README.md#como-acrescentar-um-contrato): muda-se o Schema e os dourados, roda-se `npm run gen:rust` e os arquivos de `src/generated/` entram no mesmo commit. O gerador (`contracts/tools/gen-rust-lib.mjs`) para com erro quando encontra uma palavra-chave ou uma expressão regular que não sabe traduzir; nesse caso amplia-se o gerador, nunca o arquivo gerado.

O gerador nunca apaga: se um Schema sai, `npm run gen:rust` e `npm run gen:check` acusam o módulo que sobrou, e ele é removido pelo nome.
