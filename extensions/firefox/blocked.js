/* global browser */
const formatRemaining = (seconds) => `${Math.floor(seconds / 60).toString().padStart(2, "0")}:${Math.floor(seconds % 60).toString().padStart(2, "0")}`;

async function render() {
  try {
    const state = await browser.runtime.sendMessage({ type: "deepify-blocked-state" });
    document.querySelector("#destination").textContent = state.destination;
    document.querySelector("#remaining").textContent = state.timerState === "inactive" ? "Finishing restoration…" : formatRemaining(state.remainingSeconds);
  } catch {
    document.querySelector("#destination").textContent = "website";
    document.querySelector("#remaining").textContent = "Checking session state…";
  } finally {
    document.querySelector("main").focus();
  }
}
document.querySelector("#new-tab").addEventListener("click", () => browser.tabs.create({}));
void render();
