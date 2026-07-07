Gate: WebView Source Preview Visual Smoke
Completion status: pass
Date/time: 2026-07-07T01:55:48.871Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate source; exit 0
Release build-info hash: executableSha256=f2b0c5f2ee9716c491f6174c7b381e7b277f05fdc09b51449921243d96f89245; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-095540\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-095540\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected a physical screen and visible helper app-window source, refreshed source cards without unexpected failed previews, captured bitmap/DWM-backed cards, scrolled a preview card partially offscreen, resized the app window, restored the preview area, and refreshed again without stale/error cards
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-095540-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-095540-app.log; docs\manual-gate-evidence\logs\webview-source-preview-initial-20260707-095540.png; docs\manual-gate-evidence\logs\webview-source-preview-partial-scroll-20260707-095540.png; docs\manual-gate-evidence\logs\webview-source-preview-restored-20260707-095540.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2/runtime path on this interactive desktop; it does not prove every possible monitor topology or every third-party app window
