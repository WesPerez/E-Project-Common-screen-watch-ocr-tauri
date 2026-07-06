# Manual Gates Runbook

These gates cover the remaining checks that need real desktop state, real OCR
assets, installer tooling, or visual evidence. Keep this file aligned with
`docs\FUNCTIONAL_ACCEPTANCE.md` and `scripts\verify-migration.ps1`.

Before running any manual gate, record:

- Date/time and machine name.
- Git/worktree note, or "screen-watch-ocr-tauri is not a git repository".
- Release exe build-info hash from
  `target\release\screen-watch-ocr-tauri.build-info.json` when a packaged exe is
  used.
- Exact commands run and their exit codes.
- Screenshots/video/log excerpts proving the observed UI state.
- Any temporary model/image/evidence directories used.
- Cleanup performed, or a note that cleanup was intentionally not performed.

## Evidence Record Template

Copy this block for every manual gate result. Keep the evidence files beside the
gate notes, or record absolute paths if the files stay elsewhere.

To create the standard record files under `docs\manual-gate-evidence`, run:

```powershell
npm run manual:evidence -- -New
```

To inspect the current manual-gate completion state without changing records,
run:

```powershell
npm run manual:evidence -- -Status
```

The status output includes a `manualGateEvidenceStatus:` summary line.

After filling the records, validate that every manual gate is marked `pass` and
has all required evidence fields:

```powershell
npm run manual:evidence
```

During partial work, use `-AllowNonPass` to check field completeness without
claiming all gates have passed.

```text
Gate:
Completion status: pass | fail | blocked
Date/time:
Machine:
Worktree note:
Command(s) and exit code(s):
Release build-info hash:
Model/image/evidence dirs:
Observed result:
Evidence files:
Cleanup performed:
Remaining risk:
```

## Baseline Before Manual Gates

Run the fast automated migration gate first:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease
```

When changing frontend behavior, also run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease
```

Expected evidence:

- `rustCoreTests`, `tauriTests`, and `ocrFeatureTests` meet current baselines.
- `liteSizeGate: passed`.
- `requiredRealGates: 19 workspace gates, 2 OCR gates`.
- `manualGateEvidenceStatus:` includes the profile clipboard paste, profile
  one-shot scan, profile monitoring restart, and WebView layout resize gates
  when validating final evidence.

## Desktop Backend Smoke

Run on an interactive Windows desktop:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke
```

Expected evidence:

- `desktopSmoke: 16 gates`.
- Real screen capture, monitor listing, window capture, monitoring, profile
  scan, profile monitoring, and DWM API gates pass.
- No app process remains after the command exits.

## Real OCR Model Smoke

Prerequisites:

- External model directory containing `det.onnx`, `rec.onnx`, and
  `ppocrv5_dict.txt`.
- For recognition smoke, a PNG image containing known text and the expected
  text substring.

Probe-only command:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeOcrSmoke -OcrModelDir "D:\Models\rapidocr"
```

