Gate: WebView Layout Resize Smoke
Completion status: pass
Date/time: 2026-07-07T15:45:42.090Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\target\release\screen-watch-ocr-tauri.exe --gate layout; exit 0
Release build-info hash: actualExeSha256=45dd760e926a15563b9e2b0cb4ec4d69b93c54a3b695319627d2acb7946864a7; exePath=target\release\screen-watch-ocr-tauri.exe; buildInfoExecutableSha256=45dd760e926a15563b9e2b0cb4ec4d69b93c54a3b695319627d2acb7946864a7; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: generated input fixture directory was not retained for this focused gate; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-234534\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke dragged the target/settings splitter, settings/preview splitter, target-list/log splitter, and a control-panel group splitter; each drag produced measured dimension changes without horizontal overflow
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-234534-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-234534-app.log; docs\manual-gate-evidence\logs\webview-layout-resized-20260707-234534.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2 layout resize path on this desktop viewport; it does not exhaustively cover every DPI scale or very narrow mobile layout
