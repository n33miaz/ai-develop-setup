import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { checkContracts, idToKey, lintClosedObjects, URN_RE } from "./lib.mjs";
import { freshDir } from "./test-tmp.mjs";

const D = "https://json-schema.org/draft/2020-12/schema";

function baseSchema(extra = {}) {
  return {
    $schema: D,
    $id: "urn:ads:record:thing:1",
    title: "Coisa",
    description: "Uma coisa de teste.",
    type: "object",
    additionalProperties: false,
    required: ["n"],
    properties: { n: { type: "integer", minimum: 0 } },
    examples: [{ n: 1 }],
    ...extra,
  };
}

// Monta uma árvore de contratos mínima e válida; o teste estraga o que quiser antes de ela ir para o disco.
function fixture(mutate) {
  const files = new Map();
  const put = (rel, data) => files.set(rel, typeof data === "string" ? data : JSON.stringify(data));
  put("schemas/record.thing.v1.schema.json", baseSchema());
  put("golden/record.thing.v1/valid/01.json", { n: 1 });
  put("golden/record.thing.v1/valid/02.json", { n: 2 });
  put("golden/record.thing.v1/invalid/01-negative.json", { n: -1 });
  put("golden/record.thing.v1/invalid/02-text.json", { n: "x" });
  put("golden/record.thing.v1/invalid/03-extra.json", { n: 1, x: 1 });
  put("golden/record.thing.v1/invalid/index.json", {
    "01-negative.json": { rule: "n não pode ser negativo", keyword: "minimum", instancePath: "/n" },
    "02-text.json": { rule: "n é inteiro", keyword: "type", instancePath: "/n" },
    "03-extra.json": { rule: "objeto fechado", keyword: "additionalProperties", instancePath: "", params: { additionalProperty: "x" } },
  });
  put("README.md", "| `urn:ads:record:thing:1` | coisa |\n");
  mutate?.({ put, rm: (rel) => files.delete(rel) });

  const root = freshDir("validate");
  for (const [rel, text] of files) {
    const file = path.join(root, rel);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, text);
  }
  return root;
}

function run(mutate) {
  return checkContracts(fixture(mutate));
}

function assertFails(result, fragment) {
  assert.ok(result.errors.some((e) => e.includes(fragment)), `esperava erro com "${fragment}"; veio:\n${result.errors.join("\n")}`);
}

test("árvore mínima válida passa e conta os dourados", () => {
  const r = run();
  assert.deepEqual(r.errors, []);
  assert.deepEqual(r.stats, { schemas: 1, valid: 2, invalid: 3 });
});

test("URN: aceita o padrão e recusa o resto", () => {
  for (const ok of ["urn:ads:event:session.created:1", "urn:ads:contract:harness-adapter:1", "urn:ads:common:ulid:12"]) {
    assert.ok(URN_RE.test(ok), ok);
  }
  for (const bad of ["urn:ads:event:session.created", "urn:ads:event:session.created:0", "urn:ads:event:Session:1", "urn:other:event:x:1", "urn:ads:nada:x:1", "urn:ads:event:x:1:2"]) {
    assert.ok(!URN_RE.test(bad), bad);
  }
  assert.equal(idToKey("urn:ads:event:session.created:1"), "event.session.created.v1");
});

test("Schema sem $id reprova", () => {
  const r = run(({ put }) => {
    const s = baseSchema();
    delete s.$id;
    put("schemas/record.thing.v1.schema.json", s);
  });
  assertFails(r, "$id ausente");
});

test("$id fora do padrão da URN reprova", () => {
  const r = run(({ put }) => put("schemas/record.thing.v1.schema.json", baseSchema({ $id: "urn:ads:Record:thing:1" })));
  assertFails(r, "fora do padrão");
});

test("$id repetido reprova", () => {
  const r = run(({ put }) => {
    put("schemas/record.copy.v1.schema.json", baseSchema());
    put("README.md", "`urn:ads:record:thing:1`");
  });
  assertFails(r, "$id repetido");
});

test("nome de arquivo diferente do $id reprova", () => {
  const r = run(({ put, rm }) => {
    rm("schemas/record.thing.v1.schema.json");
    put("schemas/record.other.v1.schema.json", baseSchema());
  });
  assertFails(r, "o nome do arquivo deveria ser record.thing.v1.schema.json");
});

