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
  refreshMusicLibrary,
  removeWhitelist,
  repairIntegrations,
  resolveBlockedApps,
  rerunSetup,
  saveSetting,
  sessionEnd,
  sessionPause,
  sessionResume,
  sessionStart,
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
  if (!snapshot.settings.setupComplete && !active)
    return (
      <SetupWizard
        health={snapshot.health}
        onComplete={async () => setSnapshot(await completeSetup())}
        onAcceptPairing={async () => setSnapshot(await browserAcceptPairing())}
        onRetryBrowser={async () => setSnapshot(await browserRetryHealth())}
        onRetryIntegrations={async () =>
          setSnapshot(await repairIntegrations())
        }
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
        {visualDemoSnapshot
          ? "Visual preview — all protection shown here is simulated."
          : "Sessions also require Niri window monitoring and Noctalia Do Not Disturb."}
      </section>
      <main className="content">
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        {(snapshot.unidentifiedAppCount ?? 0) > 0 && (
          <p role="status">
            Unidentified app — allowed in MVP: {snapshot.unidentifiedAppCount}
          </p>
        )}
        {!active && snapshot.session.latestNotice && (
          <p className="warning" role="alert">
            {snapshot.session.latestNotice}
          </p>
        )}
        {snapshot.health.some((item) => item.status === "unhealthy") && (
          <section
            className="warning"
            role="status"
            aria-label="Integration health"
          >
            {snapshot.health
              .filter((item) => item.status === "unhealthy")
              .map((item) => (
                <p key={item.component}>
                  {item.component}: {item.detail}
                </p>
              ))}
          </section>
        )}
        <Routes location={active ? "/" : undefined}>
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
  const valid = Number.isInteger(planned) && planned > 0 && planned <= 86400;
  const start = async () => {
    if (!valid)
      return report("Enter a duration between one second and 24 hours.");
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
      update(await appSnapshot());
      report(
        `Pre-session validation failed. Review the open applications and integration health. Diagnostic ID: ${snapshot.diagnosticId}`,
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
  if (snapshot.blockedApps.length > 0 && session.state === "not_working")
    return (
      <section className="card focus" aria-labelledby="blocked-title">
        <p className="eyebrow">PRE-SESSION CHECK</p>
        <h2 id="blocked-title">Close blocked apps to continue</h2>
        <p>
          These applications are outside your next-session whitelist. Close
          their windows or allow an application for your next session:
        </p>
        <ul>
          {snapshot.blockedApps.map((appId) => (
            <li key={appId}>
              {appId}{" "}
              <button
                onClick={async () => {
                  await addWhitelist("application", appId);
                  update(await resolveBlockedApps());
                }}
              >
                Allow {appId}
              </button>
            </li>
          ))}
        </ul>
        <button
          className="primary"
          onClick={async () => update(await resolveBlockedApps())}
        >
          I closed them — check again
        </button>
        <p role="status">
          Deepify waits for you to resolve these windows before starting.
        </p>
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
            {snapshot.playback.currentIndex == null
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
            Restrictions active: {session.restrictionsActive ? "Yes" : "No"} ·
            Blocked attempts: {session.blockedAttempts}
          </p>
          <p>{session.latestNotice}</p>
          {snapshot.blockedApps.length > 0 && (
            <div className="warning" role="alert">
              <p>
                Close these applications manually:{" "}
                {snapshot.blockedApps.join(", ")}. They remained open after a
                close request. Save any unsaved work; Deepify will not force
                them to quit.
              </p>
            </div>
          )}
        </section>
        {ending && (
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="end-title"
            aria-describedby="end-description"
            className="dialog"
            onKeyDown={(event) => {
              if (event.key === "Tab") {
                const buttons =
                  event.currentTarget.querySelectorAll<HTMLButtonElement>(
                    "button",
                  );
                const target = event.shiftKey
                  ? buttons[buttons.length - 1]
                  : buttons[0];
                if (
                  (event.shiftKey && document.activeElement === buttons[0]) ||
                  (!event.shiftKey &&
                    document.activeElement === buttons[buttons.length - 1])
                ) {
                  event.preventDefault();
                  target?.focus();
                }
              }
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
          <p>Deepify starts automatically when all integration checks pass.</p>
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
      <h2>
        {summary.reason === "completed"
          ? "Deep work complete"
          : "Session ended"}
      </h2>
      <p className="timer">{time(summary.focusedSeconds)}</p>
      <dl>
        <dt>Blocked attempts</dt>
        <dd>{summary.blockedAttempts}</dd>
        <dt>Finish reason</dt>
        <dd>{reason(summary.reason)}</dd>
        <dt>Cleanup</dt>
        <dd>
          {summary.cleanupComplete
            ? "Restrictions removed"
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
  const [error, setError] = useState("");
  useEffect(() => {
    let mounted = true;
    void refreshMusicLibrary()
      .then((value) => {
        if (mounted) update(value);
      })
      .catch(() => {
        if (mounted)
          setError("The library could not refresh. Reopen it to try again.");
      });
    return () => {
      mounted = false;
    };
  }, [update]);
  const importSource = async (action: typeof importMusicFiles) => {
    setError("");
    try {
      update(await action());
    } catch {
      setError(
        "Music could not be imported. Check that the selected files or folder are accessible.",
      );
    }
  };
  return (
    <section className="card">
      <h2>Sound Library</h2>
      <div className="controls">
        <button onClick={() => void importSource(importMusicFiles)}>
          Import MP3 files
        </button>
        <button onClick={() => void importSource(importMusicFolder)}>
          Import folder
        </button>
      </div>
      {error && <p role="alert">{error}</p>}
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
        Implicitly allowed: Deepify · Firefox and Zen · desktop shell,
        authentication and portal dialogs · local addresses
      </p>
      <p>
        <strong>Unidentified app — allowed in MVP</strong>. Private windows and
        unmonitored browser profiles may bypass website protection. Terminal
        commands are outside window-based enforcement.
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
  const [error, setError] = useState("");
  const [forgetting, setForgetting] = useState(false);
  const forgetTrigger = useRef<HTMLButtonElement>(null);
  const cancelForget = useRef<HTMLButtonElement>(null);
  const wasForgetting = useRef(false);
  useEffect(() => {
    if (forgetting) cancelForget.current?.focus();
    else if (wasForgetting.current) forgetTrigger.current?.focus();
    wasForgetting.current = forgetting;
  }, [forgetting]);
  const apply = async (action: () => Promise<AppSnapshot>) => {
    try {
      update(await action());
      setError("");
      return true;
    } catch {
      setError(
        "The setting could not be changed. Check integration health and try again.",
      );
      return false;
    }
  };
  const active = ["starting", "working", "paused", "ending"].includes(
    snapshot.session.state,
  );
  const [defaultDuration, setDefaultDuration] = useState(
    String(snapshot.settings.defaultDurationSeconds / 60),
  );
  const defaultDurationSeconds = Number(defaultDuration) * 60;
  const validDefaultDuration =
    Number.isInteger(defaultDurationSeconds) &&
    defaultDurationSeconds > 0 &&
    defaultDurationSeconds <= 86400;
  return (
    <section className="card">
      <h2>Settings</h2>
      {error && <p role="alert">{error}</p>}
      {forgetting && (
        <div
          className="dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="forget-title"
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              setForgetting(false);
            }
            if (event.key === "Tab") {
              const buttons =
                event.currentTarget.querySelectorAll<HTMLButtonElement>(
                  "button",
                );
              if (
                (event.shiftKey && document.activeElement === buttons[0]) ||
                (!event.shiftKey && document.activeElement === buttons[1])
              ) {
                event.preventDefault();
                buttons[event.shiftKey ? 1 : 0]?.focus();
              }
            }
          }}
        >
          <h3 id="forget-title">Forget this browser profile?</h3>
          <p>
            You will need to pair a browser again before starting another
            session.
          </p>
          <button ref={cancelForget} onClick={() => setForgetting(false)}>
            Keep paired profile
          </button>
          <button
            className="danger"
            onClick={async () => {
              if (await apply(browserForgetPairing)) setForgetting(false);
            }}
          >
            Forget profile
          </button>
        </div>
      )}
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
                <button onClick={() => void apply(browserRetryHealth)}>
                  Retry browser
                </button>
                <button ref={forgetTrigger} onClick={() => setForgetting(true)}>
                  Forget paired profile
                </button>
              </span>
            )}
          </li>
        ))}
      </ul>
      <button onClick={() => void apply(repairIntegrations)}>
        Rerun integration health checks
      </button>
      <h3>Focus defaults</h3>
      <form
        onSubmit={async (event) => {
          event.preventDefault();
          if (validDefaultDuration) {
            await apply(() =>
              saveSetting(
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
            apply(() => saveSetting("theme", event.target.value))
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
            await apply(() =>
              saveSetting("notifications", String(event.target.checked)),
            )
          }
        />{" "}
        Session completion notifications
      </label>
      <h3>Recovery and privacy</h3>
      <button
        onClick={async () => {
          if (await apply(rerunSetup)) navigate("/");
        }}
      >
        Run setup wizard again
      </button>
      <p>
        Emergency recovery retries unresolved cleanup before another session may
        start.
      </p>
    </section>
  );
}
