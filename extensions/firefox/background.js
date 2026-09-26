/* global browser, crypto */
// The extension owns in-memory tab restoration. URLs never cross native messaging.
const PROTOCOL_VERSION = 1;
const EXTENSION_ID = "focus@deepify.local";
const NATIVE_HOST = "com.deepify.browser";
const BLOCKED_PAGE = browser.runtime.getURL("blocked.html");
const state = {
  port: null,
  connection: "disconnected", // disconnected | pending_pair | paired | rejected
  token: null,
  browserKind: "firefox",
  profileLabel: "Firefox profile",
  session: null,
  cleanupSessionId: null,
  permissionsHealthy: true,
  originals: new Map(),
  blocked: new Map(),
  reconnectDelay: 1000,
  reconnectTimer: null,
};

const messageId = () => crypto.randomUUID();
const safeString = (value, maximum = 128) =>
  typeof value === "string" && value.length > 0 && value.length <= maximum && !/[\x00-\x1f\x7f]/.test(value);
const b64 = (bytes) => btoa(String.fromCharCode(...bytes));
const allowedCapabilities = ["top_level_web_request", "tab_restore", "container_tabs"];

function isLocalHost(host) {
  const normalized = host.toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
  if (normalized === "localhost" || normalized === "::1") return true;
  const octets = normalized.split(".");
  if (octets.length === 4 && octets.every((part) => /^\d+$/.test(part) && Number(part) <= 255)) {
    const [a, b] = octets.map(Number);
    return a === 127 || a === 10 || (a === 192 && b === 168) ||
      (a === 169 && b === 254) || (a === 172 && b >= 16 && b <= 31);
  }
  // URL.hostname has already parsed and canonicalized IPv6. A DNS name such
  // as fc-news.example or 10.example must never inherit local-IP exemptions.
  if (!normalized.includes(":")) return false;
  const mapped = normalized.match(/^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/i);
  if (mapped) {
    const first = Number.parseInt(mapped[1], 16);
    const second = Number.parseInt(mapped[2], 16);
    return isLocalHost(`${first >> 8}.${first & 255}.${second >> 8}.${second & 255}`);
  }
  return /^(fc|fd|fe[89ab])/i.test(normalized);
}

