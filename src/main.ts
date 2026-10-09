import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { save } from "@tauri-apps/plugin-dialog";

type PageName = "generator" | "vpn" | "ip" | "history" | "settings";

interface HistoryItem { id: string; date: string; format: string; dns: string; mode: string; config: string; }

interface GenerateOptions {
  format: string; connection: string; dns: string; endpoint: string;
  excludeLan: boolean; ipv6: string; keepalive: number; mtu: number; customI1Domain: string | null;
  profile: string | null;
}

interface SavedSettings {
  autoHistory: boolean; excludeLan: boolean; endpointCustom: string;
  customI1: string; mtuCustom: string; selects: Record<string, string>;
}

type IpInfo = {
  ip?: string;
  type?: string;
  country?: string;
  city?: string;
  connection?: { isp?: string; org?: string; asn?: number };
  timezone?: { id?: string };
  success?: boolean;
  message?: string;
};

const ENDPOINTS: Record<string, string> = {
  default: "162.159.192.1:2408",
  default2: "engage.cloudflareclient.com:2408",
  fra1: "188.114.97.66:2408",
  fra2: "188.114.96.125:2408",
};

const RANDOM_POOL: string[] = [
  "162.159.192.1:2408", "162.159.193.5:2408", "162.159.195.1:2408",
  "162.159.204.1:2408", "188.114.96.125:2408", "188.114.97.66:2408",
];

const SELECT_IDS = ["config-format", "connection", "dns", "endpoint", "ipv6", "mtu", "keepalive", "profile"];

const appWindow = getCurrentWindow();

const navItems = document.querySelectorAll<HTMLButtonElement>(".nav-item");
const pages = document.querySelectorAll<HTMLElement>(".page");
const generatorForm = document.querySelector<HTMLFormElement>("#generator-form");
const advancedToggle = document.querySelector<HTMLButtonElement>("#advanced-toggle");
const advancedContent = document.querySelector<HTMLElement>("#advanced-content");
const previewFormat = document.querySelector<HTMLElement>("#preview-format");
const previewDns = document.querySelector<HTMLElement>("#preview-dns");
const historyList = document.querySelector<HTMLElement>("#history-list");
const clearHistory = document.querySelector<HTMLButtonElement>("#clear-history");
const autoHistory = document.querySelector<HTMLInputElement>("#auto-history");
const excludeLan = document.querySelector<HTMLInputElement>("#exclude-lan");
const minimizeButton = document.querySelector<HTMLButtonElement>("#window-minimize");
const closeButton = document.querySelector<HTMLButtonElement>("#window-close");
const endpointCustomField = document.querySelector<HTMLElement>("#endpoint-custom-field");
const endpointCustomInput = document.querySelector<HTMLInputElement>("#endpoint-custom");
const customI1Input = document.querySelector<HTMLInputElement>("#custom-i1");
const customI1Error = document.querySelector<HTMLElement>("#custom-i1-error");
const mtuCustomField = document.querySelector<HTMLElement>("#mtu-custom-field");
const mtuCustomInput = document.querySelector<HTMLInputElement>("#mtu-custom");
const mtuCustomError = document.querySelector<HTMLElement>("#mtu-custom-error");
const resultModal = document.querySelector<HTMLElement>("#result-modal");
const modalFilename = document.querySelector<HTMLElement>("#modal-filename");
const modalClose = document.querySelector<HTMLButtonElement>("#modal-close");
const modalOpen = document.querySelector<HTMLButtonElement>("#modal-open");
const modalSave = document.querySelector<HTMLButtonElement>("#modal-save");
const modalCopy = document.querySelector<HTMLButtonElement>("#modal-copy");
const guideModal = document.querySelector<HTMLElement>("#guide-modal");
const guideFilename = document.querySelector<HTMLElement>("#guide-filename");
const guideClose = document.querySelector<HTMLButtonElement>("#guide-close");
const guideOk = document.querySelector<HTMLButtonElement>("#guide-ok");
const confirmModal = document.querySelector<HTMLElement>("#confirm-modal");
const confirmClose = document.querySelector<HTMLButtonElement>("#confirm-close");
const confirmYes = document.querySelector<HTMLButtonElement>("#confirm-yes");
const confirmNo = document.querySelector<HTMLButtonElement>("#confirm-no");
const errorModal = document.querySelector<HTMLElement>("#error-modal");
const errorTitle = document.querySelector<HTMLElement>("#error-title");
const errorSubtitle = document.querySelector<HTMLElement>("#error-subtitle");
const errorMessage = document.querySelector<HTMLElement>("#error-message");
const errorClose = document.querySelector<HTMLButtonElement>("#error-close");
const errorOk = document.querySelector<HTMLButtonElement>("#error-ok");

