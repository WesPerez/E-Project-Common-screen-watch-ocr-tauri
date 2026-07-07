Gate: Profile Clipboard Paste Smoke
Completion status: pass
Date/time: 2026-07-07T13:25:55.756Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate clipboard; exit 0
Release build-info hash: actualExeSha256=8986f1168578cf6b564229e3d80c12dc1e8809138b0786b38c8dd99b46e3bf9a; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=8986f1168578cf6b564229e3d80c12dc1e8809138b0786b38c8dd99b46e3bf9a; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-212548\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-212548\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke saved the user's clipboard object, pasted a generated bitmap through the visible paste-images button, pasted a generated image file list through Ctrl+V, verified each paste created a selected template card with rendered thumbnail geometry, and restored the saved clipboard object before exit
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-212548-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-212548-app.log; docs\manual-gate-evidence\logs\profile-clipboard-image-paste-20260707-212548.png; docs\manual-gate-evidence\logs\profile-clipboard-file-paste-20260707-212548.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves CF_DIB bitmap paste and CF_HDROP image-file paste on this Windows interactive desktop; it does not exhaustively prove every clipboard producer application or every image codec
