Gate: WebView Source Preview Visual Smoke
Completion status: pass
Date/time: 2026-07-06T19:48:26.622Z
Machine: DESKTOP-9FRQ8VV
Worktree note: screen-watch-ocr-tauri is not a git repository
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate source; exit 0
Release build-info hash: executableSha256=2934248886303006f834a5ba3310261cfd5d9ea37ffb64e14063fdeb4397d66f; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-034819\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-034819\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke selected a physical screen and visible helper app-window source, refreshed source cards without unexpected failed previews, captured bitmap/DWM-backed cards, scrolled a preview card partially offscreen, resized the app window, restored the preview area, and refreshed again without stale/error cards
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-034819-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-034819-app.log; docs\manual-gate-evidence\logs\webview-source-preview-initial-20260707-034819.png; docs\manual-gate-evidence\logs\webview-source-preview-partial-scroll-20260707-034819.png; docs\manual-gate-evidence\logs\webview-source-preview-restored-20260707-034819.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2/runtime path on this interactive desktop; it does not prove every possible monitor topology or every third-party app window
