Gate: Profile Monitoring Soak Smoke
Completion status: pass
Date/time: 2026-07-06T21:42:14.772Z
Machine: DESKTOP-9FRQ8VV
Worktree note: screen-watch-ocr-tauri is not a git repository
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate monitoring-soak; exit 0
Release build-info hash: executableSha256=b67d3ef1329d753451db661c2c0a06240e1b32459a4724b68ca9466bd2b47bf8; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-054109\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-054109\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, ran profile monitoring for 60000ms, sampled 31 UI states, observed tick delta 112, hit delta 112, progress-log delta 47, and stopped cleanly with the button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-054109-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-054109-app.log; docs\manual-gate-evidence\logs\profile-monitoring-soak-prepared-20260707-054109.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-mid-20260707-054109.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-running-20260707-054109.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves a sustained packaged WebView2 monitoring run on this Windows interactive desktop with a generated stable window source; it is still not a multi-hour production soak or an exhaustive third-party window capture matrix
