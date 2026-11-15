// Gera os tipos Rust (serde) a partir dos Schemas. A saída fica versionada em crates/contracts/src/generated.
//
// Cada palavra-chave de cada nó ou vira tipo, ou vira regra conferida na construção, ou derruba a geração:
// nada do Schema é descartado em silêncio. O que os tipos gerados usam em tempo de execução (erro, padrões de
// texto, regras entre campos) está escrito à mão em crates/contracts/src/support.rs.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { tsName } from "./gen-lib.mjs";
import { createAjv, idToKey, loadSchemas } from "./lib.mjs";

const ANNOTATIONS = new Set(["$schema", "$id", "title", "description", "examples", "$comment", "$defs"]);
const MERGED = Symbol("strings fundidas por allOf");
const RUST_KEYWORDS = new Set(
  ("abstract as async await become box break const continue do dyn else enum extern false final fn for gen if impl in let " +
    "loop macro match mod move mut override priv pub ref return static struct trait true try type typeof unsafe unsized " +
    "use virtual where while yield").split(" "),
);
const RAW_FORBIDDEN = new Set(["self", "Self", "super", "crate"]);

export const rustName = tsName;

// urn:ads:event:session.created:1 -> event_session_created_v1
export function moduleName(id) {
  return idToKey(id).replace(/[.-]/g, "_");
}

function pascal(text) {
  return String(text)
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((w) => w[0].toUpperCase() + w.slice(1))
    .join("");
}

function screaming(name) {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toUpperCase();
}

function rustString(text, at) {
  if (/[\u0000-\u001f\u007f]/.test(text)) throw new Error(`${at}: texto com caractere de controle não vira literal Rust`);
  return `"${text.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

function rawString(text) {
  let hashes = "";
  while (text.includes(`"${hashes}`)) hashes += "#";
  return `r${hashes}"${text}"${hashes}`;
}

// Traduz a expressão regular do Schema (ECMA-262) para a sintaxe do regex-lite, aceitando só o subconjunto em que
// as duas têm o mesmo significado. Qualquer construção fora da lista derruba a geração.
export function rustPattern(source, at) {
  const bad = (why) => {
    throw new Error(`${at}: padrão ${JSON.stringify(source)} fora do subconjunto suportado (${why})`);
  };
  let out = "";
  let inClass = false;
  let i = 0;
  while (i < source.length) {
    const c = source[i];
    if (c === "\\") {
      const next = source[i + 1];
      if (next === "u") {
        const hex = source.slice(i + 2, i + 6);
        if (!/^[0-9a-fA-F]{4}$/.test(hex)) bad("escape \\u incompleto");
        out += `\\x{${hex}}`;
        i += 6;
      } else if (next !== undefined && "\\./-][^$*+?(){}|".includes(next)) {
        out += next === "/" ? "/" : `\\${next}`;
        i += 2;
      } else {
        bad(`escape \\${next ?? ""}`);
      }
    } else if (inClass) {
      if (c === "[") bad("colchete dentro de classe");
      if ("&~-".includes(c) && source[i + 1] === c) bad(`"${c}${c}" dentro de classe`);
      if (c === "]") inClass = false;
      out += c;
      i += 1;
    } else if (c === "[") {
      inClass = true;
      out += c;
      i += 1;
      if (source[i] === "^") {
        out += "^";
        i += 1;
      }
      if (source[i] === "]") bad("classe vazia");
    } else if (c === "(") {
      if (source[i + 1] === "?") {
        if (source[i + 2] !== ":") bad("grupo especial");
        out += "(?:";
        i += 3;
      } else {
        out += c;
        i += 1;
      }
    } else if (c === "{") {
      const m = /^\{[0-9]+(,[0-9]*)?\}/.exec(source.slice(i));
      if (!m) bad("chave solta");
      out += m[0];
      i += m[0].length;
      if ("?+".includes(source[i] ?? " ")) bad("quantificador preguiçoso ou possessivo");
    } else if ("*+?".includes(c)) {
      out += c;
      i += 1;
      if ("?+".includes(source[i] ?? " ")) bad("quantificador preguiçoso ou possessivo");
    } else if (c === ".") {
      bad("ponto fora de classe");
    } else if (c === "}" || c === "]") {
      bad(`"${c}" solto`);
    } else {
      out += c;
      i += 1;
    }
  }
  if (inClass) bad("classe aberta");
  return out;
}

function words(name) {
  return name.match(/[A-Z][a-z0-9]*/g) ?? [name];
}

// Nomes das variantes de um enum. Palavra inicial ou final comum a todas sai (o lint de nomes de variante a recusa).
function variantNames(values, at) {
  let names = values.map((v) => {
    if (typeof v === "number") return v < 0 ? `Neg${-v}` : `N${v}`;
    const name = pascal(v);
    return /^[0-9]/.test(name) ? `V${name}` : name;
  });
  for (const end of [0, -1]) {
    for (;;) {
      const split = names.map(words);
      if (names.length < 2 || split.some((w) => w.length < 2)) break;
      const word = split[0].at(end);
      if (!split.every((w) => w.at(end) === word)) break;
      names = split.map((w) => (end === 0 ? w.slice(1) : w.slice(0, -1)).join(""));
    }
  }
  if (names.some((n) => !/^[A-Z][A-Za-z0-9]*$/.test(n)) || new Set(names).size !== names.length) {
    throw new Error(`${at}: valores de enum não viram nomes de variante distintos (${values.join(", ")})`);
  }
  return names;
}

function fieldIdent(prop, at) {
  let ident = prop.replace(/^_+/, "");
  if (!/^[a-z][a-z0-9_]*$/.test(ident)) throw new Error(`${at}: propriedade "${prop}" não vira nome de campo`);
  if (RAW_FORBIDDEN.has(ident)) ident += "_";
  const renamed = ident !== prop;
  return { ident: RUST_KEYWORDS.has(ident) ? `r#${ident}` : ident, renamed };
}

function doc(text) {
  return text
    .split("\n")
    .map((l) => `/// ${l.trim()}`.trimEnd())
    .join("\n");
}

