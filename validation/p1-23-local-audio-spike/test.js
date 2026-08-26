#!/usr/bin/env node

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, renameSync, rmSync, readdirSync, statSync, existsSync, accessSync, constants } from "node:fs";
import { tmpdir } from "node:os";
import { join, basename, extname, relative, resolve } from "node:path";

const root = mkdtempSync(join(tmpdir(), "deepwork-p1-23-"));

function id3TextFrame(id, value) {
  const text = Buffer.concat([Buffer.from([3]), Buffer.from(value, "utf8")]);
  const size = Buffer.alloc(4);
  size.writeUInt32BE(text.length);
  return Buffer.concat([Buffer.from(id), size, Buffer.from([0, 0]), text]);
}

function addId3Tags(mp3, tags) {
  const frames = Object.entries(tags)
    .filter(([, value]) => value)
    .map(([key, value]) => id3TextFrame(key, value));
  const body = Buffer.concat(frames);
  const header = Buffer.from([0x49, 0x44, 0x33, 3, 0, 0, (body.length >> 21) & 0x7f,
    (body.length >> 14) & 0x7f, (body.length >> 7) & 0x7f, body.length & 0x7f]);
  return Buffer.concat([header, body, mp3]);
}

function readSynchsafe(bytes) {
  return ((bytes[0] & 0x7f) << 21) | ((bytes[1] & 0x7f) << 14) |
    ((bytes[2] & 0x7f) << 7) | (bytes[3] & 0x7f);
}

function decodeId3Text(frame) {
  if (!frame.length) return "";
  const encoding = frame[0];
  const payload = frame.subarray(1);
  if (encoding === 1) return payload.toString("utf16le").replace(/^\uFEFF/, "").replace(/\0+$/, "");
  if (encoding === 2) return payload.toString("utf16le").replace(/\0+$/, "");
  return payload.toString(encoding === 3 ? "utf8" : "latin1").replace(/\0+$/, "");
}

function readMetadata(file) {
  const data = readFileSync(file);
  const tags = {};
  if (data.subarray(0, 3).toString() === "ID3") {
    const version = data[3];
    const end = Math.min(data.length, 10 + readSynchsafe(data.subarray(6, 10)));
    let offset = 10;
    while (offset + 10 <= end) {
      const id = data.subarray(offset, offset + 4).toString("ascii");
      if (!/^[A-Z0-9]{4}$/.test(id) || id[0] === "\0") break;
      const size = version === 4 ? readSynchsafe(data.subarray(offset + 4, offset + 8)) : data.readUInt32BE(offset + 4);
      if (!size || offset + 10 + size > end) break;
      if (id === "TIT2") tags.title = decodeId3Text(data.subarray(offset + 10, offset + 10 + size));
      if (id === "TPE1") tags.artist = decodeId3Text(data.subarray(offset + 10, offset + 10 + size));
      if (id === "TALB") tags.album = decodeId3Text(data.subarray(offset + 10, offset + 10 + size));
      offset += 10 + size;
    }
  }

  const stem = basename(file, extname(file));
  const fallback = stem.match(/^(.+?)\s+-\s+(.+)$/);
  const fallbackMetadata = fallback ? { artist: fallback[1], title: fallback[2] } : { title: stem };
  return {
    title: tags.title || fallbackMetadata.title,
    artist: tags.artist || fallbackMetadata.artist || null,
    album: tags.album || null,
    source: Object.keys(tags).length ? "id3" : "filename",
  };
}

function scanFolder(folder) {
  const tracks = [];
  function visit(directory) {
    for (const entry of readFileEntries(directory)) {
      const path = join(directory, entry);
      if (entry.startsWith(".")) continue;
      if (isDirectory(path)) visit(path);
      else if (extname(entry).toLowerCase() === ".mp3") tracks.push(path);
    }
  }
  visit(folder);
  return tracks.sort((a, b) => relative(folder, a).localeCompare(relative(folder, b)));
}

function readFileEntries(directory) {
  // Kept behind a small adapter so the scan policy is easy to replace with Tauri commands.
  return readdirSync(directory).sort();
}

