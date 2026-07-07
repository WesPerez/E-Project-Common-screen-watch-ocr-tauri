# Verification Run 2026-07-07 16:54 +08:00

This record summarizes the continuation audit performed after commit
`9aa8501` against the current source tree and the delivered single-file app:

- Delivered exe: `release-single\ScreenWatchOCRTauri.exe`
- Size: `3587584` bytes
- SHA-256: `200C0C8E8EFB8AF4A2DD56A37C9762C2582C45DB441555E669A114AF5D1737B2`
- Build flavor: lite, OCR models external
- Python packaged baseline: `E:\Project\Common\screen-watch-ocr\dist\ScreenWatchOCR.exe`
- Python packaged SHA-256: `A5689E32BD7696381DB5A9186977C377DE1BFF7D5A6F1A7F3C22D35C8B240200`

## Fresh Commands

| Command | Result |
| --- | --- |
| `npm run verify:migration` | Passed. Python inventory `98`, Python unittest `98`, Rust core `124 passed, 3 ignored`, Tauri `92 passed, 16 ignored`, OCR feature `28 passed`, frontend `103 passed`, frontend production build `True`, Tauri lite release build `True`, `singleFileDeliverableContract: 3587584 bytes, 200C0C8E..., WindowsGui`, `liteSizeGate: passed`. |
| `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke -IncludeTemplateBenchmark -IncludePackagedSmoke` | Passed. Desktop smoke `16 gates`; template benchmark `73ms` flat `8/8` and `438ms` textured `8/8`; packaged smoke passed WindowsGui, start-minimized, legacy app_data migration, geometry restore, close-to-tray, and second-instance wake against the rebuilt release exe. |
| `npm run python:profile:compat` | Passed. Old Python source app loaded required Tauri-shaped shared profile/state/template fields from an isolated `ScreenWatchOCR` data root and saved back the required Python-compatible shape. Result: `docs\manual-gate-evidence\logs\python-profile-compat-smoke-20260707-164743-result.json`. |
| `npm run template:parity` | Passed. Python flat `46ms` `8/8`; Python textured `41ms` with the known baseline `4/8`; Rust flat `68ms` `8/8`; Rust textured `444ms` `8/8`. |
| `npm run production:template:smoke` | Passed. Real shared `profile_1.json` production template set: `18/18` enabled templates matched on a 2560x1440 synthetic placement frame in `6767ms`, threshold `0.90`, scales `1.0`, template workers `2`. |
| `npm run ocr:text:parity` | Passed. Old Python supplied-row OCR matching semantics still align with Rust OCR text detection and ScanEngine OCR backend tests. Result: `docs\manual-gate-evidence\logs\ocr-text-parity-smoke-20260707-164825-result.json`. |
| `npm run ocr:corpus:smoke` | Passed. External PP-OCRv5-style models recognized generated English and Chinese corpus cases: READY, ALERT 42, OCR TEST, SCAN COMPLETE, ERROR 100%, 准备好了, 开始监控, 屏幕监控, 发现异常. Result: `docs\manual-gate-evidence\logs\ocr-corpus-smoke-20260707-164827-result.json`. |
| `npm run tray:smoke -- -ExePath .\release-single\ScreenWatchOCRTauri.exe` | Passed against the final single exe. Tauri PID `56444`; tray hidden window class `tray_icon_app`; Show menu PID `56444`; Exit menu PID `56444`; process exited with code `0`. |
| `npm run coexistence:smoke -- -TauriExePath .\release-single\ScreenWatchOCRTauri.exe` | Passed against old packaged Python plus final Tauri single exe. Python process name `ScreenWatchOCR`, Tauri process name `ScreenWatchOCRTauri`; Python port `47627`, Tauri port `47628`; cross-protocol commands rejected both ways; own commands accepted; both second instances exited `0`; Tauri WebView2 children used the smoke-owned user data folder. Result: `docs\manual-gate-evidence\logs\coexistence-smoke-20260707-165344-result.json`. |
| `npm run evidence:references` | Passed after correcting stale historical evidence wording. All parsed current local references to `docs\manual-gate-evidence\logs`, `target`, and `release-single\ScreenWatchOCRTauri.exe` exist, excluding intentionally historical `target\installer-smoke*` install roots. This check is now part of `scripts\verify-migration.ps1` as `evidenceReferenceContract`. |
| `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease` after contract hardening | Passed after locking the key smoke npm scripts into `packageScriptContract` and adding the packaged Python/Tauri coexistence smoke to the manual-gate runbook contract. The summary includes `packageScriptContract: passed`, `manualGateRunbookContract: passed`, and `evidenceReferenceContract: passed`. |
| `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease` after UI surface contract hardening | Passed. Summary includes Rust core `124 passed, 3 ignored`, Tauri `92 passed, 16 ignored`, OCR feature `28 passed`, `legacyVisibleWorkflowContract: passed`, new `legacyUiSurfaceContract: passed`, `singleFileDeliverableContract: 3587584 bytes, 200C0C8E..., WindowsGui`, and UTF-8 PowerShell source reading fixed for non-ASCII contracts. |
| `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease` after default settings contract hardening | Passed. Summary includes Rust core `124 passed, 3 ignored`, Tauri `92 passed, 16 ignored`, OCR feature `28 passed`, `legacyDefaultSettingsContract: passed`, and the same final `singleFileDeliverableContract: 3587584 bytes, 200C0C8E..., WindowsGui`. |
| `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease` after failure guard contract hardening | Passed. Summary includes Rust core `124 passed, 3 ignored`, Tauri `92 passed, 16 ignored`, OCR feature `28 passed`, `legacyFailureGuardContract: passed`, and final `singleFileDeliverableContract: 3587584 bytes, 200C0C8E..., WindowsGui`. |

