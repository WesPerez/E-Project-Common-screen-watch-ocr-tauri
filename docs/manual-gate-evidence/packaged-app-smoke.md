Gate: Packaged App Smoke
Completion status: pass
Date/time: 2026-07-07T05:05:10+08:00
Machine: DESKTOP-9FRQ8VV
Worktree note: tracked Tauri source was at commit 70b39aa; Python baseline repo was not modified by this gate
Command(s) and exit code(s): npm run packaged:smoke exited 0 after wrapping scripts\packaged-smoke.ps1 against sourceExePath E:\Project\Common\screen-watch-ocr-tauri\target\release\screen-watch-ocr-tauri.exe
Release build-info hash: target\release\screen-watch-ocr-tauri.build-info.json reports flavor lite, exe 3582976 bytes, sha256 68b7f9f8b706e0cc1927de1059b09b44159d9a2d428f6bf9eefd94135dbfaec6; release-single\ScreenWatchOCRTauri.exe SHA-256 68B7F9F8B706E0CC1927DE1059B09B44159D9A2D428F6BF9EEFD94135DBFAEC6
Model/image/evidence dirs: no OCR models used; smoke staged a copied packaged exe under C:\Users\Wes\AppData\Local\Temp\screen-watch-ocr-tauri-packaged-smoke-app-5f3e7fa0 and isolated LOCALAPPDATA under C:\Users\Wes\AppData\Local\Temp\screen-watch-ocr-tauri-packaged-smoke-5f3e7fa0, then removed both
Observed result: packaged smoke copied 5 legacy app_data files into the isolated shared ScreenWatchOCR data dir, verified start-minimized kept the app running without a visible main window, restored migrated legacy geometry at 668x491+13+20 with DPI probe scale 1.5, closed the visible main window to tray while keeping the process alive, launched a second instance on the isolated port, observed the second instance exit code 0, and verified the first instance main window woke back to visible state
Evidence files: command output retained in the Codex thread; repeatable script scripts\packaged-smoke.ps1; source exe target\release\screen-watch-ocr-tauri.exe; final single exe release-single\ScreenWatchOCRTauri.exe
Cleanup performed: stopped only the smoke-owned start-minimized and close-to-tray Tauri processes, observed the second instance already exited, removed only the smoke-created temp app root and isolated LOCALAPPDATA, and did not stop or modify old Python ScreenWatchOCR.exe processes, shared production ScreenWatchOCR data, configs, credentials, OCR model directories, or unrelated tray icons
Remaining risk: proves packaged startup/migration/geometry/close-to-tray/single-instance wake on this Windows desktop with isolated data; it does not prove every DPI topology, every user startup shortcut state, or long-running tray residency
