Gate: WebView Layout Resize Smoke
Completion status: pass
Date/time: 2026-07-06T21:28:48.517Z
Machine: DESKTOP-9FRQ8VV
Worktree note: screen-watch-ocr-tauri is not a git repository
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate all; exit 0
Release build-info hash: executableSha256=b67d3ef1329d753451db661c2c0a06240e1b32459a4724b68ca9466bd2b47bf8; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-052803\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-052803\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke dragged the target/settings splitter, settings/preview splitter, target-list/log splitter, and a control-panel group splitter; each drag produced measured dimension changes without horizontal overflow
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-052803-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-052803-app.log; docs\manual-gate-evidence\logs\webview-layout-resized-20260707-052803.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2 layout resize path on this desktop viewport; it does not exhaustively cover every DPI scale or very narrow mobile layout
