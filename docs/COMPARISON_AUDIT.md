# Python To Tauri Comparison Audit

Last updated: 2026-07-07 07:47 +08:00

This is the current requirement-by-requirement audit for replacing
`E:\Project\Common\screen-watch-ocr` with this Rust/Tauri implementation.
The Python app remains the behavioral baseline for any item marked partial or
future.

## Current Deliverable

- Single-file app: `release-single\ScreenWatchOCRTauri.exe`
- Size: 3,585,024 bytes
- SHA-256: `F50203EE5348DB4EE7C41CE0337D0554EDE411F4EFFA0A3E438E65D4677FC372`
- Build flavor: lite, OCR models external
- Functional source state: current UI/monitoring fix set

## Identity And Data Boundary

The new app intentionally shares only compatible user data with the Python app:

- Shared app data name: `ScreenWatchOCR`
- Shared profiles: `profiles\profile_1.json` through `profile_5.json`
- Shared state file: `state.json`
- Shared templates: `templates\`
- Shared alert files: `screenshots\` and `alerts.jsonl`

Everything else is separated so old and new processes do not collide:

- Tauri product/window title: `Screen Watch OCR Tauri`
- Tauri bundle identifier: `local.screenwatchocrtauri.tauri`
- Tauri release exe: `screen-watch-ocr-tauri.exe`
- Delivered renamed exe: `ScreenWatchOCRTauri.exe`
- Tauri startup link: `屏幕监控OCR Tauri.lnk`
- Tauri single-instance protocol: `127.0.0.1:47628`,
  `ScreenWatchOCRTauri:show\n`
- Tauri tray hidden window class observed in smoke: `tray_icon_app`

## Evidence Summary

| Evidence | Current result |
| --- | --- |
| Python baseline unittest | Current rerun passed 98 tests with `PYTHONPATH=src; python -m unittest -v`; `python -m screen_watch app --smoke-test` returned `{"ok": true, "monitors": 3}` |
| Main migration verifier | Rust core 120, Tauri 85, OCR feature 24, frontend 101, frontend build and static contracts passed; `legacyVisibleWorkflowContract` maps old visible Python workflows to new controls, frontend handlers, and registered Tauri commands, including the Python-compatible global `state.json` `max_alerts` path; Python baseline has fresh standalone evidence above |
| Desktop smoke | 16 real Windows desktop gates passed |
| Packaged smoke | final SHA-256 `F50203EE...` is PE subsystem WindowsGui (2), not console; it passed start-minimized, legacy app_data migration, legacy geometry restore, close-to-tray, and second-instance wake with isolated appdata/port using `release-single\ScreenWatchOCRTauri.exe` |
| Tray menu smoke | final SHA-256 `F50203EE...` passed Tauri-owned native menu `Show Tauri` and `Exit Tauri`, tray menu PID matched Tauri PID, exit code 0; old Python tray/processes were not touched |
| WebView visual smoke | final SHA-256 `F50203EE...` passed clipboard paste, monitoring restart, layout resize, and legacy late-start remembered app-window recovery in packaged WebView2 runs. The late-start smoke loaded an old Python-shaped profile while the remembered app was absent, recorded `skippedWindowApps=1`, then started the app later and scanned/monitored with positive hits without refreshing or reselecting. Clipboard smoke verified CF_DIB bitmap paste and CF_HDROP Ctrl+V file paste with 43x25 target thumbs inside 48x52 cards; monitoring restart recorded generation 1 then 2, first run 7 ticks/7 hits and second run 2 ticks/2 hits with the button restored to `开始监控`; layout smoke measured target/settings +78px, settings/preview +26px, target-list/log +54px, and control-panel group splitter +32px/-32px |
| WebView monitoring soak | final SHA-256 `F50203EE...` ran profile monitoring for 30,000ms with 16 UI samples, tick delta 56, hit delta 56, progress-log delta 28, and stopped with the button restored to `开始监控` |
| Portable package verification | latest lite portable 1,613,143 bytes remains a historical content-verified package; full portable 3,749,839 bytes remains the latest historical full package verification; final user deliverable is the fresh single exe |
| Template benchmark | 2560x1440, 8 templates: flat 65ms 8/8, textured 432ms 8/8 |
| Production template smoke | profile_1 real templates: 18/18 matched on 2560x1440 synthetic placement; 8579ms recorded |
| Real OCR smoke | Current rerun passed with external PP-OCRv5 English models: probe initialized and READY PNG was recognized through `cargo test --features ocr`; `npm run ocr:text:parity` also compared old Python `Detector._ocr` supplied-row semantics against Rust OCR text detection/ScanEngine tests, including min_score, case sensitivity, box flattening, missing-box behavior, and a Unicode contains row |
| Manual evidence status | 16 pass, 0 blocked, 0 fail, 0 missing |

## Feature Matrix

| Python baseline feature | Tauri status | Evidence | Remaining risk |
| --- | --- | --- | --- |
| 1-5 profile slots, compatible profile JSON, unknown-field tolerance | Proven | core/profile tests, verifier state/profile contracts | None known for current schema |
| Shared state geometry, last profile, and global screenshot retention | Proven | window layout tests, max-alerts state compatibility tests, packaged geometry smoke | DPI-specific restore is tested with tolerance, not every monitor topology |
| Template import from files | Proven | backend gallery workflow, WebView visual smoke | Broad user image corpus not exhaustively sampled |
| Clipboard/path paste templates | Proven | clipboard import tests, frontend paste guards, packaged WebView clipboard smoke for CF_DIB bitmap paste and CF_HDROP file-list paste | Every clipboard producer and image codec is not exhaustively sampled |
| Capture selected screen/window as template | Proven | desktop capture gates, WebView gallery capture-source smoke | Third-party hardware-accelerated/minimized windows can still return stale/black frames |
| Template naming, prune limit, reorder, delete, clear | Proven | profile tests, backend gallery workflow, WebView gallery smoke | None known |
| Target enable/disable and select-all/invert | Proven | core/frontend tests, WebView gallery smoke | None known |
| Hit-count badges and clear hit menu | Proven | frontend tests, WebView context-menu smoke | None known |
| Pixel target detection | Proven | Python baseline, Rust core tests, scan tests | None known |
| Template target detection, scales, worker limit | Proven | Rust detector tests, parity/benchmark gates | Production-profile smoke records 8.6s for 18 real templates on synthetic 1440p placement; acceptable but worth tracking |
| OCR target detection | Partially proven | text-row core tests, Python-vs-Rust OCR text parity smoke, current real PP-OCRv5 English READY smoke | Chinese model recognition accuracy, PP-OCRv6/RapidOCR-native compatibility, broad OCR quality are future validation items |
| Screen source listing and mss-style monitor indexes | Proven | desktop monitor-listing smoke | Exotic multi-monitor DPI/topology combinations still need spot checks |
| App-window listing, duplicate ordinals, remembered apps | Proven | window source tests, desktop remembered-window gates | Apps that refuse capture remain an OS/window limitation |
| Existing Python profile/template/state compatibility | Proven | core profile preservation tests, packaged migration smoke, legacy profile WebView end-to-end smoke, legacy late-start remembered-app WebView smoke | None known for generated present-at-startup or late-start remembered app-window workflows |
| Screen capture and one-shot scan evidence | Proven | desktop screen capture and one-shot scan gates; packaged WebView profile one-shot scan smoke drives the visible `扫描一次` button and verifies hit/evidence/profile updates | None known for current generated screen/window sources |
| Window capture with black PrintWindow fallback | Proven | capture tests and desktop window gates | Some GPU/minimized windows may still be uninspectable |
| Source preview with DWM handoff and bitmap fallback | Proven | source-preview tests, real DWM gate, WebView visual smoke | Every third-party window class is not exhaustively covered |
| Persistent monitoring start/stop/status | Proven | monitor session tests, desktop monitoring gates, packaged WebView monitoring restart smoke, packaged 30s monitoring soak | Multi-hour production soak and broad third-party window matrix are not recorded |
| Stop then start monitoring again | Proven | frontend monitoring state tests, desktop gates, packaged WebView monitoring restart smoke with button restored to `开始监控` after both stops | Multi-hour manual UI soak still useful before production use |
| Tick/event logs while monitoring | Proven | frontend tests, `monitor-session` contract, packaged WebView restart smoke progress rows, packaged 30s soak with progress-log delta 28 and per-second heartbeat rows while waiting | Log cadence depends on capture speed and configured interval |
| Alert screenshots, JSONL, cooldown, pruning, evidence directory open | Proven | evidence/scan tests, one-shot desktop gates, packaged WebView one-shot scan smoke checking `alerts.jsonl` plus screenshot output, and legacy visible workflow contract covering `open_evidence_dir` from button to registered backend command | Native Explorer launch is verified by command/contract rather than visual Explorer screenshot |
| Beep behavior and throttling | Proven | audio tests and Tauri beep state tests | Actual speaker output is not recorded in smoke |
| Resizable layout splitters | Proven | frontend layout tests for three-pane, stacked, and multi-pane control layouts; packaged WebView layout smoke drags the target/settings splitter, settings/preview splitter, target-list/log splitter, and control-panel group splitter with measured deltas and no horizontal overflow | Very narrow/mobile layouts are covered by responsive CSS and static tests, not exhaustive visual smoke |
| Smaller image thumbnails | Proven | WebView clipboard smoke measured target thumbs 43x25 inside 48x52 cards and verified toolbar text is not clipped on the smoke viewport | None known |
| Close hides to tray | Proven | packaged smoke | None known |
| Final exe does not use the console subsystem | Proven | packaged smoke parses PE headers and requires WindowsGui subsystem 2 for `release-single\ScreenWatchOCRTauri.exe` and the staged smoke exe | None known for the final single exe |
| Tray Show/Exit | Proven | tray-menu smoke using Tauri-owned native menu command IDs | Visual hover tooltip/icon recording not captured, backend icon/tooltip tests cover state |
| Tray monitoring icon/tooltip state | Proven by backend tests | tray monitoring status contract and icon pixel tests | No visual tray hover screenshot in current evidence |
| Start minimized | Proven | packaged smoke and tray smoke | None known |
| Single-instance wake | Proven | packaged smoke | None known |
| Startup shortcut | Proven by tests | startup path/status tests | Creating/removing real user startup shortcut is not performed during smoke |
| Lite package size | Proven | verifier lite size gate | Full OCR build remains larger but still far below Python baseline |
| Installer repeatability | Historical pass | manual evidence records lite/full installer smoke | Not rerun after the current UI/monitoring fix; current single-file exe smoke is fresh |
| Portable package lite/full | Fresh lite pass, historical full pass | package verifier produced and content-verified a fresh lite portable zip after the current UI/monitoring fix; full portable zip verification is historical | None known for lite package contents; final user deliverable remains the single exe |

## Current Conclusion

The Tauri lite app is the preferred replacement for the current packaged Python
desktop app when the workflow is template/pixel detection, screen/window
capture, profile/template management, tray/startup behavior, and a small
single-file executable. The current delivered exe is about 3.42 MiB versus the
recorded Python/PyInstaller baseline of 102,021,797 bytes.

Do not claim broad OCR model parity with the Python RapidOCR/PP-OCRv6 path yet.
The new app has a working optional OCR architecture, a Python-vs-Rust OCR text
matching parity smoke for supplied rows, and a real external PP-OCRv5 English
smoke pass, but production OCR recognition quality across Chinese text,
PP-OCRv6 assets, and varied real screenshots remains a future validation item.

For adding future features, keep these guardrails:

- Do not change shared profile/state/template file shapes without adding a
  Python-compatibility test.
- Do not reuse Python process names, startup link names, tray identities, or
  single-instance ports.
- Keep OCR models external to avoid turning the lite app back into a large
  bundle.
- Any change to monitoring, tray, or gallery workflow should rerun the relevant
  packaged/WebView smoke, not just unit tests.
