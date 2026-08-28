import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type SessionState =
  | "not_working"
  | "starting"
  | "working"
  | "paused"
  | "ending";
export type FinishReason =
  | "completed"
  | "ended_early"
  | "interrupted"
  | "extension_or_app_crash";
export type SessionSummary = {
  id: string;
  focusedSeconds: number;
  pausedSeconds: number;
  blockedAttempts: number;
  reason: FinishReason;
  cleanupComplete: boolean;
};
export type SessionSnapshot = {
  state: SessionState;
  remainingSeconds: number;
  focusedSeconds: number;
  pausedSeconds: number;
  blockedAttempts: number;
  restrictionsActive: boolean;
  intention?: string;
  latestNotice?: string;
};
export type Health = {
  component: string;
  status: "healthy" | "unhealthy";
  lastChecked: number;
  detail: string;
};
export type WhitelistItem = {
  id: string;
  kind: "application" | "website";
  value: string;
};
export type Track = {
  path: string;
  title: string;
  artist?: string;
  album?: string;
  available: boolean;
};
export type HistoryItem = SessionSummary & {
  startedAt: number;
  intention?: string;
};
export type Settings = {
  defaultDurationSeconds: number;
  theme: "obsidian" | "mist";
  notifications: boolean;
  setupComplete: boolean;
};
export type Playback = {
  currentIndex?: number;
  playing: boolean;
  volume: number;
  health: "ready" | "waiting_for_output_device" | "missing_file";
  output?: string;
};
export type AppSnapshot = {
  session: SessionSnapshot;
  settings: Settings;
  health: Health[];
  whitelist: WhitelistItem[];
  tracks: Track[];
  queue: Track[];
  history: HistoryItem[];
  playback: Playback;
  blockedApps: string[];
  summary?: SessionSummary;
};

const demoState =
  import.meta.env.DEV && typeof window !== "undefined"
    ? new URLSearchParams(window.location.search).get("demo")
    : null;

const visualSnapshot = (): AppSnapshot => {
  const snapshot: AppSnapshot = {
    session: {
      state: "not_working",
      remainingSeconds: 0,
      focusedSeconds: 0,
      pausedSeconds: 0,
      blockedAttempts: 0,
      restrictionsActive: false,
    },
    settings: {
      defaultDurationSeconds: 1500,
      theme: "obsidian",
      notifications: true,
      setupComplete: demoState !== "setup",
    },
    health: [
      {
        component: "Firefox/Zen profile",
        status: "healthy",
        lastChecked: 1,
        detail: "Simulated connection",
      },
      {
        component: "Niri application monitor",
        status: "healthy",
        lastChecked: 1,
        detail: "Simulated inventory",
      },
      {
        component: "Noctalia DND",
        status: "healthy",
        lastChecked: 1,
        detail: "Simulated preservation",
      },
    ],
    whitelist: [
      { id: "rule-1", kind: "website", value: "docs.example.com/work" },
    ],
    tracks: [
      {
        path: "/music/Quiet Work.mp3",
        title: "Quiet Work",
        artist: "Deepify Demo",
        available: true,
      },
    ],
    queue: [
      {
        path: "/music/Quiet Work.mp3",
        title: "Quiet Work",
        artist: "Deepify Demo",
        available: true,
      },
    ],
    history: [
      {
        id: "session-demo",
        focusedSeconds: 1500,
        pausedSeconds: 90,
        blockedAttempts: 3,
        reason: "completed",
        cleanupComplete: true,
        startedAt: 1,
        intention: "Write the launch brief",
      },
    ],
    playback: {
      currentIndex: 0,
      playing: demoState === "working",
      volume: 72,
      health: "ready",
      output: "Default system output",
    },
    blockedApps: demoState === "blocked" ? ["com.example.Chat"] : [],
  };
  if (
    demoState === "working" ||
    demoState === "paused" ||
    demoState === "ending"
  ) {
    snapshot.session = {
      state: demoState === "ending" ? "working" : demoState,
      remainingSeconds: 1193,
      focusedSeconds: 307,
      pausedSeconds: demoState === "paused" ? 42 : 0,
      blockedAttempts: 2,
      restrictionsActive: true,
      intention: "Write the launch brief",
      latestNotice: "Example distraction blocked — simulated",
    };
  }
  if (demoState === "summary") {
    snapshot.summary = {
      id: "session-demo",
      focusedSeconds: 1500,
      pausedSeconds: 90,
      blockedAttempts: 3,
      reason: "completed",
      cleanupComplete: true,
    };
  }
  if (demoState === "failed") {
    snapshot.summary = {
      id: "session-demo",
      focusedSeconds: 307,
      pausedSeconds: 42,
      blockedAttempts: 2,
      reason: "extension_or_app_crash",
      cleanupComplete: false,
    };
  }
  return snapshot;
};

export const visualDemoSnapshot = demoState ? visualSnapshot() : undefined;

const request = <T>(command: string, args?: Record<string, unknown>) =>
  visualDemoSnapshot
    ? Promise.resolve(visualDemoSnapshot as T)
    : invoke<T>(command, args);

export const appSnapshot = () => request<AppSnapshot>("app_snapshot");
export const sessionStart = (
  seconds: number,
  intention?: string,
  trackPath?: string,
) => request<AppSnapshot>("session_start", { seconds, intention, trackPath });
export const sessionPause = () => request<AppSnapshot>("session_pause");
export const sessionResume = () => request<AppSnapshot>("session_resume");
export const sessionEnd = () => request<AppSnapshot>("session_end");
export const dismissSummary = () => request<AppSnapshot>("dismiss_summary");
export const saveSetting = (key: string, value: string) =>
  request<AppSnapshot>("save_setting", { key, value });
export const completeSetup = () => request<AppSnapshot>("complete_setup");
export const addWhitelist = (kind: WhitelistItem["kind"], value: string) =>
  request<AppSnapshot>("add_whitelist", { kind, value });
export const removeWhitelist = (id: string) =>
  request<AppSnapshot>("remove_whitelist", { id });
export const importMusicFiles = () =>
  request<AppSnapshot>("import_music_files");
export const importMusicFolder = () =>
  request<AppSnapshot>("import_music_folder");
export const audioToggle = () => request<AppSnapshot>("audio_toggle");
export const audioPrevious = () => request<AppSnapshot>("audio_previous");
export const audioNext = () => request<AppSnapshot>("audio_next");
export const audioSetVolume = (volume: number) =>
  request<AppSnapshot>("audio_set_volume", { volume });
export const simulateBlockedAttempt = () =>
  request<AppSnapshot>("simulate_blocked_attempt");
export const simulateBlockedApp = () =>
  request<AppSnapshot>("simulate_blocked_app");
export const resolveBlockedApps = () =>
  request<AppSnapshot>("resolve_blocked_apps");
export const simulateRuntimeFailure = () =>
  request<AppSnapshot>("simulate_runtime_failure");
export const rerunSetup = () => request<AppSnapshot>("rerun_setup");
export const repairIntegrations = () =>
  request<AppSnapshot>("repair_integrations");
export const onSnapshotChanged = (callback: (snapshot: AppSnapshot) => void) =>
  demoState
    ? Promise.resolve(() => undefined)
    : listen<AppSnapshot>("app://snapshot", (event) => callback(event.payload));
