import { useState } from 'react';

const steps = ['Welcome', 'Browser connection', 'Application checks', 'Sound library', 'Ready'];
/** Phase 2 setup is deliberately explicit about simulated integrations. */
export function SetupWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState(0);
  const next = () => step === steps.length - 1 ? onComplete() : setStep(value => value + 1);
  return <section aria-labelledby="setup-title" className="card">
    <p className="eyebrow">SETUP · STEP {step + 1} OF {steps.length}</p>
    <h2 id="setup-title">{steps[step]}</h2>
    <p className="notice">Browser, Niri, and Noctalia checks are simulated in this Phase 2 prototype.</p>
    {step === 3 && <button onClick={next}>Skip music for now</button>}
    <button className="primary" onClick={next}>{step === steps.length - 1 ? 'Finish setup' : 'Continue'}</button>
  </section>;
}
