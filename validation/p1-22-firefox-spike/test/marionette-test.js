#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");

const root = path.resolve(__dirname, "..");
const firefox = process.env.FIREFOX_BIN || "firefox";
const extensionId = "p1-22-spike@deepwork-focus-bot.invalid";

function sleep(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }

class Marionette {
  constructor(socket) {
    this.socket = socket;
    this.buffer = Buffer.alloc(0);
    this.nextId = 1;
    this.waiting = new Map();
    this.greeting = null;
    socket.on("data", (chunk) => this.receive(chunk));
    socket.on("error", (error) => this.fail(error));
    socket.on("close", () => this.fail(new Error("Marionette socket closed")));
  }

  fail(error) {
    for (const { reject } of this.waiting.values()) reject(error);
    this.waiting.clear();
  }

  receive(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length > 0) {
      const separator = this.buffer.indexOf(0x3a);
      if (separator === -1) return;
      const length = Number(this.buffer.subarray(0, separator).toString());
      if (!Number.isInteger(length) || this.buffer.length < separator + 1 + length) return;
      const payload = this.buffer.subarray(separator + 1, separator + 1 + length).toString();
      this.buffer = this.buffer.subarray(separator + 1 + length);
      const message = JSON.parse(payload);
      if (!this.greeting) { this.greeting = message; continue; }
      if (message[0] !== 1) continue;
      const waiter = this.waiting.get(message[1]);
      if (!waiter) continue;
      this.waiting.delete(message[1]);
      if (message[2]) waiter.reject(new Error(`${message[2].error}: ${message[2].message}`));
      else waiter.resolve(message[3]);
    }
  }

  command(name, parameters = {}) {
    const id = this.nextId++;
    const body = Buffer.from(JSON.stringify([0, id, name, parameters]));
    this.socket.write(Buffer.from(`${body.length}:`));
    this.socket.write(body);
    return new Promise((resolve, reject) => this.waiting.set(id, { resolve, reject }));
  }
}

async function connectMarionette(port = 2828) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      const candidate = net.createConnection({ host: "127.0.0.1", port });
      const client = new Marionette(candidate);
      await new Promise((resolve, reject) => {
        candidate.once("connect", resolve);
        candidate.once("error", reject);
      });
      for (let i = 0; i < 50 && !client.greeting; i += 1) await sleep(20);
      if (!client.greeting) throw new Error("Firefox did not send the Marionette greeting");
      return client;
    } catch (_) { await sleep(100); }
  }
  throw new Error("Timed out waiting for Firefox Marionette");
}

async function waitFor(description, check, timeout = 8000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await check()) return;
    await sleep(100);
  }
  throw new Error(`Timed out waiting for ${description}`);
}

