import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { NavLink, Route, Routes, useNavigate } from "react-router-dom";
import {
  addWhitelist,
  appSnapshot,
  audioNext,
  audioPrevious,
  audioRetryOutput,
  audioSetVolume,
  audioToggle,
  browserAcceptPairing,
  browserForgetPairing,
  browserRetryHealth,
  completeSetup,
  dismissSummary,
  importMusicFiles,
  importMusicFolder,
  onSnapshotChanged,
  removeWhitelist,
  repairIntegrations,
  resolveBlockedApps,
  rerunSetup,
  saveSetting,
  sessionEnd,
  sessionPause,
  sessionResume,
  sessionStart,
  simulateBlockedAttempt,
  simulateBlockedApp,
  simulateRuntimeFailure,
  simulateUnhealthyIntegration,
  testWhitelist,
  type AppSnapshot,
  type SessionSummary,
  type WhitelistTestResult,
  visualDemoSnapshot,
} from "./backend";
import { SetupWizard } from "./SetupWizard";

const empty: AppSnapshot = {
  diagnosticId: "loading",
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
    setupComplete: false,
  },
  health: [],
  whitelist: [],
  tracks: [],
  queue: [],
  history: [],
  playback: {
    playing: false,
    volume: 80,
    health: "waiting_for_output_device",
  },
  blockedApps: [],
};
const time = (seconds: number) =>
  `${Math.floor(seconds / 60)
    .toString()
    .padStart(2, "0")}:${Math.floor(seconds % 60)
    .toString()
    .padStart(2, "0")}`;
const reason = (value: SessionSummary["reason"]) =>
  ({
    completed: "Completed",
    ended_early: "Ended early",
    interrupted: "Interrupted",
    extension_or_app_crash: "Extension/app crashed",
  })[value];

export function App() {
  const [snapshot, setSnapshot] = useState(visualDemoSnapshot ?? empty);
  const [loaded, setLoaded] = useState(Boolean(visualDemoSnapshot));
  const [error, setError] = useState("");
  const active = ["starting", "working", "paused", "ending"].includes(
    snapshot.session.state,
  );
  useEffect(() => {
    if (visualDemoSnapshot) return;
    void appSnapshot()
      .then(setSnapshot)
      .catch(() => setError("Deepify could not load its local data."))
      .finally(() => setLoaded(true));
    let dispose: (() => void) | undefined;
    void onSnapshotChanged(setSnapshot).then((callback) => {
      dispose = callback;
    });
    return () => dispose?.();
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = snapshot.settings.theme;
  }, [snapshot.settings.theme]);
  if (!loaded)
    return (
      <main className="center" aria-live="polite">
        Loading Deepify…
      </main>
    );
  if (!snapshot.settings.setupComplete)
    return (
      <SetupWizard
        health={snapshot.health}
        onComplete={async () => setSnapshot(await completeSetup())}
        onAcceptPairing={async () => setSnapshot(await browserAcceptPairing())}
        onRetryBrowser={async () => setSnapshot(await browserRetryHealth())}
      />
    );
  return (
    <div className="app-shell">
      <header className="topbar">
        <div>
          <p className="eyebrow">DEEPIFY</p>
          <h1>Deepify</h1>
        </div>
        <strong>
          {snapshot.session.state === "not_working"
            ? "Ready"
            : snapshot.session.state === "paused"
              ? "Paused"
              : "Working"}
        </strong>
      </header>
      {!active && (
        <nav className="sidebar" aria-label="Primary">
          {["/", "/library", "/whitelist", "/history", "/settings"].map(
            (path, index) => (
              <NavLink key={path} to={path}>
                {
                  [
                    "Focus Room",
                    "Sound Library",
                    "Whitelist",
                    "Session History",
                    "Settings",
                  ][index]
                }
              </NavLink>
            ),
          )}
        </nav>
      )}
      <section className="simulation" role="status">
        <strong>
          Website protection is real only for a paired, healthy Firefox/Zen
          profile.
        </strong>{" "}
        Niri application monitoring and Noctalia Do Not Disturb remain
        simulated.
      </section>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <main className="content">
        <Routes>
          <Route
            path="/"
            element={
              <FocusRoom
                snapshot={snapshot}
                update={setSnapshot}
                report={setError}
              />
            }
          />
          <Route
            path="/library"
            element={<SoundLibrary snapshot={snapshot} update={setSnapshot} />}
          />
          <Route
            path="/whitelist"
            element={
              <Whitelist
                snapshot={snapshot}
                update={setSnapshot}
                report={setError}
              />
            }
          />
          <Route path="/history" element={<History snapshot={snapshot} />} />
          <Route
            path="/settings"
            element={<Settings snapshot={snapshot} update={setSnapshot} />}
          />
          <Route
            path="*"
            element={
              <FocusRoom
                snapshot={snapshot}
                update={setSnapshot}
                report={setError}
              />
            }
          />
        </Routes>
      </main>
      <footer>
        Local-only data · no account, sign-in, analytics, or cloud service
      </footer>
    </div>
  );
}

