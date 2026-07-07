Gate: Legacy Late-Start Window End-to-End Smoke
Completion status: pass
Date/time: 2026-07-07T14:48:15.012Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate legacy-late-window; exit 0
Release build-info hash: actualExeSha256=1d03e007175f987b95e2523c16e611f27cd71a86c10eed9aab0ac24ea5d189fe; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=1d03e007175f987b95e2523c16e611f27cd71a86c10eed9aab0ac24ea5d189fe; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: generated input fixture directory was not retained for this focused gate; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-224804\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke staged a Python-shaped profile_1.json before launch while the remembered app window was absent; verified scan was still runnable and reported the missing remembered app instead of losing the source; started the remembered app window after Tauri had loaded the profile; then scanned and monitored without reselecting the window, producing positive hit/evidence/profile updates
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-224804-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-224804-app.log; docs\manual-gate-evidence\logs\legacy-late-window-loaded-missing-20260707-224804.png; docs\manual-gate-evidence\logs\legacy-late-window-scan-hit-20260707-224804.png; docs\manual-gate-evidence\logs\legacy-late-window-monitoring-20260707-224804.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves late-start recovery for one generated remembered app window on this Windows desktop; broad third-party window capture behavior still depends on OS/window-class support
