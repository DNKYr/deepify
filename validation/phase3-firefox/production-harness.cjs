#!/usr/bin/env node
/*
 * Disposable Phase 3 Firefox/Zen integration harness.
 * It exercises the production extension and Rust stdio native host, with a
 * minimal Unix-socket desktop peer. It never opens a normal browser profile.
 */
const assert = require("node:assert/strict");
const http = require("node:http");
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { spawn, execFileSync } = require("node:child_process");

const root = path.resolve(__dirname, "../..");
const firefox = process.env.FIREFOX_BIN || "firefox";
const browserName = path.basename(firefox).toLowerCase().includes("zen") ? "Zen" : "Firefox";
const privateWindow = process.env.DEEPIFY_PRIVATE_WINDOW === "1";
const signedInstall = process.env.DEEPIFY_SIGNED_INSTALL === "1";
const lifecycleMode = process.env.DEEPIFY_LIFECYCLE_MODE;
const platformMode = lifecycleMode?.startsWith("platform-");
const extension = process.env.DEEPIFY_EXTENSION_PATH || path.join(root, "extensions/firefox");
const nativeHost = process.env.DEEPIFY_NATIVE_HOST || path.join(root, "target/debug/deepify-browser-native-host");
const lifecycleHarness = path.join(root, platformMode ? "target/debug/examples/phase4_platform" : "target/debug/examples/phase3_lifecycle");
const extensionId = "focus@deepify.local";
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
const unique = () => `test-${Date.now()}-${Math.random().toString(16).slice(2)}`;

class Marionette {
  constructor(socket) {
    this.socket = socket;
    this.buffer = Buffer.alloc(0);
    this.waiting = new Map();
    this.nextId = 1;
    this.greeting = null;
    socket.on("data", (chunk) => this.receive(chunk));
    socket.on("error", (error) => this.fail(error));
    socket.on("close", () => this.fail(new Error("Marionette closed")));
  }
  fail(error) { for (const { reject } of this.waiting.values()) reject(error); this.waiting.clear(); }
  receive(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const separator = this.buffer.indexOf(0x3a);
      if (separator < 0) return;
      const length = Number(this.buffer.subarray(0, separator).toString());
      if (!Number.isInteger(length) || this.buffer.length < separator + length + 1) return;
      const message = JSON.parse(this.buffer.subarray(separator + 1, separator + length + 1).toString());
      this.buffer = this.buffer.subarray(separator + length + 1);
      if (!this.greeting) { this.greeting = message; continue; }
      if (message[0] !== 1) continue;
      const waiter = this.waiting.get(message[1]);
      if (!waiter) continue;
      this.waiting.delete(message[1]);
      if (message[2]) waiter.reject(new Error(message[2].message)); else waiter.resolve(message[3]);
    }
  }
  command(name, parameters = {}) {
    const id = this.nextId++;
    const body = Buffer.from(JSON.stringify([0, id, name, parameters]));
    this.socket.write(Buffer.from(`${body.length}:`)); this.socket.write(body);
    return new Promise((resolve, reject) => this.waiting.set(id, { resolve, reject }));
  }
}

async function marionette(port) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const socket = net.createConnection({ host: "127.0.0.1", port });
      const client = new Marionette(socket);
      await new Promise((resolve, reject) => { socket.once("connect", resolve); socket.once("error", reject); });
      for (let wait = 0; wait < 50 && !client.greeting; wait += 1) await sleep(20);
      if (client.greeting) return client;
    } catch { /* retry while Firefox starts */ }
    await sleep(100);
  }
  throw new Error("timed out waiting for Firefox Marionette");
}

async function waitFor(label, check, timeout = 10000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { if (await check()) return; await sleep(100); }
  throw new Error(`timed out waiting for ${label}`);
}