test("$ref que não resolve reprova", () => {
  const r = run(({ put }) =>
    put("schemas/record.thing.v1.schema.json", baseSchema({ properties: { n: { $ref: "urn:ads:common:missing:1" } } })),
  );
  assert.ok(r.errors.length > 0);
});

test("palavra-chave desconhecida reprova (modo estrito)", () => {
  const r = run(({ put }) => put("schemas/record.thing.v1.schema.json", baseSchema({ minimumm: 3 })));
  assertFails(r, "unknown keyword");
});

test("objeto sem additionalProperties explícito reprova", () => {
  const s = baseSchema();
  delete s.additionalProperties;
  const r = run(({ put }) => put("schemas/record.thing.v1.schema.json", s));
  assertFails(r, "sem \"additionalProperties\"");
});

test("lint aceita ramo de extensão de allOf com $ref irmão", () => {
  const s = { allOf: [{ $ref: "urn:ads:event:envelope:1" }, { type: "object", properties: { a: { type: "string" } } }] };
  assert.deepEqual(lintClosedObjects(s), []);
});

test("dourado válido que falha reprova", () => {
  const r = run(({ put }) => put("golden/record.thing.v1/valid/02.json", { n: -5 }));
  assertFails(r, "valid/02.json: deveria passar");
});

test("dourado inválido que passa reprova", () => {
  const r = run(({ put }) => put("golden/record.thing.v1/invalid/01-negative.json", { n: 5 }));
  assertFails(r, "invalid/01-negative.json: deveria falhar e passou");
});

test("dourado inválido que falha por outro motivo reprova", () => {
  const r = run(({ put }) => put("golden/record.thing.v1/invalid/01-negative.json", { n: "texto" }));
  assertFails(r, "falhou por outro motivo");
});

test("inválido sem index.json reprova", () => {
  const r = run(({ rm }) => rm("golden/record.thing.v1/invalid/index.json"));
  assertFails(r, "index.json: faltando");
});

test("inválido fora do index.json reprova", () => {
  const r = run(({ put }) => put("golden/record.thing.v1/invalid/04-new.json", { n: -9 }));
  assertFails(r, "04-new.json: sem entrada no index.json");
});

test("entrada do index.json sem arquivo reprova", () => {
  const r = run(({ rm }) => rm("golden/record.thing.v1/invalid/03-extra.json"));
  assertFails(r, 'entrada "03-extra.json" sem arquivo');
});

test("menos dourados que o mínimo reprova", () => {
  const r = run(({ rm }) => {
    rm("golden/record.thing.v1/valid/02.json");
    rm("golden/record.thing.v1/invalid/03-extra.json");
    rm("golden/record.thing.v1/invalid/02-text.json");
  });
  assertFails(r, "valid: 1 arquivo(s)");
  assertFails(r, "invalid: 1 arquivo(s)");
});

test("pasta de dourados sem Schema reprova", () => {
  const r = run(({ put }) => put("golden/record.ghost.v1/valid/01.json", {}));
  assertFails(r, "pasta sem Schema correspondente");
});

test("exemplo do Schema diferente do primeiro dourado reprova", () => {
  const r = run(({ put }) => put("schemas/record.thing.v1.schema.json", baseSchema({ examples: [{ n: 99 }] })));
  assertFails(r, "examples[0] difere");
});

test("README que não cita o $id reprova", () => {
  const r = run(({ put }) => put("README.md", "vazio"));
  assertFails(r, "README.md");
});

test("JSON malformado reprova", () => {
  const r = run(({ put }) => put("golden/record.thing.v1/valid/02.json", "{ não é json"));
  assertFails(r, "JSON inválido");
});

test("lint não cobra additionalProperties dentro de if/then/else", () => {
  const s = {
    type: "object",
    additionalProperties: false,
    properties: { a: { type: "string" } },
    if: { properties: { a: { const: "x" } } },
    then: { properties: { a: { type: "string" } } },
  };
  assert.deepEqual(lintClosedObjects(s), []);
});

test("lint cobra additionalProperties em objeto aninhado", () => {
  const s = { type: "object", additionalProperties: false, properties: { inner: { type: "object", properties: { b: { type: "integer" } } } } };
  assert.equal(lintClosedObjects(s).length, 1);
});
