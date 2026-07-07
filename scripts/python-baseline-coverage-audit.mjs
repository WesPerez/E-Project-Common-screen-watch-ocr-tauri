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

const baselinePath = join(projectRoot, "docs", "PYTHON_BASELINE_TESTS.txt");
const baselineTests = readFileSync(baselinePath, "utf8")
  .split(/\r?\n/)
  .map((line) => line.trim())
  .filter((line) => line && !line.startsWith("#"));

const sources = {
  pythonTests: read("tests/test_core.py", pythonProject),
  functionalAcceptance: read("docs/FUNCTIONAL_ACCEPTANCE.md"),
  comparisonAudit: read("docs/COMPARISON_AUDIT.md"),
  frontendTests: read("src/ui-behavior.test.js"),
  frontendBehavior: read("src/ui-behavior.js"),
  frontend: read("src/main.js"),
  coreDetect: read("crates/screen-watch-core/src/detect.rs"),
  coreProfile: read("crates/screen-watch-core/src/profile.rs"),
  coreSources: read("crates/screen-watch-core/src/sources.rs"),
  coreEvidence: read("crates/screen-watch-core/src/evidence.rs"),
  coreAudio: read("crates/screen-watch-core/src/audio.rs"),
  coreConfig: read("crates/screen-watch-core/src/config.rs"),
  tauriLib: read("src-tauri/src/lib.rs"),
  monitorSession: read("src-tauri/src/monitor_session.rs"),
  tray: read("src-tauri/src/tray.rs"),
  startup: read("src-tauri/src/startup.rs"),
  singleInstance: read("src-tauri/src/single_instance.rs"),
  windowCapture: read("src-tauri/src/window_capture.rs"),
  dwmPreview: read("src-tauri/src/dwm_preview.rs"),
  windowLayout: read("src-tauri/src/window_layout.rs"),
  windowSources: read("src-tauri/src/window_sources.rs"),
  previewCache: read("src-tauri/src/preview_cache.rs"),
  screenCapture: read("src-tauri/src/screen_capture.rs"),
  packagedSmoke: read("scripts/packaged-smoke.ps1"),
  webviewSmoke: read("scripts/webview-visual-smoke.mjs"),
  coexistenceSmoke: read("scripts/coexistence-smoke.ps1"),
};

function assertContains(category, sourceName, needle) {
  if (!sources[sourceName].includes(needle)) {
    throw new Error(`${category}: missing ${sourceName} evidence needle: ${needle}`);
  }
}

function testName(testId) {
  return testId.split(".").pop();
}

