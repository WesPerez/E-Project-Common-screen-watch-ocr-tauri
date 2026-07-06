Gate: Production Template Performance Smoke
Completion status: pass
Date/time: 2026-07-06T22:04:40.6709204+08:00
Machine: DESKTOP-9FRQ8VV
Worktree note: screen-watch-ocr-tauri is not a git repository; Python baseline repo has unrelated pre-existing changes and was not modified by this gate
Command(s) and exit code(s): npm run production:template:smoke exited 0 after wrapping scripts\production-template-performance-smoke.ps1; earlier wrapper attempts exited 1 before benchmark execution because of PowerShell argument bugs that were fixed
Release build-info hash: n/a for this detector benchmark gate because no packaged exe was launched; latest available lite build-info sidecar records executableSha256 b8e21cff363157332e607a7dc523ba6b5d461a75e64aaae8d012c3bdbe0c11d0
Model/image/evidence dirs: no OCR models used; read-only production profile/templates from C:\Users\Wes\AppData\Local\ScreenWatchOCR; evidence logs stored under docs\manual-gate-evidence\logs
Observed result: fixed flat parity recorded Python/OpenCV 89ms and Rust 89ms for 8/8 matches; fixed textured parity recorded Python/OpenCV 80ms with the known 4/8 odd-phase baseline miss while Rust recorded 570ms and 8/8 matches; production profile profile_1.json used 18 enabled real template targets on a 2560x1440 frame with threshold 0.90, scales 1.0, template_workers 2, and Rust matched 18/18 in 8579ms after bounding coarse-refine search margins
Evidence files: pass log docs\manual-gate-evidence\logs\production-template-performance-smoke-20260706-220350.log; retained wrapper troubleshooting logs docs\manual-gate-evidence\logs\production-template-performance-smoke-20260706-220200.log and docs\manual-gate-evidence\logs\production-template-performance-smoke-20260706-220251.log
Cleanup performed: no cleanup performed; script restored temporary SCREENWATCH_PRODUCTION_* environment variables and did not write to the shared ScreenWatchOCR data directory
Remaining risk: this gate verifies detector performance against real shared profile/template files with synthetic placement; live WebView workflow, tray interaction, installer repeatability, and real OCR model smoke remain separate manual gates
