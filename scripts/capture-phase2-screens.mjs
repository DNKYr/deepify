import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";

const root = new URL("..", import.meta.url).pathname;
const output = join(root, "validation", "phase2-screens");
mkdirSync(output, { recursive: true });

const states = [
  ["01-setup", "http://127.0.0.1:4173/?demo=setup"],
  ["02-focus-idle", "http://127.0.0.1:4173/?demo=idle"],
  ["03-preflight-validation", "http://127.0.0.1:4173/?demo=validation"],
  ["04-blocked-apps", "http://127.0.0.1:4173/?demo=blocked"],
  ["05-focus-working", "http://127.0.0.1:4173/?demo=working"],
  ["06-focus-paused", "http://127.0.0.1:4173/?demo=paused"],
  ["07-end-confirmation", "http://127.0.0.1:4173/?demo=ending"],
  ["08-session-summary", "http://127.0.0.1:4173/?demo=summary"],
  ["09-failure-summary", "http://127.0.0.1:4173/?demo=failed"],
  ["10-sound-library", "http://127.0.0.1:4173/library?demo=library"],
  ["11-whitelist", "http://127.0.0.1:4173/whitelist?demo=whitelist"],
  ["12-settings", "http://127.0.0.1:4173/settings?demo=settings"],
  ["13-session-history", "http://127.0.0.1:4173/history?demo=history"],
  [
    "14-browser-blocked-page",
    `file://${join(root, "extensions", "firefox", "blocked.html")}?destination=example.com%20%2F%20path&remaining=37%3A42`,
  ],
];

for (const [name, url] of states) {
  const profile = mkdtempSync(join(tmpdir(), "deepify-firefox-shot-"));
  const file = join(output, `${name}.png`);
  const result = spawnSync(
    "firefox",
    [
      "--headless",
      "--no-remote",
      "--profile",
      profile,
      "--window-size",
      "1440,1000",
      "--screenshot",
      file,
      url,
    ],
    { stdio: "inherit", timeout: 60_000 },
  );
  rmSync(profile, { recursive: true, force: true });
  if (result.status !== 0) {
    throw new Error(
      `Firefox failed to capture ${name} (exit ${result.status}${result.signal ? `; signal ${result.signal}` : ""}${result.error ? `; ${result.error.message}` : ""})`,
    );
  }
}
