# Screen Watch OCR Tauri

Rust/Tauri successor for `screen-watch-ocr`.

This project is intentionally built beside the Python app first. The Python app remains
the behavior baseline while this version grows toward the long-term architecture:

- Rust/Tauri main application.
- Shared, versioned Rust core crate for config, detection, and platform integrations.
- Optional OCR module.
- External OCR model directory.
- `lite` and `full` build flavors.
- Compatible user data directory and profile/config JSON shape.

Decision record: [docs/DECISION.md](docs/DECISION.md)

Functional migration gates: [docs/FUNCTIONAL_ACCEPTANCE.md](docs/FUNCTIONAL_ACCEPTANCE.md)

Manual gate runbook: [docs/MANUAL_GATES.md](docs/MANUAL_GATES.md)

Current measured lite executable size after release-size tuning:

- Python/PyInstaller baseline: 102,021,797 bytes, about 97.3 MiB.
- Rust/Tauri lite exe: 3,565,568 bytes, about 3.40 MiB and about 3.50% of the
  Python executable.
- Rust/Tauri full exe with native OCR backend linked: last verified at
  9,515,008 bytes, about 9.07 MiB, with OCR models still external.

The migration verifier now treats the lite size as a guardrail, not just a
report: when `target\release\screen-watch-ocr-tauri.exe` exists, it must stay
under 15 MiB and under 25% of the Python/PyInstaller exe size by default.
The verifier reports `liteExeBytes` separately from the current
`tauriExeBytes`, so a later full build/package step cannot accidentally satisfy
the lite guardrail by ambiguity. It also checks Cargo metadata to keep the OCR
inference dependency behind the explicit `ocr` feature instead of the default
lite build.

Current core migration tests:

- Python baseline: 98 tests.
- Rust core: 117 tests, plus 3 ignored template benchmark gates.
- Tauri shell/backend: 82 tests, plus 16 desktop-only manual gates ignored by
  default.
- Frontend behavior: 89 tests.

## Compatibility Contract

The user data directory remains `ScreenWatchOCR`, matching the Python app:

- Windows: `%LOCALAPPDATA%\ScreenWatchOCR`
- macOS: `~/Library/Application Support/ScreenWatchOCR`
- Linux: `$XDG_DATA_HOME/ScreenWatchOCR` or `~/.local/share/ScreenWatchOCR`

On startup, the Tauri app also attempts a non-destructive copy migration from a
legacy sibling `app_data` directory into the shared user data directory. The
source is left in place, and existing files in `ScreenWatchOCR` are not
overwritten.

Profile/config JSON should stay readable by both applications. New fields must be
optional and unknown fields must be preserved or ignored safely.

## Commands

```powershell
cargo test -p screen-watch-core
npm install
npm run test:frontend
npm run verify:migration
npm run ocr:smoke
npm run template:benchmark
npm run template:parity
npm run production:template:smoke
npm run packaged:smoke
npm run manual:evidence
npm run webview:visual:smoke
npm run package:portable:lite
npm run package:portable:full
npm run tauri:dev
npm run tauri:build:lite
npm run tauri:build:full
```

The migration verifier runs a static Python test inventory check, the Python
98-test baseline, Rust formatting, Cargo OCR feature-boundary and dependency
tree checks, workspace tests, OCR feature checks, frontend unit tests, frontend
production build, a no-bundle Tauri lite app build, and executable size
reporting.
It also parses the locked Python baseline test names plus the Rust core, Tauri
shell/backend, OCR feature, and frontend test counts, then fails if they drop
below the current migration baselines. Named real/manual gates are checked too:
the flat/textured/production-profile large-template benchmarks, 16 desktop smoke gates, and 2 OCR real-model gates
must remain present even when skipped by default. It additionally checks that
those real/manual gate names are listed in `docs\FUNCTIONAL_ACCEPTANCE.md`, so
the executable smoke entrypoints and the human-readable migration checklist
cannot silently drift apart. It also locks the main `package.json` command
entrypoints for migration verification, frontend tests, OCR smoke, template
benchmarks, production template smoke, packaged/portable smoke, manual gate evidence, and Tauri lite/full builds. Remaining
manual gates are captured in `docs\MANUAL_GATES.md`, and the verifier checks
that runbook for required sections, commands, prerequisites, and evidence
notes. `scripts\manual-gate-evidence.ps1` can create and validate the concrete
evidence records; the verifier runs its self-test so a final pass cannot be
claimed by vague or incomplete manual notes. Use
`npm run manual:evidence -- -Status` to see the current pass/blocked/fail/missing
manual-gate count before deciding what to verify next. The verifier
also checks that
every frontend `invoke("...")` command in `src\main.js` is registered in the
Tauri `generate_handler![...]` list, so UI actions cannot silently lose their
  backend command during migration. The frontend argument keys for those invokes
  are checked against the backend command parameters as well, so `profileNumber`,
  `sourceKey`, `baseDir`, and similar bridge names cannot drift from their Rust
  counterparts. The reverse direction is checked too: every
  backend `#[tauri::command]` function must be in `generate_handler![...]`, so a
  new command cannot be written but left unreachable. The verifier also locks the
  lite/full npm build entrypoints to `scripts\build-tauri.mjs`, including the
  `SCREENWATCH_BUILD_FLAVOR` env handoff, full-build `--features ocr`, and
  release build-info/hash sidecar. Successful bundle builds also preserve
  flavor-specific installer and build-info copies for repeatability evidence.
  Tauri bundle config is checked as well:
  frontend assets must come from `../dist`, the main window must start hidden
  for tray/startup policy, the current verified installer target remains NSIS,
  and bundle resources/external binaries must not reference OCR model assets.
  The live monitoring event
  name is checked across the frontend listener and backend emitter as well, so
