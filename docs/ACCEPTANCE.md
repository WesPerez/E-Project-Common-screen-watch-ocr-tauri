# Acceptance Checklist

This checklist is the migration guardrail. The Python app currently has 98 unittest
cases passing and remains the reference implementation until every item here is
covered by Rust tests or an explicit manual verification script.

Detailed real-workflow gates are tracked in [FUNCTIONAL_ACCEPTANCE.md](FUNCTIONAL_ACCEPTANCE.md).

## Baseline

- Python baseline command: `PYTHONPATH=src python -m unittest discover -s tests -v`
- Python test method count from source: 98.
- Observed baseline: 98 tests passing.
- Python/PyInstaller executable observed at `E:\Project\Common\screen-watch-ocr\dist\ScreenWatchOCR.exe`:
  102,021,797 bytes, about 97.3 MiB.
- Existing Python config/data directory and JSON formats are compatibility contracts,
  not cleanup targets.
- The migration verifier statically inventories `tests/**/*.py` `test*`
  functions and requires the unittest run count to match that inventory, so new
  Python baseline tests cannot be silently missed by discovery.
- `docs\PYTHON_BASELINE_TESTS.txt` locks the current 98 Python acceptance test
  names. The verifier allows extra Python tests later, but every locked name
  must remain discoverable.

## Current Rust/Tauri Verification

- `scripts\verify-migration.ps1` is the repeatable migration gate. It runs the
  static Python test inventory check, Python baseline count check, Rust
  formatting, Cargo OCR feature-boundary and dependency-tree checks, Rust
  workspace tests, OCR feature tests/checks, frontend unit tests, frontend production build,
  no-bundle Tauri lite app build, and exe size reporting. When a Tauri release
  exe exists, it also enforces the
  lite-size guardrail: by default the exe must stay under 15 MiB and under 25%
  of the Python/PyInstaller exe size when that baseline exe is present. The
  verifier reports the checked lite artifact as `liteExeBytes` separately from
  the current release exe `tauriExeBytes`, so a later full package step cannot
  be mistaken for a lite-size pass. The Cargo metadata gate requires empty
  default features, keeps `pure-onnx-ocr` optional in `screen-watch-core`, and
  only exposes it to Tauri through the explicit `ocr` feature. The dependency
  tree gate requires lite to exclude `pure-onnx-ocr`/`tract-onnx` while full
  includes them. Use
  `-SkipRelease` for faster local loops. Use `-IncludeDesktopSmoke` on an
  interactive Windows desktop to run the 16 ignored real monitor/capture/window/profile-session/DWM
  smoke gates. Use `-IncludeTemplateBenchmark` to run the ignored fixed
  large-frame flat and textured template benchmarks. Use `-IncludePackagedSmoke` to launch the
  packaged exe and verify `--start-minimized`, close-to-tray hiding, and
  single-instance wake restore behavior, plus startup migration from a
  temporary legacy `app_data` fixture into `ScreenWatchOCR` and real-window
  restoration of migrated `state.json` geometry. The default packaged smoke
  wait is 18 seconds. Use
  `-IncludePortablePackage` to verify a portable lite zip package
  and its required contents without relying on NSIS/WiX installer downloads.
  Use `-IncludeFullPortablePackage` to build and verify a full portable zip
  package with the OCR Cargo feature enabled and OCR models still external.
  The verifier parses and enforces the locked Python baseline test names plus
  minimum observed counts for Rust core tests, Tauri shell/backend tests, OCR
  feature tests, and frontend tests, so those suites cannot silently shrink
  while still reporting a green run. It also requires the named real/manual
  gates to remain present in test output: the flat/textured large-template benchmarks, 16
  desktop smoke gates, and 2 OCR real-model gates. The default verifier also
  checks build/package contracts: every frontend `invoke("...")` command in
  `src\main.js` must remain registered in Tauri `generate_handler![...]`, and
  every frontend invoke argument key must match the backend command parameter
  bridge after snake_case-to-camelCase conversion. Hard-coded frontend `#id`
  selectors in `src\main.js` must also exist in the root `index.html`; static
  action controls must remain wired with click handlers for buttons and change
  handlers for selects/checkboxes. Dynamic profile target cards must keep their
  click, right-click menu, checkbox, row button, and drag/drop action paths
  wired to the profile edit commands. The default verifier now also locks a
  legacy-visible-workflow contract so the old Python UI's visible workflows
  continue to map to new Tauri controls, frontend handlers, and registered
  backend commands, including evidence-directory opening. Every
  backend `#[tauri::command]` function must remain registered in
  `generate_handler![...]`, build flavor must remain compile-time
  packaged state instead of a runtime environment override, lite/full npm build
  entrypoints must continue routing through `scripts\build-tauri.mjs` with
  `SCREENWATCH_BUILD_FLAVOR`, full-build `--features ocr`, and release
  build-info/hash sidecar generation intact, Tauri bundle config must keep
  frontend assets under `../dist`, keep the main window initially hidden for
  tray/startup policy, keep the current verified NSIS target, and avoid bundled
  OCR model resources/external binaries, the frontend
  monitoring listener and backend monitoring emitter must agree on
  `MONITOR_SESSION_EVENT`, monitor-session events must update the tray
  tooltip/icon from `snapshot.running` before the frontend event is emitted,
  and source-preview DWM handoff must continue using the tested visible-card
  rect helper with bitmap fallback and per-frame cleanup paths wired. Tauri
  backend tests must keep monitor-session event payloads plus `app_info`/OCR
  readiness JSON fields in the
  frontend-facing camelCase shape. The frontend OCR readiness and backend-probe
  UI must keep rendering model readiness, model paths/sizes, and probe result
  fields. Core tests must also keep `state.json`
  Python-compatible on disk while exposing Tauri command results in frontend
  camelCase. The critical `package.json` command entrypoints must keep routing
  to the expected verifier, frontend, OCR smoke, benchmark, packaged-smoke,
  portable-package, and Tauri lite/full build scripts. The portable package
  script must agree with the Rust core's
  external OCR model contract: shared `ScreenWatchOCR` app-data name,
  `SCREENWATCH_OCR_MODEL_DIR`,
  `%LOCALAPPDATA%\ScreenWatchOCR\models\rapidocr`, and required native OCR
  asset filenames. The OCR smoke script must also agree with the Rust core's
  model directory, `SCREENWATCH_OCR_MODEL_DIR`, required native OCR asset
  filenames, and missing-model preflight output contract. The functional
  acceptance checklist must list every verifier-required real/manual gate test
  name, so source-level smoke entrypoints and the checklist cannot drift apart.
  The remaining manual gate runbook must also keep concrete commands,
  prerequisites, and evidence expectations for the hard gates that need real
  desktop state, real OCR assets, installer tooling, or visual evidence.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1`:
  passed in the full default verification path, including the no-bundle Tauri
  lite release build and executable size guardrail. The summary reported Python
  static inventory `98`, locked Python baseline names `98`, Python unittest
  `98`, Rust core `115 passed, 1 ignored`, Tauri shell/backend `79 passed, 16
  ignored`, OCR feature `23 passed`, frontend `83 passed`,
  `frontendCommandContract: passed`, `frontendCommandArgumentContract: passed`,
  `frontendDomContract: passed`, `frontendActionBindingContract: passed`,
  `backendCommandContract: passed`, `monitorSessionEventContract: passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`,
  `releaseBuildInfo: lite, sha256 recorded`, `releaseBuild: True`,
  `liteSizeGate: passed`, and `liteExeBytes: 3565568`. The Python/PyInstaller
  baseline was `102021797` bytes, so this lite build is about 3.50% of the
  Python executable.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludePortablePackage`:
  passed using the current lite release executable. The verifier rebuilt no
  release artifact, kept `releaseBuildInfo: lite, sha256 recorded`, kept
  `liteSizeGate: passed`, and produced a verified portable lite zip at
  `target\portable\screen-watch-ocr-tauri-lite-portable-20260706-100645-be3dec27.zip`.
  The package was `1,600,420` bytes and its embedded build-info/manifest
  matched the external OCR model contract.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke`:
  passed all 16 interactive Windows desktop smoke gates. The run covered real
  screen capture, Python/mss-compatible monitor listing, one-shot screen scan
  evidence writing, profile screen scan, profile screen template capture,
  direct-window template capture, remembered-window template capture, profile
  monitoring hit persistence, profile window scan, one-shot window scan,
  persistent screen monitoring, persistent window monitoring, app-window
  enumeration, app-window preview capture, app-window frame capture, and real
  DWM thumbnail register/update/clear. The summary reported `desktopSmoke: 16
  gates`, `rustCoreTests: 115 passed, 1 ignored`, `tauriTests: 79 passed, 16
  ignored`, `ocrFeatureTests: 23 passed`, `liteSizeGate: passed`, and
  `releaseBuildInfo: lite, sha256 recorded`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludePackagedSmoke`:
  passed against the current lite release executable. The packaged smoke staged
  a temporary app root and isolated `LOCALAPPDATA`, verified legacy `app_data`
  migration copied 5 files into `ScreenWatchOCR` without deleting the source,
  verified `--start-minimized` kept the app running without a visible main
  window, verified the restored packaged main window matched the migrated
  `980x680+20+30` geometry through a DPI scale of `1.5`, posted `WM_CLOSE` and
  verified close-to-tray hiding, launched a second instance on the isolated
  single-instance port, verified the second instance exited with code `0`, and
  verified the first instance became visible again. The script reported
  `packagedSmokeVerified: True`, `packagedSmoke: ran`,
  `cleanupStoppedProcess: True`, `cleanupStoppedCloseToTrayProcess: True`,
  `cleanupStoppedSecondInstanceProcess: False`, `cleanupRemovedAppData: True`,
  and `cleanupRemovedAppRoot: True`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeFullPortablePackage`:
  passed after building the full no-bundle release with the optional OCR feature
  and packaging it as
  `target\portable\screen-watch-ocr-tauri-full-portable-20260706-101802-11a74d19.zip`.
  The full release executable was `9,515,008` bytes and the verified full
  portable zip was `3,736,479` bytes. The archive verifier confirmed the
  executable, build-info sidecar, manifest, README, full flavor metadata,
  executable bytes/SHA-256, external `ScreenWatchOCR` app-data contract,
  `SCREENWATCH_OCR_MODEL_DIR`, required model filenames, and absence of bundled
  `.onnx` or required OCR model assets.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend`:
  passed immediately after the full portable build to restore the current
  `target\release\screen-watch-ocr-tauri.exe` artifact to lite. The summary
  reported `releaseBuildInfo: lite, sha256 recorded`, `releaseBuild: True`,
  `liteSizeGate: passed`, `liteExeBytes: 3565568`, and `tauriExeBytes:
  3565568`, so the working release artifact is again the lite build after
  verifying the full package path.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the frontend DOM selector, action binding contract,
  dynamic target action contract, and OCR smoke script contract gates. The verifier now
  also parses hard-coded `document.querySelector("#id")` and
  `querySelectorAll("#id")` selectors in `src\main.js` and requires the ids to
  exist in root `index.html`; it also requires static buttons/selects/checkboxes
  to keep their expected event bindings. It checks the dynamically rendered
  profile target cards keep click-to-select/open, right-click hit-count menu,
  enabled checkbox, row button, drag/drop reorder, and profile edit command
  paths wired. It also checks `scripts\ocr-smoke.ps1` against the Rust OCR
  model constants and requires the preflight to run before the real-model Cargo
  probe. The latest summary reported Rust core `115 passed, 1 ignored`, Tauri
  shell/backend `79 passed, 16 ignored`, OCR feature `23 passed`,
  `frontendCommandContract: passed`,
  `frontendCommandArgumentContract: passed`, `frontendDomContract: passed`,
  `frontendActionBindingContract: passed`,
  `frontendDynamicTargetContract: passed`, `backendCommandContract: passed`,
  `monitorSessionEventContract: passed`, `buildFlavorContract: passed`,
  `portableOcrContract: passed`, `ocrSmokeContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`,
  `releaseBuildInfo: lite, sha256 recorded`, `liteSizeGate: passed`,
  `liteExeBytes: 3565568`, and `tauriExeBytes: 3565568`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed with the frontend DOM selector and action binding contracts in the full
  default migration loop. The summary reported Python static inventory `98`, locked
  Python baseline names `98`, Python unittest `98`, Rust core `115 passed, 1
  ignored`, Tauri shell/backend `79 passed, 16 ignored`, OCR feature `23
  passed`, frontend `83 passed`, `frontendCommandContract: passed`,
  `frontendCommandArgumentContract: passed`, `frontendDomContract: passed`,
  `frontendActionBindingContract: passed`,
  `backendCommandContract: passed`,
  `monitorSessionEventContract: passed`, `buildFlavorContract: passed`,
  `portableOcrContract: passed`, `ocrFeatureBoundary: passed`,
  `ocrDependencyTree: lite excludes, full includes`, `requiredRealGates: 17
  workspace gates, 2 OCR gates`, and current release build-info `full, sha256
  recorded`; the lite size gate was skipped because the current release exe is
  a full build. Python/Tk still prints intermittent destroyed-window callback
  noise during teardown, but unittest finishes with `Ran 98 tests ... OK`.
