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

// Regras que valem para os dourados válidos da pasta `key` (`<tipo>.<nome>.v<versão>`).
export function crossRuleErrors(key, data) {
  if (key.startsWith("event.")) {
    const problem = streamBindingError(data);
    return problem ? [problem] : [];
  }
  if (/^record\.action-plan\.v\d+$/.test(key)) return actionPlanErrors(data);
  if (key === "record.queue-task.v1") return queueTaskErrors(data);
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
