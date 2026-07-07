Gate: Production Template Performance Smoke
Completion status: pass
Date/time: 2026-07-07T11:37:20+08:00
Machine: DESKTOP-9FRQ8VV
Worktree note: Tauri repo present; the gate read the shared production ScreenWatchOCR profile/templates but did not modify the Python baseline repo or start/stop old Python GUI processes.
Command(s) and exit code(s): npm run template:parity exited 0; npm run production:template:smoke exited 0 after wrapping scripts\production-template-performance-smoke.ps1.
Release build-info hash: n/a for this detector benchmark gate because no packaged exe was launched; current lite single-file exe is recorded separately as 3587072 bytes with SHA-256 6363339BD12B57FAB97204785C314C62E52C6DECF73823187BBA723FCFD96BAB.
Model/image/evidence dirs: no OCR models used; read-only production profile/templates from C:\Users\Wes\AppData\Local\ScreenWatchOCR; evidence logs stored under docs\manual-gate-evidence\logs
Observed result: current production smoke recorded Python/OpenCV 46ms flat for 8/8 matches and 45ms textured with the known 4/8 odd-phase baseline miss. Rust release recorded 76ms flat for 8/8 matches and 452ms textured for 8/8 matches. Production profile profile_1.json used 18 enabled real template targets on a 2560x1440 frame with threshold 0.90, scales 1.0, template_workers 2, and Rust matched 18/18 in 6445ms.
Evidence files: current command output retained in the Codex thread; historical pass log docs\manual-gate-evidence\logs\production-template-performance-smoke-20260706-220350.log remains retained; wrapper troubleshooting logs docs\manual-gate-evidence\logs\production-template-performance-smoke-20260706-220200.log and docs\manual-gate-evidence\logs\production-template-performance-smoke-20260706-220251.log remain retained only as troubleshooting context.
Cleanup performed: the scripts restored temporary SCREENWATCH_PRODUCTION_* environment variables, removed the temporary Python parity script, and did not write to the shared ScreenWatchOCR data directory.
Remaining risk: this gate verifies detector performance against real shared profile/template files with synthetic placement; live WebView workflow, tray interaction, installer repeatability, and real OCR model smoke remain separate manual gates.
