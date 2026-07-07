Gate: Profile Monitoring Soak Smoke
Completion status: pass
Date/time: 2026-07-07T02:13:55.269Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate monitoring-soak; exit 0
Release build-info hash: executableSha256=426be3c7cda81186bf4b04381e04c7a850b9ad13cab494af7065e7296205b911; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-101319\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-101319\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, ran profile monitoring for 30000ms, sampled 16 UI states, observed tick delta 56, hit delta 56, progress-log delta 28, and stopped cleanly with the button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-101319-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-101319-app.log; docs\manual-gate-evidence\logs\profile-monitoring-soak-prepared-20260707-101319.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-mid-20260707-101319.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-running-20260707-101319.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves a sustained packaged WebView2 monitoring run on this Windows interactive desktop with a generated stable window source; it is still not a multi-hour production soak or an exhaustive third-party window capture matrix
