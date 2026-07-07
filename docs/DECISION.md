# Lightweight Runtime Decision

## Final Conclusion

Use Rust + Tauri as the long-term application shell and core runtime.

This is not because Rust is magically the smallest language in every scenario.
C, C++, and Zig can produce smaller binaries for very narrow native tools. The
reason Rust/Tauri is the best fit here is the balance:

- Very small native executable for this app class.
- No bundled Python interpreter.
- No bundled browser engine; Tauri uses the system WebView on Windows.
- Good access to Windows APIs for capture, tray, startup, single-instance, and
  window integration.
- Strong type system and testability for a growing monitoring app.
- OCR models can stay external instead of being packed into the exe.
- The OCR runtime can stay behind a Cargo feature, so the `lite` executable
  does not accidentally inherit OCR inference dependencies as the project grows.

This small-exe conclusion has an important Windows runtime boundary. Tauri uses
Microsoft Edge WebView2 on Windows instead of bundling Chromium. Tauri's
official WebView documentation states that WebView2 is preinstalled on Windows
11, and that on versions older than Windows 11 the Tauri-generated installer is
the mechanism that ensures WebView2 is installed:
https://v2.tauri.app/reference/webview-versions/#webview2-windows. The final
single exe is therefore the right artifact for WebView2-present Windows
machines. A machine without WebView2 needs the installer or a separate WebView2
installation first; bundling an offline/fixed WebView2 runtime would add roughly
127-180 MiB according to Tauri's config reference:
https://v2.tauri.app/reference/config/#webviewinstallmode.

Measured on this machine:

| Build | File | Size |
| --- | --- | ---: |
| Current Python/PyInstaller app | `E:\Project\Common\screen-watch-ocr\dist\ScreenWatchOCR.exe` | 102,021,797 bytes, about 97.3 MiB |
| New Rust/Tauri lite exe before release-size tuning | `target\release\screen-watch-ocr-tauri.exe` | 8,811,008 bytes, about 8.4 MiB |
| New Rust/Tauri lite final single exe after release-size tuning, migration hardening, and production-template fast path | `release-single\ScreenWatchOCRTauri.exe` | 3,591,168 bytes, about 3.43 MiB |
| New Rust/Tauri full exe with native `pure-onnx-ocr` backend linked through the lazy OCR worker and OCR models external | `target\release\screen-watch-ocr-tauri.exe` after `scripts\package-portable.ps1 -Flavor full` | 9,536,512 bytes, about 9.09 MiB |
| New Rust/Tauri full portable zip, OCR models external | `target\portable\screen-watch-ocr-tauri-full-portable-20260707-031231-0b794cfa.zip` | 3,752,774 bytes, about 3.58 MiB |

The Rust `target/` directory is much larger because it is a build cache. It is
not shipped to users.

## Why The Python Package Is Large

The current Python app packages:

- Python runtime.
- PyInstaller bootloader.
- Tkinter/Pillow support.
- NumPy.
- OpenCV.
- ONNXRuntime.
- RapidOCR and its collected assets.
- pystray and Windows integration dependencies.

The current spec explicitly collects all RapidOCR assets:

```python
tmp_ret = collect_all('rapidocr')
datas += tmp_ret[0]; binaries += tmp_ret[1]; hiddenimports += tmp_ret[2]
```

So the 100 MiB class size is expected. It can be trimmed, but it is hard to make
this Python stack genuinely small because NumPy, OpenCV, ONNXRuntime, and the
Python runtime are all substantial.

## Language And Framework Comparison

| Option | Expected package size | Can implement all current features | Risk |
| --- | ---: | --- | --- |
| Keep Python + PyInstaller | High, currently about 97 MiB | Yes, already does | Size grows as native deps grow |
| Python trimmed build | Medium/high | Yes | Fragile hidden imports; OCR/OpenCV still heavy |
| Go + Wails/Fyne | Low/medium | Mostly yes | Windows capture/OCR ecosystem less direct than Rust/C++; Go runtime included |
| .NET Native AOT / WinUI | Medium | Yes | Good Windows APIs, but UI/runtime packaging can grow |
| C++ native | Very low to medium | Yes | Highest maintenance cost and memory-safety risk |
| Zig native | Very low | Theoretically yes | Ecosystem maturity risk for GUI/OCR/Windows integration |
| Rust native GUI | Very low | Yes | UI ecosystem less polished unless using WebView/Tauri |
| Rust + Tauri | Very low for app exe; installer depends on bundler | Yes | Need staged port to avoid feature loss |

