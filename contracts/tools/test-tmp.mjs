// Pasta de trabalho dos testes: contracts/.test-tmp/<execução>/<caso>, fora do git. Nada aqui é apagado por código;
// cada execução usa uma subpasta nova e a limpeza fica com quem roda.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const base = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", ".test-tmp");
const run = `${new Date().toISOString().replace(/[^0-9]/g, "")}-${process.pid}`;
let n = 0;

export function freshDir(label) {
  const dir = path.join(base, run, `${label}-${String(++n).padStart(3, "0")}`);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}