- `cargo test -p screen-watch-core profile_state_result_serializes_frontend_contract_without_changing_state_file_shape`:
  passed after adding the state/profile bridge contract. The test verifies that
  the stored `state.json` keeps Python-compatible `last_profile`, layout
  geometry, and unknown fields, while the Tauri-facing result serializes as
  `lastProfile`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after raising the Rust core baseline to 115 tests. The summary
  reported Rust core `115 passed, 1 ignored`, Tauri shell/backend `79 passed,
  16 ignored`, OCR feature `23 passed`, `frontendCommandContract: passed`,
  `backendCommandContract: passed`, `monitorSessionEventContract: passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed with the state/profile bridge contract in the full default migration
  loop. The summary reported Python static inventory `98`, locked Python
  baseline names `98`, Python unittest `98`, Rust core `115 passed, 1 ignored`,
  Tauri shell/backend `79 passed, 16 ignored`, OCR feature `23 passed`,
  frontend `83 passed`, `frontendCommandContract: passed`,
  `backendCommandContract: passed`, `monitorSessionEventContract: passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build. Python/Tk still prints
  intermittent destroyed-window callback noise during teardown, but unittest
  finishes with `Ran 98 tests ... OK`.
- `cargo test -p screen-watch-ocr-tauri monitor_session_event_serializes_frontend_status_contract_as_camel_case`:
  passed after adding the monitor-session event payload serialization contract.
  The test locks the camelCase fields consumed by the frontend monitoring
  status flow, including event `kind`, `tickHitCount`, `tickError`, and
  snapshot fields such as `lastTick`, `hitCount`, `errorCount`,
  `skippedWindows`, `skippedWindowApps`, and `pollIntervalMs`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after raising the Tauri shell/backend baseline to 79 tests. The
  summary reported Rust core `114 passed, 1 ignored`, Tauri shell/backend `79
  passed, 16 ignored`, OCR feature `23 passed`, `frontendCommandContract:
  passed`, `backendCommandContract: passed`, `monitorSessionEventContract:
  passed`, `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed with the monitor-session event payload contract in the full default
  migration loop. The summary reported Python static inventory `98`, locked
  Python baseline names `98`, Python unittest `98`, Rust core `114 passed, 1
  ignored`, Tauri shell/backend `79 passed, 16 ignored`, OCR feature `23
  passed`, frontend `83 passed`, `frontendCommandContract: passed`,
  `backendCommandContract: passed`, `monitorSessionEventContract: passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build. Python/Tk still prints
  intermittent destroyed-window callback noise during teardown, but unittest
  finishes with `Ran 98 tests ... OK`.
- `cargo test -p screen-watch-ocr-tauri app_info_serializes_frontend_ocr_contract_fields_as_camel_case`:
  passed after adding the `app_info`/OCR readiness serialization contract. The
  test locks the camelCase fields consumed by the frontend, including
  `buildFlavor`, `dataDir`, `modelsReady`, `backendReady`, `backendName`,
  `modelProfile`, `modelDir`, `requiredModels`, `referenceModels`,
  `missingModels`, and each required-model `name`/`path`/`exists`/`bytes`
  object field.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after raising the Tauri shell/backend baseline to 78 tests. The
  summary reported Rust core `114 passed, 1 ignored`, Tauri shell/backend `78
  passed, 16 ignored`, OCR feature `23 passed`, `frontendCommandContract:
  passed`, `backendCommandContract: passed`, `monitorSessionEventContract:
  passed`, `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed with the `app_info`/OCR readiness contract in the full default
  migration loop. The summary reported Python static inventory `98`, locked
  Python baseline names `98`, Python unittest `98`, Rust core `114 passed, 1
  ignored`, Tauri shell/backend `78 passed, 16 ignored`, OCR feature `23
  passed`, frontend `83 passed`, `frontendCommandContract: passed`,
  `backendCommandContract: passed`, `monitorSessionEventContract: passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build. Python/Tk still prints
  intermittent destroyed-window callback noise during teardown, but unittest
  finishes with `Ran 98 tests ... OK`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the monitor session event contract gate. The verifier now
  checks that the frontend and backend `MONITOR_SESSION_EVENT` constants match,
  that `src\main.js` listens with that constant, and that the Tauri backend
  emits with that constant. The summary reported Rust core `114 passed, 1
  ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23
  passed`, `frontendCommandContract: passed`, `backendCommandContract:
  passed`, `monitorSessionEventContract: passed`, `buildFlavorContract:
  passed`, `portableOcrContract: passed`, `ocrFeatureBoundary: passed`,
  `ocrDependencyTree: lite excludes, full includes`, `requiredRealGates: 17
  workspace gates, 2 OCR gates`, and current release build-info `full, sha256
  recorded`; the lite size gate was skipped because the current release exe is
  a full build.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed with the monitor session event contract in the full default migration
  loop. The summary reported Python static inventory `98`, locked Python
  baseline names `98`, Python unittest `98`, Rust core `114 passed, 1 ignored`,
  Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23 passed`,
  frontend `83 passed`, `frontendCommandContract: passed`,
  `backendCommandContract: passed`, `monitorSessionEventContract: passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build. Python/Tk still prints
  intermittent destroyed-window callback noise during teardown, but unittest
  finishes with `Ran 98 tests ... OK`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the frontend command contract gate. The verifier now
  fails if any string-form frontend `invoke("command")` in `src\main.js` is not
  present in the Tauri `generate_handler![...]` list, reducing the risk that a
  migrated UI action survives visually but loses its backend command. The
  summary reported Rust core `114 passed, 1 ignored`, Tauri shell/backend `77
  passed, 16 ignored`, OCR feature `23 passed`, `frontendCommandContract:
  passed`, `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after documenting the same frontend command contract in the default
  migration loop. The summary reported Python static inventory `98`, locked
  Python baseline names `98`, Python unittest `98`, Rust core `114 passed, 1
  ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23
  passed`, frontend `83 passed`, `frontendCommandContract: passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build. Python/Tk still prints
  intermittent destroyed-window callback noise during teardown, but unittest
  finishes with `Ran 98 tests ... OK`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the backend command contract gate. The verifier now
  checks both directions of the Tauri command surface: frontend `invoke(...)`
  calls must be registered, and every backend `#[tauri::command]` function must
  appear in `generate_handler![...]` with no non-command extras. The summary
  reported Rust core `114 passed, 1 ignored`, Tauri shell/backend `77 passed,
  16 ignored`, OCR feature `23 passed`, `frontendCommandContract: passed`,
  `backendCommandContract: passed`, `buildFlavorContract: passed`,
  `portableOcrContract: passed`, `ocrFeatureBoundary: passed`,
  `ocrDependencyTree: lite excludes, full includes`, `requiredRealGates: 17
  workspace gates, 2 OCR gates`, and current release build-info `full, sha256
  recorded`; the lite size gate was skipped because the current release exe is
  a full build.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed with the backend command contract in the full default migration loop.
  The summary reported Python static inventory `98`, locked Python baseline
  names `98`, Python unittest `98`, Rust core `114 passed, 1 ignored`, Tauri
  shell/backend `77 passed, 16 ignored`, OCR feature `23 passed`, frontend `83
  passed`, `frontendCommandContract: passed`, `backendCommandContract:
  passed`, `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  release build-info `full, sha256 recorded`; the lite size gate was skipped
  because the current release exe is a full build. Python/Tk still prints
  intermittent destroyed-window callback noise during teardown, but unittest
  finishes with `Ran 98 tests ... OK`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after making build flavor a compile-time packaged contract and raising
  the Rust core baseline to 114. The summary reported Rust core `114 passed, 1
  ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23
  passed`, `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and checked
  lite exe size `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after the build-flavor contract was included in the full default
  migration loop. The summary reported Python static inventory `98`, locked
  Python baseline names `98`, Python unittest `98`, Rust core `114 passed, 1
  ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23
  passed`, frontend `83 passed`, `buildFlavorContract: passed`,
  `portableOcrContract: passed`, `ocrFeatureBoundary: passed`,
  `ocrDependencyTree: lite excludes, full includes`, `requiredRealGates: 17
  workspace gates, 2 OCR gates`, and checked lite exe size `3,567,616` bytes.
  Python/Tk still prints intermittent destroyed-window callback noise during
  teardown, but unittest finishes with `Ran 98 tests ... OK`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding the core crate build script that tracks
  `SCREENWATCH_BUILD_FLAVOR` and emits normalized
  `SCREENWATCH_COMPILED_BUILD_FLAVOR`, making the packaged lite/full flavor
  explicit even across Cargo cache reuse. The summary again reported Python
  static inventory `98`, locked Python baseline names `98`, Python unittest
  `98`, Rust core `114 passed, 1 ignored`, Tauri shell/backend `77 passed, 16
  ignored`, OCR feature `23 passed`, frontend `83 passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and checked
  lite exe size `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\package-portable.ps1 -Flavor lite`:
  passed after adding the release build-info sidecar and package hash checks.
  It rebuilt the lite release exe, wrote
  `target\release\screen-watch-ocr-tauri.build-info.json` with flavor `lite`,
  exe bytes `3,565,568`, and SHA-256
  `b21f846b223829e847fa82c05f342f5111b4fe1c9ed1d77aa248ad36db41d259`, then
  produced
  `target\portable\screen-watch-ocr-tauri-lite-portable-20260706-090709-3e5a055b.zip`
  with `packageVerified: True` and package size `1,600,421` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludePortablePackage`:
  passed, proving the verifier's `-IncludePortablePackage` path can package an
  existing release exe only after the build-info sidecar matches the requested
  flavor, exe bytes, and SHA-256. It produced
  `target\portable\screen-watch-ocr-tauri-lite-portable-20260706-090807-b78dfb84.zip`
  with `packageVerified: True`. The summary reported Rust core `114 passed, 1
  ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23
  passed`, `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrDependencyTree: lite excludes, full includes`, `requiredRealGates: 17
  workspace gates, 2 OCR gates`, and checked lite exe size `3,565,568` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after the sidecar/hash packaging changes. The summary reported Python
  static inventory `98`, locked Python baseline names `98`, Python unittest
  `98`, Rust core `114 passed, 1 ignored`, Tauri shell/backend `77 passed, 16
  ignored`, OCR feature `23 passed`, frontend `83 passed`,
  `buildFlavorContract: passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and checked
  lite exe size `3,565,568` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the portable OCR model contract gate. The summary
  reported Rust core `113 passed, 1 ignored`, Tauri shell/backend `77 passed,
  16 ignored`, OCR feature `23 passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and current
  checked lite exe size `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after the same portable OCR model contract gate was run with the full
  default migration loop. The summary reported Python static inventory `98`,
  locked Python baseline names `98`, Python unittest `98`, Rust core `113
  passed, 1 ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature
  `23 passed`, frontend `83 passed`, `portableOcrContract: passed`,
  `ocrFeatureBoundary: passed`, `ocrDependencyTree: lite excludes, full
  includes`, `requiredRealGates: 17 workspace gates, 2 OCR gates`, and checked
  lite exe size `3,567,616` bytes. Python/Tk still prints intermittent
  destroyed-window callback noise during teardown, but unittest finishes with
  `Ran 98 tests ... OK`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeTemplateBenchmark -IncludePackagedSmoke`:
  passed earlier with Python static inventory 98 tests, Python unittest 98
  tests, Rust workspace tests, Tauri shell/backend tests, OCR feature tests
  with 2 ignored real-model smoke gates, frontend 53 tests, frontend build,
  `templateBenchmarkMs=4221`, packaged smoke with `visibleMainWindows: 0`,
  `closeToTrayAfterCloseVisibleMainWindows: 0`, `secondInstanceExitCode: 0`,
  `secondInstanceExited: True`, and `singleInstanceWakeVisibleMainWindows: 1`,
  Python exe size `102,021,797` bytes, and then-current Tauri lite exe size
  `3,202,560` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -IncludePackagedSmoke`:
  passed Rust formatting, workspace tests, OCR feature tests/checks, no-bundle
  Tauri lite release build, and packaged start-minimized smoke. The smoke used
  an isolated temporary app-data directory and single-instance port, reported
  `visibleMainWindows: 0`, then stopped only the process it started and removed
  the temporary app-data directory.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludePackagedSmoke`:
  passed the extended packaged smoke entrypoint with the current release exe
  after raising the default packaged-smoke wait to 12 seconds. The summary
  reported Rust core `108 passed, 1 ignored`, Tauri shell/backend `67 passed,
  12 ignored`, OCR feature `22 passed`, `packagedSmoke: ran`, and
  `tauriExeBytes: 3205120`. The smoke reported
  `startMinimizedSmokeVerified: True`, `closeToTrayAfterCloseVisibleMainWindows:
  0`, `secondInstanceExitCode: 0`, `secondInstanceExited: True`,
  `singleInstanceWakeVisibleMainWindows: 1`, `closeToTraySmokeVerified: True`,
  and `packagedSmokeVerified: True`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludePackagedSmoke`:
  passed after adding packaged legacy `app_data` migration smoke. The script
  staged the release exe in a temporary app root, created a legacy `app_data`
  fixture beside it, verified the startup path copied profiles/templates/state
  and alerts into an isolated `ScreenWatchOCR` directory, preserved the legacy
  source files, then verified `--start-minimized`, close-to-tray, and
  single-instance wake. The summary reported Rust core `109 passed, 1 ignored`,
  Tauri shell/backend `67 passed, 12 ignored`, OCR feature `23 passed`,
  `legacyMigrationSmokeVerified: True`, `packagedSmoke: ran`, and
  `tauriExeBytes: 3205120`; cleanup reported both the temporary app-data
  directory and temporary app root removed.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludePackagedSmoke`:
  passed after adding packaged legacy geometry restore smoke and raising the
  default packaged-smoke wait to 18 seconds. The smoke reported
  `legacyGeometryMainWindowRect: 668x491+13+20`,
  `legacyGeometryProbeScale: 1.5`, and
  `legacyGeometryRestoreSmokeVerified: True`, proving the packaged app restored
  the migrated `state.json` geometry through the real startup path with
  DPI-virtualized probe coordinates. The summary again reported Rust core `109
  passed, 1 ignored`, Tauri shell/backend `67 passed, 12 ignored`, OCR feature
  `23 passed`, and `packagedSmoke: ran`.
