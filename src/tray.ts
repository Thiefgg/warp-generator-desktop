import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

const win = getCurrentWindow();

async function fit() {
  const menu = document.querySelector<HTMLElement>(".tray-menu");
  if (!menu) return;
  const r = menu.getBoundingClientRect();
  const pad = getComputedStyle(document.body);
  const side = parseFloat(pad.paddingLeft) * 2;
  const bottom = parseFloat(pad.paddingBottom);
  await win.setSize(new LogicalSize(Math.ceil(r.width + side), Math.ceil(r.height + bottom)));
}

const statusText = document.querySelector<HTMLElement>("#tray-status-text");
const statusRow = document.querySelector<HTMLElement>("#tray-status");
const disconnect = document.querySelector<HTMLElement>("#tray-disconnect");
const quietBox = document.querySelector<HTMLElement>("#tray-quiet-box");
const quietRow = document.querySelector<HTMLElement>("#tray-quiet");

let quiet = localStorage.getItem("warp-tray-quiet") !== "0";

function paint() {
  quietBox?.classList.toggle("on", quiet);
  quietRow?.classList.toggle("checked", quiet);
}

function click(el: Element | null, fn: () => void) {
  if (!el) return;
  el.addEventListener("click", () => {
    fn();
    win.hide();
  });
}

async function refresh() {
  try {
    const s = await invoke<{ running: boolean }>("vpn_get_status");
    if (statusText) statusText.textContent = s.running ? "VPN: подключен" : "VPN: отключен";
    statusRow?.classList.toggle("on", s.running);
    disconnect?.classList.toggle("disabled", !s.running);
  } catch {
    if (statusText) statusText.textContent = "VPN: недоступно";
  }
}

click(document.querySelector("#tray-open"), () => {
  void invoke("tray_show_main");
});

click(disconnect, () => {
  void invoke("vpn_disconnect");
});

click(document.querySelector("#tray-quit"), () => {
  void invoke("tray_quit");
});

quietRow?.addEventListener("click", () => {
  quiet = !quiet;
  localStorage.setItem("warp-tray-quiet", quiet ? "1" : "0");
  paint();
});

win.onFocusChanged(async ({ payload }) => {
  if (payload) {
    await refresh();
    paint();
  }
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") void win.hide();
});

paint();
void refresh();
void fit();