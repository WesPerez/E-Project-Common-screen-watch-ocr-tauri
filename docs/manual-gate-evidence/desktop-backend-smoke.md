Gate: Desktop Backend Smoke
Completion status: pass
Date/time: 2026-07-07T15:06:09+08:00
Machine: DESKTOP-9FRQ8VV
Worktree note: current git repository clean except ignored build/dependency/evidence directories; old Python process state was not modified.
Command(s) and exit code(s): powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke exited 0.
Release build-info hash: final single exe 6363339BD12B57FAB97204785C314C62E52C6DECF73823187BBA723FCFD96BAB, 3587072 bytes, WindowsGui subsystem verified by singleFileDeliverableContract.
Model/image/evidence dirs: no OCR models used; desktop smoke evidence log stored under docs\manual-gate-evidence\logs.
Observed result: current verifier rerun reported Rust core 121 passed / 3 ignored, Tauri 88 passed / 16 ignored, OCR feature 25 passed, desktopSmoke: 16 gates, and singleFileDeliverableContract: 3587072 bytes / 6363339BD12B57FAB97204785C314C62E52C6DECF73823187BBA723FCFD96BAB / WindowsGui for the final single exe. The latest captured full verifier rerun separately reported Python baseline 98 locked tests, frontend 103 passed, releaseBuild: True, and the same singleFileDeliverableContract. The desktop gates covered real screen capture, monitor listing, one-shot screen/window scan, profile screen/window workflows, screen/window/remembered-window screenshot-as-template capture, persistent screen/window monitoring, app-window enumeration/capture, and real DWM thumbnail register/update/clear tests.
Evidence files: current command output retained in the Codex thread; earlier pass log docs\manual-gate-evidence\logs\desktop-backend-smoke-20260706-205937.log remains historical evidence. Earlier incomplete log docs\manual-gate-evidence\logs\desktop-backend-smoke-20260706-205554.log was produced by a deadlocked wrapper and is retained only as troubleshooting context, not pass evidence.
Cleanup performed: current run exited naturally with no owned test process remaining; no old Python/Tauri application process was stopped.
Remaining risk: this gate proves backend desktop APIs and DWM smoke paths on this Windows desktop, but it does not prove every third-party window class, multi-hour production soak, broad OCR model accuracy, or actual speaker audio output.
