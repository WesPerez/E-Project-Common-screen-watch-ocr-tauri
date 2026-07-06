Gate: Profile Monitoring Restart Smoke
Completion status: pass
Date/time: 2026-07-06T21:04:25.069Z
Machine: DESKTOP-9FRQ8VV
Worktree note: tracked Tauri source was at commit 70b39aa; Python baseline repo was not modified by this gate
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate all; exit 0
Release build-info hash: executableSha256=68b7f9f8b706e0cc1927de1059b09b44159d9a2d428f6bf9eefd94135dbfaec6; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-050341\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-050341\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, started profile monitoring, observed ticking progress log rows and positive hit counts, stopped monitoring through the main run button, started monitoring again, observed a second running session, and stopped cleanly with the button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-050341-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-050341-app.log; docs\manual-gate-evidence\logs\profile-monitoring-prepared-20260707-050341.png; docs\manual-gate-evidence\logs\profile-monitoring-running-20260707-050341.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves start/stop/restart and progress logging on this Windows interactive desktop with a generated stable window source; it does not prove every third-party window capture implementation or every long-running production workload
