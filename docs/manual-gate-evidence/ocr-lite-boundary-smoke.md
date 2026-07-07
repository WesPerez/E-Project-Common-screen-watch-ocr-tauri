Gate: OCR Lite Boundary Smoke
Completion status: pass
Date/time: 2026-07-07T13:27:45.755Z
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; smoke used isolated LOCALAPPDATA and did not modify the Python baseline
Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate ocr-lite-boundary; exit 0
Release build-info hash: actualExeSha256=8986f1168578cf6b564229e3d80c12dc1e8809138b0786b38c8dd99b46e3bf9a; exePath=release-single\ScreenWatchOCRTauri.exe; buildInfoExecutableSha256=8986f1168578cf6b564229e3d80c12dc1e8809138b0786b38c8dd99b46e3bf9a; buildInfo=target\release\screen-watch-ocr-tauri.build-info.json; buildInfoMatchesActual=true
Model/image/evidence dirs: generated input fixture directory was not retained for this focused gate; localAppData=E:\Project\Common\screen-watch-ocr-tauri\target\webview-visual-smoke\20260707-212741\localappdata; evidenceLogDir=E:\Project\Common\screen-watch-ocr-tauri\docs\manual-gate-evidence\logs
Observed result: automated real WebView2/CDP smoke filled the hidden raw scan-config textarea with an old-style OCR text target config, clicked the packaged app's raw scan-once and raw monitor-start buttons, observed the explicit OCR backend unavailable error before any scan loop, verified alert evidence counts did not increase, queried monitoring status, and confirmed the session stayed stopped with the visible run button restored to start
Evidence files: docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-212741-result.json; docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-212741-app.log; docs\manual-gate-evidence\logs\ocr-lite-boundary-scan-error-20260707-212741.png; docs\manual-gate-evidence\logs\ocr-lite-boundary-monitor-error-20260707-212741.png
Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit
Remaining risk: proves the delivered lite packaged exe rejects OCR-target configs clearly instead of entering a failing monitor loop; it does not make lite OCR equivalent to the full OCR build
