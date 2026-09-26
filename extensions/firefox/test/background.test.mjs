import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const source = await readFile(new URL("../background.js", import.meta.url), "utf8");

function loadBackground({ tabs = [] } = {}) {
  const posted = [];
  const updates = [];
  const listeners = {};
  const port = {
    onMessage: { addListener(listener) { listeners.nativeMessage = listener; } },
    onDisconnect: { addListener(listener) { listeners.nativeDisconnect = listener; } },
    postMessage(message) { posted.push(message); },
  };
  const browser = {
    permissions: { contains: async () => true, onRemoved: { addListener(listener) { listeners.permissionsRemoved = listener; } } },
    runtime: {
      connectNative() { return port; },
      getURL(path) { return `moz-extension://deepify/${path}`; },
      getBrowserInfo: async () => ({ name: "Firefox" }),
      onMessage: { addListener(listener) { listeners.runtimeMessage = listener; } },
    },
    storage: { local: { async get() { return {}; }, async set() {} } },
    tabs: {
      async query() { return tabs; },
      async update(tabId, update) { updates.push({ tabId, ...update }); },
      onRemoved: { addListener(listener) { listeners.tabRemoved = listener; } },
      onUpdated: { addListener(listener) { listeners.tabUpdated = listener; } },
    },
    webRequest: { onBeforeRequest: { addListener(listener) { listeners.beforeRequest = listener; } } },
  };
  const context = vm.createContext({
    browser,
    btoa: (value) => Buffer.from(value, "binary").toString("base64"),
    Buffer,
    crypto: { randomUUID: () => "message-id", getRandomValues(bytes) { return bytes.fill(1); } },
    URL,
    setTimeout,
    clearTimeout,
  });
  vm.runInContext(`${source}\nglobalThis.__deepifyBackgroundTest = { destinationAllowed, validateMessage, onNativeMessage, state };`, context);
  return { api: context.__deepifyBackgroundTest, listeners, posted, updates, browser };
}

test("production background matcher follows every shared URL fixture", async () => {
  const fixture = JSON.parse(await readFile(new URL("../../../contracts/url-rule-cases.json", import.meta.url)));
  const { api } = loadBackground();
  for (const item of fixture.cases) {
    assert.equal(api.destinationAllowed(item.url, item.rules), item.allowed, item.url);
  }
});

test("production background rejects unsafe native messages", () => {
  const { api } = loadBackground();
  assert.equal(api.validateMessage({ version: 1, type: "status", message_id: "id" }), true);
  assert.equal(api.validateMessage({ version: 1, type: "status", message_id: "id", url: "https://private.invalid" }), false);
  assert.equal(api.validateMessage({ version: 1, type: "state", message_id: "id", health: "active", timer_state: "paused", remaining_seconds: 10 }), true);
  assert.equal(api.validateMessage({ version: 2, type: "status", message_id: "id" }), false);
  assert.equal(api.validateMessage({ version: 1, type: "unknown", message_id: "id" }), false);
  assert.equal(api.validateMessage({ version: 1, type: "status", message_id: "id", rules: [] }), false);
  assert.equal(api.validateMessage({ version: 1, type: "start_session", message_id: "id", session_id: "one", timer_state: "working", remaining_seconds: 60, rules: [{ host: 3, path: "/" }] }), false);
  assert.equal(api.validateMessage({ version: 1, type: "state", message_id: "id", health: "pretend", timer_state: "paused", remaining_seconds: 10 }), false);
});

test("paired production background restricts existing tabs and restores them on stop", async () => {
  const { api, posted, updates } = loadBackground({ tabs: [{ id: 7, url: "https://distracting.example/feed" }] });
  await api.onNativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  await api.onNativeMessage({
    version: 1,
    type: "start_session",
    message_id: "start",
    session_id: "session-1",
    timer_state: "working",
    remaining_seconds: 1200,
    rules: [{ host: "allowed.example", path: "/" }],
  });
  assert.deepEqual(updates, [{ tabId: 7, url: "moz-extension://deepify/blocked.html" }]);
  assert.equal(posted.some((message) => message.type === "blocked_attempt" && !Object.hasOwn(message, "url")), true);
  await api.onNativeMessage({ version: 1, type: "stop_session", message_id: "stop", session_id: "session-1" });
  assert.deepEqual(updates.at(-1), { tabId: 7, url: "https://distracting.example/feed" });
  assert.equal(api.state.session, null);
});

test("production background restores the first blocked destination after repeated navigation", async () => {
  const { api, listeners, updates } = loadBackground({ tabs: [{ id: 7, url: "https://first.example/path" }] });
  await api.onNativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  await api.onNativeMessage({
    version: 1, type: "start_session", message_id: "start", session_id: "session-1", timer_state: "working", remaining_seconds: 1200, rules: [],
  });
  listeners.beforeRequest({ tabId: 7, type: "main_frame", url: "https://second.example/path" });
  await api.onNativeMessage({ version: 1, type: "stop_session", message_id: "stop", session_id: "session-1" });
  assert.deepEqual(updates.at(-1), { tabId: 7, url: "https://first.example/path" });
});

