import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  applyCheckIndicatorScale,
  buildProfileSourceOptions,
  buildSelectedPreviewSources,
  buildSelectedRegionConfigs,
  buildSelectedWindowAppConfigs,
  buildSelectedWindowConfigs,
  clearRestorePreviewFrames,
  coverRestorePreviewFrames,
  fitFixedMenuInViewport,
  installAutohideScrollbar,
  installEntryCursorEndHandlers,
  installCustomCheckIndicators,
  layoutBusy,
  monitoringEventTransition,
  monitoringStatusText,
  previewStatusText,
  profileImportRequest,
  profileImportStatusText,
  profileTargetsEnabledStatusText,
  profileWorkflowActionState,
  recordRepeatClick,
  installWheelScroll,
  scanStatusText,
  selectIndexedListItem,
  selectedWindowRecords,
  shouldHandleProfilePaste,
  sourcePreviewCardPresentation,
  sourcePreviewRefreshGate,
  targetDropAfter,
  targetDropInsertIndex,
  targetEnabled,
  targetHitCount,
  targetActionState,
  targetMenuState,
  targetSelectionIndexFromEditResult,
  targetSelectionIndexForProfileLoad,
  updateAutohideScrollbar,
  visiblePreviewRect,
  windowResolutionStatusText,
} from "./ui-behavior.js";
import "./styles.css";

const MONITOR_SESSION_EVENT = "screen-watch://monitor-session";
const SOURCE_PREVIEW_SYNC_MS = 1000;
const SOURCE_PREVIEW_BUSY_RETRY_MS = 250;

let currentMonitors = [];
let currentWindows = [];
let currentDataDir = "";
let currentProfile = null;
let legacyMaxAlerts = null;
let selectedMonitorIndexes = new Set();
let selectedWindowHandles = new Set();
let draggedTargetIndex = null;
let targetContextMenu = null;
let unlistenMonitorSession = null;
let applyingProfileSources = false;
let profileMonitoringActive = false;
let sourcePreviewTimer = null;
let sourcePreviewsEnabled = false;
let sourcePreviewRefreshing = false;
let sourcePreviewLayoutBusyUntil = 0;
let sourcePreviewRenderSignatureText = "";
let selectedTargetIndex = null;
let targetLastClick = {};

async function refresh() {
  const status = document.querySelector("#status");
  status.textContent = "读取运行信息...";
  try {
    const info = await invoke("app_info");
    document.querySelector("#flavor").textContent = info.buildFlavor;
    renderOcrStatus(info.ocr);
    document.querySelector("#data-dir").textContent = info.dataDir;
    currentDataDir = info.dataDir;
    const profileState = await invoke("load_profile_state");
    legacyMaxAlerts = legacyMaxAlertsFromState(profileState.state);
    document.querySelector("#profile-number").value = String(
      profileState.lastProfile || 1,
    );
    const monitors = await invoke("list_monitors");
    currentMonitors = monitors;
    seedScanConfig();
    await refreshWindows();
    await refreshStartupStatus();
    await loadProfile({ selectFirstTarget: true });
    updateRunControls();
    appendLog("应用已就绪");
    status.textContent = "Ready";
  } catch (error) {
    status.textContent = String(error);
  }
}

function renderOcrStatus(ocr) {
  document.querySelector("#ocr").textContent = ocr.available
    ? "available"
    : ocr.reason;
  document.querySelector("#ocr-flags").textContent = [
    `enabled=${yesNo(ocr.enabled)}`,
    `module=${yesNo(ocr.moduleCompiled)}`,
    `models=${yesNo(ocr.modelsReady)}`,
    `backend=${yesNo(ocr.backendReady)} (${ocr.backendName || "-"})`,
    `profile=${ocr.modelProfile || "-"}`,
  ].join("  ");
  document.querySelector("#model-dir").textContent = ocr.modelDir;
  const models = Array.isArray(ocr.requiredModels) ? ocr.requiredModels : [];
  const modelList = document.querySelector("#ocr-models");
  if (models.length === 0) {
    const item = document.createElement("li");
    item.textContent = "-";
    modelList.replaceChildren(item);
    return;
  }
  modelList.replaceChildren(
    ...models.map((model) => {
      const item = document.createElement("li");
      item.className = model.exists ? "is-ready" : "is-missing";
      const state = document.createElement("span");
      state.className = "ocr-model-state";
      state.textContent = model.exists ? "ready" : "missing";
      const name = document.createElement("strong");
      name.textContent = model.name;
      const size = document.createElement("small");
      size.textContent = model.exists
        ? `${formatBytes(model.bytes)}  ${model.path}`
        : model.path;
      item.replaceChildren(state, name, size);
      return item;
    }),
  );
}

