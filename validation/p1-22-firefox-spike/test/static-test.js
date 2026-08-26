#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const manifest = JSON.parse(fs.readFileSync(path.join(root, "extension", "manifest.json")));
const hostManifest = JSON.parse(fs.readFileSync(path.join(root, "native", "org.deepwork_focus.p1_22_spike.json.in"), "utf8").replace("@HOST_PATH@", "/tmp/host.js"));

assert.equal(manifest.browser_specific_settings.gecko.id, hostManifest.allowed_extensions[0]);
assert.ok(manifest.permissions.includes("nativeMessaging"));
assert.equal(hostManifest.type, "stdio");
assert.match(hostManifest.name, /^\w+(\.\w+)*$/);

function frame(message) {
  const body = Buffer.from(JSON.stringify(message));
  const header = Buffer.alloc(4);
  header.writeUInt32LE(body.length, 0);
  return Buffer.concat([header, body]);
}

const encoded = frame({ type: "hello", tabs: [] });
assert.equal(encoded.readUInt32LE(0), encoded.length - 4);
assert.deepEqual(JSON.parse(encoded.subarray(4).toString()), { type: "hello", tabs: [] });
console.log("static manifest and native stdio framing: PASS");