- `powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath target\release\screen-watch-ocr-tauri.exe -StartupWaitSeconds 12`:
  passed against the latest release exe. It reported
  `legacyMigrationSmokeVerified: True`,
  `startMinimizedSmokeVerified: True`,
  `closeToTrayAfterCloseVisibleMainWindows: 0`,
  `secondInstanceExitCode: 0`, `secondInstanceExited: True`,
  `singleInstanceWakeVisibleMainWindows: 1`, and
  `closeToTraySmokeVerified: True`, then stopped both test-owned long-lived
  processes and removed the temporary app-data directory plus the temporary app
  root.
- `powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath target\release\screen-watch-ocr-tauri.exe -StartupWaitSeconds 18`:
  passed with `legacyMigrationSmokeVerified: True`,
  `legacyGeometryRestoreSmokeVerified: True`,
  `startMinimizedSmokeVerified: True`, `closeToTraySmokeVerified: True`, and
  `packagedSmokeVerified: True`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipFrontend -SkipRelease -IncludeDesktopSmoke`:
  passed with Python static inventory 98 tests, Python unittest 98 tests, Rust
  workspace tests, 12 desktop smoke gates, OCR feature tests/checks, Python exe
  size `102,021,797` bytes, and Tauri lite exe size `3,205,120` bytes. The
  summary reported Rust core `108 passed, 1 ignored`, Tauri shell/backend
  `67 passed, 12 ignored`, and `desktopSmoke: 12 gates`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke`:
  passed after adding the profile window workflow gate. The summary reported
  Rust core `109 passed, 1 ignored`, Tauri shell/backend `67 passed, 13
  ignored`, OCR feature `23 passed`, and `desktopSmoke: 13 gates`.
- `npm run test:frontend`: 58 frontend behavior tests passing after extracting
  the source-preview refresh gate and DWM bitmap-fallback decision into tested
  helpers. The new coverage locks scheduled refresh skipping while disabled,
  manual refresh before timer enablement, retry behavior during active refresh
  or layout-busy states, and the DWM handoff path that keeps a window preview
  card healthy when bitmap capture fails.
- `npm run test:frontend`: 86 frontend behavior tests passing after routing
  source-preview card updates through `sourcePreviewCardPresentation`. The new
  coverage locks successful bitmap image presentation, DWM handoff fallback
  behavior when bitmap capture fails, and stale-image clearing plus error state
  when no DWM fallback exists. `scripts\verify-migration.ps1` now requires at
  least 86 frontend tests.
- `npm run test:frontend`: 89 frontend behavior tests passing after extracting
  source-preview visible-rect calculation into `ui-behavior.js`. The new
  coverage locks partially offscreen WebView frame clipping and prevents DWM
  sync when the preview frame or viewport is hidden/tiny. The verifier now
  requires at least 89 frontend tests and checks that `main.js` still routes
  `sync_dwm_preview` through the tested rect helper with bitmap fallback and
  frame cleanup intact.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend test baseline to 58. The summary reported
  Rust core `109 passed, 1 ignored`, Tauri shell/backend `67 passed, 13
  ignored`, OCR feature `23 passed`, frontend `58 passed`, frontend build
  `True`, release build `False`, and current Tauri lite exe size `3,205,120`
  bytes.
- `npm run test:frontend`: 63 frontend behavior tests passing after extracting
  profile source/preview source selection helpers. The new coverage locks
  virtual-monitor exclusion, region offsets for preview cards, concrete
  window profile option shape, remembered app-window option shape, and the
  no-source guard used before profile scans or monitoring starts.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend test baseline to 63. The summary reported
  Rust core `109 passed, 1 ignored`, Tauri shell/backend `67 passed, 13
  ignored`, OCR feature `23 passed`, frontend `63 passed`, frontend build
  `True`, release build `False`, and current Tauri lite exe size `3,205,120`
  bytes.
- `npm run test:frontend`: 64 frontend behavior tests passing after extracting
  profile target action state. The new coverage locks the row-button behavior
  for up/down reorder insert indexes, first/last target move disabling,
  open-image availability, and hit-count clearing only when a stable target id
  and positive hit count are both present.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend test baseline to 64. The summary reported
  Rust core `109 passed, 1 ignored`, Tauri shell/backend `67 passed, 13
  ignored`, OCR feature `23 passed`, frontend `64 passed`, frontend build
  `True`, release build `False`, and current Tauri lite exe size `3,205,120`
  bytes.
- `npm run test:frontend`: 66 frontend behavior tests passing after extracting
  frontend monitoring-event transition logic. The new coverage locks profile
  refresh on profile-monitoring tick hits, stopped events clearing
  profile-monitoring state, snake/camel tick event field handling, and current
  tick-error text taking precedence over stale snapshot errors.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend test baseline to 66. The summary reported
  Rust core `109 passed, 1 ignored`, Tauri shell/backend `67 passed, 13
  ignored`, OCR feature `23 passed`, frontend `66 passed`, frontend build
  `True`, release build `False`, and current Tauri lite exe size `3,205,120`
  bytes.
- `cargo test -p screen-watch-ocr-tauri monitor_session`: 11 monitoring-session
  backend tests passing, plus 2 ignored interactive desktop gates, after adding
  pure coverage for zero-tick start/stop event payloads and skipped direct-window
  plus remembered-app-window tick snapshots.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the Tauri shell/backend baseline to 69. The summary
  reported Rust core `109 passed, 1 ignored`, Tauri shell/backend `69 passed,
  13 ignored`, OCR feature `23 passed`, frontend `66 passed`, frontend build
  `True`, release build `False`, and current Tauri lite exe size `3,205,120`
  bytes.
- `npm run test:frontend`: 68 frontend behavior tests passing after extracting
  profile workflow action gating. The new coverage locks no-enabled-template
  rejection, no-source rejection, and duplicate profile-monitoring start
  rejection before backend commands are invoked.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend test baseline to 68. The summary reported
  Rust core `109 passed, 1 ignored`, Tauri shell/backend `69 passed, 13
  ignored`, OCR feature `23 passed`, frontend `68 passed`, frontend build
  `True`, release build `False`, and current Tauri lite exe size `3,205,120`
  bytes.
- `npm run test:frontend`: 70 frontend behavior tests passing after extracting
  profile import request parsing. The new coverage locks pasted text path
  trimming, blank-line filtering, native picker array handling, order
  preservation, and max-template limit clamping.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend test baseline to 70. The summary reported
  Rust core `109 passed, 1 ignored`, Tauri shell/backend `69 passed, 13
  ignored`, OCR feature `23 passed`, frontend `70 passed`, frontend build
  `True`, release build `False`, and current Tauri lite exe size `3,205,120`
  bytes.
- `cargo test -p screen-watch-ocr-tauri single_instance`: 6 single-instance
  tests passing after adding a port-in-use retry gate. The new coverage locks
  the case where the first wake notification connects to an existing listener
  but receives no ack, bind then reports the port in use, and the second wake
  notification succeeds instead of returning unavailable.
- `npm run test:frontend`: 71 frontend behavior tests passing after adding
  profile import result feedback. The new coverage locks the imported,
  pruned, and current target count status text for both camelCase and snake_case
  result payloads.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the Tauri shell/backend baseline to 70 and frontend
  baseline to 71. The summary reported Rust core `109 passed, 1 ignored`, Tauri
  shell/backend `70 passed, 13 ignored`, OCR feature `23 passed`, frontend `71
  passed`, frontend build `True`, release build `False`, and current Tauri lite
  exe size `3,205,120` bytes.
- `npm run test:frontend`: 73 frontend behavior tests passing after wiring
  backend-returned profile edit selection results into the target list. The new
  coverage locks camelCase/snake_case selected-index handling, stale-selection
  clearing, invalid-index rejection, and fallback preservation when an edit
  result has no selection field.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend baseline to 73. The summary reported Rust
  core `109 passed, 1 ignored`, Tauri shell/backend `70 passed, 13 ignored`,
  OCR feature `23 passed`, frontend `73 passed`, frontend build `True`, release
  build `False`, and current Tauri lite exe size `3,205,120` bytes.
- `cargo test -p screen-watch-core clear_profile_target_hit_count`: 2 targeted
  profile hit-clear tests passing after changing stable-id hit clearing to
  return `ProfileTargetsEditResult` with `selected_index`.
- `cargo test -p screen-watch-ocr-tauri profile_targets_edit_result_serializes_selected_index_for_frontend`:
  1 targeted backend serialization test passing, locking the frontend-facing
  `selectedIndex` field name.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the Rust core baseline to 110, Tauri shell/backend
  baseline to 71, and frontend baseline to 74. The summary reported Rust core
  `110 passed, 1 ignored`, Tauri shell/backend `71 passed, 13 ignored`, OCR
  feature `23 passed`, frontend `74 passed`, frontend build `True`, release
  build `False`, and current Tauri lite exe size `3,205,120` bytes.