function frame(message) {
  const body = Buffer.from(JSON.stringify(message));
  const length = Buffer.alloc(4); length.writeUInt32LE(body.length); return Buffer.concat([length, body]);
}
function send(socket, type, payload = {}, requestId) {
  const message = { version: 1, type, message_id: unique(), ...payload };
  if (requestId) message.request_id = requestId;
  socket.write(frame(message));
}

async function run() {
  assert.ok(fs.existsSync(nativeHost), `build ${nativeHost} first`);
  if (lifecycleMode) assert.ok(fs.existsSync(lifecycleHarness), `build ${lifecycleHarness} first`);
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "deepify-p3-firefox-"));
  const home = path.join(temp, "home");
  const runtime = path.join(temp, "runtime");
  const profile = path.join(temp, "profile");
  const port = 2900 + Math.floor(Math.random() * 400);
  fs.mkdirSync(home, { recursive: true }); fs.mkdirSync(runtime, { mode: 0o700 }); fs.mkdirSync(profile);
  fs.writeFileSync(path.join(profile, "user.js"), `user_pref("marionette.port", ${port});\nuser_pref("xpinstall.signatures.required", true);\n`);
  const hostDirectory = path.join(home, ".mozilla", "native-messaging-hosts");
  fs.mkdirSync(hostDirectory, { recursive: true });
  fs.writeFileSync(path.join(hostDirectory, "com.deepify.browser.json"), JSON.stringify({
    name: "com.deepify.browser", description: "Deepify test host", path: nativeHost, type: "stdio", allowed_extensions: [extensionId],
  }));
  const socketDirectory = path.join(runtime, "deepify"); fs.mkdirSync(socketDirectory, { mode: 0o700 });
  const socketPath = path.join(socketDirectory, "browser-v1.sock");
  const localServer = http.createServer((request, response) => {
    if (request.url === "/redirect") {
      response.writeHead(302, { location: "https://example.org/phase5-redirect" });
      response.end();
      return;
    }
    response.writeHead(200, { "content-type": "text/html" });
    response.end("<title>Local Deepify fixture</title>");
  });
  await new Promise((resolve, reject) => localServer.listen(0, "127.0.0.1", (error) => error ? reject(error) : resolve()));
  const localUrl = `http://127.0.0.1:${localServer.address().port}/local`;
  const events = [];
  let peer;
  let server;
  let lifecycle;
  let lifecycleOutput = "";
  let lifecycleExit;
  const pairingRecord = path.join(temp, "pairing-token-hash");
  const originalDnd = platformMode ? JSON.parse(execFileSync("noctalia-shell", ["ipc", "call", "state", "all"], { encoding:"utf8" })).state.doNotDisturb : undefined;
  const launchLifecycle = (mode) => {
    lifecycleOutput = "";
    lifecycleExit = undefined;
    lifecycle = spawn(lifecycleHarness, [platformMode ? "session" : mode], {
      env: { ...process.env, XDG_RUNTIME_DIR: platformMode ? process.env.XDG_RUNTIME_DIR : runtime, DEEPIFY_BROWSER_RUNTIME: runtime, DEEPIFY_PAIRING_RECORD: pairingRecord },
      stdio: [platformMode ? "pipe" : "ignore", "pipe", "pipe"],
    });
    lifecycle.stdout.on("data", (value) => { lifecycleOutput += value; });
    lifecycle.stderr.on("data", (value) => { lifecycleOutput += value; });
    lifecycle.on("exit", (code) => { lifecycleExit = code; });
  };
  if (lifecycleMode) {
    assert.ok(["desktop-loss", "browser-restart", "startup-recovery", "platform-finish", "platform-failure"].includes(lifecycleMode), "unknown lifecycle mode");
    launchLifecycle(lifecycleMode === "startup-recovery" ? "startup-first" : lifecycleMode);
  } else {
    server = net.createServer((socket) => {
      peer = socket;
      let buffer = Buffer.alloc(0);
      socket.on("data", (chunk) => {
        buffer = Buffer.concat([buffer, chunk]);
        while (buffer.length >= 4) {
          const size = buffer.readUInt32LE(0); if (buffer.length < size + 4) return;
          const message = JSON.parse(buffer.subarray(4, size + 4).toString()); buffer = buffer.subarray(size + 4); events.push(message);
          if (message.type === "hello") send(socket, "pair_result", { accepted: true }, message.message_id);
          if (message.type === "heartbeat") send(socket, "heartbeat_ack", {}, message.message_id);
        }
      });
    });
    await new Promise((resolve, reject) => server.listen(socketPath, (error) => error ? reject(error) : resolve()));
    fs.chmodSync(socketPath, 0o600);
  }
  // Firefox 155+ requires explicit system access to inspect the extension's
  // privileged blocked page. This applies only to this disposable test process.
  // https://firefox-source-docs.mozilla.org/remote/Prefs.html#remote-system-access-check-enabled
  const browserArguments = ["--headless", "--marionette", "--remote-allow-system-access", "--no-remote", "--new-instance", "--profile", profile];
  if (privateWindow) browserArguments.push("--private-window");
  browserArguments.push("about:blank");
  const launchBrowser = () => {
    const launched = spawn(firefox, browserArguments, {
      env: { ...process.env, HOME: home, XDG_CONFIG_HOME: path.join(home, ".config"), XDG_CACHE_HOME: path.join(home, ".cache"), XDG_RUNTIME_DIR: runtime, MOZ_HEADLESS: "1" },
      stdio: ["ignore", "pipe", "pipe"],
    });
    launched.stdout.on("data", () => {}); launched.stderr.on("data", () => {});
    return launched;
  };
  let browser = launchBrowser();
  let client = await marionette(port);
  try {
    await client.command("WebDriver:NewSession", { capabilities: { alwaysMatch: { pageLoadStrategy: "none" } } });
    if (lifecycleMode) {
      // Arrange the original before pairing can start policy. Zen may otherwise
      // restore an earlier startup page correctly while this test expects ours.
      const destination = `https://example.org/phase3-${lifecycleMode}`;
      await client.command("WebDriver:Navigate", { url: destination });
      await waitFor("lifecycle original destination", async () => {
        const result = await client.command("WebDriver:GetCurrentURL");
        return (result.value ?? result).startsWith(destination);
      });
    }
    console.error(`${browserName} harness: installing extension`);
    await client.command("Addon:Install", { path: extension, temporary: !signedInstall });
    if (lifecycleMode) {
      await waitFor("Rust desktop broker readiness", () => {
        assert.equal(lifecycleExit, undefined, lifecycleOutput);
        return lifecycleOutput.includes(platformMode ? "PHASE4_PLATFORM_READY" : "PHASE3_LIFECYCLE_READY");
      }, 30000);
      console.error(`${browserName} harness: verifying ${lifecycleMode}`);
      const currentUrl = async () => {
        const response = await client.command("WebDriver:GetCurrentURL");
        return response.value ?? response;
      };
      await waitFor("blocked lifecycle destination", async () => (await currentUrl()).endsWith("/blocked.html"));
      if (platformMode) {
        lifecycle.stdin.write(lifecycleMode === "platform-failure" ? "failure\n" : "finish\n");
        await waitFor("combined platform cleanup", () => lifecycleOutput.includes("PHASE4_PLATFORM_CLEANUP_COMPLETE"), 15000);
        await waitFor("restoration after platform cleanup", async () => (await currentUrl()).startsWith(`https://example.org/phase3-${lifecycleMode}`));
        await waitFor("platform harness exit", () => lifecycleExit !== undefined);
        assert.equal(lifecycleExit,0,lifecycleOutput);
        console.log(`${browserName} combined browser/Niri/Noctalia ${lifecycleMode}: PASS`);
        return;
      }
      if (lifecycleMode === "desktop-loss" || lifecycleMode === "startup-recovery") {
        await waitFor("Rust desktop broker exit", () => lifecycleExit !== undefined, 10000);
        assert.equal(lifecycleExit, 0, lifecycleOutput);
        await waitFor("restoration after desktop process loss", async () => (await currentUrl()).startsWith(`https://example.org/phase3-${lifecycleMode}`));
        if (lifecycleMode === "startup-recovery") {
          launchLifecycle("startup-recover");
          await waitFor("startup cleanup checkpoint recovery", () => lifecycleOutput.includes("PHASE3_STARTUP_RECOVERY_COMPLETE"), 20000);
          await waitFor("recovery broker exit", () => lifecycleExit !== undefined, 10000);
          assert.equal(lifecycleExit, 0, lifecycleOutput);
          console.log(`${browserName} startup-checkpoint recovery: PASS`);
          return;
        }
        console.log(`${browserName} desktop-process-loss restoration: PASS`);
        return;
      }
      // Exercise an ordinary browser quit and wait for profile locks/storage to
      // settle before relaunch. SIGTERM is a separate process-loss scenario.
      await client.command("Marionette:Quit", { flags: ["eAttemptQuit"] });
      await waitFor("browser process exit", () => browser.exitCode !== null || browser.signalCode !== null, 15000);
      await waitFor("Rust broker detects browser disconnect", () => lifecycleOutput.includes("PHASE3_BROWSER_DISCONNECTED"), 15000);
      browser = launchBrowser();
      client = await marionette(port);
      await client.command("WebDriver:NewSession", { capabilities: { alwaysMatch: { pageLoadStrategy: "none" } } });
      await client.command("Addon:Install", { path: extension, temporary: !signedInstall });
      await waitFor("idle browser reconnect", () => {
        if (lifecycleOutput.includes("PHASE3_BROWSER_RECONNECTED_IDLE")) return true;
        assert.equal(lifecycleExit, undefined, lifecycleOutput);
        return false;
      }, 20000).catch(error => { throw new Error(`${error.message}; broker: ${lifecycleOutput}`); });
      await waitFor("Rust broker exit", () => lifecycleExit !== undefined, 10000);
      assert.equal(lifecycleExit, 0, lifecycleOutput);
      console.log(`${browserName} browser-close/restart idle reconnect: PASS`);
      return;
    }
    await waitFor("production extension hello", () => events.some((event) => event.type === "hello"));
    await waitFor("native host socket connection", () => Boolean(peer));
    if (privateWindow) {
      console.error(`${browserName} harness: verifying disclosed private-window bypass`);
      send(peer, "start_session", { session_id: "phase3-private-window", timer_state: "working", remaining_seconds: 600, rules: [] });
      await waitFor("start result", () => events.some((event) => event.type === "start_result" && event.accepted));
      await client.command("WebDriver:ExecuteScript", {
        script: "const destination = arguments[0]; setTimeout(() => window.location.assign(destination), 0); return true;",
        args: ["https://example.org/phase3-private-window"],
      });
      await waitFor("unmonitored private destination", async () => {
        const response = await client.command("WebDriver:GetCurrentURL");
        return (response.value ?? response).startsWith("https://example.org/phase3-private-window");
      });
      await sleep(500);
      const privateResponse = await client.command("WebDriver:GetCurrentURL");
      assert.equal((privateResponse.value ?? privateResponse).endsWith("/blocked.html"), false, "private windows must remain an explicit bypass");
      assert.equal(events.some((event) => event.type === "blocked_attempt"), false, "private-window destination must not reach Deepify");
      console.log(`${browserName} private-window bypass: PASS`);
      return;
    }
    console.error(`${browserName} harness: arranging an existing blocked tab`);
    await client.command("WebDriver:ExecuteScript", {
      script: "const destination = arguments[0]; setTimeout(() => window.location.assign(destination), 0); return true;",
      args: ["https://example.org/phase3-history-first"],
    });
    await waitFor("first public history destination", async () => {
      const response = await client.command("WebDriver:GetCurrentURL");
      return (response.value ?? response).startsWith("https://example.org/phase3-history-first");
    });
    await client.command("WebDriver:ExecuteScript", {
      script: "const destination = arguments[0]; setTimeout(() => window.location.assign(destination), 0); return true;",
      args: ["https://example.org/phase3-existing"],
    });
    await waitFor("existing public destination", async () => {
      const response = await client.command("WebDriver:GetCurrentURL");
      return (response.value ?? response).startsWith("https://example.org/phase3-existing");
    });
    console.error(`${browserName} harness: starting paired policy`);
    send(peer, "start_session", { session_id: "phase3-session", timer_state: "working", remaining_seconds: 600, rules: [] });
    await waitFor("start result", () => events.some((event) => event.type === "start_result" && event.accepted));
    let observedUrl = "unknown";
    const currentUrl = async () => {
      const response = await client.command("WebDriver:GetCurrentURL");
      observedUrl = response.value ?? response;
      return observedUrl;
    };
    await waitFor("existing blocked page", async () => (await currentUrl()).endsWith("/blocked.html"));
    await waitFor("blocked-page keyboard focus", async () => {
      const response = await client.command("WebDriver:ExecuteScript", { script: "return document.activeElement?.tagName ?? null;" });
      return (response.value ?? response) === "MAIN";
    });
    await client.command("WebDriver:ExecuteScript", { script: "document.querySelector('#new-tab').focus(); return true;" });
    send(peer, "state", { health: "active", session_id: "phase3-session", timer_state: "working", remaining_seconds: 598 });
    await waitFor("live blocked-page countdown without focus loss", async () => {
      const response = await client.command("WebDriver:ExecuteScript", { script: "return document.querySelector('#remaining')?.textContent === '09:58' && document.activeElement?.id === 'new-tab';" });
      return response.value ?? response;
    });
    const chromeScript = async (script) => {
      await client.command("Marionette:SetContext", { value: "chrome" });
      try {
        const result = await client.command("WebDriver:ExecuteScript", { script });
        return result.value ?? result;
      } finally { await client.command("Marionette:SetContext", { value: "content" }); }
    };
    await chromeScript("for (const label of ['surviving', 'closed']) { const tab=gBrowser.addTab('https://example.org/phase5-container-'+label, {userContextId:1,triggeringPrincipal:Services.scriptSecurityManager.getSystemPrincipal()}); tab.setAttribute('deepify-fixture',label); } return true;");
    await waitFor("container tab restriction without cookies permission", () => chromeScript("return ['surviving','closed'].every(label => Array.from(gBrowser.tabs).some(tab => tab.getAttribute('deepify-fixture')===label && tab.userContextId===1 && tab.linkedBrowser.currentURI.spec.endsWith('/blocked.html')));"));
    await chromeScript("gBrowser.removeTab(Array.from(gBrowser.tabs).find(tab => tab.getAttribute('deepify-fixture')==='closed')); return true;");
    console.error(`${browserName} harness: checking Back and reload protection`);
    await client.command("WebDriver:ExecuteScript", { script: "history.back(); return true;" });
    await waitFor("Back remains blocked", async () => (await currentUrl()).endsWith("/blocked.html"));
    await client.command("WebDriver:ExecuteScript", { script: "location.reload(); return true;" });
    await waitFor("reload remains blocked", async () => (await currentUrl()).endsWith("/blocked.html"));
    console.error(`${browserName} harness: allowing loopback navigation`);
    await client.command("WebDriver:ExecuteScript", {
      script: "const destination = arguments[0]; setTimeout(() => window.location.assign(destination), 0); return true;",
      args: [localUrl],
    });
    await waitFor("loopback page", async () => (await currentUrl()).startsWith(localUrl));
    await client.command("WebDriver:ExecuteScript", {
      script: "setTimeout(() => location.assign(arguments[0]), 0); return true;",
      args: [new URL("/redirect", localUrl).href],
    });
    await waitFor("redirect to public destination is blocked", async () => (await currentUrl()).endsWith("/blocked.html"));
    console.error(`${browserName} harness: navigating to blocked destination`);
    await client.command("WebDriver:ExecuteScript", {
      script: "const destination = arguments[0]; setTimeout(() => window.location.assign(destination), 0); return true;",
      args: ["https://example.org/phase3-private-path?secret=value#fragment"],
    });
    await waitFor("blocked page", async () => (await currentUrl()).endsWith("/blocked.html")).catch((error) => {
      const safeEvents = events.map((event) => ({ type: event.type, keys: Object.keys(event).filter((key) => key !== "token") }));
      throw new Error(`${error.message}; current=${observedUrl}; events=${JSON.stringify(safeEvents)}`);
    });
    assert.ok(events.some((event) => event.type === "blocked_attempt" && event.session_id === "phase3-session"));
    assert.equal(JSON.stringify(events).includes("example.org"), false, "desktop peer must never receive a destination");
    console.error(`${browserName} harness: checking paused navigation protection`);
    send(peer, "state", { health: "active", session_id: "phase3-session", timer_state: "paused", remaining_seconds: 599 });
    await waitFor("live blocked-page pause state", async () => {
      const response = await client.command("WebDriver:ExecuteScript", { script: "return document.querySelector('#remaining')?.textContent === '09:59 · Paused';" });
      return response.value ?? response;
    });
    await client.command("WebDriver:ExecuteScript", {
      script: "const destination = arguments[0]; setTimeout(() => window.location.assign(destination), 0); return true;",
      args: ["https://example.org/phase3-paused"],
    });
    await waitFor("paused navigation remains blocked", async () => (await currentUrl()).endsWith("/blocked.html"));
    console.error(`${browserName} harness: requesting restoration`);
    send(peer, "stop_session", { session_id: "phase3-session" });
    await waitFor("restored original tab", async () => (await currentUrl()).startsWith("https://example.org/phase3-existing"));
    await waitFor("restoration acknowledgment", () => events.some((event) => event.type === "restore_complete" && event.restored_count >= 1));
    await waitFor("container restoration and closed-tab preservation", () => chromeScript("return Array.from(gBrowser.tabs).some(tab => tab.getAttribute('deepify-fixture')==='surviving' && tab.userContextId===1 && tab.linkedBrowser.currentURI.spec==='https://example.org/phase5-container-surviving') && !Array.from(gBrowser.tabs).some(tab => tab.getAttribute('deepify-fixture')==='closed');"));
    console.error(`${browserName} harness: validating native disconnect restoration`);
    send(peer, "start_session", { session_id: "phase3-disconnect", timer_state: "working", remaining_seconds: 600, rules: [] });
    await waitFor("second start result", () => events.filter((event) => event.type === "start_result" && event.accepted).length === 2);
    await client.command("WebDriver:ExecuteScript", {
      script: "const destination = arguments[0]; setTimeout(() => window.location.assign(destination), 0); return true;",
      args: ["https://example.org/phase3-disconnect"],
    });
    await waitFor("second blocked page", async () => (await currentUrl()).endsWith("/blocked.html"));
    peer.destroy();
    await waitFor("restoration after native disconnect", async () => (await currentUrl()).startsWith("https://example.org/phase3-existing"));
    console.log(`${browserName} production native-host/block/restore: PASS`);
  } finally {
    console.error(`${browserName} harness: cleanup`);
    try { await client.command("WebDriver:DeleteSession"); } catch { /* browser may already have exited */ }
    browser.kill("SIGTERM"); lifecycle?.kill("SIGTERM"); server?.close(); localServer.close(); fs.rmSync(temp, { recursive: true, force: true });
    if (platformMode) execFileSync("noctalia-shell", ["ipc", "call", "notifications", originalDnd ? "enableDND" : "disableDND"], { stdio:"ignore" });
  }
}

run().catch((error) => { console.error(error.stack); process.exitCode = 1; });
