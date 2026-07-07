param(
    [string]$PythonProject = "",
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $PythonProject) {
    $PythonProject = (Resolve-Path (Join-Path $ProjectRootPath "..\screen-watch-ocr")).Path
} else {
    $PythonProject = (Resolve-Path $PythonProject).Path
}
if (-not $OutputDir) {
    $OutputDir = Join-Path $ProjectRootPath "docs\manual-gate-evidence\logs"
}

function Get-TimeStamp {
    return (Get-Date).ToString("yyyyMMdd-HHmmss")
}

function Invoke-CargoStep {
    param(
        [string]$Name,
        [string[]]$Arguments
    )

    Write-Host ""
    Write-Host "==> $Name"
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = "cargo"
    $startInfo.WorkingDirectory = $ProjectRootPath
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.UseShellExecute = $false
    $startInfo.StandardOutputEncoding = [Text.Encoding]::UTF8
    $startInfo.StandardErrorEncoding = [Text.Encoding]::UTF8
    $startInfo.Arguments = $Arguments -join " "
    $process = [Diagnostics.Process]::Start($startInfo)
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    $exitCode = $process.ExitCode
    $output = (@($stdout, $stderr) | Where-Object { $_ }) -join "`n"

    if ($exitCode -ne 0) {
        throw "$Name failed with exit code $exitCode`n$output"
    }

    return [pscustomobject][ordered]@{
        name = $Name
        command = "cargo $($Arguments -join ' ')"
        exitCode = $exitCode
        output = $output.Trim()
    }
}

