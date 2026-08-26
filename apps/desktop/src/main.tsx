import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';
import { onSessionChanged, sessionEnd, sessionPause, sessionResume, sessionStart } from './backend';

type State = 'not_working' | 'working' | 'paused' | 'summary';
type Summary = { focused: number; blocked: number; reason: string };

const formatTime = (seconds: number) => `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;

function App() {
  const [state, setState] = useState<State>('not_working');
  const [duration, setDuration] = useState('25');
  const [intention, setIntention] = useState('');
  const [remaining, setRemaining] = useState(0);
  const [notice, setNotice] = useState('');
  const [summary, setSummary] = useState<Summary | null>(null);
  const [theme, setTheme] = useState(localStorage.getItem('deepify.theme') ?? 'obsidian');
  const validDuration = Number(duration) > 0;
  useEffect(() => { localStorage.setItem('deepify.theme', theme); document.documentElement.dataset.theme = theme; }, [theme]);
  useEffect(() => { let stop: (() => void) | undefined; onSessionChanged(snapshot => { setRemaining(snapshot.remaining); setState(snapshot.state); }).then(unlisten => { stop = unlisten; }); return () => stop?.(); }, []);
  const status = useMemo(() => state === 'working' ? 'Working' : state === 'paused' ? 'Paused' : state === 'summary' ? 'Session complete' : 'Not working', [state]);
  const start = async () => { if (!validDuration) { setNotice('Enter a duration greater than zero.'); return; } try { await sessionStart(Number(duration) * 60, intention || undefined); setNotice('Simulated preflight passed. Restriction protection is simulated in Phase 2.'); } catch { setNotice('Session could not start. Check simulated integration health.'); } };
  const end = async () => { if (!window.confirm('End this session early?')) return; await sessionEnd(); setSummary({ focused: Number(duration) * 60 - remaining, blocked: 0, reason: 'Ended early' }); setState('summary'); };
  const togglePause = async () => { try { await (state === 'paused' ? sessionResume() : sessionPause()); } catch { setNotice('The session command failed; backend state was preserved.'); } };
  return <main className="shell"><header><div><p className="eyebrow">DEEPIFY</p><h1>Focus Room</h1></div><nav aria-label="Primary"><button onClick={() => setNotice('History is local to this device.')}>History</button><label>Theme <select value={theme} onChange={event => setTheme(event.target.value)}><option value="obsidian">Obsidian</option><option value="mist">Mist</option></select></label></nav></header>
    <section className="health" role="status"><span className="dot"/>Simulated protection · Browser, app, and Do Not Disturb integrations are not enforced in Phase 2</section>
    {state === 'summary' && summary ? <section className="card summary" aria-labelledby="summary-title"><p className="eyebrow">SESSION SUMMARY</p><h2 id="summary-title">Deep work complete</h2><p>{formatTime(summary.focused)} focused · {summary.blocked} blocked attempts · {summary.reason}</p><button className="primary" onClick={() => { setState('not_working'); setSummary(null); }}>Back to Focus Room</button></section> : <section className="card focus" aria-labelledby="state-title"><p className="eyebrow">{status.toUpperCase()}</p><h2 id="state-title">{state === 'not_working' ? 'Make space for one thing.' : formatTime(remaining)}</h2>{state === 'not_working' ? <><label>Work duration (minutes)<input inputMode="numeric" value={duration} onChange={event => setDuration(event.target.value)} aria-invalid={!validDuration}/></label><label>Intention <input value={intention} onChange={event => setIntention(event.target.value)} placeholder="What will you focus on?"/></label><button className="primary" onClick={start}>Start focus</button></> : <div className="controls"><button onClick={togglePause}>{state === 'paused' ? 'Resume' : 'Pause'}</button><button onClick={end}>End session</button></div>}</section>}
    {notice && <p className="notice" role="alert">{notice}</p>}<footer>Local-only prototype · MP3 library and settings are available from the next screens.</footer></main>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
