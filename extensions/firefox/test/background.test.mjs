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
  return { api: context.__deepifyBackgroundTest, listeners, posted, updates };
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
