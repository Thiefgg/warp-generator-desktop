import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { save } from "@tauri-apps/plugin-dialog";

type PageName = "generator" | "history" | "settings";

interface HistoryItem {
  id: string;
  date: string;
  format: string;
  dns: string;
  mode: string;
  config: string;
}

interface GenerateOptions {
  format: string;
  connection: string;
  dns: string;
  endpoint: string;
  excludeLan: boolean;
  ipv6: string;
  keepalive: number;
  mtu: number;
  customI1Domain: string | null;
}

interface SavedSettings {
  autoHistory: boolean;
  excludeLan: boolean;
  endpointCustom: string;
  customI1: string;
  selects: Record<string, string>;
}

const ENDPOINTS: Record<string, string> = {
  default: "162.159.195.1:500",
  default2: "engage.cloudflareclient.com:2408",
  fra1: "188.114.97.66:4500",
  fra2: "188.114.96.125:4500",
};

const RANDOM_POOL: string[] = [
  "162.159.192.1:4500",
  "162.159.193.1:4500",
  "162.159.195.1:4500",
  "162.159.204.1:4500",
  "188.114.96.125:4500",
  "188.114.97.66:4500",
];

const SELECT_IDS = [
  "config-format",
  "connection",
  "dns",
  "endpoint",
  "ipv6",
  "mtu",
  "keepalive",
];

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
const maximizeButton = document.querySelector<HTMLButtonElement>("#window-maximize");
const closeButton = document.querySelector<HTMLButtonElement>("#window-close");
const endpointCustomField = document.querySelector<HTMLElement>("#endpoint-custom-field");
const endpointCustomInput = document.querySelector<HTMLInputElement>("#endpoint-custom");
const customI1Input = document.querySelector<HTMLInputElement>("#custom-i1");
const customI1Error = document.querySelector<HTMLElement>("#custom-i1-error");
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

let appHistory: HistoryItem[] = loadHistory();
let currentConfig = "";
let currentFileName = "";

function getCustomValue(id: string): string {
  const input = document.querySelector<HTMLInputElement>(`#${id}`);
  return input?.value ?? "";
}

function setCustomValue(id: string, value: string, label: string) {
  const input = document.querySelector<HTMLInputElement>(`#${id}`);
  const select = document.querySelector<HTMLElement>(`.custom-select[data-select-id="${id}"]`);
  if (!input || !select) return;

  input.value = value;

  const valueElement = select.querySelector<HTMLElement>(".custom-select-value");
  if (valueElement) valueElement.textContent = label;

  select.querySelectorAll<HTMLButtonElement>(".custom-option").forEach((option) => {
    option.classList.toggle("selected", option.dataset.value === value);
  });
}

function closeAllCustomSelects(except?: HTMLElement) {
  document.querySelectorAll<HTMLElement>(".custom-select.open").forEach((select) => {
    if (select !== except) closeCustomSelect(select);
  });
}

function openCustomSelect(select: HTMLElement) {
  closeAllCustomSelects(select);
  select.classList.add("open");
  const trigger = select.querySelector<HTMLButtonElement>(".custom-select-trigger");
  trigger?.setAttribute("aria-expanded", "true");
}

function closeCustomSelect(select: HTMLElement) {
  select.classList.remove("open");
  const trigger = select.querySelector<HTMLButtonElement>(".custom-select-trigger");
  trigger?.setAttribute("aria-expanded", "false");
}

function updateEndpointVisibility() {
  if (!endpointCustomField) return;
  const mode = getCustomValue("endpoint");
  if (mode === "custom") {
    endpointCustomField.removeAttribute("hidden");
  } else {
    endpointCustomField.setAttribute("hidden", "true");
  }
}

function initCustomSelects() {
  document.querySelectorAll<HTMLElement>(".custom-select").forEach((select) => {
    const id = select.dataset.selectId;
    if (!id) return;

    const trigger = select.querySelector<HTMLButtonElement>(".custom-select-trigger");
    const options = select.querySelectorAll<HTMLButtonElement>(".custom-option");

    trigger?.addEventListener("click", (event) => {
      event.stopPropagation();
      if (select.classList.contains("open")) closeCustomSelect(select);
      else openCustomSelect(select);
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
      });
    });
  });
}

function switchPage(pageName: PageName) {
  closeAllCustomSelects();
  hideResultModal();
  hideGuideModal();
  hideConfirmModal();

  navItems.forEach((item) => {
    item.classList.toggle("active", item.dataset.page === pageName);
  });

  pages.forEach((page) => {
    page.classList.toggle("active", page.dataset.pageContent === pageName);
  });
}

function updatePreview() {
  const format = getCustomValue("config-format");
  const dns = getCustomValue("dns");

  if (previewFormat) previewFormat.textContent = format === "amneziawg" ? "AmneziaWG" : "WireGuard";
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
    if (value.length > 0) return value;
    return ENDPOINTS.default;
  }

  if (mode === "random") return RANDOM_POOL[Math.floor(Math.random() * RANDOM_POOL.length)];

  return ENDPOINTS[mode] ?? ENDPOINTS.default;
}