type PageProps = {
  snapshot: AppSnapshot;
  update: (value: AppSnapshot) => void;
};
function FocusRoom({
  snapshot,
  update,
  report,
}: PageProps & { report: (message: string) => void }) {
  const [duration, setDuration] = useState(
    String(snapshot.settings.defaultDurationSeconds / 60),
  );
  const [intention, setIntention] = useState("");
  const [trackPath, setTrackPath] = useState("");
  const visualState =
    import.meta.env.DEV && typeof window !== "undefined"
      ? new URLSearchParams(window.location.search).get("demo")
      : null;
  const [ending, setEnding] = useState(visualState === "ending");
  const [validating, setValidating] = useState(visualState === "validation");
  const endTrigger = useRef<HTMLButtonElement>(null);
  const keepWorking = useRef<HTMLButtonElement>(null);
  const wasEnding = useRef(ending);
  useEffect(() => {
    if (ending) keepWorking.current?.focus();
    else if (wasEnding.current) endTrigger.current?.focus();
    wasEnding.current = ending;
  }, [ending]);
  const session = snapshot.session;
  const planned = Number(duration) * 60;
  const valid = Number.isFinite(planned) && planned > 0;
  const start = async () => {
    if (!valid) return report("Enter a duration greater than zero.");
    setValidating(true);
    try {
      update(
        await sessionStart(
          planned,
          intention.trim() || undefined,
          trackPath || undefined,
        ),
      );
    } catch {
      report(
        `Pre-session validation failed. Review browser integration health. Diagnostic ID: ${snapshot.diagnosticId}`,
      );
    } finally {
      setValidating(false);
    }
  };
  if (snapshot.summary)
    return (
      <Summary
        summary={snapshot.summary}
        onDismiss={async () => update(await dismissSummary())}
      />
    );
  if (snapshot.blockedApps.length > 0)
    return (
      <section className="card focus" aria-labelledby="blocked-title">
        <p className="eyebrow">PRE-SESSION CHECK</p>
        <h2 id="blocked-title">Close blocked apps to continue</h2>
        <p>
          Simulated protection found applications outside your next-session
          whitelist:
        </p>
        <ul>
          {snapshot.blockedApps.map((appId) => (
            <li key={appId}>{appId}</li>
          ))}
        </ul>
        <button
          className="primary"
          onClick={async () => update(await resolveBlockedApps())}
        >
          I closed them — check again
        </button>
        <p role="status">No real application was closed or blocked.</p>
      </section>
    );
  if (
    session.state === "working" ||
    session.state === "paused" ||
    session.state === "ending"
  )
    return (
      <section className="card focus" aria-labelledby="focus-title">
        <p className="eyebrow">
          {session.state === "paused" ? "PAUSED" : "WORKING"}
        </p>
        <h2 id="focus-title" className="timer">
          {time(session.remainingSeconds)}
        </h2>
        <p>{session.intention || "Focused work session"}</p>
        {session.state === "paused" && (
          <p className="warning">
            <strong>Restrictions remain active while paused.</strong>
          </p>
        )}
        <div className="controls">
          <button
            className="primary"
            onClick={async () =>
              update(
                await (session.state === "paused"
                  ? sessionResume()
                  : sessionPause()),
              )
            }
          >
            {session.state === "paused" ? "Resume" : "Pause"}
          </button>
          <button ref={endTrigger} onClick={() => setEnding(true)}>
            End session
          </button>
        </div>
        <section aria-label="Sound">
          <h3>Sound</h3>
          <p>
            {snapshot.playback.currentIndex === undefined
              ? "No music selected"
              : (snapshot.queue[snapshot.playback.currentIndex]?.title ??
                "Unavailable track")}
          </p>
          <div className="controls">
            <button
              aria-label="Previous track"
              onClick={async () => update(await audioPrevious())}
            >
              Previous
            </button>
            <button
              aria-label={
                snapshot.playback.playing ? "Pause music" : "Play music"
              }
              onClick={async () => update(await audioToggle())}
            >
              {snapshot.playback.playing ? "Pause" : "Play"}
            </button>
            <button
              aria-label="Next track"
              onClick={async () => update(await audioNext())}
            >
              Next
            </button>
            <label>
              Volume
              <input
                aria-label="Music volume"
                type="range"
                min="0"
                max="100"
                value={snapshot.playback.volume}
                onChange={async (event) =>
                  update(await audioSetVolume(Number(event.target.value)))
                }
              />
            </label>
          </div>
          {snapshot.playback.health !== "ready" && (
            <div className="warning" role="status">
              <p>
                {snapshot.playback.health === "waiting_for_output_device"
                  ? "Waiting for an output device — focus continues without music."
                  : "The selected MP3 is unavailable — focus continues without music."}
              </p>
              {snapshot.playback.health === "waiting_for_output_device" && (
                <button onClick={async () => update(await audioRetryOutput())}>
                  Retry current output device
                </button>
              )}
            </div>
          )}
        </section>
        <section>
          <h3>Focus Shield</h3>
          <p>
            Restrictions active:{" "}
            {session.restrictionsActive ? "Yes — simulated" : "No"} · Blocked
            attempts: {session.blockedAttempts}
          </p>
          <p>{session.latestNotice}</p>
        </section>
        {ending && (
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="end-title"
            aria-describedby="end-description"
            className="dialog"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                setEnding(false);
              }
            }}
          >
            <h3 id="end-title">End this session early?</h3>
            <p id="end-description">
              No reason is required. Cleanup begins immediately.
            </p>
            <button ref={keepWorking} onClick={() => setEnding(false)}>
              Keep working
            </button>
            <button
              className="danger"
              onClick={async () => update(await sessionEnd())}
            >
              End session
            </button>
          </div>
        )}
      </section>
    );
  return (
    <section className="card focus">
      <p className="eyebrow">NOT WORKING</p>
      <h2>Make space for one thing.</h2>
      <label>
        Work duration (minutes) *
        <input
          aria-label="Work duration (minutes)"
          inputMode="numeric"
          value={duration}
          onChange={(event) => setDuration(event.target.value)}
          aria-invalid={!valid}
        />
      </label>
      <label>
        Intention (optional)
        <input
          value={intention}
          onChange={(event) => setIntention(event.target.value)}
        />
      </label>
      <label>
        Playlist (optional)
        <select
          value={trackPath}
          onChange={(event) => setTrackPath(event.target.value)}
        >
          <option value="">No music</option>
          {snapshot.queue.map((track) => (
            <option key={track.path} value={track.path}>
              {track.title}
            </option>
          ))}
        </select>
      </label>
      <button
        className="primary"
        disabled={!valid || validating}
        onClick={() => void start()}
      >
        {validating ? "Checks passed — starting…" : "Start focus session"}
      </button>
      {validating && (
        <section role="status" aria-label="Pre-session validation">
          <h3>Checking your focus setup…</h3>
          <ul>
            {snapshot.health.map((item) => (
              <li key={item.component}>
                {item.component}: {item.status} — {item.detail}
              </li>
            ))}
          </ul>
          <p>Deepify starts automatically when all simulated checks pass.</p>
        </section>
      )}
      <p>
        Focus Shield ready · {snapshot.whitelist.length} explicit whitelist
        rules
      </p>
    </section>
  );
}

