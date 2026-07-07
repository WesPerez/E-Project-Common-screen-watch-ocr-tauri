# Real Functional Acceptance Checklist

This checklist is derived from the existing Python implementation and its 98
baseline tests in `E:\Project\Common\screen-watch-ocr\tests\test_core.py`.

The Python app remains the behavioral reference until every item below has a
Rust unit test, a Tauri command test, or a manual Windows verification gate.
The remaining manual gates have an executable runbook in `docs\MANUAL_GATES.md`;
the verifier checks that the runbook keeps the required commands and evidence
notes.

## Baseline Gate

- The Python suite must keep passing:

```powershell
cd E:\Project\Common\screen-watch-ocr
$env:PYTHONPATH = "src"
python -m unittest discover -s tests -v
```

- Current discovered count from source: 98 `test_` methods.
- Current observed baseline from this migration work: 98 tests passing.
- Current locked baseline names: 98 entries in `docs\PYTHON_BASELINE_TESTS.txt`;
  the verifier allows additional Python tests but fails if any locked baseline
  name disappears.
- The old Python app and the new Rust/Tauri app must keep sharing the same
  app-data directory name and compatible JSON shapes.

## Compatibility Invariants

- App-data directory remains `ScreenWatchOCR`.
- Legacy sibling `app_data` can be copied into `ScreenWatchOCR` without
  deleting the source or overwriting existing shared files.
- Profiles remain `profiles/profile_1.json` through `profiles/profile_5.json`.
- State remains `state.json`.
- `state.json` keeps Python-compatible snake_case keys on disk, including
  `last_profile`; Tauri command results expose frontend-facing camelCase keys
  such as `lastProfile` without rewriting the stored state payload.
- The GUI screenshot retention setting remains the Python-compatible global
  `state.json` `max_alerts` value; profile saves must not persist a
  per-profile `match.max_alerts` field, though old Tauri-written values are
  tolerated when reading and cleaned on the next profile save.
- Templates remain under `templates/`.
- Alert screenshots remain under `screenshots/` when using GUI profiles.
- Alert JSONL remains `alerts.jsonl` for GUI profiles.
- CLI/demo configs using `evidence/alerts` and `evidence/alerts.jsonl` remain
  readable.
- Target ids remain stable across rename/reorder/hit-count updates.
- Unknown JSON fields must not break forward/backward compatibility.
- Frontend Tauri command calls must remain registered on the backend; the
  verifier fails if `src\main.js` invokes a command missing from
  `generate_handler![...]`.
- Frontend Tauri command argument keys must match backend command parameters
  after Tauri's snake_case-to-camelCase bridge, excluding injected
  `tauri::Window` and `tauri::State` parameters.
- Hard-coded frontend `#id` selectors in `src\main.js` must exist in the root
  `index.html`, so renamed controls cannot silently lose event binding or data
  binding during migration.
- Static frontend action controls must remain wired: root `index.html` buttons
  need matching click handlers in `src\main.js`, and static selects/checkboxes
  need matching change handlers.
- Dynamic profile target cards must keep click-to-select/open, right-click
  hit-count menu, enabled checkbox, row button, drag/drop reorder, and profile
  edit command paths wired.
- Legacy visible Python workflows must continue mapping to real Tauri UI paths:
  profile slot, startup toggle, image upload/paste/capture, delete/clear target,
  target enable/invert/open/reorder/hit-menu, screen/window source selection,
  source preview, region/match settings, start/stop monitoring, one-shot scan,
  event log, and evidence-directory opening must each have a visible control,
  frontend handler, and registered backend command where applicable.
- Backend Tauri commands must remain reachable; the verifier fails if a
  `#[tauri::command]` function is missing from `generate_handler![...]` or if
  the handler list contains a non-command function.
- Frontend monitoring-event listeners and backend monitoring-event emitters
  must use the same `MONITOR_SESSION_EVENT` contract; the verifier fails if the
  event name, frontend `listen(...)`, or backend `emit(...)` path drifts.
- Monitor-session event payloads must keep the frontend-facing camelCase JSON
  contract, including `kind`, `tickHitCount`, `tickError`, and snapshot fields
  such as `lastTick`, `hitCount`, `errorCount`, `skippedWindows`,
  `skippedWindowApps`, and `pollIntervalMs`.
- `app_info` and OCR readiness payloads must keep the frontend-facing camelCase
  JSON contract, including `buildFlavor`, `modelsReady`, `backendReady`,
  `requiredModels`, `referenceModels`, and per-model `name`/`path`/`exists`/`bytes`
  fields.
- The real-model OCR smoke script must stay aligned with the Rust OCR constants
  for `SCREENWATCH_OCR_MODEL_DIR`, the default `ScreenWatchOCR\models\rapidocr`
  external model directory, required native model filenames, and missing-model
  preflight output. The missing-model path must also remain executable through
  `scripts\ocr-smoke.ps1 -SelfTestMissingModels`, not just statically present.

## Detection Gates

