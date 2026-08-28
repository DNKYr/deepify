const parameters = new URLSearchParams(globalThis.location.search);
const destination = parameters.get("destination");
const remaining = parameters.get("remaining");
if (destination)
  document.querySelector("#destination").textContent = destination;
if (remaining) document.querySelector("#remaining").textContent = remaining;
document.querySelector("#new-tab").addEventListener("click", () => {
  if (globalThis.browser?.tabs) browser.tabs.create({});
});
