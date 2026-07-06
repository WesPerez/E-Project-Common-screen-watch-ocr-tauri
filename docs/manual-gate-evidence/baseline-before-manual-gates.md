Gate: Baseline Before Manual Gates
Completion status: pass
Date/time: 2026-07-07T06:56:00+08:00
Machine: DESKTOP-9FRQ8VV
Worktree note: Python baseline repo was tested directly with a temporary process environment (`PYTHONPATH=src`); no old Python GUI process was started or stopped.
Command(s) and exit code(s): in `E:\Project\Common\screen-watch-ocr`, `$env:PYTHONPATH='src'; python -m unittest -v` exited 0; `$env:PYTHONPATH='src'; python -m screen_watch app --smoke-test` exited 0.
Release build-info hash: not applicable to the Python baseline rerun; current Tauri single-file exe is recorded separately in packaged/WebView evidence.
Model/image/evidence dirs: no OCR models used; Python unittest created only temporary self-test output under the user temp directory and the retained evidence log under docs\manual-gate-evidence\logs.
Observed result: Python baseline rerun passed 98 tests. The app smoke-test returned `{"ok": true, "monitors": 3}`, proving current baseline monitor enumeration on this desktop.
Evidence files: docs\manual-gate-evidence\logs\python-baseline-current-20260707-0656.log; older verifier baseline logs are retained in docs\manual-gate-evidence\logs as historical context.
Cleanup performed: command exited naturally; no owned Python GUI, verifier, cargo, rustup, or Tauri test process remained. No old Python ScreenWatchOCR.exe process was stopped or modified.
Remaining risk: this gate proves the current Python unit-test baseline and monitor smoke only; it does not prove real OCR model inference, full WebView visual workflows, or real tray menu clicks.