- `npm run test:frontend -- --test-name-pattern "target selection|profile load target selection|profile refresh target selection"`:
  77 frontend behavior tests passing after adding profile-load selection tests.
  The new coverage locks Python-compatible first-target selection for normal
  profile loads/switches, valid-index preservation for profile refreshes, and
  clearing invalid/empty profile selection.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend baseline to 77. The summary reported Rust
  core `110 passed, 1 ignored`, Tauri shell/backend `71 passed, 13 ignored`,
  OCR feature `23 passed`, frontend `77 passed`, frontend build `True`, release
  build `False`, and current Tauri lite exe size `3,205,120` bytes.
- `npm run test:frontend -- --test-name-pattern "profile target enabled status|profile import status"`:
  79 frontend behavior tests passing after adding Python-style enabled/total
  template status text for single-target enabled changes and select-all/invert
  results.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the frontend baseline to 79. The summary reported Rust
  core `110 passed, 1 ignored`, Tauri shell/backend `71 passed, 13 ignored`,
  OCR feature `23 passed`, frontend `79 passed`, frontend build `True`, release
  build `False`, and current Tauri lite exe size `3,205,120` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after adding common image template decoding/import normalization and
  raising the Rust core baseline to 112. PNG/JPG/JPEG/BMP/WEBP inputs are
  accepted while profile imports still write normalized PNGs under
  `templates/`. The summary reported Rust core `112 passed, 1 ignored`, Tauri
  shell/backend `71 passed, 13 ignored`, OCR feature `23 passed`, frontend `79
  passed`, frontend build `True`, release build `False`, and current Tauri
  lite exe size `3,205,120` bytes.
- `npx tauri build --no-bundle --ci`:
  passed after adding common image template decoding/import normalization. The
  rebuilt lite exe at `target\release\screen-watch-ocr-tauri.exe` is
  `3,544,064` bytes, about 3.38 MiB.
- `cargo test --workspace` and `npm run test:frontend`:
  passed after adding Python-style clipboard image/path paste for profile
  templates. The backend can read supported image file paths from the Windows
  clipboard and decode CF_DIB/CF_DIBV5 clipboard bitmaps into RGB frames, then
  writes normalized PNG templates through the same profile import path. The
  summary counts were Rust core `113 passed, 1 ignored`, Tauri shell/backend
  `73 passed, 13 ignored`, and frontend `81 passed`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipRelease`:
  passed after raising the Rust core baseline to 113, Tauri shell/backend
  baseline to 73, and frontend baseline to 81. The summary reported OCR feature
  `23 passed`, frontend build `True`, release build `False`, and current Tauri
  lite exe size `3,544,064` bytes before the following release rebuild.
- `npx tauri build --no-bundle --ci`:
  passed after adding clipboard image/path paste. The rebuilt lite exe at
  `target\release\screen-watch-ocr-tauri.exe` is `3,564,032` bytes, about
  3.40 MiB.
- `cargo test --workspace` and `npm run test:frontend`:
  passed after adding the Python-style screenshot-as-template workflow. The
  backend can capture the first selected screen region, or a selected/resolved
  app window when no screen region is selected, and writes the captured frame
  through the same PNG-normalized profile template path. The summary counts
  were Rust core `113 passed, 1 ignored`, Tauri shell/backend `76 passed, 13
  ignored`, and frontend `82 passed`.
- `npx tauri build --no-bundle --ci`:
  passed after adding the Python-style screenshot-as-template workflow. The
  rebuilt lite exe at `target\release\screen-watch-ocr-tauri.exe` is
  `3,567,616` bytes, about 3.40 MiB.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after rebuilding the screenshot-as-template lite release exe and
  updating the size record. The summary reported Python static inventory `98`,
  Python unittest `98`, Rust core `113 passed, 1 ignored`, Tauri shell/backend
  `76 passed, 13 ignored`, OCR feature `23 passed`, frontend `82 passed`,
  frontend build `True`, release build `False`, Python exe `102,021,797`
  bytes, and current Tauri lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke`:
  passed after adding the real desktop screenshot-as-template gate. The summary
  reported Rust core `113 passed, 1 ignored`, Tauri shell/backend `76 passed,
  14 ignored`, OCR feature `23 passed`, `desktopSmoke: 14 gates`, frontend
  skipped, release build `False`, and current Tauri lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after the desktop screenshot-as-template gate was added. The summary
  reported Python static inventory `98`, Python unittest `98`, Rust core `113
  passed, 1 ignored`, Tauri shell/backend `76 passed, 14 ignored`, OCR feature
  `23 passed`, frontend `82 passed`, frontend build `True`, release build
  `False`, Python exe `102,021,797` bytes, and current Tauri lite exe
  `3,567,616` bytes.
