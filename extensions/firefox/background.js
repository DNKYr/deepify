/* Phase 2 only: transport health is represented, navigation is not blocked. */
const state = { simulated: true, connected: false, sessionActive: false };
browser.runtime.onMessage.addListener(message => {
  if (message?.type === 'status') return Promise.resolve({ ...state });
  return undefined;
});
