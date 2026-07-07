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
  pythonCore: read("src/screen_watch/core.py", pythonProject),
  pythonApp: read("src/screen_watch/app.py", pythonProject),
  pythonReadme: read("README.md", pythonProject),
  tauriMain: read("src-tauri/src/main.rs"),
  tauriLib: read("src-tauri/src/lib.rs"),
  tray: read("src-tauri/src/tray.rs"),
  frontend: read("src/main.js"),
  packageJson: read("package.json"),
  comparisonAudit: read("docs/COMPARISON_AUDIT.md"),
  functionalAcceptance: read("docs/FUNCTIONAL_ACCEPTANCE.md"),
  webviewSmoke: read("scripts/webview-visual-smoke.mjs"),
  packagedSmoke: read("scripts/packaged-smoke.ps1"),
  verifyMigration: read("scripts/verify-migration.ps1"),
};

function requireNeedle(label, sourceName, needle) {
  if (!sources[sourceName].includes(needle)) {
    throw new Error(`${label}: missing ${sourceName} needle: ${needle}`);
  }
}

function rejectNeedle(label, sourceName, needle) {
  if (sources[sourceName].includes(needle)) {
    throw new Error(`${label}: unexpected ${sourceName} needle: ${needle}`);
  }
}

const legacyCli = [
  {
    command: "app",
    legacy: ['sub.add_parser("app")', 'p.add_argument("--smoke-test"', 'p.add_argument("--start-minimized"'],
    tauriEquivalent: [
      ["tauriMain", "screen_watch_ocr_tauri::run();"],
      ["tray", "start_minimized_from_env_args"],
      ["packagedSmoke", "--start-minimized"],
    ],
    boundary: "GUI app entry preserved as the packaged Tauri application, not as a Python module command.",
  },
  {
    command: "list-monitors",
    legacy: ['sub.add_parser("list-monitors")', "print(json.dumps(list_monitors()"],
    tauriEquivalent: [
      ["tauriLib", "fn list_monitors"],
      ["frontend", 'invoke("list_monitors"'],
      ["functionalAcceptance", "Profile source selection keeps Python-compatible monitor/window shapes"],
    ],
    boundary: "Monitor listing exists through Tauri backend/frontend and desktop smoke, but not as a stdout CLI command.",
  },
  {
    command: "once",
    legacy: ['sub.add_parser("once")', 'p.add_argument("--config", required=True)', "scan_frames(config, once=args.cmd == \"once\""],
    tauriEquivalent: [
      ["tauriLib", "fn scan_config_text_once"],
      ["tauriLib", "fn scan_profile_once"],
      ["frontend", 'invoke("scan_profile_once"'],
      ["webviewSmoke", "profile scan-once"],
    ],
    boundary: "One-shot scanning exists through raw-config/backend and visible profile scan paths, not as a command-line exit-code workflow.",
  },
  {
    command: "watch",
    legacy: ['sub.add_parser("watch")', 'p.add_argument("--duration", type=float)', "scan_frames(config, once=args.cmd == \"once\""],
    tauriEquivalent: [
      ["tauriLib", "fn start_monitoring_session"],
      ["tauriLib", "fn start_profile_monitoring_session"],
      ["frontend", 'invoke("start_profile_monitoring_session"'],
      ["webviewSmoke", "running profile monitoring restart gate"],
    ],
    boundary: "Continuous watch exists through Tauri monitoring sessions and WebView smoke, not as a blocking terminal command.",
  },
  {
    command: "screenshot",
    legacy: [
      'sub.add_parser("screenshot")',
      'p.add_argument("--monitor", type=int, default=1)',
      'p.add_argument("--out", default="evidence/screenshot.png")',
      "def screenshot(args):",
    ],
    tauriEquivalent: [
      ["tauriLib", "fn capture_screen_region_preview"],
      ["tauriLib", "fn capture_profile_source_template"],
      ["frontend", 'invoke("capture_screen_region_preview_cached"'],
      ["functionalAcceptance", "Capture current source as a template"],
    ],
    boundary: "Screen capture exists for preview/template workflows, but arbitrary CLI screenshot-to-path output is not preserved.",
  },
  {
    command: "make-demo",
    legacy: ['sub.add_parser("make-demo")', "def make_demo(out):", "config.demo.json"],
    tauriEquivalent: [
      ["webviewSmoke", "Visual Smoke Source"],
      ["webviewSmoke", "template-gallery"],
      ["verifyMigration", "OCR smoke missing-model self-test"],
    ],
    boundary: "Demo fixture generation remains a Python/dev-test utility; Tauri uses dedicated smoke fixtures instead of shipping this CLI command.",
  },
  {
    command: "self-test",
    legacy: ['sub.add_parser("self-test")', "def self_test(args):", 'p.add_argument("--ocr", action="store_true")'],
    tauriEquivalent: [
      ["functionalAcceptance", "Pixel target matches RGB tolerance and target identity"],
      ["functionalAcceptance", "Template matching supports scaled templates"],
      ["verifyMigration", "OCR smoke missing-model self-test"],
    ],
    boundary: "Self-test coverage exists in Rust/frontend/smoke tests; the packaged GUI exe does not expose the old CLI self-test command.",
  },
];

for (const item of legacyCli) {
  for (const needle of item.legacy) {
    requireNeedle(`legacy CLI ${item.command}`, "pythonCore", needle);
  }
  for (const [sourceName, needle] of item.tauriEquivalent) {
    requireNeedle(`Tauri equivalent for legacy CLI ${item.command}`, sourceName, needle);
  }
}

for (const needle of [
  "-m screen_watch list-monitors",
  "-m screen_watch screenshot",
  "-m screen_watch once",
  "-m screen_watch watch",
  "-m screen_watch make-demo",
  "-m screen_watch self-test",
]) {
  requireNeedle("legacy README CLI documentation", "pythonReadme", needle);
}

requireNeedle("Tauri desktop arg boundary", "tray", "--start-minimized");
requireNeedle("Tauri GUI entry", "tauriMain", "#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]");
rejectNeedle("Tauri source must not claim Python CLI parser parity", "tauriLib", "list-monitors");
rejectNeedle("Tauri source must not claim Python CLI parser parity", "tauriLib", "make-demo");
rejectNeedle("Tauri source must not claim Python CLI parser parity", "tauriLib", "self-test");
rejectNeedle("Tauri source must not claim Python CLI parser parity", "tauriMain", "clap");
rejectNeedle("Tauri source must not claim Python CLI parser parity", "tauriMain", "pico_args");

for (const needle of [
  "Legacy Python CLI commands",
  "CLI interface not preserved",
  "list-monitors",
  "once/watch",
  "screenshot-to-path",
  "make-demo/self-test",
]) {
  requireNeedle("comparison audit CLI boundary", "comparisonAudit", needle);
}

console.log(
  JSON.stringify(
    {
      legacyCliBoundaryAudit: "passed",
      legacyCommands: legacyCli.map((item) => item.command),
      boundary: "Tauri preserves desktop/backend behavior for the replacement app, but does not preserve the old Python command-line interface.",
      pythonProject,
      projectRoot,
    },
    null,
    2,
  ),
);