const vpnConnectBtn = document.querySelector<HTMLButtonElement>("#vpn-connect-btn");
const vpnStatusText = document.querySelector<HTMLElement>("#vpn-status-text");
const vpnStatusTextSmall = document.querySelector<HTMLElement>("#vpn-status-text-small");
const vpnStatusHint = document.querySelector<HTMLElement>("#vpn-status-hint");
const vpnStatusEl = document.querySelector<HTMLElement>("#vpn-status");
const vpnStatsEl = document.querySelector<HTMLElement>("#vpn-stats");
const vpnTimeEl = document.querySelector<HTMLElement>("#vpn-time");
const vpnRxEl = document.querySelector<HTMLElement>("#vpn-rx");
const vpnTxEl = document.querySelector<HTMLElement>("#vpn-tx");
const vpnIfaceEl = document.querySelector<HTMLElement>("#vpn-iface");
const vpnEndpointEl = document.querySelector<HTMLElement>("#vpn-endpoint");
const vpnPanel = document.querySelector<HTMLElement>("#vpn-panel");

let appHistory: HistoryItem[] = loadHistory();
let currentConfig = "";
let currentFileName = "";
let ipLoading = false;
let vpnState: "disconnected" | "connecting" | "connected" = "disconnected";
let vpnStartTime = 0;

function getCustomValue(id: string): string {
  return document.querySelector<HTMLInputElement>(`#${id}`)?.value ?? "";
}

function setCustomValue(id: string, value: string, label: string) {
  const input = document.querySelector<HTMLInputElement>(`#${id}`);
  const select = document.querySelector<HTMLElement>(`.custom-select[data-select-id="${id}"]`);
  if (!input || !select) return;

  input.value = value;
  const valueEl = select.querySelector<HTMLElement>(".custom-select-value");
  if (valueEl) valueEl.textContent = label;

  select.querySelectorAll<HTMLButtonElement>(".custom-option").forEach((opt) => {
    opt.classList.toggle("selected", opt.dataset.value === value);
  });
}

function closeAllCustomSelects(except?: HTMLElement) {
  document.querySelectorAll<HTMLElement>(".custom-select.open").forEach((s) => {
    if (s !== except) closeCustomSelect(s);
  });
}

function openCustomSelect(select: HTMLElement) {
  closeAllCustomSelects(select);
  select.classList.add("open");
  select.querySelector<HTMLButtonElement>(".custom-select-trigger")?.setAttribute("aria-expanded", "true");
}

function closeCustomSelect(select: HTMLElement) {
  select.classList.remove("open");
  select.querySelector<HTMLButtonElement>(".custom-select-trigger")?.setAttribute("aria-expanded", "false");
}

function updateProfileVisibility() {
  const field = document.querySelector<HTMLElement>("#profile-field");
  if (!field) return;
  const on = getCustomValue("connection") === "sakeen";
  field.toggleAttribute("hidden", !on);
  if (on) {
    const val = getCustomValue("config-format");
    if (val !== "sakeen") setCustomValue("config-format", "sakeen", "Sakeen");
  }
}

function updateFormatFromConnection() {
  const conn = getCustomValue("connection");
  const fmt = getCustomValue("config-format");
  if (conn === "sakeen" && fmt !== "sakeen") setCustomValue("config-format", "sakeen", "Sakeen");
  if (conn === "wireguard" && fmt === "sakeen") setCustomValue("config-format", "wireguard", "WireGuard");
  if (conn.startsWith("amneziawg") && fmt === "sakeen") setCustomValue("config-format", "amneziawg", "AmneziaWG");
  updateProfileVisibility();
}

function updateEndpointVisibility() {
  if (!endpointCustomField) return;
  endpointCustomField.toggleAttribute("hidden", getCustomValue("endpoint") !== "custom");
}

function updateMtuVisibility() {
  if (!mtuCustomField) return;
  mtuCustomField.toggleAttribute("hidden", getCustomValue("mtu") !== "custom");
}

function validateMtu(raw: string): string | null {
  if (raw.trim().length === 0) return "Введи значение";
  const n = parseInt(raw, 10);
  if (!Number.isFinite(n)) return "Должно быть число";
  if (n < 576) return "Минимум 576";
  if (n > 1500) return "Максимум 1500";
  return null;
}

