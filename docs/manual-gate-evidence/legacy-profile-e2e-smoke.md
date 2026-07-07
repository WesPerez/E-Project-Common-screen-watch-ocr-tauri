Gate: Legacy Profile End-to-End Smoke
Completion status: pass
Date/time: 2026-07-07T01:53:19.654Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate legacy-profile; exit 0
Release build-info hash: executableSha256=f2b0c5f2ee9716c491f6174c7b381e7b277f05fdc09b51449921243d96f89245; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-095312\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-095312\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke staged a Python-shaped profile_1.json with an existing template, no selected monitors, remembered app-window source, legacy match settings, and unknown fields; launched the packaged Tauri app without changing configuration; verified the visible UI restored the old profile, clicked scan once, observed a positive window-source hit plus alerts.jsonl/screenshot/profile hit_count evidence, then started and stopped monitoring with continued tick/hit progress
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-095312-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-095312-app.log; docs\manual-gate-evidence\logs\legacy-profile-loaded-20260707-095312.png; docs\manual-gate-evidence\logs\legacy-profile-scan-hit-20260707-095312.png; docs\manual-gate-evidence\logs\legacy-profile-monitoring-20260707-095312.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves old Python-shaped profile data works when the remembered app window is present at Tauri startup; a separate late-start remembered-app gate is still needed for apps launched after Tauri has already loaded the profile