function Summary({
  summary,
  onDismiss,
}: {
  summary: SessionSummary;
  onDismiss: () => Promise<void>;
}) {
  return (
    <section className="card summary">
      <p className="eyebrow">SESSION SUMMARY</p>
      <h2>Deep work complete</h2>
      <p className="timer">{time(summary.focusedSeconds)}</p>
      <dl>
        <dt>Blocked attempts</dt>
        <dd>{summary.blockedAttempts}</dd>
        <dt>Finish reason</dt>
        <dd>{reason(summary.reason)}</dd>
        <dt>Cleanup</dt>
        <dd>
          {summary.cleanupComplete
            ? "Simulated restrictions removed"
            : "Incomplete — open recovery"}
        </dd>
      </dl>
      <button className="primary" onClick={() => void onDismiss()}>
        Back to Focus Room
      </button>
    </section>
  );
}

function SoundLibrary({ snapshot, update }: PageProps) {
  return (
    <section className="card">
      <h2>Sound Library</h2>
      <div className="controls">
        <button onClick={async () => update(await importMusicFiles())}>
          Import MP3 files
        </button>
        <button onClick={async () => update(await importMusicFolder())}>
          Import folder
        </button>
      </div>
      <p>
        MP3 only · live folders rescan recursively · shuffle/repeat unavailable
        in MVP
      </p>
      <h3>Tracks</h3>
      {snapshot.tracks.length ? (
        <ul>
          {snapshot.tracks.map((track) => (
            <li key={track.path}>
              {track.artist ? `${track.artist} — ` : ""}
              {track.title} {track.available ? "" : "— Unavailable"}
            </li>
          ))}
        </ul>
      ) : (
        <p>No tracks imported. Music is optional.</p>
      )}
      <h3>Playback queue (saved locally)</h3>
      <ol>
        {snapshot.queue.map((track) => (
          <li key={track.path}>{track.title}</li>
        ))}
      </ol>
    </section>
  );
}