async function probeOcrBackend() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#ocr-probe-result");
  status.textContent = "测试 OCR 后端...";
  result.textContent = "running...";
  try {
    const probe = await invoke("ocr_backend_probe");
    renderOcrProbeResult(probe);
    if (probe.initialized) {
      status.textContent = "Ready - OCR backend initialized";
    } else if (probe.attempted) {
      status.textContent = "OCR 后端初始化失败";
    } else {
      status.textContent = "OCR 后端未尝试初始化";
    }
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

function renderOcrProbeResult(probe) {
  const availability = probe.availability || {};
  document.querySelector("#ocr-probe-result").textContent = JSON.stringify(
    {
      attempted: Boolean(probe.attempted),
      initialized: Boolean(probe.initialized),
      reason: probe.reason || "-",
      error: probe.error || null,
      backend: availability.backendName || "-",
      profile: availability.modelProfile || "-",
      modelDir: availability.modelDir || "-",
    },
    null,
    2,
  );
}

function yesNo(value) {
  return value ? "yes" : "no";
}

function formatBytes(value) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return "-";
  }
  if (value < 1024) {
    return `${value} B`;
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KiB`;
  }
  return `${(value / 1024 / 1024).toFixed(2)} MiB`;
}

function legacyMaxAlertsFromState(state) {
  const value = Number(state?.max_alerts);
  return Number.isFinite(value) && value > 0 ? Math.floor(value) : null;
}

async function attachMonitoringEvents() {
  if (unlistenMonitorSession) {
    return;
  }
  unlistenMonitorSession = await listen(MONITOR_SESSION_EVENT, (event) => {
    renderMonitoringEvent(event.payload);
  });
}

function renderMonitoringEvent(payload) {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  const transition = monitoringEventTransition(payload, {
    profileMonitoringActive,
  });
  result.textContent = JSON.stringify(payload, null, 2);
  status.textContent = transition.statusText;
  profileMonitoringActive = transition.nextProfileMonitoringActive;
  appendLog(monitoringEventLogText(payload, transition.statusText));
  updateRunControls();
  if (transition.shouldRefreshProfile) {
    loadProfile();
  }
}

function monitoringEventLogText(payload, fallback) {
  const snapshot = payload?.snapshot || {};
  if (payload?.kind === "tick") {
    const parts = [
      `扫描 ${snapshot.regionCount || 0} 屏 / ${snapshot.windowCount || 0} 应用`,
      `命中 ${payload.tickHitCount || 0}`,
    ];
    if (payload.tickError) {
      parts.push(payload.tickError);
    }
    return parts.join("，");
  }
  if (payload?.kind === "started") {
    return "监控中";
  }
  if (payload?.kind === "stopped") {
    return "已停止";
  }
  return fallback || "状态更新";
}

function appendLog(message) {
  const log = document.querySelector("#event-log");
  if (!log) {
    return;
  }
  const row = document.createElement("tr");
  const time = document.createElement("td");
  const event = document.createElement("td");
  time.textContent = new Date().toLocaleTimeString("zh-CN", { hour12: false });
  event.textContent = String(message || "-");
  row.replaceChildren(time, event);
  log.prepend(row);
  while (log.children.length > 100) {
    log.lastElementChild?.remove();
  }
}

function seedScanConfig() {
  const editor = document.querySelector("#scan-config");
  if (editor.value.trim()) {
    return;
  }
  const monitor =
    currentMonitors.find((item) => !item.isVirtual) || currentMonitors[0];
  if (!monitor) {
    editor.value = "";
    return;
  }
  const width = Math.min(240, monitor.width);
  const height = Math.min(160, monitor.height);
  editor.value = JSON.stringify(
    {
      cooldown_seconds: 3,
      regions: [
        {
          name: "preview-scan",
          monitor: monitor.index,
          left: 0,
          top: 0,
          width,
          height,
        },
      ],
      targets: [
        {
          kind: "pixel",
          id: "preview-pixel",
          name: "preview-pixel",
          x: 0,
          y: 0,
          rgb: [0, 0, 0],
          tolerance: 255,
        },
      ],
      alarm: {
        beep: false,
        save_dir: "screenshots",
        jsonl: "alerts.jsonl",
        max_alerts: 5,
      },
    },
    null,
    2,
  );
}

function ensureDefaultMonitorSelection() {
  const physical = currentMonitors.filter((monitor) => !monitor.isVirtual);
  const physicalIndexes = new Set(physical.map((monitor) => monitor.index));
  selectedMonitorIndexes = new Set(
    [...selectedMonitorIndexes].filter((index) => physicalIndexes.has(index)),
  );
  if (
    selectedMonitorIndexes.size === 0 &&
    selectedWindowHandles.size === 0 &&
    physical[0]
  ) {
    selectedMonitorIndexes.add(physical[0].index);
  }
}

function renderMonitorList() {
  const monitorList = document.querySelector("#monitors");
  monitorList.replaceChildren(
    ...currentMonitors.map((monitor) => {
      const item = document.createElement("li");
      item.className = "source-row";
      const label = document.createElement("label");
      label.className = "check-control";
      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.disabled = monitor.isVirtual;
      checkbox.checked =
        !monitor.isVirtual && selectedMonitorIndexes.has(monitor.index);
      checkbox.addEventListener("change", () => {
        if (checkbox.checked) {
          selectedMonitorIndexes.add(monitor.index);
        } else {
          selectedMonitorIndexes.delete(monitor.index);
        }
        renderSourcePreviewPlaceholders();
        scheduleSourcePreviews(0);
        persistProfileSources();
      });
      const name = monitor.isVirtual
        ? "virtual"
        : monitor.name || `monitor-${monitor.index}`;
      const text = document.createElement("span");
      text.textContent = `${monitor.index}: ${name} ${monitor.left},${monitor.top} ${monitor.width}x${monitor.height}`;
      label.replaceChildren(checkbox, text);
      applyCheckIndicatorScale(label);
      item.replaceChildren(label);
      return item;
    }),
  );
}

function renderStartupStatus(startup) {
  document.querySelector("#startup-supported").textContent = startup.supported
    ? "yes"
    : "no";
  document.querySelector("#startup-enabled").textContent = startup.enabled
    ? "yes"
    : "no";
  document.querySelector("#startup-link").textContent = startup.linkPath || "-";
  document.querySelector("#startup-target").textContent =
    startup.targetPath || "-";
  document.querySelector("#startup-args").textContent = startup.arguments || "-";
  const startupToggle = document.querySelector("#startup-toggle");
  if (startupToggle) {
    startupToggle.checked = Boolean(startup.enabled);
    startupToggle.disabled = !startup.supported;
  }
}

async function refreshStartupStatus() {
  const status = document.querySelector("#status");
  try {
    renderStartupStatus(await invoke("startup_status"));
  } catch (error) {
    status.textContent = String(error);
  }
}

async function setStartup(enabled) {
  const status = document.querySelector("#status");
  status.textContent = enabled ? "启用启动项..." : "关闭启动项...";
  try {
    renderStartupStatus(await invoke("set_startup_enabled", { enabled }));
    status.textContent = "Ready";
  } catch (error) {
    status.textContent = String(error);
  }
}

async function capturePreview() {
  const status = document.querySelector("#status");
  const monitor =
    currentMonitors.find((item) => !item.isVirtual) || currentMonitors[0];
  if (!monitor) {
    status.textContent = "没有可用屏幕";
    return;
  }
  const width = Math.min(200, monitor.width);
  const height = Math.min(120, monitor.height);
  status.textContent = "抓取预览...";
  try {
    const preview = await invoke("capture_screen_region_preview_cached", {
      sourceKey: `screen:monitor-${monitor.index}`,
      left: monitor.left,
      top: monitor.top,
      width,
      height,
    });
    const image = document.querySelector("#preview");
    image.src = preview.dataUrl;
    image.width = preview.width;
    image.height = preview.height;
    status.textContent = "Ready";
  } catch (error) {
    status.textContent = String(error);
  }
}

async function refreshWindows() {
  const status = document.querySelector("#status");
  try {
    const windows = await invoke("list_app_windows");
    currentWindows = windows;
    const availableHandles = new Set(windows.map((window) => String(window.hwnd)));
    selectedWindowHandles = new Set(
      [...selectedWindowHandles].filter((hwnd) => availableHandles.has(hwnd)),
    );
    renderWindowList();
    renderSourcePreviewPlaceholders();
    scheduleSourcePreviews(0);
  } catch (error) {
    status.textContent = String(error);
  }
}

function renderWindowList() {
  const windowList = document.querySelector("#windows");
  windowList.replaceChildren(...currentWindows.map(renderWindowSource));
  if (!currentWindows.length) {
    const item = document.createElement("li");
    item.textContent = "没有可选择窗口";
    windowList.replaceChildren(item);
  }
}

function renderWindowSource(window) {
  const item = document.createElement("li");
  item.className = "source-row";
  const label = document.createElement("label");
  label.className = "check-control";
  const checkbox = document.createElement("input");
  const hwnd = String(window.hwnd);
  checkbox.type = "checkbox";
  checkbox.checked = selectedWindowHandles.has(hwnd);
  checkbox.addEventListener("change", () => {
    if (checkbox.checked) {
      selectedWindowHandles.add(hwnd);
    } else {
      selectedWindowHandles.delete(hwnd);
    }
    renderSourcePreviewPlaceholders();
    scheduleSourcePreviews(0);
    persistProfileSources();
  });
  const text = document.createElement("span");
  text.textContent = `${window.display} ${window.width}x${window.height}`;
  label.replaceChildren(checkbox, text);
  applyCheckIndicatorScale(label);
  item.replaceChildren(label);
  return item;
}

async function captureWindowPreview() {
  const status = document.querySelector("#status");
  const window = selectedWindowObjects()[0] || currentWindows[0];
  if (!window) {
    status.textContent = "没有可预览窗口";
    return;
  }
  status.textContent = "抓取窗口预览...";
  try {
    const preview = await invoke("capture_window_preview_cached", {
      sourceKey: `app:${window.hwnd}`,
      hwnd: window.hwnd,
    });
    const image = document.querySelector("#preview");
    image.src = preview.dataUrl;
    image.width = preview.width;
    image.height = preview.height;
    status.textContent = `Ready - ${window.display}`;
  } catch (error) {
    status.textContent = String(error);
  }
}

function selectedPreviewSources() {
  return buildSelectedPreviewSources({
    monitors: currentMonitors,
    region: profileRegionInputs(),
    selectedMonitorIndexes,
    selectedWindowHandles,
    windows: currentWindows,
  });
}

function renderSourcePreviewPlaceholders() {
  renderSourcePreviewCards(selectedPreviewSources());
}

function scheduleSourcePreviews(delay = SOURCE_PREVIEW_SYNC_MS) {
  if (sourcePreviewTimer) {
    window.clearTimeout(sourcePreviewTimer);
    sourcePreviewTimer = null;
  }
  if (!sourcePreviewsEnabled) {
    return;
  }
  sourcePreviewTimer = window.setTimeout(() => {
    sourcePreviewTimer = null;
    refreshSourcePreviews({ scheduled: true });
  }, Math.max(0, delay));
}

function enableSourcePreviews(delay = 0) {
  sourcePreviewsEnabled = true;
  scheduleSourcePreviews(delay);
}

function disableSourcePreviews() {
  sourcePreviewsEnabled = false;
  coverSourcePreviewFrames();
  if (sourcePreviewTimer) {
    window.clearTimeout(sourcePreviewTimer);
    sourcePreviewTimer = null;
  }
  clearDwmPreviews();
}

function coverSourcePreviewFrames() {
  return coverRestorePreviewFrames(document.querySelector("#source-previews"));
}

function clearSourcePreviewFrames(root) {
  return clearRestorePreviewFrames(root || document.querySelector("#source-previews"));
}

function markSourcePreviewLayoutBusy(duration = SOURCE_PREVIEW_BUSY_RETRY_MS) {
  sourcePreviewLayoutBusyUntil = Math.max(
    sourcePreviewLayoutBusyUntil,
    Date.now() + duration,
  );
}

function sourcePreviewLayoutBusy() {
  return layoutBusy({
    extraBusy: document.hidden,
    layoutActiveUntil:
      draggedTargetIndex !== null ? Number.POSITIVE_INFINITY : 0,
    now: Date.now(),
    resizeActiveUntil: sourcePreviewLayoutBusyUntil,
  });
}

function renderSourcePreviewCards(sources) {
  const container = document.querySelector("#source-previews");
  const renderSignature = sourcePreviewRenderSignature(sources);
  if (!sources.length) {
    clearDwmPreviews();
    clearSourcePreviewFrames(container);
    const empty = document.createElement("p");
    empty.className = "source-preview-empty";
    empty.textContent = "未选择来源";
    container.replaceChildren(empty);
    sourcePreviewRenderSignatureText = renderSignature;
    return new Map();
  }

  if (renderSignature === sourcePreviewRenderSignatureText) {
    const cards = new Map();
    for (const source of sources) {
      const card = [...container.children].find(
        (item) => item.dataset.sourceKey === source.key,
      );
      if (!card) {
        break;
      }
      cards.set(source.key, {
        card,
        frame: card.querySelector(".source-preview-frame"),
        image: card.querySelector("img"),
        message: card.querySelector(".source-preview-message"),
        meta: card.querySelector("small"),
      });
    }
    if (cards.size === sources.length) {
      return cards;
    }
  }

  clearDwmPreviews();
  const cards = new Map();
  const elements = sources.map((source) => {
    const card = document.createElement("div");
    card.className = "source-preview-card";
    card.dataset.sourceKey = source.key;

    const frame = document.createElement("div");
    frame.className = "source-preview-frame";

    const image = document.createElement("img");
    image.alt = source.name;

    const message = document.createElement("span");
    message.className = "source-preview-message";
    message.textContent = "等待画面";

    const title = document.createElement("strong");
    title.textContent = source.name;

    const meta = document.createElement("small");
    meta.textContent =
      source.kind === "screen"
        ? `${source.left},${source.top} ${source.width}x${source.height}`
        : `hwnd ${source.hwnd}`;

    frame.replaceChildren(image, message);
    card.replaceChildren(frame, title, meta);
    cards.set(source.key, { card, frame, image, message, meta });
    return card;
  });
  container.replaceChildren(...elements);
  sourcePreviewRenderSignatureText = renderSignature;
  return cards;
}

function sourcePreviewRenderSignature(sources) {
  return JSON.stringify(
    sources.map((source) => ({
      kind: source.kind,
      key: source.key,
      name: source.name,
      left: source.left,
      top: source.top,
      width: source.width,
      height: source.height,
      hwnd: source.hwnd,
    })),
  );
}

async function refreshSourcePreviews({ scheduled = false } = {}) {
  const status = document.querySelector("#status");
  const gate = sourcePreviewRefreshGate(
    {
      enabled: sourcePreviewsEnabled,
      layoutBusy: sourcePreviewLayoutBusy(),
      refreshing: sourcePreviewRefreshing,
      scheduled,
    },
    { retryDelay: SOURCE_PREVIEW_BUSY_RETRY_MS },
  );
  if (!gate.canRefresh) {
    if (gate.statusText) {
      status.textContent = gate.statusText;
    }
    if (gate.retryDelay !== null) {
      scheduleSourcePreviews(gate.retryDelay);
    }
    return;
  }
  sourcePreviewRefreshing = true;
  const sources = selectedPreviewSources();
  const cards = renderSourcePreviewCards(sources);
  try {
    await invoke("retain_cached_preview_sources", {
      sourceKeys: sources.map((source) => source.key),
    });
    await retainDwmPreviewSources(
      sources
        .filter((source) => source.kind === "window")
        .map((source) => source.key),
    );
    if (!sources.length) {
      if (!scheduled) {
        status.textContent = "没有可预览来源";
      }
      return;
    }

    if (!scheduled) {
      status.textContent = "刷新来源预览...";
    }
    let okCount = 0;
    for (const source of sources) {
      const card = cards.get(source.key);
      let dwmActive = false;
      try {
        if (source.kind === "window") {
          dwmActive = await syncDwmPreview(source, card);
        } else {
          card.card.classList.remove("uses-dwm");
        }
        const preview =
          source.kind === "screen"
            ? await invoke("capture_screen_region_preview_cached", {
                sourceKey: source.key,
                left: source.left,
                top: source.top,
                width: source.width,
                height: source.height,
              })
            : await invoke("capture_window_preview_cached", {
                sourceKey: source.key,
                hwnd: source.hwnd,
              });
        applySourcePreviewCardPresentation(
          card,
          sourcePreviewCardPresentation({ preview }),
        );
        okCount += 1;
      } catch (error) {
        const presentation = sourcePreviewCardPresentation({ dwmActive, error });
        if (presentation.ok) {
          applySourcePreviewCardPresentation(card, presentation);
          okCount += 1;
        } else {
          applySourcePreviewCardPresentation(card, presentation);
        }
      } finally {
        clearSourcePreviewFrames(card?.frame);
      }
    }
    if (!scheduled) {
      status.textContent = previewStatusText(okCount, sources.length);
    }
  } catch (error) {
    status.textContent = String(error);
  } finally {
    sourcePreviewRefreshing = false;
    scheduleSourcePreviews();
  }
}

function applySourcePreviewCardPresentation(card, presentation) {
  if (!card || !presentation) {
    return;
  }
  if (presentation.clearImage) {
    card.image.removeAttribute("src");
  } else if (presentation.imageSrc) {
    card.image.src = presentation.imageSrc;
  }
  card.message.textContent = presentation.message;
  card.card.classList.toggle("is-error", presentation.isError);
}

async function retainDwmPreviewSources(sourceKeys) {
  try {
    await invoke("retain_dwm_preview_sources", { sourceKeys });
  } catch (_error) {
    // Bitmap previews remain the fallback when DWM is unavailable.
  }
}

function clearDwmPreviews() {
  invoke("clear_dwm_previews").catch(() => {
    // DWM is Windows-only; unsupported platforms keep bitmap previews.
  });
}

async function syncDwmPreview(source, card) {
  const rect = visiblePreviewRect(card.frame);
  if (!rect) {
    card.card.classList.remove("uses-dwm");
    return false;
  }
  try {
    await invoke("sync_dwm_preview", {
      sourceKey: source.key,
      hwnd: source.hwnd,
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
    });
    card.card.classList.add("uses-dwm");
    return true;
  } catch (_error) {
    card.card.classList.remove("uses-dwm");
    return false;
  }
}

function selectedRegionConfigs() {
  return buildSelectedRegionConfigs({
    monitors: currentMonitors,
    region: profileRegionInputs(),
    selectedMonitorIndexes,
  });
}

function selectedWindowObjects() {
  return selectedWindowRecords(currentWindows, selectedWindowHandles);
}

function selectedWindowConfigs() {
  return buildSelectedWindowConfigs({
    selectedWindowHandles,
    windows: currentWindows,
  });
}

function selectedWindowAppConfigs() {
  return buildSelectedWindowAppConfigs({
    selectedWindowHandles,
    windows: currentWindows,
  });
}

function useRememberedWindowApps() {
  return document.querySelector("#profile-use-remembered-windows").checked;
}

function buildProfileOptions() {
  const sourceOptions = buildProfileSourceOptions({
    monitors: currentMonitors,
    region: profileRegionInputs(),
    rememberedWindows: useRememberedWindowApps(),
    selectedMonitorIndexes,
    selectedWindowHandles,
    windows: currentWindows,
  });
  return {
    ...sourceOptions,
    threshold: boundedNumber("#profile-threshold", 0.9, 0, 1),
    scales: document.querySelector("#profile-scales").value.trim() || "1.0",
    pollIntervalSeconds: Math.max(0.05, numberInput("#profile-interval-ms", 250)) / 1000,
    cooldownSeconds: Math.max(0, numberInput("#profile-cooldown", 1)),
    beep: document.querySelector("#profile-beep").checked,
    beepSeconds: Math.max(0.1, numberInput("#profile-beep-seconds", 3)),
    beepVolume: Math.round(boundedNumber("#profile-beep-volume", 100, 0, 100)),
    maxTemplates: Math.max(
      1,
      Math.floor(numberInput("#profile-max-templates", 100)),
    ),
    maxAlerts: Math.max(1, Math.floor(numberInput("#profile-max-alerts", 50))),
  };
}

function profileRegionInputs() {
  const region = {
    left: Math.round(numberInput("#profile-region-left", 0)),
    top: Math.round(numberInput("#profile-region-top", 0)),
  };
  const width = optionalPositiveInteger("#profile-region-width");
  const height = optionalPositiveInteger("#profile-region-height");
  if (width !== undefined) {
    region.width = width;
  }
  if (height !== undefined) {
    region.height = height;
  }
  return region;
}

function numberInput(selector, fallback) {
  const value = Number(document.querySelector(selector).value);
  return Number.isFinite(value) ? value : fallback;
}

function boundedNumber(selector, fallback, min, max) {
  return Math.min(max, Math.max(min, numberInput(selector, fallback)));
}

function optionalPositiveInteger(selector) {
  const text = document.querySelector(selector).value.trim();
  if (!text) {
    return undefined;
  }
  const value = Math.floor(Number(text));
  return Number.isFinite(value) && value > 0 ? value : undefined;
}

function requireProfileOptions(action = "scan") {
  const options = buildProfileOptions();
  const actionState = profileWorkflowActionState(currentProfile, options, {
    action,
    profileMonitoringActive,
  });
  if (!actionState.canRun) {
    throw new Error(actionState.reason);
  }
  return options;
}

function applyProfileSources(profileData) {
  applyingProfileSources = true;
  try {
    const hasSavedMonitors = Object.prototype.hasOwnProperty.call(
      profileData,
      "monitors",
    );
    if (hasSavedMonitors && Array.isArray(profileData.monitors)) {
      const physicalIndexes = new Set(
        currentMonitors
          .filter((monitor) => !monitor.isVirtual)
          .map((monitor) => monitor.index),
      );
      selectedMonitorIndexes = new Set(
        profileData.monitors
          .map((value) => Number(value))
          .filter((value) => physicalIndexes.has(value)),
      );
    } else {
      ensureDefaultMonitorSelection();
    }
    applyProfileRegion(profileData.region || {});
    applyProfileMatch(profileData.match || {});

    const savedWindowKeys = new Set(
      Array.isArray(profileData.windows)
        ? profileData.windows.map(profileWindowKey).filter(Boolean)
        : [],
    );
    selectedWindowHandles = new Set(
      currentWindows
        .filter((window) => savedWindowKeys.has(profileWindowKey(window)))
        .map((window) => String(window.hwnd)),
    );
    if (savedWindowKeys.size > 0) {
      document.querySelector("#profile-use-remembered-windows").checked = true;
    }
    renderMonitorList();
    renderWindowList();
    renderSourcePreviewPlaceholders();
  } finally {
    applyingProfileSources = false;
  }
}

function profileWindowKey(window) {
  if (!window || !window.title) {
    return "";
  }
  return `${window.title}\0${Number(window.ordinal || 1)}`;
}

function applyProfileRegion(region) {
  document.querySelector("#profile-region-left").value = region.left ?? 0;
  document.querySelector("#profile-region-top").value = region.top ?? 0;
  document.querySelector("#profile-region-width").value = region.width ?? "";
  document.querySelector("#profile-region-height").value = region.height ?? "";
}

function applyProfileMatch(match) {
  document.querySelector("#profile-threshold").value = match.threshold ?? 0.9;
  document.querySelector("#profile-scales").value = scaleText(match.scales);
  document.querySelector("#profile-interval-ms").value = match.interval_ms ?? 250;
  document.querySelector("#profile-cooldown").value = match.cooldown ?? 1;
  document.querySelector("#profile-beep").checked = match.beep ?? true;
  document.querySelector("#profile-beep-seconds").value =
    match.beep_seconds ?? match.beep_count ?? 3;
  document.querySelector("#profile-beep-volume").value = match.beep_volume ?? 100;
  document.querySelector("#profile-max-templates").value = match.max_templates ?? 100;
  document.querySelector("#profile-max-alerts").value =
    match.max_alerts ?? legacyMaxAlerts ?? 50;
}

function scaleText(scales) {
  if (typeof scales === "string") {
    return scales;
  }
  if (scales === undefined || scales === null) {
    return "1.0";
  }
  return Array.isArray(scales) ? scales.join(",") : String(scales);
}

async function persistLastProfile() {
  try {
    await invoke("save_last_profile", {
      profileNumber: selectedProfileNumber(),
    });
  } catch (error) {
    document.querySelector("#status").textContent = String(error);
  }
}

async function persistProfileSources() {
  if (applyingProfileSources || !currentDataDir) {
    return;
  }
  try {
    await persistLastProfile();
    await invoke("save_profile_sources", {
      profileNumber: selectedProfileNumber(),
      options: buildProfileOptions(),
    });
  } catch (error) {
    document.querySelector("#status").textContent = String(error);
  }
}

async function scanOnce() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  status.textContent = "扫描中...";
  try {
    const scan = await invoke("scan_config_text_once", {
      text: document.querySelector("#scan-config").value,
      baseDir: currentDataDir,
    });
    result.textContent = JSON.stringify(scan, null, 2);
    status.textContent = scanStatusText(scan);
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function resolveWindows() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  status.textContent = "解析窗口来源...";
  try {
    const resolution = await invoke("resolve_config_text_window_sources", {
      text: document.querySelector("#scan-config").value,
    });
    currentWindows = resolution.availableWindows;
    result.textContent = JSON.stringify(resolution, null, 2);
    status.textContent = windowResolutionStatusText(resolution);
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function startMonitoring() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  status.textContent = "启动监控...";
  try {
    const session = await invoke("start_monitoring_session", {
      text: document.querySelector("#scan-config").value,
      baseDir: currentDataDir,
    });
    profileMonitoringActive = false;
    result.textContent = JSON.stringify(session, null, 2);
    status.textContent = monitoringStatusText(session);
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function stopMonitoring() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  status.textContent = "停止监控...";
  try {
    const session = await invoke("stop_monitoring_session");
    profileMonitoringActive = false;
    result.textContent = JSON.stringify(session, null, 2);
    status.textContent = monitoringStatusText(session);
    appendLog("已请求停止");
    updateRunControls();
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function refreshMonitoringStatus() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  try {
    const session = await invoke("monitoring_session_status");
    result.textContent = JSON.stringify(session, null, 2);
    status.textContent = monitoringStatusText(session);
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

function selectedProfileNumber() {
  return Number(document.querySelector("#profile-number").value);
}

function renderProfile(profile, options = {}) {
  currentProfile = profile;
  const summary = document.querySelector("#profile-summary");
  const result = document.querySelector("#profile-result");
  const targetList = document.querySelector("#profile-targets");
  const state = profile.exists ? "已存在" : "未创建";
  const allEnabled = profile.allEnabled ? "全部启用" : "部分/全部停用";
  summary.textContent = `${state}，${profile.targets.length} 张模板，启用 ${profile.enabledCount} 张，${allEnabled}`;
  targetList.replaceChildren(...profile.targets.map(renderProfileTarget));
  selectedTargetIndex = targetSelectionIndexForProfileLoad(
    profile,
    selectedTargetIndex,
    { selectFirst: options.selectFirstTarget },
  );
  if (!profile.targets.length) {
    selectedTargetIndex = null;
    targetLastClick = {};
    const empty = document.createElement("li");
    empty.className = "target-empty";
    empty.textContent = "没有模板";
    targetList.replaceChildren(empty);
  } else if (
    selectedTargetIndex !== null &&
    selectedTargetIndex < profile.targets.length
  ) {
    selectIndexedListItem(targetList, selectedTargetIndex);
  } else {
    selectedTargetIndex = null;
    targetLastClick = {};
  }
  result.textContent = JSON.stringify(profile, null, 2);
  loadProfileTargetThumbnails(selectedProfileNumber(), profile.targets.length);
  refreshTargetListScrollbar();
}

function renderProfileTarget(target, index) {
  const item = document.createElement("li");
  item.className = "target-card";
  item.draggable = true;
  item.dataset.index = String(index);
  if (!targetEnabled(target)) {
    item.classList.add("is-disabled");
  }
  item.title = "拖拽排序，再次点击打开图片，右键打开命中菜单";
  item.addEventListener("click", () => clickProfileTarget(index));
  item.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    showTargetContextMenu(event.clientX, event.clientY, target, index);
  });
  item.addEventListener("dragstart", (event) => {
    draggedTargetIndex = index;
    item.classList.add("is-dragging");
    markSourcePreviewLayoutBusy();
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", String(index));
  });
  item.addEventListener("dragover", (event) => {
    if (draggedTargetIndex === null || draggedTargetIndex === index) {
      return;
    }
    event.preventDefault();
    const dropAfter = targetDropAfter(item, event.clientY);
    markTargetDrop(item, dropAfter);
    event.dataTransfer.dropEffect = "move";
  });
  item.addEventListener("dragleave", () => {
    item.classList.remove("is-drop-before", "is-drop-after");
  });
  item.addEventListener("drop", async (event) => {
    event.preventDefault();
    if (draggedTargetIndex === null) {
      return;
    }
    const dropAfter = targetDropAfter(item, event.clientY);
    clearTargetDropMarkers();
    const insertIndex = targetDropInsertIndex(index, dropAfter);
    await reorderProfileTarget(draggedTargetIndex, insertIndex);
  });
  item.addEventListener("dragend", () => {
    draggedTargetIndex = null;
    clearTargetDropMarkers();
    markSourcePreviewLayoutBusy();
    scheduleSourcePreviews(SOURCE_PREVIEW_BUSY_RETRY_MS);
  });

  const main = document.createElement("div");
  main.className = "target-main";
  const hitCount = targetHitCount(target);
  const actionState = targetActionState(
    target,
    index,
    currentProfile?.targets.length || 0,
  );

  const enabled = document.createElement("input");
  enabled.type = "checkbox";
  enabled.className = "target-enable-check is-custom-check";
  enabled.checked = targetEnabled(target);
  enabled.title = "启用";
  enabled.addEventListener("click", (event) => event.stopPropagation());
  enabled.addEventListener("change", () =>
    setProfileTargetEnabled(index, enabled.checked),
  );
  applyCheckIndicatorScale(enabled);

  const thumbnail = document.createElement("div");
  thumbnail.className = "target-thumb";
  const thumbImage = document.createElement("img");
  thumbImage.alt = target.name || target.id || `target-${index + 1}`;
  thumbImage.dataset.profileTargetIndex = String(index);
  const thumbPlaceholder = document.createElement("span");
  thumbPlaceholder.textContent = "PNG";
  thumbnail.replaceChildren(thumbImage, thumbPlaceholder);
  if (hitCount > 0) {
    const badge = document.createElement("b");
    badge.className = "target-hit-badge";
    badge.textContent = String(hitCount);
    thumbnail.appendChild(badge);
  }

  const text = document.createElement("div");
  text.className = "target-text";

  const title = document.createElement("strong");
  title.textContent = target.name || target.id || `target-${index + 1}`;

  const path = document.createElement("span");
  path.textContent = target.path || "-";

  const meta = document.createElement("small");
  meta.textContent = [
    `#${index + 1}`,
    target.size || "unknown size",
    `hits ${hitCount}`,
    target.id ? `id ${target.id}` : "no id",
  ].join(" · ");

  text.replaceChildren(title, path, meta);
  main.replaceChildren(enabled, thumbnail, text);

  const actions = document.createElement("div");
  actions.className = "target-actions";
  actions.replaceChildren(
    targetButton(
      "上移",
      () => reorderProfileTarget(index, actionState.moveUpInsertIndex),
      actionState.moveUpDisabled,
    ),
    targetButton(
      "下移",
      () => reorderProfileTarget(index, actionState.moveDownInsertIndex),
      actionState.moveDownDisabled,
    ),
    targetButton("打开", () => openProfileTarget(index), actionState.openDisabled),
    targetButton(
      "清零",
      () => clearProfileHitCount(actionState.targetId),
      actionState.clearHitsDisabled,
    ),
    targetButton(
      "删除",
      () => removeProfileTarget(index),
      actionState.deleteDisabled,
      "danger",
    ),
  );

  item.replaceChildren(main, actions);
  return item;
}

