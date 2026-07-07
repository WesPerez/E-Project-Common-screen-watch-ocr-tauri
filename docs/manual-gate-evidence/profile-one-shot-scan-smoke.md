Gate: Profile One Shot Scan Smoke
Completion status: pass
Date/time: 2026-07-07T03:32:59.997Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe; exit 0
Release build-info hash: actualExeSha256=426be3c7cda81186bf4b04381e04c7a850b9ad13cab494af7065e7296205b911; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=180cf7e8e38da145d730b46141f594cbcc7d23f13c8be26add3853ada4003ab9; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-113130\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-113130\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, clicked the visible profile scan-once button, observed a positive hit count and log row in the UI, verified the target hit badge/profile hit_count updated, and confirmed alerts.jsonl plus screenshot evidence were written
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-113130-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-113130-app.log; docs\manual-gate-evidence\logs\profile-one-shot-scan-prepared-20260707-113130.png; docs\manual-gate-evidence\logs\profile-one-shot-scan-hit-20260707-113130.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the packaged visible one-shot scan path on this Windows interactive desktop with a generated stable window source; it does not prove every third-party window capture implementation or every production image corpus