function initCustomSelects() {
  document.querySelectorAll<HTMLElement>(".custom-select").forEach((select) => {
    const id = select.dataset.selectId;
    if (!id) return;

    const trigger = select.querySelector<HTMLButtonElement>(".custom-select-trigger");
    const options = select.querySelectorAll<HTMLButtonElement>(".custom-option");

    trigger?.addEventListener("click", (event) => {
      event.stopPropagation();
      select.classList.contains("open") ? closeCustomSelect(select) : openCustomSelect(select);
    });

    options.forEach((option) => {
      option.addEventListener("click", (event) => {
        event.stopPropagation();
        const value = option.dataset.value;
        if (!value) return;

        setCustomValue(id, value, option.textContent?.trim() ?? "");
        closeCustomSelect(select);
        updatePreview();
        saveSettings();

        if (id === "endpoint") updateEndpointVisibility();
        if (id === "mtu") updateMtuVisibility();
        if (id === "connection") updateFormatFromConnection();
        if (id === "config-format") updateProfileVisibility();
      });
    });
  });
}

function switchPage(pageName: PageName) {
  closeAllCustomSelects();
  hideResultModal();
  hideGuideModal();
  hideConfirmModal();
  hideErrorModal();

  navItems.forEach((item) => item.classList.toggle("active", item.dataset.page === pageName));
  pages.forEach((page) => page.classList.toggle("active", page.dataset.pageContent === pageName));
}

function updatePreview() {
  const format = getCustomValue("config-format");
  const dns = getCustomValue("dns");

  if (previewFormat) previewFormat.textContent = format === "amneziawg" ? "AmneziaWG" : format === "sakeen" ? "Sakeen" : "WireGuard";
  if (previewDns) previewDns.textContent = dns || "1.1.1.1";
}

function toggleAdvanced() {
  if (!advancedContent || !advancedToggle) return;
  const isOpen = advancedContent.classList.toggle("open");
  advancedToggle.classList.toggle("open", isOpen);
}

function resolveEndpoint(): string {
  const mode = getCustomValue("endpoint") || "default";

  if (mode === "custom") {
    const value = endpointCustomInput?.value.trim() ?? "";
    return value.length > 0 ? value : ENDPOINTS.default;
  }

  if (mode === "random") return RANDOM_POOL[Math.floor(Math.random() * RANDOM_POOL.length)];
  return ENDPOINTS[mode] ?? ENDPOINTS.default;
}

function resolveMtu(): number {
  const mode = getCustomValue("mtu");

  if (mode === "custom") {
    const raw = parseInt(mtuCustomInput?.value ?? "", 10);
    return Number.isFinite(raw) && raw >= 576 && raw <= 1500 ? raw : 1280;
  }

  const raw = parseInt(mode, 10);
  return Number.isFinite(raw) && raw > 0 ? raw : 1280;
}

function validateI1Domain(raw: string): string | null {
  const s = raw.trim();
  if (s.length === 0) return null;
  if (s.length > 253) return "Домен длиннее 253 символов";
  if (!s.includes(".")) return "Домен должен содержать точку (например ozon.ru)";
  if (s.startsWith(".") || s.endsWith(".") || s.includes("..")) return "Некорректное расположение точек";

  for (const label of s.split(".")) {
    if (label.length === 0) return "Пустая метка домена";
    if (label.length > 63) return "Метка домена длиннее 63 символов";
    if (label.startsWith("-") || label.endsWith("-")) return "Метка не может начинаться или заканчиваться дефисом";
    if (!/^[a-z0-9-]+$/.test(label)) return "Только латиница в нижнем регистре, цифры и дефис";
  }

  return null;
}

function buildOptions(): GenerateOptions {
  const keepaliveRaw = parseInt(getCustomValue("keepalive"), 10);
  const keepalive = Number.isFinite(keepaliveRaw) ? keepaliveRaw : 0;
  const customI1Raw = (customI1Input?.value ?? "").trim().toLowerCase();
  const profileRaw = (document.querySelector<HTMLInputElement>("#profile")?.value ?? "").trim();

  return {
    format: getCustomValue("config-format") || "wireguard",
    connection: getCustomValue("connection") || "amneziawg15",
    dns: getCustomValue("dns") || "1.1.1.1",
    endpoint: resolveEndpoint(),
    excludeLan: excludeLan?.checked ?? false,
    ipv6: getCustomValue("ipv6") || "enabled",
    keepalive,
    mtu: resolveMtu(),
    customI1Domain: customI1Raw.length > 0 ? customI1Raw : null,
    profile: profileRaw.length > 0 ? profileRaw : null,
  };
}

function generateFileName(format: string): string {
  const id = Math.floor(Math.random() * 9_000_000) + 1_000_000;
  return `${format === "amneziawg" ? "AMNEZIA" : format === "sakeen" ? "SAKEEN" : "WARP"}${id}.conf`;
}

function createHistoryItem(config: string): HistoryItem {
  return {
    id: crypto.randomUUID(),
    date: new Date().toLocaleString("ru-RU"),
    format: getCustomValue("config-format") === "amneziawg" ? "AmneziaWG" : getCustomValue("config-format") === "sakeen" ? "Sakeen" : "WireGuard",
    dns: getCustomValue("dns") || "1.1.1.1",
    mode: "Все сайты",
    config,
  };
}

