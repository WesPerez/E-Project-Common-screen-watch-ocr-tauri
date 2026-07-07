import test from "node:test";
import assert from "node:assert/strict";
import {
  applyCheckIndicatorScale,
  buildProfileSourceOptions,
  buildRememberedWindowAppConfigs,
  buildSelectedPreviewSources,
  buildSelectedRegionConfigs,
  buildSelectedWindowAppConfigs,
  buildSelectedWindowConfigs,
  checkIndicatorMetrics,
  clearRestorePreviewFrames,
  captureLayoutRatios,
  coverRestorePreviewFrames,
  currentWindowGeometry,
  evidenceDirectoryLogText,
  evidenceDirectoryStatusText,
  horizontalSashes,
  installAutohideScrollbar,
  installEntryCursorEndHandlers,
  installCustomCheckIndicators,
  isHiddenWindowState,
  layoutBusy,
  missingSourceSummary,
  monitorErrorSummary,
  monitoringEventFreshness,
  monitoringEventTransition,
  monitoringHeartbeatLogText,
  monitoringProgressLogText,
  monitoringSessionGeneration,
  monitoringStatusText,
  previewStatusText,
  profileImportRequest,
  profileImportStatusText,
  profileTargetsEnabledStatusText,
  profileToggleAllLabel,
  profileWorkflowActionState,
  profileSourceOptionsHaveSources,
  recordRepeatClick,
  rememberWindowGeometry,
  resizeMultiPaneLayout,
  resizeStackedPaneLayout,
  resizeThreePaneLayout,
  restoreLayoutPlan,
  scrollbarOverflowState,
  selectIndexedListItem,
  setRestoreOverlayCovered,
  sidePaneWidth,
  scrollElementByWheel,
  scanStatusText,
  selectedPhysicalMonitors,
  selectedWindowRecords,
  shouldHandleProfilePaste,
  fitFixedMenuInViewport,
  sourcePreviewCardPresentation,
  sourcePreviewCaptureFailureState,
  sourcePreviewRefreshGate,
  targetActionState,
  targetDropAfter,
  targetDropInsertIndex,
  targetEnabled,
  targetHitCount,
  targetMenuState,
  targetSelectionIndexFromEditResult,
  targetSelectionIndexForProfileLoad,
  updateAutohideScrollbar,
  visiblePreviewRect,
  windowResolutionStatusText,
  windowResizeTransition,
} from "./ui-behavior.js";

function fakeInput(value) {
  const listeners = new Map();
  return {
    value,
    selection: null,
    addEventListener(name, handler) {
      listeners.set(name, handler);
    },
    dispatch(name) {
      listeners.get(name)?.();
    },
    setSelectionRange(start, end) {
      this.selection = { start, end };
    },
  };
}

function fakeCard(index, classes = []) {
  const values = new Set(classes);
  return {
    dataset: { index: String(index) },
    classList: {
      add(name) {
        values.add(name);
      },
      remove(name) {
        values.delete(name);
      },
      contains(name) {
        return values.has(name);
      },
    },
  };
}

function fakeClassList(classes = []) {
  const values = new Set(classes);
  return {
    add(name) {
      values.add(name);
    },
    remove(name) {
      values.delete(name);
    },
    contains(name) {
      return values.has(name);
    },
  };
}

function fakeStyle() {
  const values = new Map();
  return {
    setProperty(name, value) {
      values.set(name, value);
    },
    getPropertyValue(name) {
      return values.get(name) || "";
    },
  };
}

function fakeCheckboxHost(classes = ["check-control"]) {
  const host = {
    style: fakeStyle(),
    classList: fakeClassList(classes),
    querySelector(selector) {
      return selector === "input[type='checkbox']" ? input : null;
    },
  };
  const input = {
    style: fakeStyle(),
    classList: fakeClassList(),
    closest(selector) {
      return selector === ".check-control" && host.classList.contains("check-control")
        ? host
        : null;
    },
    matches(selector) {
      return selector === "input[type='checkbox']";
    },
  };
  return { host, input };
}

function fakeScrollable(scrollHeight, clientHeight) {
  return {
    clientHeight,
    dataset: {},
    scrollHeight,
    style: {},
    classList: fakeClassList(),
    addEventListener() {},
  };
}

function fakePreviewFrame() {
  return {
    classList: fakeClassList(),
    dataset: {},
    matches(selector) {
      return selector === ".source-preview-frame";
    },
    querySelectorAll() {
      return [];
    },
  };
}

function fakePreviewRoot(frames) {
  return {
    matches() {
      return false;
    },
    querySelectorAll(selector) {
      assert.equal(selector, ".source-preview-frame");
      return frames;
    },
  };
}

function fakeDropItem(top, height) {
  return {
    getBoundingClientRect() {
      return { height, top };
    },
  };
}

function fakeRectElement(rect) {
  return {
    getBoundingClientRect() {
      return rect;
    },
  };
}

test("entry cursor handlers keep single-line inputs at the end after clicks", () => {
  const input = fakeInput("0.8-1.2:5%");

  installEntryCursorEndHandlers([input], { schedule: (callback) => callback() });

  input.setSelectionRange(0, 0);
  input.dispatch("click");

  assert.deepEqual(input.selection, { start: 10, end: 10 });
});

test("entry cursor handlers tolerate inputs without text selection support", () => {
  const input = {
    value: "123",
    addEventListener(_name, handler) {
      this.handler = handler;
    },
  };

  installEntryCursorEndHandlers([input], { schedule: (callback) => callback() });

  assert.doesNotThrow(() => input.handler());
});

test("gallery wheel events scroll the target list and consume handled wheel input", () => {
  let prevented = false;
  const list = { scrollTop: 12 };

  const changed = scrollElementByWheel(list, {
    deltaY: 48,
    preventDefault() {
      prevented = true;
    },
  });

  assert.equal(changed, true);
  assert.equal(list.scrollTop, 60);
  assert.equal(prevented, true);
});

test("gallery wheel events are ignored when there is no scroll delta", () => {
  let prevented = false;
  const list = { scrollTop: 12 };

  const changed = scrollElementByWheel(list, {
    deltaY: 0,
    preventDefault() {
      prevented = true;
    },
  });

  assert.equal(changed, false);
  assert.equal(list.scrollTop, 12);
  assert.equal(prevented, false);
});

test("autohide scrollbar state hides when content fits", () => {
  const list = fakeScrollable(80, 100);

  const state = updateAutohideScrollbar(list);

  assert.deepEqual(state, {
    axis: "vertical",
    clientSize: 100,
    hasOverflow: false,
    scrollSize: 80,
  });
  assert.equal(list.classList.contains("is-scrollbar-hidden"), true);
  assert.equal(list.classList.contains("has-scrollbar"), false);
  assert.equal(list.dataset.scrollbarState, "hidden");
  assert.equal(list.style.overflowY, "hidden");
});

