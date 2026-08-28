import { useState } from "react";
import type { Health } from "./backend";

const steps = ["Integrations", "Browser", "Whitelist", "Music", "Ready"];
export function SetupWizard({
  health,
  onComplete,
}: {
  health: Health[];
  onComplete: () => Promise<void>;
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
          <strong>Simulated protection:</strong> Browser, Niri, and Noctalia
          checks do not enforce restrictions in Phase 2.
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
        {step === 1 && (
          <p>
            One simulated Firefox or Zen Browser profile is paired and healthy.
          </p>
        )}
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
