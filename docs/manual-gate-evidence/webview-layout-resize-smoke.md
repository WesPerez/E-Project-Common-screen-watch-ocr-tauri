Gate: WebView Layout Resize Smoke
Completion status: pass
Date/time: 2026-07-07T10:33:31.465Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe; exit 0
Release build-info hash: actualExeSha256=98f47746b032b9f5326083ab2c4bf6bb8f1567583f5fcc71757f56e121a0c7c0; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=98f47746b032b9f5326083ab2c4bf6bb8f1567583f5fcc71757f56e121a0c7c0; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-183233\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-183233\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke dragged the target/settings splitter, settings/preview splitter, target-list/log splitter, and a control-panel group splitter; each drag produced measured dimension changes without horizontal overflow
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-183233-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-183233-app.log; docs\manual-gate-evidence\logs\webview-layout-resized-20260707-183233.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2 layout resize path on this desktop viewport; it does not exhaustively cover every DPI scale or very narrow mobile layout
