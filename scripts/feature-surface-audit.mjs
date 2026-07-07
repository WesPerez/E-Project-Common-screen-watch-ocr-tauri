import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const projectRoot = resolve(process.cwd());
const pythonProject = resolve(process.argv[2] || join(projectRoot, "..", "screen-watch-ocr"));

function read(relativePath, root = projectRoot) {
  const path = join(root, relativePath);
  if (!existsSync(path)) {
    throw new Error(`missing required file: ${path}`);
  }
  return readFileSync(path, "utf8");
}

const sources = {
  pythonApp: read("src/screen_watch/app.py", pythonProject),
  pythonCore: read("src/screen_watch/core.py", pythonProject),
  pythonTests: read("tests/test_core.py", pythonProject),
  html: read("index.html"),
  frontend: read("src/main.js"),
  frontendBehavior: read("src/ui-behavior.js"),
  frontendTests: read("src/ui-behavior.test.js"),
  tauriLib: read("src-tauri/src/lib.rs"),
  tray: read("src-tauri/src/tray.rs"),
  startup: read("src-tauri/src/startup.rs"),
  singleInstance: read("src-tauri/src/single_instance.rs"),
  monitorSession: read("src-tauri/src/monitor_session.rs"),
  windowSources: read("src-tauri/src/window_sources.rs"),
  windowCapture: read("src-tauri/src/window_capture.rs"),
  dwmPreview: read("src-tauri/src/dwm_preview.rs"),
  coreProfile: read("crates/screen-watch-core/src/profile.rs"),
  coreDetect: read("crates/screen-watch-core/src/detect.rs"),
  coreScan: read("crates/screen-watch-core/src/scan.rs"),
  coreEvidence: read("crates/screen-watch-core/src/evidence.rs"),
  coreOcr: read("crates/screen-watch-core/src/ocr.rs"),
  packageJson: read("package.json"),
  comparisonAudit: read("docs/COMPARISON_AUDIT.md"),
  acceptance: read("docs/ACCEPTANCE.md"),
  manualGates: read("docs/MANUAL_GATES.md"),
  verifyMigration: read("scripts/verify-migration.ps1"),
  webviewSmoke: read("scripts/webview-visual-smoke.mjs"),
  packagedSmoke: read("scripts/packaged-smoke.ps1"),
  coexistenceSmoke: read("scripts/coexistence-smoke.ps1"),
  pythonProfileCompatSmoke: read("scripts/python-profile-compat-smoke.ps1"),
};

const generatedHandlers = (() => {
  const match = sources.tauriLib.match(/generate_handler!\s*\[([\s\S]*?)\]\)/);
  if (!match) {
    throw new Error("could not locate tauri::generate_handler![...] block");
  }
  return new Set(
    match[1]
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean),
  );
})();

function requireNeedle(feature, side, sourceName, needle) {
  const source = sources[sourceName];
  if (!source.includes(needle)) {
    throw new Error(`${feature}: ${side} missing ${sourceName} needle: ${needle}`);
  }
}

function requireCommand(feature, command) {
  requireNeedle(feature, "frontend invoke", "frontend", `invoke("${command}"`);
  if (!generatedHandlers.has(command)) {
    throw new Error(`${feature}: backend command is not registered: ${command}`);
  }
  requireNeedle(feature, "backend command", "tauriLib", `fn ${command}`);
}