function validateI1Domain(raw: string): string | null {
  const s = raw.trim();
  if (s.length === 0) return null;
  if (s.length > 253) return "Домен длиннее 253 символов";
  if (!s.includes(".")) return "Домен должен содержать точку (например ozon.ru)";
  if (s.startsWith(".") || s.endsWith(".") || s.includes(".."))
    return "Некорректное расположение точек";

  const labels = s.split(".");
  for (const label of labels) {
    if (label.length === 0) return "Пустая метка домена";
    if (label.length > 63) return "Метка домена длиннее 63 символов";
    if (label.startsWith("-") || label.endsWith("-"))
      return "Метка не может начинаться или заканчиваться дефисом";
    if (!/^[a-z0-9-]+$/.test(label))
      return "Только латиница в нижнем регистре, цифры и дефис";
  }

  return null;
}

function buildOptions(): GenerateOptions {
  const keepaliveRaw = parseInt(getCustomValue("keepalive"), 10);
  const keepalive = Number.isFinite(keepaliveRaw) ? keepaliveRaw : 0;

  const mtuRaw = parseInt(getCustomValue("mtu"), 10);
  const mtu = Number.isFinite(mtuRaw) && mtuRaw > 0 ? mtuRaw : 1280;

  const customI1Raw = customI1Input?.value.trim() ?? "";

  return {
    format: getCustomValue("config-format") || "wireguard",
    connection: getCustomValue("connection") || "amneziawg15",
    dns: getCustomValue("dns") || "1.1.1.1",
    endpoint: resolveEndpoint(),
    excludeLan: excludeLan?.checked ?? false,
    ipv6: getCustomValue("ipv6") || "enabled",
    keepalive,
    mtu,
    customI1Domain: customI1Raw.length > 0 ? customI1Raw : null,
  };
}

function generateFileName(format: string): string {
  const id = Math.floor(Math.random() * 9_000_000) + 1_000_000;
  const prefix = format === "amneziawg" ? "AMNEZIA" : "WARP";
  return `${prefix}${id}.conf`;
}

function createHistoryItem(config: string): HistoryItem {
  const format = getCustomValue("config-format") === "amneziawg" ? "AmneziaWG" : "WireGuard";
  return {
    id: crypto.randomUUID(),
    date: new Date().toLocaleString("ru-RU"),
    format,
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
  } catch {
    return [];
  }
}

function saveHistory() {
  localStorage.setItem("warp-generator-history", JSON.stringify(appHistory));
}

function loadSettings(): SavedSettings | null {
  try {
    const stored = localStorage.getItem("warp-generator-settings");
    if (!stored) return null;
    return JSON.parse(stored) as SavedSettings;
  } catch {
    return null;
  }
}

