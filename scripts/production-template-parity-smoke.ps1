param(
    [string]$DataDir = "",
    [string]$ProfilePath = "",
    [string]$Frame = "2560x1440",
    [string]$Scales = "1.0",
    [double]$Threshold = 0.90,
    [int]$TemplateWorkers = 2,
    [string]$PythonProject = "",
    [string]$ResultPath = "",
    [switch]$RustDebug
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $PythonProject) {
    $PythonProject = Join-Path $ProjectRootPath "..\screen-watch-ocr"
}
$PythonProjectPath = (Resolve-Path $PythonProject).Path
if (-not $DataDir) {
    $DataDir = Join-Path $env:LOCALAPPDATA "ScreenWatchOCR"
}
$DataDir = (Resolve-Path $DataDir).Path
if (-not $ResultPath) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $ResultPath = Join-Path $ProjectRootPath "docs\manual-gate-evidence\logs\production-template-parity-smoke-$stamp-result.json"
} else {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
}
$ResultPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ResultPath)
$RunRoot = Join-Path $ProjectRootPath "target\production-template-parity-smoke\$stamp"
$pythonScriptPath = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-production-template-parity-$PID-$([guid]::NewGuid().ToString('N')).py"

function Resolve-PythonExe {
    param([string]$ProjectPath)

    $venvPython = Join-Path $ProjectPath ".venv\Scripts\python.exe"
    if (Test-Path -LiteralPath $venvPython) {
        return (Resolve-Path $venvPython).Path
    }
    return "python"
}

function Get-EnabledTemplateTargetCount {
    param([string]$Path)

    $profile = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    $targets = @($profile.targets)
    return @(
        $targets | Where-Object {
            $_.path -and (
                -not ($_.PSObject.Properties.Name -contains "enabled") -or
                $_.enabled -ne $false
            )
        }
    ).Count
}

