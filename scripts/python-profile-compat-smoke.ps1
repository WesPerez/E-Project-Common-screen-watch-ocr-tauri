param(
    [string]$PythonProject = "",
    [string]$ResultPath = ""
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $PythonProject) {
    $PythonProject = Join-Path $ProjectRootPath "..\screen-watch-ocr"
}
$PythonProjectPath = (Resolve-Path $PythonProject).Path
if (-not $ResultPath) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $ResultPath = Join-Path $ProjectRootPath "docs\manual-gate-evidence\logs\python-profile-compat-smoke-$stamp-result.json"
}
$ResultPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ResultPath)

function Resolve-PythonExe {
    param([string]$ProjectPath)

    $venvPython = Join-Path $ProjectPath ".venv\Scripts\python.exe"
    if (Test-Path -LiteralPath $venvPython) {
        return (Resolve-Path $venvPython).Path
    }
    return "python"
}

function Write-Utf8NoBom {
    param(
        [string]$Path,
        [string]$Text
    )
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}

$smokeId = ([guid]::NewGuid().ToString("N")).Substring(0, 8)
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-python-profile-compat-$smokeId"
$dataDir = Join-Path $tempRoot "ScreenWatchOCR"
$profilesDir = Join-Path $dataDir "profiles"
$templatesDir = Join-Path $dataDir "templates"
$screenshotsDir = Join-Path $dataDir "screenshots"
$profilePath = Join-Path $profilesDir "profile_1.json"
$statePath = Join-Path $dataDir "state.json"
$templatePath = Join-Path $templatesDir "1-1-tauri-compat.png"
$helperPath = Join-Path $tempRoot "python_read_tauri_profile.py"
$pythonExe = Resolve-PythonExe $PythonProjectPath
$failure = $null

$result = [ordered]@{
    status = "running"
    timestamp = (Get-Date).ToString("o")
    machine = $env:COMPUTERNAME
    pythonProject = $PythonProjectPath
    pythonExe = $pythonExe
    tempRoot = $tempRoot
    dataDir = $dataDir
    profilePath = $profilePath
    statePath = $statePath
    templatePath = $templatePath
    loaded = $null
    pythonSave = [ordered]@{}
    cleanup = [ordered]@{}
}