- `npx tauri build --no-bundle --ci`:
  passed after adding the desktop screenshot-as-template gate. The rebuilt lite
  exe at `target\release\screen-watch-ocr-tauri.exe` remains `3,567,616`
  bytes, about 3.40 MiB.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke`:
  passed after adding the real window-source screenshot-as-template gate. The
  summary reported Rust core `113 passed, 1 ignored`, Tauri shell/backend `76
  passed, 15 ignored`, OCR feature `23 passed`, `desktopSmoke: 15 gates`,
  frontend skipped, release build `False`, and current Tauri lite exe
  `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding the real window-source screenshot-as-template gate. The
  summary reported Python static inventory `98`, Python unittest `98`, Rust
  core `113 passed, 1 ignored`, Tauri shell/backend `76 passed, 15 ignored`,
  OCR feature `23 passed`, frontend `82 passed`, frontend build `True`,
  release build `False`, Python exe `102,021,797` bytes, and current Tauri
  lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke`:
  passed after adding the remembered `window_apps` screenshot-as-template
  gate. The smoke starts an external WinForms window, verifies real window
  enumeration can see it, resolves it from Python-compatible `title` plus
  `ordinal`, captures the resolved window frame, and writes that frame into a
  profile template. The summary reported Rust core `113 passed, 1 ignored`,
  Tauri shell/backend `76 passed, 16 ignored`, OCR feature `23 passed`,
  `desktopSmoke: 16 gates`, frontend skipped, release build `False`, and
  current Tauri lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding the remembered `window_apps` screenshot-as-template
  gate. The summary reported Python static inventory `98`, Python unittest
  `98`, Rust core `113 passed, 1 ignored`, Tauri shell/backend `76 passed, 16
  ignored`, OCR feature `23 passed`, frontend `82 passed`, frontend build
  `True`, release build `False`, Python exe `102,021,797` bytes, and current
  Tauri lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding the Tauri backend profile-gallery edit workflow gate and
  raising the Tauri shell/backend baseline to 77. The summary reported Python
  static inventory `98`, Python unittest `98`, Rust core `113 passed, 1
  ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23
  passed`, frontend `82 passed`, frontend build `True`, release build `False`,
  Python exe `102,021,797` bytes, and current Tauri lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding the frontend profile-gallery edit selection workflow gate
  and raising the frontend baseline to 83. The summary reported Python static
  inventory `98`, Python unittest `98`, Rust core `113 passed, 1 ignored`,
  Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23 passed`,
  frontend `83 passed`, frontend build `True`, release build `False`, Python
  exe `102,021,797` bytes, and current Tauri lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding the lite exe size guardrail to the verifier. The summary
  reported Python static inventory `98`, Python unittest `98`, Rust core `113
  passed, 1 ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature
  `23 passed`, frontend `83 passed`, frontend build `True`, release build
  `False`, `liteSizeGate: passed`, Python exe `102,021,797` bytes, and current
  Tauri lite exe `3,567,616` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after separating `liteExeBytes` from the current release exe bytes in
  the verifier, so the lite size gate is tied to a confirmed lite artifact. The
  summary reported Python static inventory `98`, Python unittest `98`, Rust
  core `113 passed, 1 ignored`, Tauri shell/backend `77 passed, 16 ignored`,
  OCR feature `23 passed`, frontend `83 passed`, frontend build `True`, release
  build `False`, `liteSizeGate: passed`, Python exe `102,021,797` bytes,
  `liteExeBytes: 3,567,616`, and `tauriExeBytes: 3,567,616`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding the Cargo OCR feature-boundary guardrail. The verifier now
  fails if Cargo metadata stops keeping default features empty, if
  `pure-onnx-ocr` stops being optional in `screen-watch-core`, or if Tauri
  enables `screen-watch-core/ocr` by default. The summary reported Python static
  inventory `98`, Python unittest `98`, Rust core `113 passed, 1 ignored`,
  Tauri shell/backend `77 passed, 16 ignored`, OCR feature `23 passed`,
  frontend `83 passed`, frontend build `True`, release build `False`,
  `ocrFeatureBoundary: passed`, `liteSizeGate: passed`, Python exe
  `102,021,797` bytes, `liteExeBytes: 3,567,616`, and
  `tauriExeBytes: 3,567,616`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding named real/manual gate preservation to the verifier. The
  verifier now fails if the required large-template benchmark gates, any of the 16 desktop
  smoke gates, or either OCR real-model gate disappears from the relevant Cargo
  test output. The summary reported Python static inventory `98`, Python
  unittest `98`, Rust core `113 passed, 1 ignored`, Tauri shell/backend `77
  passed, 16 ignored`, OCR feature `23 passed`, frontend `83 passed`, frontend
  build `True`, release build `False`, `ocrFeatureBoundary: passed`,
  `requiredRealGates: 17 workspace gates, 2 OCR gates`, `liteSizeGate: passed`,
  Python exe `102,021,797` bytes, `liteExeBytes: 3,567,616`, and
  `tauriExeBytes: 3,567,616`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease`:
  passed after adding `docs\PYTHON_BASELINE_TESTS.txt` and locking the current
  98 Python acceptance test names. The verifier now allows additional Python
  tests but fails if any locked baseline name disappears from the static
  inventory. The summary reported Python static inventory `98`,
  `pythonBaselineNames: 98 locked`, Python unittest `98`, Rust core `113
  passed, 1 ignored`, Tauri shell/backend `77 passed, 16 ignored`, OCR feature
  `23 passed`, frontend `83 passed`, frontend build `True`, release build
  `False`, `ocrFeatureBoundary: passed`, `requiredRealGates: 17 workspace gates,
  2 OCR gates`, `liteSizeGate: passed`, Python exe `102,021,797` bytes,
  `liteExeBytes: 3,567,616`, and `tauriExeBytes: 3,567,616`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipFrontend -SkipRelease`:
  passed after embedding build-flavor fallback logic. The summary reported
  Python static inventory 98, Python unittest 98, Rust core `109 passed, 1
  ignored`, Tauri shell/backend `67 passed, 12 ignored`, OCR feature `23
  passed`, and Tauri lite exe size `3,205,120` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipFrontend -SkipRelease -IncludePortablePackage`:
  passed and produced a verified portable lite zip under `target\portable`;
  the packaging gate reported `packageVerified: True`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeFullPortablePackage`:
  passed after adding the full portable verifier switch. It produced
  `screen-watch-ocr-tauri-full-portable-20260706-053109-807ac5ec.zip` under
  `target\portable`; the packaging gate reported `packageVerified: True`,
  full exe size `8,602,624` bytes, and full portable zip size `3,354,066`
  bytes. The summary reported Rust core `109 passed, 1 ignored`, Tauri
  shell/backend `67 passed, 12 ignored`, and OCR feature `23 passed`.
- `powershell -ExecutionPolicy Bypass -File scripts\package-portable.ps1 -Flavor lite -SkipBuild`:
  passed against the latest release exe and produced verified zip
  `screen-watch-ocr-tauri-lite-portable-20260706-044015-91e04f46.zip`
  at `1,433,718` bytes. The latest lite exe was rebuilt after the template
  performance and DWM smoke additions and is `3,205,120` bytes.
- `npm run package:portable:lite`: passed after the build-flavor fallback and
  stricter OCR asset package checks. It produced
  `screen-watch-ocr-tauri-lite-portable-20260706-053237-b339afb1.zip` under
  `target\portable`; the packaging gate reported `packageVerified: True`, lite
  exe size `3,205,120` bytes, and lite portable zip size `1,433,740` bytes.
- `npm run package:portable:full`: passed and produced a verified portable
  full zip under `target\portable`, with OCR models still external and
  `packageVerified: True`. After embedding build-flavor fallback and linking
  the native `pure-onnx-ocr` backend, the worker-backed full exe was
  `8,602,624` bytes and the verified full portable zip was `3,354,066` bytes.
- `cargo fmt`: passed.
- `cargo test -p screen-watch-core`: 13 tests passing.
- After adding compatibility modules: `cargo test -p screen-watch-core`: 24 tests passing.
- After adding in-memory template matching: `cargo test -p screen-watch-core`: 29 tests passing.
- After adding PNG template loading and prepared detector: `cargo test -p screen-watch-core`:
  33 tests passing.
- After adding profile/template file normalization: `cargo test -p screen-watch-core`:
  39 tests passing.
- After adding hit-count profile writing and clear helpers: `cargo test -p screen-watch-core`:
  43 tests passing.
- After adding evidence screenshot/JSONL writing: `cargo test -p screen-watch-core`:
  45 tests passing.
- After adding frame scan/evidence orchestration: `cargo test -p screen-watch-core`:
  48 tests passing.
- After adding source planning and window-only source handling:
  `cargo test -p screen-watch-core`: 51 tests passing.
- After adding alert screenshot labels and OCR text-row matching:
  `cargo test -p screen-watch-core`: 55 tests passing.
- After adding Python-style large-frame template coarse/refine matching:
  `cargo test -p screen-watch-core`: 57 tests passing.
- After adding `template_workers` config parsing and bounded parallel template
  scheduling: `cargo test -p screen-watch-core`: 60 tests passing.
- After adding a repeatable large-frame multi-template benchmark gate:
  `npm run template:benchmark` passed with
  `templateBenchmarkMs=3983 frame=2560x1440 templates=8 workers=4 matches=8`.
  The benchmark is ignored by default and can be included in migration
  verification with `-IncludeTemplateBenchmark`; `-TemplateBenchmarkMaxMs`
  turns the elapsed time into a machine-specific hard threshold.
- After adding exact-flat-template integral scoring and shared gray/scale
  frames across template jobs:
  `cargo test -p screen-watch-core detect`: 21 detection-related tests passed
  with 1 ignored benchmark gate. `npm run template:parity` passed on the same
  2560x1440, 8-template synthetic workload with
  `pythonTemplateBenchmarkMs=47` and Rust release `templateBenchmarkMs=296`.
  The pure Rust path is much smaller but still slower than OpenCV on this fixed
  workload, so representative production parity remains an optimization gate.
- After caching flat-template frame integrals across coarse/refine passes and
  storing prepared templates as gray frames:
  `cargo test -p screen-watch-core` passed with Rust core `115 passed, 1
  ignored`. A same-session pre-change parity run reported
  `pythonTemplateBenchmarkMs=62` and Rust release `templateBenchmarkMs=404`;
  after the optimization, `npm run template:parity` reported
  `pythonTemplateBenchmarkMs=53` and Rust release `templateBenchmarkMs=137` on
  the same fixed 2560x1440, 8-template workload. Rust remains behind
  Python/OpenCV on this synthetic case, but the pure Rust path is now
  substantially closer while keeping the lite build small.
- After adding perfect-score early return for pure flat-template scans:
  `cargo test -p screen-watch-core` passed with Rust core `115 passed, 1
  ignored`. `npm run template:parity` then reported
  `pythonTemplateBenchmarkMs=48` and Rust release `templateBenchmarkMs=67` on
  the same fixed 2560x1440, 8-template workload, keeping all expected boxes and
  target ids. The remaining performance gate is to measure representative
  production template sets rather than only this synthetic flat-template case.
- After adding integral-assisted window mean/energy for textured-template NCC:
  `cargo test -p screen-watch-core` passed with Rust core `116 passed, 1
  ignored`, including a parity test that compares cached and uncached NCC
  scoring. `npm run template:parity` reported `pythonTemplateBenchmarkMs=49`
  and Rust release `templateBenchmarkMs=70` on the same fixed 2560x1440,
  8-template workload. `scripts\verify-migration.ps1` now requires at least
  116 Rust core tests so this coverage cannot silently disappear.
- After adding phase-aware coarse templates for small textured-template scans:
  `cargo test -p screen-watch-core` passed with Rust core `116 passed, 2
  ignored`. `scripts\template-benchmark.ps1` now runs both ignored benchmark
  gates in release mode by default and reported Rust flat `templateBenchmarkMs=87`
  plus Rust textured `texturedTemplateBenchmarkMs=3381`, both with 8/8 matches
  on 2560x1440 frames. `scripts\template-parity-benchmark.ps1` reported
  Python/OpenCV flat `pythonTemplateBenchmarkMs=46` with 8/8 matches and the
  odd-phase textured baseline `pythonTexturedTemplateBenchmarkMs=42` with 4/8
  matches, while Rust textured still hit 8/8 at `texturedTemplateBenchmarkMs=3408`.
  `scripts\verify-migration.ps1` now requires both benchmark gate names, so
  `requiredRealGates` becomes 18 workspace gates plus 2 OCR gates.
- After replacing full coarse-location NCC with phase-aware sparse coarse
  candidate scoring for textured templates:
  `cargo test -p screen-watch-core` passed with Rust core `117 passed, 2
  ignored`, including `sparse_texture_candidate_score_prefers_exact_window`.
  `scripts\template-benchmark.ps1` reported Rust flat `templateBenchmarkMs=83`
  and Rust textured `texturedTemplateBenchmarkMs=435`, both with 8/8 matches on
  2560x1440 frames. `scripts\template-parity-benchmark.ps1` reported
  Python/OpenCV flat `pythonTemplateBenchmarkMs=63` with 8/8 matches and
  odd-phase textured `pythonTexturedTemplateBenchmarkMs=60` with 4/8 matches,
  while Rust release reported flat `templateBenchmarkMs=72` and textured
  `texturedTemplateBenchmarkMs=413`, both with 8/8 matches. The verifier
  minimum Rust core test count is now 117.
- After adding a profile-to-scan scaled-template integration gate:
  `cargo test -p screen-watch-core profile_watch_config_scans_scaled_template_through_engine`
  passed. The gate reads a GUI profile target from `templates/`, builds a
  profile-derived `WatchConfig` with scaled template matching, runs it through
  `ScanEngine`, writes evidence, and verifies the scaled hit box/id.
- After adding alarm beep settings, WAV generation, volume clamping, and
  no-restart throttle behavior: `cargo test -p screen-watch-core`: 65 tests
  passing.
- After adding profile template PNG importing with prune-before-name behavior:
  `cargo test -p screen-watch-core`: 67 tests passing.
- After adding common image decoding for prepared detectors and profile import
  PNG normalization:
  `cargo test -p screen-watch-core`: 112 tests passing, 1 ignored benchmark
  gate.
- After adding profile target reorder/remove/clear helpers:
  `cargo test -p screen-watch-core`: 70 tests passing.
- After adding profile target enabled filtering and select-all/invert helpers:
  `cargo test -p screen-watch-core`: 74 tests passing.
- After adding profile-to-watch-config construction:
  `cargo test -p screen-watch-core`: 76 tests passing.
- After adding profile one-shot scan and monitoring-session commands:
  `cargo test --workspace`: passed with 76 core tests and 42 Tauri
  shell/backend tests; the then-current Tauri desktop-only manual gates were
  ignored by default.
- After adding profile read snapshots and a frontend profile panel:
  `cargo test -p screen-watch-core`: 78 tests passing.
- After adding profile target list controls and guarded profile-image opening:
  `cargo test -p screen-watch-ocr-tauri --lib profile_target_file_to_open`: 3
  tests passing.
- After adding native PNG selection parsing and frontend import wiring:
  `cargo test -p screen-watch-ocr-tauri --lib parse_open_file_name_buffer`: 3
  tests passing.
- After adding Windows screen-region capture and preview command:
  `cargo test -p screen-watch-ocr-tauri`: 11 tests passing, 3 desktop-only
  capture/scan/session tests ignored by default.
- After adding monitoring session event payloads and frontend event listening:
  `cargo test -p screen-watch-ocr-tauri`: 14 tests passing, 3 desktop-only
  capture/scan/session tests ignored by default.
- After adding Windows app-window enumeration and remembered app resolution:
  `cargo test -p screen-watch-ocr-tauri`: 18 tests passing, 4 desktop-only
  capture/scan/session/window tests ignored by default.
- After adding Windows window capture, visible fallback, black-frame detection,
  and black-padding crop logic:
  `cargo test -p screen-watch-ocr-tauri`: 22 tests passing, 6 desktop-only
  capture/scan/session/window tests ignored by default.
- After wiring window sources into one-shot scans and persistent monitoring:
  `cargo test -p screen-watch-ocr-tauri`: 25 tests passing, 8 desktop-only
  capture/scan/session/window tests ignored by default.
- After adding the Tauri alarm beep runtime and scan/session trigger wiring:
  `cargo test -p screen-watch-ocr-tauri`: 27 tests passing, 8 desktop-only
  capture/scan/session/window tests ignored by default.
- After adding the audio alarm parity contract:
  `cargo test -p screen-watch-ocr-tauri audio --lib` passed 4 audio tests,
  `cargo test -p screen-watch-core audio --lib` passed 4 audio tests, and
  `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1
  -SkipPython -SkipFrontend -SkipRelease` reported `audioAlarmParityContract:
  passed` with Rust core 121 passed / 3 ignored, Tauri 87 passed / 16
  ignored, and OCR feature 25 passed. The contract locks Python
  `winsound.PlaySound(..., SND_MEMORY)`/in-memory WAV/beep-throttle behavior
  against Tauri `PlaySoundW`/`SND_MEMORY` plus one-shot and monitoring hit
  triggers.
- After adding the Windows startup shortcut backend and frontend controls:
  `cargo test -p screen-watch-ocr-tauri`: 33 tests passing, 8 desktop-only
  capture/scan/session/window tests ignored by default.
- After adding the initial single-instance wake protocol:
  `cargo test -p screen-watch-ocr-tauri --lib single_instance`: 4 tests
  passing.
- After adding an isolated single-instance port override for packaged smoke
  runs:
  `cargo test -p screen-watch-ocr-tauri --lib single_instance`: 5 tests
  passing. After identity separation, the default Tauri port is
  `127.0.0.1:47628`; the override is used by smoke automation to avoid
  interfering with an already-running Tauri instance.
- After adding a retry when the existing instance port is already bound but the
  first wake notification misses its ack:
  `cargo test -p screen-watch-ocr-tauri single_instance`: 6 tests passing.
- After adding the Tauri tray lifecycle backend:
  `cargo test -p screen-watch-ocr-tauri --lib tray`: 5 tests passing.
- After locking Tauri tray menu and click routing contracts:
  `cargo test -p screen-watch-ocr-tauri --lib tray`: 7 tests passing. The new
  coverage verifies the packaged tray menu IDs and labels route to show/exit
  actions, and that left-click release routes to show while right-click/down
  events do not. The verifier minimum Tauri shell/backend test count was 81 at
  that point.
- After locking tray monitoring status presentation and the backend event-sink
  update path:
  `cargo test -p screen-watch-ocr-tauri --lib tray`: 8 tests passing. The new
  coverage verifies tooltip/icon/dimension presentation as one unit, and the
  verifier now checks that `TauriMonitorEventSink` updates tray status from
  `event.snapshot.running` before emitting `MONITOR_SESSION_EVENT`. The
  verifier minimum Tauri shell/backend test count is now 82.
- After adding scan-engine OCR backend interface wiring and explicit
  unavailable-backend/model errors for OCR targets:
  `cargo test -p screen-watch-core`: 80 tests passing.
- After adding Python-compatible profile source/match persistence and
  `state.json` `last_profile` preservation:
  `cargo test -p screen-watch-core`: 83 tests passing.
- After tightening profile source/config compatibility for frontend camelCase
  options, concrete window ordinals, remembered `window_apps`, and mixed
  window-source config boundaries:
  `cargo test -p screen-watch-core profile`: 33 profile-related tests passing,
  and `cargo test -p screen-watch-core`: 98 tests passing.
- After adding non-destructive legacy `app_data` migration into the shared user
  data directory:
  `cargo test -p screen-watch-core data_dir`: 6 data-dir/OCR-model path tests
  passing. The migration copies missing legacy files but does not delete the
  legacy source or overwrite existing shared `ScreenWatchOCR` files.
- After adding Cargo feature boundaries, separated OCR model/backend readiness,
  and a shared runtime OCR backend factory for one-shot and monitoring paths:
  `cargo test -p screen-watch-core ocr`: 16 OCR-related tests passing, and
  `cargo check -p screen-watch-ocr-tauri --features ocr`: passed.
- After wiring the first native Rust OCR backend through `pure-onnx-ocr`:
  `cargo test -p screen-watch-core ocr --features ocr`: 18 OCR-related tests
  passing. The full backend now reports `pure-onnx-ocr`/`ppocrv5-dbnet-svtr`
  readiness and expects external `det.onnx`, `rec.onnx`, and
  `ppocrv5_dict.txt` assets; PP-OCRv6/RapidOCR filenames are reported as a
  separate reference model status until that exact native profile is wired.
- The native OCR backend now starts a dedicated worker lazily on the first OCR
  recognition request, keeps the non-`Send`/non-`Sync` `pure-onnx-ocr` engine
  inside that worker, and reuses the initialized engine or cached
  initialization failure across frames. Scans without OCR targets do not start
  an OCR worker.
- After adding an explicit OCR backend initialization probe:
  `cargo test -p screen-watch-core`: 95 tests passing, and
  `cargo test -p screen-watch-core ocr --features ocr`: 22 OCR-related tests
  passing. The probe reports skipped lite/not-compiled/missing-model states
  without starting OCR, and attempts native initialization only when the linked
  backend and required external model files are both present.
- After adding repeatable real-model OCR smoke gates:
  `cargo test -p screen-watch-core --features ocr ocr`: 22 OCR-related tests
  passing with 2 ignored real-model gates. The ignored gates are runnable
  through `scripts\ocr-smoke.ps1` or
  `scripts\verify-migration.ps1 -IncludeOcrSmoke`; one verifies native backend
  initialization from external assets and the second verifies recognized text
  from a caller-supplied PNG smoke image.
- After supplying external English and Chinese PP-OCRv5 ONNX model sets for
  real OCR smoke:
  `powershell -ExecutionPolicy Bypass -File scripts\ocr-smoke.ps1 -ModelDir target\ocr-model-smoke\monkt-ppocrv5-english -Image target\ocr-model-smoke\ready-smoke.png -Expect READY`
  passed, and
  `powershell -ExecutionPolicy Bypass -File scripts\ocr-smoke.ps1 -ModelDir target\ocr-model-smoke\monkt-ppocrv5-chinese -Image target\ocr-model-smoke\zh-ready-smoke.png -Expect 准备`
  passed. Each external model directory contains `det.onnx`, `rec.onnx`, and
  `ppocrv5_dict.txt`; the generated `READY` PNG and `准备好了` PNG recognition
  gates passed. The model files are retained under `target\ocr-model-smoke` as
  external smoke evidence and are not bundled into lite/full builds or copied
  into the shared `ScreenWatchOCR` data directory.
- After adding OCR smoke model-asset preflight:
  `powershell -ExecutionPolicy Bypass -File scripts\ocr-smoke.ps1` fails fast
  before Cargo when required external assets are missing, reporting
  `modelDir: C:\Users\Wes\AppData\Local\ScreenWatchOCR\models\rapidocr` plus
  missing `det.onnx`, `rec.onnx`, and `ppocrv5_dict.txt` paths and the
  `-ModelDir`/`SCREENWATCH_OCR_MODEL_DIR` setup hint. This is an expected
  missing-model result on this machine, not a recorded real-model OCR pass.
- After adding the OCR smoke script contract:
  `scripts\verify-migration.ps1` now fails if `scripts\ocr-smoke.ps1` drifts
  from the Rust core's `SCREENWATCH_OCR_MODEL_DIR`, default
  `ScreenWatchOCR\models\rapidocr` model location, required native model
  filenames, missing-model preflight output, or required preflight-before-Cargo
  ordering.
- After adding the executable OCR missing-model self-test:
  `powershell -ExecutionPolicy Bypass -File scripts\ocr-smoke.ps1 -SelfTestMissingModels`
  passed. It probes an isolated temp model directory, reports missing
  `det.onnx`, `rec.onnx`, and `ppocrv5_dict.txt`, and prints
  `missingModelSelfTest: passed` without running Cargo or requiring real OCR
  assets. The migration verifier now runs this self-test by default and reports
  `ocrSmokeMissingModelSelfTest: passed`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after wiring the OCR missing-model self-test into the main verifier.
  The summary reported Rust core `117 passed, 2 ignored`, Tauri shell/backend
  `82 passed, 16 ignored`, OCR feature `23 passed`, `ocrSmokeContract:
  passed`, `ocrSmokeMissingModelSelfTest: passed`, `tauriBuildScriptContract:
  passed`, `tauriBundleConfigContract: passed`, `frontendSourcePreviewContract:
  passed`, `trayMonitoringStatusContract: passed`, `requiredRealGates: 18
  workspace gates, 2 OCR gates`, `liteSizeGate: passed`, and `liteExeBytes:
  3565568`.
- After adding the real/manual gate documentation contract:
  `scripts\verify-migration.ps1` now fails if any verifier-required
  real/manual gate name is missing from `docs\FUNCTIONAL_ACCEPTANCE.md`, which
  keeps the executable smoke gates and human checklist aligned.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the functional checklist gate-name contract. The summary
  reported Rust core `117 passed, 2 ignored`, Tauri shell/backend `82 passed,
  16 ignored`, OCR feature `23 passed`, `acceptanceRealGateContract: passed`,
  `ocrSmokeMissingModelSelfTest: passed`, `requiredRealGates: 18 workspace
  gates, 2 OCR gates`, `liteSizeGate: passed`, and `liteExeBytes: 3565568`.
- After adding the package script contract:
  `scripts\verify-migration.ps1` now fails if critical `package.json` scripts
  for frontend tests, migration verification, OCR smoke, template benchmarks,
  packaged smoke, portable lite/full packages, or Tauri lite/full builds drift
  from their expected command lines or reference missing script files.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the package script contract. The summary reported Rust
  core `117 passed, 2 ignored`, Tauri shell/backend `82 passed, 16 ignored`,
  OCR feature `23 passed`, `packageScriptContract: passed`,
  `acceptanceRealGateContract: passed`, `ocrSmokeMissingModelSelfTest:
  passed`, `requiredRealGates: 18 workspace gates, 2 OCR gates`,
  `liteSizeGate: passed`, and `liteExeBytes: 3565568`.
- After adding the frontend OCR readiness/probe contract:
  `scripts\verify-migration.ps1` now fails if the UI stops rendering OCR
  readiness flags, model directory, required model ready/missing states, model
  paths/sizes, the OCR backend probe button, the `ocr_backend_probe` invoke, or
  the attempted/initialized/reason/error/backend/profile/modelDir probe result
  fields.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the frontend OCR readiness/probe contract. The summary
  reported Rust core `117 passed, 2 ignored`, Tauri shell/backend `82 passed,
  16 ignored`, OCR feature `23 passed`, `frontendOcrReadinessContract:
  passed`, `packageScriptContract: passed`, `acceptanceRealGateContract:
  passed`, `ocrSmokeMissingModelSelfTest: passed`, `requiredRealGates: 18
  workspace gates, 2 OCR gates`, `liteSizeGate: passed`, and `liteExeBytes:
  3565568`.
- After adding the manual gate runbook:
  `docs\MANUAL_GATES.md` now records prerequisites, commands, and evidence
  expectations for the remaining hard manual gates: desktop smoke, real OCR
  model smoke, WebView source-preview visual smoke, template-gallery workflow,
  packaged tray menu/icon smoke, installer repeatability, and production
  template performance. `scripts\verify-migration.ps1` now reports
  `manualGateRunbookContract: passed` when the runbook keeps the required
  sections, commands, and evidence markers.
- After adding manual gate evidence records:
  `scripts\manual-gate-evidence.ps1` can generate one standard record per
  manual gate with `npm run manual:evidence -- -New`, and final signoff can run
  `npm run manual:evidence` to require every record to be present, fully
  filled, and marked `pass`. `npm run manual:evidence -- -Status` reports the
  current pass/blocked/fail/missing/incomplete/invalid count so partial
  progress cannot be confused with final completion. The migration verifier runs
  `scripts\manual-gate-evidence.ps1 -SelfTest`, including a negative check that
  a `blocked` record is rejected by pass-only validation.
- `npm run manual:evidence -- -Status`:
  at that evidence milestone reported `pass=4, blocked=1, fail=0, missing=3, incomplete=0,
  invalid=0`, with `baseline-before-manual-gates` and
  `desktop-backend-smoke` backed by pass records,
  `production-template-performance-smoke` and `installer-repeatability-smoke`
  backed by real build/install evidence, and
  `real-ocr-model-smoke` blocked by missing external OCR assets. This confirms
  the overall migration goal is not complete until the real OCR, WebView visual,
  template-gallery visual, and packaged tray evidence records are filled and
  pass.
- After adding the real WebView2/CDP visual smoke:
  `npm run webview:visual:smoke -- --gate source` passed against the current
  packaged release exe with an isolated `LOCALAPPDATA`, unique single-instance
  port, a visible helper window source, real source-preview refreshes, app-window
  resize/scroll/restore, and native window screenshots. `npm run
  webview:visual:smoke -- --gate gallery` passed against the same packaged
  WebView path with generated PNG/JPG/JPEG/BMP/WebP path imports, target
  enable/toggle-all actions, row-button reorder, drag/drop reorder, hit-count
  context menu clear, delete, clear-all, and screenshot-as-template capture.
  `npm run manual:evidence --
  -Status` then reported `pass=6, blocked=1, fail=0, missing=1, incomplete=0,
  invalid=0`; only the real OCR model smoke remains blocked by missing external
  model assets, and only the packaged tray menu/icon click smoke remains
  missing.
- After refreshing the final packaged WebView2/CDP full smoke:
  `node scripts\webview-visual-smoke.mjs --exe-path .\release-single\ScreenWatchOCRTauri.exe`
  passed in `webview-visual-smoke-20260707-153009-result.json` with source
  preview, legacy Python profile restore/scan/monitoring, template-gallery
  PNG/JPG/JPEG/BMP/WebP path imports, clipboard bitmap and file-list paste,
  one-shot scan evidence, OCR-lite raw config rejection, monitoring
  start/stop/restart, and all layout splitter drags in one final-exe run. The
  late-start remembered-window gate also passed in
  `webview-visual-smoke-20260707-153651-result.json`.
- Current default `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1`
  passed with Python inventory `98`, Python unittest `98`, Rust core `121
  passed, 3 ignored`, Tauri shell/backend `88 passed, 16 ignored`, OCR feature
  `25 passed`, frontend `103 passed`, frontend build `True`, release build
  `True`, `singleFileDeliverableContract: 3587584 bytes,
  B7356D3A96810AA70FEF42EE1FB360516411D145B1E8630F6A49F840C1EFE3A4,
  WindowsGui`, and `liteSizeGate: passed`.
- After hardening the evidence-directory open path:
  `cargo test -p screen-watch-ocr-tauri --lib` passed with Tauri shell/backend
  `91 passed, 16 ignored`. The new tests prove `open_evidence_dir` creates and
  returns the Python-compatible `ScreenWatchOCR\screenshots` directory without
  switching to the legacy `alerts` path, and that shell-open failures surface as
  command errors. The focused verifier
  `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1
  -SkipPython -SkipFrontend -SkipRelease` then passed with Rust core `124
  passed, 3 ignored`, Tauri shell/backend `91 passed, 16 ignored`, OCR feature
  `28 passed`, and the same final single-file deliverable hash
  `B7356D3A96810AA70FEF42EE1FB360516411D145B1E8630F6A49F840C1EFE3A4`.
- Current optional `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1
  -SkipPython -SkipFrontend -SkipRelease -IncludeTemplateBenchmark
  -IncludePackagedSmoke -IncludePortablePackage -IncludeFullPortablePackage`
  passed with template benchmarks `81ms` flat and `457ms` textured, packaged
  smoke `ran`, lite portable
  `screen-watch-ocr-tauri-lite-portable-20260707-065219-73bb7825.zip`
  verified at `1,616,329` bytes, and full portable
  `screen-watch-ocr-tauri-full-portable-20260707-065500-3652923c.zip`
  verified at `3,753,074` bytes.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease`:
  passed after adding the production-profile template benchmark gate and
  verifier contract. The summary reported Rust core `117 passed, 3 ignored`,
  Tauri shell/backend `82 passed, 16 ignored`, OCR feature `23 passed`,
  `manualGateRunbookContract: passed`, `frontendOcrReadinessContract: passed`,
  `acceptanceRealGateContract: passed`, `ocrSmokeMissingModelSelfTest: passed`,
  `requiredRealGates: 19 workspace gates, 2 OCR gates`, `liteSizeGate:
  passed`, and `liteExeBytes: 3565568`.