function loadHistory(): HistoryItem[] {
  try {
    const stored = localStorage.getItem("warp-generator-history");
    if (!stored) return [];
    const parsed: unknown = JSON.parse(stored);
    return Array.isArray(parsed) ? (parsed as HistoryItem[]) : [];
  } catch { return []; }
}

function saveHistory() {
  localStorage.setItem("warp-generator-history", JSON.stringify(appHistory));
}

function loadSettings(): SavedSettings | null {
  try {
    const stored = localStorage.getItem("warp-generator-settings");
    return stored ? JSON.parse(stored) as SavedSettings : null;
  } catch { return null; }
}

function saveSettings() {
  const selects: Record<string, string> = {};
  SELECT_IDS.forEach((id) => { selects[id] = getCustomValue(id); });

  const data: SavedSettings = {
    autoHistory: autoHistory?.checked ?? true,
    excludeLan: excludeLan?.checked ?? false,
    endpointCustom: endpointCustomInput?.value ?? "",
    customI1: customI1Input?.value ?? "",
    mtuCustom: mtuCustomInput?.value ?? "",
    selects,
  };

  localStorage.setItem("warp-generator-settings", JSON.stringify(data));
}

function applySettings() {
  const saved = loadSettings();
  if (!saved) return;

  SELECT_IDS.forEach((id) => {
    const value = saved.selects[id];
    if (!value) return;

    const option = document.querySelector<HTMLButtonElement>(
      `.custom-select[data-select-id="${id}"] .custom-option[data-value="${value}"]`,
    );
    setCustomValue(id, value, option?.textContent?.trim() ?? value);
  });

  if (typeof saved.autoHistory === "boolean" && autoHistory) autoHistory.checked = saved.autoHistory;
  if (typeof saved.excludeLan === "boolean" && excludeLan) excludeLan.checked = saved.excludeLan;
  if (saved.endpointCustom && endpointCustomInput) endpointCustomInput.value = saved.endpointCustom;
  if (saved.customI1 && customI1Input) customI1Input.value = saved.customI1;
  if (saved.mtuCustom && mtuCustomInput) mtuCustomInput.value = saved.mtuCustom;
}

function renderHistory() {
  if (!historyList) return;

  if (appHistory.length === 0) {
    historyList.innerHTML = `
      <div class="empty-history">
        <div class="empty-icon">◷</div>
        <h3>История пока пуста</h3>
        <p>После первой генерации здесь появятся сохранённые конфигурации.</p>
        <button class="secondary-button" id="go-generator" type="button">Перейти к генератору</button>
      </div>
    `;

    document.querySelector<HTMLButtonElement>("#go-generator")
      ?.addEventListener("click", () => switchPage("generator"));
    return;
  }

  historyList.innerHTML = appHistory.map((item) => `
    <article class="history-item" data-history-id="${item.id}">
      <div class="history-item-icon">W</div>
      <div class="history-item-main">
        <strong>${item.format}</strong>
        <span>${item.mode}</span>
      </div>
      <div class="history-item-meta">
        <span>${item.dns}</span>
        <small>${item.date}</small>
      </div>
      <button class="history-delete" data-history-id="${item.id}" type="button">×</button>
    </article>
  `).join("");

  historyList.querySelectorAll<HTMLElement>(".history-item").forEach((el) => {
    el.addEventListener("click", (event) => {
      const target = event.target as HTMLElement;
      if (target.classList.contains("history-delete")) return;

      const id = el.dataset.historyId;
      if (!id) return;

      const item = appHistory.find((x) => x.id === id);
      if (item) showResultModal(item.config, `WARP${id.slice(0, 6)}.conf`);
    });
  });

  historyList.querySelectorAll<HTMLButtonElement>(".history-delete").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const id = button.dataset.historyId;
      if (!id) return;

      appHistory = appHistory.filter((item) => item.id !== id);
      saveHistory();
      renderHistory();
    });
  });
}

function showResultModal(config: string, fileName: string) {
  currentConfig = config;
  currentFileName = fileName;
  if (modalFilename) modalFilename.textContent = fileName;
  resultModal?.removeAttribute("hidden");
}

function hideResultModal() { resultModal?.setAttribute("hidden", "true"); }

function showGuideModal(fileName: string) {
  if (guideFilename) guideFilename.textContent = fileName;
  guideModal?.removeAttribute("hidden");
}

function hideGuideModal() { guideModal?.setAttribute("hidden", "true"); }

function showConfirmModal() { confirmModal?.removeAttribute("hidden"); }

