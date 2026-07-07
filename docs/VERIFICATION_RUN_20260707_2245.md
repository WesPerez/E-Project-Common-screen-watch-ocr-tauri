# Verification Run 2026-07-07 22:45 +08:00

## Final Single Exe

- Path: `release-single\ScreenWatchOCRTauri.exe`
- Size: `3591168` bytes
- SHA-256: `1D03E007175F987B95E2523C16E611F27CD71A86C10EED9AAB0AC24EA5D189FE`
- Build flavor: lite
- Boundary: runs on Windows machines with Microsoft Edge WebView2 Runtime present; OCR models remain external.

## Commands

| Command | Result |
| --- | --- |
| `npm run tauri:build:lite` | Passed. Rebuilt `target\release\screen-watch-ocr-tauri.exe` and NSIS bundle as lite. Build-info recorded 3,591,168 bytes and SHA-256 `1d03e007175f987b95e2523c16e611f27cd71a86c10eed9aab0ac24ea5d189fe`. |
| Copy `target\release\screen-watch-ocr-tauri.exe` to `release-single\ScreenWatchOCRTauri.exe` | Passed. Source and destination hashes matched. |
| `powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath .\release-single\ScreenWatchOCRTauri.exe -StartupWaitSeconds 18 -CloseDelayMilliseconds 750` | Passed. Verified source/staged PE subsystem WindowsGui (2), legacy app_data migration, start-minimized, legacy geometry restore, close-to-tray, and second-instance wake; smoke-owned temp roots were removed. |
| `powershell -ExecutionPolicy Bypass -File scripts\tray-menu-smoke.ps1 -ExePath .\release-single\ScreenWatchOCRTauri.exe` | Passed. Verified Tauri-owned tray host class `tray_icon_app`, native menu commands `Show Tauri` and `Exit Tauri`, and exit code 0. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe` | Passed. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-224646-result.json`. Covered legacy profile restore, source preview, template gallery, clipboard image/file paste, one-shot scan, OCR-lite rejection, monitoring restart, and layout splitters. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate legacy-late-window` | Passed. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-224804-result.json`. |
| `cargo test -p screen-watch-core` | Passed earlier in this continuation with `125 passed, 3 ignored`; includes `exact_gray_template_detection_uses_rarest_anchor_and_checks_all_columns`. |
| `npm run production:template:parity` | Passed earlier in this continuation. Old Python Detector matched 18/18 in 421ms; Rust matched the same 18/18 ids in 66ms. Result: `docs\manual-gate-evidence\logs\production-template-parity-smoke-20260707-221918-result.json`. |
| `npm run production:template:smoke -- -SkipParity` | Passed earlier in this continuation. Rust matched 18/18 production templates in 103ms. |
| `npm run template:parity` | Passed earlier in this continuation. Rust release matched 8/8 flat templates in 33ms and 8/8 textured templates in 27ms. |
| `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease` | Passed after the final exe rebuild and hash contract update with Rust core `125 passed, 3 ignored`, Tauri `92 passed, 16 ignored`, OCR feature `28 passed`, dependency-tree contracts, `singleFileDeliverableContract: 3591168 bytes, 1D03E007175F987B95E2523C16E611F27CD71A86C10EED9AAB0AC24EA5D189FE, WindowsGui`, and `liteSizeGate: passed`. |

## Cleanup

- The packaged smoke removed its smoke-owned temp app root and isolated `LOCALAPPDATA`.
- The tray smoke removed its smoke-owned isolated app-data directory.
- WebView visual smoke retained its `target\webview-visual-smoke\20260707-224646` and `20260707-224804` run directories plus evidence logs/screenshots for audit.
- Earlier failed startup shortcut tests left two temp `screen-watch-ocr-tauri-startup-*` directories; those exact current-session directories were inspected and removed.
- Existing ignored build/dependency/evidence directories (`dist/`, `target/`, `release-single/`, `node_modules/`, `docs/manual-gate-evidence/logs/`) are retained.

## Notes

- The old Python `ScreenWatchOCR.exe` process visible during the final checks was not started or stopped by this run.
- The previous `8986F116...` final exe remains useful historical evidence for the 600-second monitoring soak and coexistence smoke. The current `1D03E007...` final exe has fresh packaged, tray, full WebView, late-window, template parity, and verifier evidence.
