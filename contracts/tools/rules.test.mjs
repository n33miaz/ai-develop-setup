import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createAjv, loadSchemas } from "./lib.mjs";
import { actionPlanErrors, canonicalPlan, compareTargets, crossRuleErrors, planHash, STREAM_ID_FIELD, streamBindingError, targetText } from "./rules.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function realValidator(id) {
  const ajv = createAjv();
  for (const { schema } of loadSchemas(root, [])) ajv.addSchema(schema);
  return ajv.getSchema(id);
}

function goldenValid(key) {
  const dir = path.join(root, "golden", key, "valid");
  return fs
    .readdirSync(dir)
    .sort()
    .map((f) => [f, JSON.parse(fs.readFileSync(path.join(dir, f), "utf8"))]);
}

const target = (p, pattern = null, repo_root = false) => ({ path: p, pattern, repo_root });

// O plano do teste `canonical_text_is_stable` do núcleo, com o hash calculado lá.
function vector() {
  return {
    operation: "fs.delete_recursive",
    work_root: "/work/proj",
    destination: null,
    targets: [target("/work/proj/a"), target("/work/proj/b")],
    estimate: { items: 3, bytes: 4096 },
    quarantined: false,
  };
}
const VECTOR_HASH = "sha256:c7b870d89ef435eb431fcdf8e99f82f2b10c9b05a25eb86eb2ed603fa366eb94";

test("stream: aceita o stream da entidade de data e recusa o de outra", () => {
  const event = { stream: "approval/appr-02", data: { approval_id: "appr-02" } };
  assert.equal(streamBindingError(event), null);
  // O caso do dourado que motivou a regra: stream de appr-01 com approval_id appr-02.
  assert.match(streamBindingError({ ...event, stream: "approval/appr-01" }), /esperado "approval\/appr-02"/);
  assert.match(streamBindingError({ stream: "task/task-01", data: {} }), /data\.task_id/);
  assert.equal(streamBindingError({ stream: "routing/task-01", data: { task_id: "task-01" } }), null);
  const queued = { stream: "queue/queue-01", data: { queue_id: "queue-01", task_id: "task-01" } };
  assert.equal(streamBindingError(queued), null);
  assert.match(streamBindingError({ ...queued, stream: "queue/task-01" }), /data\.queue_id/);
});

test("stream: a passagem identifica a mesma entidade no stream e em data", () => {
  const [, event] = goldenValid("event.session.handoff.v2")[0];
  assert.equal(streamBindingError(event), null);
  assert.match(streamBindingError({ ...event, stream: "handoff/other" }), /data\.handoff_id/);
});

test("passagem: cada registro aponta para o aceite da própria sessão de origem", () => {
  const events = new Map(goldenValid("event.session.handoff.v2").map(([, event]) => [event.id, event]));
  for (const [, record] of goldenValid("record.session-handoff.v2")) {
    const event = events.get(record.consent.event_id);
    assert.ok(event);
    assert.equal(event.actor.kind, "device");
    assert.equal(event.data.status, "accepted");
    assert.equal(event.data.handoff_id, record.id);
    assert.equal(event.data.source_session_id, record.source_session_id);
  }
});

test("stream: família fora da tabela não tem regra", () => {
  assert.equal(streamBindingError({ stream: "host/host-01", data: { event_id: "x" } }), null);
  assert.equal(streamBindingError({ stream: "usage/task-01", data: {} }), null);
  assert.deepEqual(Object.keys(STREAM_ID_FIELD).sort(), ["action", "approval", "handoff", "queue", "review", "routing", "session", "task"]);
});

test("stream: a regra vale para os dourados de evento e só para eles", () => {
  const wrong = { stream: "session/sess-01", data: { session_id: "sess-02" } };
  assert.equal(crossRuleErrors("event.session.resized.v1", wrong).length, 1);
  assert.equal(crossRuleErrors("event.envelope.v1", wrong).length, 1);
  assert.deepEqual(crossRuleErrors("record.thing.v1", wrong), []);
});

test("plano: o texto canônico e o hash batem com o vetor do núcleo", () => {
  assert.equal(
    canonicalPlan(vector()),
    "ads.action-plan/1\nop:fs.delete_recursive\nwork_root:10:/work/proj\ndestination:-\nestimate:3:4096\ntargets:2\ntarget:0:12:/work/proj/a\ntarget:0:12:/work/proj/b\n",
  );
  assert.equal(planHash(vector()), VECTOR_HASH);
});

test("plano: quarentena entra como mover; estimativa ausente e raiz de repositório têm marca própria", () => {
  const plan = { ...vector(), operation: "fs.delete", destination: "/work/proj/.ads/quarantine/0011223344556677", estimate: null, quarantined: true };
  plan.targets = [target("/work/proj/a", null, true)];
  assert.equal(
    canonicalPlan(plan),
    "ads.action-plan/1\nop:fs.move\nwork_root:10:/work/proj\ndestination:43:/work/proj/.ads/quarantine/0011223344556677\nestimate:unknown\ntargets:1\ntarget:1:12:/work/proj/a\n",
  );
});

test("plano: o tamanho do caminho é contado em bytes UTF-8", () => {
  const plan = { ...vector(), work_root: "/work/ação" };
  assert.match(canonicalPlan(plan), /\nwork_root:12:\/work\/ação\n/);
});

