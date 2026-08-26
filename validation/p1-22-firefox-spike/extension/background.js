/* global browser */

const NATIVE_HOST = "org.deepwork_focus.p1_22_spike";
const BLOCKED_PAGE = browser.runtime.getURL("blocked.html");
const state = {
  active: false,
  whitelist: [],
  blocked: new Map(),
  restoring: new Set(),
  port: null,
  lastError: null
};

function emit(type, payload = {}) {
  if (!state.port) return;
  try { state.port.postMessage({ type, ...payload }); }
  catch (error) { state.lastError = String(error); }
}

function isHttpUrl(url) { return /^https?:\/\//i.test(url || ""); }

function isLocalAddress(hostname) {
  const host = (hostname || "").toLowerCase().replace(/[.]$/, "");
  if (host === "localhost" || host === "::1" || host === "0:0:0:0:0:0:0:1") return true;
  const octets = host.split(".").map(Number);
  if (octets.length === 4 && octets.every(Number.isInteger)) {
    const [a, b] = octets;
    return a === 10 || a === 127 || (a === 172 && b >= 16 && b <= 31) ||
      (a === 192 && b === 168) || (a === 169 && b === 254);
  }
  return host.startsWith("fc") || host.startsWith("fd") || host.startsWith("fe80:");
}

function parseRule(rule) {
  let value = String(rule || "").trim().toLowerCase();
  value = value.replace(/^[a-z][a-z0-9+.-]*:\/\//, "").split("#", 1)[0];
  const slash = value.indexOf("/");
  const hostPart = slash === -1 ? value : value.slice(0, slash);
  const path = slash === -1 ? "/" : `/${value.slice(slash + 1)}`;
  const host = hostPart.startsWith("[")
    ? hostPart.slice(1, hostPart.indexOf("]"))
    : hostPart.split(":")[0];
  return { host: host.replace(/[.]$/, ""), path: path.replace(/\/+/g, "/") };
}

function matchesRule(url, rule) {
  let parsed;
  try { parsed = new URL(url); } catch (_) { return false; }
  const candidate = parseRule(rule);
  const host = parsed.hostname.toLowerCase().replace(/[.]$/, "");
  return (host === candidate.host || host.endsWith(`.${candidate.host}`)) &&
    parsed.pathname.startsWith(candidate.path);
}

function isAllowed(url) {
  if (!isHttpUrl(url)) return true;
  let parsed;
  try { parsed = new URL(url); } catch (_) { return true; }
  if (isLocalAddress(parsed.hostname)) return true;
  return state.whitelist.some((rule) => matchesRule(url, rule));
}

function blockedUrl(originalUrl, tabId) {
  return `${BLOCKED_PAGE}?tab=${encodeURIComponent(tabId)}&url=${encodeURIComponent(originalUrl)}`;
}

async function describeTabs() {
  const tabs = await browser.tabs.query({});
  return tabs.map((tab) => ({
    id: tab.id, url: tab.url || "", title: tab.title || "", windowId: tab.windowId,
    incognito: Boolean(tab.incognito), cookieStoreId: tab.cookieStoreId || null
  }));
}

async function getProfilePairingToken() {
  const stored = await browser.storage.local.get("profilePairingToken");
  if (stored.profilePairingToken) return stored.profilePairingToken;
  const token = `${browser.runtime.id}:${crypto.randomUUID()}`;
  await browser.storage.local.set({ profilePairingToken: token });
  return token;
}

async function restrictTab(tab) {
  if (!state.active || tab.incognito || !tab.url || isAllowed(tab.url) || tab.url.startsWith(BLOCKED_PAGE)) return false;
  if (!state.blocked.has(tab.id)) state.blocked.set(tab.id, tab.url);
  await browser.tabs.update(tab.id, { url: blockedUrl(state.blocked.get(tab.id), tab.id) });
  emit("tab_blocked", {
    tabId: tab.id,
    originalUrl: state.blocked.get(tab.id),
    cookieStoreId: tab.cookieStoreId || null,
    incognito: Boolean(tab.incognito)
  });
  return true;
}

async function restrictExistingTabs() {
  const tabs = await browser.tabs.query({});
  for (const tab of tabs) await restrictTab(tab);
}

async function restoreTabs(reason) {
  const entries = [...state.blocked.entries()];
  state.blocked.clear();
  state.active = false;
  for (const [tabId, originalUrl] of entries) {
    try {
      state.restoring.add(tabId);
      await browser.tabs.update(tabId, { url: originalUrl });
      emit("tab_restored", { tabId, originalUrl, reason });
    } catch (error) {
      emit("tab_restore_failed", { tabId, originalUrl, reason, error: String(error) });
    } finally { state.restoring.delete(tabId); }
  }
  emit("state", { active: false, blockedCount: 0, reason });
}

async function startSession(message) {
  state.whitelist = Array.isArray(message.whitelist) ? message.whitelist : [];
  state.active = true;
  emit("state", { active: true, whitelist: state.whitelist });
  await restrictExistingTabs();
  emit("start_complete", { blockedCount: state.blocked.size });
}

async function connectNative() {
  try {
    state.port = browser.runtime.connectNative(NATIVE_HOST);
    state.port.onMessage.addListener(async (message) => {
      emit("native_message", { messageType: message.type || null });
      if (message.type === "start") await startSession(message);
      if (message.type === "stop") await restoreTabs(message.reason || "desktop_stop");
      if (message.type === "status") emit("status", { active: state.active, blockedCount: state.blocked.size });
      if (message.type === "container_probe") {
        try {
          const existing = await browser.contextualIdentities.query({});
          const identity = existing[0] || await browser.contextualIdentities.create({
            name: "P1-22 test container", color: "blue", icon: "fingerprint"
          });
          const tab = await browser.tabs.create({
            url: "https://example.org/p1-22-container",
            active: false,
            cookieStoreId: identity.cookieStoreId
          });
          emit("container_result", {
            cookieStoreId: identity.cookieStoreId,
            tabId: tab.id,
            tabCookieStoreId: tab.cookieStoreId || null
          });
        } catch (error) {
          emit("container_result", { error: String(error) });
        }
      }
    });
    state.port.onDisconnect.addListener(async () => {
      const error = browser.runtime.lastError;
      state.lastError = error ? error.message : null;
      console.error("P1-22 native disconnect", state.lastError);
      state.port = null;
      if (state.active) await restoreTabs("native_disconnect");
    });
    emit("hello", {
      extensionId: browser.runtime.id,
      profilePairingToken: await getProfilePairingToken(),
      browser: navigator.userAgent,
      privateWindows: false,
      tabs: await describeTabs()
    });
  } catch (error) {
    state.lastError = String(error);
    console.error("P1-22 native connect failed", state.lastError);
  }
}

browser.tabs.onUpdated.addListener(async (tabId, changeInfo, tab) => {
  if (!state.active || state.restoring.has(tabId) || !changeInfo.url) return;
  await restrictTab(tab);
});

browser.tabs.onRemoved.addListener((tabId) => state.blocked.delete(tabId));

browser.runtime.onMessage.addListener(async (message) => {
  if (message.type === "restore") {
    const originalUrl = state.blocked.get(message.tabId);
    if (originalUrl) {
      state.blocked.delete(message.tabId);
      state.restoring.add(message.tabId);
      await browser.tabs.update(message.tabId, { url: originalUrl });
      state.restoring.delete(message.tabId);
    }
  }
  if (message.type === "debug_state") return { active: state.active, blocked: [...state.blocked.entries()] };
  return undefined;
});

connectNative();

// The production desktop app keeps a one-minute health check. The spike uses
// one second so a short disposable run proves that the persistent connection
// carries liveness traffic in both directions.
setInterval(() => {
  if (state.port) emit("heartbeat", { at: Date.now() });
}, 1000);