## Can Rust/Tauri Implement Everything The Python App Has?

Yes, but it should not be rewritten in one jump.

Current Python features include multi-monitor screen capture, region monitoring,
template matching, pixel matching, OCR text matching, template gallery, profile
slots, app-window monitoring, DWM live preview, tray behavior, startup shortcut,
single-instance wake behavior, evidence screenshots, alert JSONL, screenshot
pruning, and config persistence.

Rust/Tauri can cover them with this split:

| Current feature | Rust/Tauri route |
| --- | --- |
| Config/profile compatibility | Rust serde models using the same JSON shape |
| Multi-monitor capture | Windows capture APIs or a Rust screenshot crate, isolated behind a capture trait |
| Window capture / DWM preview | `windows` crate / Windows API bindings |
| Template matching | Pure Rust matching for lite mode, optional OpenCV backend only if needed |
| Pixel matching | Pure Rust, already started |
| OCR text matching | Optional `ort`/ONNX module with PP-OCR/RapidOCR-compatible external models |
| Tray/startup/single-instance | Tauri plugins or small Rust platform modules |
| GUI/template gallery | Tauri frontend, with Rust commands for stateful operations |
| Evidence screenshots/logging | Rust core plus stable app-data paths |

## The Safe Migration Strategy

The safest solution is an incremental parallel port:

1. Keep the Python app as the known-good baseline.
2. Keep the same app-data directory: `ScreenWatchOCR`.
3. Keep the same profile/config JSON shape.
4. Add Rust tests for every config/detection behavior before replacing it.
5. Build two flavors:
   - `lite`: no OCR Cargo feature, no OCR models, small app, template/pixel/core monitoring.
   - `full`: OCR Cargo feature enabled, but models live in the external app-data model directory.
6. Only switch the user-facing app after each Python feature has an equivalent
   Rust acceptance test or manual verification gate.

This directly addresses the main risk: a rewrite that is smaller but quietly
loses features.

## Current Implementation State

Created project: `E:\Project\Common\screen-watch-ocr-tauri`

Implemented and verified:

- Rust workspace with a separated `screen-watch-core` crate.
- Tauri desktop shell.
- Same app-data directory name: `ScreenWatchOCR`.
- Python-style config parsing and validation.
- Unknown JSON fields preserved/ignored safely.
- Compatible target kinds: `template`, `pixel`, `ocr_text`.
- Compatible scale syntax, including percent ranges.
- Pixel detection core.
- Deterministic exact-template test helper.
- Pure Rust in-memory template matching with scaled templates, textured-template
  correlation scoring, flat-template difference scoring, threshold filtering,
  and scale reporting.
- Python-style large-frame template coarse/refine matching with 0.5/0.25 coarse
  factors, local refinement, flat-template early exits, and phase-aware sparse
  textured-template coarse candidates so odd-origin small textures are not
  missed without full NCC at every coarse location. It still falls back when
  coarse downscaling would erase textured-template detail.
- PNG/JPG/JPEG/BMP/WEBP template loading from config-relative paths and a
  prepared detector that preserves configured target order.
- `template_workers` config parsing and bounded parallel template matching that
  clamps zero to one effective worker, caps workers to template job count, and
  preserves configured target order.
- Repeatable large-frame flat and textured multi-template benchmark wiring
  through `scripts\template-benchmark.ps1` and `-IncludeTemplateBenchmark`. The
  fixed Rust benchmarks each verify 8 template matches on a 2560x1440 frame and
  report elapsed time, with an optional max-ms threshold for machine-specific
  regression checks.
