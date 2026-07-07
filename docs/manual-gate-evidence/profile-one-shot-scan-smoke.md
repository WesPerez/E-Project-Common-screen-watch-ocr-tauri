Gate: Profile One Shot Scan Smoke
Completion status: pass
Date/time: 2026-07-07T01:52:16.668Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate scan; exit 0
Release build-info hash: executableSha256=f2b0c5f2ee9716c491f6174c7b381e7b277f05fdc09b51449921243d96f89245; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-095211\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-095211\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, clicked the visible profile scan-once button, observed a positive hit count and log row in the UI, verified the target hit badge/profile hit_count updated, and confirmed alerts.jsonl plus screenshot evidence were written
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-095211-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-095211-app.log; docs\manual-gate-evidence\logs\profile-one-shot-scan-prepared-20260707-095211.png; docs\manual-gate-evidence\logs\profile-one-shot-scan-hit-20260707-095211.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the packaged visible one-shot scan path on this Windows interactive desktop with a generated stable window source; it does not prove every third-party window capture implementation or every production image corpus