if (-not $ProfilePath) {
    $profilesDir = Join-Path $DataDir "profiles"
    if (-not (Test-Path -LiteralPath $profilesDir)) {
        throw "Profiles directory does not exist: $profilesDir"
    }
    $profiles = @(
        Get-ChildItem -LiteralPath $profilesDir -Filter "profile_*.json" |
            ForEach-Object {
                [pscustomobject][ordered]@{
                    Path = $_.FullName
                    EnabledTemplateTargets = Get-EnabledTemplateTargetCount $_.FullName
                }
            } |
            Where-Object { $_.EnabledTemplateTargets -gt 0 } |
            Sort-Object `
                @{ Expression = "EnabledTemplateTargets"; Descending = $true },
                @{ Expression = "Path"; Descending = $false }
    )
    if ($profiles.Count -eq 0) {
        throw "No profile_*.json under $profilesDir contains enabled template targets"
    }
    $ProfilePath = $profiles[0].Path
}
$ProfilePath = (Resolve-Path $ProfilePath).Path
$targetCount = Get-EnabledTemplateTargetCount $ProfilePath
$pythonExe = Resolve-PythonExe $PythonProjectPath
$pythonPathOld = $env:PYTHONPATH

$pythonSource = @'
import argparse
import json
import time
from pathlib import Path

import numpy as np
from PIL import Image

from screen_watch.core import Detector


def parse_frame_spec(value):
    left, right = value.lower().split("x", 1)
    width, height = int(left), int(right)
    if width <= 0 or height <= 0:
        raise ValueError("frame dimensions must be positive")
    return width, height


def synthetic_background(width, height):
    y = np.arange(height, dtype=np.uint32)[:, None]
    x = np.arange(width, dtype=np.uint32)[None, :]
    frame = np.empty((height, width, 3), dtype=np.uint8)
    frame[:, :, 0] = 3 + ((x * 17 + y * 11) % 29)
    frame[:, :, 1] = 5 + ((x * 13 + y * 19) % 31)
    frame[:, :, 2] = 7 + ((x * 23 + y * 7) % 37)
    return frame


def resolve_template_path(data_dir, value):
    path = Path(value)
    return path if path.is_absolute() else data_dir / path


def template_rgb(path):
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.uint8)


def sanitize(value):
    if isinstance(value, dict):
        return {str(k): sanitize(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [sanitize(v) for v in value]
    if isinstance(value, np.generic):
        return value.item()
    return value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", required=True)
    parser.add_argument("--data-dir", required=True)
    parser.add_argument("--frame", required=True)
    parser.add_argument("--threshold", type=float, required=True)
    parser.add_argument("--scales", required=True)
    parser.add_argument("--template-workers", type=int, required=True)
    parser.add_argument("--run-root", required=True)
    args = parser.parse_args()

    profile_path = Path(args.profile)
    data_dir = Path(args.data_dir)
    run_root = Path(args.run_root)
    run_root.mkdir(parents=True, exist_ok=True)

    width, height = parse_frame_spec(args.frame)
    profile = json.loads(profile_path.read_text(encoding="utf-8"))
    frame = synthetic_background(width, height)
    targets = []
    placements = []
    cursor_x = 32
    cursor_y = 32
    row_height = 0
    margin = 19

    for target in profile.get("targets", []):
        if not target.get("path"):
            continue
        if target.get("enabled", True) is False:
            continue
        name = str(target.get("name") or Path(str(target["path"])).stem)
        target_id = str(target.get("id") or name)
        template_path = resolve_template_path(data_dir, target["path"])
        template = template_rgb(template_path)
        th, tw = template.shape[:2]
        if tw + margin >= width or th + margin >= height:
            raise RuntimeError(f"template too large for benchmark frame: {template_path}")
        if cursor_x + tw + margin > width:
            cursor_x = 32
            cursor_y += row_height + margin
            row_height = 0
        if cursor_y + th + margin > height:
            raise RuntimeError("not enough benchmark frame space for all production templates")
        frame[cursor_y:cursor_y + th, cursor_x:cursor_x + tw, :] = template
        placements.append({
            "targetId": target_id,
            "name": name,
            "left": cursor_x,
            "top": cursor_y,
            "width": tw,
            "height": th,
        })
        targets.append({
            "kind": "template",
            "id": target_id,
            "name": name,
            "path": target["path"],
            "threshold": args.threshold,
            "scales": args.scales,
        })
        cursor_x += tw + margin
        row_height = max(row_height, th)

    if not targets:
        raise RuntimeError("profile has no enabled template targets")

    frame_path = run_root / "production-template-parity-frame.png"
    config_path = run_root / "production-template-parity-config.json"
    Image.fromarray(frame).save(frame_path)
    config = {
        "_base_dir": str(data_dir),
        "template_workers": args.template_workers,
        "targets": targets,
    }
    config_path.write_text(json.dumps(config, ensure_ascii=False, indent=2), encoding="utf-8")

    detector = Detector(config)
    started = time.perf_counter()
    matches = detector.run(frame)
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    result = {
        "status": "pass",
        "profilePath": str(profile_path),
        "dataDir": str(data_dir),
        "framePath": str(frame_path),
        "configPath": str(config_path),
        "frame": {"width": width, "height": height},
        "threshold": args.threshold,
        "scales": args.scales,
        "templateWorkers": args.template_workers,
        "rawTargetCount": len(profile.get("targets", [])),
        "expectedIds": sorted(item["id"] for item in targets),
        "placements": placements,
        "elapsedMs": elapsed_ms,
        "matches": sanitize(matches),
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
'@

try {
    New-Item -ItemType Directory -Force -Path $RunRoot | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $ResultPath) | Out-Null
    [IO.File]::WriteAllText($pythonScriptPath, $pythonSource, [Text.UTF8Encoding]::new($false))
    $env:PYTHONPATH = Join-Path $PythonProjectPath "src"

    Write-Host "productionTemplateParityProfile: $ProfilePath"
    Write-Host "productionTemplateParityDataDir: $DataDir"
    Write-Host "productionTemplateParityDataset: frame=$Frame enabledTemplateTargets=$targetCount threshold=$Threshold scales=$Scales workers=$TemplateWorkers"

    $pythonOutput = & $pythonExe $pythonScriptPath `
        --profile $ProfilePath `
        --data-dir $DataDir `
        --frame $Frame `
        --threshold ([string]$Threshold) `
        --scales $Scales `
        --template-workers ([string]$TemplateWorkers) `
        --run-root $RunRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Python production template parity failed with exit code $LASTEXITCODE"
    }
    $pythonJson = ($pythonOutput | Out-String)
    $pythonResult = $pythonJson | ConvertFrom-Json

    Push-Location $ProjectRootPath
    try {
        $cargoArgs = @("run", "--quiet")
        if (-not $RustDebug) {
            $cargoArgs += "--release"
        }
        $cargoArgs += @(
            "-p",
            "screen-watch-core",
            "--example",
            "detect-config-frame",
            "--",
            [string]$pythonResult.configPath,
            [string]$pythonResult.framePath,
            $DataDir
        )
        $rustOutput = & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Rust production template parity failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
    $rustJson = ($rustOutput | Out-String)
    $rustResult = $rustJson | ConvertFrom-Json

    $expectedIds = @($pythonResult.expectedIds | ForEach-Object { [string]$_ } | Sort-Object)
    $pythonIds = @($pythonResult.matches | ForEach-Object { [string]$_.target_id } | Sort-Object)
    $rustIds = @($rustResult.matches | ForEach-Object { [string]$_.targetId } | Sort-Object)
    $rustMissingExpected = @($expectedIds | Where-Object { $rustIds -notcontains $_ })
    $pythonHitMissingInRust = @($pythonIds | Where-Object { $rustIds -notcontains $_ })
    $rustExtraComparedToPython = @($rustIds | Where-Object { $pythonIds -notcontains $_ })

    $status = if ($rustMissingExpected.Count -eq 0 -and $pythonHitMissingInRust.Count -eq 0) {
        "pass"
    } else {
        "fail"
    }
    $tempPythonScriptRemoved = $false
    if (Test-Path -LiteralPath $pythonScriptPath) {
        Remove-Item -LiteralPath $pythonScriptPath -Force
        $tempPythonScriptRemoved = $true
    }

    $result = [ordered]@{
        status = $status
        timestamp = (Get-Date).ToString("o")
        machine = $env:COMPUTERNAME
        pythonProject = $PythonProjectPath
        pythonExe = $pythonExe
        profilePath = $ProfilePath
        dataDir = $DataDir
        runRoot = $RunRoot
        frame = $Frame
        threshold = $Threshold
        scales = $Scales
        templateWorkers = $TemplateWorkers
        expectedCount = $expectedIds.Count
        python = [ordered]@{
            elapsedMs = [int]$pythonResult.elapsedMs
            matchCount = $pythonIds.Count
            matchedIds = $pythonIds
        }
        rust = [ordered]@{
            elapsedMs = [int]$rustResult.elapsedMs
            matchCount = $rustIds.Count
            matchedIds = $rustIds
        }
        comparison = [ordered]@{
            rustMissingExpected = $rustMissingExpected
            pythonHitMissingInRust = $pythonHitMissingInRust
            rustExtraComparedToPython = $rustExtraComparedToPython
        }
        artifacts = [ordered]@{
            configPath = [string]$pythonResult.configPath
            framePath = [string]$pythonResult.framePath
            resultPath = $ResultPath
        }
        cleanup = [ordered]@{
            tempPythonScriptRemoved = $tempPythonScriptRemoved
            runRootRetained = $true
        }
    }
    $result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $ResultPath -Encoding UTF8

    Write-Host "pythonProductionTemplateParityMs: $($result.python.elapsedMs)"
    Write-Host "rustProductionTemplateParityMs: $($result.rust.elapsedMs)"
    Write-Host "expectedTemplateTargets: $($result.expectedCount)"
    Write-Host "pythonProductionTemplateMatches: $($result.python.matchCount)"
    Write-Host "rustProductionTemplateMatches: $($result.rust.matchCount)"
    Write-Host "rustExtraComparedToPython: $($rustExtraComparedToPython -join ',')"
    Write-Host "resultPath: $ResultPath"

    if ($status -ne "pass") {
        throw "Production template parity failed: $($result.comparison | ConvertTo-Json -Compress)"
    }
} finally {
    $env:PYTHONPATH = $pythonPathOld
    if (Test-Path -LiteralPath $pythonScriptPath) {
        Remove-Item -LiteralPath $pythonScriptPath -Force
    }
}