test("autohide scrollbar state shows when content overflows", () => {
  const list = fakeScrollable(220, 100);

  const state = updateAutohideScrollbar(list);

  assert.equal(state.hasOverflow, true);
  assert.equal(list.classList.contains("has-scrollbar"), true);
  assert.equal(list.classList.contains("is-scrollbar-hidden"), false);
  assert.equal(list.dataset.scrollbarState, "visible");
  assert.equal(list.style.overflowY, "auto");
});

test("autohide scrollbar installer updates initially and reacts to resize", () => {
  let resizeHandler = null;
  const list = fakeScrollable(80, 100);
  const windowTarget = {
    addEventListener(name, handler) {
      if (name === "resize") {
        resizeHandler = handler;
      }
    },
    removeEventListener() {},
  };

  const controller = installAutohideScrollbar(list, { window: windowTarget });
  assert.equal(list.classList.contains("autohide-scrollbar"), true);
  assert.equal(list.dataset.scrollbarState, "hidden");

  list.scrollHeight = 160;
  resizeHandler();

  assert.equal(list.dataset.scrollbarState, "visible");
  assert.equal(scrollbarOverflowState(list).hasOverflow, true);
  assert.equal(typeof controller.update, "function");
  assert.doesNotThrow(() => controller.disconnect());
});

test("restore overlay helpers cover and clear a single preview frame", () => {
  const frame = fakePreviewFrame();

  assert.equal(setRestoreOverlayCovered(frame, true), true);
  assert.equal(frame.classList.contains("is-restore-covered"), true);
  assert.equal(frame.dataset.restoreOverlay, "covered");

  assert.equal(setRestoreOverlayCovered(frame, false), true);
  assert.equal(frame.classList.contains("is-restore-covered"), false);
  assert.equal(frame.dataset.restoreOverlay, "clear");
});

test("restore overlay helpers cover every source preview frame in a root", () => {
  const frames = [fakePreviewFrame(), fakePreviewFrame()];
  const touched = coverRestorePreviewFrames(fakePreviewRoot(frames));

  assert.equal(touched, 2);
  assert.equal(frames[0].classList.contains("is-restore-covered"), true);
  assert.equal(frames[1].classList.contains("is-restore-covered"), true);
});

test("restore overlay helpers can clear a frame root directly", () => {
  const frame = fakePreviewFrame();
  coverRestorePreviewFrames(frame);

  const touched = clearRestorePreviewFrames(frame);

  assert.equal(touched, 1);
  assert.equal(frame.classList.contains("is-restore-covered"), false);
  assert.equal(frame.dataset.restoreOverlay, "clear");
});

test("missing source summary reports direct and remembered windows", () => {
  const summary = missingSourceSummary({
    skippedWindows: 1,
    skippedWindowApps: 2,
  });

  assert.deepEqual(summary, {
    skippedWindowApps: 2,
    skippedWindows: 1,
    text: "缺失 1 个窗口，缺失 2 个应用窗口",
    total: 3,
  });
});

test("missing source summary accepts snake case and ignores invalid counts", () => {
  const summary = missingSourceSummary({
    skipped_windows: "3",
    skipped_window_apps: -2,
  });

  assert.equal(summary.skippedWindows, 3);
  assert.equal(summary.skippedWindowApps, 0);
  assert.equal(summary.text, "缺失 3 个窗口");
});

test("scan status exposes skipped sources alongside hit count", () => {
  const status = scanStatusText({
    hitCount: 4,
    skippedWindows: 1,
    skippedWindowApps: 1,
  });

  assert.equal(status, "Ready - 4 hits，缺失 1 个窗口，缺失 1 个应用窗口");
});

test("window resolution status exposes missing remembered windows", () => {
  const status = windowResolutionStatusText({
    windows: [{ title: "Demo" }],
    missingWindowApps: [{ title: "Missing", ordinal: 1 }],
  });

  assert.equal(status, "Ready - 1 windows，缺失 1 个应用窗口");
});

test("window resolution status accepts snake case missing windows", () => {
  const status = windowResolutionStatusText({
    windows: [],
    missing_window_apps: [{ title: "Missing", ordinal: 1 }],
  });

  assert.equal(status, "Ready - 0 windows，缺失 1 个应用窗口");
});

test("preview status exposes failed preview count", () => {
  assert.equal(previewStatusText(2, 3), "Ready - 2/3 previews，失败 1 个");
});

test("preview status clamps invalid counts", () => {
  assert.equal(previewStatusText(Number.NaN, -4), "Ready - 0/0 previews");
  assert.equal(previewStatusText(5, 2), "Ready - 2/2 previews");
});

test("source preview scheduled refresh stops cleanly when previews are disabled", () => {
  assert.deepEqual(
    sourcePreviewRefreshGate({
      enabled: false,
      scheduled: true,
    }),
    {
      canRefresh: false,
      kind: "disabled",
      retryDelay: null,
      statusText: "",
    },
  );
});

test("source preview manual refresh can run even before the timer is enabled", () => {
  assert.deepEqual(
    sourcePreviewRefreshGate({
      enabled: false,
      scheduled: false,
    }),
    {
      canRefresh: true,
      kind: "ready",
      retryDelay: null,
      statusText: "",
    },
  );
});

test("source preview refresh retries while another refresh is active", () => {
  assert.deepEqual(
    sourcePreviewRefreshGate(
      {
        enabled: true,
        refreshing: true,
        scheduled: false,
      },
      { retryDelay: 125 },
    ),
    {
      canRefresh: false,
      kind: "refreshing",
      retryDelay: 125,
      statusText: "来源预览正在刷新",
    },
  );
});

test("source preview refresh retries silently while layout is busy on timer ticks", () => {
  assert.deepEqual(
    sourcePreviewRefreshGate(
      {
        enabled: true,
        layoutBusy: true,
        scheduled: true,
      },
      { retryDelay: 125 },
    ),
    {
      canRefresh: false,
      kind: "layout-busy",
      retryDelay: 125,
      statusText: "",
    },
  );
});

test("source preview DWM handoff keeps a card healthy when bitmap capture fails", () => {
  assert.deepEqual(sourcePreviewCaptureFailureState(true, "bitmap failed"), {
    clearImage: false,
    isError: false,
    message: "",
    ok: true,
  });
  assert.deepEqual(sourcePreviewCaptureFailureState(false, "bitmap failed"), {
    clearImage: true,
    isError: true,
    message: "bitmap failed",
    ok: false,
  });
});

test("source preview card presentation applies successful bitmap previews", () => {
  assert.deepEqual(
    sourcePreviewCardPresentation({
      preview: { dataUrl: "data:image/png;base64,ok" },
    }),
    {
      clearImage: false,
      imageSrc: "data:image/png;base64,ok",
      isError: false,
      message: "",
      ok: true,
    },
  );
});