- Profile-derived scaled-template scans now have a cross-layer gate from
  profile JSON and `templates/` PNG loading through `WatchConfig`,
  `PreparedDetector`, `ScanEngine`, and evidence writing.
- Source planning that preserves Python/mss monitor indexing, defaults empty
  screen regions to physical monitors, and keeps window-only configs from
  implicitly selecting all monitors.
- A real Windows monitor-listing smoke gate that verifies virtual monitor `0`,
  physical monitor indexes starting at `1`, and virtual bounds against Win32
  system metrics.
- Tauri commands for app info, monitor listing, config validation, and config
  source resolution.
- Windows GDI screen-region capture into `RgbFrame`, plus a Tauri PNG preview
  command for manual verification and future preview wiring.
- Source preview signatures and an in-memory preview-frame cache for screen and
  window preview commands, so unchanged sources can reuse frames until the
  source geometry signature changes.
- A basic frontend source preview panel that renders the currently selected
  physical monitor and app-window sources through the cached preview commands.
- Automatic bitmap-preview refresh scheduling through a single frontend timer,
  with stale-cache pruning and busy-state skips for hidden pages, settling
  resizes, and template drag/drop.
- Windows DWM preview overlay state and Tauri commands for source-keyed
  register/update/reuse, stale-source retention, and clear-on-hide/close paths,
  with frontend window-source cards attempting DWM handoff from their visible
  DOM rect while retaining bitmap previews as the fallback.
- Real Windows DWM thumbnail smoke gate that creates two temporary Win32
  windows, registers and updates a source-keyed thumbnail, verifies reuse, and
  clears it through the same preview state.
- Tauri one-shot screen scan command/helper that resolves screen regions from
  config text, captures real frames, runs `ScanEngine`, and writes evidence.
- Tauri one-shot app-window scan path that resolves concrete hwnd-backed window
  sources, captures real window frames, runs `ScanEngine`, and writes evidence.
- Tauri monitoring session state with start/stop/status commands, reusable
  `ScanEngine` cooldown state, safe polling floor, refreshed remembered app
  windows, screen/window source scanning, and worker join on stop.
- Tauri monitoring event stream (`screen-watch://monitor-session`) for start,
  tick, hit/error, and stop snapshots, with the frontend listening for live
  status updates instead of relying only on manual polling. Backend tests lock
  zero-tick start/stop transition payloads and skipped-source snapshot fields.
- Windows app-window enumeration with Python-compatible filtering, title sorting,
  duplicate ordinal numbering, legacy `title\0ordinal` keys, display labels,
  and remembered `window_apps` resolution to concrete hwnd-backed window
  sources.
- Windows app-window capture with window rect resolution, PrintWindow capture,
  visible screen fallback, black-frame detection, black-padding crop behavior,
  and a Tauri PNG preview command.
- Profile/template file normalization: stable ids, hit-count normalization,
  legacy thumb removal, missing-template pruning, gap-filling rename, and guarded
  deletion limited to files under `templates/`.
- Non-destructive legacy `app_data` migration that copies missing old
  `profiles/`, `templates/`, `state.json`, `alerts.jsonl`, `alerts/`, and
  `screenshots/` files into the shared `ScreenWatchOCR` user data directory
  without overwriting newer shared files.
- Profile read snapshots that preserve unknown JSON, avoid creating missing
  profile files, and report target/enabled/all-enabled state for the frontend.
- Profile source and match persistence that preserves Python-compatible
  `monitors`, `windows`, `region`, `match`, and `state.json` `last_profile`
  fields without discarding unknown profile, match, or layout data.
- Main-window geometry persistence that reads and writes Python-compatible
  `state.json` `layout.geometry`, preserves existing layout ratios and unknown
  state fields, and ignores minimized/hidden taskbar placeholder geometry.
- Profile source/config compatibility tests for the Tauri frontend boundary:
  camelCase option deserialization, concrete window ordinal preservation,
  remembered `window_apps` persistence precedence, and mixed concrete plus
  remembered source config construction.
- Profile hit-count recording and clearing by stable target id, exposed through
  Tauri commands that operate on the shared app-data profile files.
