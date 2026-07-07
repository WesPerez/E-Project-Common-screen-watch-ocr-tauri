# Python To Tauri Comparison Audit

Last updated: 2026-07-07 22:48 +08:00

This is the current requirement-by-requirement audit for replacing
`E:\Project\Common\screen-watch-ocr` with this Rust/Tauri implementation.
The Python app remains the behavioral baseline for any item marked partial or
future.

## Current Deliverable

- Single-file app: `release-single\ScreenWatchOCRTauri.exe`
- Size: 3591168 bytes
- SHA-256: `1D03E007175F987B95E2523C16E611F27CD71A86C10EED9AAB0AC24EA5D189FE`
- Build flavor: lite, OCR models external
- Functional source state: current UI/monitoring fix set, OCR-lite startup guard,
  production-template exact-gray fast path, and longer isolated startup-shortcut
  PowerShell timeout for slow WScript.Shell COM startup
- Artifact identity note: `target\release\screen-watch-ocr-tauri.exe` may be
  refreshed by later verifier/build runs and is not treated as bit-for-bit
  proof for the delivered exe. Packaged/WebView evidence keys off the actual
  supplied exe hash, and current WebView smoke records `buildInfoMatchesActual`
  so a `target\release` build-info sidecar cannot be mistaken for proof of
  `release-single\ScreenWatchOCRTauri.exe`.
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
| Latest continuation audit | Fresh current-exe continuation run is recorded in `docs\VERIFICATION_RUN_20260707_2245.md`. It passed current-hash packaged smoke, Tauri-owned tray menu smoke, full WebView visual smoke, late-start remembered-window smoke, focused migration verifier, production template parity/performance smoke, and source-level template benchmarks. Earlier `docs\VERIFICATION_RUN_20260707_1947.md` still records the previous current-hash 600-second monitoring soak and packaged Python/Tauri coexistence proof before the final exact-gray fast-path rebuild |
| Python baseline unittest | Latest continuation rerun passed 98 tests with `PYTHONPATH=src; python -m unittest discover -s tests -t . -v`; `python -m screen_watch app --smoke-test` returned `{"ok": true, "monitors": 3}`. Python/Tk still emits intermittent destroyed-window callback noise during teardown, but unittest exits `OK` |
| Main migration verifier | Current focused verifier contract locks `release-single\ScreenWatchOCRTauri.exe` at 3,591,168 bytes with SHA-256 `1D03E007...` and WindowsGui subsystem. Earlier full verifier evidence passed Python 98, Rust core 121 / 3 ignored, Tauri 88 / 16 ignored, OCR feature 25 / 2 ignored, frontend 103, frontend production build, lite release build, dependency boundaries, `requiredRealGates: 19 workspace gates, 2 OCR gates`, and `liteSizeGate: passed`. Later focused contracts raised frontend coverage to 105 tests and locked visible workflow, UI surface, default settings, failure guards, template file boundaries, profile persistence, audio parity, app-window self-filtering, evidence-directory behavior, close-to-tray prevent-before-hide ordering, and the exact-gray template fast path. The delivered `release-single\ScreenWatchOCRTauri.exe` is the separately WebView/packaged/tray-verified single-file artifact; release builds are not treated as bit-for-bit reproducible evidence |
| Frontend visible UI parity | Latest focused frontend rerun passed 105 unit tests after locking target select-all/invert behavior and evidence-directory feedback. The Tauri target toolbar starts with `全选`, switches through `profileToggleAllLabel()` to `反选` only when all targets are enabled, `legacyVisibleWorkflowContract` statically checks the old Python `全选`/`反选` source text, new HTML text, frontend helper, and unit test, and `legacyUiSurfaceContract` verifies the old profile/startup/gallery/source/region/match/run/status/log/preview surface remains represented in the compact resizable Tauri UI |
| Automated feature surface audit | `npm run audit:feature-surface` now checks the old Python app source, Tauri HTML/frontend/backend command registration, comparison evidence, and migration verifier for 13 feature groups: profile/startup, template toolbar, dynamic target cards, screen/app sources, match settings, previews, scan/monitor/logs, evidence files, tray/single-instance identity, profile compatibility, detection engines, resizable one-page layout, and packaging/WebView2 boundaries |
| Automated Python baseline coverage audit | `npm run audit:python-baseline-coverage` maps all 98 locked Python baseline tests into 6 behavior groups and requires Tauri-side test/smoke/documentation evidence for detector/config logic, window capture/preview/DWM, profile gallery/hit-count behavior, layout/input behavior, tray/startup/single-instance lifecycle, and monitoring/events/evidence/audio behavior |
| Legacy Python CLI boundary audit | `npm run audit:legacy-cli-boundary` verifies the old Python CLI subcommands `app`, `list-monitors`, `once`, `watch`, `screenshot`, `make-demo`, and `self-test`, then checks the Tauri backend/GUI equivalents and the documented boundary. The CLI interface is not preserved in the single GUI exe: `list-monitors`, `once/watch`, arbitrary `screenshot-to-path`, and `make-demo/self-test` are covered by backend commands, GUI workflows, and smoke fixtures, but not exposed as terminal subcommands |
| Desktop smoke | Latest `-IncludeDesktopSmoke` rerun passed 16 real Windows desktop gates with Rust core 124, Tauri 92, and OCR feature 28 still passing: screen capture, monitor listing, one-shot screen/window scan, profile screen/window scan, screen/window/remembered-window screenshot-as-template capture, persistent screen/window monitoring, app-window enumeration, preview/frame capture, and real DWM thumbnail register/update/clear |
| Packaged smoke | Current rerun against final SHA-256 `1D03E007...` verified PE subsystem WindowsGui (2), start-minimized, legacy app_data migration, legacy geometry restore, close-to-tray, and second-instance wake with isolated appdata/ports using `release-single\ScreenWatchOCRTauri.exe`; after the old immediate-close automation reproduced the hide timeout, the app now prevents close before hiding and the smoke waits 750ms after visible startup. Earlier hash `8986F116...` also passed a 10/10 packaged-smoke loop in `packaged-smoke-loop-20260707-192844.log` |
| Tray menu smoke | Current rerun against final SHA-256 `1D03E007...` passed Tauri-owned native menu `Show Tauri` and `Exit Tauri`; tray menu PID matched the Tauri PID, exit code was 0, and old Python tray/processes were not touched |
| Python/Tauri coexistence smoke | Latest full coexistence proof was run before the final exact-gray fast-path rebuild: old Python SHA-256 `A5689E...` and Tauri SHA-256 `8986F116...` were launched together against one shared isolated `ScreenWatchOCR` data root in `coexistence-smoke-20260707-213630-result.json`. Process trees were distinct, both visible main windows were detected, default ports `47627` and `47628` were simultaneously busy, cross-protocol commands were rejected both ways, each app acknowledged only its own command, each second instance exited 0, and Tauri WebView2 children used a smoke-owned user data folder. The final `1D03E007...` rebuild did not change process names, ports, or single-instance protocol, but full coexistence was not rerun because an unrelated old Python process was already running |
| Python-read-Tauri profile compatibility smoke | Latest continuation rerun `python-profile-compat-smoke-20260707-213630-result.json` had the old Python source app load a Tauri-shaped `profile_1.json`, `state.json`, and template PNG from an isolated shared `ScreenWatchOCR` data root, then save once. Required loaded fields matched the fixture and required profile/state keys remained after Python save. The smoke also injected a Tauri-style disk update after Python load; Python stale save overwrote the post-load target hit_count, future profile/target/match/state/layout fields, and post-load max_alerts. Therefore current fields are readable, but simultaneous old/new writes to the same profile are not safe |
| WebView visual smoke | Current full rerun `webview-visual-smoke-20260707-224646-result.json` explicitly launched final SHA-256 `1D03E007...` through `--exe-path .\release-single\ScreenWatchOCRTauri.exe` and passed legacy Python profile restore/scan/monitoring, source preview, template gallery import/edit/reorder/delete/clear/capture with generated PNG/JPG/JPEG/BMP/WebP path imports, clipboard bitmap and file-list paste, one-shot scan with evidence output, OCR-lite raw config rejection, monitoring start/stop/restart with progress logs, and all resizable layout splitter drags. Separate current rerun `webview-visual-smoke-20260707-224804-result.json` also passed the late-start remembered-window workflow after Tauri had already loaded the legacy profile |
| WebView monitoring soak | Previous final SHA-256 `8986F116...` ran profile monitoring for 600,000ms in `webview-visual-smoke-20260707-193601-result.json` with 300 UI samples, tick delta 2106, hit delta 526, progress-log delta 46, distinct tick counts across the samples, and stopped with the button restored to `开始监控`. The current SHA-256 `1D03E007...` passed the full WebView monitoring restart gate, but the 600-second soak was not rerun after the exact-gray fast-path rebuild |
| WebView2 runtime boundary | Local read-only runtime audit found Microsoft Edge WebView2 Runtime `149.0.4022.98`; Tauri official docs confirm WebView2 is preinstalled on Windows 11 and installer-handled on older Windows versions | Final single-exe smoke proves this machine and other WebView2-present Windows machines; it does not prove machines where WebView2 has been removed or was never installed |
| Startup shortcut isolated write/read smoke | `cargo test -p screen-watch-ocr-tauri startup_manager_writes_reads_and_removes_isolated_shortcut` passed; it wrote a real `屏幕监控OCR Tauri.lnk` under a temp Startup-shaped directory, read target/arguments/working-dir through `WScript.Shell`, then removed the temp link | Proves real `.lnk` COM creation without mutating the user's actual Startup folder |
| Portable package verification | latest lite portable `screen-watch-ocr-tauri-lite-portable-20260707-065219-73bb7825.zip` is 1,616,329 bytes and latest full portable `screen-watch-ocr-tauri-full-portable-20260707-065500-3652923c.zip` is 3,753,074 bytes; both are content-verified by `scripts\package-portable.ps1`, keep OCR models external, and reject bundled `.onnx` assets; final user deliverable remains the fresh single exe |
| Template benchmark | Latest continuation rerun on 2560x1440, 8 templates: Python/OpenCV flat 53ms 8/8 and textured 48ms with the known 4/8 odd-phase baseline miss; Rust release flat 75ms 8/8 and textured 485ms 8/8 |
| Production template smoke | Latest continuation rerun against the real shared `profile_1.json`: after the exact-gray rare-anchor fast path, the Rust-only production smoke matched 18/18 on 2560x1440 synthetic placement in 103ms with threshold 0.90, scales 1.0, and template_workers 2; the latest same-frame Python-vs-Rust production parity smoke `production-template-parity-smoke-20260707-221918-result.json` had old Python Detector match 18/18 in 421ms and Rust match the same 18/18 ids in 66ms with no Rust-missing Python hits |
| Real OCR smoke | Latest rerun passed `npm run ocr:text:parity` in `ocr-text-parity-smoke-20260707-214331-result.json` and `npm run ocr:corpus:smoke` in `ocr-corpus-smoke-20260707-214357-result.json`. External PP-OCRv5 English/Chinese assets recognized READY, ALERT 42, OCR TEST, SCAN COMPLETE, ERROR 100%, 准备好了, 开始监控, 屏幕监控, and 发现异常 generated PNGs; text parity compared old Python `Detector._ocr` supplied-row semantics against Rust OCR text detection/ScanEngine tests |
| Manual evidence status | 19 pass, 0 blocked, 0 fail, 0 missing, 0 incomplete, 0 invalid |

