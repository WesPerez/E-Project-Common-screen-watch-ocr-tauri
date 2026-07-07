Gate: Legacy Profile End-to-End Smoke
Completion status: pass
Date/time: 2026-07-07T00:30:03.356Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate all; exit 0
Release build-info hash: executableSha256=5f24f3e399ce42b85206de15274ab3e027aae62c2dac3b48accd872c4d35aebc; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-082916\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-082916\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke staged a Python-shaped profile_1.json with an existing template, no selected monitors, remembered app-window source, legacy match settings, and unknown fields; launched the packaged Tauri app without changing configuration; verified the visible UI restored the old profile, clicked scan once, observed a positive window-source hit plus alerts.jsonl/screenshot/profile hit_count evidence, then started and stopped monitoring with continued tick/hit progress
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-082916-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-082916-app.log; docs\manual-gate-evidence\logs\legacy-profile-loaded-20260707-082916.png; docs\manual-gate-evidence\logs\legacy-profile-scan-hit-20260707-082916.png; docs\manual-gate-evidence\logs\legacy-profile-monitoring-20260707-082916.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves old Python-shaped profile data works when the remembered app window is present at Tauri startup; a separate late-start remembered-app gate is still needed for apps launched after Tauri has already loaded the profile