function saveSettings() {
  const selects: Record<string, string> = {};
  SELECT_IDS.forEach((id) => {
    selects[id] = getCustomValue(id);
  });

  const data: SavedSettings = {
    autoHistory: autoHistory?.checked ?? true,
    excludeLan: excludeLan?.checked ?? false,
    endpointCustom: endpointCustomInput?.value ?? "",
    customI1: customI1Input?.value ?? "",
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

  if (typeof saved.autoHistory === "boolean" && autoHistory) {
    autoHistory.checked = saved.autoHistory;
  }

  if (typeof saved.excludeLan === "boolean" && excludeLan) {
    excludeLan.checked = saved.excludeLan;
  }

  if (saved.endpointCustom && endpointCustomInput) {
    endpointCustomInput.value = saved.endpointCustom;
  }

  if (saved.customI1 && customI1Input) {
    customI1Input.value = saved.customI1;
  }
}

function renderHistory() {
  if (!historyList) return;

  if (appHistory.length === 0) {
    historyList.innerHTML = `
      <div class="empty-history">
        <div class="empty-icon">◷</div>
        <h3>История пока пуста</h3>
        <p>
          После первой генерации здесь появятся
          сохранённые конфигурации.
        </p>
        <button class="secondary-button" id="go-generator" type="button">
          Перейти к генератору
        </button>
      </div>
    `;

    document.querySelector<HTMLButtonElement>("#go-generator")
      ?.addEventListener("click", () => switchPage("generator"));

    return;
  }

  historyList.innerHTML = appHistory
    .map(
      (item) => `
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
      `,
    )
    .join("");

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

function hideResultModal() {
  resultModal?.setAttribute("hidden", "true");
}

function showGuideModal(fileName: string) {
  if (guideFilename) guideFilename.textContent = fileName;
  guideModal?.removeAttribute("hidden");
}

function hideGuideModal() {
  guideModal?.setAttribute("hidden", "true");
}

function showConfirmModal() {
  confirmModal?.removeAttribute("hidden");
}

function hideConfirmModal() {
  confirmModal?.setAttribute("hidden", "true");
}

async function generateConfiguration() {
  const button = document.querySelector<HTMLButtonElement>("#generate-button");
  if (!button) return;

  const i1Raw = customI1Input?.value.trim() ?? "";
  const i1Error = validateI1Domain(i1Raw);

  if (i1Error) {
    if (customI1Error) {
      customI1Error.textContent = i1Error;
      customI1Error.removeAttribute("hidden");
    }
    customI1Input?.classList.add("invalid");
    advancedContent?.classList.add("open");
    advancedToggle?.classList.add("open");
    return;
  }

  if (customI1Error) customI1Error.setAttribute("hidden", "true");
  customI1Input?.classList.remove("invalid");

  button.disabled = true;
  const originalContent = button.innerHTML;

  button.innerHTML = `
    <span class="loading-spinner"></span>
    <span>Генерация...</span>
  `;

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

    showResultModal(config, fileName);
  } catch (err) {
    const message =
      typeof err === "string"
        ? err
        : err instanceof Error
          ? err.message
          : "Неизвестная ошибка";

    alert(`Ошибка генерации: ${message}`);
  } finally {
    button.disabled = false;
    button.innerHTML = originalContent;
  }
}

async function loadRepoStars() {
  const badge = document.querySelector<HTMLElement>("#repo-stars");
  if (!badge) return;

  try {
    const response = await tauriFetch(
      "https://api.github.com/repos/Thiefgg/warp-generator-desktop",
      {
        method: "GET",
        headers: { Accept: "application/vnd.github+json" },
      },
    );

    if (!response.ok) return;

    const data = (await response.json()) as { stargazers_count?: number };
    const stars = data.stargazers_count ?? 0;
    badge.textContent = `★ ${stars}`;
  } catch {
    badge.textContent = "★ —";
  }
}

navItems.forEach((item) => {
  item.addEventListener("click", () => {
    const page = item.dataset.page as PageName | undefined;
    if (page) switchPage(page);
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
  saveSettings();

  if (!customI1Error || !customI1Input) return;

  const raw = customI1Input.value.trim();
  const err = validateI1Domain(raw);

  if (err) {
    customI1Error.textContent = err;
    customI1Error.removeAttribute("hidden");
    customI1Input.classList.add("invalid");
  } else {
    customI1Error.setAttribute("hidden", "true");
    customI1Input.classList.remove("invalid");
  }
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

modalClose?.addEventListener("click", hideResultModal);
guideClose?.addEventListener("click", hideGuideModal);
guideOk?.addEventListener("click", hideGuideModal);

modalOpen?.addEventListener("click", async () => {
  if (!currentConfig || !currentFileName) return;
  try {
    await invoke("open_in_amnezia", {
      content: currentConfig,
      filename: currentFileName,
    });

    hideResultModal();
    showGuideModal(currentFileName);
  } catch (err) {
    alert(`Не удалось открыть: ${err}`);
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

    await invoke<string>("save_to_path", {
      path,
      content: currentConfig,
    });
  } catch (err) {
    alert(`Не удалось сохранить: ${err}`);
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
    setTimeout(() => {
      label.textContent = original;
    }, 1500);
  } catch {}
});

document.addEventListener("click", () => {
  closeAllCustomSelects();
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    hideResultModal();
    hideGuideModal();
    hideConfirmModal();
  }
});

minimizeButton?.addEventListener("click", async (event) => {
  event.stopPropagation();
  await appWindow.minimize();
});

maximizeButton?.addEventListener("click", async (event) => {
  event.stopPropagation();
  await appWindow.toggleMaximize();
  const maximized = await appWindow.isMaximized();
  maximizeButton.classList.toggle("is-maximized", maximized);
});

closeButton?.addEventListener("click", async (event) => {
  event.stopPropagation();
  await appWindow.close();
});

appWindow.onResized(async () => {
  if (!maximizeButton) return;
  const maximized = await appWindow.isMaximized();
  maximizeButton.classList.toggle("is-maximized", maximized);
});

document.querySelectorAll<HTMLAnchorElement>(".footer a").forEach((link) => {
  link.addEventListener("click", async (event) => {
    event.preventDefault();
    const url = link.getAttribute("href");
    if (!url) return;
    try {
      await openUrl(url);
    } catch {}
  });
});

document.querySelectorAll<HTMLAnchorElement>(".project-link").forEach((link) => {
  link.addEventListener("click", async (event) => {
    event.preventDefault();
    const url = link.getAttribute("href");
    if (!url) return;
    try {
      await openUrl(url);
    } catch {}
  });
});

initCustomSelects();
applySettings();
renderHistory();
updatePreview();
updateEndpointVisibility();
void loadRepoStars();