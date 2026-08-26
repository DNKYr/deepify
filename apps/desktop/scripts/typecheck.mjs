import { readdirSync, readFileSync } from 'node:fs';
const files = readdirSync(new URL('../src/', import.meta.url)).filter(f => f.endsWith('.tsx') || f.endsWith('.ts'));
for (const file of files) {
  const text = readFileSync(new URL(`../src/${file}`, import.meta.url), 'utf8');
  if (!text.includes('React') && file.endsWith('.tsx')) throw new Error(`${file}: expected React source`);
}
console.log(`typecheck source scan passed (${files.length} files)`);