function selectProfileTarget(index) {
  selectedTargetIndex = index;
  selectIndexedListItem(document.querySelector("#profile-targets"), index);
}

function clickProfileTarget(index) {
  const click = recordRepeatClick(targetLastClick, index, {
    now: () => performance.now(),
  });
  selectProfileTarget(index);
  if (click.repeated) {
    openProfileTarget(index);
  }
}

function refreshTargetListScrollbar() {
  const targetList = document.querySelector("#profile-targets");
  if (!targetList) {
    return;
  }
  updateAutohideScrollbar(targetList);
  requestAnimationFrame(() => updateAutohideScrollbar(targetList));
}

function loadProfileTargetThumbnails(profileNumber, targetCount) {
  for (let index = 0; index < targetCount; index += 1) {
    loadProfileTargetThumbnail(profileNumber, index);
  }
}

async function loadProfileTargetThumbnail(profileNumber, index) {
  const image = document.querySelector(
    `img[data-profile-target-index="${index}"]`,
  );
  if (!image) {
    return;
  }
  try {
    const thumbnail = await invoke("profile_target_thumbnail", {
      profileNumber,
      index,
    });
    if (selectedProfileNumber() !== profileNumber) {
      return;
    }
    image.src = thumbnail.dataUrl;
    image.title = thumbnail.path;
    image.closest(".target-thumb")?.classList.add("has-image");
  } catch (_error) {
    image.removeAttribute("src");
  }
}

