// Roda todos os testes de e2e/tests em sequência. Uso: `node run.mjs [filtro]`.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = path.join(path.dirname(fileURLToPath(import.meta.url)), 'tests');
const filter = process.argv[2] ?? '';
const files = fs.readdirSync(dir).filter((f) => f.endsWith('.test.mjs') && f.includes(filter)).sort();

let failed = 0;
for (const file of files) {
  console.log(`\n=== ${file}`);
  const result = spawnSync(process.execPath, [path.join(dir, file)], { stdio: 'inherit' });
  if (result.status !== 0) failed++;
}
console.log(`\n${files.length - failed}/${files.length} arquivos de teste passaram`);
process.exit(failed ? 1 : 0);
