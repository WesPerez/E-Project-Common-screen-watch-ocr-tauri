export function moveTextCursorToEnd(input) {
  if (!input || typeof input.setSelectionRange !== "function") {
    return false;
  }
  const end = String(input.value ?? "").length;
  try {
    input.setSelectionRange(end, end);
    return true;
  } catch (_error) {
    return false;
  }
}

export function installEntryCursorEndHandlers(inputs, options = {}) {
  const schedule = options.schedule || ((callback) => window.setTimeout(callback, 0));
  for (const input of inputs || []) {
    for (const eventName of ["focus", "mouseup", "click"]) {
      input.addEventListener(eventName, () => {
        schedule(() => moveTextCursorToEnd(input));
      });
    }
  }
}

export function scrollElementByWheel(element, event) {
  if (!element || !event) {
    return false;
  }
  const delta = Number(event.deltaY || event.deltaX || 0);
  if (!Number.isFinite(delta) || delta === 0) {
    return false;
  }
  const before = element.scrollTop;
  element.scrollTop += delta;
  if (element.scrollTop === before) {
    return false;
  }
  event.preventDefault?.();
  return true;
}

export function installWheelScroll(element) {
  if (!element) {
    return;
  }
  element.addEventListener("wheel", (event) => scrollElementByWheel(element, event), {
    passive: false,
  });
}

export function scrollbarOverflowState(element, options = {}) {
  const axis = options.axis || "vertical";
  const tolerance = Number(options.tolerance ?? 1);
  const safeTolerance = Number.isFinite(tolerance) ? tolerance : 1;
  const scrollSize = Number(
    axis === "horizontal" ? element?.scrollWidth : element?.scrollHeight,
  );
  const clientSize = Number(
    axis === "horizontal" ? element?.clientWidth : element?.clientHeight,
  );
  const hasOverflow =
    Number.isFinite(scrollSize) &&
    Number.isFinite(clientSize) &&
    scrollSize > clientSize + safeTolerance;

  return {
    axis,
    clientSize: Number.isFinite(clientSize) ? clientSize : 0,
    hasOverflow,
    scrollSize: Number.isFinite(scrollSize) ? scrollSize : 0,
  };
}

export function updateAutohideScrollbar(element, options = {}) {
  const state = scrollbarOverflowState(element, options);
  if (!element) {
    return state;
  }
  const hiddenClass = options.hiddenClass || "is-scrollbar-hidden";
  const visibleClass = options.visibleClass || "has-scrollbar";
  const overflowProperty = state.axis === "horizontal" ? "overflowX" : "overflowY";
  const visibleOverflow = options.visibleOverflow || "auto";
  const hiddenOverflow = options.hiddenOverflow || "hidden";

  if (state.hasOverflow) {
    element.classList?.add?.(visibleClass);
    element.classList?.remove?.(hiddenClass);
    if (element.style) {
      element.style[overflowProperty] = visibleOverflow;
    }
  } else {
    element.classList?.add?.(hiddenClass);
    element.classList?.remove?.(visibleClass);
    if (element.style) {
      element.style[overflowProperty] = hiddenOverflow;
    }
  }
  if (element.dataset) {
    element.dataset.scrollbarState = state.hasOverflow ? "visible" : "hidden";
  }
  return state;
}

export function installAutohideScrollbar(element, options = {}) {
  if (!element) {
    return null;
  }
  element.classList?.add?.(options.rootClass || "autohide-scrollbar");
  const update = () => updateAutohideScrollbar(element, options);
  update();

  const ResizeObserverCtor =
    options.ResizeObserver ||
    (typeof ResizeObserver !== "undefined" ? ResizeObserver : null);
  const observer = ResizeObserverCtor ? new ResizeObserverCtor(update) : null;
  observer?.observe?.(element);

  const windowTarget =
    options.window || (typeof window !== "undefined" ? window : null);
  windowTarget?.addEventListener?.("resize", update);

  return {
    disconnect() {
      observer?.disconnect?.();
      windowTarget?.removeEventListener?.("resize", update);
    },
    update,
  };
}

export function setRestoreOverlayCovered(element, covered = true, options = {}) {
  if (!element) {
    return false;
  }
  const coveredClass = options.coveredClass || "is-restore-covered";
  if (covered) {
    element.classList?.add?.(coveredClass);
  } else {
    element.classList?.remove?.(coveredClass);
  }
  if (element.dataset) {
    element.dataset.restoreOverlay = covered ? "covered" : "clear";
  }
  return true;
}

export function updateRestoreOverlayFrames(root, covered = true, options = {}) {
  if (!root) {
    return 0;
  }
  const selector = options.selector || ".source-preview-frame";
  const frames = [];
  if (root.matches?.(selector)) {
    frames.push(root);
  }
  frames.push(...Array.from(root.querySelectorAll?.(selector) || []));
  let touched = 0;
  for (const frame of frames) {
    if (setRestoreOverlayCovered(frame, covered, options)) {
      touched += 1;
    }
  }
  return touched;
}

export function coverRestorePreviewFrames(root, options = {}) {
  return updateRestoreOverlayFrames(root, true, options);
}

export function clearRestorePreviewFrames(root, options = {}) {
  return updateRestoreOverlayFrames(root, false, options);
}

function countFromEitherCase(object, camelName, snakeName) {
  const value = Number(object?.[camelName] ?? object?.[snakeName] ?? 0);
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;
}

function formatDurationMs(value) {
  const ms = Number(value);
  if (!Number.isFinite(ms) || ms <= 0) {
    return "0ms";
  }
  if (ms < 1000) {
    return `${Math.trunc(ms)}ms`;
  }
  return `${(ms / 1000).toFixed(ms < 10000 ? 2 : 1)}s`;
}

function textFromEitherCase(object, camelName, snakeName) {
  const value = object?.[camelName] ?? object?.[snakeName] ?? "";
  return typeof value === "string" ? value.trim() : "";
}

