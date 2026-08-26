import { readdirSync, readFileSync } from 'node:fs';
const files = readdirSync(new URL('../src/', import.meta.url));
for (const file of files) if (file.endsWith('.tsx') || file.endsWith('.ts')) {
  const text = readFileSync(new URL(`../src/${file}`, import.meta.url), 'utf8');
  if (/console\.log/.test(text)) throw new Error(`${file}: no console.log in UI source`);
}
console.log('lint source scan passed');