| Gate | Python evidence | Rust/Tauri target |
| --- | --- | --- |
| Parse scale strings, numeric lists, ranges, percent ranges, and cap expansion | `test_parse_scales` | Done in `screen-watch-core::config` |
| Pixel target matches RGB tolerance and target identity | `test_template_and_pixel_demo`, detector config tests | Done in `screen-watch-core::detect` |
| Template matching supports scaled templates | `test_detector_matches_scaled_template_from_range` | Core backend done, including common image template loading for PNG/JPG/JPEG/BMP/WEBP. Profile/app integration now has a cross-layer gate: `scan::tests::profile_watch_config_scans_scaled_template_through_engine` reads a profile template from `templates/`, builds a GUI-profile `WatchConfig` with scaled matching, runs `ScanEngine`, writes evidence, and verifies the scaled hit |
| Large-frame template path uses coarse/refine without missing targets | `test_detector_matches_template_on_large_frame_fast_path`, `test_detector_large_frame_does_not_skip_unaligned_templates`, `test_detector_does_not_miss_when_coarse_template_loses_detail` | Done in `screen-watch-core::detect` with coarse/refine tests; exact flat templates now use integral scoring, template jobs share gray/scale frames, frame integrals are cached across coarse/refine passes, prepared detectors store gray templates instead of converting every frame, pure flat-template scans stop once a perfect-score window is found, and textured-template NCC uses integral-assisted window mean/energy plus phase-aware sparse coarse candidates so odd-origin small textures are not missed without full NCC at every coarse location. Coarse-refine now keeps the final search margin bounded by downsample step instead of template size, avoiding huge NCC windows for production-scale templates. Ignored benchmark gates verify flat and textured 8-template matches plus a real shared-profile production template set on a 2560x1440 frame |
| Flat and textured templates use appropriate matching behavior | Python `Detector._match_result` | Core scoring backend done. The fixed parity script now covers flat performance and documents Python's odd-phase textured miss behavior while requiring the Rust textured gate to hit all 8 targets |
| Multiple template targets preserve config order and load relative image paths | detector config behavior | Done in `screen-watch-core::detect::PreparedDetector`, including PNG/JPG/JPEG/BMP/WEBP path decoding |
| Multiple template targets respect configured worker limit | `test_detector_uses_configured_template_worker_limit` | Done in `screen-watch-core::detect`; `template_workers` is parsed, clamped, capped to job count, and target order is preserved. `scripts\template-benchmark.ps1` provides opt-in flat/textured multi-template benchmarks with optional max-ms threshold, `scripts\template-parity-benchmark.ps1` compares the fixed workloads against Python/OpenCV, and `scripts\production-template-performance-smoke.ps1` runs the fixed parity gate plus the real-profile production benchmark |
| OCR target is optional and does not break lite builds | Python OCR tests and app config | Done for lite/full architecture and an English PP-OCRv5 real-model smoke: OCR availability/model checks with per-file model status, separated model/backend readiness, Cargo feature boundary for the optional OCR module, shared runtime OCR backend factory, scan-engine backend interface, one-shot/monitoring wiring, explicit disabled/not-compiled/missing-model/backend errors, text-row matching core, and native `pure-onnx-ocr` backend wiring for PP-OCRv5-style `det.onnx`/`rec.onnx`/`ppocrv5_dict.txt`; native backend now runs through a lazy reusable OCR worker so monitoring does not reload the engine every frame, an explicit backend probe validates initialization without starting monitoring, and real-model smoke passed with external models under `target\ocr-model-smoke\monkt-ppocrv5-english` plus `ready-smoke.png` expecting `READY`. `npm run ocr:text:parity` also compares old Python `Detector._ocr` supplied-row behavior against Rust OCR text detection/ScanEngine tests. The smoke script preflights missing model assets with exact paths and setup hints, exposes an executable missing-model self-test, and the verifier locks that preflight script to the Rust OCR constants. PP-OCRv6/RapidOCR-native profile and broad OCR accuracy remain future validation items |
| OCR text matching supports case-insensitive contains and min score | Python `Detector._ocr` | Proven for supplied OCR rows by `npm run ocr:text:parity`: old Python `Detector._ocr` passes case-insensitive contains, min_score miss, case-sensitive miss/hit, concrete box flattening, missing-box behavior, and Unicode contains (`准备好了` contains `准备`), while Rust `screen-watch-core::detect` and `ScanEngine` OCR backend tests pass the aligned cases. Native OCR recognition still has a repeatable ignored smoke gate that requires real external models plus a caller-supplied PNG/expected text |

## Source And Capture Gates