test("plano: texto do alvo com curinga, nos dois estilos e na raiz", () => {
  assert.equal(targetText(target("C:\\work\\proj\\cache", "*.tmp")), "C:\\work\\proj\\cache\\*.tmp");
  assert.equal(targetText(target("/work/proj/cache", "*.tmp")), "/work/proj/cache/*.tmp");
  assert.equal(targetText(target("D:\\", "*")), "D:\\*");
  assert.equal(targetText(target("/", "*")), "/*");
  assert.equal(targetText(target("/work/proj")), "/work/proj");
});

test("plano: a ordem canônica compara componente a componente", () => {
  const ordered = [
    target("C:\\work\\a"),
    target("D:\\a"),
    target("/a"),
    target("/a", "*.log"),
    target("/a", "*.tmp"),
    target("/a/b"),
    target("/a-b"),
    target("/b"),
  ];
  for (let i = 1; i < ordered.length; i++) {
    assert.ok(compareTargets(ordered[i - 1], ordered[i]) < 0, `${targetText(ordered[i - 1])} antes de ${targetText(ordered[i])}`);
    assert.ok(compareTargets(ordered[i], ordered[i - 1]) > 0);
  }
  assert.equal(compareTargets(target("/a/b"), target("/a/b", null, true)), 0);
});

test("plano: hash de outro plano, alvo fora de ordem e alvo repetido reprovam", () => {
  const good = { ...vector(), plan_hash: VECTOR_HASH };
  assert.deepEqual(actionPlanErrors(good), []);

  const tampered = structuredClone(good);
  tampered.targets[1].path = "/work/proj/c";
  assert.match(actionPlanErrors(tampered).join("\n"), /plan_hash não é o da serialização canônica/);

  const swapped = structuredClone(good);
  swapped.targets.reverse();
  swapped.plan_hash = planHash(swapped);
  assert.match(actionPlanErrors(swapped).join("\n"), /fora da ordem canônica/);

  const repeated = structuredClone(good);
  repeated.targets[1] = { ...repeated.targets[0], repo_root: true };
  repeated.plan_hash = planHash(repeated);
  assert.match(actionPlanErrors(repeated).join("\n"), /repete o anterior/);
});

test("plano: todo dourado válido do registro tem o hash da serialização canônica", () => {
  const files = goldenValid("record.action-plan.v1");
  assert.ok(files.length >= 2);
  for (const [name, plan] of files) assert.deepEqual(actionPlanErrors(plan), [], name);
  assert.equal(files[0][1].plan_hash, VECTOR_HASH);
  assert.ok(files.some(([, plan]) => plan.quarantined), "falta um dourado com quarentena");
  assert.ok(files.some(([, plan]) => plan.targets.some((t) => t.pattern !== null)), "falta um dourado com curinga");
  // A versão 2 do registro: mesmas regras, mesmo vetor (o hash não cobre a versão).
  const v2 = goldenValid("record.action-plan.v2");
  assert.ok(v2.length >= 2);
  for (const [name, plan] of v2) assert.deepEqual(actionPlanErrors(plan), [], name);
  assert.equal(v2[0][1].plan_hash, VECTOR_HASH);
  assert.ok(v2.some(([, plan]) => plan.quarantined), "falta um dourado com quarentena na versão 2");
});

test("caminho absoluto: aceita só a forma normalizada, Windows e POSIX", () => {
  const validate = realValidator("urn:ads:common:abspath:1");
  for (const ok of ["C:\\", "/", "C:\\work\\proj", "/work/proj", "Z:\\a b\\c.d\\e", "/work/proj/.hidden", "/work/a~b", "C:\\work\\..hidden"]) {
    assert.ok(validate(ok), ok);
  }
  const refused = {
    relativo: ["build/cache", "build", ".\\build", "\\build", "C:build", "C:"],
    "componente ..": ["/work/../proj", "/..", "C:\\work\\..", "C:\\..\\work"],
    variável: ["/work/$HOME", "/work/${NAME}/a", "C:\\work\\%TEMP%", "$HOME/build"],
    til: ["~", "~/proj", "/work/~user", "C:\\work\\~user"],
    rede: ["\\\\server\\share", "//server/share", "\\\\?\\C:\\work", "\\\\.\\C:\\work"],
    "fora da forma normal": ["c:\\work", "C:/work", "C:\\work\\", "/work/", "/work//proj", "C:\\work\\\\proj", "/work/./proj", "C:\\work\\name.", "C:\\work\\name ", "C:\\work\\a:b"],
    curinga: ["/work/*", "C:\\work\\?.txt"],
    controle: ["/work/a\nb", "/work/a\u0000b"],
  };
  for (const [why, paths] of Object.entries(refused)) {
    for (const bad of paths) assert.ok(!validate(bad), `${why}: ${JSON.stringify(bad)}`);
  }
});

test("queue task cross rules compare attempt ceilings and UTC fractions", () => {
  const key = "record.queue-task.v1";
  const base = {
    attempts: 2,
    attempt_limit: 2,
    created_at: "2030-01-02T03:04:05Z",
    updated_at: "2030-01-02T03:04:05Z",
  };
  for (const [changes, errors] of [
    [{}, []],
    [{ attempts: 3 }, ["attempts excede attempt_limit"]],
    [{ updated_at: "2030-01-02T03:04:04.999999Z" }, ["updated_at antecede created_at"]],
    [{ updated_at: "2030-01-02T03:04:05.000001Z" }, []],
    [{ created_at: "2030-01-02T03:04:05.1Z", updated_at: "2030-01-02T03:04:05.100000Z" }, []],
  ]) {
    assert.deepEqual(crossRuleErrors(key, { ...base, ...changes }), errors);
  }
});
