Gate: Desktop Backend Smoke
Completion status: pass
Date/time: 2026-07-07T09:13:37.2697778+08:00
Machine: DESKTOP-9FRQ8VV
Worktree note: current git repository clean except ignored build/dependency/evidence directories; old Python process state was not modified.
Command(s) and exit code(s): powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeDesktopSmoke -IncludePackagedSmoke -PackagedSmokeStartupWaitSeconds 8 exited 0.
Release build-info hash: final single exe DB695C7D6382B6F33D6862945380F75BDB69BD6DE4E1B61A1F37D55B91BF10CA, 3587072 bytes, WindowsGui subsystem verified by singleFileDeliverableContract.
Model/image/evidence dirs: no OCR models used; desktop smoke evidence log stored under docs\manual-gate-evidence\logs.
Observed result: verifier summary reported Python baseline 98 locked tests, Rust core 121 passed / 3 ignored, Tauri 85 passed / 16 ignored, OCR feature 25 passed, frontend 102 passed, desktopSmoke: 16 gates, packagedSmoke: ran, and singleFileDeliverableContract: 3587072 bytes / DB695C7D6382B6F33D6862945380F75BDB69BD6DE4E1B61A1F37D55B91BF10CA / WindowsGui. The desktop gates covered real screen capture, monitor listing, one-shot screen/window scan, profile screen/window workflows, screen/window/remembered-window screenshot-as-template capture, persistent screen/window monitoring, app-window enumeration/capture, and real DWM thumbnail register/update/clear tests.
Evidence files: current command output retained in the Codex thread; earlier pass log docs\manual-gate-evidence\logs\desktop-backend-smoke-20260706-205937.log remains historical evidence. Earlier incomplete log docs\manual-gate-evidence\logs\desktop-backend-smoke-20260706-205554.log was produced by a deadlocked wrapper and is retained only as troubleshooting context, not pass evidence.
Cleanup performed: current run exited naturally with no owned test process remaining; no old Python/Tauri application process was stopped.
Remaining risk: this gate proves backend desktop APIs and DWM smoke paths on this Windows desktop, but it does not prove every third-party window class, multi-hour production soak, broad OCR model accuracy, or actual speaker audio output.