function hideConfirmModal() { confirmModal?.setAttribute("hidden", "true"); }

function showErrorModal(title: string, subtitle: string, message: string) {
  if (errorTitle) errorTitle.textContent = title;
  if (errorSubtitle) errorSubtitle.textContent = subtitle;
  if (errorMessage) errorMessage.textContent = message;
  errorModal?.removeAttribute("hidden");
}

function hideErrorModal() { errorModal?.setAttribute("hidden", "true"); }

function classifyError(raw: string): { title: string; subtitle: string; message: string } {
  const lower = raw.toLowerCase();

  if (lower.includes("network") || lower.includes("error sending request") || lower.includes("dns") || lower.includes("connect")) {
    return {
      title: "Нет соединения",
      subtitle: "Не удалось связаться с Cloudflare",
      message: "Проверь подключение к интернету и попробуй снова. Если используешь VPN или прокси — попробуй временно отключить.",
    };
  }

  if (lower.includes("timeout") || lower.includes("timed out")) {
    return {
      title: "Превышено время ожидания",
      subtitle: "Cloudflare не отвечает",
      message: "Сервер не ответил вовремя. Попробуй ещё раз через несколько секунд. Если проблема повторяется — смени DNS или endpoint.",
    };
  }

  if (lower.includes("http 4") || lower.includes("http 5")) {
    return {
      title: "Ошибка Cloudflare",
      subtitle: "Сервер отклонил запрос",
      message: `Cloudflare вернул ошибку: ${raw}. Обычно это проходит само через минуту. Попробуй снова позже.`,
    };
  }

  if (lower.includes("invalid domain") || lower.includes("домен") || lower.includes("invalid")) {
    return {
      title: "Некорректный ввод",
      subtitle: "Проверь параметры",
      message: raw,
    };
  }

  return {
    title: "Ошибка генерации",
    subtitle: "Что-то пошло не так",
    message: raw,
  };
}

function setStatus(state: "ready" | "loading" | "error", text?: string) {
  const statusEl = document.querySelector<HTMLElement>("#status");
  if (!statusEl) return;

  statusEl.dataset.state = state;

  const textEl = statusEl.querySelector<HTMLElement>(".status-text");
  if (!textEl) return;

  if (text !== undefined) { textEl.textContent = text; return; }

  if (state === "loading") textEl.textContent = "Генерация...";
  else if (state === "error") textEl.textContent = "Ошибка";
  else textEl.textContent = "Готово";
}

async function generateConfiguration() {
  const button = document.querySelector<HTMLButtonElement>("#generate-button");
  if (!button) return;

  if (getCustomValue("mtu") === "custom") {
    const err = validateMtu(mtuCustomInput?.value ?? "");
    if (err) {
      if (mtuCustomError) { mtuCustomError.textContent = err; mtuCustomError.removeAttribute("hidden"); }
      mtuCustomInput?.classList.add("invalid");
      advancedContent?.classList.add("open");
      advancedToggle?.classList.add("open");
      return;
    }
    if (mtuCustomError) mtuCustomError.setAttribute("hidden", "true");
    mtuCustomInput?.classList.remove("invalid");
  }

  const i1Raw = customI1Input?.value.trim() ?? "";
  const i1Error = validateI1Domain(i1Raw);

  if (i1Error) {
    if (customI1Error) { customI1Error.textContent = i1Error; customI1Error.removeAttribute("hidden"); }
    customI1Input?.classList.add("invalid");
    advancedContent?.classList.add("open");
    advancedToggle?.classList.add("open");
    return;
  }

  if (customI1Error) customI1Error.setAttribute("hidden", "true");
  customI1Input?.classList.remove("invalid");

  button.disabled = true;
  const originalContent = button.innerHTML;
  button.innerHTML = `<span class="loading-spinner"></span><span>Генерация...</span>`;
  setStatus("loading");

  try {
    const options = buildOptions();
    const config = await invoke<string>("generate_warp_config", { options });
    const fileName = generateFileName(options.format);

    if (autoHistory?.checked) {
      appHistory.unshift(createHistoryItem(config));
      appHistory = appHistory.slice(0, 50);
      saveHistory();
      renderHistory();
    }

    setStatus("ready");
    showResultModal(config, fileName);
  } catch (err) {
    const raw = typeof err === "string" ? err : err instanceof Error ? err.message : "Неизвестная ошибка";
    const info = classifyError(raw);

    setStatus("error");
    showErrorModal(info.title, info.subtitle, info.message);

    setTimeout(() => {
      if (document.querySelector<HTMLElement>("#status")?.dataset.state === "error") {
        setStatus("ready");
      }
    }, 5000);
  } finally {
    button.disabled = false;
    button.innerHTML = originalContent;
  }
}