## Current Difference And Risk Audit

| Area | Current conclusion |
| --- | --- |
| Console popup | Proven absent for the delivered exe by PE subsystem checks requiring WindowsGui subsystem `2`. |
| Old/new process identity | Proven separated by executable name, process name, window title, startup link name, bundle identifier, single-instance command, default port, and tray hidden window class. |
| Shared data compatibility | Proven for current required profile/state/template fields. Boundary remains: old Python can overwrite Tauri writes made after Python has already loaded a profile, so simultaneous old/new active monitoring against the same profile is unsupported. |
| Monitoring freeze/restart concern | Proven for start/stop/restart by packaged WebView smoke, for a 600000ms monitoring soak, and by `legacyFailureGuardContract` covering duplicate-start blocking, failed-start UI recovery, stopped button restoration, and OCR-lite worker rejection before a session starts. Latest recorded soak: tick delta `2149`, hit delta `537`, progress-log delta `46`, stopped with the button restored to start. |
| Template/pixel detection | Proven by core tests, Python/Rust template parity, real production profile smoke, one-shot scan gates, and WebView scan smoke. Rust currently finds all textured benchmark placements where the old Python baseline records the known `4/8` textured miss. |
| GUI defaults and existing-profile fallback | Proven by `legacyDefaultSettingsContract`, which locks old Python GUI initialization/profile-load defaults against Tauri HTML defaults, frontend scan-option build/load fallbacks, and Rust profile defaults for threshold, scales, interval, cooldown, beep, retention, region, and first-monitor selection. |
| UI compactness/resizable panes | Proven by frontend layout tests, WebView splitter drag smoke, and the new legacy UI surface contract covering old profile/startup/gallery/source/region/match/run/status/log/preview labels and equivalent Tauri IDs/layout splitters. Very narrow/mobile-like widths are not exhaustively visually sampled. |
| OCR | Lite app correctly keeps OCR models external and rejects OCR work without bundled models. Full OCR source path has real external PP-OCRv5 English/Chinese smoke and corpus evidence. Broad real screenshot OCR quality, PP-OCRv6 asset compatibility, and RapidOCR-native parity are still not fully proven. |
| WebView2 dependency | The small single exe is proven on this WebView2-present Windows machine. Machines without WebView2 still need the installer or WebView2 installed first. |
| Third-party windows | Real desktop gates cover screen capture, app-window enumeration, window capture, DWM preview, and window monitoring on this machine. Some protected, minimized, GPU-only, or capture-blocking third-party windows can still be OS/window limitations. |

## Cleanup Notes

The commands removed their own temp roots and stopped only smoke-owned
processes. Generated build output, dependencies, smoke logs, screenshots, and
the delivered exe remain in ignored directories:

- `dist/`
- `target/`
- `release-single/`
- `node_modules/`
- `docs/manual-gate-evidence/logs/`