## Feature Matrix

| Python baseline feature | Tauri status | Evidence | Remaining risk |
| --- | --- | --- | --- |
| 1-5 profile slots, compatible profile JSON, unknown-field tolerance | Proven | core/profile tests, verifier state/profile contracts, legacy default settings contract | None known for current schema |
| Shared state geometry, last profile, and global screenshot retention | Proven | window layout tests, max-alerts state compatibility tests, packaged geometry smoke | DPI-specific restore is tested with tolerance, not every monitor topology |
| Template import from files | Proven | backend gallery workflow, core JPG/BMP/WEBP conversion test, packaged WebView visual smoke importing PNG/JPG/JPEG/BMP/WebP through the visible path UI | Broad real-world user image corpus is not exhaustively sampled |
| Clipboard/path paste templates | Proven | clipboard import tests, frontend paste guards, packaged WebView clipboard smoke for CF_DIB bitmap paste and CF_HDROP image-file paste | Every clipboard producer and image codec is not exhaustively sampled |
| Capture selected screen/window as template | Proven | desktop capture gates, WebView gallery capture-source smoke | Third-party hardware-accelerated/minimized windows can still return stale/black frames |
| Template naming, prune limit, reorder, delete, clear | Proven | profile tests, backend gallery workflow, WebView gallery smoke, `legacyTemplateFileBoundaryContract` | None known |
| Target enable/disable and select-all/invert | Proven | core/frontend tests, WebView gallery smoke, frontend `profileToggleAllLabel` parity test, migration visible workflow contract | None known |
| Hit-count badges and clear hit menu | Proven | frontend tests, WebView context-menu smoke | None known |
| Pixel target detection | Proven | Python baseline, Rust core tests, scan tests | None known |
| Template target detection, scales, worker limit | Proven | Rust detector tests, parity/benchmark gates, production same-frame Python-vs-Rust parity smoke | Current production-profile same-frame parity records Rust preserving the old Python 18/18 hit set and taking 66ms versus old Python 421ms on synthetic 1440p placement; broader live template distributions and near-match-heavy workloads remain future validation items |
| OCR target detection | Partially proven | text-row core tests, Python-vs-Rust OCR text parity smoke, current real PP-OCRv5 English/Chinese recognition smokes, and a 9-case generated OCR corpus covering English status/number/symbol text plus Chinese UI/alert text | PP-OCRv6/RapidOCR-native compatibility, broad real screenshot quality, and production OCR workload performance remain future validation items |
| Screen source listing, mss-style monitor indexes, and region persistence | Proven | desktop monitor-listing smoke, frontend `profileRegion` test, core window-only profile save test | Exotic multi-monitor DPI/topology combinations still need spot checks |
| App-window listing, duplicate ordinals, remembered apps | Proven | window source tests, desktop remembered-window gates, and verifier contract locking current-process plus old/new app-title self-filtering for `ScreenWatchOCR`, `Screen Watch OCR`, and `Screen Watch OCR Tauri` | Apps that refuse capture remain an OS/window limitation |
| Existing Python profile/template/state compatibility | Proven with write-concurrency boundary | core profile preservation tests, packaged migration smoke, legacy profile WebView end-to-end smoke, legacy late-start remembered-app WebView smoke, Python-read-Tauri profile compatibility smoke | Tauri preserves unknown profile/state fields in the current contract; old Python can read required Tauri-shaped fields, but old Python save drops unknown future top-level/match/state/layout fields and overwrites Tauri-style disk updates made after Python load |
| Screen capture and one-shot scan evidence | Proven | desktop screen capture and one-shot scan gates; packaged WebView profile one-shot scan smoke drives the visible `扫描一次` button and verifies hit/evidence/profile updates | None known for current generated screen/window sources |
| Window capture with black PrintWindow fallback | Proven | capture tests and desktop window gates | Some GPU/minimized windows may still be uninspectable |
| Source preview with DWM handoff and bitmap fallback | Proven | source-preview tests, real DWM gate, WebView visual smoke | Every third-party window class is not exhaustively covered |
| Persistent monitoring start/stop/status | Proven | monitor session tests, desktop monitoring gates, current packaged WebView monitoring restart smoke, current packaged 600s monitoring soak | Multi-hour production soak and broad third-party window matrix are not recorded |
| Stop then start monitoring again | Proven | frontend monitoring state tests, desktop gates, packaged WebView monitoring restart smoke with button restored to `开始监控` after both stops | Multi-hour manual UI soak still useful before production use |
| Tick/event logs while monitoring | Proven | frontend tests, `monitor-session` contract, current packaged WebView restart smoke progress rows, current packaged 600s soak with tick delta 2106 and progress-log delta 46 | Log cadence depends on capture speed and configured interval |
| Alert screenshots, JSONL, cooldown, pruning, evidence directory open | Proven | evidence/scan tests, one-shot desktop gates, packaged WebView one-shot scan smoke checking `alerts.jsonl` plus screenshot output, legacy visible workflow contract covering `open_evidence_dir` from button to registered backend command, `frontendEvidenceDirectoryContract` plus frontend tests locking opened-path status/log text, and Tauri backend tests proving the command creates/returns the Python-compatible `ScreenWatchOCR\screenshots` directory and reports shell-open failure | Native Explorer launch itself is not recorded as a visual Explorer screenshot |
| Beep behavior and throttling | Proven | audio tests, Tauri beep state tests, and migration `audioAlarmParityContract` covering Python `winsound.PlaySound(..., SND_MEMORY)`, Tauri `PlaySoundW`/`SND_MEMORY`, and one-shot/monitoring hit triggers | Actual speaker output is not recorded in smoke |
| Resizable layout splitters | Proven | frontend layout tests for three-pane, stacked, and multi-pane control layouts; packaged WebView layout smoke drags the target/settings splitter, settings/preview splitter, target-list/log splitter, and control-panel group splitter with measured deltas and no horizontal overflow | Very narrow/mobile layouts are covered by responsive CSS and static tests, not exhaustive visual smoke |
| Smaller image thumbnails | Proven | WebView clipboard smoke measured target thumbs 43x25 inside 48x52 cards and verified toolbar text is not clipped on the smoke viewport | None known |
| Close hides to tray | Proven after hardening | close handler now prevents close before hiding, packaged smoke posts close after visible startup settles, and current packaged smoke loop passed 10/10 close-to-tray plus second-instance wake iterations | Very early synthetic close messages are now covered by a settled-startup smoke; broad manual close timing remains worth watching during real use |
| Final exe does not use the console subsystem | Proven | packaged smoke parses PE headers and requires WindowsGui subsystem 2 for `release-single\ScreenWatchOCRTauri.exe` and the staged smoke exe | None known for the final single exe |
| Tray Show/Exit | Proven | tray-menu smoke using Tauri-owned native menu command IDs | Visual hover tooltip/icon recording not captured, backend icon/tooltip tests cover state |
| Tray monitoring icon/tooltip state | Proven by backend tests | tray monitoring status contract and icon pixel tests | No visual tray hover screenshot in current evidence |
| Start minimized | Proven | packaged smoke and tray smoke | None known |
| Single-instance wake | Proven | packaged smoke; packaged Python/Tauri coexistence smoke proves old and new default ports/protocols are isolated | None known for process/protocol isolation |
| New/old process identity isolation | Proven with shared-write boundary | coexistence smoke launched old `ScreenWatchOCR.exe` and final `ScreenWatchOCRTauri.exe` together with shared isolated `ScreenWatchOCR` data, distinct process trees, visible old/new main windows, rejected cross-protocol commands, own-protocol acknowledgements, independent second-instance exits, and smoke-owned Tauri WebView2 UDF; Python-read-Tauri compatibility smoke proves old Python stale save overwrites post-load external profile/state updates | Do not run old and new apps in active monitoring/write mode against the same profile at the same time; simultaneous opening and read compatibility are proven, simultaneous shared JSON writes are not safe because old Python rewrites stale in-memory state |
| Startup shortcut | Proven by isolated real-link test | startup path/status tests plus temp `.lnk` write/read/remove test | Creating/removing the user's actual Startup shortcut is intentionally not performed during smoke |
| Lite package size | Proven | verifier lite size gate | Full OCR build remains larger but still far below Python baseline |
| Single exe launch on arbitrary Windows PCs | Proven with boundary | final single exe smokes plus local WebView2 audit | Machines without WebView2 need the installer or WebView2 installed first; the tiny single exe intentionally does not bundle a browser engine |
| Installer repeatability | Historical pass | manual evidence records lite/full installer smoke | Not rerun after the current UI/monitoring fix; current single-file exe smoke is fresh |
| Portable package lite/full | Fresh lite and full pass | package verifier produced and content-verified fresh lite and full portable zips, including manifest/build-info/hash checks and no bundled OCR models | None known for package contents; final user deliverable remains the single exe |
| Legacy Python CLI commands | Behavior covered, CLI interface not preserved | `list-monitors` maps to Tauri `list_monitors` and visible monitor selection; `once/watch` map to raw/profile scan and monitoring commands; screenshot capture maps to preview and screenshot-as-template workflows; `make-demo/self-test` map to generated smoke fixtures and Rust/frontend/OCR smoke tests | Users who automate `python -m screen_watch ...` terminal commands should keep the Python CLI or request a separate CLI deliverable; the current Tauri deliverable is a single Windows GUI exe |