export function missingSourceSummary(object) {
  const skippedWindows = countFromEitherCase(
    object,
    "skippedWindows",
    "skipped_windows",
  );
  const skippedWindowApps = countFromEitherCase(
    object,
    "skippedWindowApps",
    "skipped_window_apps",
  );
  const parts = [];
  if (skippedWindows > 0) {
    parts.push(`缺失 ${skippedWindows} 个窗口`);
  }
  if (skippedWindowApps > 0) {
    parts.push(`缺失 ${skippedWindowApps} 个应用窗口`);
  }
  return {
    skippedWindowApps,
    skippedWindows,
    text: parts.join("，"),
    total: skippedWindows + skippedWindowApps,
  };
}

export function monitorErrorSummary(object, options = {}) {
  const errorText =
    String(options.tickError || "").trim() ||
    textFromEitherCase(object, "lastError", "last_error");
  const errorCount = countFromEitherCase(object, "errorCount", "error_count");
  if (errorText) {
    return {
      errorCount,
      text: `错误: ${errorText}`,
    };
  }
  return {
    errorCount,
    text: errorCount > 0 ? `错误 ${errorCount} 次` : "",
  };
}

export function scanStatusText(scan, options = {}) {
  const prefix = options.prefix || "Ready";
  const hitCount = countFromEitherCase(scan, "hitCount", "hit_count");
  const missing = missingSourceSummary(scan);
  const warning = missing.text ? `，${missing.text}` : "";
  return `${prefix} - ${hitCount} hits${warning}`;
}

export function previewStatusText(okCount, totalCount, options = {}) {
  const prefix = options.prefix || "Ready";
  const ok = Math.max(0, Math.trunc(Number(okCount) || 0));
  const total = Math.max(0, Math.trunc(Number(totalCount) || 0));
  const failed = Math.max(0, total - Math.min(ok, total));
  const warning = failed > 0 ? `，失败 ${failed} 个` : "";
  return `${prefix} - ${Math.min(ok, total)}/${total} previews${warning}`;
}

export function evidenceDirectoryPath(result = {}) {
  return String(result?.path || result?.dir || "").trim();
}

export function evidenceDirectoryStatusText(result = {}, options = {}) {
  const prefix = options.prefix || "Ready";
  const path = evidenceDirectoryPath(result);
  return path ? `${prefix} - ${path}` : `${prefix} - 证据目录已打开`;
}

export function evidenceDirectoryLogText(result = {}) {
  const path = evidenceDirectoryPath(result);
  return path ? `打开证据目录：${path}` : "打开证据目录";
}

export function sourcePreviewRefreshGate(state = {}, options = {}) {
  const retryDelay = Math.max(
    0,
    Math.trunc(Number(options.retryDelay ?? 250) || 0),
  );
  const scheduled = Boolean(state.scheduled);
  if (!state.enabled && scheduled) {
    return {
      canRefresh: false,
      kind: "disabled",
      retryDelay: null,
      statusText: "",
    };
  }
  if (state.refreshing) {
    return {
      canRefresh: false,
      kind: "refreshing",
      retryDelay,
      statusText: scheduled
        ? ""
        : options.refreshingText || "来源预览正在刷新",
    };
  }
  if (state.layoutBusy) {
    return {
      canRefresh: false,
      kind: "layout-busy",
      retryDelay,
      statusText: scheduled
        ? ""
        : options.layoutBusyText || "布局调整中，稍后刷新预览",
    };
  }
  return {
    canRefresh: true,
    kind: "ready",
    retryDelay: null,
    statusText: "",
  };
}

export function sourcePreviewCaptureFailureState(dwmActive, error) {
  if (dwmActive) {
    return {
      clearImage: false,
      isError: false,
      message: "",
      ok: true,
    };
  }
  return {
    clearImage: true,
    isError: true,
    message: String(error),
    ok: false,
  };
}

export function sourcePreviewCardPresentation(outcome = {}) {
  if (outcome.preview) {
    return {
      clearImage: false,
      imageSrc: outcome.preview.dataUrl || "",
      isError: false,
      message: "",
      ok: true,
    };
  }
  return sourcePreviewCaptureFailureState(
    Boolean(outcome.dwmActive),
    outcome.error,
  );
}

export function visiblePreviewRect(element, viewport = {}) {
  if (!element || typeof element.getBoundingClientRect !== "function") {
    return null;
  }
  const windowLike = typeof window !== "undefined" ? window : {};
  const viewportWidth = finiteNumber(
    viewport.width ?? viewport.innerWidth ?? windowLike.innerWidth,
    0,
  );
  const viewportHeight = finiteNumber(
    viewport.height ?? viewport.innerHeight ?? windowLike.innerHeight,
    0,
  );
  const minSize = Math.max(0, finiteNumber(viewport.minSize, 2));
  if (viewportWidth <= 0 || viewportHeight <= 0) {
    return null;
  }

  const rect = element.getBoundingClientRect();
  const left = Math.max(0, finiteNumber(rect.left, 0));
  const top = Math.max(0, finiteNumber(rect.top, 0));
  const right = Math.min(viewportWidth, finiteNumber(rect.right, 0));
  const bottom = Math.min(viewportHeight, finiteNumber(rect.bottom, 0));
  const width = right - left;
  const height = bottom - top;
  if (width < minSize || height < minSize) {
    return null;
  }
  return { height, left, top, width };
}

export function selectedPhysicalMonitors(monitors = [], selectedMonitorIndexes = new Set()) {
  return (monitors || []).filter(
    (monitor) =>
      !monitor?.isVirtual &&
      selectionHas(selectedMonitorIndexes, monitor?.index, Number(monitor?.index)),
  );
}

export function selectedWindowRecords(windows = [], selectedWindowHandles = new Set()) {
  return (windows || []).filter((windowRecord) =>
    selectionHas(
      selectedWindowHandles,
      windowRecord?.hwnd,
      String(windowRecord?.hwnd),
    ),
  );
}

