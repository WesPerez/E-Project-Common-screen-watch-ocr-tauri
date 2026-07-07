# Python To Tauri Comparison Audit

Last updated: 2026-07-07 11:34 +08:00

This is the current requirement-by-requirement audit for replacing
`E:\Project\Common\screen-watch-ocr` with this Rust/Tauri implementation.
The Python app remains the behavioral baseline for any item marked partial or
future.

## Current Deliverable

- Single-file app: `release-single\ScreenWatchOCRTauri.exe`
- Size: 3587072 bytes
- SHA-256: `426BE3C7CDA81186BF4B04381E04C7A850B9AD13CAB494AF7065E7296205B911`
- Build flavor: lite, OCR models external
- Functional source state: current UI/monitoring fix set
- Runtime boundary: this tiny single exe uses the system Microsoft Edge
  WebView2 runtime. The current test machine has WebView2 Runtime
  `149.0.4022.98` at
  `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\149.0.4022.98\msedgewebview2.exe`.
  Tauri's official WebView documentation says WebView2 is preinstalled on
  Windows 11, while older Windows versions rely on the installer to ensure
  WebView2 is installed:
  https://v2.tauri.app/reference/webview-versions/#webview2-windows.
  Therefore the single exe is proven on WebView2-present Windows machines; a
  machine without WebView2 needs the NSIS installer or a separate WebView2
  installation first.

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
| Main migration verifier | Latest captured full migration verifier rerun passed Python 98, Rust core 121, Tauri 88, OCR feature 25, frontend 102, frontend production build, lite release build, dependency boundaries, documentation contracts, manual evidence self-test, command/DOM/event contracts, and `singleFileDeliverableContract`; it also recorded `requiredRealGates: 19 workspace gates, 2 OCR gates`, `liteSizeGate: passed`, Python exe 102,021,797 bytes, and Tauri lite exe 3,587,072 bytes. The heavier real OCR, template benchmark, packaged, desktop, tray, and WebView gates are tracked separately below. `legacyVisibleWorkflowContract` maps old visible Python workflows to new controls, frontend handlers, and registered Tauri commands; `legacyProfilePersistenceContract` locks Python `save_state`/`save_current_profile` field placement; `audioAlarmParityContract` locks Python `winsound.PlaySound(..., SND_MEMORY)` behavior against the Tauri runtime; `singleFileDeliverableContract` verifies `release-single\ScreenWatchOCRTauri.exe` size/hash and WindowsGui subsystem |
| Desktop smoke | Current `-IncludeDesktopSmoke` rerun passed 16 real Windows desktop gates with Rust core 121, Tauri 88, and OCR feature 25 still passing: screen capture, monitor listing, one-shot screen/window scan, profile screen/window scan, screen/window/remembered-window screenshot-as-template capture, persistent screen/window monitoring, app-window enumeration, preview/frame capture, and real DWM thumbnail register/update/clear |
| Packaged smoke | Current rerun against final SHA-256 `426BE3C...` verified PE subsystem WindowsGui (2), start-minimized, legacy app_data migration, legacy geometry restore, close-to-tray, and second-instance wake with isolated appdata/ports using `release-single\ScreenWatchOCRTauri.exe` |
| Tray menu smoke | Current rerun against final SHA-256 `426BE3C...` passed Tauri-owned native menu `Show Tauri` and `Exit Tauri`; tray menu PID matched the Tauri PID, exit code was 0, and old Python tray/processes were not touched |
| WebView visual smoke | Current rerun explicitly launched final SHA-256 `426BE3C...` through `--exe-path .\release-single\ScreenWatchOCRTauri.exe` and passed source preview, template gallery, clipboard paste, one-shot scan, monitoring restart, layout resize, and legacy profile in packaged WebView2 runs. Clipboard smoke verified CF_DIB bitmap paste and CF_HDROP Ctrl+V file paste with compact target thumbs inside fixed-size cards; monitoring restart recorded positive tick/hit counts and the button restored to `开始监控`; layout smoke measured all splitters with no horizontal overflow. The separate late-start remembered-window gate remains covered by the earlier fresh dedicated run |
| WebView monitoring soak | final SHA-256 `426BE3C...` was explicitly launched through `--exe-path .\release-single\ScreenWatchOCRTauri.exe` and ran profile monitoring for 120,000ms with 61 UI samples, tick delta 220, hit delta 220, progress-log delta 47, and stopped with the button restored to `开始监控` |
| WebView2 runtime boundary | Local read-only runtime audit found Microsoft Edge WebView2 Runtime `149.0.4022.98`; Tauri official docs confirm WebView2 is preinstalled on Windows 11 and installer-handled on older Windows versions | Final single-exe smoke proves this machine and other WebView2-present Windows machines; it does not prove machines where WebView2 has been removed or was never installed |
| Startup shortcut isolated write/read smoke | `cargo test -p screen-watch-ocr-tauri startup_manager_writes_reads_and_removes_isolated_shortcut` passed; it wrote a real `屏幕监控OCR Tauri.lnk` under a temp Startup-shaped directory, read target/arguments/working-dir through `WScript.Shell`, then removed the temp link | Proves real `.lnk` COM creation without mutating the user's actual Startup folder |
| Portable package verification | latest lite portable 1,616,206 bytes and latest full portable 3,752,774 bytes are both freshly content-verified by `scripts\package-portable.ps1`; both keep OCR models external and reject bundled `.onnx` assets; final user deliverable remains the fresh single exe |
| Template benchmark | 2560x1440, 8 templates: Rust flat 81ms 8/8, Rust textured 509ms 8/8; Python/OpenCV flat 59ms 8/8, Python/OpenCV textured 58ms with the known 4/8 odd-phase baseline miss |
| Production template smoke | profile_1 real templates: 18/18 matched on 2560x1440 synthetic placement; 6583ms recorded |
| Real OCR smoke | Current rerun passed with external PP-OCRv5 English models recognizing READY and external PP-OCRv5 Chinese models recognizing a generated `准备好了` PNG through `cargo test --features ocr`; `npm run ocr:text:parity` also compared old Python `Detector._ocr` supplied-row semantics against Rust OCR text detection/ScanEngine tests, including min_score, case sensitivity, box flattening, missing-box behavior, and a Unicode contains row |
| Manual evidence status | 16 pass, 0 blocked, 0 fail, 0 missing, 0 incomplete, 0 invalid |

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
| Template target detection, scales, worker limit | Proven | Rust detector tests, parity/benchmark gates | Production-profile smoke records about 6.6s for 18 real templates on synthetic 1440p placement; acceptable but worth tracking |
| OCR target detection | Partially proven | text-row core tests, Python-vs-Rust OCR text parity smoke, current real PP-OCRv5 English READY and Chinese `准备` recognition smokes | PP-OCRv6/RapidOCR-native compatibility, broad OCR quality, and varied real screenshots remain future validation items |
| Screen source listing, mss-style monitor indexes, and region persistence | Proven | desktop monitor-listing smoke, frontend `profileRegion` test, core window-only profile save test | Exotic multi-monitor DPI/topology combinations still need spot checks |
| App-window listing, duplicate ordinals, remembered apps | Proven | window source tests, desktop remembered-window gates | Apps that refuse capture remain an OS/window limitation |
| Existing Python profile/template/state compatibility | Proven | core profile preservation tests, packaged migration smoke, legacy profile WebView end-to-end smoke, legacy late-start remembered-app WebView smoke | None known for generated present-at-startup or late-start remembered app-window workflows |
| Screen capture and one-shot scan evidence | Proven | desktop screen capture and one-shot scan gates; packaged WebView profile one-shot scan smoke drives the visible `扫描一次` button and verifies hit/evidence/profile updates | None known for current generated screen/window sources |
| Window capture with black PrintWindow fallback | Proven | capture tests and desktop window gates | Some GPU/minimized windows may still be uninspectable |
| Source preview with DWM handoff and bitmap fallback | Proven | source-preview tests, real DWM gate, WebView visual smoke | Every third-party window class is not exhaustively covered |
| Persistent monitoring start/stop/status | Proven | monitor session tests, desktop monitoring gates, packaged WebView monitoring restart smoke, packaged 120s monitoring soak | Multi-hour production soak and broad third-party window matrix are not recorded |
| Stop then start monitoring again | Proven | frontend monitoring state tests, desktop gates, packaged WebView monitoring restart smoke with button restored to `开始监控` after both stops | Multi-hour manual UI soak still useful before production use |
| Tick/event logs while monitoring | Proven | frontend tests, `monitor-session` contract, packaged WebView restart smoke progress rows, packaged 120s soak with progress-log delta 47 and per-second heartbeat rows while waiting | Log cadence depends on capture speed and configured interval |
| Alert screenshots, JSONL, cooldown, pruning, evidence directory open | Proven | evidence/scan tests, one-shot desktop gates, packaged WebView one-shot scan smoke checking `alerts.jsonl` plus screenshot output, and legacy visible workflow contract covering `open_evidence_dir` from button to registered backend command | Native Explorer launch is verified by command/contract rather than visual Explorer screenshot |
| Beep behavior and throttling | Proven | audio tests, Tauri beep state tests, and migration `audioAlarmParityContract` covering Python `winsound.PlaySound(..., SND_MEMORY)`, Tauri `PlaySoundW`/`SND_MEMORY`, and one-shot/monitoring hit triggers | Actual speaker output is not recorded in smoke |
| Resizable layout splitters | Proven | frontend layout tests for three-pane, stacked, and multi-pane control layouts; packaged WebView layout smoke drags the target/settings splitter, settings/preview splitter, target-list/log splitter, and control-panel group splitter with measured deltas and no horizontal overflow | Very narrow/mobile layouts are covered by responsive CSS and static tests, not exhaustive visual smoke |
| Smaller image thumbnails | Proven | WebView clipboard smoke measured target thumbs 43x25 inside 48x52 cards and verified toolbar text is not clipped on the smoke viewport | None known |
| Close hides to tray | Proven | packaged smoke | None known |
| Final exe does not use the console subsystem | Proven | packaged smoke parses PE headers and requires WindowsGui subsystem 2 for `release-single\ScreenWatchOCRTauri.exe` and the staged smoke exe | None known for the final single exe |
| Tray Show/Exit | Proven | tray-menu smoke using Tauri-owned native menu command IDs | Visual hover tooltip/icon recording not captured, backend icon/tooltip tests cover state |
| Tray monitoring icon/tooltip state | Proven by backend tests | tray monitoring status contract and icon pixel tests | No visual tray hover screenshot in current evidence |
| Start minimized | Proven | packaged smoke and tray smoke | None known |
| Single-instance wake | Proven | packaged smoke | None known |
| Startup shortcut | Proven by isolated real-link test | startup path/status tests plus temp `.lnk` write/read/remove test | Creating/removing the user's actual Startup shortcut is intentionally not performed during smoke |
| Lite package size | Proven | verifier lite size gate | Full OCR build remains larger but still far below Python baseline |
| Single exe launch on arbitrary Windows PCs | Proven with boundary | final single exe smokes plus local WebView2 audit | Machines without WebView2 need the installer or WebView2 installed first; the tiny single exe intentionally does not bundle a browser engine |
| Installer repeatability | Historical pass | manual evidence records lite/full installer smoke | Not rerun after the current UI/monitoring fix; current single-file exe smoke is fresh |
| Portable package lite/full | Fresh lite and full pass | package verifier produced and content-verified fresh lite and full portable zips, including manifest/build-info/hash checks and no bundled OCR models | None known for package contents; final user deliverable remains the single exe |

