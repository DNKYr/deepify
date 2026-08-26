const params = new URLSearchParams(location.search);
const tabId = Number(params.get("tab"));
const originalUrl = params.get("url");
document.querySelector("#destination").textContent =
  `Blocked: ${originalUrl || "unknown destination"}`;
document.querySelector("#restore").addEventListener("click", () => {
  browser.runtime.sendMessage({ type: "restore", tabId });
});
