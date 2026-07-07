Gate: Legacy Late-Start Window End-to-End Smoke
Completion status: pass
Date/time: 2026-07-07T08:17:03.322Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate legacy-late-window; exit 0
Release build-info hash: actualExeSha256=200c0c8e8efb8af4a2dd56a37c9762c2582c45db441555e669a114af5d1737b2; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=200c0c8e8efb8af4a2dd56a37c9762c2582c45db441555e669a114af5d1737b2; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-161653\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs; the generated input fixture directory was not retained for this focused gate
Observed result: automated real WebView2/CDP smoke staged a Python-shaped profile_1.json before launch while the remembered app window was absent; verified scan was still runnable and reported the missing remembered app instead of losing the source; started the remembered app window after Tauri had loaded the profile; then scanned and monitored without reselecting the window, producing positive hit/evidence/profile updates
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-161653-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-161653-app.log; docs\manual-gate-evidence\logs\legacy-late-window-loaded-missing-20260707-161653.png; docs\manual-gate-evidence\logs\legacy-late-window-scan-hit-20260707-161653.png; docs\manual-gate-evidence\logs\legacy-late-window-monitoring-20260707-161653.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves late-start recovery for one generated remembered app window on this Windows desktop; broad third-party window capture behavior still depends on OS/window-class support
