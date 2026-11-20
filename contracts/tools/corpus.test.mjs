// O corpus comum de caminhos absolutos (corpus/abspath.json) contra o Schema. O núcleo roda o mesmo arquivo pelo
// seu normalizador (crates/core/tests/gate_golden.rs): os dois têm de dar o veredito que o corpus declara.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { createAjv, loadSchemas } from "./lib.mjs";
import { planHash } from "./rules.mjs";
import { readAlphabet, titlePatterns } from "./title-alphabet.mjs";

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

// Os mesmos valores de fronteira que crates/core/tests/gate_golden.rs passa pelo leitor Rust do registro.
test("a estimativa do registro action-plan:2 vai de 0 a 2^53 - 1, e a :1 não tinha máximo", () => {
  const errors = [];
  const ajv = createAjv();
  for (const { schema } of loadSchemas(root, errors)) ajv.addSchema(schema);
  const v1 = ajv.getSchema("urn:ads:record:action-plan:1");
  const v2 = ajv.getSchema("urn:ads:record:action-plan:2");
  const base = JSON.parse(
    fs.readFileSync(path.join(root, "golden", "record.action-plan.v2", "valid", "06-estimate-at-the-limit.json"), "utf8"),
  );
  const withEstimate = (version, items, bytes) => {
    const record = { ...base, version, estimate: { items, bytes } };
    record.plan_hash = planHash(record);
    return record;
  };
  const MAX = 9007199254740991;
  assert.equal(MAX, Number.MAX_SAFE_INTEGER);
  for (const [items, bytes] of [[0, 0], [MAX, 0], [0, MAX], [MAX, MAX], [MAX - 1, 1]]) {
    assert.ok(v2(withEstimate(2, items, bytes)), `${items}:${bytes}`);
  }
  for (const [items, bytes] of [[MAX + 1, 0], [0, MAX + 1], [2 ** 63, 0], [18446744073709551615, 0], [18446744073709551616, 0], [-1, 0], [0.5, 0]]) {
    assert.ok(!v2(withEstimate(2, items, bytes)), `${items}:${bytes}`);
  }
  // A divergência que a :2 fecha: a :1 aceita o que nenhum leitor de 64 bits lê.
  assert.ok(v1(withEstimate(1, 18446744073709551616, 0)));
  assert.ok(v1(withEstimate(1, MAX, MAX)));
});

test("todo caminho dos dourados válidos de record.action-plan cabe na abspath:2", () => {
  const { v2 } = validators();
  const dir = path.join(root, "golden", "record.action-plan.v2", "valid");
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

// O alfabeto do título (corpus/title-chars.json) contra o Schema; crates/contracts/tests/title_corpus.rs confere o
// tipo Rust contra o mesmo arquivo. Os padrões são faixas explícitas do Unicode 16 montadas das classes do corpus, então
// os dois aceitam o mesmo conjunto em qualquer versão de Unicode do motor.
test("o Schema do título é montado das faixas do corpus", () => {
  const { pattern, not } = titlePatterns(readAlphabet().classes);
  const schema = JSON.parse(fs.readFileSync(path.join(root, "schemas", "record.queue-task.v1.schema.json"), "utf8"));
  assert.equal(schema.properties.title.pattern, pattern, "rode node tools/title-alphabet.mjs --write");
  assert.deepEqual(schema.properties.title.not, { pattern: not }, "rode node tools/title-alphabet.mjs --write");
});

test("título aceita exatamente o alfabeto do corpus em cada posição", () => {
  const alphabet = readAlphabet();
  const errors = [];
  const ajv = createAjv();
  for (const { schema } of loadSchemas(root, errors)) ajv.addSchema(schema);
  assert.deepEqual(errors, []);
  const validate = ajv.getSchema(`${alphabet.schema}#/properties/title`);
  const hex = (n) => n.toString(16).toUpperCase().padStart(4, "0");
  assert.ok(alphabet.probes.length >= 3, "o corpus testa várias posições");
  for (const { template, accepted } of alphabet.probes) {
    const ranges = [];
    for (let code = 0; code <= 0x10ffff; code += 1) {
      if (code >= 0xd800 && code <= 0xdfff) continue;
      if (!validate(template.replace("{c}", String.fromCodePoint(code)))) continue;
      const last = ranges.at(-1);
      if (last && last[1] + 1 === code) last[1] = code;
      else ranges.push([code, code]);
    }
    assert.deepEqual(ranges.map(([a, b]) => [hex(a), hex(b)]), accepted, template);
  }
});

// Os dois Schemas repetem o apelido e a origem até a versão 2 extraí-los para `common`; até lá, não podem divergir.
test("o apelido e a origem da tarefa são os mesmos no registro e em queue.task_created", () => {
  const read = (name) => JSON.parse(fs.readFileSync(path.join(root, "schemas", name), "utf8"));
  const record = read("record.queue-task.v1.schema.json").properties;
  const created = read("event.queue.task_created.v1.schema.json").allOf[1].properties.data.properties;
  for (const field of ["workspace_alias", "origin"]) assert.deepEqual(created[field], record[field], field);
});
