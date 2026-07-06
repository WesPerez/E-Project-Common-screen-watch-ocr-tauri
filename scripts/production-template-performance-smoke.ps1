param(
    [string]$DataDir = "",
    [string]$ProfilePath = "",
    [string]$Frame = "2560x1440",
    [string]$Scales = "1.0",
    [double]$Threshold = 0.90,
    [int]$TemplateWorkers = 2,
    [switch]$RustDebug,
    [switch]$SkipParity
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $DataDir) {
    $DataDir = Join-Path $env:LOCALAPPDATA "ScreenWatchOCR"
}
$DataDir = (Resolve-Path $DataDir).Path

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

Write-Host "productionTemplateProfile: $ProfilePath"
Write-Host "productionTemplateDataDir: $DataDir"
Write-Host "productionTemplateDataset: frame=$Frame enabledTemplateTargets=$targetCount threshold=$Threshold scales=$Scales workers=$TemplateWorkers"

$oldProfile = $env:SCREENWATCH_PRODUCTION_PROFILE
$oldDataDir = $env:SCREENWATCH_PRODUCTION_DATA_DIR
$oldFrame = $env:SCREENWATCH_PRODUCTION_FRAME
$oldScales = $env:SCREENWATCH_PRODUCTION_SCALES
$oldThreshold = $env:SCREENWATCH_PRODUCTION_THRESHOLD
$oldWorkers = $env:SCREENWATCH_PRODUCTION_TEMPLATE_WORKERS

try {
    if (-not $SkipParity) {
        $parityArgs = @(
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            (Join-Path $ScriptRoot "template-parity-benchmark.ps1")
        )
        if ($RustDebug) {
            $parityArgs += "-RustDebug"
        }
        powershell @parityArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Template parity benchmark failed with exit code $LASTEXITCODE"
        }
    }

    $env:SCREENWATCH_PRODUCTION_PROFILE = $ProfilePath
    $env:SCREENWATCH_PRODUCTION_DATA_DIR = $DataDir
    $env:SCREENWATCH_PRODUCTION_FRAME = $Frame
    $env:SCREENWATCH_PRODUCTION_SCALES = $Scales
    $env:SCREENWATCH_PRODUCTION_THRESHOLD = [string]$Threshold
    $env:SCREENWATCH_PRODUCTION_TEMPLATE_WORKERS = [string]$TemplateWorkers

    Push-Location $ProjectRootPath
    try {
        $cargoArgs = @("test")
        if (-not $RustDebug) {
            $cargoArgs += "--release"
        }
        $cargoArgs += @(
            "-p",
            "screen-watch-core",
            "benchmark_production_profile_template_scan",
            "--",
            "--ignored",
            "--nocapture"
        )
        cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Production profile template benchmark failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
} finally {
    if ($null -eq $oldProfile) {
        Remove-Item Env:\SCREENWATCH_PRODUCTION_PROFILE -ErrorAction SilentlyContinue
    } else {
        $env:SCREENWATCH_PRODUCTION_PROFILE = $oldProfile
    }
    if ($null -eq $oldDataDir) {
        Remove-Item Env:\SCREENWATCH_PRODUCTION_DATA_DIR -ErrorAction SilentlyContinue
    } else {
        $env:SCREENWATCH_PRODUCTION_DATA_DIR = $oldDataDir
    }
    if ($null -eq $oldFrame) {
        Remove-Item Env:\SCREENWATCH_PRODUCTION_FRAME -ErrorAction SilentlyContinue
    } else {
        $env:SCREENWATCH_PRODUCTION_FRAME = $oldFrame
    }
    if ($null -eq $oldScales) {
        Remove-Item Env:\SCREENWATCH_PRODUCTION_SCALES -ErrorAction SilentlyContinue
    } else {
        $env:SCREENWATCH_PRODUCTION_SCALES = $oldScales
    }
    if ($null -eq $oldThreshold) {
        Remove-Item Env:\SCREENWATCH_PRODUCTION_THRESHOLD -ErrorAction SilentlyContinue
    } else {
        $env:SCREENWATCH_PRODUCTION_THRESHOLD = $oldThreshold
    }
    if ($null -eq $oldWorkers) {
        Remove-Item Env:\SCREENWATCH_PRODUCTION_TEMPLATE_WORKERS -ErrorAction SilentlyContinue
    } else {
        $env:SCREENWATCH_PRODUCTION_TEMPLATE_WORKERS = $oldWorkers
    }
}