| Gate | Python evidence | Rust/Tauri target |
| --- | --- | --- |
| Empty `regions` defaults to all physical monitors and excludes monitor `0` | `config_regions` behavior | Done in `screen-watch-core::sources` |
| Region bbox is monitor-origin plus configured left/top/width/height | `test_screen_preview_captures_real_frame` and `config_regions` | Done in `screen-watch-core::sources` |
| Unknown monitor ids are rejected clearly | `config_regions` behavior | Done in `screen-watch-core::sources` |
| Tauri lists monitors using Python/mss-compatible indexes including virtual `0` | `list_monitors` behavior | Done in Tauri `list_monitors`; an ignored Windows desktop gate now verifies real Win32 monitor enumeration has virtual monitor `0`, physical monitor indexes starting at `1`, and virtual bounds matching system metrics |
| Screen preview captures a real frame | `test_screen_preview_captures_real_frame` | Windows GDI region capture command done; desktop capture gate passed; full preview workflow pending |
| One-shot screen/window scan captures resolved sources and writes evidence | Python `scan_frames(..., once=True)` | Done for screen regions and concrete app windows in Tauri command/helper; desktop gates passed |
| Persistent screen/window monitoring keeps cooldown/session state | Python `scan_frames` loop / `poll_events` | Tauri session start/stop/status, remembered app refresh, window capture, and frontend event stream done. Backend tests lock zero-tick start/stop event payloads, tick snapshots for skipped direct-window and remembered-app-window sources, and the serialized camelCase monitor-session event JSON contract consumed by the frontend. Monitoring status now surfaces tick hits, skipped sources, latest backend errors, error counts, and heartbeat progress rows in the main status/log UI, covered by frontend unit tests. Frontend monitoring-event transition tests now verify profile refresh on profile-monitoring tick hits, snake/camel event field handling, tick-error precedence, and stopped events clearing profile-monitoring state. Real Windows desktop smoke gates verify persistent screen and window monitoring write evidence, stop cleanly, and record hits. The packaged WebView monitoring restart smoke now captures a real helper window as a template, starts monitoring, observes positive tick/hit counts and progress rows, stops, starts again, and stops with the button restored. The packaged WebView monitoring soak gate now runs the visible profile monitoring workflow for 30 seconds and records 16 UI samples, tick delta 56, hit delta 56, progress-log delta 28, and clean stop-button recovery. The legacy late-start WebView smoke proves a remembered app window can be absent when Tauri loads the old profile, appear later, and then monitor with positive hits without refreshing or reselecting the window |
| Preview frames are reused until source signatures change | `test_screen_preview_frame_is_reused_until_source_changes` | Tauri backend done with source-key/signature preview cache tests; selected screen/window sources now render through a basic multi-source preview panel using cached preview commands, a single auto-refresh timer, and stale-key cache pruning. Window-source cards now attempt DWM overlay handoff while keeping bitmap previews as fallback, the main status text reports failed preview counts after refresh, and frontend tests now lock scheduled/manual refresh gating, DWM bitmap-failure fallback, successful bitmap presentation, stale-image clearing when no DWM fallback exists, and visible WebView frame clipping before DWM sync. Real screen/window preview capture, real DWM API smoke, and packaged real WebView2/CDP visual source-preview smoke now pass with native window screenshots |
| App-window list uses stable title sorting, duplicate ordinals, legacy keys, and display labels | `list_app_windows`, `window_key`, selection tests | Done in Tauri `window_sources`; desktop enumeration gate passed |
| Remembered `window_apps` resolve to currently available concrete windows | `selected_windows`, worker refresh lookup | Done in Tauri command resolver and monitoring loop refresh. The frontend window-resolution action now reports missing remembered app-window counts in the status text, covered by frontend unit tests. A real Windows desktop smoke now starts an external window, verifies enumeration sees it, resolves the remembered `title` plus `ordinal` entry to a concrete hwnd source, captures that window, and stores the frame as a profile template. Frontend source options preserve remembered apps that are not currently listed, and the packaged legacy late-start WebView smoke proves a Python-shaped remembered app source is not lost when the app starts after Tauri |
| Window capture falls back from black PrintWindow output to visible capture | `test_window_capture_falls_back_to_visible_screen_when_printwindow_black` | Done in Tauri `window_capture`; desktop preview/frame gates passed |
| Window capture caches visible-mode fallback after black output | `test_window_capture_caches_visible_mode_after_black_printwindow` | Done in Tauri `WindowCaptureModeCache` |
| PrintWindow black padding is cropped before visible fallback | `test_window_capture_crops_printwindow_black_padding_before_visible_fallback` | Done in Tauri `crop_black_padding` |
| Selected window-only monitoring works without selected monitors | `test_detector_config_allows_window_only_source` | Core source planner, one-shot window scan, persistent window monitoring, and profile window-scan desktop gates passed; full UI workflow pending |
| Missing source is reported instead of crashing | `test_capture_target_frame_reports_missing_source` | Core source planner reports empty sources, direct windows without hwnd are skipped, and remembered apps report missing entries. Tauri one-shot/profile scan, window-resolution, monitoring, screenshot-as-template, and stopped-session status now surface skipped direct-window and remembered-app-window counts or Python-style no-source text in the main status text, covered by backend tick-snapshot/capture-source tests and frontend unit tests |

## Alert And Evidence Gates

| Gate | Python evidence | Rust/Tauri target |
| --- | --- | --- |
| Same region/target obeys cooldown | Python `emit_alert` | Done in `screen-watch-core::evidence` and scan orchestration |
| Different target in same region can alert during cooldown | Python `emit_alert` | Done in `screen-watch-core::evidence` and scan orchestration |
| Alert filenames sanitize unsafe characters | Python `_safe_name` / `safe_name` | Done in `screen-watch-core::evidence` |
| One supplied frame runs detection, cooldown, and evidence writing as a scan step | Python poll/emit path | Done in `screen-watch-core::scan`; one-shot real screen integration done |
| Screenshots are annotated with red boxes and labels | Python `save_rgb` | Done in `screen-watch-core::evidence` with decoded PNG pixel tests |
| Alert JSONL appends event with time, region, matches, screenshot | Python `Alarm.emit`, `App.emit_alert` | Done in `screen-watch-core::evidence` |
| Screenshot pruning keeps newest N png files and ignores non-png | `test_prune_alerts_keeps_newest` | Done in `screen-watch-core::evidence` |
| Beep duration does not restart while already beeping | Python `start_beep` | Done in `screen-watch-core::audio::BeepThrottle` and Tauri `AlarmBeepState`; one-shot scans and monitoring sessions trigger it after alert hits |
| Beep volume clamps and waveform amplitude changes | `test_beep_volume_is_clamped_and_changes_wave_amplitude` | Done in `screen-watch-core::audio` with dependency-free PCM WAV generation and amplitude tests |

## Profile And Template Gates