- Profile image importing that accepts PNG/JPG/JPEG/BMP/WEBP inputs, prunes to
  the configured template limit before naming, writes normalized RGB PNGs under
  `templates/`, preserves unknown profile fields, and is exposed through a
  Tauri command for the gallery UI.
  The Tauri backend also supports Python-style clipboard import for supported
  image file paths and CF_DIB/CF_DIBV5 bitmap data, using the same normalized
  PNG storage path. Frontend import requests now normalize pasted path text,
  native picker arrays, and clipboard paste actions through tested path/limit
  contracts, then report added, pruned, and current target counts from the
  backend result.
- Screenshot-as-template capture for GUI profiles: selected screen regions are
  preferred before selected/resolved windows like the Python app, empty profiles
  can capture their first template, and the captured frame is stored through the
  same normalized PNG profile import path.
- Frontend target selection now follows backend `selectedIndex`/`selected_index`
  results after profile import, reorder, remove, target clearing, and hit-count
  clearing actions, while clearing stale repeat-click state after those edits.
- Normal frontend profile loads and profile switches select the first template
  like Python `load_profile`; background profile refreshes keep the current
  selection or remain unselected when the current index is invalid.
- Frontend target enable/toggle-all actions now surface Python-style
  enabled/total template counts from the backend result instead of collapsing to
  a generic `Ready` status.
- Profile target reorder/remove/clear backend commands that preserve stable ids,
  rename template files by the new position, and delete only files proven to be
  under `templates/`.
- Profile target enabled-state helpers and Tauri commands, including
  Python-compatible select-all/invert behavior and detector-target filtering
  that skips disabled templates while treating missing `enabled` as true.
- Profile-to-`WatchConfig` construction for the future GUI workflow, preserving
  Python GUI defaults for selected sources, template threshold/scales, poll
  interval, cooldown, evidence paths, worker compatibility fields, and alarm
  settings.
- Profile one-shot scan and persistent monitoring-session Tauri commands that
  reuse the same source resolution, capture, scan, evidence, beep, and event
  paths as config-text commands, and record profile target hit counts from the
  same cooldown-filtered alerted matches as the Python `target_hits` worker
  path.
- Frontend profile workflow gating that blocks profile config preview, one-shot
  scan, and monitoring start before backend invocation when there are no enabled
  templates, no selected sources, or an already-running profile monitoring
  session.
- A real Windows profile-monitoring smoke gate that captures a desktop region,
  matches an imported profile template through the session backend, writes
  evidence, and persists profile `hit_count` updates through `ProfileHitSink`.
- A basic Tauri frontend profile panel for reading profile state, restoring the
  last profile, persisting selected screen/window sources, normalizing a
  profile, toggling all targets, and refreshing hit-count badges after profile
  monitoring hit events.
- Alert evidence writer: red-box PNG screenshots with target labels, JSONL
  append, and screenshot pruning using the same app-data path conventions as
  the Python app.
- Frame scan engine that runs prepared detection, per-region/target cooldown,
  and evidence writing for supplied RGB frames.
- Alarm beep settings parsed from compatible config fields, dependency-free
  PCM WAV generation, 0..100 volume clamping, no-restart throttle behavior, and
  Tauri scan/session trigger wiring after alert hits.
- Windows Startup shortcut backend that uses the distinct Tauri `.lnk` name
  `屏幕监控OCR Tauri.lnk`,
  writes packaged startup arguments as `--start-minimized`, avoids deleting
  foreign startup shortcuts, and exposes explicit startup status/toggle commands
  plus frontend controls. The backend now has an isolated Windows-only real
  shortcut smoke test that creates, reads, and removes a temp `.lnk` through
  `WScript.Shell` without touching the user's actual Startup folder.
- Tauri-specific single-instance wake protocol on `127.0.0.1:47628` using
  `ScreenWatchOCRTauri:show\n` and `ok\n`, with second-instance exit behavior
  and main-window show/unminimize/focus handling in Tauri. This intentionally
  does not reuse the Python app's port or wake command, so old and new apps can
  run side by side while sharing only the `ScreenWatchOCR` app-data directory.
  If the first notify attempt misses an already-bound instance port, the claim
  path retries wake notification once before reporting the port unavailable.
