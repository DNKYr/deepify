/* global browser */
const formatRemaining = (seconds) => `${Math.floor(seconds / 60).toString().padStart(2, "0")}:${Math.floor(seconds % 60).toString().padStart(2, "0")}`;

async function render() {
  try {
    const state = await browser.runtime.sendMessage({ type: "deepify-blocked-state" });
    document.querySelector("#destination").textContent = state.destination;
    document.querySelector("#remaining").textContent = state.timerState === "inactive" ? "Session ended" : `${formatRemaining(state.remainingSeconds)}${state.timerState === "paused" ? " · Paused" : ""}`;
    document.querySelector(".notice").textContent = state.timerState === "inactive"
      ? "Restrictions have ended. If this page remains open, retry cleanup in Deepify."
      : "Restrictions are active. Your original page will return when the session ends.";
  } catch {
    document.querySelector("#destination").textContent = "website";
    document.querySelector("#remaining").textContent = "Checking session state…";
  }
}
document.querySelector("#new-tab").addEventListener("click", () => browser.tabs.create({}));
document.querySelector("main").focus();
void render();
// The desktop owns elapsed time; this only refreshes its latest snapshot.
setInterval(() => { void render(); }, 1000);
