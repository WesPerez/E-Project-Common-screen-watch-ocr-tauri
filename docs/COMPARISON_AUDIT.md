# Python To Tauri Comparison Audit

Last updated: 2026-07-07 04:03 +08:00

This is the current requirement-by-requirement audit for replacing
`E:\Project\Common\screen-watch-ocr` with this Rust/Tauri implementation.
The Python app remains the behavioral baseline for any item marked partial or
future.

## Current Deliverable

- Single-file app: `release-single\ScreenWatchOCRTauri.exe`
- Size: 3,582,464 bytes
- SHA-256: `2934248886303006F834A5BA3310261CFD5D9EA37FFB64E14063FDEB4397D66F`
- Build flavor: lite, OCR models external
- Last functional code commit: `cc7034d Fix monitoring restart and compact resizable UI`

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
| Python baseline unittest | 98 tests passed |
| Main migration verifier | Python inventory 98, Python unittest 98, Rust core 117, Tauri 84, OCR feature 23, frontend 94, frontend build and static contracts passed; release rebuild skipped in the latest comprehensive verifier |
| Desktop smoke | 16 real Windows desktop gates passed |
| Packaged smoke | start-minimized, legacy migration, geometry restore, close-to-tray, second-instance wake passed |
| Tray menu smoke | Tauri-owned native menu `Show Tauri` and `Exit Tauri` passed, exit code 0 |
| WebView visual smoke | source preview, gallery workflow, profile-monitoring restart, and layout resize passed; thumbnails measured 75x48; monitoring restart recorded first run 10 ticks/10 hits and second run 5 ticks/5 hits; layout smoke measured target/settings +78px, settings/preview +26px, target-list/log +54px, control-group +58px |
| Portable package verification | lite portable 1,612,062 bytes and full portable 3,749,839 bytes content-verified after `cc7034d` |
| Template benchmark | 2560x1440, 8 templates: flat 65ms 8/8, textured 432ms 8/8 |
| Production template smoke | profile_1 real templates: 18/18 matched on 2560x1440 synthetic placement; 8579ms recorded |
| Real OCR smoke | external PP-OCRv5 English models initialized; READY PNG recognized |
| Manual evidence status | 10 pass, 0 blocked, 0 fail, 0 missing |

## Feature Matrix

