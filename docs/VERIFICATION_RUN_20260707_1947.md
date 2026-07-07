# Verification Run 2026-07-07 19:47 +08:00

This record refreshes the delivered single-file app after hardening the
close-to-tray path that intermittently timed out under repeated packaged smoke.

- Delivered exe: `release-single\ScreenWatchOCRTauri.exe`
- Size: `3587584` bytes
- SHA-256: `8986F1168578CF6B564229E3D80C12DC1E8809138B0786B38C8DD99B46E3BF9A`
- Build flavor: lite, OCR models external
- Build-info: `target\release\screen-watch-ocr-tauri.build-info.json`

## Fresh Commands

| Command | Result |
| --- | --- |
| `powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath .\release-single\ScreenWatchOCRTauri.exe -StartupWaitSeconds 18` loop, 10 iterations | Passed 10/10 after close-to-tray hardening and settled-startup close timing. Log: `docs\manual-gate-evidence\logs\packaged-smoke-loop-20260707-192844.log`. Every iteration verified WindowsGui subsystem, start-minimized, legacy app_data migration, geometry restore, close-to-tray hiding, second-instance exit code 0, first-instance wake, and smoke-owned cleanup. |
| `npm run tray:smoke -- -ExePath .\release-single\ScreenWatchOCRTauri.exe` | Passed. Tauri PID `48988`; tray hidden window class `tray_icon_app`; Show menu PID `48988`; Exit menu PID `48988`; process exited with code `0`. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe` | Passed. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-193356-result.json`. Covered legacy Python profile restore, source preview, template gallery, clipboard bitmap/file paste, one-shot scan, OCR-lite rejection, monitoring start/stop/restart, and layout splitter drags. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate legacy-late-window` | Passed. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-193519-result.json`. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate monitoring-soak --soak-ms 600000` | Passed. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-193601-result.json`. Ran 600000ms, sampled 300 UI states, observed tick delta `2106`, hit delta `526`, progress-log delta `46`, and stopped with the button restored to start. |
| `powershell -ExecutionPolicy Bypass -File scripts\coexistence-smoke.ps1 -PythonExePath E:\Project\Common\screen-watch-ocr\dist\ScreenWatchOCR.exe -TauriExePath .\release-single\ScreenWatchOCRTauri.exe` | Passed after the old Python app was closed. Result: `docs\manual-gate-evidence\logs\coexistence-smoke-20260707-195458-result.json`. Old Python SHA-256 `A5689E32BD7696381DB5A9186977C377DE1BFF7D5A6F1A7F3C22D35C8B240200`; current final Tauri SHA-256 `8986F1168578CF6B564229E3D80C12DC1E8809138B0786B38C8DD99B46E3BF9A`. Process names, exe names, default ports, single-instance commands, visible windows, and WebView2 user-data folder stayed isolated while sharing only a smoke-owned `ScreenWatchOCR` data root. |
| `powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath .\release-single\ScreenWatchOCRTauri.exe -StartupWaitSeconds 18` | Passed as a fresh one-iteration packaged smoke after the coexistence rerun. Verified source/staged PE subsystem WindowsGui (2), start-minimized, legacy app_data migration, legacy geometry restore, close-to-tray, second-instance wake, and smoke-owned cleanup. |
| `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe --gate monitoring` | Passed. Result: `docs\manual-gate-evidence\logs\webview-visual-smoke-20260707-200036-result.json`. Reproved the visible start, progress logging, stop, second start, second progress, and final stop path against the current final exe; the evidence record now avoids referencing a missing focused-gate input directory. |

## Change And Risk Audit

| Area | Current conclusion |
| --- | --- |
| Close-to-tray intermittent timeout | Fixed/hardened. `src-tauri\src\tray.rs` now prevents the close request before hiding the main window. `scripts\packaged-smoke.ps1` now posts close after visible startup settles and prints remaining windows on failure. The current packaged loop passed 10/10. |
| Console popup | Current final exe remains WindowsGui subsystem `2`. |
| Monitoring freeze/restart concern | Current final exe passed visible start/stop/restart smoke and the 600-second monitoring soak. |
| Tray identity | Current final exe tray smoke proved the native menu belonged to the Tauri PID and used the Tauri hidden tray host window class, not the old Python tray class. |
| Old/new coexistence | Fresh current-hash coexistence now passed after the old Python app was closed. The old packaged Python exe and final Tauri exe ran together against one isolated shared `ScreenWatchOCR` data root; Python port `47627` and Tauri port `47628` stayed separate, cross-protocol wake commands were rejected both ways, own commands were accepted, both second instances exited 0, and process names/window titles remained distinct. |
| Single-file deployment boundary | The lite exe stays small and keeps OCR models external. It is proven on this WebView2-present Windows machine; machines without WebView2 still need the installer or WebView2 installed first. |

## Cleanup Notes

The smoke commands stopped only their own launched processes and removed only
their own temporary app roots/app-data directories. Ignored build output,
WebView evidence logs/screenshots, and ignored `target\webview-visual-smoke`
run directories remain for audit.