Probe plus recognition command:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeOcrSmoke -OcrModelDir "D:\Models\rapidocr" -OcrSmokeImage ".\smoke.png" -OcrSmokeExpect "READY"
```

Expected evidence:

- `ocrSmoke: probe` or `ocrSmoke: probe and recognition`.
- `native_ocr_real_model_probe_initializes_from_external_assets` passes.
- `native_ocr_real_model_recognizes_smoke_png` passes when image/expect are
  supplied.
- The run does not copy or embed OCR model files into the app package.

## WebView Source Preview Visual Smoke

Run the app:

```powershell
npm run tauri:dev
```

Repeatable automated evidence can also be collected against the current packaged
release exe on an interactive Windows desktop:

```powershell
npm run webview:visual:smoke -- --gate source
```

Manual steps:

- Select at least one physical screen source and at least one visible app-window
  source.
- Click `刷新来源预览`.
- Scroll and resize so one source card becomes partially offscreen, then returns
  onscreen.
- Confirm bitmap previews remain visible and DWM-backed window previews do not
  leave black or stale overlays.
- Start and stop monitoring, then confirm preview cards still refresh.

Expected evidence:

- Screenshot or video of screen and window source cards.
- Screenshot or video after scroll/resize restore.
- Main status text shows preview counts without unexpected failed previews.
- No stale DWM overlay remains after closing/minimizing/restoring the window.

## Template Gallery Visual Workflow Smoke

Run the app:

```powershell
npm run tauri:dev
```

Repeatable automated evidence can also be collected against the current packaged
release exe on an interactive Windows desktop:

```powershell
npm run webview:visual:smoke -- --gate gallery
```

Manual steps:

- Import several PNG/JPG images into a profile.
- Toggle one target off and use select-all/invert.
- Drag/drop reorder targets.
- Use up/down row buttons.
- Right-click a target with hit count and clear the hit count.
- Delete one target and then clear all.
- Capture current source as a template.

Expected evidence:

- Gallery selection remains on the intended target after each backend edit.
- Thumbnails have stable dimensions and no bottom-border clipping.
- Drag/drop and row-button reorder produce the expected order.
- Only template files under `templates\` are removed by delete/clear actions.

## Profile Clipboard Paste Smoke

Repeatable automated evidence can be collected against the current packaged
release exe on an interactive Windows desktop:

```powershell
npm run webview:clipboard:smoke
```

Manual fallback steps:

- Save or note the current clipboard content before the test.
- Copy a bitmap image from a screenshot tool or image editor.
- Click `粘贴图片` and confirm a selected template card appears.
- Clear the profile targets.
- Copy one supported image file as a Windows file-list clipboard item.
- Press `Ctrl+V` while the app is focused and confirm a selected template card
  appears.
- Restore the previous clipboard content if it was changed.

Expected evidence:

- Result JSON proving CF_DIB bitmap paste and CF_HDROP image-file paste both
  created template cards.
- Screenshot or video after each paste.
- Thumbnail dimensions remain compact and non-zero.
- Cleanup notes state whether the previous clipboard object was restored.

## Profile One Shot Scan Smoke

Repeatable automated evidence can be collected against the current packaged
release exe on an interactive Windows desktop:

```powershell
npm run webview:scan:smoke
```

Manual fallback steps:

- Select one stable app-window source and no screen source.
- Capture that source as a template.
- Click `扫描一次`.
- Confirm the UI reports a positive hit count and appends a scan result row to
  the log table.
- Confirm the target hit badge/profile hit count updates.

Expected evidence:

- Screenshot or video before the scan and after the hit.
- Result JSON proving a positive one-shot `hitCount`, zero skipped selected
  window sources, and at least one window scan result.
- alerts.jsonl plus screenshot evidence written under the isolated
  `ScreenWatchOCR` data directory.
- Profile JSON showing the matching target `hit_count` increased.

## Profile Monitoring Restart Smoke

Repeatable automated evidence can be collected against the current packaged
release exe on an interactive Windows desktop:

```powershell
npm run webview:monitoring:smoke
```

Manual fallback steps:

- Select one stable app-window source and no screen source.
- Capture that source as a template.
- Click `开始监控`.
- Confirm the log table keeps adding progress rows while monitoring runs.
- Click `停止监控`, then click `开始监控` again.
- Stop the second run and confirm the run button returns to `开始监控`.

Expected evidence:

- Screenshot or video of the prepared template and the running monitor state.
- Log or result JSON proving positive tick count and hit count.
- Evidence that start/stop/restart monitoring restores the button state and does
  not leave an app process running after the smoke exits.

## WebView Layout Resize Smoke

Repeatable automated evidence can be collected against the current packaged
release exe on an interactive Windows desktop:

```powershell
npm run webview:layout:smoke
```

Manual fallback steps:

- Drag the target/settings splitter horizontally.
- Drag the settings/preview splitter horizontally.
- Drag the target-list/log splitter vertically.
- Drag one settings-section splitter vertically, such as the monitor-screen /
  monitor-app divider.
- Confirm the target toolbar, template grid, settings controls, log table, and
  preview panel remain visible without horizontal overflow.

Expected evidence:

- Result JSON showing measured width/height deltas for the target/settings
  splitter, settings/preview splitter, target-list/log splitter, and
  control-panel group splitter.
- Screenshot or video after the resize sequence.
- No horizontal page overflow and no clipped critical controls after the drags.

## Packaged Tray Menu And Icon Smoke

Build or reuse a lite release exe:

```powershell
npm run tauri:build:lite
```

Run packaged baseline smoke:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath target\release\screen-watch-ocr-tauri.exe -StartupWaitSeconds 18
```

Run the repeatable tray menu smoke:

```powershell
npm run tray:smoke -- -ExePath target\release\screen-watch-ocr-tauri.exe
```

Manual fallback steps:

- Launch `target\release\screen-watch-ocr-tauri.exe --start-minimized`.
- Confirm the app starts hidden with a tray icon.
- Use the real system tray menu `Show Tauri` item to show the main window.
- Start monitoring and confirm tray tooltip/icon changes.
- Use the real system tray menu `Exit Tauri` item to exit.

Expected evidence:

- `tray:smoke` output proving `Shell_NotifyIconGetRect` found a Tauri
  `tray_icon_app` hidden window, the native `#32768` menu window belongs to the
  Tauri PID, `Show Tauri` reveals the main window, and `Exit Tauri` exits with
  code 0; or screenshot/video of tray menu show and exit actions.
- Screenshot/video or log showing monitoring tooltip/icon state.
- Process exits after the tray exit action.

## Installer Repeatability Smoke

Prerequisites:

- NSIS/WiX tooling can be downloaded or is already available.
- Network/proxy setup is documented if required.

Commands:

```powershell
npm run tauri:build:lite
npm run tauri:build:full
```

Expected evidence:

- Lite and full installer artifacts appear under `target\release\bundle`.
- Lite build-info reports `flavor: lite`; full build-info reports
  `flavor: full`.
- Flavor-specific installer and build-info copies are retained so lite/full
  evidence can coexist after both builds finish.
- OCR models are not bundled in either installer.
- Installed lite and full executables pass packaged runtime smoke without OCR
  models; full OCR tests report missing model status until external assets are
  supplied.

## Production Template Performance Smoke

Run the fixed parity script:

```powershell
npm run template:parity
```

Run the fixed flat/textured Rust benchmark gate when you need the lower-level
template detector timings separately:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\template-benchmark.ps1
```

Then run a representative production template set from the shared compatible
profile/data directory:

```powershell
npm run production:template:smoke
```

Expected evidence:

- Fixed parity output records Python/OpenCV and Rust timings.
- Production dataset description, frame resolution, target count, scale range,
  match counts, placements, and elapsed time are recorded.
- Any Rust-vs-Python slowdown is documented before declaring completion.