function isDirectory(path) { return statSync(path).isDirectory(); }

function makeTrack(source, path, folder = null) {
  return {
    source,
    path: resolve(path),
    relativePath: folder ? relative(folder, path) : null,
    metadata: readMetadata(path),
  };
}

function resolvePlaylist(ref) {
  if (ref.kind === "file") {
    if (!existsSync(ref.path)) return [{ source: "file", path: resolve(ref.path), metadata: null, available: false }];
    return [makeTrack("file", ref.path)];
  }
  return scanFolder(ref.path).map(path => makeTrack("folder", path, ref.path));
}

function reconcileQueue(previous, discovered) {
  const byPath = new Map(discovered.map(track => [track.path, track]));
  const retained = previous.filter(path => byPath.has(path)).map(path => byPath.get(path));
  const retainedPaths = new Set(retained.map(track => track.path));
  const appended = discovered.filter(track => !retainedPaths.has(track.path));
  return [...retained, ...appended];
}

function updateOutputDevice(player, availableDevices) {
  if (player.device && availableDevices.includes(player.device)) {
    player.state = "playing";
    return;
  }
  player.device = availableDevices[0] || null;
  player.state = player.device ? "playing" : "waiting-for-output-device";
}

function persistLibrary(file, library, queue) {
  writeFileSync(file, JSON.stringify({ version: 1, library, queue }, null, 2));
}

function loadLibrary(file) { return JSON.parse(readFileSync(file, "utf8")); }

function run(command, args) {
  execFileSync(command, args, { stdio: "ignore", timeout: 10000 });
}

function findExecutable(names) {
  for (const directory of (process.env.PATH || "").split(":").filter(Boolean)) {
    for (const name of names) {
      const path = join(directory, name);
      try { accessSync(path, constants.X_OK); return path; } catch {}
    }
  }
  return null;
}

function findNixExecutable(name) {
  try {
    const storeEntry = readdirSync("/nix/store").find(entry => entry.includes("ffmpeg") && entry.endsWith("-bin"));
    if (storeEntry) return join("/nix/store", storeEntry, "bin", name);
  } catch {}
  return null;
}

function createWav(path) {
  const sampleRate = 44100;
  const samples = Math.floor(sampleRate * 0.35);
  const pcm = Buffer.alloc(samples * 2);
  for (let i = 0; i < samples; i++) pcm.writeInt16LE(Math.round(Math.sin(i / 8) * 7000), i * 2);
  const header = Buffer.alloc(44);
  header.write("RIFF", 0); header.writeUInt32LE(36 + pcm.length, 4); header.write("WAVE", 8);
  header.write("fmt ", 12); header.writeUInt32LE(16, 16); header.writeUInt16LE(1, 20);
  header.writeUInt16LE(1, 22); header.writeUInt32LE(sampleRate, 24); header.writeUInt32LE(sampleRate * 2, 28);
  header.writeUInt16LE(2, 32); header.writeUInt16LE(16, 34); header.write("data", 36); header.writeUInt32LE(pcm.length, 40);
  writeFileSync(path, Buffer.concat([header, pcm]));
}

function createMp3(path, tags = null) {
  const wav = join(root, `${basename(path)}.wav`);
  createWav(wav);
  const ffmpeg = findExecutable(["ffmpeg"]) || findNixExecutable("ffmpeg");
  assert(ffmpeg, "FFmpeg is required to create the deterministic MP3 fixture");
  run(ffmpeg, ["-y", "-loglevel", "error", "-i", wav, "-codec:a", "libmp3lame", "-b:a", "128k", path]);
  const mp3 = readFileSync(path);
  writeFileSync(path, tags ? addId3Tags(mp3, tags) : mp3);
}

