// Regras de contrato que o JSON Schema não exprime (dependem de comparar dois campos ou de recalcular um valor).
// O validador as aplica a todo dourado válido; o README as descreve.
import { createHash } from "node:crypto";

// Família do stream -> campo de `data` que nomeia a entidade do stream. Família fora desta tabela não tem regra.
export const STREAM_ID_FIELD = {
  session: "session_id",
  task: "task_id",
  approval: "approval_id",
  action: "action_id",
  review: "review_id",
  routing: "task_id",
  queue: "queue_id",
  handoff: "handoff_id",
  answer: "approval_id",
  effects: "action_id",
};

// Devolve o texto do problema, ou null quando o stream nomeia a mesma entidade que `data`.
export function streamBindingError(event) {
  if (event === null || typeof event !== "object" || typeof event.stream !== "string") return null;
  const slash = event.stream.indexOf("/");
  const field = STREAM_ID_FIELD[event.stream.slice(0, slash)];
  if (slash < 0 || !field) return null;
  const expected = `${event.stream.slice(0, slash)}/${event.data?.[field]}`;
  if (event.stream === expected) return null;
  return `stream "${event.stream}" não é o da entidade em data.${field} (esperado "${expected}")`;
}

const WINDOWS_ROOT = /^[A-Z]:\\$/;

function isWindows(path) {
  return /^[A-Z]:\\/.test(path);
}

// O texto de um alvo: o caminho, ou o caminho mais o curinga no último componente.
export function targetText(target) {
  if (target.pattern === null) return target.path;
  if (target.path === "/" || WINDOWS_ROOT.test(target.path)) return target.path + target.pattern;
  return target.path + (isWindows(target.path) ? "\\" : "/") + target.pattern;
}

// Ordem canônica dos alvos: estilo (Windows antes de POSIX), unidade, componentes um a um (bytes UTF-8;
// o caminho mais curto vem antes do que o estende) e, por fim, o curinga (sem curinga antes de com curinga).
function targetKey(target) {
  const windows = isWindows(target.path);
  const rest = windows ? target.path.slice(3) : target.path.slice(1);
  const components = rest === "" ? [] : rest.split(windows ? "\\" : "/");
  return { style: windows ? 0 : 1, drive: windows ? target.path[0] : "", components, pattern: target.pattern };
}

function compareText(a, b) {
  return Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));
}

export function compareTargets(a, b) {
  const x = targetKey(a);
  const y = targetKey(b);
  if (x.style !== y.style) return x.style - y.style;
  if (x.drive !== y.drive) return compareText(x.drive, y.drive);
  for (let i = 0; i < Math.min(x.components.length, y.components.length); i++) {
    const c = compareText(x.components[i], y.components[i]);
    if (c !== 0) return c;
  }
  if (x.components.length !== y.components.length) return x.components.length - y.components.length;
  if (x.pattern === y.pattern) return 0;
  if (x.pattern === null) return -1;
  if (y.pattern === null) return 1;
  return compareText(x.pattern, y.pattern);
}

// A serialização canônica de um registro action-plan (README, "Plano de ação e plan_hash").
export function canonicalPlan(plan) {
  const field = (text) => `${Buffer.byteLength(text, "utf8")}:${text}`;
  const lines = ["ads.action-plan/1", `op:${plan.quarantined ? "fs.move" : plan.operation}`, `work_root:${field(plan.work_root)}`];
  lines.push(plan.destination === null ? "destination:-" : `destination:${field(plan.destination)}`);
  lines.push(plan.estimate === null ? "estimate:unknown" : `estimate:${plan.estimate.items}:${plan.estimate.bytes}`);
  lines.push(`targets:${plan.targets.length}`);
  for (const target of plan.targets) lines.push(`target:${target.repo_root ? 1 : 0}:${field(targetText(target))}`);
  return lines.join("\n") + "\n";
}

export function planHash(plan) {
  return `sha256:${createHash("sha256").update(canonicalPlan(plan), "utf8").digest("hex")}`;
}

// Devolve os problemas de um registro action-plan que já passou no Schema: alvos fora da ordem canônica ou
// repetidos, e plan_hash que não é o da serialização canônica.
export function actionPlanErrors(plan) {
  const errors = [];
  for (let i = 1; i < plan.targets.length; i++) {
    if (compareTargets(plan.targets[i - 1], plan.targets[i]) >= 0) {
      errors.push(`targets[${i}] está fora da ordem canônica ou repete o anterior`);
    }
  }
  const expected = planHash(plan);
  if (plan.plan_hash !== expected) errors.push(`plan_hash não é o da serialização canônica (esperado ${expected})`);
  return errors;
}

// A forma canônica de um valor JSON sem números fracionários: compacta, sem espaços, com as chaves de cada
// objeto em ordem de bytes UTF-8 (README, "Efeitos concretos e effects_hash").
export function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const keys = Object.keys(value).sort(compareText);
    return `{${keys.map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

export function effectsHash(effects) {
  return `sha256:${createHash("sha256").update(canonicalJson(effects), "utf8").digest("hex")}`;
}

// Um registro de efeitos e o hash e a ação que o acompanham.
export function effectsErrors(effects, hash, actionId, planHash) {
  const errors = [];
  const expected = effectsHash(effects);
  if (hash !== expected) errors.push(`effects_hash não é o da forma canônica (esperado ${expected})`);
  if (effects.action_id !== actionId) errors.push("o registro de efeitos é de outra ação");
  if (effects.plan_hash !== planHash) errors.push("o registro de efeitos é de outro plano");
  return errors;
}

// Regras que valem para os dourados válidos da pasta `key` (`<tipo>.<nome>.v<versão>`).
export function crossRuleErrors(key, data) {
  if (key.startsWith("event.")) {
    const problem = streamBindingError(data);
    const errors = problem ? [problem] : [];
    if (/^event\.effects\.recorded\.v\d+$/.test(key)) {
      errors.push(...effectsErrors(data.data.effects, data.data.effects_hash, data.data.action_id, data.data.plan_hash));
    }
    return errors;
  }
  if (/^command\.approval\.list\.response\.v([2-9]|\d{2,})$/.test(key)) {
    return data.result.approvals.flatMap((item, at) =>
      item.effects === null
        ? []
        : effectsErrors(item.effects, item.effects_hash, item.action_id, item.plan_hash).map((e) => `approvals[${at}]: ${e}`),
    );
  }
  if (/^record\.action-plan\.v\d+$/.test(key)) return actionPlanErrors(data);
  if (/^record\.queue-task\.v\d+$/.test(key)) return queueTaskErrors(data);
  return [];
}

export function queueTaskErrors(task) {
  const errors = [];
  if (task.attempts > task.attempt_limit) errors.push("attempts excede attempt_limit");
  if (sortableTimestamp(task.updated_at) < sortableTimestamp(task.created_at)) {
    errors.push("updated_at antecede created_at");
  }
  return errors;
}

function sortableTimestamp(value) {
  const [seconds, fraction = ""] = value.slice(0, -1).split(".");
  return seconds + fraction.padEnd(6, "0");
}