function markTargetDrop(item, dropAfter) {
  clearTargetDropMarkers();
  item.classList.toggle("is-drop-before", !dropAfter);
  item.classList.toggle("is-drop-after", dropAfter);
}

function clearTargetDropMarkers() {
  document
    .querySelectorAll(".target-card.is-dragging, .target-card.is-drop-before, .target-card.is-drop-after")
    .forEach((item) => {
      item.classList.remove("is-dragging", "is-drop-before", "is-drop-after");
    });
}

function showTargetContextMenu(x, y, target, index) {
  hideTargetContextMenu();
  const menuState = targetMenuState(target);
  const menu = document.createElement("div");
  menu.className = "target-menu";
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;
  menu.replaceChildren(
    targetMenuButton("打开图片", () => openProfileTarget(index), !menuState.canOpen),
    targetMenuButton(
      "清零命中次数",
      () => clearProfileHitCount(target.id),
      !menuState.canClearHits,
    ),
  );
  document.body.appendChild(menu);
  targetContextMenu = menu;
  keepMenuInViewport(menu);
}

function targetMenuButton(label, onClick, disabled = false) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = disabled;
  button.addEventListener("click", () => {
    hideTargetContextMenu();
    onClick();
  });
  return button;
}

function keepMenuInViewport(menu) {
  const rect = menu.getBoundingClientRect();
  const position = fitFixedMenuInViewport(rect, {
    height: window.innerHeight,
    width: window.innerWidth,
  });
  if (position.adjustedLeft) {
    menu.style.left = `${position.left}px`;
  }
  if (position.adjustedTop) {
    menu.style.top = `${position.top}px`;
  }
}