| Gate | Python evidence | Rust/Tauri target |
| --- | --- | --- |
| Template names use `profile-count-stamp` | `test_template_name_uses_profile_count_date` | Done in `screen-watch-core::profile` |
| Profile paths stay under `profiles/profile_N.json` | `test_profile_roundtrip` | Done in `screen-watch-core::profile` |
| Legacy local `app_data` migrates into shared user data | `test_migrate_legacy_data_maps_alerts` | Done in `screen-watch-core::data_dir` and called once from the Tauri primary startup path; copies profiles/templates/state/alerts into the shared `ScreenWatchOCR` data dir without deleting the legacy source or overwriting existing shared files. `scripts\packaged-smoke.ps1` now stages a packaged exe in a temporary app root with a legacy `app_data` fixture and verifies the migration through the real startup path |
| Template directory remains `templates/` | profile/template tests | Done in `screen-watch-core::profile` |
| Alert directory remains `screenshots/` for GUI | profile/alert tests | Done in path helpers |
| Target identity prefers `id`, then template suffix, then name/path stem | hit count and reorder tests | Done in `screen-watch-core::profile` |
| Profile can be read without rewriting unknown data | `test_profile_roundtrip` and forward-compat behavior | Core `read_profile_at` and Tauri `load_profile` command done; missing profile files return an empty snapshot without creating files, and the frontend profile panel can read current profile state. The legacy profile WebView smoke stages a Python-shaped `profile_1.json` before launch, verifies the visible UI restores the target, remembered app window, region, and match settings without manual reconfiguration, and confirms unknown top-level/target fields survive scan/monitor hit updates. The legacy late-start WebView smoke also verifies old remembered app source data survives an initially missing app window and later scan/monitor hit updates |
| Deleted template number gaps are filled by rename/normalize | `test_normalize_target_names_fills_deleted_number_gap` | Done in `screen-watch-core::profile` |
| Profile normalization removes missing template refs and legacy thumbs | `test_normalize_profile_file_updates_saved_paths` | Done in `screen-watch-core::profile` |
| Reordering templates renames files by new position | `test_reorder_target_renames_files_by_new_position` | Core helper, Tauri `reorder_profile_target` command, and frontend up/down plus drag/drop controls done; frontend tests now cover midpoint-based drag/drop insert indexes, backend-returned selected-index synchronization after profile edits, and a continuous gallery edit selection flow across import, toggle, reorder, remove, re-import, and clear. The Tauri backend gallery workflow test now exercises reorder in sequence with import, enable toggles, delete, and clear-file boundaries. Packaged real WebView2/CDP gallery smoke now exercises row-button and drag/drop reorder with native window screenshots |
| Removing a selected target deletes only template files under `templates/` | `test_remove_selected_deletes_template_file` | Core helper, Tauri `remove_profile_target` command, and frontend delete control done; the Tauri backend gallery workflow test now verifies a removed template file is deleted and its target id disappears while later clear preserves an external file. Packaged real WebView2/CDP gallery smoke now deletes one target through the visible target-card action |
| Clearing all targets deletes only template files under `templates/` | Python `clear_targets` behavior | Core helper, Tauri `clear_profile_targets` command, and frontend clear-all control done; the Tauri backend gallery workflow test now verifies all remaining template files are deleted while an external path is preserved and unknown profile fields survive. Packaged real WebView2/CDP gallery smoke now clears all targets through the visible profile control |
| Adding images prunes to template limit before naming | `test_add_image_prunes_to_template_limit_before_naming` | Backend done in `screen-watch-core::profile` and exposed as Tauri `add_profile_template_pngs`; PNG/JPG/JPEG/BMP/WEBP inputs are decoded and normalized to RGB PNG files under `templates/`. Frontend supports path-based image import, native image selection, clipboard image/path paste, max-template limits, import result feedback, and backend-returned selected-index synchronization so imported targets can be selected immediately. Frontend tests now lock text-path parsing, blank-line filtering, native picker array handling, order preservation, max-template clamping before backend import, paste shortcut focus rules, added/pruned/target-total status text after import, and selected-index fallback/clearing rules. Native picker buffer parsing and clipboard DIB decoding have Tauri tests, the Tauri backend gallery workflow test imports three in-memory frames into profile templates before exercising gallery edits, packaged real WebView2/CDP gallery smoke imports generated PNG paths through the visible UI, and packaged clipboard smoke now verifies CF_DIB bitmap paste through `粘贴图片` plus CF_HDROP file-list paste through `Ctrl+V` with compact rendered target thumbnails |
| Capture current source as a template | `test_capture_target_frame_uses_selected_window_without_monitor`, `test_capture_target_frame_reports_missing_source` | Tauri `capture_profile_source_template` captures the first selected screen region, or the first selected/resolved app window when no screen region is selected, then writes the frame through the same PNG-normalized profile import path. Backend tests cover empty-profile capture source selection, remembered-app resolution, and no-source errors; frontend tests cover allowing this action without existing enabled templates. Real Windows desktop smokes now capture screen, direct-window, and remembered-app-window frames, then write each into a profile template through the same backend helper. Packaged real WebView2/CDP gallery smoke clicks the visible screenshot-as-template button after clearing the gallery and verifies a new target appears |
| Profile detector config uses only checked/enabled targets | `test_detector_config_uses_only_checked_targets` | Core helper and Tauri `build_profile_watch_config`, `scan_profile_once`, and `start_profile_monitoring_session` commands done; disabled targets are skipped, missing `enabled` defaults to true, GUI defaults use `screenshots`/`alerts.jsonl`, and missing templates/enabled targets/sources are rejected. Frontend can now select physical screens and app windows, choose remembered `window_apps` or concrete `windows`, preview the generated config, run one-shot profile scans, start profile monitoring, restore `state.json` `last_profile`, persist Python-compatible `monitors`/`windows`/`region`/`match` settings, and keep GUI `max_alerts` global in `state.json` rather than per profile. Core tests now lock frontend camelCase option deserialization, concrete window ordinal preservation, remembered-window persistence precedence, mixed concrete/remembered source config boundaries, and removal of stale `match.max_alerts` on save. Desktop profile screen and profile-monitoring smokes now capture a real screen region, import a cropped template into a profile, scan/monitor the same screen region, write evidence, and record the profile target hit count; a profile window workflow smoke creates a temporary Win32 window, imports a cropped window template, scans that real window source, writes evidence, and records `hit_count`. The packaged WebView one-shot scan smoke now drives the visible `扫描一次` button against an isolated generated app-window source and verifies positive `hitCount`, zero skipped selected window sources, alert evidence, and target hit-count refresh. The legacy profile WebView smoke starts from old Python-shaped profile/template/state files and proves scanning plus monitoring hit without changing the loaded configuration. The legacy late-start WebView smoke proves the same old remembered app configuration stays runnable while missing, then resolves and hits after the app starts later without refreshing or reselecting |
| Target hit counts persist and update matching ids | `test_record_target_hits_updates_matching_template_counts`, `test_poll_events_records_target_hit_counts` | Done in `screen-watch-core::profile`; Tauri commands exposed; profile one-shot scans and profile monitoring sessions now record stable target ids only from cooldown-filtered alerted matches, and the frontend refreshes the active profile after profile-monitoring hit events. Real Windows desktop smokes verify profile monitoring and one-shot scan workflows write evidence and increment `hit_count`; the packaged WebView one-shot scan smoke verifies the visible scan path updates the profile target hit badge and stored `hit_count` |
| Target hit counts can be cleared by stable id | `test_clear_target_hit_count_resets_badge_count` | Done in `screen-watch-core::profile`; Tauri command returns the edited target list plus `selectedIndex`, so clearing by stable id keeps the same target selected like Python even when the count was already zero. Frontend tests now cover hit-count parsing, context-menu clear enablement, and unchanged clear-result selection |

