Gate: WebView Source Preview Visual Smoke
Completion status: pass
Date/time: 2026-07-07T14:47:42.133Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe; exit 0
Release build-info hash: actualExeSha256=1d03e007175f987b95e2523c16e611f27cd71a86c10eed9aab0ac24ea5d189fe; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=1d03e007175f987b95e2523c16e611f27cd71a86c10eed9aab0ac24ea5d189fe; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-224646\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-224646\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected a physical screen and visible helper app-window source, refreshed source cards without unexpected failed previews, captured bitmap/DWM-backed cards, scrolled a preview card partially offscreen, resized the app window, restored the preview area, and refreshed again without stale/error cards
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-224646-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-224646-app.log; docs\manual-gate-evidence\logs\webview-source-preview-initial-20260707-224646.png; docs\manual-gate-evidence\logs\webview-source-preview-partial-scroll-20260707-224646.png; docs\manual-gate-evidence\logs\webview-source-preview-restored-20260707-224646.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2/runtime path on this interactive desktop; it does not prove every possible monitor topology or every third-party app window
