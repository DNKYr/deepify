import { useState } from "react";
import type { Health } from "./backend";

const steps = ["Integrations", "Browser", "Whitelist", "Music", "Ready"];
export function SetupWizard({
  health,
  onComplete,
  onAcceptPairing,
  onRetryBrowser,
}: {
  health: Health[];
  onComplete: () => Promise<void>;
  onAcceptPairing: () => Promise<void>;
  onRetryBrowser: () => Promise<void>;
}) {
  const [step, setStep] = useState(0);
  const [musicSkipped, setMusicSkipped] = useState(false);
  const healthy =
    health.length === 0 || health.every((item) => item.status === "healthy");
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
          paired and healthy. Niri and Noctalia remain simulated.
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
                  {item.component}: {item.status}
                </li>
              ))}
            </ul>
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
                    onClick={() => void onAcceptPairing()}
                  >
                    Pair this profile
                  </button>
                )}
                {browser?.status !== "healthy" && (
                  <button onClick={() => void onRetryBrowser()}>
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
        <button
          className="primary"
          disabled={step === 0 && !healthy}
          onClick={() => void next()}
        >
          {step === 4 ? "Open Focus Room" : "Continue"}
        </button>
      </section>
    </main>
  );
}