test("source preview card presentation keeps DWM handoff healthy on bitmap failure", () => {
  assert.deepEqual(
    sourcePreviewCardPresentation({
      dwmActive: true,
      error: "bitmap failed",
    }),
    {
      clearImage: false,
      isError: false,
      message: "",
      ok: true,
    },
  );
});

test("source preview card presentation clears stale bitmap when no DWM fallback exists", () => {
  assert.deepEqual(
    sourcePreviewCardPresentation({
      dwmActive: false,
      error: new Error("capture failed"),
    }),
    {
      clearImage: true,
      isError: true,
      message: "Error: capture failed",
      ok: false,
    },
  );
});

test("visible preview rect clips partially offscreen WebView frames", () => {
  const rect = visiblePreviewRect(
    fakeRectElement({ bottom: 140, left: -20, right: 130, top: 10 }),
    { height: 120, width: 100 },
  );

  assert.deepEqual(rect, {
    height: 110,
    left: 0,
    top: 10,
    width: 100,
  });
});

test("visible preview rect skips DWM sync for hidden or tiny frames", () => {
  assert.equal(
    visiblePreviewRect(
      fakeRectElement({ bottom: 40, left: 60, right: 140, top: 10 }),
      { height: 120, width: 50 },
    ),
    null,
  );
  assert.equal(
    visiblePreviewRect(
      fakeRectElement({ bottom: 12, left: 10, right: 11, top: 10 }),
      { height: 120, width: 100 },
    ),
    null,
  );
});

test("visible preview rect requires a valid viewport before DWM sync", () => {
  assert.equal(
    visiblePreviewRect(
      fakeRectElement({ bottom: 80, left: 10, right: 90, top: 10 }),
      { height: 0, width: 100 },
    ),
    null,
  );
  assert.equal(visiblePreviewRect(null, { height: 120, width: 100 }), null);
});

test("selected source helpers exclude virtual monitors and preserve selected handles", () => {
  const monitors = [
    { index: 0, isVirtual: true },
    { index: 1, isVirtual: false },
    { index: 2, isVirtual: false },
  ];
  const windows = [
    { hwnd: 101, title: "One" },
    { hwnd: 202, title: "Two" },
  ];

  assert.deepEqual(
    selectedPhysicalMonitors(monitors, new Set([0, 2])).map(
      (monitor) => monitor.index,
    ),
    [2],
  );
  assert.deepEqual(
    selectedWindowRecords(windows, new Set(["202"])).map(
      (windowRecord) => windowRecord.hwnd,
    ),
    [202],
  );
});

test("selected preview sources apply region offsets but keep monitor fallback size", () => {
  const sources = buildSelectedPreviewSources({
    monitors: [
      {
        height: 1080,
        index: 1,
        isVirtual: false,
        left: 100,
        name: "Primary",
        top: 20,
        width: 1920,
      },
    ],
    region: { height: "", left: 12.4, top: 8.6, width: 320 },
    selectedMonitorIndexes: new Set([1]),
    selectedWindowHandles: new Set(["9001"]),
    windows: [
      {
        display: "Editor #2",
        hwnd: 9001,
        ordinal: 2,
        title: "Editor",
      },
    ],
  });

  assert.deepEqual(sources, [
    {
      height: 1080,
      key: "screen:monitor-1",
      kind: "screen",
      left: 112,
      name: "Primary",
      top: 29,
      width: 320,
    },
    {
      hwnd: 9001,
      key: "app:9001",
      kind: "window",
      name: "Editor #2",
    },
  ]);
});

test("profile source options preserve Python-compatible concrete window shape", () => {
  const state = {
    monitors: [
      {
        height: 1080,
        index: 2,
        isVirtual: false,
        left: 1920,
        top: 0,
        width: 1920,
      },
    ],
    region: { height: 240, left: 5, top: 6, width: 320 },
    selectedMonitorIndexes: new Set([2]),
    selectedWindowHandles: new Set(["44"]),
    windows: [
      {
        display: "Logs #3",
        hwnd: 44,
        ordinal: 3,
        title: "Logs",
      },
    ],
  };

  assert.deepEqual(buildSelectedRegionConfigs(state), [
    {
      height: 240,
      left: 5,
      monitor: 2,
      name: "monitor-2",
      top: 6,
      width: 320,
    },
  ]);
  assert.deepEqual(buildSelectedWindowConfigs(state), [
    {
      display: "Logs #3",
      hwnd: 44,
      name: "Logs #3",
      ordinal: 3,
      title: "Logs",
    },
  ]);
  assert.deepEqual(buildProfileSourceOptions(state), {
    profileRegion: {
      height: 240,
      left: 5,
      top: 6,
      width: 320,
    },
    regions: [
      {
        height: 240,
        left: 5,
        monitor: 2,
        name: "monitor-2",
        top: 6,
        width: 320,
      },
    ],
    windowApps: [],
    windows: [
      {
        display: "Logs #3",
        hwnd: 44,
        name: "Logs #3",
        ordinal: 3,
        title: "Logs",
      },
    ],
  });
});

test("profile source options can persist remembered app windows instead of hwnds", () => {
  const state = {
    region: { left: 11, top: 12, width: 320 },
    rememberedWindows: true,
    selectedWindowHandles: new Set(["44"]),
    windows: [
      {
        display: "Logs #3",
        hwnd: 44,
        ordinal: 3,
        title: "Logs",
      },
    ],
  };

  assert.deepEqual(buildSelectedWindowAppConfigs(state), [
    {
      ordinal: 3,
      title: "Logs",
    },
  ]);
  assert.deepEqual(buildProfileSourceOptions(state), {
    profileRegion: {
      left: 11,
      top: 12,
      width: 320,
    },
    regions: [],
    windowApps: [
      {
        ordinal: 3,
        title: "Logs",
      },
    ],
    windows: [],
  });
});

test("profile source options preserve remembered apps that are not currently listed", () => {
  const state = {
    region: { height: 180, left: -3, top: 8 },
    rememberedWindows: true,
    rememberedWindowApps: [
      {
        title: "Late App",
        ordinal: 2,
      },
      {
        title: "Logs",
        ordinal: 3,
      },
    ],
    selectedWindowHandles: new Set(["44"]),
    windows: [
      {
        display: "Logs #3",
        hwnd: 44,
        ordinal: 3,
        title: "Logs",
      },
    ],
  };

  assert.deepEqual(buildRememberedWindowAppConfigs(state), [
    {
      ordinal: 2,
      title: "Late App",
    },
    {
      ordinal: 3,
      title: "Logs",
    },
  ]);
  assert.deepEqual(buildProfileSourceOptions(state), {
    profileRegion: {
      height: 180,
      left: -3,
      top: 8,
    },
    regions: [],
    windowApps: [
      {
        ordinal: 2,
        title: "Late App",
      },
      {
        ordinal: 3,
        title: "Logs",
      },
    ],
    windows: [],
  });
});

