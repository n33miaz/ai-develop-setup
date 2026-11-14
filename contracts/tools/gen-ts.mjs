// npm run gen:ts grava os tipos; npm run gen:check reprova se a saída versionada estiver desatualizada.
import path from "node:path";
import { fileURLToPath } from "node:url";
import { diffAll, generateAll, writeAll } from "./gen-lib.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const files = await generateAll(root);

if (check) {
  const problems = diffAll(root, files);
  if (problems.length) {
    for (const p of problems) console.error(`erro: ${p}`);
    console.error("Tipos gerados fora de dia. Rode `npm run gen:ts` e inclua o resultado no commit.");
    process.exit(1);
  }
  console.log(`gen:check: ok (${files.size} arquivos em dia)`);
} else {
  writeAll(root, files);
  console.log(`gen:ts: ${files.size} arquivos gravados em generated/ts`);
}
