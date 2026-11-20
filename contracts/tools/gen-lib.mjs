// Gera tipos e constantes TypeScript a partir dos Schemas em generated/ts.
import fs from "node:fs";
import path from "node:path";
import { compile } from "json-schema-to-typescript";
import { idToKey, loadSchemas } from "./lib.mjs";

// urn:ads:event:session.created:1 -> EventSessionCreatedV1 (único e estável entre versões)
export function tsName(id) {
  const [, , tipo, nome, versao] = id.split(":");
  const words = [tipo, ...nome.split(/[._-]/), `v${versao}`];
  return words.map((w) => w[0].toUpperCase() + w.slice(1)).join("");
}

function integerConstants(schema) {
  if (!Array.isArray(schema.anyOf) || schema.anyOf.length === 0) return [];

  const refPrefix = "#/$defs/";
  const names = schema.anyOf.map((branch) => {
    const ref = branch?.$ref;
    return Object.keys(branch ?? {}).length === 1 && typeof ref === "string" && ref.startsWith(refPrefix)
      ? ref.slice(refPrefix.length)
      : null;
  });
  if (!names.every((name) => name !== null && schema.$defs?.[name]?.type === "integer")) return [];

  return names.map((name) => {
    const { const: value, description } = schema.$defs[name];
    if (!/^[a-z][a-z0-9_]*$/.test(name) || !Number.isSafeInteger(value)) {
      throw new Error(`${schema.$id}: $defs/${name} precisa de um nome minúsculo e um const inteiro seguro`);
    }
    const doc = description ? `/** ${description.replaceAll("*/", "* /")} */\n` : "";
    return `${doc}export const ${name.toUpperCase()} = ${value}`;
  });
}

export async function generateAll(root) {
  const errors = [];
  const loaded = loadSchemas(root, errors);
  if (errors.length) throw new Error(errors.join("\n"));

  // O título em português vira texto de documentação; o nome do tipo sai do $id.
  const byId = new Map();
  for (const { file, schema } of loaded) {
    const clone = structuredClone(schema);
    clone.description = `${schema.title}. ${schema.description}`;
    clone.title = tsName(schema.$id);
    delete clone.examples;
    byId.set(schema.$id, { file, schema: clone });
  }
  const resolver = {
    order: 1,
    canRead: /^urn:ads:/,
    read: (file) => {
      const hit = byId.get(file.url);
      if (!hit) throw new Error(`$ref sem Schema correspondente: ${file.url}`);
      return JSON.stringify(hit.schema);
    },
  };

  const out = new Map();
  for (const [id, { file, schema }] of byId) {
    const banner = `/* Gerado por tools/gen-ts.mjs a partir de schemas/${file}. Não edite à mão. */`;
    const text = await compile(schema, tsName(id), {
      cwd: root,
      $refOptions: { resolve: { ads: resolver } },
      bannerComment: banner,
      additionalProperties: false,
      format: true,
    });
    out.set(`${idToKey(id)}.d.ts`, text.replace(/\r\n/g, "\n"));
    const constants = integerConstants(schema);
    if (constants.length) {
      out.set(`${idToKey(id)}.constants.ts`, `${banner}\n\n${constants.join("\n\n")}\n`);
    }
  }
  return out;
}

export function outDir(root) {
  return path.join(root, "generated", "ts");
}

export function writeAll(root, files) {
  const dir = outDir(root);
  fs.mkdirSync(dir, { recursive: true });
  for (const f of fs.readdirSync(dir)) {
    if (f.endsWith(".d.ts") && !files.has(f)) fs.rmSync(path.join(dir, f));
  }
  for (const [name, text] of files) fs.writeFileSync(path.join(dir, name), text);
}

// Devolve a lista de diferenças entre o que seria gerado e o que está no disco.
export function diffAll(root, files) {
  const dir = outDir(root);
  const problems = [];
  const present = fs.existsSync(dir)
    ? fs.readdirSync(dir).filter((f) => f.endsWith(".d.ts") || f.endsWith(".constants.ts"))
    : [];
  for (const [name, text] of files) {
    const file = path.join(dir, name);
    if (!fs.existsSync(file)) problems.push(`faltando: generated/ts/${name}`);
    else if (fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n") !== text) problems.push(`desatualizado: generated/ts/${name}`);
  }
  for (const f of present) if (!files.has(f)) problems.push(`sobrando: generated/ts/${f}`);
  return problems;
}
