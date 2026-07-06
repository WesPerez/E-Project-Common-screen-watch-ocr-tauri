Gate: Profile Clipboard Paste Smoke
Completion status: pass
Date/time: 2026-07-06T22:44:16.941Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate clipboard; exit 0
Release build-info hash: executableSha256=f50203ee5348db4ee7c41ce0337d0554ede411f4effa0a3e438e65d4677fc372; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-064410\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-064410\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke saved the user's clipboard object, pasted a generated bitmap through the visible paste-images button, pasted a generated image file list through Ctrl+V, verified each paste created a selected template card with rendered thumbnail geometry, and restored the saved clipboard object before exit
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-064410-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-064410-app.log; docs\manual-gate-evidence\logs\profile-clipboard-image-paste-20260707-064410.png; docs\manual-gate-evidence\logs\profile-clipboard-file-paste-20260707-064410.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves CF_DIB bitmap paste and CF_HDROP image-file paste on this Windows interactive desktop; it does not exhaustively prove every clipboard producer application or every image codec
