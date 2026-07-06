Gate: Template Gallery Visual Workflow Smoke
Completion status: pass
Date/time: 2026-07-06T15:00:23.675Z
Machine: DESKTOP-9FRQ8VV
Worktree note: screen-watch-ocr-tauri is not a git repository
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate gallery; exit 0
Release build-info hash: executableSha256=512d0e5b9410acd5a91921c86a8a522ce807b3ea68ef30bb2838468c4f6d5fa1; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json
Model/image/evidence dirs: inputDir=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260706-230009\inputs; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260706-230009\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke imported four generated PNG templates into an isolated profile, preserved thumbnail geometry and selection, toggled target enablement, exercised select-all/invert, used row-button reorder, exercised drag/drop reorder, opened the hit-count context menu and cleared hits, deleted one target, cleared all targets, and captured the current source as a new template
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260706-230009-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260706-230009-app.log; docs\manual-gate-evidence\logs\template-gallery-imported-20260706-230009.png; docs\manual-gate-evidence\logs\template-gallery-reordered-20260706-230009.png; docs\manual-gate-evidence\logs\template-gallery-context-menu-20260706-230009.png; docs\manual-gate-evidence\logs\template-gallery-cleared-20260706-230009.png; docs\manual-gate-evidence\logs\template-gallery-captured-source-20260706-230009.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the current packaged WebView2/gallery workflow against isolated generated images and a real screen capture source; it does not prove every user image codec or long-running manual editing session
