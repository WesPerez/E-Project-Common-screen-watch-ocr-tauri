Gate: Profile Monitoring Restart Smoke
Completion status: pass
Date/time: 2026-07-07T07:31:01.747Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe; exit 0
Release build-info hash: actualExeSha256=b7356d3a96810aa70fef42ee1fb360516411d145b1e8630f6a49f840c1efe3a4; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=b7356d3a96810aa70fef42ee1fb360516411d145b1e8630f6a49f840c1efe3a4; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-153009\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-153009\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, started profile monitoring, observed ticking progress log rows and positive hit counts, stopped monitoring through the main run button, started monitoring again, observed a second running session, and stopped cleanly with the button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-153009-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-153009-app.log; docs\manual-gate-evidence\logs\profile-monitoring-prepared-20260707-153009.png; docs\manual-gate-evidence\logs\profile-monitoring-running-20260707-153009.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves start/stop/restart and progress logging on this Windows interactive desktop with a generated stable window source; it does not prove every third-party window capture implementation or every long-running production workload
