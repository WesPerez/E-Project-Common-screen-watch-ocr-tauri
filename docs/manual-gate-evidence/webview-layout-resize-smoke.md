Gate: WebView Layout Resize Smoke
Completion status: pass
Date/time: 2026-07-07T07:31:01.750Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe; exit 0
Release build-info hash: actualExeSha256=b7356d3a96810aa70fef42ee1fb360516411d145b1e8630f6a49f840c1efe3a4; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=b7356d3a96810aa70fef42ee1fb360516411d145b1e8630f6a49f840c1efe3a4; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-153009\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-153009\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke dragged the target/settings splitter, settings/preview splitter, target-list/log splitter, and a control-panel group splitter; each drag produced measured dimension changes without horizontal overflow
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-153009-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-153009-app.log; docs\manual-gate-evidence\logs\webview-layout-resized-20260707-153009.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2 layout resize path on this desktop viewport; it does not exhaustively cover every DPI scale or very narrow mobile layout
