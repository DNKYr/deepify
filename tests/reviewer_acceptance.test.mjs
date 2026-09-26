import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8');
const json = async (path) => JSON.parse(await read(path));

test('P2-01 uses genuine immutable pins and permanent identifiers', async () => {
  const lock = await json('flake.lock');
  const pinned = lock.nodes.nixpkgs.locked;
  assert.match(pinned.rev, /^[0-9a-f]{40}$/, 'nixpkgs revision must be an immutable commit');
  assert.ok(pinned.lastModified > 0, 'nixpkgs lock must have a real timestamp');
  assert.match(pinned.narHash, /^sha256-[A-Za-z0-9+/]{43}=$/, 'nixpkgs lock must have a real SRI hash');
  assert.notEqual(pinned.narHash, 'sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=');

  const identifiers = await read('IDENTIFIERS.md');
  const tauri = await json('apps/desktop/src-tauri/tauri.conf.json');
  const extension = await json('extensions/firefox/manifest.json');
  const host = await json('extensions/firefox/native-manifest.json');
  assert.equal(tauri.identifier, 'com.deepify.desktop');
  assert.equal(host.name, 'com.deepify.browser');
  assert.equal(extension.browser_specific_settings.gecko.id, 'focus@deepify.local');
  for (const value of [tauri.identifier, host.name, extension.browser_specific_settings.gecko.id]) {
    assert.match(identifiers, new RegExp(value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  }
});

test('P2-02 quality commands execute the real toolchain', async () => {
  const root = await json('package.json');
  const desktop = await json('apps/desktop/package.json');
  assert.match(root.scripts.dev, /tauri\s+dev/);
  assert.match(desktop.scripts.test, /vitest\s+run/);
  assert.match(desktop.scripts.typecheck, /tsc\s+--noEmit/);
  assert.match(desktop.scripts.lint, /eslint/);
  assert.match(desktop.scripts['format:check'], /prettier\s+--check/);
  for (const dependency of ['@testing-library/react', '@testing-library/user-event', 'jsdom', 'prettier', 'vitest']) {
    assert.ok(desktop.devDependencies[dependency], `${dependency} must be pinned`);
  }
});

test('P2-03 persistence is embedded rusqlite, parameterized, and backend-owned', async () => {
  const manifest = await read('apps/desktop/src-tauri/Cargo.toml');
  const storage = await read('apps/desktop/src-tauri/src/storage.rs');
  assert.match(manifest, /rusqlite\s*=/);
  assert.doesNotMatch(storage, /process::Command|Command::new\(["']sqlite3/);
  assert.match(storage, /Connection/);
  assert.match(storage, /params!/);
  const migration = await read('apps/desktop/src-tauri/migrations/001_initial.sql');
  for (const table of ['sessions', 'whitelist_entries', 'music_sources', 'music_tracks', 'queue_entries', 'settings', 'integration_state']) {
    assert.match(migration, new RegExp(`CREATE TABLE IF NOT EXISTS ${table}`));
  }
  assert.match(migration, /CREATE UNIQUE INDEX IF NOT EXISTS one_active_session/);
});

test('P2-04 through P2-06 register backend commands and keep React presentation-only', async () => {
  const main = await read('apps/desktop/src-tauri/src/main.rs');
  const domain = await read('apps/desktop/src-tauri/src/lib.rs');
  const ui = await Promise.all(['apps/desktop/src/main.tsx', 'apps/desktop/src/App.tsx', 'apps/desktop/src/backend.ts'].map(read)).then((parts) => parts.join('\n'));
  assert.match(main, /invoke_handler\s*\(/);
  for (const command of ['app_snapshot', 'session_start', 'session_pause', 'session_resume', 'session_end']) {
    assert.match(main, new RegExp(command));
  }
  assert.match(main, /tauri_plugin_single_instance/);
  assert.match(main, /tauri_plugin_notification/);
  for (const boundary of ['BrowserRestrictionAdapter', 'ApplicationRestrictionAdapter', 'DoNotDisturbAdapter', 'RestrictionCoordinator']) {
    assert.match(domain, new RegExp(boundary), `missing restriction boundary: ${boundary}`);
  }
  assert.doesNotMatch(main, /simulate_blocked_app/);
  assert.match(main, /resolve_blocked_apps/);
  assert.doesNotMatch(ui, /localStorage/);
  assert.doesNotMatch(ui, /setInterval|setTimeout/);
  assert.match(ui, /sessionSummary|summary/i);
});

test('P2-06 through P2-10 expose every prototype screen and production audio boundary', async () => {
  const allUi = await Promise.all([
    'apps/desktop/src/main.tsx',
    'apps/desktop/src/App.tsx',
    'apps/desktop/src/SetupWizard.tsx',
    'apps/desktop/src/styles.css',
  ].map(read)).then(parts => parts.join('\n'));
  for (const label of [
    'Focus Room', 'Sound Library', 'Whitelist', 'Session History', 'Settings',
    'Website protection', 'Unidentified app — allowed in MVP',
    'Restrictions remain active while paused', 'Blocked attempts', 'Skip music',
    'Default duration', 'Rerun integration health checks',
  ]) assert.match(allUi, new RegExp(label, 'i'), `missing required UI state: ${label}`);

  const cargo = await read('apps/desktop/src-tauri/Cargo.toml');
  assert.match(cargo, /(rodio|symphonia|kira)\s*=/, 'a production MP3 decoder/player must be selected');
  assert.match(cargo, /id3\s*=/, 'ID3 metadata library must be selected');
  assert.match(cargo, /notify\s*=/, 'live folder watcher must be selected');
  const runtime = await read('apps/desktop/src-tauri/src/main.rs');
  for (const command of ['audio_toggle', 'audio_previous', 'audio_next', 'audio_set_volume', 'repair_integrations']) {
    assert.match(runtime, new RegExp(command), `missing production audio command: ${command}`);
  }
  const architecture = await read('ARCHITECTURE.md');
  assert.match(architecture, /Phase 2 audio adapter decision/i);
});

test('P2-05 through P2-10 close health, simulation, history, accessibility, and diagnostics gaps', async () => {
  const domain = await read('apps/desktop/src-tauri/src/lib.rs');
  const runtime = await read('apps/desktop/src-tauri/src/main.rs');
  const ui = await read('apps/desktop/src/App.tsx');
  for (const pattern of [
    /pub latency: Duration/,
    /thread::sleep\(self\.latency\)/,
    /configurable_mock_latency_applies_to_each_adapter_step/,
  ]) assert.match(domain, pattern);
  for (const command of [
    'test_whitelist',
    'repair_integrations',
    'audio_retry_output',
  ]) assert.match(runtime, new RegExp(command));
  assert.match(runtime, /diagnostic_id/);
  assert.match(runtime, /restriction\.browser\.healthy\(\)/);
  for (const pattern of [
    /This week/,
    /Test current configuration/,
    /This test is local and is not saved/,
    /Local diagnostic ID/,
    /aria-describedby="end-description"/,
    /event\.key === "Escape"/,
  ]) assert.match(ui, pattern);
});

test('P2-11 does not claim completion while required Phase 2 gates remain open', async () => {
  const handoff = await read('PHASE2_HANDOFF.md');
  assert.doesNotMatch(handoff, /Tauri and rusqlite dependency installation.*remain open/i);
  assert.doesNotMatch(handoff, /production decoder.*remain open/i);
  assert.doesNotMatch(handoff, /backend Tauri command registration.*remain open/i);
  const plan = await read('PHASE2_IMPLEMENTATION_PLAN.md');
  for (let item = 1; item <= 11; item += 1) {
    const section = plan.match(new RegExp(`## P2-${String(item).padStart(2, '0')}[\\s\\S]*?(?=\\n## P2-|\\n## 4\\.)`))?.[0];
    assert.ok(section, `missing P2-${String(item).padStart(2, '0')} section`);
    assert.match(section, /\*\*Status:\*\* Complete/, `P2-${String(item).padStart(2, '0')} is not complete`);
  }
});