function destinationAllowed(value, rules) {
  let url;
  try { url = new URL(value); } catch { return !/^http/i.test(value); }
  if (url.protocol !== "http:" && url.protocol !== "https:") return true;
  const host = url.hostname.toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
  if (isLocalHost(host)) return true;
  return rules.some((rule) => {
    let ruleHost, rulePath;
    if (typeof rule === "string") {
      try {
        const parsed = new URL(/^https?:\/\//.test(rule) ? rule : `http://${rule}`);
        ruleHost = parsed.hostname;
        rulePath = parsed.pathname;
      } catch { return false; }
    } else {
      ruleHost = rule.host;
      rulePath = rule.path;
    }
    ruleHost = ruleHost.toLowerCase().replace(/^\[/, "").replace(/\]$/, "").replace(/\.$/, "");
    return ruleHost.length > 0 && (host === ruleHost || host.endsWith(`.${ruleHost}`)) && url.pathname.startsWith(rulePath);
  });
}

function sanitizedDestination(value) {
  try {
    const url = new URL(value);
    return `${url.hostname}${url.pathname || "/"}`.slice(0, 2048);
  } catch { return "website"; }
}

function validateMessage(message) {
  if (!message || typeof message !== "object" || Array.isArray(message) ||
      message.version !== PROTOCOL_VERSION || !safeString(message.message_id)) return false;
  const shapes = {
    pair_result: ["accepted", "error_code"], heartbeat: [], status: [],
    start_session: ["session_id", "timer_state", "remaining_seconds", "rules"],
    stop_session: ["session_id"], state: ["health", "session_id", "timer_state", "remaining_seconds"],
  };
  if (!Object.hasOwn(shapes, message.type)) return false;
  const allowed = new Set(["version", "type", "message_id", "request_id", ...shapes[message.type]]);
  if (Object.keys(message).some((key) => !allowed.has(key))) return false;
  if (message.request_id != null && !safeString(message.request_id)) return false;
  if (message.type === "pair_result") return safeString(message.request_id) &&
    typeof message.accepted === "boolean" && (message.error_code === undefined ||
      ["unpaired", "rejected", "invalid_request", "permission_lost", "session_mismatch", "restore_failed", "internal"].includes(message.error_code));
  if (["start_session", "stop_session"].includes(message.type) && !safeString(message.session_id)) return false;
  if (["start_session", "state"].includes(message.type)) {
    if (!["working", "paused", "inactive"].includes(message.timer_state) ||
        !Number.isInteger(message.remaining_seconds) || message.remaining_seconds < 0 || message.remaining_seconds > 86400) return false;
  }
  if (message.type === "state" && (!["pending_pair", "healthy_idle", "active", "unhealthy", "cleanup_required"].includes(message.health) ||
      (message.session_id != null && !safeString(message.session_id)))) return false;
  if (message.type === "start_session" && (!Array.isArray(message.rules) || message.rules.length > 1024 ||
      !message.rules.every((rule) => rule && !Array.isArray(rule) && Object.keys(rule).length === 2 &&
        safeString(rule.host, 253) && safeString(rule.path, 2048)))) return false;
  return true;
}

function send(type, payload = {}, requestId) {
  if (!state.port) return false;
  const message = { version: PROTOCOL_VERSION, type, message_id: messageId(), ...payload };
  if (requestId) message.request_id = requestId;
  try { state.port.postMessage(message); return true; } catch { return false; }
}

async function ensureToken() {
  const stored = await browser.storage.local.get("pairingToken");
  if (safeString(stored.pairingToken, 128) && stored.pairingToken.length >= 43) return stored.pairingToken;
  const token = b64(crypto.getRandomValues(new Uint8Array(32)));
  await browser.storage.local.set({ pairingToken: token });
  return token;
}

async function hello() {
  state.token = await ensureToken();
  const info = await browser.runtime.getBrowserInfo?.().catch(() => null);
  state.browserKind = /zen/i.test(info?.name ?? "") ? "zen" : "firefox";
  state.profileLabel = `${state.browserKind === "zen" ? "Zen" : "Firefox"} profile`;
  send("hello", {
    token: state.token,
    extension_id: EXTENSION_ID,
    browser_kind: state.browserKind,
    profile_label: state.profileLabel,
    capabilities: allowedCapabilities,
  });
}

function scheduleReconnect() {
  if (state.reconnectTimer) return;
  const delay = state.reconnectDelay;
  state.reconnectDelay = Math.min(state.reconnectDelay * 2, 30000);
  state.reconnectTimer = setTimeout(() => { state.reconnectTimer = null; connect(); }, delay);
}

async function restoreAll() {
  const entries = [...state.originals.entries()];
  state.cleanupSessionId ??= state.session?.id ?? null;
  state.session = null;
  if (!entries.length) { state.cleanupSessionId = null; return { restored: 0, failed: 0 }; }
  let liveTabs;
  try { liveTabs = new Set((await browser.tabs.query({})).map((tab) => tab.id)); }
  catch { return { restored: 0, failed: entries.length }; }
  let restored = 0;
  for (const [tabId, url] of entries) {
    let resolved = !liveTabs.has(tabId);
    if (!resolved) {
      try { await browser.tabs.update(tabId, { url }); restored += 1; resolved = true; }
      catch {
        // A close racing restoration is harmless; other failures retain the URL
        // in memory for retry instead of claiming successful cleanup.
        try { resolved = !(await browser.tabs.query({})).some((tab) => tab.id === tabId); }
        catch { resolved = false; }
      }
    }
    if (resolved) { state.originals.delete(tabId); state.blocked.delete(tabId); }
  }
  if (!state.originals.size) state.cleanupSessionId = null;
  return { restored, failed: state.originals.size };
}

async function permissionsAvailable() {
  try {
    state.permissionsHealthy = await browser.permissions.contains({
      permissions: ["storage", "tabs", "webRequest", "webRequestBlocking", "nativeMessaging"],
      origins: ["<all_urls>"],
    });
  } catch { state.permissionsHealthy = false; }
  return state.permissionsHealthy;
}

async function permissionFailure() {
  const sessionId = state.session?.id ?? state.cleanupSessionId;
  await restoreAll();
  send("integration_error", { error_code: "permission_lost", ...(sessionId ? { session_id: sessionId } : {}) });
}

async function restrictTab(tabId, url) {
  if (!state.session || destinationAllowed(url, state.session.rules) || state.blocked.get(tabId) === url) return false;
  if (!state.originals.has(tabId)) state.originals.set(tabId, url);
  state.blocked.set(tabId, url);
  send("blocked_attempt", { session_id: state.session.id });
  try { await browser.tabs.update(tabId, { url: BLOCKED_PAGE }); }
  catch (error) {
    if ((await browser.tabs.query({})).some((tab) => tab.id === tabId)) throw error;
    state.originals.delete(tabId);
    state.blocked.delete(tabId);
    return false;
  }
  return true;
}

// Preserve lifecycle order across asynchronous browser APIs. Stop/disconnect
// restoration must run after an already-requested tab replacement has settled.
let operations = Promise.resolve();
function enqueue(operation, message) {
  operations = operations.then(operation).catch(async () => {
    const sessionId = state.session?.id ?? state.cleanupSessionId ?? message?.session_id;
    await restoreAll();
    send("integration_error", { error_code: "internal", ...(sessionId ? { session_id: sessionId } : {}) });
    if (message?.type === "start_session") {
      send("start_result", { session_id: message.session_id, accepted: false, error_code: "internal" }, message.message_id);
    }
  });
  return operations;
}

async function startSession(message) {
  if (!(await permissionsAvailable())) {
    await permissionFailure();
    send("start_result", { session_id: message.session_id, accepted: false, error_code: "permission_lost" }, message.message_id);
    return;
  }
  if (state.cleanupSessionId || (state.session && state.session.id !== message.session_id)) {
    send("start_result", { session_id: message.session_id, accepted: false, error_code: "session_mismatch" }, message.message_id);
    return;
  }
  if (state.connection !== "paired" || !Array.isArray(message.rules) || !safeString(message.session_id)) {
    send("start_result", { session_id: message.session_id || "unknown", accepted: false, error_code: "unpaired" }, message.message_id);
    return;
  }
  if (state.session?.id === message.session_id) {
    send("start_result", { session_id: message.session_id, accepted: true }, message.message_id);
    return;
  }
  state.session = { id: message.session_id, rules: message.rules, timerState: message.timer_state, remainingSeconds: message.remaining_seconds };
  let blockedCount = 0;
  for (const tab of await browser.tabs.query({})) {
    if (tab.id !== undefined && tab.url && await restrictTab(tab.id, tab.url)) blockedCount += 1;
  }
  send("start_result", { session_id: message.session_id, accepted: true, blocked_count: blockedCount }, message.message_id);
}

async function stopSession(message) {
  if ((state.session?.id ?? state.cleanupSessionId) && (state.session?.id ?? state.cleanupSessionId) !== message.session_id) {
    send("stop_result", { session_id: message.session_id, accepted: false, error_code: "session_mismatch" }, message.message_id);
    return;
  }
  const result = await restoreAll();
  if (result.failed) {
    send("restore_error", { session_id: message.session_id, error_code: "restore_failed" });
    send("stop_result", { session_id: message.session_id, accepted: false, error_code: "restore_failed" }, message.message_id);
  } else {
    send("restore_complete", { session_id: message.session_id, restored_count: result.restored });
    send("stop_result", { session_id: message.session_id, accepted: true, restored_count: result.restored }, message.message_id);
  }
}

async function respondStatus(message) {
  if (!(await permissionsAvailable())) await permissionFailure();
  send("state", {
    health: state.cleanupSessionId ? "cleanup_required" : !state.permissionsHealthy ? "unhealthy" : state.session ? "active" : state.connection === "paired" ? "healthy_idle" : state.connection === "pending_pair" ? "pending_pair" : "unhealthy",
    session_id: state.session?.id ?? state.cleanupSessionId ?? undefined,
    timer_state: state.session?.timerState ?? "inactive",
    remaining_seconds: state.session?.remainingSeconds ?? 0,
  }, message.message_id);
}

async function onNativeMessage(message) {
  if (!validateMessage(message)) return;
  if (message.type === "pair_result") state.connection = message.accepted ? "paired" : "rejected";
  if (message.type === "heartbeat") send("heartbeat_ack", {}, message.message_id);
  if (message.type === "status") await respondStatus(message);
  if (message.type === "start_session") await startSession(message);
  if (message.type === "stop_session") await stopSession(message);
  if (message.type === "state" && state.session && message.session_id === state.session.id) {
    state.session.timerState = message.timer_state;
    state.session.remainingSeconds = message.remaining_seconds;
  }
}

function connect() {
  if (state.port) return;
  try {
    state.port = browser.runtime.connectNative(NATIVE_HOST);
    state.port.onMessage.addListener((message) => enqueue(() => onNativeMessage(message), message));
    state.port.onDisconnect.addListener(async () => {
      state.port = null;
      state.connection = "disconnected";
      await enqueue(() => restoreAll());
      scheduleReconnect();
    });
    state.reconnectDelay = 1000;
    void hello();
  } catch { scheduleReconnect(); }
}

browser.permissions.onRemoved.addListener(() => enqueue(async () => {
  if (!(await permissionsAvailable())) await permissionFailure();
}));

browser.webRequest.onBeforeRequest.addListener((details) => {
  if (!state.session || details.type !== "main_frame" || destinationAllowed(details.url, state.session.rules)) return {};
  if (!state.originals.has(details.tabId)) state.originals.set(details.tabId, details.url);
  state.blocked.set(details.tabId, details.url);
  send("blocked_attempt", { session_id: state.session.id });
  return { redirectUrl: BLOCKED_PAGE };
}, { urls: ["<all_urls>"], types: ["main_frame"] }, ["blocking"]);

// Firefox may satisfy Back/reload from session history without emitting the
// blocking webRequest event. The tabs event is a backstop using an existing
// permission; `originals` still retains the first destination for restoration.
browser.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  const url = changeInfo.url ?? tab.url;
  if (url) void enqueue(() => restrictTab(tabId, url));
});
browser.tabs.onRemoved.addListener((tabId) => { state.originals.delete(tabId); state.blocked.delete(tabId); });
browser.runtime.onMessage.addListener((message, sender) => {
  if (message?.type !== "deepify-blocked-state" || sender.tab?.id === undefined) return undefined;
  const original = state.originals.get(sender.tab.id);
  return Promise.resolve({ destination: original ? sanitizedDestination(original) : "website", timerState: state.session?.timerState ?? "inactive", remainingSeconds: state.session?.remainingSeconds ?? 0 });
});
connect();