class Module {
  constructor(gen, id, file, schema) {
    this.gen = gen;
    this.id = id;
    this.file = file;
    this.schema = schema;
    this.root = rustName(id);
    this.mod = moduleName(id);
    this.items = [];
    this.uses = new Map();
    this.defs = new Map();
  }

  use(full) {
    const at = full.lastIndexOf("::");
    const prefix = full.slice(0, at);
    if (!this.uses.has(prefix)) this.uses.set(prefix, new Set());
    this.uses.get(prefix).add(full.slice(at + 2));
  }

  // Confere que o nó só usa as palavras-chave que quem chama sabe traduzir.
  take(node, at, allowed) {
    for (const k of Object.keys(node)) {
      if (!ANNOTATIONS.has(k) && !allowed.includes(k)) {
        throw new Error(`${this.file}${at.slice(1)}: palavra-chave "${k}" sem tradução para Rust neste lugar`);
      }
    }
  }

  fail(at, why) {
    throw new Error(`${this.file}${at.slice(1)}: ${why}`);
  }

  docOf(node, where) {
    const lines = [];
    if (where.at === "#") lines.push(`${this.schema.title}. ${this.schema.description}`, "", `\`${this.id}\`.`);
    else {
      if (typeof node.description === "string") lines.push(node.description, "");
      lines.push(`\`${where.path}\` in \`${this.id}\`.`);
    }
    return doc(lines.join("\n"));
  }

  claim(name, at) {
    if (this.gen.names.has(name)) this.fail(at, `o nome de tipo ${name} já foi usado`);
    this.gen.names.add(name);
  }

  // Devolve { ty, hash, copy, nullable } e deixa em this.items as definições que o tipo precisa.
  typeOf(node, name, where) {
    const slot = this.items.length;
    this.items.push(null);
    const emit = (text) => {
      this.claim(name, where.at);
      this.items[slot] = text;
    };
    return this.build(node, name, where, emit);
  }

  build(node, name, where, emit) {
    const at = where.at;
    if (node === null || typeof node !== "object" || Array.isArray(node)) this.fail(at, "Schema que não é objeto");
    if ("$ref" in node) {
      this.take(node, at, ["$ref"]);
      return this.refType(node.$ref, at);
    }
    if ("allOf" in node && !("type" in node)) return this.mergedType(node, name, where, emit);
    if ("enum" in node) return this.enumType(node, name, where, emit);
    if ("const" in node) return this.constType(node, name, where, emit);
    if ("anyOf" in node && !("type" in node)) return this.anyOfType(node, name, where, emit);
    if ("oneOf" in node && !("type" in node)) return this.taggedType(node, name, where, emit);
    if (Array.isArray(node.type)) return this.typeListType(node, name, where, emit);
    switch (node.type) {
      case "string":
        return this.stringType(node, name, where, emit);
      case "integer":
        return this.integerType(node, name, where, emit);
      case "number":
        return this.numberType(node, name, where, emit);
      case "boolean":
        this.take(node, at, ["type"]);
        return { ty: "bool", hash: true, copy: true };
      case "array":
        return this.arrayType(node, name, where, emit);
      case "object":
        return this.objectType(node, name, where, emit, []);
      default:
        return this.fail(at, `forma de Schema sem tradução para Rust (type: ${JSON.stringify(node.type)})`);
    }
  }

  refType(ref, at) {
    if (ref.startsWith("#/$defs/")) {
      const key = ref.slice("#/$defs/".length);
      if (!this.defs.has(key)) {
        const target = this.schema.$defs?.[key];
        if (!target) this.fail(at, `$ref sem definição: ${ref}`);
        this.defs.set(key, null);
        this.defs.set(key, this.typeOf(target, this.root + pascal(key), { at: `#/$defs/${key}`, path: `$defs.${key}` }));
      }
      const info = this.defs.get(key);
      if (!info) this.fail(at, `$ref em ciclo: ${ref}`);
      return info;
    }
    const other = this.gen.module(ref, `${this.file}${at.slice(1)}`);
    if (other.info.ty === other.root) this.use(`super::${other.mod}::${other.root}`);
    return other.info;
  }

  // ---- texto ---------------------------------------------------------------------------------------------

  stringLimits(node, at, into) {
    this.take(node, at, ["type", "minLength", "maxLength", "pattern", "format", "not"]);
    if (node.type !== "string") this.fail(at, "fusão de texto com o que não é texto");
    if (node.minLength !== undefined) into.min = Math.max(into.min ?? 0, node.minLength);
    if (node.maxLength !== undefined) into.max = Math.min(into.max ?? Infinity, node.maxLength);
    if (node.pattern !== undefined) into.patterns.push(rustPattern(node.pattern, `${this.file}${at.slice(1)}`));
    if (node.format !== undefined) {
      if (node.format !== "date-time") this.fail(at, `format "${node.format}" sem tradução`);
      into.dateTime = true;
    }
    if (node.not !== undefined) {
      const not = node.not;
      const list = "anyOf" in not ? not.anyOf : [not];
      this.take(not, `${at}/not`, ["anyOf", "pattern"]);
      if ("anyOf" in not && "pattern" in not) this.fail(`${at}/not`, "not com anyOf e pattern juntos");
      for (const [i, sub] of list.entries()) {
        this.take(sub, `${at}/not/${i}`, ["pattern"]);
        if (typeof sub.pattern !== "string") this.fail(`${at}/not`, "em texto, not só nega padrões");
        into.excluded.push(rustPattern(sub.pattern, `${this.file}${at.slice(1)}/not`));
      }
    }
  }