export function buildSelectedPreviewSources(state = {}) {
  const region = normalizedRegion(state.region);
  const screenSources = selectedPhysicalMonitors(
    state.monitors,
    state.selectedMonitorIndexes,
  ).map((monitor) => ({
    kind: "screen",
    key: `screen:monitor-${monitor.index}`,
    name: monitor.name || `monitor-${monitor.index}`,
    left: Number(monitor.left || 0) + region.left,
    top: Number(monitor.top || 0) + region.top,
    width: region.width || monitor.width,
    height: region.height || monitor.height,
  }));
  const windowSources = selectedWindowRecords(
    state.windows,
    state.selectedWindowHandles,
  ).map((windowRecord) => ({
    kind: "window",
    key: `app:${windowRecord.hwnd}`,
    name: windowRecord.display || windowRecord.title,
    hwnd: windowRecord.hwnd,
  }));
  return [...screenSources, ...windowSources];
}

export function buildSelectedRegionConfigs(state = {}) {
  const region = normalizedRegion(state.region);
  return selectedPhysicalMonitors(
    state.monitors,
    state.selectedMonitorIndexes,
  ).map((monitor) => ({
    name: `monitor-${monitor.index}`,
    monitor: monitor.index,
    ...region,
  }));
}

export function buildSelectedWindowConfigs(state = {}) {
  return selectedWindowRecords(state.windows, state.selectedWindowHandles).map(
    (windowRecord) => ({
      name: windowRecord.display || windowRecord.title,
      title: windowRecord.title,
      display: windowRecord.display,
      hwnd: windowRecord.hwnd,
      ordinal: windowRecord.ordinal,
    }),
  );
}

export function buildSelectedWindowAppConfigs(state = {}) {
  return selectedWindowRecords(state.windows, state.selectedWindowHandles).map(
    (windowRecord) => ({
      title: windowRecord.title,
      ordinal: windowRecord.ordinal,
    }),
  );
}

function windowAppKey(app = {}) {
  const title = String(app.title || "").trim();
  if (!title) {
    return "";
  }
  const ordinal = Math.max(1, Math.trunc(Number(app.ordinal || 1)));
  return `${title}\0${ordinal}`;
}

function normalizedWindowApp(app = {}) {
  const title = String(app.title || "").trim();
  if (!title) {
    return null;
  }
  return {
    title,
    ordinal: Math.max(1, Math.trunc(Number(app.ordinal || 1))),
  };
}

export function buildRememberedWindowAppConfigs(state = {}) {
  const byKey = new Map();
  const add = (app) => {
    const normalized = normalizedWindowApp(app);
    if (!normalized) {
      return;
    }
    byKey.set(windowAppKey(normalized), normalized);
  };
  (state.rememberedWindowApps || []).forEach(add);
  buildSelectedWindowAppConfigs(state).forEach(add);
  return [...byKey.values()];
}

export function buildProfileSourceOptions(state = {}) {
  const rememberedWindows = Boolean(state.rememberedWindows);
  return {
    profileRegion: normalizedRegion(state.region),
    regions: buildSelectedRegionConfigs(state),
    windows: rememberedWindows ? [] : buildSelectedWindowConfigs(state),
    windowApps: rememberedWindows ? buildRememberedWindowAppConfigs(state) : [],
  };
}

export function profileSourceOptionsHaveSources(options = {}) {
  return (
    (options.regions || []).length > 0 ||
    (options.windows || []).length > 0 ||
    (options.windowApps || []).length > 0
  );
}

export function profileWorkflowActionState(profile = {}, sourceOptions = {}, state = {}) {
  const action = String(state.action || "scan").toLowerCase();
  const enabledTargetCount = profileEnabledTargetCount(profile);
  const hasSources = profileSourceOptionsHaveSources(sourceOptions);
  const monitoringActive = Boolean(state.profileMonitoringActive);
  const isStartMonitoring =
    action === "monitor" ||
    action === "monitoring" ||
    action === "start-monitoring";
  const isCaptureTarget =
    action === "capture" ||
    action === "capture-target" ||
    action === "screenshot-template";

  let reason = "";
  if (!isCaptureTarget && enabledTargetCount <= 0) {
    reason = state.noTargetsText || "请先添加并启用至少一个模板";
  } else if (!hasSources) {
    reason = state.noSourcesText || "请至少选择一个物理屏幕或窗口";
  } else if (isStartMonitoring && monitoringActive) {
    reason = state.alreadyMonitoringText || "Profile 监控已在运行";
  }

  return {
    action,
    canRun: !reason,
    enabledTargetCount,
    hasSources,
    monitoringActive,
    reason,
    statusText: reason,
  };
}

function profileEnabledTargetCount(profile = {}) {
  const explicit = Number(profile?.enabledCount ?? profile?.enabled_count);
  if (Number.isFinite(explicit)) {
    return Math.max(0, Math.trunc(explicit));
  }
  return (profile?.targets || []).filter(targetEnabled).length;
}

export function profileImportRequest(input, maxTemplates) {
  const rawItems = Array.isArray(input)
    ? input
    : String(input ?? "").split(/[\n\r]+/);
  const imagePaths = rawItems
    .map((item) => String(item ?? "").trim())
    .filter(Boolean);
  const numericLimit = Number(maxTemplates);
  const flooredLimit = Number.isFinite(numericLimit) ? Math.floor(numericLimit) : 1;
  return {
    hasPaths: imagePaths.length > 0,
    imagePaths,
    maxTemplates: Math.max(1, flooredLimit),
  };
}

export function shouldHandleProfilePaste(event, activeElement) {
  const key = String(event?.key || "").toLowerCase();
  const pasteShortcut = key === "v" && (event?.ctrlKey || event?.metaKey) && !event?.altKey;
  if (!pasteShortcut) {
    return false;
  }
  if (!activeElement) {
    return true;
  }
  const tagName = String(activeElement.tagName || "").toLowerCase();
  if (["input", "textarea", "select"].includes(tagName)) {
    return false;
  }
  if (activeElement.isContentEditable) {
    return false;
  }
  const role = String(activeElement.getAttribute?.("role") || "").toLowerCase();
  return role !== "textbox";
}