test("profile source options keep region inputs even without selected monitors", () => {
  assert.deepEqual(
    buildProfileSourceOptions({
      monitors: [
        {
          height: 1080,
          index: 1,
          isVirtual: false,
          left: 0,
          top: 0,
          width: 1920,
        },
      ],
      region: { height: 200, left: 15, top: 20, width: 300 },
      selectedMonitorIndexes: new Set(),
      selectedWindowHandles: new Set(["77"]),
      windows: [
        {
          display: "Window",
          hwnd: 77,
          ordinal: 1,
          title: "Window",
        },
      ],
    }),
    {
      profileRegion: {
        height: 200,
        left: 15,
        top: 20,
        width: 300,
      },
      regions: [],
      windowApps: [],
      windows: [
        {
          display: "Window",
          hwnd: 77,
          name: "Window",
          ordinal: 1,
          title: "Window",
        },
      ],
    },
  );
});

test("profile source option presence accepts any source family", () => {
  assert.equal(profileSourceOptionsHaveSources({}), false);
  assert.equal(profileSourceOptionsHaveSources({ regions: [{}] }), true);
  assert.equal(profileSourceOptionsHaveSources({ windows: [{}] }), true);
  assert.equal(profileSourceOptionsHaveSources({ windowApps: [{}] }), true);
});

test("profile workflow action state blocks missing enabled targets before invokes", () => {
  const state = profileWorkflowActionState(
    {
      enabledCount: 0,
      targets: [{ enabled: true }],
    },
    {
      regions: [{}],
    },
  );

  assert.equal(state.canRun, false);
  assert.equal(state.enabledTargetCount, 0);
  assert.equal(state.hasSources, true);
  assert.equal(state.reason, "请先添加并启用至少一个模板");
});

test("profile workflow action state gates sources and duplicate monitoring starts", () => {
  const profile = {
    targets: [{ enabled: false }, { enabled: true }],
  };

  assert.deepEqual(
    profileWorkflowActionState(profile, {}, { action: "scan" }),
    {
      action: "scan",
      canRun: false,
      enabledTargetCount: 1,
      hasSources: false,
      monitoringActive: false,
      reason: "请至少选择一个物理屏幕或窗口",
      statusText: "请至少选择一个物理屏幕或窗口",
    },
  );

  assert.deepEqual(
    profileWorkflowActionState(
      profile,
      { windowApps: [{ title: "Logs", ordinal: 1 }] },
      { action: "start-monitoring", profileMonitoringActive: true },
    ),
    {
      action: "start-monitoring",
      canRun: false,
      enabledTargetCount: 1,
      hasSources: true,
      monitoringActive: true,
      reason: "Profile 监控已在运行",
      statusText: "Profile 监控已在运行",
    },
  );

  assert.equal(
    profileWorkflowActionState(
      profile,
      { windowApps: [{ title: "Logs", ordinal: 1 }] },
      { action: "start-monitoring" },
    ).canRun,
    true,
  );
});

test("profile capture target workflow allows empty profiles but still requires sources", () => {
  assert.deepEqual(
    profileWorkflowActionState(
      { enabledCount: 0, targets: [] },
      { regions: [{ monitor: 1 }] },
      { action: "capture-target" },
    ),
    {
      action: "capture-target",
      canRun: true,
      enabledTargetCount: 0,
      hasSources: true,
      monitoringActive: false,
      reason: "",
      statusText: "",
    },
  );

  assert.equal(
    profileWorkflowActionState(
      { enabledCount: 0, targets: [] },
      {},
      { action: "capture-target" },
    ).reason,
    "请至少选择一个物理屏幕或窗口",
  );
});

test("profile import request parses text paths and clamps template limit", () => {
  assert.deepEqual(
    profileImportRequest(
      `  ${String.raw`C:\images\one.png`}\n\n${String.raw`D:\two.png`}  \r\n`,
      "2.8",
    ),
    {
      hasPaths: true,
      imagePaths: [String.raw`C:\images\one.png`, String.raw`D:\two.png`],
      maxTemplates: 2,
    },
  );

  assert.deepEqual(profileImportRequest(" \n\t", 0), {
    hasPaths: false,
    imagePaths: [],
    maxTemplates: 1,
  });
});

test("profile import request accepts native picker arrays without changing order", () => {
  assert.deepEqual(
    profileImportRequest(
      [String.raw`D:\b.png`, " ", String.raw`C:\a.png`, String.raw`D:\b.png`],
      "not-a-number",
    ),
    {
      hasPaths: true,
      imagePaths: [
        String.raw`D:\b.png`,
        String.raw`C:\a.png`,
        String.raw`D:\b.png`,
      ],
      maxTemplates: 1,
    },
  );
});

test("profile paste shortcut ignores editable focused controls", () => {
  assert.equal(
    shouldHandleProfilePaste({ key: "v", ctrlKey: true }, { tagName: "DIV" }),
    true,
  );
  assert.equal(
    shouldHandleProfilePaste({ key: "V", metaKey: true }, { tagName: "SECTION" }),
    true,
  );
  assert.equal(
    shouldHandleProfilePaste({ key: "v", ctrlKey: true }, { tagName: "INPUT" }),
    false,
  );
  assert.equal(
    shouldHandleProfilePaste({ key: "v", ctrlKey: true }, { tagName: "textarea" }),
    false,
  );
  assert.equal(
    shouldHandleProfilePaste(
      { key: "v", ctrlKey: true },
      { tagName: "DIV", isContentEditable: true },
    ),
    false,
  );
  assert.equal(
    shouldHandleProfilePaste(
      { key: "v", ctrlKey: true },
      { tagName: "DIV", getAttribute: () => "textbox" },
    ),
    false,
  );
});

test("profile paste shortcut requires plain Ctrl or Meta V", () => {
  assert.equal(shouldHandleProfilePaste({ key: "v" }, { tagName: "DIV" }), false);
  assert.equal(
    shouldHandleProfilePaste({ key: "x", ctrlKey: true }, { tagName: "DIV" }),
    false,
  );
  assert.equal(
    shouldHandleProfilePaste({ key: "v", ctrlKey: true, altKey: true }, { tagName: "DIV" }),
    false,
  );
});

test("profile import status reports added, pruned, and target totals", () => {
  assert.equal(
    profileImportStatusText({
      addedCount: 2,
      prunedCount: 1,
      targets: [{}, {}, {}],
    }),
    "Ready - 导入 2 张，裁剪 1 张，当前 3 张",
  );

  assert.equal(
    profileImportStatusText({
      added_count: 1,
      pruned_count: 0,
      targets: [{}],
    }),
    "Ready - 导入 1 张，当前 1 张",
  );
});

