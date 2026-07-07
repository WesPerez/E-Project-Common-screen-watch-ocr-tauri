Gate: Profile Monitoring Soak Smoke
Completion status: pass
Date/time: 2026-07-07T10:57:49.311Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate monitoring-soak --soak-ms 600000; exit 0
Release build-info hash: actualExeSha256=98f47746b032b9f5326083ab2c4bf6bb8f1567583f5fcc71757f56e121a0c7c0; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=98f47746b032b9f5326083ab2c4bf6bb8f1567583f5fcc71757f56e121a0c7c0; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: generated input fixture directory was not retained for this focused gate; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-184743\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, ran profile monitoring for 600000ms, sampled 300 UI states, observed tick delta 1109, hit delta 1109, progress-log delta 47, and stopped cleanly with the button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-184743-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-184743-app.log; docs\manual-gate-evidence\logs\profile-monitoring-soak-prepared-20260707-184743.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-mid-20260707-184743.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-running-20260707-184743.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves a sustained packaged WebView2 monitoring run on this Windows interactive desktop with a generated stable window source; it is still not a multi-hour production soak or an exhaustive third-party window capture matrix