export function profileImportStatusText(result = {}, options = {}) {
  const prefix = options.prefix || "Ready";
  const addedCount = countFromEitherCase(result, "addedCount", "added_count");
  const prunedCount = countFromEitherCase(result, "prunedCount", "pruned_count");
  const targets = Array.isArray(result?.targets) ? result.targets.length : 0;
  const parts = [`导入 ${addedCount} 张`];
  if (prunedCount > 0) {
    parts.push(`裁剪 ${prunedCount} 张`);
  }
  parts.push(`当前 ${targets} 张`);
  return `${prefix} - ${parts.join("，")}`;
}

export function profileTargetsEnabledStatusText(result = {}, options = {}) {
  const prefix = options.prefix || "Ready";
  const targets = Array.isArray(result?.targets) ? result.targets : [];
  const totalCount = targets.length;
  const explicitEnabledCount = Number(result?.enabledCount ?? result?.enabled_count);
  const enabledCount = Number.isFinite(explicitEnabledCount)
    ? Math.max(0, Math.trunc(explicitEnabledCount))
    : targets.filter(targetEnabled).length;
  return `${prefix} - 当前 ${totalCount} 张模板，启用 ${enabledCount} 张`;
}

export function profileToggleAllLabel(profile = {}) {
  const targets = Array.isArray(profile?.targets) ? profile.targets : [];
  const allEnabled =
    targets.length > 0 &&
    (profile?.allEnabled === true ||
      profile?.all_enabled === true ||
      targets.every(targetEnabled));
  return allEnabled ? "反选" : "全选";
}

function normalizedRegion(region = {}) {
  const normalized = {
    left: Math.round(finiteNumber(region.left, 0)),
    top: Math.round(finiteNumber(region.top, 0)),
  };
  const width = positiveInteger(region.width);
  const height = positiveInteger(region.height);
  if (width !== undefined) {
    normalized.width = width;
  }
  if (height !== undefined) {
    normalized.height = height;
  }
  return normalized;
}

