// Núcleo do validador dos contratos. A CLI (validate.mjs) e os testes chamam as mesmas funções.
import fs from "node:fs";
import path from "node:path";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

export const SCHEMA_DIALECT = "https://json-schema.org/draft/2020-12/schema";
export const URN_RE =
  /^urn:ads:(common|event|command|record|contract|relay):[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*:[1-9][0-9]*$/;
export const MIN_VALID = 2;
export const MIN_INVALID = 3;

// urn:ads:event:session.created:1 -> event.session.created.v1
export function idToKey(id) {
  const [, , tipo, nome, versao] = id.split(":");
  return `${tipo}.${nome}.v${versao}`;
}

function readJson(file, errors) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (e) {
    errors.push(`${file}: JSON inválido (${e.message})`);
    return undefined;
  }
}

function listJson(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".json"))
    .sort();
}

export function loadSchemas(root, errors) {
  const dir = path.join(root, "schemas");
  const out = [];
  for (const f of fs.readdirSync(dir).sort()) {
    if (!f.endsWith(".schema.json")) {
      errors.push(`schemas/${f}: nome fora do padrão <tipo>.<nome>.v<versão>.schema.json`);
      continue;
    }
    const schema = readJson(path.join(dir, f), errors);
    if (schema !== undefined) out.push({ file: f, schema });
  }
  return out;
}

// Modo estrito (palavra desconhecida e tipo faltando reprovam). strictRequired fica fora: ramos de allOf e de
// if/then citam em `required` campos que as `properties` do ramo irmão definem.
export function createAjv() {
  const ajv = new Ajv2020({ strict: true, strictRequired: false, allowUnionTypes: true, allErrors: true });
  addFormats(ajv);
  return ajv;
}

const SCHEMA_MAP_KEYS = ["properties", "$defs", "patternProperties", "dependentSchemas"];
const SCHEMA_LIST_KEYS = ["allOf", "anyOf", "oneOf", "prefixItems"];
const SCHEMA_KEYS = ["items", "additionalProperties", "if", "then", "else", "not", "contains", "propertyNames"];

// Todo objeto com `properties` declara `additionalProperties`, fechado ou aberto de propósito.
// Exceções: ramo de extensão de um allOf que tem um `$ref` irmão (o envelope já fecha o objeto) e tudo o que fica
// dentro de if/then/else/not, que só restringe valores e não descreve um objeto.
export function lintClosedObjects(schema, where = "#", exempt = false, errors = []) {
  if (schema === null || typeof schema !== "object" || Array.isArray(schema)) return errors;
  if (schema.properties && typeof schema.properties === "object" && !exempt && !("additionalProperties" in schema)) {
    errors.push(`${where}: objeto com "properties" sem "additionalProperties" explícito`);
  }
  const inherit = exempt === "conditional";
  for (const k of SCHEMA_MAP_KEYS) {
    if (schema[k] && typeof schema[k] === "object") {
      for (const [name, sub] of Object.entries(schema[k])) lintClosedObjects(sub, `${where}/${k}/${name}`, inherit ? "conditional" : false, errors);
    }
  }
  for (const k of SCHEMA_LIST_KEYS) {
    if (Array.isArray(schema[k])) {
      const hasRef = k === "allOf" && schema[k].some((s) => s && typeof s === "object" && "$ref" in s);
      schema[k].forEach((sub, i) => {
        const ext = hasRef && !(sub && typeof sub === "object" && "$ref" in sub);
        lintClosedObjects(sub, `${where}/${k}/${i}`, inherit ? "conditional" : ext, errors);
      });
    }
  }
  for (const k of SCHEMA_KEYS) {
    const cond = inherit || ["if", "then", "else", "not"].includes(k);
    lintClosedObjects(schema[k], `${where}/${k}`, cond ? "conditional" : false, errors);
  }
  return errors;
}

function matchesExpectation(errs, exp) {
  return errs.some((e) => {
    if (e.keyword !== exp.keyword) return false;
    if (e.instancePath !== exp.instancePath) return false;
    if (exp.params) {
      for (const [k, v] of Object.entries(exp.params)) {
        if (JSON.stringify(e.params?.[k]) !== JSON.stringify(v)) return false;
      }
    }
    return true;
  });
}

function describeErrors(errs) {
  return (errs ?? []).map((e) => `${e.keyword}@${e.instancePath || "/"}`).join(", ");
}