test("production background re-blocks history navigation that bypasses webRequest", async () => {
  const { api, listeners, updates } = loadBackground({ tabs: [] });
  await api.onNativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  await api.onNativeMessage({
    version: 1, type: "start_session", message_id: "start", session_id: "session-1", timer_state: "paused", remaining_seconds: 1200, rules: [],
  });
  listeners.tabUpdated(7, { url: "https://history.example/path" }, { id: 7, url: "https://history.example/path" });
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(updates, [{ tabId: 7, url: "moz-extension://deepify/blocked.html" }]);
});


test("a failed restoration is reported and retained until the same session retries", async () => {
  const { api, posted, browser } = loadBackground({ tabs: [{ id: 7, url: "https://restore.example/original" }] });
  await api.onNativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  await api.onNativeMessage({ version: 1, type: "start_session", message_id: "start", session_id: "one", timer_state: "working", remaining_seconds: 60, rules: [] });
  const update = browser.tabs.update;
  browser.tabs.update = async () => { throw new Error("temporary restore failure"); };
  await api.onNativeMessage({ version: 1, type: "stop_session", message_id: "stop", session_id: "one" });
  assert.equal(posted.at(-1).accepted, false);
  assert.equal(api.state.originals.size, 1);
  await api.onNativeMessage({ version: 1, type: "status", message_id: "status" });
  assert.equal(posted.at(-1).health, "cleanup_required");
  assert.equal(posted.at(-1).session_id, "one");
  browser.tabs.update = update;
  await api.onNativeMessage({ version: 1, type: "stop_session", message_id: "retry", session_id: "one" });
  assert.equal(posted.at(-1).accepted, true);
  assert.equal(api.state.originals.size, 0);
  assert.equal(api.state.cleanupSessionId, null);
  assert.equal(JSON.stringify(posted).includes("restore.example"), false);
});

test("permission removal restores tabs and reports unhealthy rather than continuing protection", async () => {
  const { api, browser, listeners, posted, updates } = loadBackground({ tabs: [{ id: 7, url: "https://restore.example/original" }] });
  await api.onNativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  await api.onNativeMessage({ version: 1, type: "start_session", message_id: "start", session_id: "one", timer_state: "working", remaining_seconds: 60, rules: [] });
  browser.permissions.contains = async () => false;
  await listeners.permissionsRemoved();
  assert.equal(api.state.session, null);
  assert.equal(updates.at(-1).url, "https://restore.example/original");
  assert.equal(posted.at(-1).error_code, "permission_lost");
  await api.onNativeMessage({ version: 1, type: "status", message_id: "status" });
  assert.equal(posted.at(-1).health, "unhealthy");
});

test("closed blocked tabs are not recreated during restoration", async () => {
  const tabs = [{ id: 7, url: "https://closed.example/original" }];
  const { api, posted, updates } = loadBackground({ tabs });
  await api.onNativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  await api.onNativeMessage({ version: 1, type: "start_session", message_id: "start", session_id: "one", timer_state: "working", remaining_seconds: 60, rules: [] });
  tabs.length = 0;
  await api.onNativeMessage({ version: 1, type: "stop_session", message_id: "stop", session_id: "one" });
  assert.equal(updates.length, 1);
  assert.equal(posted.at(-1).accepted, true);
  assert.equal(posted.at(-1).restored_count, 0);
});

test("stop waits for an in-flight restriction before restoring the original tab", async () => {
  const { api, browser, listeners, updates, posted } = loadBackground({ tabs: [{ id: 7, url: "https://race.example/original" }] });
  await listeners.nativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  const update = browser.tabs.update;
  let release;
  browser.tabs.update = async (id, value) => {
    if (value.url.endsWith("blocked.html")) await new Promise((resolve) => { release = resolve; });
    await update(id, value);
  };
  const starting = listeners.nativeMessage({ version: 1, type: "start_session", message_id: "start", session_id: "one", timer_state: "working", remaining_seconds: 60, rules: [] });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(typeof release, "function");
  const stopping = listeners.nativeMessage({ version: 1, type: "stop_session", message_id: "stop", session_id: "one" });
  release();
  await Promise.all([starting, stopping]);
  assert.equal(updates.at(-1).url, "https://race.example/original");
  assert.equal(api.state.session, null);
  assert.equal(posted.at(-1).accepted, true);
});

test("tab API failure reports a safe integration error and removes policy", async () => {
  const { api, browser, listeners, posted } = loadBackground({ tabs: [{ id: 7, url: "https://failure.example/private" }] });
  await listeners.nativeMessage({ version: 1, type: "pair_result", message_id: "pair", request_id: "hello", accepted: true });
  browser.tabs.update = async () => { throw new Error("URL-bearing browser error must stay private"); };
  await listeners.nativeMessage({ version: 1, type: "start_session", message_id: "start", session_id: "one", timer_state: "working", remaining_seconds: 60, rules: [] });
  assert.equal(api.state.session, null);
  assert.equal(api.state.cleanupSessionId, "one");
  assert.equal(posted.at(-1).accepted, false);
  assert.equal(posted.some((message) => message.type === "integration_error"), true);
  assert.equal(JSON.stringify(posted).includes("URL-bearing"), false);
});
