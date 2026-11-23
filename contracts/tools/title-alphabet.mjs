// Monta os padrões do título da tarefa da fila a partir das faixas explícitas de corpus/title-chars.json (Unicode 16),
// para que o Ajv e o regex do Rust não dependam da tabela Unicode de cada motor. Só classes de caracteres, sem
// quantificador aninhado. `node tools/title-alphabet.mjs --write` regrava o tipo comum do título e as versões do registro
// que o repetem por extenso; o teste de corpus confere que eles continuam iguais ao que este arquivo monta.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const ALPHABET_FILE = path.join(root, "corpus", "title-chars.json");
// Cada Schema com o título por extenso e onde ele fica no arquivo.
export const TITLE_SCHEMAS = [
  { file: path.join(root, "schemas", "common.queue-task-title.v1.schema.json"), title: (schema) => schema },
  ...[1, 2].map((version) => ({
    file: path.join(root, "schemas", `record.queue-task.v${version}.schema.json`),
    title: (schema) => schema.properties.title,
  })),
];

function escape(code) {
  return code <= 0xffff ? `\\u${code.toString(16).padStart(4, "0")}` : `\\u{${code.toString(16).toUpperCase()}}`;
}

function charClass(ranges, negated = false) {
  const body = ranges
    .map(([a, b]) => [Number.parseInt(a, 16), Number.parseInt(b, 16)])
    .map(([a, b]) => (a === b ? escape(a) : `${escape(a)}-${escape(b)}`))
    .join("");
  return `[${negated ? "^" : ""}${body}]`;
}

// O título usa só o alfabeto permitido, tem ao menos uma letra ou um número, não começa nem termina com espaço,
// não repete espaço e não empilha quatro marcas combinantes.
export function titlePatterns(classes) {
  return {
    pattern: `^${charClass(classes.allowed)}*$`,
    not: `^${charClass(classes.letters_and_digits, true)}*$|^ | $|  |${charClass(classes.marks)}{4}`,
  };
}

export function readAlphabet() {
  return JSON.parse(fs.readFileSync(ALPHABET_FILE, "utf8"));
}

if (process.argv[1] === fileURLToPath(import.meta.url) && process.argv.includes("--write")) {
  const { pattern, not } = titlePatterns(readAlphabet().classes);
  for (const { file, title } of TITLE_SCHEMAS) {
    const schema = JSON.parse(fs.readFileSync(file, "utf8"));
    title(schema).pattern = pattern;
    title(schema).not = { pattern: not };
    fs.writeFileSync(file, `${JSON.stringify(schema, null, 2)}\n`);
  }
  console.log("title-alphabet: Schemas regravados");
}