test("evidence directory status and log expose the opened screenshots path", () => {
  const result = {
    path: String.raw`C:\Users\Wes\AppData\Local\ScreenWatchOCR\screenshots`,
  };

  assert.equal(
    evidenceDirectoryStatusText(result),
    String.raw`Ready - C:\Users\Wes\AppData\Local\ScreenWatchOCR\screenshots`,
  );
  assert.equal(
    evidenceDirectoryLogText(result),
    String.raw`打开证据目录：C:\Users\Wes\AppData\Local\ScreenWatchOCR\screenshots`,
  );
});

test("evidence directory status has a safe fallback when the path is absent", () => {
  assert.equal(evidenceDirectoryStatusText({}, { prefix: "Done" }), "Done - 证据目录已打开");
  assert.equal(evidenceDirectoryLogText({}), "打开证据目录");
});

test("profile target enabled status mirrors Python template counts", () => {
  assert.equal(
    profileTargetsEnabledStatusText({
      enabledCount: 2,
      targets: [{}, {}, {}],
    }),
    "Ready - 当前 3 张模板，启用 2 张",
  );

  assert.equal(
    profileTargetsEnabledStatusText({
      enabled_count: 1,
      targets: [{}, {}],
    }),
    "Ready - 当前 2 张模板，启用 1 张",
  );
});

test("profile target enabled status falls back to target enabled flags", () => {
  assert.equal(
    profileTargetsEnabledStatusText(
      {
        targets: [{ enabled: true }, { enabled: false }, {}],
      },
      { prefix: "Done" },
    ),
    "Done - 当前 3 张模板，启用 2 张",
  );
});

test("profile toggle all label follows the Python all-select button", () => {
  assert.equal(profileToggleAllLabel({ targets: [] }), "全选");
  assert.equal(
    profileToggleAllLabel({ targets: [{ enabled: true }, { enabled: false }] }),
    "全选",
  );
  assert.equal(
    profileToggleAllLabel({ targets: [{ enabled: true }, {}] }),
    "反选",
  );
  assert.equal(
    profileToggleAllLabel({ allEnabled: true, targets: [{ enabled: false }] }),
    "反选",
  );
});

test("monitoring status exposes tick hits and skipped remembered windows", () => {
  const status = monitoringStatusText(
    {
      running: true,
      skippedWindowApps: 2,
    },
    {
      tickHitCount: 3,
    },
  );

  assert.equal(status, "监控中 - 本轮 3 hits，缺失 2 个应用窗口");
});

test("monitoring status exposes the latest backend error", () => {
  const status = monitoringStatusText({
    running: true,
    errorCount: 2,
    lastError: "capture failed",
  });

  assert.equal(status, "监控中，错误: capture failed");
});

test("monitoring status prefers current tick error text", () => {
  const status = monitoringStatusText(
    {
      running: true,
      lastError: "old error",
    },
    {
      tickError: "new capture error",
    },
  );

  assert.equal(status, "监控中，错误: new capture error");
});

test("monitoring progress log text reports heartbeat tick details", () => {
  assert.equal(
    monitoringProgressLogText(
      {
        hitCount: 12,
        regionCount: 2,
        tickCount: 8,
        windowCount: 1,
      },
      {
        tickError: "capture failed",
        tickHitCount: 3,
      },
    ),
    "第 8 轮，扫描 2 屏 / 1 应用，本轮命中 3，累计命中 12，capture failed",
  );
});

test("monitoring heartbeat log text reports running status without a new tick", () => {
  assert.equal(
    monitoringHeartbeatLogText({
      hitCount: 4,
      regionCount: 1,
      tickCount: 0,
      windowCount: 2,
    }),
    "监控心跳：已完成 0 轮，扫描 1 屏 / 2 应用，累计命中 4",
  );
});

test("monitoring session generation accepts camel and snake case fields", () => {
  assert.equal(monitoringSessionGeneration({ generation: 7 }), 7);
  assert.equal(monitoringSessionGeneration({ sessionGeneration: 8 }), 8);
  assert.equal(monitoringSessionGeneration({ session_generation: 9 }), 9);
  assert.equal(monitoringSessionGeneration({ generation: "bad" }), 0);
});

test("monitoring event freshness rejects stale stopped and tick events", () => {
  assert.deepEqual(
    monitoringEventFreshness(
      { kind: "tick", snapshot: { generation: 2, running: true } },
      { currentGeneration: 3 },
    ),
    { accepted: false, generation: 2, stale: true },
  );
  assert.deepEqual(
    monitoringEventFreshness(
      { kind: "stopped", snapshot: { generation: 3, running: false } },
      { stoppedGeneration: 3 },
    ),
    { accepted: false, generation: 3, stale: true },
  );
});

test("monitoring event freshness accepts current events and pending starts", () => {
  assert.deepEqual(
    monitoringEventFreshness(
      { kind: "tick", snapshot: { generation: 4, running: true } },
      { currentGeneration: 4 },
    ),
    { accepted: true, generation: 4, stale: false },
  );
  assert.deepEqual(
    monitoringEventFreshness(
      { kind: "started", snapshot: { generation: 5, running: true } },
      { operationPending: "start", stoppedGeneration: 5 },
    ),
    { accepted: true, generation: 5, stale: false },
  );
});

test("monitoring event transition refreshes profile only for profile tick hits", () => {
  const transition = monitoringEventTransition(
    {
      kind: "tick",
      snapshot: {
        running: true,
        skippedWindowApps: 1,
      },
      tickHitCount: 2,
    },
    {
      profileMonitoringActive: true,
    },
  );

  assert.deepEqual(transition, {
    nextMonitoringActive: true,
    nextProfileMonitoringActive: true,
    shouldRefreshProfile: true,
    snapshot: {
      running: true,
      skippedWindowApps: 1,
    },
    statusText: "监控中 - 本轮 2 hits，缺失 1 个应用窗口",
  });
});

test("monitoring event transition handles stopped and snake-case tick errors", () => {
  const tick = monitoringEventTransition(
    {
      kind: "tick",
      running: true,
      tick_error: "capture failed",
      tick_hit_count: 3,
    },
    {
      profileMonitoringActive: false,
    },
  );
  assert.equal(tick.shouldRefreshProfile, false);
  assert.equal(tick.nextMonitoringActive, true);
  assert.equal(tick.statusText, "监控中 - 本轮 3 hits，错误: capture failed");

  const stopped = monitoringEventTransition(
    {
      kind: "stopped",
      snapshot: { running: false },
    },
    {
      profileMonitoringActive: true,
    },
  );
  assert.equal(stopped.nextMonitoringActive, false);
  assert.equal(stopped.nextProfileMonitoringActive, false);
  assert.equal(stopped.statusText, "Ready");
});