function playbackSmokeTest(mp3) {
  const ffmpeg = findExecutable(["ffmpeg"]) || findNixExecutable("ffmpeg");
  assert(ffmpeg, "FFmpeg is required for the playback smoke test");
  // Decode the complete file through the native audio stack without requiring a live device.
  run(ffmpeg, ["-v", "error", "-i", mp3, "-f", "null", "-"]);
  const vlc = findExecutable(["cvlc", "vlc"]);
  assert(vlc, "VLC is required for the playback smoke test");
  // VLC's dummy output exercises the player path while remaining safe in a headless shell.
  run(vlc, ["--intf", "dummy", "--no-video", "--aout", "dummy", "--play-and-exit", mp3, "vlc://quit"]);
}

try {
  const music = join(root, "music");
  mkdirSync(join(music, "nested"), { recursive: true });
  const tagged = join(music, "Tagged.mp3");
  const fallback = join(music, "Composer - Filename title.mp3");
  createMp3(tagged, { TIT2: "Tagged title", TPE1: "Tagged artist", TALB: "Tagged album" });
  createMp3(fallback);

  // Individual file selection and tag-first / filename-fallback metadata behavior.
  const taggedTrack = makeTrack("file", tagged);
  assert.deepEqual(taggedTrack.metadata, { title: "Tagged title", artist: "Tagged artist", album: "Tagged album", source: "id3" });
  const fallbackTrack = makeTrack("file", fallback);
  assert.deepEqual(fallbackTrack.metadata, { title: "Filename title", artist: "Composer", album: null, source: "filename" });
  playbackSmokeTest(tagged);
  console.log("PASS individual MP3 selection, ID3 metadata, filename fallback, and native playback");

  // Directory selection is a live recursive MP3 playlist; non-MP3 files are ignored.
  writeFileSync(join(music, "ignore.txt"), "not audio");
  const folderRef = { kind: "folder", path: music };
  let playlist = resolvePlaylist(folderRef);
  assert.deepEqual(playlist.map(track => relative(music, track.path)), ["Composer - Filename title.mp3", "Tagged.mp3"]);
  const libraryFile = join(root, "library.json");
  persistLibrary(libraryFile, [{ kind: "file", path: tagged }, folderRef], playlist.map(track => track.path));
  const restored = loadLibrary(libraryFile);
  assert.deepEqual(restored.library, [{ kind: "file", path: tagged }, folderRef]);
  assert.equal(resolvePlaylist(restored.library[1]).length, 2);
  console.log("PASS individual-file and live-folder references survive JSON persistence/restart");

  // New files append, while unchanged path order is retained in the playback queue.
  const added = join(music, "Added.mp3");
  createMp3(added);
  playlist = resolvePlaylist(folderRef);
  let queue = reconcileQueue(restored.queue, playlist);
  assert.deepEqual(queue.map(track => basename(track.path)), ["Composer - Filename title.mp3", "Tagged.mp3", "Added.mp3"]);

  // Rename and move are remove+add events. Deleted files disappear; new paths append.
  const moved = join(music, "nested", "Moved.mp3");
  renameSync(fallback, moved);
  rmSync(tagged);
  const missing = resolvePlaylist({ kind: "file", path: tagged })[0];
  assert.deepEqual(missing, { source: "file", path: resolve(tagged), metadata: null, available: false });
  playlist = resolvePlaylist(folderRef);
  queue = reconcileQueue(queue.map(track => track.path), playlist);
  assert.deepEqual(queue.map(track => relative(music, track.path)), ["Added.mp3", "nested/Moved.mp3"]);
  assert(!queue.some(track => track.path === tagged), "deleted file must leave the queue");
  console.log("PASS folder rescan after add, move, rename, and delete; deterministic queue reconciliation");

  // Simulate a device loss/recovery contract: playback state becomes waiting and resumes only after a device appears.
  const player = { state: "playing", device: "default" };
  updateOutputDevice(player, []);
  assert.equal(player.state, "waiting-for-output-device");
  updateOutputDevice(player, ["new-default"]);
  assert.equal(player.device, "new-default");
  assert.equal(player.state, "playing");
  console.log("PASS device-loss state-machine contract (hardware device switch not available in this headless run)");

  console.log("RESULT: 5 checks passed; the output-device assertion is a model-level contract only.");
} finally {
  rmSync(root, { recursive: true, force: true });
}