monitoring status updates cannot silently drift apart. The verifier also checks
that monitor-session events update the tray tooltip/icon from
`snapshot.running` before the frontend event is emitted. The source-preview DWM
handoff path is checked too: visible card rect calculation must stay in the
tested behavior layer, `sync_dwm_preview` must keep using that rect, and bitmap
fallback/cleanup paths must remain wired. Hard-coded frontend `#id` selectors in
`src\main.js` are checked
against the root `index.html`, so renamed or removed controls cannot silently
break UI event wiring. Static action controls are checked too: buttons must keep
click handlers, and selects/checkboxes must keep change handlers. Tauri backend tests also
lock the monitor-session event payload and the `app_info`/OCR readiness JSON
shape used by the frontend, including `tickHitCount`, `skippedWindowApps`,
`buildFlavor`, `modelsReady`, `backendReady`, `requiredModels`, and per-model
status fields. The frontend OCR readiness/probe path is locked too: the UI
must keep showing model readiness, model paths/sizes, and explicit
`ocr_backend_probe` result fields. Core tests keep `state.json` Python-compatible on disk while
serializing Tauri command results as frontend-facing camelCase, such as
`lastProfile`. The verifier also checks that the portable package script still agrees with the Rust core on the shared
`ScreenWatchOCR` app-data name, `SCREENWATCH_OCR_MODEL_DIR`, the external
`models\rapidocr` default, and the required native OCR asset filenames.
For quicker local loops:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease
```

On an interactive Windows desktop, add `-IncludeDesktopSmoke` to run the 16
ignored real monitor/capture/window/profile-session/DWM smoke gates.

Add `-IncludePortablePackage` to verify creation and zip-content integrity of a
portable lite package under `target\portable` without relying on NSIS/WiX
installer downloads.

Add `-IncludeFullPortablePackage` to build and verify the full portable package
with the optional OCR Cargo feature enabled while still keeping OCR models
external.

Add `-IncludeOcrSmoke` to run the ignored real-model OCR smoke gate. Pass
`-OcrModelDir` when the model files are not under the default app-data model
directory. Pass both `-OcrSmokeImage` and `-OcrSmokeExpect` to verify actual
recognition against a PNG smoke image in addition to backend initialization:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeOcrSmoke -OcrModelDir "D:\Models\rapidocr"
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeOcrSmoke -OcrModelDir "D:\Models\rapidocr" -OcrSmokeImage ".\smoke.png" -OcrSmokeExpect "READY"
```

Add `-IncludeTemplateBenchmark` to run the ignored large-frame flat and textured
multi-template benchmarks. They verify match count/locations and print elapsed time; pass
`-TemplateBenchmarkMaxMs` only when you want a hard machine-specific threshold:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeTemplateBenchmark
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeTemplateBenchmark -TemplateBenchmarkMaxMs 5000
```

Use `npm run template:parity` for release-mode Rust vs Python/OpenCV comparison
on the fixed 2560x1440, 8-template flat workload, plus a textured workload that
records the Python baseline's current odd-phase miss count while requiring Rust
to hit all 8 targets.

Use `npm run production:template:smoke` to run the fixed parity benchmark plus
the ignored production-profile template gate against the shared
`%LOCALAPPDATA%\ScreenWatchOCR` profile with the most enabled template targets.
Pass `-- -ProfilePath "..."` or `-- -DataDir "..."` to point it at another
compatible profile/data directory.

Add `-IncludePackagedSmoke` after a release exe exists, or without
`-SkipRelease` to build it first, to launch the packaged app with
isolated temporary app-data directories and single-instance ports. The smoke
stages the exe in a temporary app root with a legacy `app_data` fixture, then
verifies non-destructive migration into `ScreenWatchOCR`, restored
Python-compatible `state.json` `layout.geometry`, `--start-minimized`,
close-to-tray hiding without process exit, and single-instance wake restoring
the main window. The default packaged smoke wait is 18 seconds to avoid false
failures from slow desktop/tray event delivery:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -IncludePackagedSmoke
```

