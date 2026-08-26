import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type BackendSession = { state: 'not_working' | 'working' | 'paused'; remaining: number; focused: number; blocked_attempts: number; restrictions_active: boolean };
export const sessionStart = (seconds: number, intention?: string) => invoke<BackendSession>('session_start', { seconds, intention });
export const sessionPause = () => invoke<BackendSession>('session_pause');
export const sessionResume = () => invoke<BackendSession>('session_resume');
export const sessionEnd = () => invoke<unknown>('session_end');
export const onSessionChanged = (callback: (snapshot: BackendSession) => void) => listen<BackendSession>('session://changed', event => callback(event.payload));
