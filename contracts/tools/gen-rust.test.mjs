import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { diffRust, generateRust, moduleName, rustName, rustPattern, writeRust } from "./gen-rust-lib.mjs";
import { freshDir } from "./test-tmp.mjs";

const D = "https://json-schema.org/draft/2020-12/schema";

function schema(id, body, example) {
  return { $schema: D, $id: id, title: "Coisa", description: "Uma coisa.", ...body, examples: [example] };
}

function tree() {
  const root = freshDir("gen-rust");
  const outDir = path.join(root, "out");
  fs.mkdirSync(path.join(root, "schemas"));
  const put = (name, s) => fs.writeFileSync(path.join(root, "schemas", name), JSON.stringify(s));
  put("common.id.v1.schema.json", schema("urn:ads:common:id:1", { type: "string", pattern: "^[a-z]+$" }, "abc"));
  put(
    "record.thing.v1.schema.json",
    schema(
      "urn:ads:record:thing:1",
      {
        type: "object",
        additionalProperties: false,
        required: ["id", "kind", "size", "parent"],
        properties: {
          id: { $ref: "urn:ads:common:id:1" },
          kind: { enum: ["fs.small", "fs.large"] },
          size: { type: "integer", minimum: 1, maximum: 10 },
          parent: { anyOf: [{ $ref: "urn:ads:common:id:1" }, { type: "null" }] },
          note: { type: "string" },
        },
        if: { properties: { kind: { const: "fs.large" } }, required: ["kind"] },
        then: { required: ["note"] },
      },
      { id: "abc", kind: "fs.small", size: 1, parent: null },
    ),
  );
  const generate = () => generateRust(root, { outDir });
  return { root, outDir, put, generate };
}

test("os nomes vêm do $id e incluem a versão", () => {
  assert.equal(rustName("urn:ads:event:session.created:1"), "EventSessionCreatedV1");
  assert.equal(moduleName("urn:ads:contract:harness-adapter:2"), "contract_harness_adapter_v2");
});

test("gera um módulo por Schema e o índice, com cabeçalho e $ref por URN", () => {
  const { generate } = tree();
  const files = generate();
  assert.deepEqual([...files.keys()].sort(), ["common_id_v1.rs", "mod.rs", "record_thing_v1.rs"]);
  const thing = files.get("record_thing_v1.rs");
  assert.match(thing, /^\/\/ Gerado por contracts\/tools\/gen-rust\.mjs a partir de contracts\/schemas\/record\.thing\.v1\.schema\.json\. Não edite à mão\./);
  assert.match(thing, /use super::common_id_v1::CommonIdV1;/);
  assert.match(thing, /pub id: CommonIdV1,/);
  const index = files.get("mod.rs");
  assert.match(index, /pub mod record_thing_v1;/);
  assert.match(index, /\("record\.thing\.v1", "urn:ads:record:thing:1"\)/);
  assert.match(index, /"urn:ads:record:thing:1" => support::reencode::<RecordThingV1>\(value\)/);
});

