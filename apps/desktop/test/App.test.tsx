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
  browserForget: vi.fn(),
  browserRetry: vi.fn(),
  browserAccept: vi.fn(),
  add: vi.fn(),
  remove: vi.fn(),
  files: vi.fn(),
  folder: vi.fn(),
  refreshLibrary: vi.fn(),
  audioToggle: vi.fn(),
  audioRetry: vi.fn(),
  audioPrevious: vi.fn(),
  audioNext: vi.fn(),
  audioVolume: vi.fn(),
  blocked: vi.fn(),
  blockedApp: vi.fn(),
  resolveBlocked: vi.fn(),
  failure: vi.fn(),
  rerun: vi.fn(),
  repair: vi.fn(),
  unhealthy: vi.fn(),
  testWhitelist: vi.fn(),
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
  browserForgetPairing: mocks.browserForget,
  browserRetryHealth: mocks.browserRetry,
  browserAcceptPairing: mocks.browserAccept,
  addWhitelist: mocks.add,
  removeWhitelist: mocks.remove,
  importMusicFiles: mocks.files,
  importMusicFolder: mocks.folder,
  refreshMusicLibrary: mocks.refreshLibrary,
  audioToggle: mocks.audioToggle,
  audioRetryOutput: mocks.audioRetry,
  audioPrevious: mocks.audioPrevious,
  audioNext: mocks.audioNext,
  audioSetVolume: mocks.audioVolume,
  simulateBlockedAttempt: mocks.blocked,
  simulateBlockedApp: mocks.blockedApp,
  resolveBlockedApps: mocks.resolveBlocked,
  simulateRuntimeFailure: mocks.failure,
  rerunSetup: mocks.rerun,
  repairIntegrations: mocks.repair,
  simulateUnhealthyIntegration: mocks.unhealthy,
  testWhitelist: mocks.testWhitelist,
  onSnapshotChanged: vi.fn().mockResolvedValue(() => undefined),
}));
import { App } from "../src/App";

const base = (overrides: Partial<AppSnapshot> = {}): AppSnapshot => ({
  diagnosticId: "diagnostic-test-1234",
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
  mocks.testWhitelist.mockResolvedValue({
    allowed: true,
    detail: "Allowed by the current local configuration",
  });
});