## Current Conclusion

The Tauri lite app is the preferred replacement for the current packaged Python
desktop app when the workflow is template/pixel detection, screen/window
capture, profile/template management, tray/startup behavior, and a small
single-file executable. The current delivered exe is about 3.42 MiB versus the
recorded Python/PyInstaller baseline of 102,021,797 bytes. The precise wording
for distribution is: the single exe runs on Windows machines that already have
Microsoft Edge WebView2 Runtime, which is the normal Windows 11 case and the
case verified on this machine. For older or locked-down Windows machines where
WebView2 is absent, use the Tauri/NSIS installer or install WebView2 first.

Do not claim broad OCR model parity with the Python RapidOCR/PP-OCRv6 path yet.
The new app has a working optional OCR architecture, a Python-vs-Rust OCR text
matching parity smoke for supplied rows, and real external PP-OCRv5 English and
Chinese smoke passes, but production OCR recognition quality across broader
real screenshots, PP-OCRv6 assets, and RapidOCR-native profiles remains a
future validation item.

For adding future features, keep these guardrails:

- Do not change shared profile/state/template file shapes without adding a
  Python-compatibility test.
- Do not reuse Python process names, startup link names, tray identities, or
  single-instance ports.
- Keep OCR models external to avoid turning the lite app back into a large
  bundle.
- Any change to monitoring, tray, or gallery workflow should rerun the relevant
  packaged/WebView smoke, not just unit tests.