try {
    New-Item -ItemType Directory -Force -Path $profilesDir, $templatesDir, $screenshotsDir | Out-Null

    # 4x4 valid PNG so the legacy Python gallery can open it if thumbnails are rendered.
    $pngBase64 = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAEElEQVR4nGP8z4AATAxEcQAz0QEHOoQ+uAAAAABJRU5ErkJggg=="
    [IO.File]::WriteAllBytes($templatePath, [Convert]::FromBase64String($pngBase64))

    $profileFixture = [ordered]@{
        targets = @(
            [ordered]@{
                id = "tauri-target-id"
                name = "1-1-tauri-compat"
                path = $templatePath
                size = "4x4"
                enabled = $false
                hit_count = 3
                future_target = $true
            }
        )
        monitors = @()
        windows = @(
            [ordered]@{
                title = "Tauri Compatibility Window"
                ordinal = 2
                future_window = $true
            }
        )
        region = [ordered]@{
            monitor = 1
            left = 11
            top = 22
            width = 333
            height = 444
        }
        match = [ordered]@{
            threshold = 0.82
            scales = "0.9,1.0,1.1"
            interval_ms = 333
            cooldown = 1.25
            beep = $false
            beep_seconds = 4.5
            beep_volume = 37
            max_templates = 12
            future_match = $true
        }
        future_profile = [ordered]@{
            written_by = "tauri-compat-smoke"
        }
    }
    $stateFixture = [ordered]@{
        last_profile = 1
        layout = [ordered]@{
            geometry = "980x680+20+30"
            main_ratio = 0.44
            right_ratio = 0.22
            left_ratio = 0.61
            future_layout = $true
        }
        max_alerts = 13
        future_state = $true
    }
    Write-Utf8NoBom -Path $profilePath -Text ($profileFixture | ConvertTo-Json -Depth 8)
    Write-Utf8NoBom -Path $statePath -Text ($stateFixture | ConvertTo-Json -Depth 8)

    $helper = @'
import json
import os
import pathlib
import sys

from screen_watch import app as appmod

data_dir = pathlib.Path(sys.argv[1])
profile_path = data_dir / "profiles" / "profile_1.json"
state_path = data_dir / "state.json"

appmod.DATA_DIR = data_dir
appmod.PROFILES_DIR = data_dir / "profiles"
appmod.STATE_PATH = state_path
appmod.ALERTS_DIR = data_dir / "screenshots"
appmod.LEGACY_DATA_DIR = data_dir.parent / "missing_legacy_app_data"

root = appmod.Tk()
root.withdraw()
try:
    app = appmod.App(root)
    loaded = {
        "current_profile": app.current_profile,
        "left": str(app.left.get()),
        "top": str(app.top.get()),
        "width": str(app.width.get()),
        "height": str(app.height.get()),
        "threshold": float(app.threshold.get()),
        "scales": str(app.scales.get()),
        "interval_ms": int(app.interval_ms.get()),
        "cooldown": float(app.cooldown.get()),
        "beep": bool(app.beep.get()),
        "beep_seconds": float(app.beep_seconds.get()),
        "beep_volume": int(app.beep_volume.get()),
        "max_templates": int(app.max_templates.get()),
        "max_alerts": int(app.max_alerts.get()),
        "target_count": len(app.targets),
        "target_enabled": bool(app.targets[0].get("enabled", True)) if app.targets else None,
        "target_hit_count": int(app.targets[0].get("hit_count", 0)) if app.targets else None,
        "selected_apps": app.selected_apps,
        "selected_monitors": [i for i, var in app.monitor_vars.items() if var.get()],
    }
    assert loaded["current_profile"] == 1, loaded
    assert loaded["left"] == "11", loaded
    assert loaded["top"] == "22", loaded
    assert loaded["width"] == "333", loaded
    assert loaded["height"] == "444", loaded
    assert abs(loaded["threshold"] - 0.82) < 0.000001, loaded
    assert loaded["scales"] == "0.9,1.0,1.1", loaded
    assert loaded["interval_ms"] == 333, loaded
    assert abs(loaded["cooldown"] - 1.25) < 0.000001, loaded
    assert loaded["beep"] is False, loaded
    assert abs(loaded["beep_seconds"] - 4.5) < 0.000001, loaded
    assert loaded["beep_volume"] == 37, loaded
    assert loaded["max_templates"] == 12, loaded
    assert loaded["max_alerts"] == 13, loaded
    assert loaded["target_count"] == 1, loaded
    assert loaded["target_enabled"] is False, loaded
    assert loaded["target_hit_count"] == 3, loaded
    assert loaded["selected_apps"] == [{"title": "Tauri Compatibility Window", "ordinal": 2}], loaded
    assert loaded["selected_monitors"] == [], loaded

    app.save_current_profile()
    app.save_state()
finally:
    root.destroy()

saved_profile = json.loads(profile_path.read_text(encoding="utf-8"))
saved_state = json.loads(state_path.read_text(encoding="utf-8"))
saved = {
    "profile_has_required_keys": all(key in saved_profile for key in ["targets", "monitors", "windows", "region", "match"]),
    "state_has_required_keys": all(key in saved_state for key in ["last_profile", "layout", "max_alerts"]),
    "future_profile_preserved_after_python_save": "future_profile" in saved_profile,
    "future_target_preserved_after_python_save": bool(saved_profile.get("targets") and "future_target" in saved_profile["targets"][0]),
    "future_match_preserved_after_python_save": "future_match" in saved_profile.get("match", {}),
    "future_state_preserved_after_python_save": "future_state" in saved_state,
    "future_layout_preserved_after_python_save": "future_layout" in saved_state.get("layout", {}),
}
assert saved["profile_has_required_keys"], saved_profile
assert saved["state_has_required_keys"], saved_state

print(json.dumps({"loaded": loaded, "pythonSave": saved}, ensure_ascii=False))
'@
    Write-Utf8NoBom -Path $helperPath -Text $helper

    $oldPythonPath = $env:PYTHONPATH
    $oldErrorActionPreference = $ErrorActionPreference
    try {
        $env:PYTHONPATH = Join-Path $PythonProjectPath "src"
        $ErrorActionPreference = "Continue"
        $oldNativeCommandUseErrorActionPreference = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
        try {
            $output = & $pythonExe $helperPath $dataDir 2>&1
        } finally {
            $PSNativeCommandUseErrorActionPreference = $oldNativeCommandUseErrorActionPreference
        }
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
        $env:PYTHONPATH = $oldPythonPath
    }
    if ($exitCode -ne 0) {
        throw "Python profile compatibility helper failed with exit code ${exitCode}: $($output | Out-String)"
    }
    $parsed = ($output | Out-String | ConvertFrom-Json)
    $result.loaded = $parsed.loaded
    $result.pythonSave = $parsed.pythonSave
    $result.status = "pass"

    Write-Host "pythonProject: $PythonProjectPath"
    Write-Host "pythonExe: $pythonExe"
    Write-Host "dataDir: $dataDir"
    Write-Host "profilePath: $profilePath"
    Write-Host "statePath: $statePath"
    Write-Host "loadedCurrentProfile: $($result.loaded.current_profile)"
    Write-Host "loadedTargetCount: $($result.loaded.target_count)"
    Write-Host "loadedSelectedApps: $($result.loaded.selected_apps | ConvertTo-Json -Compress)"
    Write-Host "loadedSelectedMonitors: $($result.loaded.selected_monitors | ConvertTo-Json -Compress)"
    Write-Host "pythonSaveProfileRequiredKeys: $($result.pythonSave.profile_has_required_keys)"
    Write-Host "pythonSaveStateRequiredKeys: $($result.pythonSave.state_has_required_keys)"
    Write-Host "pythonSavePreservedFutureProfile: $($result.pythonSave.future_profile_preserved_after_python_save)"
    Write-Host "pythonProfileCompatSmokeVerified: True"
} catch {
    $failure = $_
    $result.status = "fail"
    $result.error = $_.Exception.Message
} finally {
    $result.cleanup.tempRootRemoved = $false
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
        $result.cleanup.tempRootRemoved = $true
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $ResultPath) | Out-Null
    $result.resultPath = $ResultPath
    $result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $ResultPath -Encoding UTF8
    Write-Host "resultPath: $ResultPath"
    Write-Host "cleanupTempRootRemoved: $($result.cleanup.tempRootRemoved)"
}

if ($failure) {
    throw $failure
}
