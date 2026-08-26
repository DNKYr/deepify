#!/usr/bin/env node

const fs = require("node:fs");

const logPath = process.env.P1_SPIKE_LOG || "/tmp/deepwork-focus-p1-22-native.log";
const scenario = process.env.P1_SPIKE_SCENARIO || "interactive";
const scenarioDelay = Number(process.env.P1_SPIKE_DELAY_MS || 3000);
const log = (event, data = {}) => {
  fs.appendFileSync(logPath, `${JSON.stringify({ event, ...data })}\n`);
};

let buffer = Buffer.alloc(0);
let started = false;

function send(message) {
  const body = Buffer.from(JSON.stringify(message));
  const header = Buffer.alloc(4);
  header.writeUInt32LE(body.length, 0);
  process.stdout.write(Buffer.concat([header, body]));
  log("send", message);
}

function onMessage(message) {
  log("receive", message);
  if (message.type === "hello") {
    send({ type: "health_ack", protocol: 1, nativeHost: "node-stdio-spike" });
    if (scenario === "start-stop" && !started) {
      started = true;
      send({ type: "start", whitelist: ["example.com"] });
      send({ type: "container_probe" });
      setTimeout(() => send({ type: "stop", reason: "test_stop" }), scenarioDelay);
    }
    if (scenario === "disconnect" && !started) {
      started = true;
      send({ type: "start", whitelist: ["example.com"] });
      send({ type: "container_probe" });
      setTimeout(() => {
        log("intentional_exit");
        process.exit(0);
      }, scenarioDelay);
    }
  }
  if (message.type === "heartbeat") send({ type: "heartbeat_ack", at: Date.now() });
}

process.stdin.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (buffer.length >= 4) {
    const length = buffer.readUInt32LE(0);
    if (buffer.length < 4 + length) return;
    const body = buffer.subarray(4, 4 + length).toString("utf8");
    buffer = buffer.subarray(4 + length);
    try { onMessage(JSON.parse(body)); }
    catch (error) { log("parse_error", { error: String(error) }); }
  }
});

process.stdin.on("end", () => log("stdin_end"));
process.on("exit", () => log("exit"));
