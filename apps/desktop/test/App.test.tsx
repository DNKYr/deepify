import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, test, vi } from "vitest";
import type { AppSnapshot } from "../src/backend";

const mocks = vi.hoisted(() => ({
  snapshot: vi.fn(),
  start: vi.fn(),
  pause: vi.fn(),
  resume: vi.fn(),
  end: vi.fn(),
  dismiss: vi.fn(),
  save: vi.fn(),
  complete: vi.fn(),
  add: vi.fn(),
  remove: vi.fn(),
  files: vi.fn(),
  folder: vi.fn(),
  audioToggle: vi.fn(),
  audioPrevious: vi.fn(),
  audioNext: vi.fn(),
  audioVolume: vi.fn(),
  blocked: vi.fn(),
  blockedApp: vi.fn(),
  resolveBlocked: vi.fn(),
  failure: vi.fn(),
  rerun: vi.fn(),
  repair: vi.fn(),
}));
vi.mock("../src/backend", () => ({
  visualDemoSnapshot: undefined,
  appSnapshot: mocks.snapshot,
  sessionStart: mocks.start,
  sessionPause: mocks.pause,
  sessionResume: mocks.resume,
  sessionEnd: mocks.end,
  dismissSummary: mocks.dismiss,
  saveSetting: mocks.save,
  completeSetup: mocks.complete,
  addWhitelist: mocks.add,
  removeWhitelist: mocks.remove,
  importMusicFiles: mocks.files,
  importMusicFolder: mocks.folder,
  audioToggle: mocks.audioToggle,
  audioPrevious: mocks.audioPrevious,
  audioNext: mocks.audioNext,
  audioSetVolume: mocks.audioVolume,
  simulateBlockedAttempt: mocks.blocked,
  simulateBlockedApp: mocks.blockedApp,
  resolveBlockedApps: mocks.resolveBlocked,
  simulateRuntimeFailure: mocks.failure,
  rerunSetup: mocks.rerun,
  repairIntegrations: mocks.repair,
  onSnapshotChanged: vi.fn().mockResolvedValue(() => undefined),
}));
import { App } from "../src/App";

const base = (overrides: Partial<AppSnapshot> = {}): AppSnapshot => ({
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
    setupComplete: true,
  },
  health: [
    {
      component: "Browser",
      status: "healthy",
      lastChecked: 1,
      detail: "Simulated connection",
    },
  ],
  whitelist: [],
  tracks: [],
  queue: [],
  history: [],
  playback: { playing: false, volume: 80, health: "ready" },
  blockedApps: [],
  ...overrides,
});
const renderApp = () =>
  render(
    <MemoryRouter>
      <App />
    </MemoryRouter>,
  );

beforeEach(() => {
  vi.clearAllMocks();
  const initial = base();
  mocks.snapshot.mockResolvedValue(initial);
  for (const mock of Object.values(mocks))
    if (mock !== mocks.snapshot) mock.mockResolvedValue(initial);
});

