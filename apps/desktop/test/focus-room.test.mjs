import test from 'node:test';
import assert from 'node:assert/strict';
test('focus room copy discloses simulated restriction protection', async () => {
  const source = await (await import('node:fs/promises')).readFile(new URL('../src/main.tsx', import.meta.url), 'utf8');
  assert.match(source, /simulated/i);
  assert.match(source, /duration/i);
});
test('theme tokens include dark and alternate built-in schemes', async () => {
  const source = await (await import('node:fs/promises')).readFile(new URL('../src/styles.css', import.meta.url), 'utf8');
  assert.match(source, /data-theme="mist"/);
  assert.match(source, /--surface/);
});