`lite` excludes bundled OCR models and does not compile the optional OCR Cargo
feature. `full` compiles with `--features ocr`; the runtime flavor is packaged
from the build-time `SCREENWATCH_BUILD_FLAVOR` value when present and otherwise
follows the compiled OCR feature boundary, so a standalone full exe still
reports full without a runtime environment variable. The core crate build
script tracks that build input and writes the normalized
`SCREENWATCH_COMPILED_BUILD_FLAVOR` value into the executable. A user's runtime
environment cannot flip an already packaged exe between lite and full. The
migration verifier checks that both default feature sets stay empty,
`screen-watch-core` keeps `pure-onnx-ocr` optional, and Tauri only enables it
through `screen-watch-core/ocr`. It also checks the resolved dependency trees:
lite must not include `pure-onnx-ocr` or `tract-onnx`, while full must include
them. Full expects models to be available from the external model directory or a
future installer step. OCR availability now reports `modelsReady` separately
from `backendReady`, including per-file status for every required model.
`available` remains false until both the external models are present and the
native inference backend is linked, so OCR targets fail with an explicit
disabled, not-compiled, missing-model, or unavailable-backend message.
The frontend shows the same readiness flags and required model file statuses so
full-build setup problems are visible without reading logs. A manual OCR backend
probe can also attempt native initialization after the required external model
files are present, without starting monitoring.

The app exe is produced at:

```text
target\release\screen-watch-ocr-tauri.exe
```

Installer bundling may download platform packaging tools such as NSIS. The
verifier uses `tauri build --no-bundle` so the executable is produced through
the real Tauri frontend-asset pipeline without requiring installer downloads,
then checks the Cargo OCR feature/dependency boundaries and lite exe size
guardrail.
Portable zip packages can be created with `npm run package:portable:lite` or
`npm run package:portable:full`; the packaging script validates the archive
root, exe size, exe SHA-256, build-info sidecar, manifest fields, README
presence, required OCR model list, and absence of bundled `.onnx` files or
required OCR asset filenames. When `-SkipBuild` is used, the script requires
the release exe's `screen-watch-ocr-tauri.build-info.json` sidecar to match the
requested lite/full flavor and the current executable hash before it will write
a portable manifest.

## OCR Models

Default external model directory:

```text
%LOCALAPPDATA%\ScreenWatchOCR\models\rapidocr
```

Override with:

```powershell
$env:SCREENWATCH_OCR_MODEL_DIR = "D:\Models\rapidocr"
```

Required default asset filenames for the native Rust OCR backend:

- `det.onnx`
- `rec.onnx`
- `ppocrv5_dict.txt`

The Python baseline still uses RapidOCR. The Rust `full` flavor currently wires
a native `pure-onnx-ocr` backend that expects PP-OCRv5-style DBNet/SVTR ONNX
assets plus a UTF-8 dictionary. PP-OCRv6/RapidOCR model names are reported as
reference status only until a compatible native backend is added for that exact
model profile. Use the frontend OCR backend probe, or the `ocr_backend_probe`
Tauri command, to validate that the linked backend can actually initialize the
external model files.

The repeatable command-line smoke gate is:

```powershell
npm run ocr:smoke -- -SelfTestMissingModels
npm run ocr:smoke -- -ModelDir "D:\Models\rapidocr"
npm run ocr:smoke -- -ModelDir "D:\Models\rapidocr" -Image ".\smoke.png" -Expect "READY"
```

The smoke script preflights the effective model directory before running Cargo
tests. If any required asset is missing, it prints `modelDir`, each
`modelMissing` path, and the exact `-ModelDir`/`SCREENWATCH_OCR_MODEL_DIR`
setup hint instead of failing later inside native OCR initialization.
`-SelfTestMissingModels` runs that missing-asset path against an isolated temp
directory and exits after confirming all required filenames are reported.
The migration verifier also checks this smoke script against the Rust OCR
constants, so the required model filenames, default model directory, and
environment variable cannot drift silently.