function hideTargetContextMenu() {
  if (targetContextMenu) {
    targetContextMenu.remove();
    targetContextMenu = null;
  }
}

function targetButton(label, onClick, disabled = false, variant = "") {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = disabled;
  if (variant) {
    button.className = variant;
  }
  button.addEventListener("click", (event) => {
    event.stopPropagation();
    onClick();
  });
  return button;
}

async function loadProfile(options = {}) {
  const status = document.querySelector("#status");
  try {
    const profile = await invoke("load_profile", {
      profileNumber: selectedProfileNumber(),
    });
    renderProfile(profile, options);
    applyProfileSources(profile.profile || {});
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function switchProfile() {
  await persistLastProfile();
  await loadProfile({ selectFirstTarget: true });
}

async function normalizeProfile() {
  const status = document.querySelector("#status");
  status.textContent = "规范化 profile...";
  try {
    await invoke("normalize_profile", {
      profileNumber: selectedProfileNumber(),
    });
    await loadProfile();
    status.textContent = "Ready";
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function toggleProfileTargets() {
  const status = document.querySelector("#status");
  status.textContent = "更新模板启用状态...";
  try {
    const editResult = await invoke("toggle_all_profile_targets", {
      profileNumber: selectedProfileNumber(),
    });
    await loadProfile();
    status.textContent = profileTargetsEnabledStatusText(editResult);
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function setProfileTargetEnabled(index, enabled) {
  const status = document.querySelector("#status");
  status.textContent = "更新模板启用状态...";
  try {
    const editResult = await invoke("set_profile_target_enabled", {
      profileNumber: selectedProfileNumber(),
      index,
      enabled,
    });
    await loadProfile();
    status.textContent = profileTargetsEnabledStatusText(editResult);
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function reorderProfileTarget(fromIndex, insertIndex) {
  const status = document.querySelector("#status");
  status.textContent = "移动模板...";
  try {
    const editResult = await invoke("reorder_profile_target", {
      profileNumber: selectedProfileNumber(),
      fromIndex,
      insertIndex,
    });
    applyProfileEditSelection(editResult);
    await loadProfile();
    status.textContent = "Ready";
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function removeProfileTarget(index) {
  if (!confirm(`删除第 ${index + 1} 个模板？`)) {
    return;
  }
  const status = document.querySelector("#status");
  status.textContent = "删除模板...";
  try {
    const editResult = await invoke("remove_profile_target", {
      profileNumber: selectedProfileNumber(),
      index,
    });
    applyProfileEditSelection(editResult);
    await loadProfile();
    status.textContent = "Ready";
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function removeSelectedProfileTarget() {
  const status = document.querySelector("#status");
  if (selectedTargetIndex === null || selectedTargetIndex === undefined) {
    status.textContent = "请先选择一个模板";
    return;
  }
  await removeProfileTarget(selectedTargetIndex);
}

async function clearProfileTargets() {
  if (!confirm("清空当前 profile 的全部模板？")) {
    return;
  }
  const status = document.querySelector("#status");
  status.textContent = "清空模板...";
  try {
    const editResult = await invoke("clear_profile_targets", {
      profileNumber: selectedProfileNumber(),
    });
    applyProfileEditSelection(editResult);
    await loadProfile();
    status.textContent = "Ready";
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function clearProfileHitCount(targetId) {
  const status = document.querySelector("#status");
  status.textContent = "清除命中次数...";
  try {
    const editResult = await invoke("clear_profile_target_hit_count", {
      profileNumber: selectedProfileNumber(),
      targetId,
    });
    applyProfileEditSelection(editResult);
    await loadProfile();
    status.textContent = "Ready";
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function openProfileTarget(index) {
  const status = document.querySelector("#status");
  status.textContent = "打开图片...";
  try {
    const result = await invoke("open_profile_target_file", {
      profileNumber: selectedProfileNumber(),
      index,
    });
    status.textContent = `Ready - ${result.path}`;
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function addProfilePaths() {
  const status = document.querySelector("#status");
  const pathText = document.querySelector("#profile-import-paths").value;
  const request = profileImportRequest(pathText, profileImportLimitInput());
  if (!request.hasPaths) {
    status.textContent = "没有可导入路径";
    return;
  }
  await importProfilePaths(request, true);
}

async function selectProfilePngs() {
  const status = document.querySelector("#status");
  status.textContent = "选择图片...";
  try {
    const imagePaths = await invoke("select_profile_template_pngs");
    const request = profileImportRequest(imagePaths, profileImportLimitInput());
    if (!request.hasPaths) {
      status.textContent = "Ready";
      return;
    }
    await importProfilePaths(request, false);
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function pasteProfileImages() {
  const status = document.querySelector("#status");
  status.textContent = "粘贴图片...";
  try {
    const importResult = await invoke("paste_profile_template_images", {
      profileNumber: selectedProfileNumber(),
      maxTemplates: profileImportLimitInput(),
    });
    applyProfileEditSelection(importResult);
    await loadProfile();
    status.textContent = profileImportStatusText(importResult);
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

function profileImportLimitInput() {
  return document.querySelector("#profile-max-templates").value;
}

async function importProfilePaths(request, clearTextInput) {
  const status = document.querySelector("#status");
  status.textContent = "导入模板...";
  try {
    const importResult = await invoke("add_profile_template_pngs", {
      profileNumber: selectedProfileNumber(),
      imagePaths: request.imagePaths,
      maxTemplates: request.maxTemplates,
    });
    applyProfileEditSelection(importResult);
    if (clearTextInput) {
      document.querySelector("#profile-import-paths").value = "";
    }
    await loadProfile();
    status.textContent = profileImportStatusText(importResult);
  } catch (error) {
    document.querySelector("#profile-result").textContent = String(error);
    status.textContent = String(error);
  }
}

function applyProfileEditSelection(editResult) {
  selectedTargetIndex = targetSelectionIndexFromEditResult(
    editResult,
    selectedTargetIndex,
  );
  targetLastClick = {};
}

async function buildProfileConfigPreview() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  status.textContent = "生成 profile 扫描配置...";
  try {
    await persistProfileSources();
    const config = await invoke("build_profile_watch_config", {
      profileNumber: selectedProfileNumber(),
      options: requireProfileOptions("preview"),
    });
    const text = JSON.stringify(config, null, 2);
    document.querySelector("#scan-config").value = text;
    result.textContent = text;
    status.textContent = "Ready";
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function captureProfileTarget() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#profile-result");
  status.textContent = "截图作模板...";
  try {
    await persistProfileSources();
    const importResult = await invoke("capture_profile_source_template", {
      profileNumber: selectedProfileNumber(),
      options: requireProfileOptions("capture-target"),
    });
    applyProfileEditSelection(importResult);
    await loadProfile();
    result.textContent = JSON.stringify(importResult, null, 2);
    status.textContent = profileImportStatusText(importResult);
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function scanProfileOnce() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  status.textContent = "Profile 扫描中...";
  try {
    await persistProfileSources();
    const scan = await invoke("scan_profile_once", {
      profileNumber: selectedProfileNumber(),
      options: requireProfileOptions("scan"),
    });
    result.textContent = JSON.stringify(scan, null, 2);
    status.textContent = scanStatusText(scan);
    appendLog(scanStatusText(scan));
    if (scan.hitCount > 0) {
      await loadProfile();
    }
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function openEvidenceDir() {
  const status = document.querySelector("#status");
  status.textContent = "打开证据目录...";
  try {
    const result = await invoke("open_evidence_dir");
    status.textContent = `Ready - ${result.path}`;
    appendLog(`打开证据目录：${result.path}`);
  } catch (error) {
    document.querySelector("#scan-result").textContent = String(error);
    status.textContent = String(error);
  }
}

async function startProfileMonitoring() {
  const status = document.querySelector("#status");
  const result = document.querySelector("#scan-result");
  status.textContent = "启动 profile 监控...";
  try {
    await persistProfileSources();
    const session = await invoke("start_profile_monitoring_session", {
      profileNumber: selectedProfileNumber(),
      options: requireProfileOptions("start-monitoring"),
    });
    profileMonitoringActive = true;
    result.textContent = JSON.stringify(session, null, 2);
    status.textContent = monitoringStatusText(session);
    appendLog("监控中");
    updateRunControls();
  } catch (error) {
    result.textContent = String(error);
    status.textContent = String(error);
  }
}

async function toggleProfileMonitoring() {
  if (profileMonitoringActive) {
    await stopMonitoring();
  } else {
    await startProfileMonitoring();
  }
}

function updateRunControls() {
  const button = document.querySelector("#profile-monitor-start");
  if (!button) {
    return;
  }
  button.textContent = profileMonitoringActive ? "停止监控" : "开始监控";
  button.classList.toggle("is-running", profileMonitoringActive);
}

document.querySelector("#refresh").addEventListener("click", refresh);
document.querySelector("#ocr-probe").addEventListener("click", probeOcrBackend);
document
  .querySelector("#startup-refresh")
  .addEventListener("click", refreshStartupStatus);
document
  .querySelector("#startup-enable")
  .addEventListener("click", () => setStartup(true));
document
  .querySelector("#startup-disable")
  .addEventListener("click", () => setStartup(false));
document
  .querySelector("#startup-toggle")
  .addEventListener("change", (event) => setStartup(event.target.checked));
document
  .querySelector("#capture-preview")
  .addEventListener("click", capturePreview);
document.querySelector("#refresh-windows").addEventListener("click", refreshWindows);
document
  .querySelector("#capture-window-preview")
  .addEventListener("click", captureWindowPreview);
document
  .querySelector("#refresh-source-previews")
  .addEventListener("click", () => refreshSourcePreviews());
document.querySelector("#scan-once").addEventListener("click", scanOnce);
document.querySelector("#resolve-windows").addEventListener("click", resolveWindows);
document
  .querySelector("#monitor-start")
  .addEventListener("click", startMonitoring);
document.querySelector("#monitor-stop").addEventListener("click", stopMonitoring);
document
  .querySelector("#monitor-status")
  .addEventListener("click", refreshMonitoringStatus);
document
  .querySelector("#profile-load")
  .addEventListener("click", () => loadProfile({ selectFirstTarget: true }));
document
  .querySelector("#profile-number")
  .addEventListener("change", switchProfile);
document
  .querySelector("#profile-use-remembered-windows")
  .addEventListener("change", persistProfileSources);
[
  "#profile-region-left",
  "#profile-region-top",
  "#profile-region-width",
  "#profile-region-height",
  "#profile-threshold",
  "#profile-scales",
  "#profile-interval-ms",
  "#profile-cooldown",
  "#profile-beep",
  "#profile-beep-seconds",
  "#profile-beep-volume",
  "#profile-max-templates",
  "#profile-max-alerts",
].forEach((selector) => {
  document.querySelector(selector).addEventListener("change", () => {
    renderSourcePreviewPlaceholders();
    scheduleSourcePreviews(0);
    persistProfileSources();
  });
});
document
  .querySelector("#profile-normalize")
  .addEventListener("click", normalizeProfile);
document
  .querySelector("#profile-toggle-all")
  .addEventListener("click", toggleProfileTargets);
document
  .querySelector("#profile-delete-selected")
  .addEventListener("click", removeSelectedProfileTarget);
document
  .querySelector("#profile-clear-all")
  .addEventListener("click", clearProfileTargets);
document
  .querySelector("#profile-add-paths")
  .addEventListener("click", addProfilePaths);
document
  .querySelector("#profile-select-pngs")
  .addEventListener("click", selectProfilePngs);
document
  .querySelector("#profile-paste-images")
  .addEventListener("click", pasteProfileImages);
document
  .querySelector("#profile-capture-target")
  .addEventListener("click", captureProfileTarget);
document
  .querySelector("#profile-build-config")
  .addEventListener("click", buildProfileConfigPreview);
document
  .querySelector("#profile-scan-once")
  .addEventListener("click", scanProfileOnce);
document
  .querySelector("#profile-monitor-start")
  .addEventListener("click", toggleProfileMonitoring);
document
  .querySelector("#profile-monitor-stop")
  .addEventListener("click", stopMonitoring);
document
  .querySelector("#open-evidence-dir")
  .addEventListener("click", openEvidenceDir);
installEntryCursorEndHandlers(
  document.querySelectorAll("input[type='text'], input[type='number']"),
);
installCustomCheckIndicators(document);
const profileTargetList = document.querySelector("#profile-targets");
installWheelScroll(profileTargetList);
installAutohideScrollbar(profileTargetList);
document.addEventListener("click", hideTargetContextMenu);
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    hideTargetContextMenu();
  }
  if (shouldHandleProfilePaste(event, document.activeElement)) {
    event.preventDefault();
    pasteProfileImages();
  }
});
window.addEventListener(
  "scroll",
  () => {
    hideTargetContextMenu();
    markSourcePreviewLayoutBusy();
    coverSourcePreviewFrames();
    clearDwmPreviews();
    scheduleSourcePreviews(SOURCE_PREVIEW_BUSY_RETRY_MS);
  },
  true,
);
window.addEventListener("resize", () => {
  markSourcePreviewLayoutBusy();
  coverSourcePreviewFrames();
  clearDwmPreviews();
  installCustomCheckIndicators(document);
  scheduleSourcePreviews(SOURCE_PREVIEW_BUSY_RETRY_MS);
});
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    disableSourcePreviews();
  } else {
    coverSourcePreviewFrames();
    enableSourcePreviews(SOURCE_PREVIEW_BUSY_RETRY_MS);
  }
});
window.addEventListener("focus", () => {
  coverSourcePreviewFrames();
  enableSourcePreviews(SOURCE_PREVIEW_BUSY_RETRY_MS);
});
refresh().finally(() => {
  enableSourcePreviews(SOURCE_PREVIEW_BUSY_RETRY_MS);
});
attachMonitoringEvents().catch((error) => {
  document.querySelector("#status").textContent = String(error);
});
