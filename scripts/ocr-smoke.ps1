param(
    [string]$ModelDir = "",
    [string]$Image = "",
    [string]$Expect = "",
    [switch]$SelfTestMissingModels
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
$RequiredOcrModels = @(
    "det.onnx",
    "rec.onnx",
    "ppocrv5_dict.txt"
)

function Invoke-CheckedStep {
    param(
        [string]$Name,
        [scriptblock]$Script
    )

    Write-Host ""
    Write-Host "==> $Name"
    Push-Location $ProjectRootPath
    $oldErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & $Script
        if ($LASTEXITCODE -ne 0) {
            throw "$Name failed with exit code $LASTEXITCODE"
        }
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
        Pop-Location
    }
}

function Get-AbsolutePath {
    param([string]$Path)

    if ([IO.Path]::IsPathRooted($Path)) {
        return [IO.Path]::GetFullPath($Path)
    }
    return [IO.Path]::GetFullPath((Join-Path (Get-Location) $Path))
}

function Get-DefaultOcrModelDir {
    if ($env:LOCALAPPDATA) {
        return (Join-Path $env:LOCALAPPDATA "ScreenWatchOCR\models\rapidocr")
    }
    if ($env:XDG_DATA_HOME) {
        return (Join-Path $env:XDG_DATA_HOME "ScreenWatchOCR/models/rapidocr")
    }
    return (Join-Path $HOME ".local/share/ScreenWatchOCR/models/rapidocr")
}

function Get-EffectiveOcrModelDir {
    if ($ModelDir) {
        return (Get-AbsolutePath $ModelDir)
    }
    if ($env:SCREENWATCH_OCR_MODEL_DIR) {
        return (Get-AbsolutePath $env:SCREENWATCH_OCR_MODEL_DIR)
    }
    return (Get-DefaultOcrModelDir)
}

function Assert-OcrModelAssets {
    param([string]$Directory)

    Write-Host "modelDir: $Directory"
    Write-Host "requiredModels: $($RequiredOcrModels -join ', ')"

    $missing = @()
    foreach ($name in $RequiredOcrModels) {
        $path = Join-Path $Directory $name
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            $item = Get-Item -LiteralPath $path
            Write-Host "modelReady: $name ($($item.Length) bytes)"
        } else {
            Write-Host "modelMissing: $name ($path)"
            $missing += $name
        }
    }

    if ($missing.Count -gt 0) {
        throw (
            "Missing external OCR model asset(s): $($missing -join ', '). " +
            "Place det.onnx, rec.onnx, and ppocrv5_dict.txt under '$Directory', " +
            "or pass -ModelDir `"D:\Models\rapidocr`", or set SCREENWATCH_OCR_MODEL_DIR."
        )
    }
}

function Invoke-MissingModelSelfTest {
    $missingModelDir = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-ocr-smoke-missing-$([Guid]::NewGuid().ToString('N'))"
    try {
        Assert-OcrModelAssets $missingModelDir
        throw "OCR smoke missing-model self-test unexpectedly passed"
    } catch {
        $message = $_.Exception.Message
        if (-not $message.Contains("Missing external OCR model asset(s):")) {
            throw
        }
        foreach ($name in $RequiredOcrModels) {
            if (-not $message.Contains($name)) {
                throw "OCR smoke missing-model self-test did not report $name"
            }
        }
        Write-Host ""
        Write-Host "missingModelSelfTest: passed"
    }
}

if ($SelfTestMissingModels) {
    Invoke-MissingModelSelfTest
    exit 0
}

$oldModelDir = $env:SCREENWATCH_OCR_MODEL_DIR
$oldSmokeImage = $env:SCREENWATCH_OCR_SMOKE_IMAGE
$oldSmokeExpect = $env:SCREENWATCH_OCR_SMOKE_EXPECT

try {
    $effectiveModelDir = Get-EffectiveOcrModelDir
    Assert-OcrModelAssets $effectiveModelDir
    $env:SCREENWATCH_OCR_MODEL_DIR = $effectiveModelDir

    Invoke-CheckedStep `
        -Name "OCR real model probe" `
        -Script {
            cargo test -p screen-watch-core --features ocr native_ocr_real_model_probe_initializes_from_external_assets -- --ignored
        }

    if ($Image -or $Expect) {
        if (-not $Image -or -not $Expect) {
            throw "Pass both -Image and -Expect to run the OCR recognition smoke gate"
        }
        $imagePath = Get-AbsolutePath $Image
        if (-not (Test-Path -LiteralPath $imagePath -PathType Leaf)) {
            throw "OCR smoke image does not exist: $imagePath"
        }
        $env:SCREENWATCH_OCR_SMOKE_IMAGE = $imagePath
        $env:SCREENWATCH_OCR_SMOKE_EXPECT = $Expect

        Invoke-CheckedStep `
            -Name "OCR real model recognition smoke" `
            -Script {
                cargo test -p screen-watch-core --features ocr native_ocr_real_model_recognizes_smoke_png -- --ignored
            }
    } else {
        Write-Host ""
        Write-Host "recognitionSmoke: skipped (pass -Image and -Expect to enable it)"
    }
} finally {
    $env:SCREENWATCH_OCR_MODEL_DIR = $oldModelDir
    $env:SCREENWATCH_OCR_SMOKE_IMAGE = $oldSmokeImage
    $env:SCREENWATCH_OCR_SMOKE_EXPECT = $oldSmokeExpect
}
