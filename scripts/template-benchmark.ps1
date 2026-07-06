param(
    [int]$MaxMs = 0,
    [switch]$RustDebug
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
$oldMaxMs = $env:SCREENWATCH_TEMPLATE_BENCH_MAX_MS

try {
    if ($MaxMs -gt 0) {
        $env:SCREENWATCH_TEMPLATE_BENCH_MAX_MS = [string]$MaxMs
    } else {
        Remove-Item Env:\SCREENWATCH_TEMPLATE_BENCH_MAX_MS -ErrorAction SilentlyContinue
    }

    Push-Location $ProjectRootPath
    try {
        $filters = @(
            "benchmark_large_frame_many_template_scan",
            "benchmark_large_frame_textured_template_scan"
        )
        foreach ($filter in $filters) {
            $cargoArgs = @("test")
            if (-not $RustDebug) {
                $cargoArgs += "--release"
            }
            $cargoArgs += @(
                "-p",
                "screen-watch-core",
                $filter,
                "--",
                "--ignored",
                "--nocapture"
            )

            cargo @cargoArgs
            if ($LASTEXITCODE -ne 0) {
                throw "Template benchmark '$filter' failed with exit code $LASTEXITCODE"
            }
        }
    } finally {
        Pop-Location
    }
} finally {
    if ($null -eq $oldMaxMs) {
        Remove-Item Env:\SCREENWATCH_TEMPLATE_BENCH_MAX_MS -ErrorAction SilentlyContinue
    } else {
        $env:SCREENWATCH_TEMPLATE_BENCH_MAX_MS = $oldMaxMs
    }
}