- After adding the production template performance smoke:
  `scripts\production-template-performance-smoke.ps1` chooses the shared
  `ScreenWatchOCR` profile with the most enabled template targets by default,
  runs the fixed Python/OpenCV-vs-Rust parity benchmark, then runs
  `benchmark_production_profile_template_scan` against the real profile and
  template files. Before bounding the coarse-refine search margin by downsample
  step, the real profile run took about 205 seconds for 18 targets; after the
  detector change, `npm run production:template:smoke` passed with 18/18
  production-profile matches in 8579ms on a 2560x1440 synthetic placement frame.
  Evidence is recorded in
  `docs\manual-gate-evidence\production-template-performance-smoke.md`.
- After completing the installer repeatability smoke:
  `npm run tauri:build:lite` and `npm run tauri:build:full` both produced NSIS
  installers. `scripts\build-tauri.mjs` now preserves flavor-specific installer
  copies and build-info sidecars, so `target\release\bundle\nsis` contains both
  `Screen Watch OCR Tauri_0.1.0_x64-lite-setup.exe` and
  `Screen Watch OCR Tauri_0.1.0_x64-full-setup.exe`, while `target\release` contains
  `screen-watch-ocr-tauri.lite.build-info.json` and
  `screen-watch-ocr-tauri.full.build-info.json`. After the Tauri identity split,
  the renamed lite/full installers also installed silently into
  `target\installer-smoke-tauri-identity-20260706-234853\lite` and
  `target\installer-smoke-tauri-identity-20260706-234853\full`, installed
  runtime smoke passed for both, no OCR model files were found under the
  installer or install-smoke directories, and the
  `full_build_reports_missing_external_models` OCR-feature test passed.
  Evidence is recorded in
  `docs\manual-gate-evidence\installer-repeatability-smoke.md`.