  stringType(node, name, where, emit) {
    const limits = { patterns: [], excluded: [], dateTime: false };
    for (const part of node[MERGED] ?? [node]) this.stringLimits(part, where.at, limits);
    const { min, max, patterns, excluded, dateTime } = limits;
    const hasMin = min !== undefined && min > 0;
    const hasMax = max !== undefined && max !== Infinity;
    if (!hasMin && !hasMax && !patterns.length && !excluded.length && !dateTime) return { ty: "String", hash: true };

    const base = screaming(name);
    const statics = [];
    const checks = [];
    if (hasMin && hasMax) checks.push(min === max ? `length == ${min}` : `(${min}..=${max}).contains(&length)`);
    else if (hasMin) checks.push(`length >= ${min}`);
    else if (hasMax) checks.push(`length <= ${max}`);
    const declare = (list, label, negate) => {
      list.forEach((p, i) => {
        const id = `${base}_${label}${list.length > 1 ? `_${i + 1}` : ""}`;
        statics.push(`static ${id}: Pattern = Pattern::new(${rawString(p)});`);
        checks.push(`${negate ? "!" : ""}${id}.matches(text)`);
        this.gen.patterns.add(p);
      });
    };
    declare(patterns, "PATTERN", false);
    declare(excluded, "EXCLUDED", true);
    if (dateTime) checks.push("support::is_date_time(text)");
    if (statics.length) this.use("crate::support::Pattern");
    if (dateTime) this.use("crate::support");
    for (const u of ["std::fmt", "serde::Deserialize", "serde::Serialize", "serde::Serializer", "crate::support::ContractError"]) this.use(u);

    emit(`${statics.join("\n")}

${this.docOf(node, where)}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct ${name}(String);

impl ${name} {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("${name}", text))
        }
    }

    fn accepts(text: &str) -> bool {
        ${hasMin || hasMax ? "let length = text.chars().count();" : ""}
        ${checks.join(" && ")}
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ${name} {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<${name}> for String {
    fn from(value: ${name}) -> String {
        value.0
    }
}

impl fmt::Display for ${name} {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for ${name} {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}`);
    return { ty: name, hash: true };
  }

  // ---- números -------------------------------------------------------------------------------------------

  bounds(node, at) {
    this.take(node, at, ["type", "minimum", "maximum"]);
    for (const k of ["minimum", "maximum"]) {
      if (node[k] !== undefined && !Number.isSafeInteger(node[k])) this.fail(at, `${k} que não é inteiro exato`);
    }
    return { min: node.minimum, max: node.maximum };
  }

  integerType(node, name, where, emit) {
    const { min, max } = this.bounds(node, where.at);
    if (max === undefined) {
      if (min === undefined) return { ty: "i64", hash: true, copy: true };
      if (min === 0) return { ty: "u64", hash: true, copy: true };
      if (min === 1) {
        this.use("std::num::NonZeroU64");
        return { ty: "NonZeroU64", hash: true, copy: true };
      }
    }
    if (min !== undefined && max !== undefined && min > max) this.fail(where.at, "minimum maior que maximum");
    let repr = "i64";
    if (min !== undefined && min >= 0) repr = max === undefined || max > 0xffffffff ? "u64" : max > 0xffff ? "u32" : "u16";
    let check;
    if (min !== undefined && max !== undefined) check = `(${min}..=${max}).contains(&value)`;
    else if (min !== undefined) check = `value >= ${min}`;
    else check = `value <= ${max}`;
    for (const u of ["serde::Deserialize", "serde::Serialize", "serde::Serializer", "crate::support::ContractError"]) this.use(u);
    emit(`${this.docOf(node, where)}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "${repr}")]
pub struct ${name}(${repr});

impl ${name} {
    pub fn new(value: ${repr}) -> Result<Self, ContractError> {
        if ${check} {
            Ok(Self(value))
        } else {
            Err(ContractError::new("${name}", value.to_string()))
        }
    }

    pub fn get(self) -> ${repr} {
        self.0
    }
}

impl TryFrom<${repr}> for ${name} {
    type Error = ContractError;

    fn try_from(value: ${repr}) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<${name}> for ${repr} {
    fn from(value: ${name}) -> ${repr} {
        value.0
    }
}

impl Serialize for ${name} {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}`);
    return { ty: name, hash: true, copy: true };
  }

  // `number` fica em serde_json::Number: nunca é NaN nem infinito e volta ao JSON como veio (12 não vira 12.0).
  numberType(node, name, where, emit) {
    const { min, max } = this.bounds(node, where.at);
    this.use("serde_json::Number");
    if (min === undefined && max === undefined) return { ty: "Number", hash: true };
    const checks = [];
    if (min !== undefined && max !== undefined) checks.push(`(${min}.0..=${max}.0).contains(&number)`);
    else if (min !== undefined) checks.push(`number >= ${min}.0`);
    else checks.push(`number <= ${max}.0`);
    for (const u of ["serde::Deserialize", "serde::Serialize", "serde::Serializer", "crate::support::ContractError"]) this.use(u);
    emit(`${this.docOf(node, where)}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Number")]
pub struct ${name}(Number);

impl ${name} {
    pub fn new(value: Number) -> Result<Self, ContractError> {
        if value.as_f64().is_some_and(|number| ${checks.join(" && ")}) {
            Ok(Self(value))
        } else {
            Err(ContractError::new("${name}", value.to_string()))
        }
    }

    pub fn get(&self) -> &Number {
        &self.0
    }
}

impl TryFrom<Number> for ${name} {
    type Error = ContractError;

    fn try_from(value: Number) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<${name}> for Number {
    fn from(value: ${name}) -> Number {
        value.0
    }
}

impl Serialize for ${name} {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}`);
    return { ty: name, hash: true };
  }

  // ---- enum e const --------------------------------------------------------------------------------------