test("monitoring event transition keeps generic monitoring active without refreshing profile", () => {
  const started = monitoringEventTransition(
    {
      kind: "started",
      snapshot: { running: true },
    },
    {
      monitoringActive: false,
      profileMonitoringActive: false,
    },
  );
  assert.equal(started.nextMonitoringActive, true);
  assert.equal(started.nextProfileMonitoringActive, false);
  assert.equal(started.shouldRefreshProfile, false);

  const stoppedBySnapshot = monitoringEventTransition(
    {
      kind: "tick",
      snapshot: { running: false },
      tickHitCount: 1,
    },
    {
      monitoringActive: true,
      profileMonitoringActive: true,
    },
  );
  assert.equal(stoppedBySnapshot.nextMonitoringActive, false);
  assert.equal(stoppedBySnapshot.nextProfileMonitoringActive, false);
  assert.equal(stoppedBySnapshot.shouldRefreshProfile, true);
});

test("monitor error summary reports error count without text", () => {
  const summary = monitorErrorSummary({
    error_count: 3,
  });

  assert.deepEqual(summary, {
    errorCount: 3,
    text: "错误 3 次",
  });
});

test("window hidden state names match Python tray and minimize handling", () => {
  assert.equal(isHiddenWindowState("withdrawn"), true);
  assert.equal(isHiddenWindowState("iconic"), true);
  assert.equal(isHiddenWindowState("minimized"), true);
  assert.equal(isHiddenWindowState("normal"), false);
});

test("current window geometry keeps the last visible value while iconic", () => {
  const geometry = currentWindowGeometry("iconic", {
    geometry: "160x28+-32000+-32000",
    lastGeometry: "1200x700+30+40",
  });

  assert.equal(geometry, "1200x700+30+40");
});

test("current window geometry accepts valid visible geometry", () => {
  const geometry = currentWindowGeometry("normal", {
    geometry: "1400x900+120+80",
    lastGeometry: "980x680+0+0",
  });

  assert.equal(geometry, "1400x900+120+80");
});

test("remembered window geometry ignores hidden taskbar configure sizes", () => {
  const geometry = rememberWindowGeometry(
    "withdrawn",
    { height: 1, width: 1, x: -32000, y: -32000 },
    { lastGeometry: "1400x900+120+80" },
  );

  assert.equal(geometry, "1400x900+120+80");
});

test("remembered window geometry stores the full visible size and position", () => {
  const geometry = rememberWindowGeometry("normal", {
    height: 720,
    width: 1200,
    x: 30,
    y: 40,
  });

  assert.equal(geometry, "1200x720+30+40");
});

test("layout ratio capture uses left pane height for the vertical sash", () => {
  const ratios = captureLayoutRatios({
    firstSashX: 700,
    leftPaneHeight: 500,
    leftSashY: 300,
    rootWidth: 1000,
    secondSashX: 900,
  });

  assert.deepEqual(ratios, {
    leftRatio: 0.6,
    mainRatio: 0.7,
    rightRatio: 0.2,
  });
});

test("layout ratio capture clamps extreme sash positions", () => {
  const ratios = captureLayoutRatios({
    firstSashX: 20,
    leftPaneHeight: 500,
    leftSashY: 900,
    rootWidth: 1000,
    secondSashX: 980,
  });

  assert.deepEqual(ratios, {
    leftRatio: 0.8,
    mainRatio: 0.25,
    rightRatio: 0.4,
  });
});

test("side pane widths stay equal and bounded like the Python app", () => {
  assert.equal(sidePaneWidth(2388, 0.5), 955);
  assert.equal(sidePaneWidth(1453, 0.16), 320);
});

test("horizontal sashes restore saved three-column ratios", () => {
  assert.deepEqual(horizontalSashes(1200, { mainRatio: 0.42, rightRatio: 0.25 }), [
    504,
    804,
  ]);
});

test("horizontal sashes fall back to bounded equal side panes when narrow", () => {
  assert.deepEqual(horizontalSashes(800, { mainRatio: 0.42, rightRatio: 0.25 }), [
    360,
    580,
  ]);
});

test("resizable three-pane layout keeps all workbench columns bounded", () => {
  const layout = resizeThreePaneLayout(
    { first: 500, second: 340, third: 360 },
    { delta: 420, splitter: "targets-controls" },
    {
      minFirst: 330,
      minSecond: 270,
      minThird: 270,
      total: 1200,
    },
  );

  assert.deepEqual(layout, {
    first: 660,
    second: 270,
    third: 270,
  });
});

test("resizable three-pane layout scales saved widths to the current window", () => {
  const layout = resizeThreePaneLayout(
    { first: 700, second: 340, third: 460 },
    {},
    {
      minFirst: 330,
      minSecond: 270,
      minThird: 270,
      total: 1000,
    },
  );

  assert.deepEqual(layout, {
    first: 460,
    second: 270,
    third: 270,
  });
});

test("resizable stacked layout keeps the image list and log usable", () => {
  const layout = resizeStackedPaneLayout(
    { first: 260, second: 140 },
    { delta: -220, splitter: "first-second" },
    {
      minFirst: 120,
      minSecond: 88,
      total: 400,
    },
  );

  assert.deepEqual(layout, {
    first: 120,
    second: 280,
  });
});

test("resizable multi-pane layout adjusts adjacent control groups only", () => {
  const layout = resizeMultiPaneLayout(
    [100, 120, 140, 260, 120],
    { index: 1, delta: 30 },
    {
      count: 5,
      minimums: [70, 70, 90, 150, 88],
      total: 740,
    },
  );

  assert.deepEqual(layout, [100, 150, 110, 260, 120]);
});

test("resizable multi-pane layout clamps when a neighboring group reaches minimum", () => {
  const layout = resizeMultiPaneLayout(
    [100, 120, 140, 260, 120],
    { index: 3, delta: 200 },
    {
      count: 5,
      minimums: [70, 70, 90, 150, 88],
      total: 740,
    },
  );

  assert.deepEqual(layout, [100, 120, 140, 292, 88]);
});

test("restore layout waits until panes are mapped", () => {
  const plan = restoreLayoutPlan(
    { leftPaneHeight: 1, width: 1, windowState: "withdrawn" },
    { leftRatio: 0.5, mainRatio: 0.42, rightRatio: 0.25 },
  );

  assert.deepEqual(plan, {
    horizontalSashes: null,
    retry: true,
    retryHorizontal: true,
    retryVertical: true,
    verticalSashY: null,
  });
});

test("restore layout applies horizontal sashes while vertical pane is not ready", () => {
  const plan = restoreLayoutPlan(
    { leftPaneHeight: 1, width: 1200, windowState: "normal" },
    { leftRatio: 0.5, mainRatio: 0.42, rightRatio: 0.25 },
  );

  assert.deepEqual(plan, {
    horizontalSashes: [504, 804],
    retry: true,
    retryHorizontal: false,
    retryVertical: true,
    verticalSashY: null,
  });
});