- Tauri tray lifecycle backend with a generated status icon, show/exit menu
  actions, close-to-tray behavior guarded by tray availability,
  `--start-minimized` handling after successful tray install, and monitoring
  state reflected in tray tooltip/icon updates.
- Packaged tray/startup smoke automation that launches release exes with a
  temporary app-data directory and isolated single-instance ports, stages the
  exe in a temporary app root with a legacy `app_data` fixture, verifies
  non-destructive migration into `ScreenWatchOCR`, verifies that the migrated
  Python-compatible `state.json` `layout.geometry` is applied to the real main
  window with DPI-aware tolerance, verifies the `--start-minimized` main-window
  hidden state, posts `WM_CLOSE` to verify close-to-tray hiding without process
  exit, launches a second exe instance to verify real single-instance exit and
  main-window restore, then stops only the processes it created and removes
  only its temporary directories. The default wait is 18 seconds to reduce
  desktop/tray event flake.
- Lite/full build flavor detection. Standalone exes now infer the runtime
  flavor from the compiled `ocr` feature boundary when no runtime
  `SCREENWATCH_BUILD_FLAVOR` override exists, so full portable builds do not
  accidentally report lite outside the build environment.
- Cargo feature boundary for the optional OCR module: default/lite builds do
  not enable `ocr`, while full builds pass `--features ocr`.
- External OCR model directory policy.
- OCR availability reporting without bundling models, including per-required
  model existence/byte status, explicit disabled, not-compiled, missing-model,
  and not-yet-linked backend states, and final `available` status gated on both
  `modelsReady` and `backendReady`.
- First native Rust OCR backend wiring through `pure-onnx-ocr`, behind the
  `ocr` Cargo feature. The linked native profile is `ppocrv5-dbnet-svtr` and
  expects external `det.onnx`, `rec.onnx`, and `ppocrv5_dict.txt` assets. The
  previous PP-OCRv6/RapidOCR model filenames are kept as reference status until
  that exact native profile is wired.
- Lazy reusable native OCR worker: scans start the worker only when OCR rows are
  actually requested, keep the non-`Send`/non-`Sync` engine inside that worker,
  and reuse the initialized engine or cached initialization error across frames.
- Explicit OCR backend probe: setup can distinguish skipped lite/not-compiled
  and missing-model states from native initialization failure without starting a
  scan or monitoring session.
- Repeatable real-model OCR smoke wiring: `scripts\ocr-smoke.ps1` runs ignored
  feature tests that require external `det.onnx`, `rec.onnx`, and
  `ppocrv5_dict.txt` assets, and can optionally verify recognized text from a
  caller-supplied PNG plus expected substring. `scripts\verify-migration.ps1`
  exposes the same gate with `-IncludeOcrSmoke`.
- Shared runtime OCR backend factory used by both one-shot scans and monitoring
  sessions, so the future native inference backend has a single integration
  point.
- Frontend OCR setup visibility that renders readiness flags and required model
  file status/path/size from the app-info command and exposes the manual backend
  probe.
- Portable lite/full zip packaging that avoids installer tool downloads,
  validates the archive contents, rejects bundled `.onnx` files and required
  OCR asset filenames, and ships the exe with a manifest and README while
  keeping OCR models external.
- Template performance parity script for fixed 2560x1440, 8-template workloads;
  latest local flat result is Python/OpenCV `46ms` versus Rust release `89ms`,
  after exact-flat-template integral scoring, shared gray/scale frame caching,
  cached flat-template frame integrals across coarse/refine passes, prepared
  gray templates, and perfect-score early return for pure flat-template scans.
  The odd-phase textured workload records Python/OpenCV `60ms` with 4/8 matches
  and Rust release `413ms` with 8/8 matches after phase-aware sparse coarse
  candidates; textured-template NCC also uses integral-assisted window
  mean/energy and has parity tests for uncached scoring and sparse candidate
  preference.
