import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { diffAll, generateAll, tsName, writeAll } from "./gen-lib.mjs";
import { freshDir } from "./test-tmp.mjs";

const D = "https://json-schema.org/draft/2020-12/schema";

function tree() {
  const root = freshDir("gen");
  fs.mkdirSync(path.join(root, "schemas"));
  const put = (name, s) => fs.writeFileSync(path.join(root, "schemas", name), JSON.stringify(s));
  put("common.id.v1.schema.json", { $schema: D, $id: "urn:ads:common:id:1", title: "Identificador", description: "Id opaco.", type: "string", pattern: "^[a-z]+$", examples: ["abc"] });
  put("record.thing.v1.schema.json", {
    $schema: D,
    $id: "urn:ads:record:thing:1",
    title: "Coisa",
    description: "Uma coisa.",
    type: "object",
    additionalProperties: false,
    required: ["id"],
    properties: { id: { $ref: "urn:ads:common:id:1" }, note: { type: "string" } },
    examples: [{ id: "abc" }],
  });
  return { root, put };
}

test("tsName vem do $id e inclui a versão", () => {
  assert.equal(tsName("urn:ads:event:session.created:1"), "EventSessionCreatedV1");
  assert.equal(tsName("urn:ads:contract:harness-adapter:2"), "ContractHarnessAdapterV2");
});

test("gera um .d.ts por Schema, resolvendo $ref por URN", async () => {
  const { root } = tree();
  const files = await generateAll(root);
  assert.deepEqual([...files.keys()].sort(), ["common.id.v1.d.ts", "record.thing.v1.d.ts"]);
  const thing = files.get("record.thing.v1.d.ts");
  assert.match(thing, /export interface RecordThingV1/);
  assert.match(thing, /id: CommonIdV1/);
  assert.match(thing, /Gerado por tools\/gen-ts\.mjs/);
});

test("saída em dia passa; tipo desatualizado, faltando ou sobrando reprova", async () => {
  const { root, put } = tree();
  writeAll(root, await generateAll(root));
  assert.deepEqual(diffAll(root, await generateAll(root)), []);

  const f = path.join(root, "generated", "ts", "record.thing.v1.d.ts");
  fs.appendFileSync(f, "// edição à mão\n");
  assert.deepEqual(diffAll(root, await generateAll(root)), ["desatualizado: generated/ts/record.thing.v1.d.ts"]);

  writeAll(root, await generateAll(root));
  fs.unlinkSync(path.join(root, "generated", "ts", "common.id.v1.d.ts"));
  assert.deepEqual(diffAll(root, await generateAll(root)), ["faltando: generated/ts/common.id.v1.d.ts"]);

  writeAll(root, await generateAll(root));
  fs.writeFileSync(path.join(root, "generated", "ts", "record.old.v1.d.ts"), "x");
  assert.deepEqual(diffAll(root, await generateAll(root)), ["sobrando: generated/ts/record.old.v1.d.ts"]);

  // mudar o Schema sem regerar também reprova
  writeAll(root, await generateAll(root));
  put("record.thing.v1.schema.json", {
    $schema: D, $id: "urn:ads:record:thing:1", title: "Coisa", description: "Uma coisa.", type: "object", additionalProperties: false,
    required: ["id", "note"], properties: { id: { $ref: "urn:ads:common:id:1" }, note: { type: "string" } }, examples: [{ id: "abc", note: "x" }],
  });
  assert.deepEqual(diffAll(root, await generateAll(root)), ["desatualizado: generated/ts/record.thing.v1.d.ts"]);
});

test("a geração é determinística", async () => {
  const { root } = tree();
  const a = await generateAll(root);
  const b = await generateAll(root);
  assert.deepEqual([...a], [...b]);
});

test("$ref para Schema inexistente falha a geração", async () => {
  const { root, put } = tree();
  put("record.bad.v1.schema.json", { $schema: D, $id: "urn:ads:record:bad:1", title: "Ruim", description: "x", type: "object", additionalProperties: false, properties: { a: { $ref: "urn:ads:common:nope:1" } }, examples: [{}] });
  await assert.rejects(generateAll(root));
});