## Window, Preview, And DWM Gates

| Gate | Python evidence | Rust/Tauri target |
| --- | --- | --- |
| DWM preview registers and reuses thumbnails | `test_dwm_preview_registers_and_reuses_thumbnail` | Basic Windows/Tauri backend done: `DwmPreviewState` registers by source key and reuses entries for the same destination/source hwnd pair; command path is wired to source preview cards. A fake DWM backend verifies registration/update reuse without a real desktop window, the real Windows desktop smoke creates two temporary Win32 windows, registers a real DWM thumbnail, verifies reuse on update, and clears it, and packaged real WebView2/CDP source-preview smoke now drives the visible source-card path with native screenshots |
| DWM sync loop schedules only once | `test_dwm_sync_loop_schedules_once` | Basic Tauri behavior done through the existing single frontend preview timer; DWM rects are refreshed on that path. Full desktop loop smoke pending |
| DWM preview falls back when widget is not visible | `test_dwm_preview_falls_back_when_widget_not_visible_yet` | Frontend computes visible card rects through the tested `visiblePreviewRect` helper, clips partially offscreen WebView frames, and skips DWM sync when the frame or viewport is hidden/tiny; bitmap previews remain the fallback. Frontend tests also verify that a card already handed off to DWM stays healthy if the bitmap capture path fails. Desktop smoke pending |
| DWM preview unregisters before hide/minimize/layout drag | DWM suspend/hide tests | DWM overlays are cleared on preview disable, hidden page, scroll/resize, source-card rerender, stale-source retention, and Tauri close/destroy events. Fake-backend tests now verify stale-key retention, source-handle replacement, and clear-time unregister behavior. Tray/minimize desktop smoke pending |
| Restore overlay protects black canvases during taskbar restore | restore overlay tests | Basic Tauri frontend done: source preview frames are covered during preview disable, hidden/visible restore, resize, scroll, and window focus, then cleared per card after preview refresh. Covered by Node frontend tests, and packaged real WebView2/CDP source-preview smoke now scrolls, resizes, restores, and refreshes source cards without stale/error cards |
| Preview layout waits while resize/drag/layout is busy | layout busy tests | Tauri frontend gate done for source previews: scheduled refreshes skip while the page is hidden, window resize is settling, or template drag/drop is active, then retry on the next timer. Frontend tests now cover disabled scheduled ticks, manual refresh before timer enablement, active-refresh retry, and layout-busy retry behavior. The packaged WebView layout resize smoke now drags visible splitters and verifies the layout stays usable without horizontal overflow |

## Tray, Startup, And Single-Instance Gates

