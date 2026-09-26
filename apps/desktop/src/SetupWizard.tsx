import { useState } from "react";
import type { Health } from "./backend";

const steps = ["Integrations", "Browser", "Whitelist", "Music", "Ready"];
export function SetupWizard({
  health,
  onComplete,
  onAcceptPairing,
  onRetryBrowser,
  onRetryIntegrations,
}: {
  health: Health[];
  onComplete: () => Promise<void>;
  onAcceptPairing: () => Promise<void>;
  onRetryBrowser: () => Promise<void>;
  onRetryIntegrations: () => Promise<void>;
}) {
  const [step, setStep] = useState(0);
  const [error, setError] = useState("");
  const run = async (action: () => Promise<void>) => {
    try {
      setError("");
      await action();
    } catch {
      setError("The integration is not ready. Check the connection and retry.");
    }
  };
  const [musicSkipped, setMusicSkipped] = useState(false);
  const healthy =
    health.length > 0 && health.every((item) => item.status === "healthy");
  const next = async () => {
    if (step === 4) await onComplete();
    else setStep((value) => value + 1);
  };
  return (
    <main className="setup">
      <section className="card" aria-labelledby="setup-title">
        <p className="eyebrow">SET UP DEEPIFY · STEP {step + 1} OF 5</p>
        <h1 id="setup-title">{steps[step]}</h1>
        <p>
          Website protection becomes real only after a Firefox or Zen profile is
          paired and healthy. Sessions also monitor Niri windows and enable
          Noctalia Do Not Disturb, restoring its prior setting afterward.
        </p>
        {step === 0 && (
          <>
            <p>
              Deepify stores everything locally and requires no account or
              internet connection.
            </p>
            <ul>
              {health.map((item) => (
                <li key={item.component}>
                  {item.component}: {item.status} — {item.detail}
                </li>
              ))}
            </ul>
            <p>
              Run Deepify in your Niri desktop session with Noctalia Shell
              running. Desktop access uses your existing user session; no
              administrator permission is needed.
            </p>
            <button onClick={() => void run(onRetryIntegrations)}>
              Check desktop integrations
            </button>
            <p>
              Known MVP limits: private windows, other profiles, unidentified
              apps, and terminal-hosted commands may bypass protection.
            </p>
          </>
        )}
        {step === 1 &&
          (() => {
            const browser = health.find(
              (item) => item.component === "Firefox/Zen profile",
            );
            return (
              <>
                <p>
                  {browser?.detail ??
                    "Connect the Deepify companion extension."}
                </p>
                <p>
                  Private windows and other browser profiles are unmonitored
                  bypasses. Container tabs use the paired profile’s shared
                  policy.
                </p>
                {browser?.detail === "Pairing required" && (
                  <button
                    className="primary"
                    onClick={() => void run(onAcceptPairing)}
                  >
                    Pair this profile
                  </button>
                )}
                {browser?.status !== "healthy" && (
                  <button onClick={() => void run(onRetryBrowser)}>
                    Retry connection
                  </button>
                )}
              </>
            );
          })()}
        {step === 2 && (
          <p>
            Create application and website rules after setup. Malformed rules
            are never saved.
          </p>
        )}
        {step === 3 && (
          <>
            <p>MP3 music is optional.</p>
            <button
              onClick={() => {
                setMusicSkipped(true);
                setStep(4);
              }}
            >
              Skip music
            </button>
          </>
        )}
        {step === 4 && (
          <p>
            Setup is ready. Music: {musicSkipped ? "Skipped" : "No imports yet"}
            . You can rerun every step from Settings.
          </p>
        )}
        {error && <p role="alert">{error}</p>}
        <button
          className="primary"
          disabled={step === 4 && !healthy}
          onClick={() => void run(next)}
        >
          {step === 4 ? "Open Focus Room" : "Continue"}
        </button>
      </section>
    </main>
  );
}