| Python baseline feature | Tauri status | Evidence | Remaining risk |
| --- | --- | --- | --- |
| 1-5 profile slots, compatible profile JSON, unknown-field tolerance | Proven | core/profile tests, verifier state/profile contracts | None known for current schema |
| Shared state geometry and last profile | Proven | window layout tests, packaged geometry smoke | DPI-specific restore is tested with tolerance, not every monitor topology |
| Template import from files | Proven | backend gallery workflow, WebView visual smoke | Broad user image corpus not exhaustively sampled |
| Clipboard/path paste templates | Proven by tests | clipboard import tests and frontend paste guards | Real clipboard bitmap path not rerun in packaged smoke |
| Capture selected screen/window as template | Proven | desktop capture gates, WebView gallery capture-source smoke | Third-party hardware-accelerated/minimized windows can still return stale/black frames |
| Template naming, prune limit, reorder, delete, clear | Proven | profile tests, backend gallery workflow, WebView gallery smoke | None known |
| Target enable/disable and select-all/invert | Proven | core/frontend tests, WebView gallery smoke | None known |
| Hit-count badges and clear hit menu | Proven | frontend tests, WebView context-menu smoke | None known |
| Pixel target detection | Proven | Python baseline, Rust core tests, scan tests | None known |
| Template target detection, scales, worker limit | Proven | Rust detector tests, parity/benchmark gates | Production-profile smoke records 8.6s for 18 real templates on synthetic 1440p placement; acceptable but worth tracking |
| OCR target detection | Partially proven | text-row core tests, real PP-OCRv5 English READY smoke | Chinese accuracy, PP-OCRv6/RapidOCR-native compatibility, broad OCR quality are future validation items |
| Screen source listing and mss-style monitor indexes | Proven | desktop monitor-listing smoke | Exotic multi-monitor DPI/topology combinations still need spot checks |
| App-window listing, duplicate ordinals, remembered apps | Proven | window source tests, desktop remembered-window gates | Apps that refuse capture remain an OS/window limitation |
| Screen capture and one-shot scan evidence | Proven | desktop screen capture and one-shot scan gates | None known |
| Window capture with black PrintWindow fallback | Proven | capture tests and desktop window gates | Some GPU/minimized windows may still be uninspectable |
| Source preview with DWM handoff and bitmap fallback | Proven | source-preview tests, real DWM gate, WebView visual smoke | Every third-party window class is not exhaustively covered |
| Persistent monitoring start/stop/status | Proven | monitor session tests, desktop monitoring gates, packaged WebView monitoring restart smoke | Long-duration soak beyond smoke length not yet recorded |
| Stop then start monitoring again | Proven | frontend monitoring state tests, desktop gates, packaged WebView monitoring restart smoke with button restored to `开始监控` after both stops | Long manual UI soak still useful before production use |
| Tick/event logs while monitoring | Proven | frontend tests, `monitor-session` contract, packaged WebView smoke progress rows (`第 N 轮...累计命中...`) | Log cadence depends on capture speed and configured interval |
| Alert screenshots, JSONL, cooldown, pruning | Proven | evidence/scan tests and one-shot desktop gates | Full UI evidence browsing is not a separate gate |
| Beep behavior and throttling | Proven | audio tests and Tauri beep state tests | Actual speaker output is not recorded in smoke |
| Resizable layout splitters | Proven | frontend layout tests for three-pane and stacked layouts; packaged WebView layout smoke drags the target/settings splitter, settings/preview splitter, target-list/log splitter, and a native vertical control-group resize handle with measured deltas and no horizontal overflow | Very narrow/mobile layouts are covered by responsive CSS and static tests, not exhaustive visual smoke |
| Smaller image thumbnails | Proven | WebView visual smoke measured target thumbs 75x48 | None known |
| Close hides to tray | Proven | packaged smoke | None known |
| Tray Show/Exit | Proven | tray-menu smoke using Tauri-owned native menu command IDs | Visual hover tooltip/icon recording not captured, backend icon/tooltip tests cover state |
| Tray monitoring icon/tooltip state | Proven by backend tests | tray monitoring status contract and icon pixel tests | No visual tray hover screenshot in current evidence |
| Start minimized | Proven | packaged smoke and tray smoke | None known |
| Single-instance wake | Proven | packaged smoke | None known |
| Startup shortcut | Proven by tests | startup path/status tests | Creating/removing real user startup shortcut is not performed during smoke |
| Lite package size | Proven | verifier lite size gate | Full OCR build remains larger but still far below Python baseline |
| Installer repeatability | Historical pass | manual evidence records lite/full installer smoke | Not rerun after commit `cc7034d`; current single-file exe smoke is fresh |
| Portable package lite/full | Fresh pass | package verifier produced and content-verified fresh lite/full portable zips after `cc7034d` | None known for package contents; final user deliverable remains the single exe |

## Current Conclusion

The Tauri lite app is the preferred replacement for the current packaged Python
desktop app when the workflow is template/pixel detection, screen/window
capture, profile/template management, tray/startup behavior, and a small
single-file executable. The current delivered exe is about 3.42 MiB versus the
recorded Python/PyInstaller baseline of 102,021,797 bytes.

Do not claim broad OCR parity with the Python RapidOCR/PP-OCRv6 path yet. The
new app has a working optional OCR architecture and a real external PP-OCRv5
English smoke pass, but production OCR quality across Chinese text, PP-OCRv6
assets, and varied real screenshots remains a future validation item.

For adding future features, keep these guardrails:

- Do not change shared profile/state/template file shapes without adding a
  Python-compatibility test.
- Do not reuse Python process names, startup link names, tray identities, or
  single-instance ports.
- Keep OCR models external to avoid turning the lite app back into a large
  bundle.
- Any change to monitoring, tray, or gallery workflow should rerun the relevant
  packaged/WebView smoke, not just unit tests.