describe("Phase 2 vertical flow", () => {
  test("labels unidentified apps even without blocked windows", async () => {
    mocks.snapshot.mockResolvedValue(base({ unidentifiedAppCount: 2 }));
    renderApp();
    expect(
      await screen.findByText("Unidentified app — allowed in MVP: 2"),
    ).toBeVisible();
    expect(screen.getByLabelText(/work duration/i)).toBeVisible();
  });

  test("opening Sound Library refreshes sources and shows missed changes", async () => {
    const track = {
      path: "/music/new.mp3",
      title: "New discovery",
      available: true,
    };
    mocks.refreshLibrary.mockResolvedValue(
      base({ tracks: [track], queue: [track] }),
    );
    renderApp();
    await userEvent.click(
      await screen.findByRole("link", { name: "Sound Library" }),
    );
    await waitFor(() => expect(mocks.refreshLibrary).toHaveBeenCalledOnce());
    expect(await screen.findAllByText("New discovery")).toHaveLength(2);
  });

  test("an active session keeps its controls visible on a stale settings route", async () => {
    mocks.snapshot.mockResolvedValue(
      base({
        session: {
          state: "working",
          remainingSeconds: 42,
          focusedSeconds: 18,
          pausedSeconds: 0,
          blockedAttempts: 0,
          restrictionsActive: true,
        },
      }),
    );
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByRole("button", { name: "Pause" })).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /run setup/i }),
    ).not.toBeInTheDocument();
  });

  test("rejects durations outside the backend's whole-second and 24-hour limits", async () => {
    renderApp();
    const duration = await screen.findByLabelText(/work duration/i);
    for (const invalid of ["1441", "0.001"]) {
      await userEvent.clear(duration);
      await userEvent.type(duration, invalid);
      expect(
        screen.getByRole("button", { name: /start focus/i }),
      ).toBeDisabled();
    }
    expect(mocks.start).not.toHaveBeenCalled();
  });

  test("forgetting a browser requires confirmation and keeps keyboard focus in the dialog", async () => {
    mocks.snapshot.mockResolvedValue(
      base({
        health: [
          {
            component: "Firefox/Zen profile",
            status: "healthy",
            lastChecked: 1,
            detail: "Paired",
          },
        ],
      }),
    );
    renderApp();
    const user = userEvent.setup();
    await user.click(await screen.findByRole("link", { name: "Settings" }));
    const trigger = screen.getByRole("button", {
      name: "Forget paired profile",
    });
    await user.click(trigger);
    const cancel = screen.getByRole("button", { name: "Keep paired profile" });
    expect(cancel).toHaveFocus();
    expect(mocks.browserForget).not.toHaveBeenCalled();
    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(
      screen.getByRole("button", { name: "Forget profile" }),
    ).toHaveFocus();
    await user.keyboard("{Tab}");
    expect(cancel).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(trigger).toHaveFocus();
    await user.click(trigger);
    await user.click(screen.getByRole("button", { name: "Forget profile" }));
    expect(mocks.browserForget).toHaveBeenCalledOnce();
  });

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
    const user = userEvent.setup();
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
    const endTrigger = await screen.findByRole("button", {
      name: /^end session$/i,
    });
    await user.click(endTrigger);
    expect(screen.getByRole("dialog")).toBeVisible();
    expect(screen.getByRole("button", { name: /keep working/i })).toHaveFocus();
    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(
      screen.getAllByRole("button", { name: /^end session$/i }).at(-1),
    ).toHaveFocus();
    await user.keyboard("{Tab}");
    expect(screen.getByRole("button", { name: /keep working/i })).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(endTrigger).toHaveFocus();
    await user.click(endTrigger);
    await user.click(
      screen.getAllByRole("button", { name: /^end session$/i }).at(-1)!,
    );
    expect(await screen.findByText("Ended early")).toBeVisible();
  });

  test("blocked-app preflight rechecks the backend inventory", async () => {
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

  test("whitelist configuration test is local, typed, and observable", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/whitelist"]}>
        <App />
      </MemoryRouter>,
    );
    const testInput = await screen.findByLabelText(
      /website or application to test/i,
    );
    await user.type(testInput, "docs.example.com/work");
    await user.click(
      screen.getByRole("button", { name: /^test configuration$/i }),
    );
    expect(mocks.testWhitelist).toHaveBeenCalledWith(
      "website",
      "docs.example.com/work",
    );
    expect(await screen.findByText(/this test is local/i)).toHaveTextContent(
      /allowed.*not saved/i,
    );
  });

  test("history shows separate daily and Monday-based weekly totals", async () => {
    const now = Math.floor(Date.now() / 1000);
    mocks.snapshot.mockResolvedValue(
      base({
        history: [
          {
            id: "today",
            focusedSeconds: 600,
            pausedSeconds: 0,
            blockedAttempts: 0,
            reason: "completed",
            cleanupComplete: true,
            startedAt: now,
          },
          {
            id: "old",
            focusedSeconds: 300,
            pausedSeconds: 0,
            blockedAttempts: 0,
            reason: "completed",
            cleanupComplete: true,
            startedAt: 1,
          },
        ],
      }),
    );
    render(
      <MemoryRouter initialEntries={["/history"]}>
        <App />
      </MemoryRouter>,
    );
    expect(
      await screen.findByText(/today · 10:00 focused · 1 sessions/i),
    ).toBeVisible();
    expect(screen.getByText(/this week · 10:00 focused/i)).toBeVisible();
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

  test("settings save the default duration and expose integration health repair", async () => {
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
    expect(screen.getByText("diagnostic-test-1234")).toBeVisible();
    const unhealthy = base({
      health: [
        {
          component: "Niri application monitor",
          status: "unhealthy",
          lastChecked: 2,
          detail: "Simulated preflight failure",
        },
      ],
    });
    mocks.repair.mockResolvedValue(unhealthy);
    await user.click(
      screen.getByRole("button", { name: /rerun integration health checks/i }),
    );
    expect(
      await screen.findByText(/niri application monitor: unhealthy/i),
    ).toBeVisible();
    expect(mocks.repair).toHaveBeenCalledOnce();
  });
  test("a refused app close keeps pause and end controls available", async () => {
    mocks.snapshot.mockResolvedValue(
      base({
        session: {
          state: "working",
          remainingSeconds: 40,
          focusedSeconds: 20,
          pausedSeconds: 0,
          blockedAttempts: 1,
          restrictionsActive: true,
        },
        blockedApps: ["com.example.Editor"],
      }),
    );
    renderApp();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      /close these applications manually/i,
    );
    expect(
      screen.getByRole("button", { name: /^end session$/i }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: /^pause$/i })).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /allow com.example.Editor/i }),
    ).not.toBeInTheDocument();
  });

  test("an identified blocked app can be whitelisted from preflight", async () => {
    mocks.snapshot.mockResolvedValue(
      base({ blockedApps: ["com.example.Editor"] }),
    );
    mocks.resolveBlocked.mockResolvedValue(base());
    renderApp();
    await userEvent.click(
      await screen.findByRole("button", { name: "Allow com.example.Editor" }),
    );
    expect(mocks.add).toHaveBeenCalledWith("application", "com.example.Editor");
    expect(mocks.resolveBlocked).toHaveBeenCalledOnce();
  });

  test("disconnected browser setup can reach the pairing step", async () => {
    mocks.snapshot.mockResolvedValue(
      base({
        settings: {
          defaultDurationSeconds: 1500,
          theme: "obsidian",
          notifications: true,
          setupComplete: false,
        },
        health: [
          {
            component: "Firefox/Zen profile",
            status: "unhealthy",
            lastChecked: 1,
            detail: "Pairing required",
          },
        ],
      }),
    );
    renderApp();
    await userEvent.click(
      await screen.findByRole("button", { name: "Continue" }),
    );
    expect(
      screen.getByRole("button", { name: "Pair this profile" }),
    ).toBeVisible();
  });
});