| Gate | Python evidence | Rust/Tauri target |
| --- | --- | --- |
| Close hides to tray instead of exiting | hide-to-tray tests | Backend done in Tauri `tray` module; close requests are prevented only when a tray icon is available, then the main window is hidden. `scripts\packaged-smoke.ps1` now launches the packaged exe, posts `WM_CLOSE` to the visible main window, verifies the process stays alive, and verifies the main window is hidden |
| Tray unavailable re-enables previews and keeps app visible | `test_hide_to_tray_reenables_previews_when_tray_unavailable` | Backend done in Tauri `tray` module; `--start-minimized` hides only if tray creation succeeds, otherwise the window remains visible |
| Tray icon reflects monitoring state | Python `tray_image`, update tests | Backend done in Tauri `tray` module; monitoring events update tray tooltip and generated RGBA icon state. Backend tests now lock show/exit menu IDs, labels, and tray left-click routing. `scripts\tray-menu-smoke.ps1` now proves the packaged tray icon belongs to the Tauri PID via `tray_icon_app` plus `Shell_NotifyIconGetRect`, opens the native `#32768` tray menu, clicks `Show Tauri`, verifies the main window appears, then clicks `Exit Tauri` and verifies process exit code 0 |
| Start minimized stays in tray | `test_main_start_minimized_stays_in_tray` | Backend done in Tauri `tray` module; the legacy `--start-minimized` argument is recognized and hides the main window only after tray install succeeds. `scripts\packaged-smoke.ps1` launches the packaged exe with an isolated app-data directory and single-instance port, then verifies it stays running without a visible main Tauri window |
| Startup shortcut uses `--start-minimized` | `test_startup_arguments_start_minimized` | Done in Tauri `startup` module; Windows Startup `.lnk` path uses the distinct `屏幕监控OCR Tauri.lnk` name, packaged app arguments use `--start-minimized`, and explicit status/toggle commands are exposed |
| Second instance wakes existing app and exits | `test_main_exits_when_existing_instance_accepts_wake`, `test_single_instance_notification_wakes_existing_app` | Done in Tauri `single_instance` module; uses the distinct Tauri TCP protocol `127.0.0.1:47628`, `ScreenWatchOCRTauri:show\n`, and `ok\n`, exits the second Tauri process after notifying an existing Tauri instance, and shows/unminimizes/focuses the main window on wake. The claim path now retries notification once when bind reports the Tauri port is already in use, covering an existing-instance port race before returning unavailable. Packaged smoke now launches a second exe instance, verifies it exits with code 0, and verifies the first instance's main window is restored after close-to-tray hiding without touching an old Python instance |

## UI And Workflow Gates

| Gate | Python evidence | Rust/Tauri target |
| --- | --- | --- |
| Entry clicks keep cursor at end | `test_entry_click_keeps_cursor_at_end` | Basic Tauri frontend done for single-line inputs through `src/ui-behavior.js`, covered by Node frontend tests |
| Custom check indicators scale | `test_custom_check_indicator_scales` | Basic Tauri frontend done through CSS-variable driven custom checkbox indicators in `src/ui-behavior.js`, using the Python `max(12, int(13 * scale))` size contract and covered by Node frontend tests |
| Window resize preserves panes and geometry | resize/layout tests | Tauri now restores Python-compatible `state.json` `layout.geometry` on main-window setup and saves visible main-window outer geometry on resize/move/scale changes while preserving existing layout ratios and unknown state fields; minimized/hidden taskbar placeholder geometry is ignored. The packaged smoke stages a legacy `state.json`, migrates it, and verifies the real packaged main window opens near the migrated `980x680+20+30` geometry, allowing DPI-virtualized probe coordinates. The frontend behavior layer mirrors Python's hidden/iconic geometry guard, visible geometry formatting, ratio capture, bounded side-pane width, horizontal sash restoration, withdrawn-pane retry planning, vertical-only restore planning, layout-busy gating, same-size configure/move detection, and adjacent multi-pane settings resizing. The packaged WebView layout resize smoke now drags the target/settings splitter, settings/preview splitter, target-list/log splitter, and a control-panel group splitter, then verifies measured dimension deltas and no horizontal overflow |
| Profile source selection keeps Python-compatible monitor/window shapes | profile source persistence tests | Basic Tauri frontend done: selected physical monitors exclude virtual monitor `0`, screen preview sources apply region offsets while profile region configs remain monitor-relative, concrete windows preserve `title`/`display`/`hwnd`/`ordinal`, remembered app windows persist as `windowApps`, and no-source detection is shared before profile scan/monitor actions. The frontend now also sends a separate `profileRegion` so the region input boxes are saved like Python even when no monitor is selected and the profile is window-only; core tests verify that window-only saves update `profile.json` `region` instead of leaving stale values. Profile workflow actions now also block missing enabled templates and duplicate profile-monitoring starts before invoking backend commands. Covered by Node frontend tests, desktop backend smokes, WebView source/gallery smokes, and the packaged monitoring restart smoke |
| Autohide scrollbars map only when needed | `test_autohide_scrollbar_only_maps_when_needed` | Basic Tauri frontend done for the target gallery through `src/ui-behavior.js`: it toggles hidden/visible scrollbar state from `scrollHeight > clientHeight`, updates on resize, and is covered by Node frontend tests. Packaged WebView gallery and layout smokes exercise the visible target-list area after resize |
| Gallery cards keep bottom border visible | `test_target_card_keeps_bottom_border_visible` | Basic Tauri target cards now use fixed-size thumbnail slots and hit-count badges; packaged real WebView2/CDP gallery smoke records native screenshots and JSON state verifying target-card thumbnail dimensions and bottom borders |
| Gallery mousewheel scrolls canvas | `test_gallery_mousewheel_scrolls_canvas` | Basic Tauri target list wheel scrolling done through `src/ui-behavior.js`, covered by Node frontend tests; packaged WebView layout smoke verifies the target-list/log splitter keeps the visible list usable after resize |
| Dynamic target card actions stay wired | target gallery action workflow | `scripts\verify-migration.ps1` now locks the dynamically rendered profile target card paths for click-to-select/open, right-click hit-count menu, enabled checkbox, row buttons, drag/drop reorder, and profile edit backend commands. Packaged real WebView2/CDP gallery smoke now exercises those visible action paths end to end |
| Toggle all targets switches select-all/invert behavior | toggle/select tests | Core helper, Tauri `toggle_all_profile_targets` command, and frontend profile-panel/list control done; profile target row action tests now also lock up/down reorder insert indexes, first/last target move disabling, and Python-style enabled/total-count status text after select-all/invert or single-target enabled changes. The Tauri backend gallery workflow test now verifies disable-one, select-all, and clear-all transitions before reorder/delete/clear. Desktop smoke pending |
| Selecting a target does not redraw the full gallery | selection tests | Basic Tauri frontend done: clicking a target updates only the previous and newly selected target card classes through `src/ui-behavior.js`, normal profile loads/switches default to the first target like Python, and profile refreshes preserve or clear the current selection without silently selecting a different target. Node frontend tests now also lock the selected-index evolution across a full profile gallery edit workflow, so import/reorder/remove/clear backend responses cannot leave the UI pointing at a stale target. Packaged real WebView2/CDP gallery smoke records selection state across import, reorder, delete, clear, and capture |
| Same-card second click opens image directly | `test_click_target_opens_on_second_click_without_preselect`, `test_open_target_file_opens_image_directly` | Tauri `open_profile_target_file` validates profile index and existing PNG path, `profile_target_thumbnail` reuses that guard before reading PNG thumbnails, and frontend target rows now use explicit same-card repeat-click detection in `src/ui-behavior.js` instead of relying on browser `dblclick` ordering. Desktop smoke pending |
| Right-click opens hit-count menu and clear action works | hit-count menu tests | Hit-count clear command, frontend hit badges, frontend button, and target-card right-click menu done; frontend applies the backend returned selection after clearing counts. Frontend tests cover menu open/clear enablement, row action state, unchanged clear-result selection, and viewport fitting. Packaged real WebView2/CDP gallery smoke opens the visible context menu, captures it, and clears the hit count |

