Gate: Profile Monitoring Soak Smoke
Completion status: pass
Date/time: 2026-07-07T08:28:01.203Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate monitoring-soak --soak-ms 600000; exit 0
Release build-info hash: actualExeSha256=200c0c8e8efb8af4a2dd56a37c9762c2582c45db441555e669a114af5d1737b2; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=200c0c8e8efb8af4a2dd56a37c9762c2582c45db441555e669a114af5d1737b2; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-161730\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs; the generated input fixture directory was not retained for this focused gate
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, ran profile monitoring for 600000ms, sampled 300 UI states, observed tick delta 2149, hit delta 537, progress-log delta 46, and stopped cleanly with the button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-161730-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-161730-app.log; docs\manual-gate-evidence\logs\profile-monitoring-soak-prepared-20260707-161730.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-mid-20260707-161730.png; docs\manual-gate-evidence\logs\profile-monitoring-soak-running-20260707-161730.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves a sustained packaged WebView2 monitoring run on this Windows interactive desktop with a generated stable window source; it is still not a multi-hour production soak or an exhaustive third-party window capture matrix