function Whitelist({
  snapshot,
  update,
  report,
}: PageProps & { report: (value: string) => void }) {
  const [kind, setKind] = useState<"application" | "website">("website");
  const [draft, setDraft] = useState("");
  const [testDraft, setTestDraft] = useState("");
  const [testResult, setTestResult] = useState<WhitelistTestResult>();
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    try {
      update(await addWhitelist(kind, draft));
      setDraft("");
    } catch {
      report(
        `Enter a valid domain, IP address, path, or Niri app ID. Nothing was saved. Diagnostic ID: ${snapshot.diagnosticId}`,
      );
    }
  };
  return (
    <section className="card">
      <h2>Whitelist</h2>
      <p>
        Changes apply to the next session. Subdomains are included, paths use
        prefix matching, ports are ignored, and HTTP/HTTPS are equivalent.
      </p>
      <form onSubmit={(event) => void submit(event)}>
        <label>
          Rule type
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value as typeof kind)}
          >
            <option value="website">Website</option>
            <option value="application">Application ID</option>
          </select>
        </label>
        <label>
          Allowed value
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
          />
        </label>
        <button className="primary">Add rule</button>
      </form>
      <ul>
        {snapshot.whitelist.map((item) => (
          <li key={item.id}>
            {item.kind}: {item.value}{" "}
            <button
              onClick={async () => update(await removeWhitelist(item.id))}
            >
              Remove
            </button>
          </li>
        ))}
      </ul>
      <p>
        Implicitly allowed: Deepify · tentative MVP system components · local
        addresses
      </p>
      <p>
        <strong>Unidentified app — allowed in MVP</strong>. Private windows and
        unmonitored browser profiles may bypass website protection; application
        protection remains simulated.
      </p>
      <h3>Test current configuration</h3>
      <label>
        Website or application to test
        <input
          value={testDraft}
          onChange={(event) => {
            setTestDraft(event.target.value);
            setTestResult(undefined);
          }}
        />
      </label>
      <button
        disabled={!testDraft.trim()}
        onClick={async () =>
          setTestResult(await testWhitelist(kind, testDraft.trim()))
        }
      >
        Test configuration
      </button>
      {testResult && (
        <p role="status">
          <strong>{testResult.allowed ? "Allowed" : "Blocked"}</strong> —{" "}
          {testResult.detail}. This test is local and is not saved.
        </p>
      )}
    </section>
  );
}