function Invoke-PythonOcrParity {
    $srcDir = Join-Path $PythonProject "src"
    if (-not (Test-Path -LiteralPath (Join-Path $srcDir "screen_watch\core.py") -PathType Leaf)) {
        throw "Python baseline core.py not found under $srcDir"
    }

    $code = @'
import json
from screen_watch import core

detector = object.__new__(core.Detector)

cases = [
    {
        "name": "case_insensitive_score_and_box",
        "rows": [
            ("quiet", 0.99, [[1.0, 1.0], [3.0, 1.0]]),
            ("ALERT-42", 0.8, [[10.2, 20.8], [30.9, 20.1], [30.4, 40.9], [10.0, 40.0]]),
        ],
        "target": {"kind": "ocr_text", "id": "ocr-id", "name": "alert-text", "text": "alert", "min_score": 0.5},
        "expect": {"hit": True, "target_id": "ocr-id", "text": "ALERT-42", "box": [10, 20, 30, 40]},
    },
    {
        "name": "min_score_miss",
        "rows": [("ALERT", 0.7, None)],
        "target": {"kind": "ocr_text", "name": "too-strict", "text": "ALERT", "min_score": 0.8},
        "expect": {"hit": False},
    },
    {
        "name": "case_sensitive_miss",
        "rows": [("ALERT", 0.7, None)],
        "target": {"kind": "ocr_text", "name": "case-miss", "text": "alert", "min_score": 0.1, "case_sensitive": True},
        "expect": {"hit": False},
    },
    {
        "name": "case_sensitive_hit_without_box",
        "rows": [("ALERT", 0.7, None)],
        "target": {"kind": "ocr_text", "name": "case-hit", "text": "ALERT", "min_score": 0.1, "case_sensitive": True},
        "expect": {"hit": True, "target_id": "case-hit", "text": "ALERT", "box": None},
    },
    {
        "name": "unicode_contains",
        "rows": [("\u51c6\u5907\u597d\u4e86", 0.88, [[5.0, 6.0], [45.0, 6.0], [45.0, 26.0], [5.0, 26.0]])],
        "target": {"kind": "ocr_text", "id": "zh-id", "name": "zh-ready", "text": "\u51c6\u5907", "min_score": 0.5},
        "expect": {"hit": True, "target_id": "zh-id", "text": "\u51c6\u5907\u597d\u4e86", "box": [5, 6, 45, 26]},
    },
]

results = []
for case in cases:
    hit = core.Detector._ocr(detector, case["rows"], case["target"])
    expected = case["expect"]
    if expected["hit"]:
        if not hit:
            raise AssertionError(f"{case['name']} expected a hit")
        for key in ("target_id", "text", "box"):
            if hit.get(key) != expected[key]:
                raise AssertionError(f"{case['name']} expected {key}={expected[key]!r}, got {hit.get(key)!r}")
    elif hit is not None:
        raise AssertionError(f"{case['name']} expected no hit, got {hit!r}")
    results.append({"name": case["name"], "hit": hit})

print(json.dumps({"status": "pass", "cases": results}, ensure_ascii=True, indent=2, sort_keys=True))
'@

    $tempScript = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-python-ocr-parity-$([Guid]::NewGuid().ToString('N')).py"
    $oldPythonPath = $env:PYTHONPATH
    $oldPythonIoEncoding = $env:PYTHONIOENCODING
    try {
        Set-Content -LiteralPath $tempScript -Value $code -Encoding UTF8
        $env:PYTHONPATH = $srcDir
        $env:PYTHONIOENCODING = "utf-8"

        $startInfo = [Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = "python"
        $startInfo.WorkingDirectory = $ProjectRootPath
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $startInfo.UseShellExecute = $false
        $startInfo.StandardOutputEncoding = [Text.Encoding]::UTF8
        $startInfo.StandardErrorEncoding = [Text.Encoding]::UTF8
        $startInfo.Arguments = "`"$tempScript`""
        $process = [Diagnostics.Process]::Start($startInfo)
        $stdout = $process.StandardOutput.ReadToEnd()
        $stderr = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        $exitCode = $process.ExitCode
        $output = (@($stdout, $stderr) | Where-Object { $_ }) -join "`n"
    } finally {
        $env:PYTHONPATH = $oldPythonPath
        $env:PYTHONIOENCODING = $oldPythonIoEncoding
        if (Test-Path -LiteralPath $tempScript) {
            Remove-Item -LiteralPath $tempScript -Force
        }
    }

    if ($exitCode -ne 0) {
        throw "Python OCR parity cases failed with exit code $exitCode`n$output"
    }

    return [pscustomobject][ordered]@{
        command = "PYTHONPATH=$srcDir PYTHONIOENCODING=utf-8 python <temp-script>"
        exitCode = $exitCode
        result = ($output | ConvertFrom-Json)
    }
}

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$stamp = Get-TimeStamp
$resultPath = Join-Path $OutputDir "ocr-text-parity-smoke-$stamp-result.json"

$pythonResult = Invoke-PythonOcrParity
$rustSteps = @()
$rustSteps += Invoke-CargoStep `
    -Name "Rust OCR text detection tests" `
    -Arguments @("test", "-p", "screen-watch-core", "ocr_text_detection")
$rustSteps += Invoke-CargoStep `
    -Name "Rust OCR ScanEngine backend test" `
    -Arguments @("test", "-p", "screen-watch-core", "scan_with_ocr_backend_matches_text_and_writes_evidence")

$result = [pscustomobject][ordered]@{
    stamp = $stamp
    status = "pass"
    pythonProject = $PythonProject
    pythonSource = (Join-Path $PythonProject "src\screen_watch\core.py")
    python = $pythonResult
    rust = $rustSteps
    notes = @(
        "This smoke compares OCR text matching semantics using supplied OCR rows, not real OCR model recognition quality.",
        "It verifies Python baseline contains/min_score/case_sensitive/box behavior and Rust detector/ScanEngine OCR-row behavior.",
        "Real Chinese OCR recognition is covered by the separate real-model OCR smoke when external Chinese PP-OCRv5 assets and a Chinese PNG are supplied."
    )
}

$result | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $resultPath -Encoding UTF8
Write-Host ""
Write-Host "ocrTextParitySmoke: passed"
Write-Host "resultPath: $resultPath"
