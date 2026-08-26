import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
test('shared URL fixtures are present and explicit', async () => {
  const fixture = JSON.parse(await readFile(new URL('../url-rule-cases.json', import.meta.url)));
  assert.equal(fixture.version, 1);
  assert.ok(fixture.cases.length >= 10);
  assert.ok(fixture.cases.some(item => item.allowed === false));
});