## Packaging Gates

| Gate | Evidence | Rust/Tauri target |
| --- | --- | --- |
| Lite build runs without OCR models | Rust OCR tests | Done |
| Lite build excludes optional OCR module by default | Cargo feature checks, verifier metadata gate, and dependency-tree gate | Done: default features do not compile the `ocr` feature, full builds/checks use `--features ocr`, and `scripts\verify-migration.ps1` fails if Cargo metadata stops keeping `pure-onnx-ocr` optional, if Tauri enables `screen-watch-core/ocr` by default, if lite resolves `pure-onnx-ocr`/`tract-onnx`, or if full stops resolving them |
| Full build reports missing external OCR models clearly | Rust OCR tests | Done, including per-required-model existence and byte status |
| Full build keeps OCR unavailable until native inference is linked | Rust OCR tests | Done: default/lite keeps `backendReady=false`; full `ocr` feature links `pure-onnx-ocr` and requires compatible native assets before `available=true` |
| App UI exposes OCR model readiness for setup troubleshooting | Frontend build, app-info data binding, and verifier static contract | Done: app-info view renders readiness flags, backend name, model profile, and required model file status/path/size; Tauri backend tests lock the serialized `app_info`/OCR readiness camelCase contract consumed by the frontend, and `scripts\verify-migration.ps1` locks the frontend DOM/rendering path for OCR availability flags, model directory, required model list, per-model ready/missing state, path, and byte display |
| App UI can explicitly probe OCR backend initialization | Rust OCR tests, Tauri command, frontend build, and verifier static contract | Done: `ocr_backend_probe` skips unavailable states and attempts native initialization only when models and backend are both present; the verifier locks the frontend button binding, backend invoke, and rendered probe fields for attempted/initialized/reason/error/backend/profile/modelDir |
| Full runtime flavor reports a missing compiled OCR module clearly | Rust OCR tests | Done |
| Full build does not embed models by default | Rust OCR model-dir policy | Done: native full backend still expects external assets |
| Packaged build flavor cannot be changed by user runtime environment | Rust build-flavor tests and verifier static gate | Done: `BuildFlavor::from_env()` now reports the compile-time packaged flavor. The core crate build script tracks `SCREENWATCH_BUILD_FLAVOR`, emits normalized `SCREENWATCH_COMPILED_BUILD_FLAVOR`, and `scripts\verify-migration.ps1` fails if the runtime build-flavor module starts reading environment variables again or if the build script stops tracking/emitting the packaged flavor. The verifier also locks `npm run tauri:build:lite/full` to `scripts\build-tauri.mjs`, including the full-build `--features ocr` handoff, release build-info/hash sidecar, flavor-specific build-info sidecars, and flavor-specific NSIS installer copies |
| Critical npm command entrypoints cannot drift | verifier static contract gate | Done: `scripts\verify-migration.ps1` locks the `package.json` scripts for frontend tests, migration verification, OCR smoke, template benchmarks/parity, production template smoke, packaged smoke, WebView monitoring restart/soak smokes, portable lite/full packaging, and Tauri lite/full builds, and verifies the referenced script files still exist |
| Lite exe remains far below Python/PyInstaller baseline | measured release exe size and verifier guardrail | Done: `scripts\verify-migration.ps1` reports the checked lite artifact as `liteExeBytes` and fails by default if that lite exe exceeds 15 MiB or 25% of the Python/PyInstaller exe size when both exes are present; current `tauriExeBytes` is reported separately so full package builds cannot be confused with a lite-size pass |
| Real/manual smoke gate entrypoints cannot disappear silently | verifier required gate names | Done: `scripts\verify-migration.ps1` fails if any flat/textured/production-profile large-template benchmark, any of the 16 desktop smoke gates, or either OCR real-model gate is missing from the relevant Cargo test output |
| Remaining manual gate runbook cannot disappear or become vague | verifier static contract gate | Done: `docs\MANUAL_GATES.md` records prerequisites, commands, and evidence expectations for desktop smoke, real OCR model smoke, WebView source preview visual smoke, template gallery visual workflow, profile monitoring restart smoke, profile monitoring soak smoke, WebView layout resize smoke, packaged app smoke, packaged tray menu/icon smoke, installer repeatability, and production template performance; `scripts\verify-migration.ps1` fails if required sections, commands, or evidence markers are removed |
| Remaining manual gate evidence cannot be incomplete at final signoff | `scripts\manual-gate-evidence.ps1` plus verifier self-test | Done for tooling: `npm run manual:evidence -- -New` creates one evidence record per manual gate, `npm run manual:evidence -- -Status` reports the current pass/blocked/fail/missing/incomplete/invalid count, `npm run manual:evidence` requires every record to be present, fully filled, and marked `pass`, and `scripts\verify-migration.ps1` runs the tool's self-test so blocked/fail records cannot be accepted as final completion |
| Portable package external-model contract cannot drift from Rust core | verifier static contract gate | Done: `scripts\verify-migration.ps1` checks that `scripts\package-portable.ps1` still agrees with the Rust core on `ScreenWatchOCR`, `SCREENWATCH_OCR_MODEL_DIR`, `%LOCALAPPDATA%\ScreenWatchOCR\models\rapidocr`, and the required native OCR asset filenames |
| OCR smoke preflight contract cannot drift from Rust core | verifier static contract gate plus executable missing-model self-test | Done: `scripts\verify-migration.ps1` checks that `scripts\ocr-smoke.ps1` still agrees with the Rust core on `SCREENWATCH_OCR_MODEL_DIR`, default `ScreenWatchOCR\models\rapidocr` model location, required native OCR asset filenames, preflight output, smoke image env vars, and preflight-before-Cargo ordering. It also runs `scripts\ocr-smoke.ps1 -SelfTestMissingModels` so the missing-model path must execute and report every required asset before a real-model OCR run is attempted |
| Portable lite/full zip can be produced and content-verified without installer tool downloads | `scripts\package-portable.ps1` | Done: package includes exe, build-info sidecar, manifest, and README while keeping OCR models external; packaging verifies archive root, exe bytes, exe SHA-256, build-info/manifest flavor agreement, README presence, required OCR model list, and absence of bundled `.onnx` files or required OCR asset filenames. `-SkipBuild` now requires a matching release build-info sidecar before packaging an existing exe |
| Tauri bundle config keeps installer assets and model policy stable | verifier static contract gate | Done: `scripts\verify-migration.ps1` checks `src-tauri\tauri.conf.json` for `../dist` frontend assets, hidden initial main window, current NSIS bundle target, and absence of bundled OCR model resources/external binaries |
| Installer can be built repeatably when NSIS/WiX tools are available | Tauri build/install smoke and evidence record | Done: `npm run tauri:build:lite` and `npm run tauri:build:full` produced NSIS installers, preserved flavor-specific installer/build-info artifacts, installed the renamed Tauri lite/full installers silently into `target\installer-smoke-tauri-identity-20260706-234853\lite` and `target\installer-smoke-tauri-identity-20260706-234853\full`, passed installed packaged-smoke for both flavors, and no OCR model assets were found in installer or install-smoke directories |
| Build artifacts are clearly separated from shipped app | size audit | Done in docs |