test("restore layout can update only the vertical sash", () => {
  const plan = restoreLayoutPlan(
    { leftPaneHeight: 500, width: 980, windowState: "normal" },
    { leftRatio: 0.5, mainRatio: 0.42, rightRatio: 0.25 },
    { horizontal: false },
  );

  assert.deepEqual(plan, {
    horizontalSashes: null,
    retry: false,
    retryHorizontal: false,
    retryVertical: false,
    verticalSashY: 250,
  });
});

test("layout busy covers resize, pane drag, window move, mouse, and extra gates", () => {
  assert.equal(layoutBusy({ now: 1000 }), false);
  assert.equal(layoutBusy({ layoutActiveUntil: 1200, now: 1000 }), true);
  assert.equal(layoutBusy({ resizeActiveUntil: 1200, now: 1000 }), true);
  assert.equal(layoutBusy({ moveActiveUntil: 1200, now: 1000 }), true);
  assert.equal(
    layoutBusy({ layoutActiveUntil: Number.POSITIVE_INFINITY, now: 1000 }),
    true,
  );
  assert.equal(layoutBusy({ mouseButtonDown: true, now: 1000 }), true);
  assert.equal(layoutBusy({ extraBusy: true, now: 1000 }), true);
});

test("window resize transition treats same-size configure as a move", () => {
  assert.deepEqual(
    windowResizeTransition(
      { height: 680, width: 980, windowState: "normal" },
      { lastRootSize: [980, 680] },
    ),
    {
      cancelResizeJob: false,
      heightChanged: false,
      kind: "move",
      rememberGeometry: true,
      scheduleScale: false,
      suspendPreviews: false,
      widthChanged: false,
    },
  );
});

test("window resize transition ignores hidden taskbar configure events", () => {
  assert.deepEqual(
    windowResizeTransition(
      { height: 28, width: 160, windowState: "iconic" },
      { lastRootSize: [1200, 700], resizeJob: "job" },
    ),
    {
      cancelResizeJob: true,
      heightChanged: false,
      kind: "hidden",
      rememberGeometry: false,
      resetResizeActive: true,
      scheduleScale: false,
      suspendPreviews: false,
      widthChanged: false,
    },
  );
});

test("window resize transition exposes vertical-only resize without horizontal reset", () => {
  const transition = windowResizeTransition(
    { height: 720, width: 980, windowState: "normal" },
    { lastRootSize: [980, 680] },
  );

  assert.equal(transition.kind, "resize");
  assert.equal(transition.widthChanged, false);
  assert.equal(transition.heightChanged, true);
  assert.equal(transition.suspendPreviews, true);
  assert.equal(transition.scheduleScale, true);
});

test("custom check indicator metrics match the Python scaling contract", () => {
  assert.deepEqual(checkIndicatorMetrics(2), { size: 26, strokeWidth: 4 });
  assert.deepEqual(checkIndicatorMetrics(0.5), { size: 12, strokeWidth: 2 });
  assert.deepEqual(checkIndicatorMetrics(Number.NaN), {
    size: 13,
    strokeWidth: 2,
  });
});

test("custom check indicators write scaled CSS variables to host and input", () => {
  const { host, input } = fakeCheckboxHost();

  const metrics = applyCheckIndicatorScale(host, { scale: 2 });

  assert.deepEqual(metrics, { size: 26, strokeWidth: 4 });
  assert.equal(host.style.getPropertyValue("--check-size"), "26px");
  assert.equal(host.style.getPropertyValue("--check-mark-stroke"), "4px");
  assert.equal(input.style.getPropertyValue("--check-size"), "26px");
  assert.equal(input.classList.contains("is-custom-check"), true);
});

test("custom check indicator installer applies every registered control", () => {
  const one = fakeCheckboxHost();
  const two = fakeCheckboxHost([]);
  const root = {
    querySelectorAll(selector) {
      assert.equal(
        selector,
        ".check-control, input[type='checkbox'].is-custom-check",
      );
      return [one.host, two.input];
    },
  };

  const count = installCustomCheckIndicators(root, { scale: 2 });

  assert.equal(count, 2);
  assert.equal(one.host.style.getPropertyValue("--check-size"), "26px");
  assert.equal(two.input.style.getPropertyValue("--check-size"), "26px");
  assert.equal(two.input.classList.contains("is-custom-check"), true);
});

test("repeat click detection opens on the second click of the same target", () => {
  const state = {};

  const first = recordRepeatClick(state, 0, {
    now: () => 10_000,
    thresholdMs: 500,
  });
  const second = recordRepeatClick(state, 0, {
    now: () => 10_300,
    thresholdMs: 500,
  });

  assert.equal(first.repeated, false);
  assert.equal(second.repeated, true);
  assert.equal(second.previousIndex, 0);
  assert.equal(second.elapsed, 300);
});

test("repeat click detection resets when a different target is clicked", () => {
  const state = {};

  recordRepeatClick(state, 0, { now: () => 10_000 });
  const next = recordRepeatClick(state, 2, { now: () => 10_200 });
  const repeated = recordRepeatClick(state, 2, { now: () => 10_350 });

  assert.equal(next.repeated, false);
  assert.equal(repeated.repeated, true);
  assert.equal(repeated.previousIndex, 2);
});

test("repeat click detection ignores stale second clicks", () => {
  const state = {};

  recordRepeatClick(state, 1, { now: () => 10_000, thresholdMs: 500 });
  const stale = recordRepeatClick(state, 1, {
    now: () => 10_650,
    thresholdMs: 500,
  });

  assert.equal(stale.repeated, false);
  assert.equal(stale.elapsed, 650);
});

test("target selection touches only the old and new target cards", () => {
  const cards = [fakeCard(0, ["is-selected"]), fakeCard(1), fakeCard(2)];
  const result = selectIndexedListItem({ children: cards }, 2);

  assert.deepEqual(result, {
    changed: true,
    previousIndex: 0,
    selectedIndex: 2,
    touched: 2,
  });
  assert.equal(cards[0].classList.contains("is-selected"), false);
  assert.equal(cards[1].classList.contains("is-selected"), false);
  assert.equal(cards[2].classList.contains("is-selected"), true);
});

test("target selection does not touch cards when the target is already selected", () => {
  const cards = [fakeCard(0), fakeCard(1, ["is-selected"]), fakeCard(2)];
  const result = selectIndexedListItem({ children: cards }, 1);

  assert.deepEqual(result, {
    changed: false,
    previousIndex: 1,
    selectedIndex: 1,
    touched: 0,
  });
});

test("target selection clears the previous card when requested index is missing", () => {
  const cards = [fakeCard(0), fakeCard(1, ["is-selected"])];
  const result = selectIndexedListItem({ children: cards }, 9);

  assert.deepEqual(result, {
    changed: true,
    previousIndex: 1,
    selectedIndex: null,
    touched: 1,
  });
  assert.equal(cards[1].classList.contains("is-selected"), false);
});

