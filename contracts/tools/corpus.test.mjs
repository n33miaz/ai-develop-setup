// O corpus comum de caminhos absolutos (corpus/abspath.json) contra o Schema. O núcleo roda o mesmo arquivo pelo
// seu normalizador (crates/core/tests/gate_golden.rs): os dois têm de dar o veredito que o corpus declara.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { createAjv, loadSchemas } from "./lib.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const corpus = JSON.parse(fs.readFileSync(path.join(root, "corpus", "abspath.json"), "utf8"));

function validators() {
  const errors = [];
  const ajv = createAjv();
  for (const { schema } of loadSchemas(root, errors)) ajv.addSchema(schema);
  assert.deepEqual(errors, []);
  return { v1: ajv.getSchema("urn:ads:common:abspath:1"), v2: ajv.getSchema(corpus.schema) };
}

// Caminho -> o que o Schema responde quando diverge do núcleo de propósito.
function knownDifferences() {
  const known = new Map();
  for (const entry of corpus.known_differences) {
    assert.equal(typeof entry.path, "string");
    assert.equal(typeof entry.schema, "boolean");
    assert.equal(typeof entry.core, "boolean");
    assert.notEqual(entry.schema, entry.core, `${JSON.stringify(entry.path)}: não é uma diferença`);
    assert.ok(typeof entry.reason === "string" && entry.reason.trim(), "toda diferença diz o motivo");
    known.set(entry.path, entry.schema);
  }
  return known;
}

test("o corpus de caminhos é grande o bastante e não se repete", () => {
  assert.equal(corpus.schema, "urn:ads:common:abspath:2");
  const paths = corpus.cases.map((c) => c.path);
  assert.equal(new Set(paths).size, paths.length);
  assert.ok(corpus.cases.filter((c) => c.valid).length >= 50);
  assert.ok(corpus.cases.filter((c) => !c.valid).length >= 100);
  for (const [p] of knownDifferences()) assert.ok(paths.includes(p), `${JSON.stringify(p)} não está nos casos`);
});

test("o Schema abspath:2 dá a todo caminho do corpus o veredito declarado", () => {
  const { v2 } = validators();
  const known = knownDifferences();
  const wrong = [];
  for (const { path: p, valid } of corpus.cases) {
    const expected = known.has(p) ? known.get(p) : valid;
    if (v2(p) !== expected) wrong.push(`${JSON.stringify(p)}: esperado ${expected}`);
  }
  assert.deepEqual(wrong, []);
});

test("a versão 2 só aperta a versão 1, e só em nome de dispositivo e nome curto", () => {
  const { v1, v2 } = validators();
  const tightened = [];
  for (const { path: p } of corpus.cases) {
    if (v2(p)) assert.ok(v1(p), `${JSON.stringify(p)}: a :2 aceita o que a :1 recusa`);
    else if (v1(p)) tightened.push(p);
  }
  assert.ok(tightened.length >= 40, `só ${tightened.length} casos separam as duas versões`);
  for (const p of tightened) assert.match(p, /^[A-Z]:\\/, `${JSON.stringify(p)}: só o estilo Windows foi apertado`);
});

test("todo caminho dos dourados válidos de record.action-plan cabe na abspath:2", () => {
  const { v2 } = validators();
  const dir = path.join(root, "golden", "record.action-plan.v1", "valid");
  let checked = 0;
  for (const file of fs.readdirSync(dir)) {
    const record = JSON.parse(fs.readFileSync(path.join(dir, file), "utf8"));
    const paths = [record.work_root, ...record.targets.map((t) => t.path)];
    if (record.destination !== null) paths.push(record.destination);
    for (const p of paths) {
      assert.ok(v2(p), `${file}: ${JSON.stringify(p)}`);
      checked++;
    }
  }
  assert.ok(checked >= 10);
});