function checkGolden(root, key, validate, errors, stats) {
  const base = path.join(root, "golden", key);
  const validFiles = listJson(path.join(base, "valid"));
  const invalidDir = path.join(base, "invalid");
  const invalidFiles = listJson(invalidDir).filter((f) => f !== "index.json");
  if (validFiles.length < MIN_VALID) errors.push(`golden/${key}/valid: ${validFiles.length} arquivo(s), o mínimo é ${MIN_VALID}`);
  if (invalidFiles.length < MIN_INVALID) errors.push(`golden/${key}/invalid: ${invalidFiles.length} arquivo(s), o mínimo é ${MIN_INVALID}`);

  for (const f of validFiles) {
    const data = readJson(path.join(base, "valid", f), errors);
    if (data === undefined) continue;
    stats.valid++;
    if (!validate(data)) errors.push(`golden/${key}/valid/${f}: deveria passar e falhou (${describeErrors(validate.errors)})`);
  }

  const indexFile = path.join(invalidDir, "index.json");
  const index = fs.existsSync(indexFile) ? readJson(indexFile, errors) : undefined;
  if (index === undefined) {
    if (!fs.existsSync(indexFile)) errors.push(`golden/${key}/invalid/index.json: faltando (declara a regra violada por cada inválido)`);
  } else {
    for (const f of Object.keys(index)) {
      if (!invalidFiles.includes(f)) errors.push(`golden/${key}/invalid/index.json: entrada "${f}" sem arquivo`);
    }
  }
  for (const f of invalidFiles) {
    const data = readJson(path.join(invalidDir, f), errors);
    if (data === undefined) continue;
    stats.invalid++;
    const exp = index?.[f];
    if (index && !exp) {
      errors.push(`golden/${key}/invalid/${f}: sem entrada no index.json`);
      continue;
    }
    if (!exp) continue;
    if (!exp.rule || !exp.keyword || typeof exp.instancePath !== "string") {
      errors.push(`golden/${key}/invalid/${f}: entrada do index.json precisa de rule, keyword e instancePath`);
      continue;
    }
    if (validate(data)) {
      errors.push(`golden/${key}/invalid/${f}: deveria falhar e passou (regra declarada: ${exp.rule})`);
    } else if (!matchesExpectation(validate.errors, exp)) {
      errors.push(
        `golden/${key}/invalid/${f}: falhou por outro motivo; esperado ${exp.keyword}@${exp.instancePath || "/"}, obtido ${describeErrors(validate.errors)}`,
      );
    }
  }
}

export function checkContracts(root) {
  const errors = [];
  const stats = { schemas: 0, valid: 0, invalid: 0 };
  const loaded = loadSchemas(root, errors);
  stats.schemas = loaded.length;

  const seen = new Map();
  const usable = [];
  for (const { file, schema } of loaded) {
    const at = `schemas/${file}`;
    const id = schema.$id;
    if (typeof id !== "string" || !URN_RE.test(id)) {
      errors.push(`${at}: $id ausente ou fora do padrão urn:ads:<tipo>:<nome>:<versão> (${id})`);
      continue;
    }
    if (seen.has(id)) {
      errors.push(`${at}: $id repetido, já usado por ${seen.get(id)} (${id})`);
      continue;
    }
    seen.set(id, file);
    if (file !== `${idToKey(id)}.schema.json`) errors.push(`${at}: o nome do arquivo deveria ser ${idToKey(id)}.schema.json`);
    if (schema.$schema !== SCHEMA_DIALECT) errors.push(`${at}: $schema deve ser ${SCHEMA_DIALECT}`);
    if (typeof schema.title !== "string" || !schema.title.trim()) errors.push(`${at}: title ausente`);
    if (typeof schema.description !== "string" || !schema.description.trim()) errors.push(`${at}: description ausente`);
    if (!Array.isArray(schema.examples) || schema.examples.length < 1) errors.push(`${at}: examples ausente`);
    for (const e of lintClosedObjects(schema)) errors.push(`${at}${e.slice(1)}`);
    usable.push({ file, schema, key: idToKey(id) });
  }

  // Compila cada Schema contra o conjunto todo: pega meta-esquema inválido e $ref que não resolve.
  const ajv = createAjv();
  const added = [];
  for (const u of usable) {
    try {
      ajv.addSchema(u.schema);
      added.push(u);
    } catch (e) {
      errors.push(`schemas/${u.file}: Schema inválido (${e.message})`);
    }
  }
  const validators = new Map();
  for (const u of added) {
    try {
      validators.set(u.key, ajv.getSchema(u.schema.$id));
    } catch (e) {
      errors.push(`schemas/${u.file}: não compila (${e.message})`);
    }
  }

  const keys = new Set(usable.map((u) => u.key));
  const goldenDir = path.join(root, "golden");
  if (fs.existsSync(goldenDir)) {
    for (const d of fs.readdirSync(goldenDir)) {
      if (!keys.has(d)) errors.push(`golden/${d}: pasta sem Schema correspondente`);
    }
  }
  for (const u of usable) {
    const validate = validators.get(u.key);
    if (!validate) continue;
    checkGolden(root, u.key, validate, errors, stats);
    const first = listJson(path.join(root, "golden", u.key, "valid"))[0];
    if (first && Array.isArray(u.schema.examples)) {
      const data = readJson(path.join(root, "golden", u.key, "valid", first), []);
      if (JSON.stringify(data) !== JSON.stringify(u.schema.examples[0])) {
        errors.push(`schemas/${u.file}: examples[0] difere de golden/${u.key}/valid/${first}`);
      }
    }
  }

  const readme = path.join(root, "README.md");
  const readmeText = fs.existsSync(readme) ? fs.readFileSync(readme, "utf8") : "";
  for (const u of usable) {
    if (!readmeText.includes(`\`${u.schema.$id}\``)) errors.push(`README.md: a tabela não cita \`${u.schema.$id}\``);
  }

  return { errors, stats };
}