test("target selection index follows backend edit results", () => {
  assert.equal(
    targetSelectionIndexFromEditResult(
      {
        selectedIndex: 2,
        targets: [{}, {}, {}],
      },
      0,
    ),
    2,
  );
  assert.equal(
    targetSelectionIndexFromEditResult(
      {
        selected_index: 1,
        targets: [{}, {}],
      },
      0,
    ),
    1,
  );
  assert.equal(
    targetSelectionIndexFromEditResult(
      {
        selectedIndex: null,
        targets: [],
      },
      0,
    ),
    null,
  );
});

test("target selection index follows unchanged hit-clear results", () => {
  assert.equal(
    targetSelectionIndexFromEditResult(
      {
        changed: false,
        selectedIndex: 1,
        targets: [{ id: "a" }, { id: "b", hit_count: 0 }],
      },
      0,
    ),
    1,
  );
});

test("target selection index follows a profile gallery edit workflow", () => {
  let selected = null;

  selected = targetSelectionIndexFromEditResult(
    {
      changed: true,
      selectedIndex: 2,
      targets: [{ id: "red" }, { id: "green" }, { id: "blue" }],
    },
    selected,
  );
  assert.equal(selected, 2);

  selected = targetSelectionIndexFromEditResult(
    {
      changed: true,
      enabledCount: 0,
      allEnabled: false,
      targets: [
        { id: "red", enabled: false },
        { id: "green", enabled: false },
        { id: "blue", enabled: false },
      ],
    },
    selected,
  );
  assert.equal(selected, 2);

  selected = targetSelectionIndexFromEditResult(
    {
      changed: true,
      selectedIndex: 0,
      targets: [{ id: "blue" }, { id: "red" }, { id: "green" }],
    },
    selected,
  );
  assert.equal(selected, 0);

  selected = targetSelectionIndexFromEditResult(
    {
      changed: true,
      selectedIndex: null,
      targets: [{ id: "red" }, { id: "green" }],
    },
    selected,
  );
  assert.equal(selected, null);

  selected = targetSelectionIndexFromEditResult(
    {
      changed: true,
      selected_index: 1,
      targets: [{ id: "red" }, { id: "new" }],
    },
    selected,
  );
  assert.equal(selected, 1);

  selected = targetSelectionIndexFromEditResult(
    {
      changed: true,
      selectedIndex: 1,
      targets: [],
    },
    selected,
  );
  assert.equal(selected, null);
});

test("target selection index preserves fallback only when edit result has no selection", () => {
  assert.equal(targetSelectionIndexFromEditResult({}, 3), 3);
  assert.equal(
    targetSelectionIndexFromEditResult(
      {
        selectedIndex: 9,
        targets: [{}, {}],
      },
      1,
    ),
    null,
  );
  assert.equal(
    targetSelectionIndexFromEditResult(
      {
        selectedIndex: "not-a-number",
        targets: [{}, {}],
      },
      1,
    ),
    null,
  );
});

test("profile load target selection defaults to first target like Python", () => {
  assert.equal(
    targetSelectionIndexForProfileLoad(
      { targets: [{ id: "a" }, { id: "b" }] },
      1,
      { selectFirst: true },
    ),
    0,
  );
});

test("profile refresh target selection preserves valid current index", () => {
  assert.equal(
    targetSelectionIndexForProfileLoad(
      { targets: [{ id: "a" }, { id: "b" }, { id: "c" }] },
      2,
    ),
    2,
  );
});

test("profile refresh target selection clears missing indexes and empty profiles", () => {
  assert.equal(
    targetSelectionIndexForProfileLoad({ targets: [{ id: "a" }] }, 3),
    null,
  );
  assert.equal(targetSelectionIndexForProfileLoad({ targets: [] }, 0), null);
});

test("target enabled helper preserves Python missing-enabled default", () => {
  assert.equal(targetEnabled({}), true);
  assert.equal(targetEnabled({ enabled: true }), true);
  assert.equal(targetEnabled({ enabled: false }), false);
});

test("target menu state enables hit clearing only for targets with id and hits", () => {
  assert.deepEqual(targetMenuState({ id: "target-a", path: "templates/a.png", hit_count: 3 }), {
    canClearHits: true,
    canOpen: true,
    clearHitsDisabled: false,
    hitCount: 3,
    openDisabled: false,
  });
  assert.equal(targetMenuState({ id: "target-a", hitCount: 0 }).canClearHits, false);
  assert.equal(targetMenuState({ path: "templates/a.png", hitCount: 4 }).canClearHits, false);
  assert.equal(targetMenuState({ id: "target-a", hitCount: 4 }).canOpen, false);
});

test("target action state mirrors profile row button behavior", () => {
  assert.deepEqual(
    targetActionState(
      { id: "target-b", path: "templates/b.png", hit_count: 2 },
      1,
      3,
    ),
    {
      canClearHits: true,
      canOpen: true,
      clearHitsDisabled: false,
      deleteDisabled: false,
      hitCount: 2,
      moveDownDisabled: false,
      moveDownInsertIndex: 3,
      moveUpDisabled: false,
      moveUpInsertIndex: 0,
      openDisabled: false,
      targetId: "target-b",
    },
  );
  assert.equal(targetActionState({}, 0, 3).moveUpDisabled, true);
  assert.equal(targetActionState({}, 2, 3).moveDownDisabled, true);
  assert.equal(targetActionState({ hitCount: 4 }, 1, 3).clearHitsDisabled, true);
  assert.equal(targetActionState({ id: "target-c" }, 1, 3).openDisabled, true);
});

test("target hit counts accept snake and camel case while clamping invalid values", () => {
  assert.equal(targetHitCount({ hit_count: "7" }), 7);
  assert.equal(targetHitCount({ hitCount: 2.9 }), 2);
  assert.equal(targetHitCount({ hit_count: -1 }), 0);
  assert.equal(targetHitCount({ hitCount: Number.NaN }), 0);
});

test("target drop position uses the card midpoint", () => {
  const item = fakeDropItem(100, 80);

  assert.equal(targetDropAfter(item, 139), false);
  assert.equal(targetDropAfter(item, 141), true);
});

test("target drop insert index mirrors before and after markers", () => {
  assert.equal(targetDropInsertIndex(2, false), 2);
  assert.equal(targetDropInsertIndex(2, true), 3);
  assert.equal(targetDropInsertIndex(-4, true), 0);
});

test("target menu viewport fitting keeps context actions visible", () => {
  const position = fitFixedMenuInViewport(
    { bottom: 230, height: 80, left: 220, right: 330, top: 150, width: 110 },
    { height: 200, width: 300 },
  );

  assert.deepEqual(position, {
    adjustedLeft: true,
    adjustedTop: true,
    left: 182,
    top: 112,
  });
});