- After completing the packaged tray menu smoke:
  `npm run tray:smoke -- -ExePath target\release\screen-watch-ocr-tauri.exe -StartupWaitSeconds 5`
  passed against the current packaged lite exe. The smoke launches with isolated
  `LOCALAPPDATA` and `SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT`, proves the tray
  icon belongs to the Tauri PID through the `tray_icon_app` hidden window plus
  `Shell_NotifyIconGetRect`, opens the native `#32768` tray menu owned by the
  same PID, clicks the first menu item to show the `Screen Watch OCR Tauri` main
  window, then clicks the last menu item and verifies process exit code 0.
  `npm run manual:evidence -- -Status` now reports `pass=8, blocked=0, fail=0,
  missing=0, incomplete=0, invalid=0`. Evidence is recorded in
  `docs\manual-gate-evidence\packaged-tray-menu-and-icon-smoke.md`.
- `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1`:
  passed after closing the tray gate. The summary reported Python inventory
  `98`, Python unittest `98`, Rust core `117 passed, 3 ignored`, Tauri
  shell/backend `82 passed, 16 ignored`, OCR feature `23 passed`, frontend
  tests `89 passed`, frontend build `True`, release build `True`,
  `tauriIdentitySeparationContract: passed`, `manualGateEvidenceSelfTest:
  passed`, `liteSizeGate: passed`, Python exe `102,021,797` bytes, and current
  Tauri lite exe `3,576,832` bytes.
- After adding profile one-shot/monitoring hit-count auto-updates from
  cooldown-filtered alerted matches:
  `cargo test -p screen-watch-ocr-tauri --lib`: 51 tests passing, 8
  desktop-only manual gates ignored by default.
- After adding source preview signatures and in-memory preview-frame caching:
  `cargo test -p screen-watch-ocr-tauri --lib`: 56 tests passing, 8
  desktop-only manual gates ignored by default.
- After adding Windows DWM preview overlay state, commands, and frontend
  source-card handoff:
  `cargo test -p screen-watch-ocr-tauri --lib dwm_preview`: 3 tests passing.
- After adding a fake DWM thumbnail backend for lifecycle verification:
  `cargo test -p screen-watch-ocr-tauri dwm_preview`: 5 DWM tests passing,
  including same-source thumbnail reuse, source-handle replacement, stale-key
  retention, and clear-time unregister behavior.
- After adding a real Windows DWM smoke gate:
  `cargo test -p screen-watch-ocr-tauri real_dwm_thumbnail_registers_updates_and_clears_on_windows_desktop -- --ignored`
  passed. The gate creates two temporary Win32 windows, registers a DWM
  thumbnail, verifies same-source reuse on update, clears the preview state,
  and destroys the test-owned windows.
- After adding frontend target workflow helpers for the profile gallery:
  `npm run test:frontend`: 53 tests passing, including target enabled defaults,
  hit-count context-menu state, snake/camel hit-count parsing, drag/drop
  midpoint insert-index calculation, and fixed context-menu viewport fitting.
- `cargo test --workspace`: passed with 109 core tests and 67 Tauri
  shell/backend tests; 11 Tauri desktop-only manual gates ignored by default.
- Manual desktop capture gate:
  `cargo test -p screen-watch-ocr-tauri captures_tiny_screen_region_on_windows_desktop -- --ignored`
  passed.
- Manual real monitor listing gate:
  `cargo test -p screen-watch-ocr-tauri real_windows_monitor_listing_matches_python_mss_indexing_on_desktop -- --ignored`
  passed. It enumerates the real Windows desktop monitors, verifies Python/mss
  indexing semantics with virtual monitor `0` and physical monitors starting at
  `1`, and checks the virtual monitor bounds against Windows system metrics.
- Manual one-shot scan gate:
  `cargo test -p screen-watch-ocr-tauri one_shot_scan_captures_screen_region_and_writes_evidence -- --ignored`
  passed.
- Manual profile screen workflow gate:
  `cargo test -p screen-watch-ocr-tauri profile_screen_scan_workflow_records_template_hit_on_windows_desktop -- --ignored`
  passed. The gate captures a real 32x32 desktop region, saves a cropped image
  as a profile template, builds the profile watch config, scans the same screen
  region, writes evidence, and records the profile target hit count.
- Manual profile monitoring workflow gate:
  `cargo test -p screen-watch-ocr-tauri profile_monitoring_session_records_template_hit_on_windows_desktop -- --ignored`
  passed. The gate captures a real 32x32 desktop region, imports a cropped
  profile template, starts a real monitoring session through the shared session
  backend with `ProfileHitSink`, then verifies evidence and profile target
  hit-count persistence before stopping the worker.
- Manual profile window workflow gate:
  `cargo test -p screen-watch-ocr-tauri profile_window_scan_workflow_records_template_hit_on_windows_desktop -- --ignored`
  passed. The gate creates a temporary Win32 window, captures it through the
  real app-window capture path, imports a cropped template into a profile,
  builds a window-only profile watch config, scans the same real window source,
  writes evidence, records profile target hit count, and destroys the
  test-owned window.
- Manual one-shot window scan gate:
  `cargo test -p screen-watch-ocr-tauri one_shot_scan_captures_window_and_writes_evidence -- --ignored`
  passed.
- Manual monitoring session gate:
  `cargo test -p screen-watch-ocr-tauri session_start_runs_ticks_and_stop_joins_worker -- --ignored`
  passed.
- Manual window monitoring session gate:
  `cargo test -p screen-watch-ocr-tauri session_start_scans_window_source_and_writes_evidence -- --ignored`
  passed.
- Manual app-window enumeration gate:
  `cargo test -p screen-watch-ocr-tauri list_app_windows_enumerates_without_panic_on_windows_desktop -- --ignored`
  passed.
- Manual app-window preview capture gate:
  `cargo test -p screen-watch-ocr-tauri capture_first_app_window_preview_on_windows_desktop -- --ignored`
  passed.
- Manual app-window frame capture gate:
  `cargo test -p screen-watch-ocr-tauri capture_first_app_window_frame_on_windows_desktop -- --ignored`
  passed.
- Manual real DWM thumbnail gate:
  `cargo test -p screen-watch-ocr-tauri real_dwm_thumbnail_registers_updates_and_clears_on_windows_desktop -- --ignored`
  passed.
- The same 16 desktop-only gates are now repeatable through:
  `powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipFrontend -SkipRelease -IncludeDesktopSmoke`.
- `cargo check -p screen-watch-ocr-tauri`: passed.
- `npm install`: passed, 0 vulnerabilities reported.
- `npm run build`: passed.
- `npm run test:frontend`: 79 frontend behavior tests passing.
- `npx tauri build --no-bundle --ci`: built the lite app exe successfully
  through the Tauri frontend-asset pipeline, with only Cargo PDB
  filename-collision warnings.
- Optimized lite exe observed at `target\release\screen-watch-ocr-tauri.exe`:
  3,567,616 bytes, about 3.40 MiB.
- Installer bundling did not complete in this environment because NSIS download
  timed out. The app exe was already produced before the installer step.
- Portable zip packaging is available through `scripts\package-portable.ps1`
  and includes the exe, `portable-manifest.json`, and `README-portable.txt`
  while keeping OCR models external. The script now verifies the archive root,
  exe byte size, manifest fields, README presence, required OCR model list, and
  absence of bundled `.onnx` files or required OCR asset filenames.

## Must Preserve

- Data directory remains `ScreenWatchOCR` under the same OS-specific location.
- Legacy sibling `app_data` is copied non-destructively into `ScreenWatchOCR`
  on startup when present, without deleting the legacy source or overwriting
  existing shared files.
- Profiles remain under `profiles/profile_N.json`.
- Window layout remains under `state.json` `layout.geometry` using the Python
  `WIDTHxHEIGHT+X+Y` geometry format, while preserving existing layout ratios
  and unknown state fields.
- GUI screenshot retention remains under the Python-compatible global
  `state.json` `max_alerts`; profile saves clean stale Tauri-written
  `match.max_alerts` values instead of creating a per-profile retention
  setting that the Python app would ignore.
- Templates remain under `templates/`.
- Screenshots/alerts remain under `screenshots/` and `alerts.jsonl` unless the user
  explicitly changes the setting.
- The app must tolerate unknown JSON fields for forward/backward compatibility.
- Existing config fields remain valid:
  - `poll_interval_seconds`
  - `cooldown_seconds`
  - `template_workers`
  - `regions`
  - `windows`
  - `window_apps`
  - `targets`
  - `alarm`
- Existing alarm fields remain valid:
  - `beep`
  - `beep_seconds`
  - `beep_volume`
  - `save_dir`
  - `jsonl`
  - `max_alerts`
- Target kinds remain valid:
  - `template`
  - `pixel`
  - `ocr_text`
- Scale syntax remains compatible:
  - single value: `1.0`
  - comma list: `0.9,1.0,1.1`
  - numeric range: `0.5-2.0:0.1`
  - percent range: `0.1-2.0:5%`
- Lite build must run without OCR models.
- Lite build must not compile optional OCR Cargo dependencies by default.
- Full build must pass the `ocr` Cargo feature as well as the full runtime
  flavor.
- `npm run tauri:build:lite` and `npm run tauri:build:full` must keep using
  `scripts\build-tauri.mjs`, so the compiled flavor, OCR Cargo feature, and
  build-info/hash sidecar are produced consistently.
- Tauri bundle config must keep OCR models external: no bundled resources or
  external binaries may reference `.onnx`, `ppocrv5_dict.txt`, or the
  `models\rapidocr` directory; the main window starts hidden so tray/startup
  policy controls first visibility.
- Full build must not embed OCR models by default; models live outside the executable.
- Full build must report required OCR model file status and missing OCR models
  clearly.
- Full runtime flavor without a compiled OCR module must report that mismatch
  clearly instead of checking models or silently skipping OCR targets.