- Desktop profile screen workflow smoke: captures a real desktop region, saves
  a cropped template through profile import, builds a profile watch config,
  scans the same screen region, writes evidence, and records the target hit
  count without touching the shared user data directory.
- Desktop profile window workflow smoke: creates a temporary Win32 window,
  captures it through the real app-window capture path, imports a cropped
  template into a profile, builds a window-only profile watch config, scans the
  same window source, writes evidence, records profile `hit_count`, and destroys
  the test-owned window.
- OCR text-row matching core that preserves Python case-sensitivity,
  minimum-score, bounding-box flattening, and configured target order once an
  external OCR backend supplies rows.
- Scan-engine OCR backend interface wiring: OCR targets now request backend rows
  through one path shared by one-shot scans and monitoring sessions, while lite,
  not-compiled, missing-model, and not-yet-linked backend states fail
  explicitly instead of silently skipping OCR targets.
- Release-size tuning for the lite executable.

Not yet fully ported:

- Production monitoring lifecycle: app lifecycle hooks, background behavior,
  and full UI workflow wiring.
- Full DWM preview parity across minimize, tray hide/show, resize, scroll, and
  WebView2 window-layer behavior.
- OpenCV/Python performance comparison for representative production
  multi-template scans; fixed synthetic parity numbers are recorded, but real
  workload parity is not yet proven.
- A recorded real-model OCR smoke pass for the native `pure-onnx-ocr` backend
  using actual external model assets and at least one supplied recognition PNG.
- Native PP-OCRv6/RapidOCR-compatible inference backend for the exact Python
  default model profile, if that remains the preferred full flavor model set.
- Remaining packaged Windows tray smoke verification for tray show/exit menu
  behavior and monitoring icon/tooltip state.

## Source/Evidence Notes

Official docs and project references behind this decision:

- PyInstaller operating mode documents that bundled apps include the Python
  interpreter and collected dependencies: https://pyinstaller.org/en/stable/operating-mode.html
- Tauri documents the WebView-based app model and small native shell direction:
  https://v2.tauri.app/
- Tauri's current start page explicitly lists smaller bundle size through the
  system native WebView: https://v2.tauri.app/start/
- Rust official site positions Rust as native, reliable, efficient systems
  software: https://www.rust-lang.org/
- Go official docs describe building native executables with the Go toolchain:
  https://go.dev/doc/tutorial/compile-install
- Wails positions itself as Go plus web technologies and a lightweight Electron
  alternative: https://wails.io/docs/introduction/
- Microsoft Native AOT docs explain .NET ahead-of-time native publishing:
  https://learn.microsoft.com/dotnet/core/deploying/native-aot/
- Electron's official docs state that it embeds Chromium and Node.js, which is
  excellent for compatibility but not ideal for the smallest package:
  https://electronjs.org/docs/latest
- ONNX Runtime is still the practical native inference runtime family for the
  future full OCR build: https://onnxruntime.ai/docs/
- RapidOCR/PaddleOCR/ONNXRuntime remain relevant for the full OCR module, but
  their models and runtime should be external or optional for the small build.

Community discussions generally align with the practical result measured here:
PyInstaller is convenient but heavy for scientific/computer-vision stacks;
Tauri/Rust is a common choice when installer size matters; Go/Wails is also
reasonable but usually not smaller than a tuned Rust/Tauri binary; C++/Zig can
win on raw size but cost more in maintenance and feature-port risk.

Community samples used only as secondary evidence:

- Tauri/Rust users frequently cite large size differences against Electron in
  real apps, but these numbers vary by frontend and assets:
  https://www.reddit.com/r/rust/comments/1nvvoee/built_a_desktop_app_with_tauri_20_impressions/
- .NET Native AOT discussions show that AOT can help deployment shape but is not
  automatically the smallest choice once UI stacks and native DLLs are included:
  https://www.reddit.com/r/AvaloniaUI/comments/1nh6koy/why_is_the_nativeaot_executable_actually_larger/
