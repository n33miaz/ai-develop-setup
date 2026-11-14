// npm run validate: todo Schema válido e resolvendo referências, dourados no lugar, URNs únicas e no padrão.
import path from "node:path";
import { fileURLToPath } from "node:url";
import { checkContracts } from "./lib.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { errors, stats } = checkContracts(root);

console.log(`schemas: ${stats.schemas}; dourados válidos: ${stats.valid}; dourados inválidos: ${stats.invalid}`);
if (errors.length) {
  for (const e of errors) console.error(`erro: ${e}`);
  console.error(`${errors.length} problema(s).`);
  process.exit(1);
}
console.log("validate: ok");