## Current Conclusion

The Tauri lite app is the preferred replacement for the current packaged Python
desktop app when the workflow is template/pixel detection, screen/window
capture, profile/template management, tray/startup behavior, and a small
single-file executable. The current delivered exe is about 3.43 MiB versus the
recorded Python/PyInstaller baseline of 102,021,797 bytes. The precise wording
for distribution is: the single exe runs on Windows machines that already have
Microsoft Edge WebView2 Runtime, which is the normal Windows 11 case and the
case verified on this machine. For older or locked-down Windows machines where
WebView2 is absent, use the Tauri/NSIS installer or install WebView2 first.

The replacement is not a command-line compatible drop-in for
`python -m screen_watch`. Old CLI behaviors are either covered through Tauri
backend/GUI workflows or retained as test fixtures, but the single GUI exe does
not expose the old terminal subcommands.

Do not claim broad OCR model parity with the Python RapidOCR/PP-OCRv6 path yet.
The new app has a working optional OCR architecture, a Python-vs-Rust OCR text
matching parity smoke for supplied rows, and real external PP-OCRv5 English and
Chinese smoke passes, but production OCR recognition quality across broader
real screenshots, PP-OCRv6 assets, and RapidOCR-native profiles remains a
future validation item.

For adding future features, keep these guardrails:

- Do not change shared profile/state/template file shapes without adding a
  Python-compatibility test, and do not rely on old Python to preserve future
  unknown fields outside the target records it already carries through.
- Do not let delete/clear paths remove user-supplied external image files;
  only generated/imported template files under `templates\` are deletable.
- Do not treat old/new coexistence as permission to actively monitor and write
  the same profile from both apps at once; old Python stale saves can overwrite
  Tauri-written hit counts and future fields.
- Do not reuse Python process names, startup link names, tray identities, or
  single-instance ports.
- Keep OCR models external to avoid turning the lite app back into a large
  bundle.
- Any change to monitoring, tray, or gallery workflow should rerun the relevant
  packaged/WebView smoke, not just unit tests.