const features = [
  {
    name: "profile slots and startup toggle",
    legacy: [
      ["pythonApp", 'ttk.Label(profile_bar, text="配置位")'],
      ["pythonApp", '"开机自启"'],
      ["pythonApp", "PROFILE_COUNT"],
      ["pythonApp", "def toggle_startup"],
    ],
    tauri: [
      ["html", 'id="profile-number"'],
      ["html", '<option value="5">5</option>'],
      ["html", 'id="startup-toggle"'],
      ["frontend", 'querySelector("#startup-toggle")'],
      ["startup", 'STARTUP_LINK_NAME: &str = "屏幕监控OCR Tauri.lnk"'],
    ],
    commands: ["startup_status", "set_startup_enabled", "save_last_profile", "load_profile_state"],
    evidence: [
      ["comparisonAudit", "Startup shortcut isolated write/read smoke"],
      ["verifyMigration", "Legacy default settings contract"],
    ],
  },
  {
    name: "template import toolbar",
    legacy: [
      ["pythonApp", '"上传图片"'],
      ["pythonApp", '"粘贴图片"'],
      ["pythonApp", '"截图作模板"'],
      ["pythonApp", '"删除选中"'],
      ["pythonApp", '"清空"'],
      ["pythonApp", "def add_files"],
      ["pythonApp", "def paste_images"],
      ["pythonApp", "def capture_as_target"],
    ],
    tauri: [
      ["html", 'id="profile-select-pngs"'],
      ["html", 'id="profile-paste-images"'],
      ["html", 'id="profile-capture-target"'],
      ["html", 'id="profile-delete-selected"'],
      ["html", 'id="profile-clear-all"'],
      ["frontend", "selectProfilePngs"],
      ["frontend", "pasteProfileImages"],
      ["frontend", "captureProfileTarget"],
    ],
    commands: [
      "select_profile_template_pngs",
      "add_profile_template_pngs",
      "paste_profile_template_images",
      "capture_profile_source_template",
      "remove_profile_target",
      "clear_profile_targets",
    ],
    evidence: [
      ["comparisonAudit", "Template import from files"],
      ["comparisonAudit", "Clipboard/path paste templates"],
      ["comparisonAudit", "Capture selected screen/window as template"],
    ],
  },
  {
    name: "dynamic target card actions",
    legacy: [
      ["pythonApp", "def click_target"],
      ["pythonApp", "def open_target_file"],
      ["pythonApp", "def begin_target_drag"],
      ["pythonApp", "def reorder_target"],
      ["pythonApp", "def toggle_target"],
      ["pythonApp", "def clear_target_hit_count"],
      ["pythonApp", "def show_target_context_menu"],
    ],
    tauri: [
      ["frontend", "clickProfileTarget"],
      ["frontend", "openProfileTarget(index)"],
      ["frontend", "reorderProfileTarget"],
      ["frontend", "setProfileTargetEnabled"],
      ["frontend", "clearProfileHitCount"],
      ["frontend", "showTargetContextMenu"],
      ["frontendTests", "target drop position uses the card midpoint"],
      ["frontendTests", "target menu state enables hit clearing"],
    ],
    commands: [
      "open_profile_target_file",
      "reorder_profile_target",
      "set_profile_target_enabled",
      "toggle_all_profile_targets",
      "clear_profile_target_hit_count",
    ],
    evidence: [
      ["comparisonAudit", "Target enable/disable and select-all/invert"],
      ["comparisonAudit", "Hit-count badges and clear hit menu"],
    ],
  },
  {
    name: "screen and app source selection",
    legacy: [
      ["pythonApp", '"监控屏幕"'],
      ["pythonApp", '"监控应用"'],
      ["pythonApp", "def refresh_monitors"],
      ["pythonApp", "def selected_regions"],
      ["pythonApp", "def refresh_windows"],
      ["pythonApp", "def selected_windows"],
      ["pythonApp", "def reload_selected_apps"],
    ],
    tauri: [
      ["html", 'id="monitors"'],
      ["html", 'id="windows"'],
      ["html", 'id="refresh-windows"'],
      ["frontend", "renderMonitorList"],
      ["frontend", "refreshWindows"],
      ["frontend", "rememberWindowApp"],
      ["windowSources", "decorate_window_records"],
    ],
    commands: ["list_monitors", "list_app_windows", "save_profile_sources", "resolve_config_text_window_sources"],
    evidence: [
      ["comparisonAudit", "Screen source listing"],
      ["comparisonAudit", "App-window listing"],
      ["comparisonAudit", "legacy late-start remembered-app WebView smoke"],
    ],
  },
  {
    name: "region and match controls",
    legacy: [
      ["pythonApp", '"区域"'],
      ["pythonApp", '"阈值"'],
      ["pythonApp", '"缩放"'],
      ["pythonApp", '"间隔ms"'],
      ["pythonApp", '"同图冷却秒"'],
      ["pythonApp", '"蜂鸣秒"'],
      ["pythonApp", '"蜂鸣音量"'],
      ["pythonApp", '"模板最多张"'],
      ["pythonApp", '"截图最多张"'],
      ["pythonApp", '"命中蜂鸣"'],
      ["pythonApp", "def detector_config"],
    ],
    tauri: [
      ["html", 'id="profile-region-left"'],
      ["html", 'id="profile-threshold"'],
      ["html", 'id="profile-scales"'],
      ["html", 'id="profile-interval-ms"'],
      ["html", 'id="profile-cooldown"'],
      ["html", 'id="profile-beep-seconds"'],
      ["html", 'id="profile-beep-volume"'],
      ["html", 'id="profile-max-templates"'],
      ["html", 'id="profile-max-alerts"'],
      ["html", 'id="profile-beep"'],
      ["frontend", "buildProfileOptions"],
      ["coreProfile", "profile_watch_config_uses_enabled_targets_and_gui_defaults"],
    ],
    commands: ["build_profile_watch_config", "save_profile_sources"],
    evidence: [
      ["comparisonAudit", "legacy default settings contract"],
      ["verifyMigration", "Assert-LegacyDefaultSettingsContract"],
    ],
  },
  {
    name: "preview and DWM handoff",
    legacy: [
      ["pythonApp", '"来源预览"'],
      ["pythonApp", "def refresh_source_previews"],
      ["pythonApp", "def sync_dwm_preview"],
      ["pythonApp", "def capture_preview_frame"],
      ["pythonApp", "mostly_black"],
    ],
    tauri: [
      ["html", 'id="source-previews"'],
      ["html", 'id="refresh-source-previews"'],
      ["frontend", "refreshSourcePreviews"],
      ["frontend", "syncDwmPreview"],
      ["dwmPreview", "sync_window_preview"],
      ["windowCapture", "choose_window_frame"],
    ],
    commands: [
      "capture_screen_region_preview_cached",
      "capture_window_preview_cached",
      "retain_cached_preview_sources",
      "sync_dwm_preview",
      "retain_dwm_preview_sources",
      "clear_dwm_previews",
    ],
    evidence: [
      ["comparisonAudit", "Source preview with DWM handoff and bitmap fallback"],
      ["comparisonAudit", "Desktop smoke"],
    ],
  },
  {
    name: "scan monitor stop restart and logs",
    legacy: [
      ["pythonApp", '"开始监控"'],
      ["pythonApp", '"停止监控"'],
      ["pythonApp", '"扫描一次"'],
      ["pythonApp", "def toggle_monitoring"],
      ["pythonApp", "def start"],
      ["pythonApp", "def stop"],
      ["pythonApp", "def scan_once"],
      ["pythonApp", "def poll_events"],
      ["pythonApp", "self.log.insert"],
    ],
    tauri: [
      ["html", 'id="profile-monitor-start"'],
      ["html", 'id="profile-scan-once"'],
      ["html", 'id="event-log"'],
      ["frontend", "toggleProfileMonitoring"],
      ["frontend", "startProfileMonitoring"],
      ["frontend", "stopMonitoring"],
      ["frontend", "refreshMonitoringStatus"],
      ["frontend", "appendLog"],
      ["monitorSession", "start_replaces_previous_worker_that_is_still_stopping"],
    ],
    commands: [
      "scan_profile_once",
      "start_profile_monitoring_session",
      "stop_monitoring_session",
      "monitoring_session_status",
    ],
    evidence: [
      ["comparisonAudit", "Persistent monitoring start/stop/status"],
      ["comparisonAudit", "Stop then start monitoring again"],
      ["comparisonAudit", "WebView monitoring soak"],
    ],
  },
  {
    name: "evidence screenshots jsonl pruning and open directory",
    legacy: [
      ["pythonApp", '"打开证据目录"'],
      ["pythonApp", "def emit_alert"],
      ["pythonApp", "prune_alerts"],
      ["pythonApp", "alerts.jsonl"],
      ["pythonApp", "def open_evidence"],
    ],
    tauri: [
      ["html", 'id="open-evidence-dir"'],
      ["frontend", "openEvidenceDir"],
      ["frontendBehavior", "evidenceDirectoryStatusText"],
      ["coreEvidence", "write_alert_evidence"],
      ["coreEvidence", "prune_alert_images"],
      ["coreEvidence", "alerts.jsonl"],
      ["tauriLib", "open_evidence_dir"],
    ],
    commands: ["open_evidence_dir"],
    evidence: [
      ["comparisonAudit", "Alert screenshots, JSONL, cooldown, pruning, evidence directory open"],
      ["comparisonAudit", "profile one-shot scan smoke"],
    ],
  },
  {
    name: "tray close start-minimized and single-instance identity",
    legacy: [
      ["pythonApp", "def hide_to_tray"],
      ["pythonApp", "def ensure_tray_icon"],
      ["pythonApp", "def show_window"],
      ["pythonApp", "def start_instance_listener"],
      ["pythonApp", "ScreenWatchOCR:show"],
    ],
    tauri: [
      ["tray", 'TRAY_ID: &str = "screen-watch-ocr-tauri-main"'],
      ["tray", 'TRAY_MENU_SHOW_LABEL: &str = "Show Tauri"'],
      ["tray", 'Screen Watch OCR Tauri - Monitoring'],
      ["singleInstance", "INSTANCE_PORT: u16 = 47628"],
      ["singleInstance", "ScreenWatchOCRTauri:show"],
      ["packagedSmoke", "WindowsGui"],
      ["coexistenceSmoke", "Python and Tauri process names must not match"],
    ],
    commands: [],
    evidence: [
      ["comparisonAudit", "Close hides to tray"],
      ["comparisonAudit", "Tray Show/Exit"],
      ["comparisonAudit", "Single-instance wake"],
      ["comparisonAudit", "New/old process identity isolation"],
    ],
  },
  {
    name: "profile state compatibility and shared data boundary",
    legacy: [
      ["pythonApp", "def save_current_profile"],
      ["pythonApp", "def load_profile"],
      ["pythonApp", "PROFILES_DIR"],
      ["pythonApp", "STATE_PATH"],
      ["pythonTests", "test_profile_roundtrip"],
    ],
    tauri: [
      ["coreProfile", "read_profile_at"],
      ["coreProfile", "save_profile_sources_at"],
      ["coreProfile", "preserves_unknown_fields"],
      ["coreProfile", "profile_sources_save_python_compatible_shape"],
      ["coreProfile", "max_alerts_state_update_preserves_python_state_shape_and_unknown_fields"],
      ["pythonProfileCompatSmoke", "future_profile_after_load"],
    ],
    commands: ["load_profile", "load_profile_state", "normalize_profile", "save_profile_sources"],
    evidence: [
      ["comparisonAudit", "Python-read-Tauri profile compatibility smoke"],
      ["comparisonAudit", "simultaneous old/new writes to the same profile are not safe"],
    ],
  },
  {
    name: "detection engines template pixel OCR",
    legacy: [
      ["pythonCore", "class Detector"],
      ["pythonCore", "cv2.matchTemplate"],
      ["pythonCore", 'kind == "pixel"'],
      ["pythonCore", 'kind == "ocr_text"'],
      ["pythonTests", "test_detector_matches_scaled_template_from_range"],
    ],
    tauri: [
      ["coreDetect", "detect_targets"],
      ["coreDetect", "pixel_detection_matches_with_tolerance"],
      ["coreDetect", "scaled_template_detection_uses_python_style_floor_dimensions"],
      ["coreDetect", "ocr_text_detection_matches_unicode_contains"],
      ["coreOcr", "REQUIRED_NATIVE_OCR_ASSETS"],
    ],
    commands: ["scan_config_text_once"],
    evidence: [
      ["comparisonAudit", "Template benchmark"],
      ["comparisonAudit", "Real OCR smoke"],
      ["comparisonAudit", "PP-OCRv6/RapidOCR-native"],
    ],
  },
  {
    name: "one-page resizable layout and compact UI",
    legacy: [
      ["pythonApp", "PanedWindow"],
      ["pythonApp", "def restore_layout"],
      ["pythonApp", "def capture_layout_ratios"],
      ["pythonApp", "def apply_scale"],
    ],
    tauri: [
      ["html", 'id="app-grid"'],
      ["html", 'data-splitter="targets-controls"'],
      ["html", 'data-splitter="controls-preview"'],
      ["html", 'data-splitter="targets-log"'],
      ["frontend", "resizeThreePaneLayout"],
      ["frontend", "resizeStackedPaneLayout"],
      ["frontend", "resizeMultiPaneLayout"],
      ["frontendTests", "resizable three-pane layout keeps all workbench columns bounded"],
    ],
    commands: [],
    evidence: [
      ["comparisonAudit", "Resizable layout splitters"],
      ["comparisonAudit", "Smaller image thumbnails"],
    ],
  },
  {
    name: "packaging size gui subsystem and WebView2 boundary",
    legacy: [
      ["pythonProject", ""],
    ],
    tauri: [
      ["packageJson", '"tauri:build:lite"'],
      ["packageJson", '"package:portable:lite"'],
      ["verifyMigration", "Assert-SingleFileDeliverableContract"],
      ["verifyMigration", "WindowsGui"],
      ["comparisonAudit", "Runtime boundary"],
      ["comparisonAudit", "WebView2"],
    ],
    commands: [],
    evidence: [
      ["comparisonAudit", "Single-file app: `release-single\\ScreenWatchOCRTauri.exe`"],
      ["comparisonAudit", "Lite package size"],
      ["comparisonAudit", "Single exe launch on arbitrary Windows PCs"],
    ],
  },
];

let checks = 0;
for (const feature of features) {
  for (const [sourceName, needle] of feature.legacy || []) {
    if (sourceName === "pythonProject" && needle === "") {
      if (!existsSync(pythonProject)) {
        throw new Error(`${feature.name}: missing Python project at ${pythonProject}`);
      }
      checks += 1;
      continue;
    }
    requireNeedle(feature.name, "legacy", sourceName, needle);
    checks += 1;
  }
  for (const [sourceName, needle] of feature.tauri || []) {
    requireNeedle(feature.name, "tauri", sourceName, needle);
    checks += 1;
  }
  for (const command of feature.commands || []) {
    requireCommand(feature.name, command);
    checks += 3;
  }
  for (const [sourceName, needle] of feature.evidence || []) {
    requireNeedle(feature.name, "evidence", sourceName, needle);
    checks += 1;
  }
}

console.log(
  JSON.stringify(
    {
      featureSurfaceAudit: "passed",
      features: features.length,
      checks,
      pythonProject,
      projectRoot,
    },
    null,
    2,
  ),
);