async function loadIpInfo() {
  if (ipLoading) return;
  ipLoading = true;

  const statusEl = document.querySelector<HTMLElement>("#ip-status");
  const addressEl = document.querySelector<HTMLElement>("#ip-address");
  const metaEl = document.querySelector<HTMLElement>("#ip-meta");
  const countryEl = document.querySelector<HTMLElement>("#ip-country");
  const cityEl = document.querySelector<HTMLElement>("#ip-city");
  const ispEl = document.querySelector<HTMLElement>("#ip-isp");
  const asnEl = document.querySelector<HTMLElement>("#ip-asn");
  const tzEl = document.querySelector<HTMLElement>("#ip-tz");
  const typeEl = document.querySelector<HTMLElement>("#ip-type");
  const refreshBtn = document.querySelector<HTMLButtonElement>("#ip-refresh");

  if (statusEl) {
    statusEl.dataset.state = "loading";
    const t = statusEl.querySelector<HTMLElement>(".status-text");
    if (t) t.textContent = "Проверка...";
  }
  if (refreshBtn) refreshBtn.disabled = true;

  try {
    const json = await invoke<string>("fetch_ip_info");
    const data = JSON.parse(json) as IpInfo;

    if (!data.success || !data.ip) throw new Error(data.message || "invalid response");

    if (addressEl) addressEl.textContent = data.ip;
    if (metaEl) metaEl.textContent = [data.country, data.city, data.connection?.isp].filter(Boolean).join(" · ");
    if (countryEl) countryEl.textContent = data.country || "—";
    if (cityEl) cityEl.textContent = data.city || "—";
    if (ispEl) ispEl.textContent = data.connection?.isp || data.connection?.org || "—";
    if (asnEl) asnEl.textContent = data.connection?.asn ? `AS${data.connection.asn}` : "—";
    if (tzEl) tzEl.textContent = data.timezone?.id || "—";
    if (typeEl) typeEl.textContent = data.type || "—";

    if (statusEl) {
      statusEl.dataset.state = "ok";
      const t = statusEl.querySelector<HTMLElement>(".status-text");
      if (t) t.textContent = "Готово";
    }
  } catch (err) {
    const raw = typeof err === "string" ? err : err instanceof Error ? err.message : "unknown error";

    if (addressEl) addressEl.textContent = "—";
    if (metaEl) metaEl.textContent = raw;
    if (countryEl) countryEl.textContent = "—";
    if (cityEl) cityEl.textContent = "—";
    if (ispEl) ispEl.textContent = "—";
    if (asnEl) asnEl.textContent = "—";
    if (tzEl) tzEl.textContent = "—";
    if (typeEl) typeEl.textContent = "—";

    if (statusEl) {
      statusEl.dataset.state = "error";
      const t = statusEl.querySelector<HTMLElement>(".status-text");
      if (t) t.textContent = "Ошибка";
    }
  } finally {
    if (refreshBtn) refreshBtn.disabled = false;
    ipLoading = false;
  }
}

