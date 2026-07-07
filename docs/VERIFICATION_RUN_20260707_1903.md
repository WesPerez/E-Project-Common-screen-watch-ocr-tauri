# Verification Run 2026-07-07 19:03 +08:00

This record refreshes the current delivered single-file app identity after the
evidence-directory frontend hardening and final lite rebuild.

- Delivered exe: `release-single\ScreenWatchOCRTauri.exe`
- Size: `3587584` bytes
- SHA-256: `98F47746B032B9F5326083AB2C4BF6BB8F1567583F5FCC71757F56E121A0C7C0`
- Build flavor: lite, OCR models external
- Build-info: `target\release\screen-watch-ocr-tauri.build-info.json`
- Python packaged baseline: `E:\Project\Common\screen-watch-ocr\dist\ScreenWatchOCR.exe`
- Python packaged SHA-256: `A5689E32BD7696381DB5A9186977C377DE1BFF7D5A6F1A7F3C22D35C8B240200`

## Fresh Commands

| Command | Result |
| --- | --- |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe` | Passed against SHA-256 `98F47746...`. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-183233-result.json`. Covered legacy Python profile restore, source preview, template gallery, clipboard bitmap/file paste, one-shot scan, OCR-lite rejection, monitoring start/stop/restart, and layout splitter drags. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate legacy-late-window` | Passed against SHA-256 `98F47746...`. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-183408-result.json`. Covered remembered app-window source appearing after Tauri had already loaded the legacy profile. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate monitoring-soak --soak-ms 600000` | Passed against SHA-256 `98F47746...`. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-184743-result.json`. Ran 600000ms, sampled 300 UI states, observed tick delta `1109`, hit delta `1109`, progress-log delta `47`, and stopped with the button restored to start. |
| `powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath .\release-single\ScreenWatchOCRTauri.exe -StartupWaitSeconds 18` | One same-hash attempt timed out waiting for the posted `WM_CLOSE` to hide the main window. Immediate unchanged rerun passed: source and staged PE subsystem WindowsGui (2), start-minimized, legacy app_data migration, geometry restore `668x491+13+20` at scale `1.5`, close-to-tray, second-instance wake, and cleanup of only smoke-owned temp roots/processes. |
| `npm run tray:smoke -- -ExePath .\release-single\ScreenWatchOCRTauri.exe` | Passed against SHA-256 `98F47746...`. Tauri PID `23784`; tray hidden window class `tray_icon_app`; Show menu PID `23784`; Exit menu PID `23784`; process exited with code `0`. |
| `npm run coexistence:smoke -- -TauriExePath .\release-single\ScreenWatchOCRTauri.exe` | Safely blocked before launching because the old Python default single-instance port `47627` was already busy. Result: `docs\manual-gate-evidence\logs\coexistence-smoke-20260707-182936-result.json`. No user-owned old Python process was stopped or modified. |

## Current Difference And Risk Audit

| Area | Current conclusion |
| --- | --- |
| Console popup | Current final exe is WindowsGui subsystem `2`, verified by packaged smoke and `singleFileDeliverableContract`. |
| Monitoring freeze/restart concern | Current final exe passed start/stop/restart in the WebView smoke and a 600-second monitoring soak with continued tick/hit/log growth and button restoration. |
| UI and layout | Current final exe passed source preview, gallery, clipboard, scan, OCR-lite boundary, and all splitter-drag smoke gates. |
| Close-to-tray | Current final exe passed packaged close-to-tray and second-instance wake on unchanged rerun, but one same-hash automation attempt timed out while waiting for the posted `WM_CLOSE` hide observation. This remains a residual flake/risk to watch with repeated loops or manual close trials. |
| Tray identity | Current final exe tray smoke proved the native menu belonged to the Tauri PID and used the Tauri hidden tray host window class, not the old Python tray class. |
| Old/new coexistence | Earlier full coexistence proof remains valid for the separated process/protocol design. A fresh current-hash default-port coexistence rerun is pending because an existing old Python app already owned port `47627`; the safety guard correctly refused to touch it. |
| Single-file deployment boundary | The lite exe stays small and keeps OCR models external. It is proven on this WebView2-present Windows machine; machines without WebView2 still need the installer or WebView2 installed first. |

## Cleanup Notes

The smoke commands stopped only their own launched processes and removed only
their own temporary app roots/app-data directories. The WebView visual smoke
retained ignored evidence logs/screenshots and ignored `target\webview-visual-smoke`
run directories for audit. The failed coexistence attempt created
`docs\manual-gate-evidence\logs\coexistence-smoke-20260707-182936-result.json`
and did not remove its temp root because it aborted before owning any app
processes; it was not cleaned here because cleanup ownership is not fully
established from the final state alone.