  enumType(node, name, where, emit) {
    const at = where.at;
    this.take(node, at, ["enum", "type"]);
    const values = node.enum;
    if (!Array.isArray(values) || values.length === 0) this.fail(at, "enum vazio");
    if (new Set(values).size !== values.length) this.fail(at, "enum com valor repetido");
    const variants = variantNames(values, `${this.file}${at.slice(1)}`);
    this.use("serde::Deserialize");
    this.use("serde::Serialize");
    if (values.every((v) => typeof v === "string")) {
      if (node.type !== undefined && node.type !== "string") this.fail(at, "enum de texto com outro type");
      const body = values.map((v, i) => `    #[serde(rename = ${rustString(v, at)})]\n    ${variants[i]},`).join("\n");
      const arms = values.map((v, i) => `            Self::${variants[i]} => ${rustString(v, at)},`).join("\n");
      emit(`${this.docOf(node, where)}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ${name} {
${body}
}

impl ${name} {
    pub fn as_str(self) -> &'static str {
        match self {
${arms}
        }
    }
}`);
    } else if (values.every((v) => Number.isSafeInteger(v))) {
      if (node.type !== undefined && node.type !== "integer") this.fail(at, "enum de inteiros com outro type");
      this.use("crate::support::ContractError");
      emit(`${this.docOf(node, where)}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub enum ${name} {
${variants.map((v) => `    ${v},`).join("\n")}
}

impl ${name} {
    pub fn number(self) -> i64 {
        match self {
${values.map((v, i) => `            Self::${variants[i]} => ${v},`).join("\n")}
        }
    }
}

impl TryFrom<i64> for ${name} {
    type Error = ContractError;

    fn try_from(number: i64) -> Result<Self, ContractError> {
        match number {
${values.map((v, i) => `            ${v} => Ok(Self::${variants[i]}),`).join("\n")}
            other => Err(ContractError::new("${name}", other.to_string())),
        }
    }
}

impl From<${name}> for i64 {
    fn from(value: ${name}) -> i64 {
        value.number()
    }
}`);
    } else {
      this.fail(at, "enum que mistura tipos");
    }
    return { ty: name, hash: true, copy: true };
  }

  constType(node, name, where, emit) {
    const at = where.at;
    this.take(node, at, ["const", "type"]);
    const value = node.const;
    let ty, literal, write, read;
    if (typeof value === "string") {
      [ty, literal, write, read] = ["&str", rustString(value, at), "serialize_str", "expect_str"];
    } else if (typeof value === "boolean") {
      [ty, literal, write, read] = ["bool", String(value), "serialize_bool", "expect_bool"];
    } else if (Number.isSafeInteger(value)) {
      [ty, literal, write, read] = ["i64", String(value), "serialize_i64", "expect_i64"];
    } else {
      this.fail(at, "const que não é texto, booleano nem inteiro");
    }
    for (const u of ["serde::Deserialize", "serde::Deserializer", "serde::Serialize", "serde::Serializer", "crate::support"]) this.use(u);
    emit(`${this.docOf(node, where)}
///
/// Always \`${JSON.stringify(value)}\`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ${name};

impl ${name} {
    pub const VALUE: ${ty} = ${literal};
}

impl Serialize for ${name} {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.${write}(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ${name} {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::${read}(deserializer, Self::VALUE).map(|()| Self)
    }
}`);
    return { ty: name, hash: true, copy: true };
  }

  // ---- uniões --------------------------------------------------------------------------------------------

  isNull(node) {
    return node && typeof node === "object" && node.type === "null" && Object.keys(node).every((k) => k === "type" || ANNOTATIONS.has(k));
  }

  optional(inner, at) {
    if (inner.nullable) this.fail(at, "null aceito duas vezes");
    return { ty: `Option<${inner.ty}>`, hash: inner.hash, copy: inner.copy, nullable: true };
  }

  untagged(node, name, where, emit, variants) {
    if (new Set(variants.map((v) => v.name)).size !== variants.length) this.fail(where.at, "alternativas sem nomes distintos");
    this.use("serde::Deserialize");
    this.use("serde::Serialize");
    const hash = variants.every((v) => v.info.hash);
    emit(`${this.docOf(node, where)}
#[derive(Debug, Clone, PartialEq, Eq, ${hash ? "Hash, " : ""}Serialize, Deserialize)]
#[serde(untagged)]
pub enum ${name} {
${variants.map((v) => `    ${v.name}(${v.info.ty}),`).join("\n")}
}`);
    return { ty: name, hash };
  }

  anyOfType(node, name, where, emit) {
    const at = where.at;
    this.take(node, at, ["anyOf"]);
    const branches = node.anyOf;
    if (!Array.isArray(branches) || branches.length < 2) this.fail(at, "anyOf com menos de duas alternativas");
    const nulls = branches.filter((b) => this.isNull(b));
    if (nulls.length === 1 && branches.length === 2) {
      const i = branches.findIndex((b) => !this.isNull(b));
      return this.optional(this.build(branches[i], name, { at: `${at}/anyOf/${i}`, path: where.path }, emit), at);
    }
    if (nulls.length) this.fail(at, "anyOf com null e mais de uma alternativa");
    const variants = branches.map((b, i) => {
      let label;
      if (typeof b.$ref === "string") label = b.$ref.startsWith("#/$defs/") ? b.$ref.slice(8) : b.$ref.split(":")[3];
      else if (typeof b.type === "string") label = b.type;
      else this.fail(`${at}/anyOf/${i}`, "alternativa sem $ref nem type para dar nome à variante");
      const variant = pascal(label);
      const sub = { at: `${at}/anyOf/${i}`, path: `${where.path}|${label}` };
      return { name: variant, info: this.typeOf(b, name + variant, sub) };
    });
    return this.untagged(node, name, where, emit, variants);
  }

  typeListType(node, name, where, emit) {
    const at = where.at;
    const types = node.type;
    if (new Set(types).size !== types.length || types.length < 2) this.fail(at, "lista de type repetida ou curta");
    if (types.includes("null")) {
      const rest = types.filter((t) => t !== "null");
      const inner = { ...node, type: rest.length === 1 ? rest[0] : rest };
      return this.optional(this.build(inner, name, where, emit), at);
    }
    this.take(node, at, ["type"]);
    if (types.includes("integer") && types.includes("number")) this.fail(at, "integer e number na mesma lista");
    const variants = types.map((t) => {
      const info = this.build({ type: t }, `${name}${pascal(t)}`, { at, path: `${where.path}|${t}` }, () => {
        this.fail(at, `type "${t}" numa lista precisa de tipo próprio`);
      });
      return { name: pascal(t), info };
    });
    return this.untagged(node, name, where, emit, variants);
  }