- Full build with all external models present must still keep OCR
  `available=false` until the native inference backend is linked. Once compiled
  with the `ocr` feature, the native backend is linked and requires the
  compatible `ppocrv5-dbnet-svtr` asset profile (`det.onnx`, `rec.onnx`,
  `ppocrv5_dict.txt`); invalid model contents fail explicitly when the backend
  initializes.
- OCR setup must expose an explicit backend probe that distinguishes skipped
  lite/not-compiled/missing-model states from attempted native initialization
  and actual initialization failure.
- OCR setup must provide a repeatable real-model smoke gate that can fail
  clearly when external assets are missing, fail clearly when native
  initialization fails, and optionally assert recognized text from a supplied
  PNG smoke image.
- Rust/Tauri must expose stable commands for app info, config validation,
  monitor listing, app-window listing, source resolution, window source
  resolution, screen-region preview capture, app-window preview capture, startup
  shortcut status/toggle, profile normalization, profile hit-count updates,
  native image selection, profile image importing, profile target
  reorder/remove/clear, profile target enable/toggle-all, profile-to-watch-config
  construction, profile one-shot scan, profile monitoring-session start, profile
  read snapshots, profile scan/monitor hit-count auto-update, guarded profile
  target PNG opening, guarded profile target thumbnail loading, DWM preview
  overlay sync/retention/clear, and OCR availability with required-model file
  statuses before the full UI is completed.

## Core Feature Gates

- Parse Python-style config JSON.
- Accept target-only CLI configs because empty regions default to physical
  monitors at source-resolution time.
- Validate at least one target during config parsing.
- Resolve at least one concrete screen/window source before monitoring starts,
  after applying default physical-monitor regions.
- Preserve Python/mss monitor indexing semantics: virtual monitor `0` is listed,
  while physical monitors start at `1` and monitor `0` is not valid for region
  capture.
- Window-only configs remain window-only instead of implicitly selecting every
  physical monitor.
- Tauri exposes monitor listing and config source-resolution commands.
- Tauri enumerates selectable Windows app windows with Python-compatible
  filtering, title sorting, duplicate ordinal numbering, legacy `title\0ordinal`
  keys, and display labels.
- Tauri resolves remembered `window_apps` config entries to currently available
  concrete window sources while reporting missing remembered apps.
- Tauri can capture a Windows screen region into an RGB frame and PNG preview.
- Tauri can cache source preview frames by stable source key and source
  signature, reusing screen/window preview frames until source geometry changes
  and dropping stale preview keys when the selected source set changes.
- The frontend can render a basic multi-source preview panel for the currently
  selected physical monitors and app windows through cached Tauri preview
  commands. Source previews auto-refresh through a single timer, prune stale
  preview cache keys, and skip refreshes while the page is hidden, window resize
  is settling, or template drag/drop is active. Window-source cards now also
  attempt Windows DWM thumbnail overlay handoff using the current visible card
  rect while keeping bitmap capture as the fallback. Desktop visual smoke for
  the overlay remains open.
- Tauri can capture a Windows app window into an RGB frame and PNG preview,
  using visible capture fallback when PrintWindow returns black output.
- Window capture preserves Python black-frame detection and black-padding crop
  behavior.
- Tauri can run a one-shot scan from config text for screen regions and concrete
  app-window sources: resolve sources, capture frames, detect targets, apply
  cooldown, and write evidence.
- Tauri can start/stop a persistent screen/window monitoring session that keeps
  `ScanEngine` cooldown state across ticks, refreshes remembered app windows,
  and joins its worker on stop.
- Tauri emits `screen-watch://monitor-session` events for monitoring start,
  tick, hit/error, and stop snapshots, and the frontend listens for live status
  updates.
- Parse and cap scale expansion at 120 values.
- Detect pixel targets with RGB tolerance.
- Detect template targets in deterministic test frames.
- Use Python-style large-frame template coarse/refine matching, including
  0.5/0.25 coarse factors, candidate refinement, and fallback when downscaling
  would erase textured-template detail.
- Load PNG/JPG/JPEG/BMP/WEBP template files from config-relative paths.
- Parse and honor Python-compatible `template_workers`, clamp zero to one
  effective worker, cap workers to template job count, and preserve configured
  target order after parallel template matching.
- Provide repeatable large-frame flat and textured multi-template benchmark
  gates that verify match count and locations while reporting elapsed time,
  with an optional machine-specific max-ms threshold.
- Preserve target order for prepared pixel/template detection.
- Normalize profile template records while preserving unknown JSON fields.
- Read profile snapshots without rewriting missing or unknown profile data, and
  report target count, enabled count, all-enabled state, targets, and raw JSON.
- Rename template files to fill profile/count gaps.
- Delete only template files that are proven to be under `templates/`.
- Add PNG/JPG/JPEG/BMP/WEBP template images to a profile by pruning before
  naming, saving normalized RGB PNGs under `templates/`, preserving unknown
  profile fields, and deleting only pruned template files under `templates/`.
- Paste images into a profile from the Windows clipboard when the clipboard
  contains supported image file paths or a CF_DIB/CF_DIBV5 bitmap, while using
  the same prune-before-name and PNG-normalization path as file imports.
- Capture the current selected screen/window source as a template even when the
  profile has no existing targets, preferring selected screen regions before
  windows like the Python `capture_target_frame` path, and reporting the
  Python-style no-source/window-black errors.
- Reorder profile targets while renaming template files by the new position and
  preserving stable target ids.
- Remove or clear profile targets while deleting only files proven to be under
  `templates/`.
- Treat missing profile target `enabled` as true, skip disabled profile targets
  when building template detector targets, and preserve stable ids.
- Set an individual profile target's enabled state and toggle all profile
  targets using Python-compatible select-all/invert behavior.
- Build a `WatchConfig` from a GUI profile using Python-compatible profile
  defaults: enabled templates only, selected screen/window/app sources,
  `screenshots` and `alerts.jsonl` evidence paths, `source_workers`,
  `template_workers`, and `min_idle_seconds` compatibility fields.
- Tauri can run a one-shot scan or start a persistent monitoring session
  directly from a GUI profile, sharing the same source resolution, capture,
  scan, evidence, beep, and monitoring event path as config-text commands.
- Record target hit counts by stable target id and preserve unknown profile
  fields.
- Clear target hit counts by stable target id.
- Tauri exposes profile normalization, hit recording, and hit-count clearing
  commands, plus profile image importing, profile clipboard image paste,
  screenshot-as-template capture, and profile target reorder/remove/clear,
  enable/toggle-all, profile read, and profile
  watch-config commands.
- The frontend profile panel renders target rows and wires basic workflow
  controls for enable/disable, select-all/invert, up/down reorder, delete, clear
  all, hit-count clear, path-based image import, native image selection,
  clipboard image paste with Ctrl+V focus guards, screenshot-as-template
  capture, fixed-size PNG thumbnails, hit-count badges, same-card
  repeat-click/default-app PNG opening, drag/drop reorder, and a right-click
  hit-count menu. Frontend unit
  tests now cover target enabled defaults, hit-count context-menu enablement,
  profile-load first-target selection, profile-refresh selection preservation,
  enabled/total-count status text after target enabled changes,
  drag/drop midpoint insert-index calculation, paste shortcut editable-control
  guards, screenshot-as-template empty-profile gating, and context-menu viewport
  fitting. Full desktop smoke verification remains open.
- The frontend can select physical monitor sources and app-window sources,
  choose Python-compatible remembered `window_apps` or concrete hwnd-backed
  `windows`, generate a GUI-profile-derived `WatchConfig`, run a one-shot
  profile scan, and start a profile monitoring session through the shared Tauri
  scan path. It also restores and persists profile source choices and match
  settings through Python-compatible `monitors`, `windows`, `region`, `match`,
  and `state.json` `last_profile` fields. Core tests now also lock frontend
  camelCase option deserialization, concrete window `ordinal` preservation, and
  remembered-window persistence precedence. Real screen profile scan and
  profile-monitoring desktop smoke gates now cover the backend workflow.
- Tauri restores `state.json` `layout.geometry` on main-window setup and saves
  updated visible main-window outer geometry on resize/move/scale changes,
  preserving Python-compatible geometry strings, existing layout ratios, and
  unknown state fields while ignoring minimized/hidden taskbar placeholder
  geometry.
- Profile one-shot scans and profile monitoring sessions record target
  `hit_count` values by stable target id only for cooldown-filtered alerted
  matches, matching the Python worker path that emits `target_hits` after
  cooldown filtering. The frontend refreshes the active profile on profile
  monitoring hit events so badges can update. A real Windows desktop profile
  monitoring smoke now verifies evidence writing and profile `hit_count`
  persistence through the shared session backend.
- Scan a supplied region frame through the prepared detector, per-region/target
  cooldown, and evidence writer.
- Suppress repeated alerts during cooldown while still reporting raw matches.
- Save red-box annotated alert screenshots with target labels as PNG.
- Append alert JSONL events with time, region, matches, and screenshot path.
- Prune alert screenshots after writing new evidence.
- Clamp alarm beep volume to 0..100, generate Python-style in-memory WAV
  beeps, and avoid restarting an already active beep.
- Build the Windows Startup shortcut path using the distinct
  `屏幕监控OCR Tauri.lnk` link name, use `--start-minimized` for packaged app
  startup, preserve the Python development argument shape for compatibility
  tests, and expose explicit startup status/toggle commands.
- Use the distinct Tauri single-instance wake protocol on `127.0.0.1:47628`,
  notify an existing Tauri instance with `ScreenWatchOCRTauri:show\n`, expect
  `ok\n`, exit the second process on successful notification, and
  show/unminimize/focus the main Tauri window on wake.
- Install a Tauri tray icon with show/exit menu actions, hide the main window on
  close only when tray creation succeeds, keep the window visible when tray
  creation fails, honor the legacy `--start-minimized` argument only after tray
  creation succeeds, and update the tray tooltip/icon from monitoring state.
- Represent OCR targets as optional capability without breaking lite builds.
- Keep the optional OCR module behind Cargo feature `ocr`, with `lite` builds
  using default features and `full` builds enabling that feature.
- Match OCR text rows against `ocr_text` targets with Python-compatible
  case-sensitivity and minimum-score behavior once an OCR backend supplies rows.
- Route OCR targets through a scan-engine OCR backend interface shared by
  one-shot scans and monitoring sessions; unavailable lite/full/model/backend
  states must return explicit errors instead of silently skipping OCR targets.
- Create runtime OCR backends through one shared settings-based factory so
  one-shot scans and monitoring sessions cannot diverge when the native
  inference backend is added.
- Resolve OCR model directory from:
  - `SCREENWATCH_OCR_MODEL_DIR`
  - default app data model directory
- Report per-required-model file status, separate model readiness from native
  backend readiness, and treat final OCR availability as
  `modelsReady && backendReady`.
- Render OCR readiness flags and required model file statuses in the frontend
  app-info view, including backend name and model profile.
- Provide a manual OCR backend probe in the frontend that can validate native
  backend initialization without starting monitoring.
- Provide `scripts\ocr-smoke.ps1` and `-IncludeOcrSmoke` verification wiring for
  real external OCR assets and optional PNG recognition smoke.
- Report lite/full build flavor.

## Future Manual Gates

- Multi-monitor screen capture and scan integration beyond the single-region
  one-shot desktop gate.
- Full production monitoring lifecycle: app lifecycle hooks, background
  behavior, and UI workflow wiring.
- Optional packaged tray visual hover recording beyond the automated smoke:
  real show/exit tray menu clicks are now covered by `scripts\tray-menu-smoke.ps1`;
  backend tray menu, click routing, generated icon pixels, tooltip text,
  `--start-minimized`, close-to-tray, and single-instance wake paths have
  repeatable coverage.
- Alert image full UI/runtime integration.
- OpenCV/Python performance comparison against representative production
  template sets; the fixed synthetic parity script exists and currently records
  Python/OpenCV ahead of pure Rust on that workload.
- Broader OCR quality coverage beyond the current 9-case generated English and
  Chinese PP-OCRv5 corpus: PP-OCRv6/RapidOCR-native profile compatibility,
  varied real screenshot accuracy, and production OCR workload performance
  remain future validation items.