const categories = [
  {
    name: "detector and config logic",
    match:
      /(?:template_and_pixel_demo|parse_scales|parse_positive_|scan_interval|detector_(?:does|large|matches|uses)|mostly_black)/,
    evidence: [
      ["functionalAcceptance", "Pixel target matches RGB tolerance and target identity"],
      ["functionalAcceptance", "Large-frame template path uses coarse/refine without missing targets"],
      ["functionalAcceptance", "Multiple template targets respect configured worker limit"],
      ["coreDetect", "large_frame_template_detection_uses_coarse_refine_without_missing_unaligned_hit"],
      ["coreDetect", "template_worker_count_caps_to_jobs_and_clamps_zero_limit"],
      ["coreConfig", "parses_scale_syntax_like_python_baseline"],
    ],
  },
  {
    name: "window capture source preview and DWM",
    match:
      /^test_(?:window_capture|window_preview|window_refresh|screen_preview|refresh_source_previews|schedule_source_previews|preview_height|dwm_|suspend_dwm|restore_overlay|window_map|window_unmap|layout_drag_suspends_dwm_preview)/,
    evidence: [
      ["functionalAcceptance", "Window capture falls back from black PrintWindow output to visible capture"],
      ["functionalAcceptance", "Screen preview captures a real frame"],
      ["functionalAcceptance", "DWM preview registers and reuses thumbnails"],
      ["windowCapture", "choose_window_frame_falls_back_to_visible_when_printwindow_is_black"],
      ["dwmPreview", "real_dwm_thumbnail_registers_updates_and_clears_on_windows_desktop"],
      ["frontendTests", "source preview DWM handoff keeps a card healthy"],
      ["webviewSmoke", "source-preview"],
    ],
  },
  {
    name: "profile template gallery and hit counts",
    match:
      /(?:template_name|normalize_target|normalize_profile|profile_roundtrip|profile_restores|remove_selected|reorder_target|target_drop|target_right_click|target_card|gallery_mousewheel|toggle_all_targets|select_all_button|select_target|click_target|open_target_file|clear_target_hit_count|count_badge|record_target_hits|add_image|prune_alerts|migrate_legacy_data)/,
    evidence: [
      ["functionalAcceptance", "Template names use `profile-count-stamp`"],
      ["functionalAcceptance", "Reordering templates renames files by new position"],
      ["functionalAcceptance", "Target hit counts persist and update matching ids"],
      ["comparisonAudit", "Template naming, prune limit, reorder, delete, clear"],
      ["tauriLib", "profile_gallery_edit_workflow_preserves_file_boundaries"],
      ["coreProfile", "reorder_profile_target_at"],
      ["frontendTests", "target drop position uses the card midpoint"],
      ["webviewSmoke", "template-gallery"],
    ],
  },
  {
    name: "layout resize and input behavior",
    match:
      /^test_(?:entry_click|custom_check|resize_|vertical_resize|horizontal_resize|taskbar_minimize|restore_layout|left_ratio|layout_busy|layout_drag_release|side_panes|horizontal_sashes|save_state|current_window_geometry|autohide_scrollbar|apply_scale|after_window_shown|main_restores_layout)/,
    evidence: [
      ["functionalAcceptance", "Entry clicks keep cursor at end"],
      ["comparisonAudit", "Resizable layout splitters"],
      ["frontendTests", "resizable three-pane layout keeps all workbench columns bounded"],
      ["frontendTests", "entry cursor handlers keep single-line inputs at the end"],
      ["windowLayout", "does_not_save_taskbar_placeholder_geometry"],
      ["webviewSmoke", "layout"],
    ],
  },
  {
    name: "tray startup and single instance lifecycle",
    match:
      /(?:hide_to_tray|show_window|start_minimized|startup_arguments|single_instance|main_exits|core_app_forwards_start_minimized)/,
    evidence: [
      ["functionalAcceptance", "Second instance wakes existing app and exits"],
      ["comparisonAudit", "Close hides to tray"],
      ["comparisonAudit", "Tray Show/Exit"],
      ["tray", "close_hides_to_tray_only_when_tray_is_available"],
      ["startup", "startup_manager_writes_reads_and_removes_isolated_shortcut"],
      ["singleInstance", "notify_existing_instance_sends_tauri_protocol_and_accepts_ack"],
      ["packagedSmoke", "closeToTraySmokeVerified"],
      ["coexistenceSmoke", "Python and Tauri process names must not match"],
    ],
  },
  {
    name: "monitoring scan events evidence and alerts",
    match:
      /(?:poll_events|capture_target_frame|detector_config_|selected_apps_reload|window_selection_keys|beep_|beep_for|beep_volume)/,
    evidence: [
      ["functionalAcceptance", "Profile detector config uses only checked/enabled targets"],
      ["functionalAcceptance", "Target hit counts persist and update matching ids"],
      ["comparisonAudit", "Persistent monitoring start/stop/status"],
      ["comparisonAudit", "Beep behavior and throttling"],
      ["monitorSession", "start_replaces_previous_worker_that_is_still_stopping"],
      ["monitorSession", "record_monitor_tick_updates_snapshot_and_builds_event_payload"],
      ["coreAudio", "volume_is_clamped_like_python_baseline"],
      ["webviewSmoke", "monitoring"],
    ],
  },
];

const assignments = new Map();
for (const testId of baselineTests) {
  const name = testName(testId);
  const matched = categories.filter((category) => category.match.test(name));
  if (matched.length !== 1) {
    throw new Error(
      `${testId}: expected exactly one coverage category, matched ${matched
        .map((category) => category.name)
        .join(", ") || "none"}`,
    );
  }
  assignments.set(testId, matched[0].name);
}

for (const category of categories) {
  for (const [sourceName, needle] of category.evidence) {
    assertContains(category.name, sourceName, needle);
  }
}

for (const testId of baselineTests) {
  const name = testName(testId);
  if (!sources.pythonTests.includes(`def ${name}`)) {
    throw new Error(`${testId}: locked baseline test is missing from Python test source`);
  }
}

const categoryCounts = Object.fromEntries(
  categories.map((category) => [
    category.name,
    Array.from(assignments.values()).filter((name) => name === category.name).length,
  ]),
);

for (const category of categories) {
  if (categoryCounts[category.name] === 0) {
    throw new Error(`${category.name}: no Python baseline tests mapped to this coverage category`);
  }
}

console.log(
  JSON.stringify(
    {
      pythonBaselineCoverageAudit: "passed",
      baselineTests: baselineTests.length,
      categories: categoryCounts,
      pythonProject,
      projectRoot,
    },
    null,
    2,
  ),
);
