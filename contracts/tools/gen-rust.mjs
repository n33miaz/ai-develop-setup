// npm run gen:rust grava os tipos Rust; com --check reprova se a saída versionada estiver desatualizada.
import path from "node:path";
import { fileURLToPath } from "node:url";
import { diffRust, generateRust, rustOutDir, writeRust } from "./gen-rust-lib.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dir = rustOutDir(root);
const label = "crates/contracts/src/generated";
const files = generateRust(root);

if (process.argv.includes("--check")) {
  const problems = diffRust(dir, files, label);
  if (problems.length) {
    for (const p of problems) console.error(`erro: ${p}`);
    console.error("Tipos Rust gerados fora de dia. Rode `npm run gen:rust` e inclua o resultado no commit; arquivo sobrando é removido pelo nome.");
    process.exit(1);
  }
  console.log(`gen:check (rust): ok (${files.size} arquivos em dia)`);
} else {
  const stale = writeRust(dir, files);
  console.log(`gen:rust: ${files.size} arquivos gravados em ${label}`);
  if (stale.length) {
    for (const f of stale) console.error(`erro: sobrando: ${label}/${f}`);
    console.error("O gerador não apaga nada: remova cada arquivo acima pelo nome.");
    process.exit(1);
  }
}