test("cada restrição do Schema vira tipo ou regra conferida", () => {
  const { generate } = tree();
  const files = generate();
  const id = files.get("common_id_v1.rs");
  assert.match(id, /Pattern::new\(r"\^\[a-z\]\+\$"\)/);
  assert.match(id, /#\[serde\(try_from = "String"\)\]\npub struct CommonIdV1\(String\);/);
  const thing = files.get("record_thing_v1.rs");
  // enum com a marca do Schema e sem o prefixo comum; inteiro com limites; obrigatório que aceita null; opcional
  assert.match(thing, /#\[serde\(rename = "fs\.small"\)\]\n\s+Small,/);
  assert.match(thing, /\(1\.\.=10\)\.contains\(&value\)/);
  assert.match(thing, /#\[serde\(deserialize_with = "support::nullable"\)\]\n\s+pub parent: Option<CommonIdV1>,/);
  assert.match(thing, /skip_serializing_if = "Option::is_none",\s+deserialize_with = "support::present"/);
  // if/then vira regra do tipo conferido, que só nasce por `new` ou pela leitura
  assert.match(thing, /#\[serde\(try_from = "RecordThingV1Fields"\)\]\npub struct RecordThingV1\(RecordThingV1Fields\);/);
  assert.match(thing, /Rule::If \{/);
  assert.match(thing, /#\[serde\(deny_unknown_fields\)\]\npub struct RecordThingV1Fields \{/);
});

test("os limites de tamanho viram constantes públicas, a fonte única para quem confere antes", () => {
  const { put, generate } = tree();
  const body = {
    type: "array",
    minItems: 1,
    maxItems: 7,
    items: { type: "string", minLength: 1, maxLength: 9 },
  };
  put("record.list.v1.schema.json", schema("urn:ads:record:list:1", body, ["a"]));
  const list = generate().get("record_list_v1.rs");
  assert.match(list, /impl RecordListV1 \{\n\s+pub const MAX_ITEMS: usize = 7;/);
  assert.match(list, /\(1\.\.=Self::MAX_ITEMS\)\.contains\(&items\.len\(\)\)/);
  assert.match(list, /impl RecordListV1Item \{\n\s+pub const MAX_CHARS: usize = 9;/);
  assert.match(list, /\(1\.\.=Self::MAX_CHARS\)\.contains\(&length\)/);
  assert.doesNotMatch(generate().get("common_id_v1.rs"), /MAX_CHARS/);
});

test("a geração é determinística", () => {
  const { generate } = tree();
  assert.deepEqual([...generate()], [...generate()]);
});

test("saída em dia passa; desatualizado, faltando ou sobrando reprova; nada é apagado", () => {
  const { outDir, put, generate } = tree();
  assert.deepEqual(writeRust(outDir, generate()), []);
  assert.deepEqual(diffRust(outDir, generate(), "out"), []);

  fs.appendFileSync(path.join(outDir, "record_thing_v1.rs"), "// edição à mão\n");
  assert.deepEqual(diffRust(outDir, generate(), "out"), ["desatualizado: out/record_thing_v1.rs"]);

  writeRust(outDir, generate());
  fs.unlinkSync(path.join(outDir, "common_id_v1.rs"));
  assert.deepEqual(diffRust(outDir, generate(), "out"), ["faltando: out/common_id_v1.rs"]);

  writeRust(outDir, generate());
  fs.writeFileSync(path.join(outDir, "record_old_v1.rs"), "x");
  assert.deepEqual(diffRust(outDir, generate(), "out"), ["sobrando: out/record_old_v1.rs"]);
  assert.deepEqual(writeRust(outDir, generate()), ["record_old_v1.rs"]);
  assert.ok(fs.existsSync(path.join(outDir, "record_old_v1.rs")));
  fs.unlinkSync(path.join(outDir, "record_old_v1.rs"));

  // mudar o Schema sem regerar também reprova
  put("common.id.v1.schema.json", schema("urn:ads:common:id:1", { type: "string", pattern: "^[a-z]{2,}$" }, "abc"));
  assert.deepEqual(diffRust(outDir, generate(), "out"), ["desatualizado: out/common_id_v1.rs", "desatualizado: out/mod.rs"]);
});

test("o que o gerador não sabe traduzir derruba a geração", () => {
  const cases = {
    "palavra-chave desconhecida": [{ type: "string", contentEncoding: "base64" }, /contentEncoding/],
    "limite que o tipo não cobre": [{ type: "integer", exclusiveMinimum: 0 }, /exclusiveMinimum/],
    "palavra-chave desconhecida numa regra": [
      { type: "object", additionalProperties: false, properties: { a: { type: "string" } }, if: { properties: { a: { minProperties: 1 } } }, then: {} },
      /minProperties/,
    ],
    "oneOf sem marca": [{ oneOf: [{ type: "string" }, { type: "integer" }] }, /oneOf sem campo de valor fixo/],
    "$ref inexistente": [{ $ref: "urn:ads:common:nope:1" }, /\$ref sem Schema correspondente/],
    "required fora de properties": [{ type: "object", additionalProperties: false, properties: {}, required: ["a"] }, /required cita "a"/],
    "padrão fora do subconjunto": [{ type: "string", pattern: "^(?=a)" }, /fora do subconjunto suportado/],
  };
  for (const [label, [body, expected]] of Object.entries(cases)) {
    const { put, generate } = tree();
    put("record.bad.v1.schema.json", schema("urn:ads:record:bad:1", body, null));
    assert.throws(generate, expected, label);
  }
});

test("padrões: só passa o que significa o mesmo nos dois motores", () => {
  assert.equal(rustPattern("^[^\\u0000-\\u001f]*$", "x"), "^[^\\x{0000}-\\x{001f}]*$");
  assert.equal(rustPattern("(^|[\\\\/])\\.\\.?([\\\\/]|$)", "x"), "(^|[\\\\/])\\.\\.?([\\\\/]|$)");
  assert.equal(rustPattern("^(?:[A-Z]{4})*a{2,}\\/$", "x"), "^(?:[A-Z]{4})*a{2,}/$");
  assert.equal(
    rustPattern("^[\\p{L}\\p{Zs} ]*[\\u{E0100}-\\u{E01EF}]$", "x"),
    "^[\\p{L}\\p{Zs} ]*[\\x{E0100}-\\x{E01EF}]$",
  );
  for (const bad of ["a.b", "\\d+", "\\p{Lu}", "\\p{Script=Latin}", "\\P{L}", "\\u{110000}", "\\u{}", "\\bx", "(?!a)", "(?<n>a)", "a+?", "a*+", "[a[b]]", "[a&&b]", "a{x}", "[abc", "\\1", "a}"]) {
    assert.throws(() => rustPattern(bad, "x"), /fora do subconjunto suportado/, bad);
  }
});

test("extensão fechada de um objeto com propriedades vira struct fechado só com o que ela nomeia", () => {
  const { put, generate } = tree();
  const base = {
    type: "object",
    additionalProperties: false,
    required: ["method", "params"],
    properties: {
      method: { type: "string", pattern: "^[a-z.]+$" },
      params: { type: "object", additionalProperties: true, required: ["_meta"], properties: { _meta: { $ref: "urn:ads:common:id:1" } } },
    },
  };
  put("command.request.v1.schema.json", schema("urn:ads:command:request:1", base, { method: "a", params: { _meta: "abc" } }));
  const ext = (params) => ({
    allOf: [{ $ref: "urn:ads:command:request:1" }, { type: "object", required: ["method", "params"], properties: { method: { const: "x.go" }, params } }],
  });
  const closed = { type: "object", additionalProperties: false, required: ["_meta", "n"], properties: { _meta: true, n: { type: "integer", minimum: 1, maximum: 9 } } };
  put("command.x.v1.schema.json", schema("urn:ads:command:x:1", ext(closed), null));
  const x = generate().get("command_x_v1.rs");
  assert.match(x, /#\[serde\(deny_unknown_fields\)\]\npub struct CommandXV1Params \{\n\s+#\[serde\(rename = "_meta"\)\]\n\s+pub meta: CommonIdV1,\n\s+pub n: CommandXV1ParamsN,\n\}/);

  const bad = {
    "true sem propriedade na base": [{ ...closed, properties: { ...closed.properties, other: true } }, /true herda da base/],
    "obrigatório da base fora da extensão": [{ ...closed, required: ["n"], properties: { n: closed.properties.n } }, /"_meta" é obrigatório/],
    "regra além da forma": [{ ...closed, minProperties: 1 }, /além da forma/],
  };
  for (const [label, [params, expected]] of Object.entries(bad)) {
    put("command.x.v1.schema.json", schema("urn:ads:command:x:1", ext(params), null));
    assert.throws(generate, expected, label);
  }
});
