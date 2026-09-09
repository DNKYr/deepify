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
  const octets = normalized.split(".");
  const local172 = octets[0] === "172" && Number(octets[1]) >= 16 && Number(octets[1]) <= 31;
  const mappedV4 = normalized.match(/^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/i);
  const mappedLocal = mappedV4 && (() => {
    const first = Number.parseInt(mappedV4[1], 16);
    const second = Number.parseInt(mappedV4[2], 16);
    const mapped = `${first >> 8}.${first & 255}.${second >> 8}.${second & 255}`;
    const mappedOctets = mapped.split(".");
    return mapped.startsWith("127.") || mapped.startsWith("10.") || mapped.startsWith("192.168.") ||
      mapped.startsWith("169.254.") || (mappedOctets[0] === "172" && Number(mappedOctets[1]) >= 16 && Number(mappedOctets[1]) <= 31);
  })();
  return normalized === "localhost" || normalized.startsWith("127.") || normalized === "::1" ||
    normalized.startsWith("10.") || normalized.startsWith("192.168.") || normalized.startsWith("169.254.") || local172 || mappedLocal ||
    /^(fc|fd|fe[89ab])/i.test(normalized);
}

function destinationAllowed(value, rules) {
  let url;
  try { url = new URL(value); } catch { return true; }
  if (url.protocol !== "http:" && url.protocol !== "https:") return true;
  const host = url.hostname.toLowerCase().replace(/^\[/, "").replace(/\]$/, "");
  if (isLocalHost(host)) return true;
  return rules.some((rule) => {
    const normalized = typeof rule === "string"
      ? rule.toLowerCase().replace(/^https?:\/\//, "")
      : `${rule.host ?? ""}${rule.path ?? "/"}`.toLowerCase();
    const [rawHost, ...pathParts] = normalized.split("/");
    const ruleHost = rawHost.replace(/^\[/, "").replace(/\]$/, "").replace(/:\d+$/, "");
    const path = `/${pathParts.join("/")}`.replace(/\/$/, "") || "/";
    return ruleHost.length > 0 && (host === ruleHost || host.endsWith(`.${ruleHost}`)) && url.pathname.startsWith(path);
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
      message.version !== PROTOCOL_VERSION || !safeString(message.type) || !safeString(message.message_id)) return false;
  if (["pair_result", "heartbeat_ack", "start_result", "stop_result"].includes(message.type) && !safeString(message.request_id)) return false;
  return !["url", "destination", "title", "query"].some((key) => Object.hasOwn(message, key));
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
  state.session = null;
  state.originals.clear();
  state.blocked.clear();
  let restored = 0;
  for (const [tabId, url] of entries) {
    try { await browser.tabs.update(tabId, { url }); restored += 1; } catch { /* closed tabs remain closed */ }
  }
  return restored;
}

async function restrictTab(tabId, url) {
  if (!state.session || destinationAllowed(url, state.session.rules) || state.blocked.get(tabId) === url) return false;
  if (!state.originals.has(tabId)) state.originals.set(tabId, url);
  state.blocked.set(tabId, url);
  send("blocked_attempt", { session_id: state.session.id });
  await browser.tabs.update(tabId, { url: BLOCKED_PAGE });
  return true;
}

async function startSession(message) {
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
  if (state.session && state.session.id !== message.session_id) {
    send("stop_result", { session_id: message.session_id, accepted: false, error_code: "session_mismatch" }, message.message_id);
    return;
  }
  const restored = await restoreAll();
  send("restore_complete", { session_id: message.session_id, restored_count: restored });
  send("stop_result", { session_id: message.session_id, accepted: true, restored_count: restored }, message.message_id);
}

function respondStatus(message) {
  send("state", {
    health: state.session ? "active" : state.connection === "paired" ? "healthy_idle" : state.connection === "pending_pair" ? "pending_pair" : "unhealthy",
    session_id: state.session?.id,
    timer_state: state.session?.timerState ?? "inactive",
    remaining_seconds: state.session?.remainingSeconds ?? 0,
  }, message.message_id);
}

async function onNativeMessage(message) {
  if (!validateMessage(message)) return;
  if (message.type === "pair_result") state.connection = message.accepted ? "paired" : "rejected";
  if (message.type === "heartbeat") send("heartbeat_ack", {}, message.message_id);
  if (message.type === "status") respondStatus(message);
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
    state.port.onMessage.addListener(onNativeMessage);
    state.port.onDisconnect.addListener(async () => {
      const wasActive = Boolean(state.session);
      state.port = null;
      state.connection = "disconnected";
      if (wasActive) await restoreAll();
      scheduleReconnect();
    });
    state.reconnectDelay = 1000;
    void hello();
  } catch { scheduleReconnect(); }
}

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
  if (url) void restrictTab(tabId, url);
});
browser.tabs.onRemoved.addListener((tabId) => { state.originals.delete(tabId); state.blocked.delete(tabId); });
browser.runtime.onMessage.addListener((message, sender) => {
  if (message?.type !== "deepify-blocked-state" || sender.tab?.id === undefined) return undefined;
  const original = state.originals.get(sender.tab.id);
  return Promise.resolve({ destination: original ? sanitizedDestination(original) : "website", timerState: state.session?.timerState ?? "inactive", remainingSeconds: state.session?.remainingSeconds ?? 0 });
});
connect();