## Tracked Real And Manual Gate Names

`scripts\verify-migration.ps1` requires these real/manual gate test names to
remain both present in Cargo test output and named here, so the executable
entrypoints and this functional checklist cannot drift apart.

Workspace/template gates:

- `benchmark_large_frame_many_template_scan`
- `benchmark_large_frame_textured_template_scan`
- `benchmark_production_profile_template_scan`
- `captures_tiny_screen_region_on_windows_desktop`
- `real_windows_monitor_listing_matches_python_mss_indexing_on_desktop`
- `one_shot_scan_captures_screen_region_and_writes_evidence`
- `profile_screen_scan_workflow_records_template_hit_on_windows_desktop`
- `profile_screen_capture_template_writes_real_desktop_frame_on_windows_desktop`
- `profile_window_capture_template_writes_real_window_frame_on_windows_desktop`
- `profile_remembered_window_capture_template_resolves_and_writes_frame_on_windows_desktop`
- `profile_monitoring_session_records_template_hit_on_windows_desktop`
- `profile_window_scan_workflow_records_template_hit_on_windows_desktop`
- `one_shot_scan_captures_window_and_writes_evidence`
- `session_start_runs_ticks_and_stop_joins_worker`
- `session_start_scans_window_source_and_writes_evidence`
- `list_app_windows_enumerates_without_panic_on_windows_desktop`
- `capture_first_app_window_preview_on_windows_desktop`
- `capture_first_app_window_frame_on_windows_desktop`
- `real_dwm_thumbnail_registers_updates_and_clears_on_windows_desktop`

OCR real-model gates:

- `native_ocr_real_model_probe_initializes_from_external_assets`
- `native_ocr_real_model_recognizes_smoke_png`

## Required Completion Rule

The migration is not complete when the Rust app merely launches. Completion
requires either:

- A Rust/Tauri automated test that covers each gate above, or
- A documented manual Windows verification result for gates that require real
  monitor/window/tray/startup behavior.

Until then, the Python app remains the production baseline.