function finiteNumber(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function positiveInteger(value) {
  if (value === undefined || value === null || value === "") {
    return undefined;
  }
  const number = Math.floor(Number(value));
  return Number.isFinite(number) && number > 0 ? number : undefined;
}

function selectionHas(selection, ...candidates) {
  if (!selection) {
    return false;
  }
  if (typeof selection.has === "function") {
    return candidates.some((candidate) => selection.has(candidate));
  }
  if (Array.isArray(selection)) {
    return candidates.some((candidate) => selection.includes(candidate));
  }
  return false;
}

export function windowResolutionStatusText(resolution, options = {}) {
  const prefix = options.prefix || "Ready";
  const windows = Array.isArray(resolution?.windows) ? resolution.windows.length : 0;
  const missingApps = Array.isArray(resolution?.missingWindowApps)
    ? resolution.missingWindowApps.length
    : Array.isArray(resolution?.missing_window_apps)
      ? resolution.missing_window_apps.length
      : 0;
  const warning = missingApps > 0 ? `，缺失 ${missingApps} 个应用窗口` : "";
  return `${prefix} - ${windows} windows${warning}`;
}

export function monitoringStatusText(snapshot, options = {}) {
  const runningText = options.runningText || "监控中";
  const readyText = options.readyText || "Ready";
  const running = Boolean(snapshot?.running);
  let text = running ? runningText : readyText;
  const tickHitCount = Number(options.tickHitCount ?? 0);
  if (Number.isFinite(tickHitCount) && tickHitCount > 0) {
    text = `${runningText} - 本轮 ${Math.trunc(tickHitCount)} hits`;
  }
  const missing = missingSourceSummary(snapshot);
  if (missing.text) {
    text = `${text}，${missing.text}`;
  }
  const error = monitorErrorSummary(snapshot, options);
  if (error.text) {
    text = `${text}，${error.text}`;
  }
  return text;
}

export function monitoringProgressLogText(snapshot = {}, options = {}) {
  const tickCount = countFromEitherCase(snapshot, "tickCount", "tick_count");
  const tickMatchCount =
    countFromEitherCase(options, "tickMatchCount", "tick_match_count") ||
    countFromEitherCase(snapshot, "lastTickMatchCount", "last_tick_match_count");
  const tickHitCount = countFromEitherCase(
    options,
    "tickHitCount",
    "tick_hit_count",
  ) || countFromEitherCase(snapshot, "lastTickHitCount", "last_tick_hit_count");
  const tickScanMs =
    countFromEitherCase(options, "tickScanMs", "tick_scan_ms") ||
    countFromEitherCase(snapshot, "lastTickScanMs", "last_tick_scan_ms");
  const hitCount = countFromEitherCase(snapshot, "hitCount", "hit_count");
  const regionCount = countFromEitherCase(snapshot, "regionCount", "region_count");
  const windowCount = countFromEitherCase(snapshot, "windowCount", "window_count");
  const parts = [
    `第 ${tickCount} 轮`,
    `耗时 ${formatDurationMs(tickScanMs)}`,
    `命中 ${tickMatchCount}`,
    `报警 ${tickHitCount}`,
    `累计 ${hitCount}`,
    `来源 ${regionCount} 屏 / ${windowCount} 应用`,
  ];
  const tickError = String(options.tickError || options.tick_error || "").trim();
  if (tickError) {
    parts.push(`错误: ${tickError}`);
  }
  return parts.join(" | ");
}

export function monitoringSessionGeneration(snapshot = {}) {
  const value = Number(
    snapshot?.generation ??
      snapshot?.sessionGeneration ??
      snapshot?.session_generation ??
      0,
  );
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;
}

export function monitoringEventFreshness(payload = {}, state = {}) {
  const generation = monitoringSessionGeneration(payload?.snapshot || payload);
  const currentGeneration = monitoringSessionGeneration({
    generation: state.currentGeneration,
  });
  const stoppedGeneration = monitoringSessionGeneration({
    generation: state.stoppedGeneration,
  });
  const kind = String(payload?.kind || "").toLowerCase();
  const operationPending = String(state.operationPending || "").toLowerCase();
  if (!generation) {
    return { accepted: true, generation, stale: false };
  }
  if (kind === "started" && operationPending === "stop") {
    return { accepted: false, generation, stale: true };
  }
  if (
    currentGeneration > 0 &&
    generation !== currentGeneration &&
    kind !== "started"
  ) {
    return { accepted: false, generation, stale: true };
  }
  if (currentGeneration > 0 && generation < currentGeneration) {
    return { accepted: false, generation, stale: true };
  }
  if (
    currentGeneration === 0 &&
    stoppedGeneration > 0 &&
    generation <= stoppedGeneration &&
    operationPending !== "start"
  ) {
    return { accepted: false, generation, stale: true };
  }
  return { accepted: true, generation, stale: false };
}

export function monitoringEventTransition(payload = {}, state = {}) {
  const snapshot = payload?.snapshot || payload || {};
  const kind = String(payload?.kind || "").toLowerCase();
  const snapshotRunning =
    typeof snapshot.running === "boolean" ? snapshot.running : undefined;
  const tickHitCount = countFromEitherCase(
    payload,
    "tickHitCount",
    "tick_hit_count",
  );
  const tickError = payload?.tickError ?? payload?.tick_error;
  const tickHasHits = kind === "tick" && tickHitCount > 0;
  const stopped = kind === "stopped" || snapshotRunning === false;
  const eventIsRunning =
    kind === "started" || kind === "tick" || snapshotRunning === true;
  const nextMonitoringActive = stopped
    ? false
    : eventIsRunning
      ? true
      : Boolean(state.monitoringActive);
  return {
    nextMonitoringActive,
    nextProfileMonitoringActive: stopped
      ? false
      : Boolean(state.profileMonitoringActive),
    shouldRefreshProfile: Boolean(state.profileMonitoringActive && tickHasHits),
    snapshot,
    statusText: monitoringStatusText(snapshot, {
      tickError,
      tickHitCount: tickHasHits ? tickHitCount : 0,
    }),
  };
}

function numberOr(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function intOr(value, fallback) {
  const number = numberOr(value, fallback);
  return Math.trunc(number);
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function scaledMinimums(minimums, total) {
  const safeMinimums = minimums.map((value) => Math.max(1, intOr(value, 1)));
  const safeTotal = Math.max(1, intOr(total, 1));
  const minTotal = safeMinimums.reduce((sum, value) => sum + value, 0);
  if (minTotal <= safeTotal) {
    return safeMinimums;
  }
  const scale = safeTotal / minTotal;
  return safeMinimums.map((value) => Math.max(1, Math.floor(value * scale)));
}

function fitThreePaneWidths(first, second, third, total, minimums) {
  const safeTotal = Math.max(1, intOr(total, 1));
  const [minFirst, minSecond, minThird] = scaledMinimums(minimums, safeTotal);
  let nextFirst = numberOr(first, minFirst);
  let nextSecond = numberOr(second, minSecond);
  let nextThird = numberOr(third, safeTotal - nextFirst - nextSecond);
  const sum = nextFirst + nextSecond + nextThird;

  if (Number.isFinite(sum) && sum > 0 && Math.abs(sum - safeTotal) > 0.5) {
    const scale = safeTotal / sum;
    nextFirst *= scale;
    nextSecond *= scale;
    nextThird *= scale;
  }

  nextFirst = clamp(nextFirst, minFirst, safeTotal - minSecond - minThird);
  nextSecond = clamp(nextSecond, minSecond, safeTotal - nextFirst - minThird);
  nextThird = safeTotal - nextFirst - nextSecond;

  if (nextThird < minThird) {
    let deficit = minThird - nextThird;
    const secondReduction = Math.min(deficit, nextSecond - minSecond);
    nextSecond -= secondReduction;
    deficit -= secondReduction;
    nextFirst -= Math.min(deficit, nextFirst - minFirst);
    nextThird = safeTotal - nextFirst - nextSecond;
  }

  return {
    first: Math.round(nextFirst),
    second: Math.round(nextSecond),
    third: Math.max(1, Math.round(nextThird)),
  };
}

export function resizeThreePaneLayout(layout = {}, drag = {}, options = {}) {
  const total = Math.max(1, intOr(options.total, 1));
  const minimums = [
    options.minFirst ?? 330,
    options.minSecond ?? 270,
    options.minThird ?? 270,
  ];
  const defaultSecond = numberOr(options.defaultSecond, 340);
  const defaultFirst = numberOr(
    options.defaultFirst,
    Math.max(minimums[0], Math.round((total - defaultSecond) * 0.58)),
  );
  let first = numberOr(layout.first ?? layout.left, defaultFirst);
  let second = numberOr(layout.second ?? layout.control, defaultSecond);
  let third = numberOr(
    layout.third ?? layout.preview,
    total - first - second,
  );
  const delta = numberOr(drag.delta, 0);
  const splitter = String(drag.splitter || "");

  if (splitter === "first-second" || splitter === "targets-controls") {
    first += delta;
    second -= delta;
  } else if (splitter === "second-third" || splitter === "controls-preview") {
    second += delta;
    third -= delta;
  }

  return fitThreePaneWidths(first, second, third, total, minimums);
}

export function resizeStackedPaneLayout(layout = {}, drag = {}, options = {}) {
  const total = Math.max(1, intOr(options.total, 1));
  const [minFirst, minSecond] = scaledMinimums(
    [options.minFirst ?? 120, options.minSecond ?? 88],
    total,
  );
  const defaultFirst = numberOr(options.defaultFirst, Math.round(total * 0.68));
  let first = numberOr(layout.first ?? layout.top, defaultFirst);
  let second = numberOr(layout.second ?? layout.bottom, total - first);
  const sum = first + second;

  if (Number.isFinite(sum) && sum > 0 && Math.abs(sum - total) > 0.5) {
    const scale = total / sum;
    first *= scale;
    second *= scale;
  }

  if (String(drag.splitter || "") === "first-second" || drag.delta) {
    const delta = numberOr(drag.delta, 0);
    first += delta;
    second -= delta;
  }

  first = clamp(first, minFirst, total - minSecond);
  second = total - first;
  return {
    first: Math.round(first),
    second: Math.max(1, Math.round(second)),
  };
}

export function resizeMultiPaneLayout(layout = [], drag = {}, options = {}) {
  const count = Math.max(
    2,
    intOr(
      options.count ??
        Math.max(
          Array.isArray(layout) ? layout.length : 0,
          Array.isArray(options.defaults) ? options.defaults.length : 0,
          Array.isArray(options.minimums) ? options.minimums.length : 0,
        ),
      2,
    ),
  );
  const total = Math.max(1, intOr(options.total, 1));
  const minimums = scaledMinimums(
    Array.from({ length: count }, (_item, index) =>
      intOr(options.minimums?.[index], 52),
    ),
    total,
  );
  const defaultBase = Math.max(
    1,
    total - minimums.reduce((sum, value) => sum + value, 0),
  );
  const defaults = Array.from({ length: count }, (_item, index) =>
    numberOr(options.defaults?.[index], minimums[index] + defaultBase / count),
  );
  let values = Array.from({ length: count }, (_item, index) =>
    numberOr(layout?.[index], defaults[index]),
  );
  const sum = values.reduce((acc, value) => acc + value, 0);

  if (Number.isFinite(sum) && sum > 0 && Math.abs(sum - total) > 0.5) {
    const scale = total / sum;
    values = values.map((value) => value * scale);
  }

  const index = Math.trunc(Number(drag.index ?? drag.paneIndex ?? -1));
  const delta = numberOr(drag.delta, 0);
  if (index >= 0 && index < count - 1 && delta !== 0) {
    const first = values[index];
    const second = values[index + 1];
    const maxPositive = Math.max(0, second - minimums[index + 1]);
    const maxNegative = -Math.max(0, first - minimums[index]);
    const boundedDelta = clamp(delta, maxNegative, maxPositive);
    values[index] = first + boundedDelta;
    values[index + 1] = second - boundedDelta;
  }

  values = values.map((value, index) => Math.max(minimums[index], value));
  const adjustedSum = values.reduce((acc, value) => acc + value, 0);
  if (adjustedSum > total) {
    let overflow = adjustedSum - total;
    for (let index = values.length - 1; index >= 0 && overflow > 0; index -= 1) {
      const reduction = Math.min(overflow, values[index] - minimums[index]);
      values[index] -= reduction;
      overflow -= reduction;
    }
  } else if (adjustedSum < total) {
    values[values.length - 1] += total - adjustedSum;
  }

  return values.map((value) => Math.max(1, Math.round(value)));
}

export function isHiddenWindowState(state) {
  return ["hidden", "iconic", "minimized", "withdrawn"].includes(
    String(state || "").toLowerCase(),
  );
}

export function currentWindowGeometry(state, options = {}) {
  const lastGeometry = options.lastGeometry || "980x680";
  const geometry = String(options.geometry || "");
  if (isHiddenWindowState(state)) {
    return lastGeometry;
  }
  return /^\d+x\d+[+-]\d+[+-]\d+$/.test(geometry) ? geometry : lastGeometry;
}

export function rememberWindowGeometry(state, metrics = {}, options = {}) {
  const lastGeometry = options.lastGeometry || "980x680";
  if (isHiddenWindowState(state)) {
    return lastGeometry;
  }
  const width = Math.max(1, intOr(metrics.width, 1));
  const height = Math.max(1, intOr(metrics.height, 1));
  const x = intOr(metrics.x, 0);
  const y = intOr(metrics.y, 0);
  return `${width}x${height}+${x}+${y}`;
}

export function captureLayoutRatios(metrics = {}) {
  const rootWidth = Math.max(1, numberOr(metrics.rootWidth, 1));
  const leftPaneHeight = Math.max(1, numberOr(metrics.leftPaneHeight, 1));
  const firstSashX = numberOr(metrics.firstSashX, 0);
  const secondSashX = numberOr(metrics.secondSashX, firstSashX);
  const leftSashY = numberOr(metrics.leftSashY, 0);
  return {
    leftRatio: clamp(leftSashY / leftPaneHeight, 0.25, 0.8),
    mainRatio: clamp(firstSashX / rootWidth, 0.25, 0.85),
    rightRatio: clamp((secondSashX - firstSashX) / rootWidth, 0.12, 0.4),
  };
}

export function sidePaneWidth(width, rightRatio = 0.25) {
  const safeWidth = Math.max(1, intOr(width, 1));
  const ratio = clamp(numberOr(rightRatio, 0.25), 0.12, 0.4);
  const maxSide = Math.max(180, Math.floor((safeWidth - 360) / 2));
  const minSide = Math.min(320, maxSide);
  let preferred = Math.trunc(safeWidth * ratio);
  if (ratio <= 0.18) {
    preferred = Math.min(360, preferred);
  }
  return Math.max(minSide, Math.min(maxSide, preferred));
}

export function horizontalSashes(width, ratios = {}) {
  const safeWidth = Math.max(1, intOr(width, 1));
  const leftMin = 360;
  const middleMin = 260;
  const previewMin = 320;
  if (safeWidth < leftMin + middleMin + previewMin) {
    const middle = sidePaneWidth(safeWidth, ratios.rightRatio);
    const first = Math.max(leftMin, safeWidth - middle * 2);
    return [first, first + middle];
  }
  let first = Math.round(
    safeWidth * clamp(numberOr(ratios.mainRatio, 0.5), 0.25, 0.85),
  );
  let second = Math.round(
    safeWidth *
      clamp(
        numberOr(ratios.mainRatio, 0.5) + numberOr(ratios.rightRatio, 0.25),
        0.37,
        0.97,
      ),
  );
  first = Math.min(Math.max(leftMin, first), safeWidth - middleMin - previewMin);
  second = Math.min(Math.max(first + middleMin, second), safeWidth - previewMin);
  return [first, second];
}

export function restoreLayoutPlan(metrics = {}, ratios = {}, options = {}) {
  const horizontal = options.horizontal !== false;
  const vertical = options.vertical !== false;
  const width = Math.max(1, intOr(metrics.width, 1));
  const leftPaneHeight = Math.max(1, intOr(metrics.leftPaneHeight, 1));
  if (String(metrics.windowState || "").toLowerCase() === "withdrawn") {
    return {
      horizontalSashes: null,
      retry: true,
      retryHorizontal: horizontal,
      retryVertical: vertical,
      verticalSashY: null,
    };
  }

  const retryHorizontal = horizontal && width < 400;
  const retryVertical = vertical && leftPaneHeight < 100;
  return {
    horizontalSashes:
      horizontal && !retryHorizontal ? horizontalSashes(width, ratios) : null,
    retry: retryHorizontal || retryVertical,
    retryHorizontal,
    retryVertical,
    verticalSashY:
      vertical && !retryVertical
        ? Math.trunc(leftPaneHeight * numberOr(ratios.leftRatio, 0.5))
        : null,
  };
}

export function layoutBusy(state = {}) {
  const now =
    typeof state.now === "function" ? Number(state.now()) : numberOr(state.now, Date.now());
  const activeDeadline = (value) => {
    const number = Number(value);
    return Number.isFinite(number) || number === Number.POSITIVE_INFINITY ? number : 0;
  };
  const activeUntil = Math.max(
    activeDeadline(state.resizeActiveUntil),
    activeDeadline(state.layoutActiveUntil),
    activeDeadline(state.moveActiveUntil),
  );
  return Boolean(state.extraBusy || state.mouseButtonDown || now < activeUntil);
}

export function windowResizeTransition(event = {}, state = {}) {
  if (event.widgetMatches === false) {
    return {
      cancelResizeJob: false,
      heightChanged: false,
      kind: "ignored",
      rememberGeometry: false,
      scheduleScale: false,
      suspendPreviews: false,
      widthChanged: false,
    };
  }
  if (isHiddenWindowState(event.windowState)) {
    return {
      cancelResizeJob: Boolean(state.resizeJob),
      heightChanged: false,
      kind: "hidden",
      rememberGeometry: false,
      resetResizeActive: true,
      scheduleScale: false,
      suspendPreviews: false,
      widthChanged: false,
    };
  }

  const width = Math.max(1, intOr(event.width, 1));
  const height = Math.max(1, intOr(event.height, 1));
  const previous = Array.isArray(state.lastRootSize) ? state.lastRootSize : null;
  const widthChanged = previous ? width !== Number(previous[0]) : true;
  const heightChanged = previous ? height !== Number(previous[1]) : true;
  if (!widthChanged && !heightChanged) {
    return {
      cancelResizeJob: false,
      heightChanged: false,
      kind: "move",
      rememberGeometry: true,
      scheduleScale: false,
      suspendPreviews: false,
      widthChanged: false,
    };
  }
  return {
    cancelResizeJob: Boolean(state.resizeJob),
    heightChanged,
    kind: "resize",
    rememberGeometry: true,
    scheduleScale: true,
    size: [width, height],
    suspendPreviews: true,
    widthChanged,
  };
}

export function checkIndicatorMetrics(scale = 1) {
  const numericScale = Number(scale);
  const safeScale = Number.isFinite(numericScale) && numericScale > 0 ? numericScale : 1;
  return {
    size: Math.max(12, Math.trunc(13 * safeScale)),
    strokeWidth: Math.max(2, Math.trunc(2 * safeScale)),
  };
}

export function resolveCheckIndicatorScale(host, options = {}) {
  const explicitScale =
    typeof options.scale === "function" ? options.scale(host) : options.scale;
  if (explicitScale !== undefined) {
    const scale = Number(explicitScale);
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
  }

  const computedStyle =
    options.computedStyle ||
    ((node) =>
      typeof window !== "undefined" && typeof window.getComputedStyle === "function"
        ? window.getComputedStyle(node)
        : null);
  const style = computedStyle(host);
  const variableScale = Number.parseFloat(
    style?.getPropertyValue?.("--screen-watch-ui-scale") || "",
  );
  if (Number.isFinite(variableScale) && variableScale > 0) {
    return variableScale;
  }

  const baseFontPx = Number(options.baseFontPx || 14);
  const fontPx = Number.parseFloat(style?.fontSize || "");
  if (Number.isFinite(fontPx) && fontPx > 0 && baseFontPx > 0) {
    return fontPx / baseFontPx;
  }
  return 1;
}

export function applyCheckIndicatorScale(control, options = {}) {
  const input =
    control?.matches?.("input[type='checkbox']")
      ? control
      : control?.querySelector?.("input[type='checkbox']");
  if (!input) {
    return null;
  }
  const host = input.closest?.(".check-control") || control;
  const metrics = checkIndicatorMetrics(resolveCheckIndicatorScale(host, options));
  const styleTargets = [host, input].filter((item) => item?.style?.setProperty);

  for (const target of styleTargets) {
    target.style.setProperty("--check-size", `${metrics.size}px`);
    target.style.setProperty("--check-mark-stroke", `${metrics.strokeWidth}px`);
  }
  host?.classList?.add?.("check-control");
  input.classList?.add?.("is-custom-check");
  return metrics;
}

export function installCustomCheckIndicators(root, options = {}) {
  const controls = root?.querySelectorAll?.(
    ".check-control, input[type='checkbox'].is-custom-check",
  );
  let applied = 0;
  for (const control of controls || []) {
    if (applyCheckIndicatorScale(control, options)) {
      applied += 1;
    }
  }
  return applied;
}

export function recordRepeatClick(state, index, options = {}) {
  const now =
    typeof options.now === "function" ? Number(options.now()) : Date.now();
  const thresholdMs = Number(options.thresholdMs ?? 500);
  const safeThreshold =
    Number.isFinite(thresholdMs) && thresholdMs >= 0 ? thresholdMs : 500;
  const previousIndex = state?.index ?? null;
  const previousTime = Number(state?.time ?? Number.NEGATIVE_INFINITY);
  const elapsed = now - previousTime;
  const repeated =
    String(previousIndex) === String(index) &&
    Number.isFinite(elapsed) &&
    elapsed >= 0 &&
    elapsed <= safeThreshold;

  if (state) {
    state.index = index;
    state.time = now;
  }
  return {
    elapsed,
    index,
    previousIndex,
    repeated,
  };
}

export function selectIndexedListItem(container, index, options = {}) {
  const selectedClass = options.selectedClass || "is-selected";
  const items = Array.from(container?.children || []).filter(
    (item) => item.dataset?.index !== undefined,
  );
  let previousIndex = null;
  let touched = 0;
  const targetIndex = String(index);

  for (const item of items) {
    const isSelected = item.classList.contains(selectedClass);
    const shouldSelect = item.dataset.index === targetIndex;
    if (isSelected) {
      previousIndex = Number(item.dataset.index);
    }
    if (isSelected && !shouldSelect) {
      item.classList.remove(selectedClass);
      touched += 1;
    } else if (!isSelected && shouldSelect) {
      item.classList.add(selectedClass);
      touched += 1;
    }
  }

  return {
    changed: touched > 0,
    previousIndex,
    selectedIndex: items.some((item) => item.dataset.index === targetIndex)
      ? Number(index)
      : null,
    touched,
  };
}

export function targetSelectionIndexFromEditResult(result = {}, fallbackIndex = null) {
  const raw = result?.selectedIndex ?? result?.selected_index;
  const hasSelectionField =
    Object.prototype.hasOwnProperty.call(result || {}, "selectedIndex") ||
    Object.prototype.hasOwnProperty.call(result || {}, "selected_index");
  if (!hasSelectionField) {
    return fallbackIndex;
  }
  if (raw === null || raw === undefined) {
    return null;
  }
  const index = Math.trunc(Number(raw));
  const targets = Array.isArray(result?.targets) ? result.targets : null;
  if (!Number.isFinite(index) || index < 0) {
    return null;
  }
  if (targets && index >= targets.length) {
    return null;
  }
  return index;
}

export function targetSelectionIndexForProfileLoad(
  profile = {},
  currentIndex = null,
  options = {},
) {
  const targets = Array.isArray(profile?.targets) ? profile.targets : [];
  if (!targets.length) {
    return null;
  }
  if (options.selectFirst) {
    return 0;
  }
  const index = Math.trunc(Number(currentIndex));
  if (!Number.isFinite(index) || index < 0 || index >= targets.length) {
    return null;
  }
  return index;
}

export function targetEnabled(target) {
  return target?.enabled !== false;
}

export function targetHitCount(target) {
  const value = Number(target?.hit_count ?? target?.hitCount ?? 0);
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;
}

export function targetMenuState(target) {
  const hitCount = targetHitCount(target);
  const hasPath = Boolean(target?.path);
  const hasId = Boolean(target?.id);
  return {
    canClearHits: hasId && hitCount > 0,
    canOpen: hasPath,
    clearHitsDisabled: !(hasId && hitCount > 0),
    hitCount,
    openDisabled: !hasPath,
  };
}

export function targetActionState(target, index, totalCount) {
  const safeIndex = Math.max(0, Math.trunc(Number(index) || 0));
  const safeTotal = Math.max(0, Math.trunc(Number(totalCount) || 0));
  const menu = targetMenuState(target);
  return {
    canClearHits: menu.canClearHits,
    canOpen: menu.canOpen,
    clearHitsDisabled: menu.clearHitsDisabled,
    deleteDisabled: false,
    hitCount: menu.hitCount,
    moveDownDisabled: safeTotal <= 0 || safeIndex >= safeTotal - 1,
    moveDownInsertIndex: Math.min(safeTotal, safeIndex + 2),
    moveUpDisabled: safeIndex <= 0,
    moveUpInsertIndex: Math.max(0, safeIndex - 1),
    openDisabled: menu.openDisabled,
    targetId: target?.id || "",
  };
}

export function targetDropAfter(item, clientY) {
  const rect = item?.getBoundingClientRect?.();
  const top = Number(rect?.top ?? 0);
  const height = Number(rect?.height ?? 0);
  const y = Number(clientY);
  if (!Number.isFinite(y)) {
    return false;
  }
  return y > top + Math.max(0, height) / 2;
}

export function targetDropInsertIndex(index, dropAfter) {
  return Math.max(0, Math.trunc(Number(index) || 0) + (dropAfter ? 1 : 0));
}

export function fitFixedMenuInViewport(rect = {}, viewport = {}, options = {}) {
  const margin = Math.max(0, Number(options.margin ?? 8));
  const viewportWidth = Math.max(0, Number(viewport.width ?? 0));
  const viewportHeight = Math.max(0, Number(viewport.height ?? 0));
  const left = Number(rect.left ?? 0);
  const top = Number(rect.top ?? 0);
  const width = Math.max(0, Number(rect.width ?? 0));
  const height = Math.max(0, Number(rect.height ?? 0));
  const right = Number(rect.right ?? left + width);
  const bottom = Number(rect.bottom ?? top + height);
  const safeLeft = Number.isFinite(left) ? left : margin;
  const safeTop = Number.isFinite(top) ? top : margin;

  let nextLeft = safeLeft;
  let nextTop = safeTop;
  if (Number.isFinite(right) && right > viewportWidth) {
    nextLeft = Math.max(margin, viewportWidth - width - margin);
  }
  if (Number.isFinite(bottom) && bottom > viewportHeight) {
    nextTop = Math.max(margin, viewportHeight - height - margin);
  }
  return {
    adjustedLeft: nextLeft !== safeLeft,
    adjustedTop: nextTop !== safeTop,
    left: nextLeft,
    top: nextTop,
  };
}