function History({ snapshot }: { snapshot: AppSnapshot }) {
  const totals = useMemo(() => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const week = new Date(today);
    week.setDate(today.getDate() - ((today.getDay() + 6) % 7));
    return snapshot.history.reduce(
      (result, item) => {
        const started = item.startedAt * 1000;
        if (started >= week.getTime()) result.week += item.focusedSeconds;
        if (started >= today.getTime()) {
          result.today += item.focusedSeconds;
          result.todaySessions += 1;
        }
        return result;
      },
      { today: 0, week: 0, todaySessions: 0 },
    );
  }, [snapshot.history]);
  return (
    <section className="card">
      <h2>Session History</h2>
      <p>
        Today · {time(totals.today)} focused · {totals.todaySessions} sessions
      </p>
      <p>This week · {time(totals.week)} focused</p>
      {snapshot.history.length ? (
        <table>
          <thead>
            <tr>
              <th>Duration</th>
              <th>Intention</th>
              <th>Blocked</th>
              <th>Finish reason</th>
            </tr>
          </thead>
          <tbody>
            {snapshot.history.map((item) => (
              <tr key={item.id}>
                <td>{time(item.focusedSeconds)}</td>
                <td>{item.intention || "—"}</td>
                <td>{item.blockedAttempts}</td>
                <td>{reason(item.reason)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p>No completed sessions yet.</p>
      )}
    </section>
  );
}

function Settings({ snapshot, update }: PageProps) {
  const navigate = useNavigate();
  const active = ["starting", "working", "paused", "ending"].includes(
    snapshot.session.state,
  );
  const [defaultDuration, setDefaultDuration] = useState(
    String(snapshot.settings.defaultDurationSeconds / 60),
  );
  const defaultDurationSeconds = Number(defaultDuration) * 60;
  const validDefaultDuration =
    Number.isFinite(defaultDurationSeconds) && defaultDurationSeconds > 0;
  return (
    <section className="card">
      <h2>Settings</h2>
      <p>
        Local diagnostic ID: <code>{snapshot.diagnosticId}</code>
      </p>
      <h3>Integration health</h3>
      <ul>
        {snapshot.health.map((item) => (
          <li key={item.component}>
            {item.component}: {item.status} · {item.detail}
            {item.component === "Firefox/Zen profile" && !active && (
              <span className="controls">
                <button
                  onClick={async () => update(await browserRetryHealth())}
                >
                  Retry browser
                </button>
                <button
                  onClick={async () => update(await browserForgetPairing())}
                >
                  Forget paired profile
                </button>
              </span>
            )}
          </li>
        ))}
      </ul>
      <button onClick={async () => update(await repairIntegrations())}>
        Rerun simulated health checks
      </button>
      <button
        onClick={async () => update(await simulateUnhealthyIntegration())}
      >
        Simulate unhealthy application-monitor health
      </button>
      <h3>Focus defaults</h3>
      <form
        onSubmit={async (event) => {
          event.preventDefault();
          if (validDefaultDuration) {
            update(
              await saveSetting(
                "default_duration_seconds",
                String(defaultDurationSeconds),
              ),
            );
          }
        }}
      >
        <label>
          Default duration (minutes)
          <input
            inputMode="numeric"
            value={defaultDuration}
            aria-invalid={!validDefaultDuration}
            onChange={(event) => setDefaultDuration(event.target.value)}
          />
        </label>
        <button disabled={!validDefaultDuration}>Save default duration</button>
      </form>
      <label>
        Color scheme
        <select
          value={snapshot.settings.theme}
          onChange={async (event) =>
            update(await saveSetting("theme", event.target.value))
          }
        >
          <option value="obsidian">Obsidian dark</option>
          <option value="mist">Mist</option>
        </select>
      </label>
      <label>
        <input
          type="checkbox"
          checked={snapshot.settings.notifications}
          onChange={async (event) =>
            update(
              await saveSetting("notifications", String(event.target.checked)),
            )
          }
        />{" "}
        Session completion notifications
      </label>
      <h3>Recovery and privacy</h3>
      <button
        onClick={async () => {
          update(await rerunSetup());
          navigate("/");
        }}
      >
        Run setup wizard again
      </button>
      <p>
        Emergency recovery retries unresolved cleanup before another session may
        start.
      </p>
      <h3>Developer simulation</h3>
      <button onClick={async () => update(await simulateBlockedApp())}>
        Simulate blocked app at preflight
      </button>
      <button onClick={async () => update(await simulateBlockedAttempt())}>
        Simulate blocked attempt
      </button>
      <button onClick={async () => update(await simulateRuntimeFailure())}>
        Simulate runtime failure
      </button>
    </section>
  );
}
