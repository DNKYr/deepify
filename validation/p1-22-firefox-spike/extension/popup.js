browser.runtime.sendMessage({ type: "debug_state" }).then((state) => {
  document.querySelector("#status").textContent = state.active
    ? `Active; ${state.blocked.length} blocked tab(s)` : "Inactive";
});
