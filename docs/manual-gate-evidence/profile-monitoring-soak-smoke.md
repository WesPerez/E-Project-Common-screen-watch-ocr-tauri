Gate: Profile Monitoring Soak Smoke
Completion status: pass
Date/time: 2026-07-07T14:58:55.282Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate monitoring-soak; exit 0
Release build-info hash: actualExeSha256=1d03e007175f987b95e2523c16e611f27cd71a86c10eed9aab0ac24ea5d189fe; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=1d03e007175f987b95e2523c16e611f27cd71a86c10eed9aab0ac24ea5d189fe; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: generated input fixture directory was not retained for this focused gate; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-225749\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, ran profile monitoring for 60000ms, sampled 31 UI states, observed tick delta 109, hit delta 109, progress-log delta 46, and stopped cleanly with the button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-225749-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-225749-app.log; docs\manual-gate-evidence\logs\profile-monitoring-soak-prepared-20260707-225749.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-mid-20260707-225749.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-running-20260707-225749.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves a sustained packaged WebView2 monitoring run on this Windows interactive desktop with a generated stable window source; it is still not a multi-hour production soak or an exhaustive third-party window capture matrix
