import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
test('setup wizard has five required, explicit steps and optional music', async () => {
  const text = await readFile(new URL('../src/SetupWizard.tsx', import.meta.url), 'utf8');
  assert.match(text, /Browser connection/); assert.match(text, /Application checks/); assert.match(text, /Skip music/); assert.match(text, /simulated/i);
});