  // oneOf de objetos distinguidos por um campo de valor fixo: enum com a marca do Schema.
  taggedType(node, name, where, emit) {
    const at = where.at;
    this.take(node, at, ["oneOf"]);
    const branches = node.oneOf;
    if (!Array.isArray(branches) || branches.length < 2) this.fail(at, "oneOf com menos de duas alternativas");
    const isTag = (b, p) => {
      const s = b?.properties?.[p];
      return b.type === "object" && s && typeof s.const === "string" && Object.keys(s).length === 1 && b.required?.includes(p);
    };
    const tag = Object.keys(branches[0]?.properties ?? {}).find(
      (p) => branches.every((b) => isTag(b, p)) && new Set(branches.map((b) => b.properties[p].const)).size === branches.length,
    );
    if (!tag) this.fail(at, "oneOf sem campo de valor fixo que distinga as alternativas");
    const values = branches.map((b) => b.properties[tag].const);
    const names = variantNames(values, `${this.file}${at.slice(1)}`);
    const infos = branches.map((b, i) => {
      const rest = structuredClone(b);
      delete rest.properties[tag];
      rest.required = rest.required.filter((p) => p !== tag);
      return this.typeOf(rest, name + names[i], { at: `${at}/oneOf/${i}`, path: `${where.path}|${values[i]}`.replace(/^\|/, "") });
    });
    this.use("serde::Deserialize");
    this.use("serde::Serialize");
    const hash = infos.every((i) => i.hash);
    emit(`${this.docOf(node, where)}
#[derive(Debug, Clone, PartialEq, Eq, ${hash ? "Hash, " : ""}Serialize, Deserialize)]
#[serde(tag = ${rustString(tag, at)})]
pub enum ${name} {
${values.map((v, i) => `    #[serde(rename = ${rustString(v, at)})]\n    ${names[i]}(${infos[i].ty}),`).join("\n")}
}`);
    return { ty: name, hash };
  }

  // ---- listas --------------------------------------------------------------------------------------------

  arrayType(node, name, where, emit) {
    const at = where.at;
    this.take(node, at, ["type", "items", "minItems", "maxItems", "uniqueItems"]);
    let item;
    if (node.items === undefined) {
      this.use("serde_json::Value");
      item = { ty: "Value", hash: false };
    } else {
      item = this.typeOf(node.items, `${name}Item`, { at: `${at}/items`, path: `${where.path}[]` });
    }
    const min = node.minItems ?? 0;
    const max = node.maxItems;
    const unique = node.uniqueItems === true;
    if (min === 0 && max === undefined && !unique) return { ty: `Vec<${item.ty}>`, hash: item.hash };
    const checks = [];
    if (min > 0 && max !== undefined) checks.push(min === max ? `items.len() == ${min}` : `(${min}..=${max}).contains(&items.len())`);
    else if (min === 1) checks.push("!items.is_empty()");
    else if (min > 0) checks.push(`items.len() >= ${min}`);
    else if (max === 0) checks.push("items.is_empty()");
    else if (max !== undefined) checks.push(`items.len() <= ${max}`);
    if (unique) checks.push(`support::${item.hash ? "all_distinct" : "all_distinct_slow"}(&items)`);
    if (unique) this.use("crate::support");
    for (const u of ["serde::Deserialize", "serde::Serialize", "serde::Serializer", "crate::support::ContractError"]) this.use(u);
    emit(`${this.docOf(node, where)}
#[derive(Debug, Clone, PartialEq, Eq, ${item.hash ? "Hash, " : ""}Deserialize)]
#[serde(try_from = "Vec<${item.ty}>")]
pub struct ${name}(Vec<${item.ty}>);

impl ${name} {
    pub fn new(items: Vec<${item.ty}>) -> Result<Self, ContractError> {
        if ${checks.join(" && ")} {
            Ok(Self(items))
        } else {
            Err(ContractError::new("${name}", format!("{} items", items.len())))
        }
    }

    pub fn as_slice(&self) -> &[${item.ty}] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<${item.ty}> {
        self.0
    }
}

impl TryFrom<Vec<${item.ty}>> for ${name} {
    type Error = ContractError;

    fn try_from(items: Vec<${item.ty}>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<${name}> for Vec<${item.ty}> {
    fn from(value: ${name}) -> Vec<${item.ty}> {
        value.0
    }
}

impl Serialize for ${name} {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}`);
    return { ty: name, hash: item.hash };
  }

  // ---- regras entre campos -------------------------------------------------------------------------------

  literal(value, at) {
    if (value === null) return "Literal::Null";
    if (typeof value === "boolean") return `Literal::Bool(${value})`;
    if (Number.isSafeInteger(value)) return `Literal::Int(${value})`;
    if (typeof value === "string") return `Literal::Str(${rustString(value, at)})`;
    return this.fail(at, "valor de const ou enum que não é null, booleano, inteiro nem texto");
  }

  // Traduz um sub-Schema condicional para a árvore Rule, palavra-chave por palavra-chave.
  ruleOf(schema, at, ctx) {
    if (schema === null || typeof schema !== "object" || Array.isArray(schema)) this.fail(at, "regra que não é objeto");
    this.use("crate::support::Rule");
    const parts = [];
    const sub = (s, where) => this.ruleOf(s, where, ctx);
    const list = (k) => `&[${schema[k].map((s, i) => sub(s, `${at}/${k}/${i}`)).join(", ")}]`;
    const size = (k) => {
      if (!Number.isSafeInteger(schema[k]) || schema[k] < 0) this.fail(at, `${k} inválido`);
      return schema[k];
    };
    for (const k of Object.keys(schema)) {
      if (k === "description" || k === "$comment") continue;
      switch (k) {
        case "type": {
          this.use("crate::support::JsonType");
          const types = Array.isArray(schema.type) ? schema.type : [schema.type];
          const known = ["null", "boolean", "integer", "number", "string", "array", "object"];
          if (!types.length || types.some((t) => !known.includes(t))) this.fail(at, "type desconhecido numa regra");
          parts.push(`Rule::Type(&[${types.map((t) => `JsonType::${pascal(t)}`).join(", ")}])`);
          break;
        }
        case "const":
          this.use("crate::support::Literal");
          parts.push(`Rule::Const(${this.literal(schema.const, at)})`);
          break;
        case "enum":
          this.use("crate::support::Literal");
          parts.push(`Rule::Enum(&[${schema.enum.map((v) => this.literal(v, at)).join(", ")}])`);
          break;
        case "required":
          parts.push(`Rule::Required(&[${schema.required.map((p) => rustString(p, at)).join(", ")}])`);
          break;
        case "properties": {
          const entries = Object.entries(schema.properties).map(([p, s]) => `(${rustString(p, at)}, ${sub(s, `${at}/properties/${p}`)})`);
          parts.push(`Rule::Properties(&[${entries.join(", ")}])`);
          break;
        }
        case "pattern": {
          this.use("crate::support::Pattern");
          const source = rustPattern(schema.pattern, `${this.file}${at.slice(1)}`);
          this.gen.patterns.add(source);
          const id = `${ctx.base}_RULE_PATTERN_${ctx.statics.length + 1}`;
          ctx.statics.push(`static ${id}: Pattern = Pattern::new(${rawString(source)});`);
          parts.push(`Rule::Pattern(&${id})`);
          break;
        }
        case "minLength":
          parts.push(`Rule::MinLength(${size(k)})`);
          break;
        case "maxLength":
          parts.push(`Rule::MaxLength(${size(k)})`);
          break;
        case "minItems":
          parts.push(`Rule::MinItems(${size(k)})`);
          break;
        case "maxItems":
          parts.push(`Rule::MaxItems(${size(k)})`);
          break;
        case "minimum":
        case "maximum":
          if (!Number.isSafeInteger(schema[k])) this.fail(at, `${k} que não é inteiro exato`);
          parts.push(`Rule::${pascal(k)}(${schema[k]}.0)`);
          break;
        case "items":
          parts.push(`Rule::Items(&${sub(schema.items, `${at}/items`)})`);
          break;
        case "not":
          parts.push(`Rule::Not(&${sub(schema.not, `${at}/not`)})`);
          break;
        case "allOf":
          parts.push(`Rule::AllOf(${list(k)})`);
          break;
        case "anyOf":
          parts.push(`Rule::AnyOf(${list(k)})`);
          break;
        case "oneOf":
          parts.push(`Rule::OneOf(${list(k)})`);
          break;
        case "if": {
          const branch = (name) => (name in schema ? `Some(&${sub(schema[name], `${at}/${name}`)})` : "None");
          parts.push(`Rule::If { when: &${sub(schema.if, `${at}/if`)}, then: ${branch("then")}, otherwise: ${branch("else")} }`);
          break;
        }
        case "then":
        case "else":
          if (!("if" in schema)) this.fail(at, `${k} sem if`);
          break;
        default:
          this.fail(at, `palavra-chave "${k}" sem tradução para regra`);
      }
    }
    return parts.length === 1 ? parts[0] : `Rule::AllOf(&[${parts.join(", ")}])`;
  }

  // ---- objetos -------------------------------------------------------------------------------------------

  objectType(node, name, where, emit, inherited) {
    const at = where.at;
    this.take(node, at, ["type", "properties", "required", "additionalProperties", "if", "then", "else", "allOf", "oneOf", "anyOf", "not"]);
    const extra = node.additionalProperties;
    const required = node.required ?? [];
    const ctx = { base: screaming(name), statics: [] };
    const rules = [...inherited.map(([place, schema]) => [place, this.ruleOf(schema, place, ctx)])];
    if ("then" in node || "else" in node || "if" in node) {
      const cond = Object.fromEntries(["if", "then", "else"].filter((k) => k in node).map((k) => [k, node[k]]));
      rules.push([`${at}/if`, this.ruleOf(cond, at, ctx)]);
    }
    (node.allOf ?? []).forEach((s, i) => rules.push([`${at}/allOf/${i}`, this.ruleOf(s, `${at}/allOf/${i}`, ctx)]));
    for (const k of ["oneOf", "anyOf", "not"]) {
      if (k in node) rules.push([`${at}/${k}`, this.ruleOf({ [k]: node[k] }, at, ctx)]);
    }

    if (extra !== null && typeof extra === "object") {
      if (node.properties || required.length || rules.length) this.fail(at, "mapa com propriedades, required ou regras");
      const value = this.typeOf(extra, `${name}Value`, { at: `${at}/additionalProperties`, path: `${where.path}.*` });
      this.use("std::collections::BTreeMap");
      return { ty: `BTreeMap<String, ${value.ty}>`, hash: value.hash };
    }
    if (extra !== false && !node.properties) {
      if (required.length || rules.length) this.fail(at, "objeto aberto sem propriedades, mas com required ou regras");
      this.use("serde_json::Map");
      this.use("serde_json::Value");
      return { ty: "Map<String, Value>", hash: false };
    }
    if (extra !== false && extra !== true && extra !== undefined) this.fail(at, "additionalProperties inválido");

    const open = extra !== false;
    const props = Object.entries(node.properties ?? {});
    for (const r of required) {
      if (!props.some(([p]) => p === r)) this.fail(at, `required cita "${r}", que não está em properties`);
    }
    const idents = new Set(open ? ["extra"] : []);
    let hash = !open;
    const fields = props.map(([prop, schema]) => {
      const sub = { at: `${at}/properties/${prop}`, path: where.path ? `${where.path}.${prop}` : prop };
      const info = this.typeOf(schema, name + pascal(prop), sub);
      const { ident, renamed } = fieldIdent(prop, `${this.file}${sub.at.slice(1)}`);
      if (idents.has(ident)) this.fail(sub.at, `dois campos viram ${ident}`);
      idents.add(ident);
      hash &&= info.hash;
      const attrs = renamed ? [`rename = ${rustString(prop, at)}`] : [];
      let ty = info.ty;
      if (!required.includes(prop)) {
        attrs.push("default", 'skip_serializing_if = "Option::is_none"', 'deserialize_with = "support::present"');
        ty = `Option<${ty}>`;
        this.use("crate::support");
      } else if (info.nullable) {
        attrs.push('deserialize_with = "support::nullable"');
        this.use("crate::support");
      }
      return `${attrs.length ? `    #[serde(${attrs.join(", ")})]\n` : ""}    pub ${ident}: ${ty},`;
    });
    if (open) {
      this.use("serde_json::Map");
      this.use("serde_json::Value");
      fields.push("    /// What the Schema does not name.\n    #[serde(flatten)]\n    pub extra: Map<String, Value>,");
    }
    this.use("serde::Deserialize");
    this.use("serde::Serialize");
    const derive = `#[derive(Debug, Clone, PartialEq, Eq, ${hash ? "Hash, " : ""}`;
    const checked = open || rules.length > 0;
    const body = fields.length ? ` {\n${fields.join("\n")}\n}` : " {}";
    if (!checked) {
      emit(`${this.docOf(node, where)}
${derive}Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ${name}${body}`);
      return { ty: name, hash };
    }

    this.claim(`${name}Fields`, at);
    for (const u of ["std::ops::Deref", "serde::Serializer", "crate::support", "crate::support::ContractError"]) this.use(u);
    const checks = [];
    if (open) checks.push(`support::check_extra("${name}", &fields.extra, &[${props.map(([p]) => rustString(p, at)).join(", ")}])?;`);
    if (rules.length) checks.push(`support::check_rules("${name}", &fields, ${ctx.base}_RULES)?;`);
    const table = rules.length
      ? `static ${ctx.base}_RULES: &[(&str, Rule)] = &[\n${rules.map(([place, rule]) => `    (${rustString(place, at)}, ${rule}),`).join("\n")}\n];\n`
      : "";
    emit(`${ctx.statics.join("\n")}

${table}
${this.docOf(node, where)}
///
/// Checked: the fields obey the rules the Schema sets between them.
${derive}Deserialize)]
#[serde(try_from = "${name}Fields")]
pub struct ${name}(${name}Fields);

/// The fields of [\`${name}\`], before the rules between them are checked.
${derive}Serialize, Deserialize)]
${open ? "" : "#[serde(deny_unknown_fields)]\n"}pub struct ${name}Fields${body}

impl ${name} {
    pub fn new(fields: ${name}Fields) -> Result<Self, ContractError> {
        ${checks.join("\n        ")}
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> ${name}Fields {
        self.0
    }
}

impl Deref for ${name} {
    type Target = ${name}Fields;

    fn deref(&self) -> &${name}Fields {
        &self.0
    }
}

impl TryFrom<${name}Fields> for ${name} {
    type Error = ContractError;

    fn try_from(fields: ${name}Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<${name}> for ${name}Fields {
    fn from(value: ${name}) -> ${name}Fields {
        value.0
    }
}

impl Serialize for ${name} {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}`);
    return { ty: name, hash };
  }

  // allOf de um $ref a um objeto com extensões que só apertam propriedades dele (todo evento estende o envelope).
  mergedType(node, name, where, emit) {
    const at = where.at;
    this.take(node, at, ["allOf"]);
    const [first, ...rest] = node.allOf;
    if (!first || typeof first.$ref !== "string" || first.$ref.startsWith("#") || rest.length === 0) {
      this.fail(at, "allOf sem type só é traduzido como $ref a outro Schema seguido de extensões");
    }
    this.take(first, `${at}/allOf/0`, ["$ref"]);
    const target = this.gen.schemaOf(first.$ref, `${this.file}${at.slice(1)}`);
    if (target.type !== "object" || !target.properties || target.additionalProperties !== false) {
      this.fail(at, "a base do allOf precisa ser um objeto fechado com propriedades");
    }
    const base = structuredClone(target);
    for (const k of ANNOTATIONS) delete base[k];
    const inherited = [];
    rest.forEach((ext, n) => {
      const here = `${at}/allOf/${n + 1}`;
      this.take(ext, here, ["type", "required", "properties", "if", "then", "else"]);
      if (ext.type !== "object") this.fail(here, "extensão que não é objeto");
      base.required = [...new Set([...(base.required ?? []), ...(ext.required ?? [])])];
      for (const [prop, schema] of Object.entries(ext.properties ?? {})) {
        if (!(prop in base.properties)) this.fail(here, `a extensão cria a propriedade "${prop}", que a base fechada recusa`);
        const merged = this.mergeProperty(base.properties[prop], schema, `${here}/properties/${prop}`);
        if (merged) base.properties[prop] = merged;
        else inherited.push([`${here}/properties/${prop}`, { properties: { [prop]: schema } }]);
      }
      if ("if" in ext || "then" in ext || "else" in ext) {
        const cond = Object.fromEntries(["if", "then", "else"].filter((k) => k in ext).map((k) => [k, ext[k]]));
        inherited.push([`${here}/if`, cond]);
      }
    });
    return this.objectType(base, name, where, emit, inherited);
  }

  // Funde a propriedade da base com a da extensão num Schema só, quando isso tem forma de tipo.
  // Sem forma de tipo devolve null: vale o tipo da base e a extensão vira regra do objeto.
  mergeProperty(base, ext, at) {
    const keys = (s) => Object.keys(s).filter((k) => !ANNOTATIONS.has(k));
    if ("const" in ext && keys(ext).every((k) => k === "const" || k === "type")) {
      if (!this.gen.ajv.compile(base)(ext.const)) this.fail(at, "o valor fixo da extensão não cabe na propriedade da base");
      return ext;
    }
    if (base.type === "string" && ext.type === "string" && !("enum" in base) && !("const" in base) && !("enum" in ext)) {
      return { type: "string", [MERGED]: [...(base[MERGED] ?? [base]), ext] };
    }
    if (base.type === "object" && keys(base).length === 1) {
      const target = typeof ext.$ref === "string" && !ext.$ref.startsWith("#") ? this.gen.schemaOf(ext.$ref, at) : ext;
      if (target.type !== "object") this.fail(at, "a extensão troca um objeto por outra coisa");
      return ext;
    }
    return null;
  }

  render() {
    const info = this.typeOf(this.schema, this.root, { at: "#", path: "" });
    if (info.ty !== this.root) {
      this.claim(this.root, "#");
      this.items.unshift(`${this.docOf(this.schema, { at: "#", path: "" })}\npub type ${this.root} = ${info.ty};`);
    }
    this.info = info;
    const group = (test) =>
      [...this.uses]
        .filter(([prefix]) => test(prefix))
        .sort(([a], [b]) => (a < b ? -1 : 1))
        .map(([prefix, names]) => {
          const list = [...names].sort();
          return `use ${prefix}::${list.length === 1 ? list[0] : `{${list.join(", ")}}`};`;
        })
        .join("\n");
    const local = (p) => p === "crate" || p.startsWith("crate::") || p.startsWith("super::");
    const std = (p) => p === "std" || p.startsWith("std::");
    const imports = [group(std), group((p) => !std(p) && !local(p)), group(local)];
    const header = `// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/${this.file}. Não edite à mão.`;
    this.text = [header, ...imports.filter(Boolean), ...this.items.filter(Boolean)].join("\n\n");
    return this;
  }
}

class Generator {
  constructor(loaded) {
    this.byId = new Map(loaded.map(({ file, schema }) => [schema.$id, { file, schema }]));
    this.modules = new Map();
    this.building = new Set();
    this.names = new Set();
    this.patterns = new Set();
    this.ajv = createAjv();
    for (const { schema } of loaded) this.ajv.addSchema(schema);
  }

  schemaOf(id, from) {
    const hit = this.byId.get(id);
    if (!hit) throw new Error(`${from}: $ref sem Schema correspondente: ${id}`);
    return hit.schema;
  }

  module(id, from) {
    if (!this.modules.has(id)) {
      const schema = this.schemaOf(id, from);
      if (this.building.has(id)) throw new Error(`${from}: $ref em ciclo: ${id}`);
      this.building.add(id);
      this.modules.set(id, new Module(this, id, this.byId.get(id).file, schema).render());
    }
    return this.modules.get(id);
  }
}

function index(modules, patterns) {
  const sorted = [...modules].sort((a, b) => (a.mod < b.mod ? -1 : 1));
  return `// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas. Não edite à mão.

use serde_json::Value;

use crate::support;

${sorted.map((m) => `pub mod ${m.mod};`).join("\n")}

${sorted.map((m) => `pub use ${m.mod}::*;`).join("\n")}

/// Every Schema a type was generated from: its file key and its \`$id\`.
pub const SCHEMAS: &[(&str, &str)] = &[
${sorted.map((m) => `    ("${idToKey(m.id)}", "${m.id}"),`).join("\n")}
];

/// Every text pattern the generated types use, as handed to the regular expression engine.
pub const PATTERNS: &[&str] = &[
${[...patterns].sort().map((p) => `    ${rawString(p)},`).join("\n")}
];

/// Reads \`value\` as the type generated from the Schema \`id\` names and writes it back.
/// \`None\` when no Schema has that \`$id\`.
pub fn reencode(id: &str, value: Value) -> Option<Result<Value, serde_json::Error>> {
    let written = match id {
${sorted.map((m) => `        "${m.id}" => support::reencode::<${m.root}>(value),`).join("\n")}
        _ => return None,
    };
    Some(written)
}
`;
}

function findUp(from, name) {
  for (let dir = path.resolve(from); ; dir = path.dirname(dir)) {
    if (fs.existsSync(path.join(dir, name))) return dir;
    if (path.dirname(dir) === dir) return undefined;
  }
}

// A saída passa pelo rustfmt (o da versão fixada em rust-toolchain.toml) para ser exatamente o que `cargo fmt` aceita.
export function rustfmt(text, configDir) {
  const args = ["--edition", "2024", "--emit", "stdout"];
  if (configDir) args.push("--config-path", configDir);
  const run = spawnSync("rustfmt", args, { input: text, encoding: "utf8", cwd: configDir });
  if (run.error) {
    throw new Error(`não consegui rodar o rustfmt (${run.error.code ?? run.error.message}): os tipos Rust gerados passam por ele; instale o Rust da versão de rust-toolchain.toml`);
  }
  if (run.status !== 0) throw new Error(`o rustfmt recusou o código gerado:\n${run.stderr}`);
  return run.stdout.replace(/\r\n/g, "\n");
}

export function rustOutDir(root) {
  return path.resolve(root, "..", "crates", "contracts", "src", "generated");
}

// Devolve Map<nome do arquivo, texto>: um módulo por Schema mais o índice mod.rs.
export function generateRust(root, { format = rustfmt, outDir = rustOutDir(root) } = {}) {
  const errors = [];
  const loaded = loadSchemas(root, errors);
  if (errors.length) throw new Error(errors.join("\n"));
  const gen = new Generator(loaded);
  for (const { schema } of loaded) gen.module(schema.$id, "schemas");
  const configDir = findUp(outDir, "rustfmt.toml") ?? findUp(root, "rustfmt.toml");
  const out = new Map();
  const mods = new Set();
  for (const m of gen.modules.values()) {
    if (mods.has(m.mod)) throw new Error(`dois Schemas viram o módulo ${m.mod}`);
    mods.add(m.mod);
    out.set(`${m.mod}.rs`, format(`${m.text}\n`, configDir));
  }
  out.set("mod.rs", format(index(gen.modules.values(), gen.patterns), configDir));
  return out;
}

function presentRust(dir) {
  return fs.existsSync(dir) ? fs.readdirSync(dir).filter((f) => f.endsWith(".rs")) : [];
}

// Grava os arquivos e devolve os que sobraram na pasta. Nada é apagado: quem roda remove cada um pelo nome.
export function writeRust(dir, files) {
  fs.mkdirSync(dir, { recursive: true });
  for (const [name, text] of files) fs.writeFileSync(path.join(dir, name), text);
  return presentRust(dir).filter((f) => !files.has(f));
}

// Devolve a lista de diferenças entre o que seria gerado e o que está no disco.
export function diffRust(dir, files, label = "crates/contracts/src/generated") {
  const problems = [];
  for (const [name, text] of files) {
    const file = path.join(dir, name);
    if (!fs.existsSync(file)) problems.push(`faltando: ${label}/${name}`);
    else if (fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n") !== text) problems.push(`desatualizado: ${label}/${name}`);
  }
  for (const f of presentRust(dir)) if (!files.has(f)) problems.push(`sobrando: ${label}/${f}`);
  return problems;
}
