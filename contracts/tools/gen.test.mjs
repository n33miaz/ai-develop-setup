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

function addIntegerCatalog(put, secondValue = -8) {
  put("contract.sample-codes.v1.schema.json", {
    $schema: D,
    $id: "urn:ads:contract:sample-codes:1",
    title: "Códigos de exemplo",
    description: "Catálogo de teste.",
    anyOf: [{ $ref: "#/$defs/first_code" }, { $ref: "#/$defs/second_code" }],
    $defs: {
      first_code: { type: "integer", const: -7, description: "Primeiro código." },
      second_code: { type: "integer", const: secondValue },
    },
    examples: [-7],
  });
}

test("generates constants for an integer catalog with a different URN", async () => {
  const { root, put } = tree();
  addIntegerCatalog(put);
  const files = await generateAll(root);
  assert.equal(files.get("contract.sample-codes.v1.constants.ts"), [
    "/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.sample-codes.v1.schema.json. Não edite à mão. */",
    "",
    "/** Primeiro código. */",
    "export const FIRST_CODE = -7",
    "",
    "export const SECOND_CODE = -8",
    "",
  ].join("\n"));
});

test("rejects an integer catalog without a safe integer constant", async () => {
  const { root, put } = tree();
  addIntegerCatalog(put, Number.MAX_SAFE_INTEGER + 1);
  await assert.rejects(generateAll(root), /second_code.*const inteiro seguro/);
});

test("reports missing, stale, and extra constants files", async () => {
  const { root, put } = tree();
  addIntegerCatalog(put);
  const files = await generateAll(root);
  const name = "contract.sample-codes.v1.constants.ts";
  const withoutConstants = new Map(files);
  withoutConstants.delete(name);

  writeAll(root, withoutConstants);
  assert.deepEqual(diffAll(root, files), ["faltando: generated/ts/" + name]);

  writeAll(root, files);
  assert.deepEqual(diffAll(root, files), []);
  assert.deepEqual(diffAll(root, withoutConstants), ["sobrando: generated/ts/" + name]);

  fs.appendFileSync(path.join(root, "generated", "ts", name), "// edição à mão\n");
  assert.deepEqual(diffAll(root, files), ["desatualizado: generated/ts/" + name]);
});

test("preserves the local API error code assigned to each name", async () => {
  const { root, put } = tree();
  const schema = JSON.parse(fs.readFileSync(new URL(
    "../schemas/contract.local-api.error-code.v1.schema.json",
    import.meta.url,
  ), "utf8"));
  put("contract.local-api.error-code.v1.schema.json", schema);

  const files = await generateAll(root);
  const constants = files.get("contract.local-api.error-code.v1.constants.ts");
  const entries = [...constants.matchAll(/^export const (\w+) = (-?\d+)$/gm)];
  assert.deepEqual(Object.fromEntries(entries.map(([, name, value]) => [name, Number(value)])), {
    METHOD_NOT_FOUND: -32601,
    INVALID_PARAMS: -32602,
    INTERNAL_ERROR: -32603,
    CAPACITY_EXCEEDED: -32003,
    SUBSCRIPTION_REPLAY_UNAVAILABLE: -32004,
    REMOTE_ORIGIN_REFUSED: -32010,
    HARNESS_NOT_CONFIGURED: -32011,
    CONFLICT: -32012,
    SESSION_CLOSE_PENDING: -32013,
    SESSION_SETUP_FAILED: -32014,
    QUEUE_NOT_CONFIGURED: -32015,
    QUEUE_ALREADY_ENABLED: -32016,
  });
});