describe("Phase 2 vertical flow", () => {
  test("requires a positive duration and keeps optional inputs optional", async () => {
    renderApp();
    const duration = await screen.findByLabelText(/work duration/i);
    await userEvent.clear(duration);
    expect(screen.getByRole("button", { name: /start focus/i })).toBeDisabled();
    await userEvent.type(duration, "1");
    await userEvent.click(screen.getByRole("button", { name: /start focus/i }));
    await waitFor(() =>
      expect(mocks.start).toHaveBeenCalledWith(60, undefined, undefined),
    );
  });

  test("active and paused sessions hide navigation and say restrictions remain active", async () => {
    mocks.snapshot.mockResolvedValue(
      base({
        session: {
          state: "paused",
          remainingSeconds: 42,
          focusedSeconds: 18,
          pausedSeconds: 5,
          blockedAttempts: 2,
          restrictionsActive: true,
        },
      }),
    );
    renderApp();
    expect(
      await screen.findByText(/restrictions remain active while paused/i),
    ).toBeVisible();
    expect(
      screen.queryByRole("navigation", { name: /primary/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/blocked attempts: 2/i)).toBeVisible();
  });

  test("early end requires confirmation and displays backend summary", async () => {
    const active = base({
      session: {
        state: "working",
        remainingSeconds: 40,
        focusedSeconds: 20,
        pausedSeconds: 0,
        blockedAttempts: 1,
        restrictionsActive: true,
      },
    });
    const ended = base({
      summary: {
        id: "one",
        focusedSeconds: 20,
        pausedSeconds: 0,
        blockedAttempts: 1,
        reason: "ended_early",
        cleanupComplete: true,
      },
    });
    mocks.snapshot.mockResolvedValue(active);
    mocks.end.mockResolvedValue(ended);
    renderApp();
    await userEvent.click(
      await screen.findByRole("button", { name: /^end session$/i }),
    );
    expect(screen.getByRole("dialog")).toBeVisible();
    await userEvent.click(
      screen.getAllByRole("button", { name: /^end session$/i }).at(-1)!,
    );
    expect(await screen.findByText("Ended early")).toBeVisible();
  });

  test("blocked-app preflight requires explicit simulated resolution", async () => {
    const blocked = base({ blockedApps: ["com.example.Chat"] });
    mocks.snapshot.mockResolvedValue(blocked);
    mocks.resolveBlocked.mockResolvedValue(base());
    renderApp();
    expect(
      await screen.findByRole("heading", { name: /close blocked apps/i }),
    ).toBeVisible();
    expect(screen.getByText("com.example.Chat")).toBeVisible();
    await userEvent.click(
      screen.getByRole("button", { name: /i closed them/i }),
    );
    expect(mocks.resolveBlocked).toHaveBeenCalledOnce();
    expect(await screen.findByLabelText(/work duration/i)).toBeVisible();
  });

  test("invalid whitelist input remains available for correction", async () => {
    mocks.add.mockRejectedValue(new Error("validation"));
    render(
      <MemoryRouter initialEntries={["/whitelist"]}>
        <App />
      </MemoryRouter>,
    );
    const input = await screen.findByLabelText(/allowed value/i);
    await userEvent.type(input, "bad value");
    await userEvent.click(screen.getByRole("button", { name: /add rule/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      /nothing was saved/i,
    );
    expect(input).toHaveValue("bad value");
  });

  test("setup has five steps and music can be skipped explicitly", async () => {
    mocks.snapshot.mockResolvedValue(
      base({
        settings: {
          defaultDurationSeconds: 1500,
          theme: "obsidian",
          notifications: true,
          setupComplete: false,
        },
      }),
    );
    renderApp();
    await screen.findByText(/step 1 of 5/i);
    await userEvent.click(screen.getByRole("button", { name: /continue/i }));
    await userEvent.click(screen.getByRole("button", { name: /continue/i }));
    await userEvent.click(screen.getByRole("button", { name: /continue/i }));
    await userEvent.click(screen.getByRole("button", { name: /skip music/i }));
    expect(screen.getByText(/music: skipped/i)).toBeVisible();
  });

  test("idle navigation and form controls follow the visible keyboard order", async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByLabelText(/work duration/i);
    for (const name of [
      "Focus Room",
      "Sound Library",
      "Whitelist",
      "Session History",
      "Settings",
    ]) {
      await user.tab();
      expect(screen.getByRole("link", { name })).toHaveFocus();
    }
    await user.tab();
    expect(screen.getByLabelText(/work duration/i)).toHaveFocus();
    await user.tab();
    expect(screen.getByLabelText(/intention/i)).toHaveFocus();
    await user.tab();
    expect(screen.getByLabelText(/playlist/i)).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: /start focus/i })).toHaveFocus();
  });

  test("settings save the default duration and expose simulated health repair", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <App />
      </MemoryRouter>,
    );
    const duration = await screen.findByLabelText(/default duration/i);
    await user.clear(duration);
    await user.type(duration, "45");
    await user.click(
      screen.getByRole("button", { name: /save default duration/i }),
    );
    expect(mocks.save).toHaveBeenCalledWith("default_duration_seconds", "2700");
    await user.click(
      screen.getByRole("button", { name: /rerun simulated health checks/i }),
    );
    expect(mocks.repair).toHaveBeenCalledOnce();
  });
});