async function loadRepoStars() {
  const badge = document.querySelector<HTMLElement>("#repo-stars");
  if (!badge) return;

  try {
    const response = await tauriFetch(
      "https://api.github.com/repos/Thiefgg/warp-generator-desktop",
      { method: "GET", headers: { Accept: "application/vnd.github+json" } },
    );

    if (!response.ok) return;

    const data = (await response.json()) as { stargazers_count?: number };
    badge.textContent = `★ ${data.stargazers_count ?? 0}`;
  } catch {
    badge.textContent = "★ —";
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

function formatTime(seconds: number): string {
  const h = Math.floor(seconds / 3600).toString().padStart(2, "0");
  const m = Math.floor((seconds % 3600) / 60).toString().padStart(2, "0");
  const s = (seconds % 60).toString().padStart(2, "0");
  return `${h}:${m}:${s}`;
}

function updateVpnUI() {
  if (!vpnStatusEl || !vpnStatusText || !vpnConnectBtn || !vpnStatsEl) return;

  vpnStatusEl.dataset.state = vpnState;

  if (vpnState === "disconnected") {
    vpnStatusText.textContent = "VPN отключен";
    if (vpnStatusTextSmall) vpnStatusTextSmall.textContent = "Отключено";
    if (vpnStatusHint) vpnStatusHint.textContent = "Нажмите кнопку ниже, чтобы установить защищённое соединение.";
    vpnConnectBtn.textContent = "Подключиться";
    vpnConnectBtn.classList.remove("disconnect");
    vpnStatsEl.hidden = true;
    vpnPanel?.classList.remove("connected");
    if (vpnIfaceEl) vpnIfaceEl.textContent = "—";
    if (vpnEndpointEl) vpnEndpointEl.textContent = "—";
    if (vpnStartTime) { vpnStartTime = 0; stopVpnPoll(); }
  } else if (vpnState === "connecting") {
    vpnStatusText.textContent = "Подключение...";
    if (vpnStatusTextSmall) vpnStatusTextSmall.textContent = "Подключение...";
    if (vpnStatusHint) vpnStatusHint.textContent = "Устанавливаем соединение, подождите.";
    vpnConnectBtn.textContent = "Отменить";
    vpnConnectBtn.classList.add("disconnect");
    vpnStatsEl.hidden = true;
  } else if (vpnState === "connected") {
    vpnStatusText.textContent = "VPN подключен";
    if (vpnStatusTextSmall) vpnStatusTextSmall.textContent = "Подключено";
    if (vpnStatusHint) vpnStatusHint.textContent = "Соединение установлено, трафик защищён.";
    vpnConnectBtn.textContent = "Отключиться";
    vpnConnectBtn.classList.add("disconnect");
    vpnStatsEl.hidden = false;
    vpnPanel?.classList.add("connected");
  }
}

let vpnPollTimer: number | null = null;

function stopVpnPoll() {
  if (vpnPollTimer !== null) {
    clearInterval(vpnPollTimer);
    vpnPollTimer = null;
  }
}

function startVpnPoll() {
  stopVpnPoll();

  vpnPollTimer = window.setInterval(async () => {
    try {
      const status = await invoke<{
        running: boolean;
        iface: string | null;
        endpoint: string | null;
        rx_bytes: number;
        tx_bytes: number;
        connected_since: number | null;
      }>("vpn_get_status");

      if (status.running && vpnState !== "connected") {
        vpnState = "connected";
        updateVpnUI();

        if (status.connected_since) {
          vpnStartTime = status.connected_since * 1000;
        } else {
          vpnStartTime = Date.now();
        }
      }

      if (!status.running && vpnState === "connected") {
        vpnState = "disconnected";
        updateVpnUI();
        stopVpnPoll();
        return;
      }

      if (status.running) {
        if (vpnIfaceEl) vpnIfaceEl.textContent = status.iface || "usque";
        if (vpnEndpointEl) vpnEndpointEl.textContent = status.endpoint || "—";
        if (vpnRxEl) vpnRxEl.textContent = formatBytes(status.rx_bytes);
        if (vpnTxEl) vpnTxEl.textContent = formatBytes(status.tx_bytes);

        const elapsed = Math.floor((Date.now() - vpnStartTime) / 1000);
        if (vpnTimeEl) vpnTimeEl.textContent = formatTime(elapsed);
      }
    } catch {}
  }, 1000);
}

vpnConnectBtn?.addEventListener("click", async () => {
  if (vpnState === "disconnected") {
    vpnState = "connecting";
    updateVpnUI();

    const options = buildOptions();

    if (vpnEndpointEl) vpnEndpointEl.textContent = "—";
    if (vpnIfaceEl) vpnIfaceEl.textContent = "—";
    if (vpnRxEl) vpnRxEl.textContent = "0 B";
    if (vpnTxEl) vpnTxEl.textContent = "0 B";

    try {
      await invoke("vpn_connect", { options });
      startVpnPoll();
    } catch (err) {
      vpnState = "disconnected";
      updateVpnUI();
      const raw = typeof err === "string" ? err : "Неизвестная ошибка";
      showErrorModal("Не удалось подключиться", "Ошибка VPN", raw);
    }
  } else if (vpnState === "connecting") {
    try {
      await invoke("vpn_disconnect");
    } catch {}
    stopVpnPoll();
    vpnState = "disconnected";
    updateVpnUI();
  } else {
    try {
      await invoke("vpn_disconnect");
    } catch {}
    stopVpnPoll();
    vpnState = "disconnected";
    updateVpnUI();
  }
});

navItems.forEach((item) => {
  item.addEventListener("click", () => {
    const page = item.dataset.page as PageName | undefined;
    if (page) switchPage(page);
    if (page === "ip") void loadIpInfo();
  });
});

advancedToggle?.addEventListener("click", toggleAdvanced);

generatorForm?.addEventListener("submit", (event) => {
  event.preventDefault();
  closeAllCustomSelects();
  void generateConfiguration();
});

autoHistory?.addEventListener("change", saveSettings);
excludeLan?.addEventListener("change", saveSettings);
endpointCustomInput?.addEventListener("input", saveSettings);

customI1Input?.addEventListener("input", () => {
  if (customI1Input) {
    const lowered = customI1Input.value.toLowerCase();
    if (customI1Input.value !== lowered) customI1Input.value = lowered;
  }

  saveSettings();

  if (!customI1Error || !customI1Input) return;

  const err = validateI1Domain(customI1Input.value.trim());

  if (err) {
    customI1Error.textContent = err;
    customI1Error.removeAttribute("hidden");
    customI1Input.classList.add("invalid");
  } else {
    customI1Error.setAttribute("hidden", "true");
    customI1Input.classList.remove("invalid");
  }
});

mtuCustomInput?.addEventListener("input", () => {
  saveSettings();

  if (!mtuCustomError || !mtuCustomInput) return;

  const raw = mtuCustomInput.value;
  const err = validateMtu(raw);

  if (err && raw.length > 0) {
    mtuCustomError.textContent = err;
    mtuCustomError.removeAttribute("hidden");
    mtuCustomInput.classList.add("invalid");
  } else {
    mtuCustomError.setAttribute("hidden", "true");
    mtuCustomInput.classList.remove("invalid");
  }
});

document.querySelector<HTMLButtonElement>("#ip-refresh")?.addEventListener("click", () => {
  void loadIpInfo();
});

clearHistory?.addEventListener("click", () => {
  if (appHistory.length === 0) return;
  showConfirmModal();
});

confirmClose?.addEventListener("click", hideConfirmModal);
confirmNo?.addEventListener("click", hideConfirmModal);

confirmYes?.addEventListener("click", () => {
  appHistory = [];
  saveHistory();
  renderHistory();
  hideConfirmModal();
});

confirmModal?.addEventListener("click", (event) => {
  if (event.target === confirmModal) hideConfirmModal();
});

resultModal?.addEventListener("click", (event) => {
  if (event.target === resultModal) hideResultModal();
});

guideModal?.addEventListener("click", (event) => {
  if (event.target === guideModal) hideGuideModal();
});

errorModal?.addEventListener("click", (event) => {
  if (event.target === errorModal) {
    hideErrorModal();
    setStatus("ready");
  }
});

modalClose?.addEventListener("click", hideResultModal);
guideClose?.addEventListener("click", hideGuideModal);
guideOk?.addEventListener("click", hideGuideModal);

errorClose?.addEventListener("click", () => {
  hideErrorModal();
  setStatus("ready");
});

errorOk?.addEventListener("click", () => {
  hideErrorModal();
  setStatus("ready");
});

modalOpen?.addEventListener("click", async () => {
  if (!currentConfig || !currentFileName) return;
  try {
    await invoke("open_in_amnezia", { content: currentConfig, filename: currentFileName });
    hideResultModal();
    showGuideModal(currentFileName);
  } catch (err) {
    const raw = typeof err === "string" ? err : "Неизвестная ошибка";
    showErrorModal("Не удалось открыть", "AmneziaWG недоступна", raw);
  }
});

modalSave?.addEventListener("click", async () => {
  if (!currentConfig || !currentFileName) return;

  try {
    const path = await save({
      defaultPath: currentFileName,
      filters: [
        { name: "WireGuard / AmneziaWG", extensions: ["conf"] },
        { name: "All files", extensions: ["*"] },
      ],
    });

    if (!path) return;
    await invoke<string>("save_to_path", { path, content: currentConfig });
  } catch (err) {
    const raw = typeof err === "string" ? err : "Неизвестная ошибка";
    showErrorModal("Не удалось сохранить", "Ошибка файловой системы", raw);
  }
});

modalCopy?.addEventListener("click", async () => {
  if (!currentConfig) return;
  try {
    await navigator.clipboard.writeText(currentConfig);
    const label = modalCopy.querySelector("span");
    if (!label) return;
    const original = label.textContent;
    label.textContent = "Скопировано";
    setTimeout(() => { label.textContent = original; }, 1500);
  } catch {}
});

document.addEventListener("click", () => closeAllCustomSelects());

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    hideResultModal();
    hideGuideModal();
    hideConfirmModal();
    hideErrorModal();
    setStatus("ready");
  }
});

minimizeButton?.addEventListener("click", async (event) => {
  event.stopPropagation();
  await appWindow.minimize();
});

closeButton?.addEventListener("click", async (event) => {
  event.stopPropagation();
  try {
    await invoke("app_quit");
  } catch {
    await appWindow.close();
  }
});

document.querySelectorAll<HTMLAnchorElement>(".footer a, .project-link").forEach((link) => {
  link.addEventListener("click", async (event) => {
    event.preventDefault();
    const url = link.getAttribute("href");
    if (!url) return;
    try { await openUrl(url); } catch {}
  });
});

initCustomSelects();
applySettings();
renderHistory();
updatePreview();
updateEndpointVisibility();
updateMtuVisibility();
updateProfileVisibility();
updateVpnUI();
void loadRepoStars();