async function runScenario(scenario, port) {
  console.error(`scenario ${scenario}: setup`);
  const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "p1-22-home-"));
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), "p1-22-profile-"));
  fs.writeFileSync(path.join(profile, "user.js"), `user_pref("marionette.port", ${port});\n`);
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "p1-22-run-"));
  const manifestDirs = [
    path.join(tempHome, ".mozilla", "native-messaging-hosts"),
    path.join(tempHome, ".config", "mozilla", "native-messaging-hosts")
  ];
  for (const manifestDir of manifestDirs) fs.mkdirSync(manifestDir, { recursive: true });
  const logPath = path.join(work, "native.log");
  const hostWrapper = path.join(work, "native-host");
  fs.writeFileSync(hostWrapper, `#!/bin/sh\nexec ${process.execPath} ${path.join(root, "native", "host.js")}\n`);
  fs.chmodSync(hostWrapper, 0o755);
  const manifest = {
    name: "org.deepwork_focus.p1_22_spike",
    description: "P1-22 test host",
    path: hostWrapper,
    type: "stdio",
    allowed_extensions: [extensionId]
  };
  for (const manifestDir of manifestDirs) {
    fs.writeFileSync(path.join(manifestDir, `${manifest.name}.json`), JSON.stringify(manifest));
  }
  const addonPath = path.join(root, "extension");
  const browser = spawn(firefox, ["--headless", "--marionette", "--no-remote", "--new-instance", "--profile", profile, "about:blank"], {
    env: {
      ...process.env,
      HOME: tempHome,
      XDG_CONFIG_HOME: path.join(tempHome, ".config"),
      XDG_CACHE_HOME: path.join(tempHome, ".cache"),
      P1_SPIKE_SCENARIO: scenario,
      P1_SPIKE_DELAY_MS: "8000",
      P1_SPIKE_LOG: logPath,
      MOZ_HEADLESS: "1"
    },
    stdio: ["ignore", "pipe", "pipe"]
  });
  console.error(`scenario ${scenario}: firefox pid ${browser.pid}`);
  let browserOutput = "";
  browser.stdout.on("data", (chunk) => { browserOutput += chunk; });
  browser.stderr.on("data", (chunk) => { browserOutput += chunk; });
  const client = await connectMarionette(port);
  console.error(`scenario ${scenario}: connected`);
  try {
    await client.command("WebDriver:NewSession", { capabilities: { alwaysMatch: {} } });
    console.error(`scenario ${scenario}: session`);
    await client.command("Addon:Install", { path: addonPath, temporary: true });
    console.error(`scenario ${scenario}: addon`);
    await waitFor("native hello", () => fs.existsSync(logPath) && fs.readFileSync(logPath, "utf8").includes('"event":"receive"')).catch((error) => {
      throw new Error(`${error.message}\nFirefox output:\n${browserOutput}`);
    });
    const normalWindow = (await client.command("WebDriver:GetWindowHandle")).value;
    await client.command("WebDriver:Navigate", { url: "https://example.org/p1-22-original" });
    const currentUrl = async () => {
      const result = await client.command("WebDriver:GetCurrentURL");
      return result && typeof result === "object" && "value" in result ? result.value : result;
    };
    await waitFor("blocked page", async () => (await currentUrl()).includes("blocked.html"));
    const blockedUrl = await currentUrl();
    assert.match(blockedUrl, /p1-22-original/);
    if (scenario === "start-stop" && process.env.P1_SKIP_PRIVATE !== "1") {
      const privateWindow = await client.command("WebDriver:NewWindow", { type: "window", private: true });
      console.error(`private window result ${JSON.stringify(privateWindow)}`);
      const privateHandle = privateWindow.value?.handle || privateWindow.handle;
      await client.command("WebDriver:SwitchToWindow", { handle: privateHandle, focus: true });
      await client.command("WebDriver:Navigate", { url: "https://example.org/p1-22-private" });
      assert.equal(await currentUrl(), "https://example.org/p1-22-private");
      await client.command("WebDriver:SwitchToWindow", { handle: normalWindow, focus: true });
    }
    if (scenario === "start-stop") {
      await waitFor("restored original URL", async () => (await currentUrl()) === "https://example.org/p1-22-original");
    } else {
      await waitFor("restored URL after native disconnect", async () => (await currentUrl()) === "https://example.org/p1-22-original");
      assert.match(fs.readFileSync(logPath, "utf8"), /intentional_exit/);
    }
    const log = fs.readFileSync(logPath, "utf8");
    assert.match(log, /"type":"hello"/);
    assert.match(log, /"type":"health_ack"/);
    assert.match(log, /"type":"heartbeat_ack"/);
    assert.match(log, /"type":"container_result"/);
    assert.match(log, /No permission for cookieStoreId/);
    const events = log.trim().split("\n").map((line) => JSON.parse(line));
    const hello = events.find((event) => event.event === "receive" && event.type === "hello");
    assert.ok(hello, "native host received hello");
    assert.match(hello.profilePairingToken, new RegExp(`^${extensionId}:`));
    assert.ok(Array.isArray(hello.tabs), "hello includes tab inventory");
    assert.ok(hello.tabs.some((tab) => typeof tab.url === "string" &&
      typeof tab.incognito === "boolean" && "cookieStoreId" in tab),
    "tab inventory includes URL, incognito, and cookieStoreId");
    assert.ok(events.some((event) => event.event === "receive" && event.type === "tab_blocked"),
      "native host received tab_blocked");
    return { scenario, blockedUrl, log, browserOutput };
  } finally {
    try { await client.command("WebDriver:DeleteSession"); } catch (_) {}
    browser.kill("SIGTERM");
    await sleep(250);
  }
}

(async () => {
  const results = [];
  for (const [index, scenario] of ["start-stop", "disconnect"].entries()) {
    results.push(await runScenario(scenario, 2830 + index));
  }
  for (const result of results) {
    console.log(`${result.scenario}: PASS; blocked=${result.blockedUrl}`);
  }
})().catch((error) => {
  console.error(error.stack || error);
  process.exitCode = 1;
});
