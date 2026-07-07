param(
    [string]$EnglishModelDir = "",
    [string]$ChineseModelDir = "",
    [string]$OutputDir = "",
    [string]$ImageDir = ""
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $EnglishModelDir) {
    $EnglishModelDir = Join-Path $ProjectRootPath "target\ocr-model-smoke\monkt-ppocrv5-english"
}
if (-not $ChineseModelDir) {
    $ChineseModelDir = Join-Path $ProjectRootPath "target\ocr-model-smoke\monkt-ppocrv5-chinese"
}
if (-not $OutputDir) {
    $OutputDir = Join-Path $ProjectRootPath "docs\manual-gate-evidence\logs"
}
if (-not $ImageDir) {
    $ImageDir = Join-Path $ProjectRootPath "target\ocr-corpus-smoke"
}

function Get-TimeStamp {
    return (Get-Date).ToString("yyyyMMdd-HHmmss")
}

function Get-AbsolutePath {
    param([string]$Path)

    if ([IO.Path]::IsPathRooted($Path)) {
        return [IO.Path]::GetFullPath($Path)
    }
    return [IO.Path]::GetFullPath((Join-Path (Get-Location) $Path))
}

function New-TextPng {
    param(
        [string]$Text,
        [string]$Path,
        [int]$Width = 520,
        [int]$Height = 150,
        [float]$FontSize = 50
    )

    Add-Type -AssemblyName System.Drawing
    $bitmap = [System.Drawing.Bitmap]::new($Width, $Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $format = [System.Drawing.StringFormat]::new()
    $brush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::Black)
    $font = [System.Drawing.Font]::new(
        "Microsoft YaHei UI",
        $FontSize,
        [System.Drawing.FontStyle]::Bold,
        [System.Drawing.GraphicsUnit]::Pixel
    )
    try {
        $graphics.Clear([System.Drawing.Color]::White)
        $graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
        $format.Alignment = [System.Drawing.StringAlignment]::Center
        $format.LineAlignment = [System.Drawing.StringAlignment]::Center
        $rect = [System.Drawing.RectangleF]::new(0, 0, $Width, $Height)
        $graphics.DrawString($Text, $font, $brush, $rect, $format)
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $font.Dispose()
        $brush.Dispose()
        $format.Dispose()
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Invoke-OcrSmokeCase {
    param(
        [string]$Name,
        [string]$ModelDir,
        [string]$Image,
        [string]$Expect,
        [string]$LogPath
    )

    $arguments = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        (Join-Path $ScriptRoot "ocr-smoke.ps1"),
        "-ModelDir",
        $ModelDir,
        "-Image",
        $Image,
        "-Expect",
        $Expect
    )
    Push-Location $ProjectRootPath
    $oldErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $output = & powershell @arguments 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
        Pop-Location
    }
    Set-Content -LiteralPath $LogPath -Value (($output | ForEach-Object { [string]$_ }) -join "`n") -Encoding UTF8
    if ($exitCode -ne 0) {
        throw "OCR corpus case '$Name' failed with exit code $exitCode. See $LogPath"
    }

    return [pscustomobject][ordered]@{
        name = $Name
        modelDir = $ModelDir
        image = $Image
        expect = $Expect
        command = "powershell -ExecutionPolicy Bypass -File scripts\ocr-smoke.ps1 -ModelDir `"$ModelDir`" -Image `"$Image`" -Expect `"$Expect`""
        exitCode = $exitCode
        logPath = $LogPath
    }
}

$stamp = Get-TimeStamp
$EnglishModelDir = Get-AbsolutePath $EnglishModelDir
$ChineseModelDir = Get-AbsolutePath $ChineseModelDir
$OutputDir = Get-AbsolutePath $OutputDir
$ImageDir = Get-AbsolutePath $ImageDir

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
New-Item -ItemType Directory -Force -Path $ImageDir | Out-Null

$cases = @(
    [pscustomobject][ordered]@{
        Name = "english-ready"
        Text = "READY"
        FileName = "english-ready.png"
        Expect = "READY"
        ModelDir = $EnglishModelDir
        Width = 420
        Height = 140
        FontSize = 48
    },
    [pscustomobject][ordered]@{
        Name = "english-alert-number"
        Text = "ALERT 42"
        FileName = "english-alert-number.png"
        Expect = "ALERT"
        ModelDir = $EnglishModelDir
        Width = 520
        Height = 150
        FontSize = 50
    },
    [pscustomobject][ordered]@{
        Name = "english-ocr-test"
        Text = "OCR TEST"
        FileName = "english-ocr-test.png"
        Expect = "OCR"
        ModelDir = $EnglishModelDir
        Width = 520
        Height = 150
        FontSize = 50
    },
    [pscustomobject][ordered]@{
        Name = "chinese-ready"
        Text = ([char[]]@(0x51C6, 0x5907, 0x597D, 0x4E86) -join "")
        FileName = "chinese-ready.png"
        Expect = ([char[]]@(0x51C6, 0x5907) -join "")
        ModelDir = $ChineseModelDir
        Width = 520
        Height = 150
        FontSize = 50
    },
    [pscustomobject][ordered]@{
        Name = "chinese-monitor"
        Text = ([char[]]@(0x5F00, 0x59CB, 0x76D1, 0x63A7) -join "")
        FileName = "chinese-monitor.png"
        Expect = ([char[]]@(0x76D1, 0x63A7) -join "")
        ModelDir = $ChineseModelDir
        Width = 520
        Height = 150
        FontSize = 50
    }
)

$results = @()
foreach ($case in $cases) {
    $imagePath = Join-Path $ImageDir "$stamp-$($case.FileName)"
    $logPath = Join-Path $OutputDir "ocr-corpus-smoke-$stamp-$($case.Name).log"
    New-TextPng `
        -Text $case.Text `
        -Path $imagePath `
        -Width $case.Width `
        -Height $case.Height `
        -FontSize $case.FontSize
    Write-Host "ocrCorpusCase: $($case.Name) text='$($case.Text)' expect='$($case.Expect)'"
    $results += Invoke-OcrSmokeCase `
        -Name $case.Name `
        -ModelDir $case.ModelDir `
        -Image $imagePath `
        -Expect $case.Expect `
        -LogPath $logPath
}

$resultPath = Join-Path $OutputDir "ocr-corpus-smoke-$stamp-result.json"
$result = [pscustomobject][ordered]@{
    stamp = $stamp
    status = "pass"
    imageDir = $ImageDir
    outputDir = $OutputDir
    englishModelDir = $EnglishModelDir
    chineseModelDir = $ChineseModelDir
    cases = $results
    notes = @(
        "This smoke expands real-model OCR coverage beyond a single English and Chinese PNG.",
        "It uses generated local PNGs and external PP-OCRv5-style ONNX assets; it does not bundle OCR models into the lite exe.",
        "It is still not broad production OCR quality validation or PP-OCRv6/RapidOCR-native compatibility."
    )
}
$result | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $resultPath -Encoding UTF8

Write-Host ""
Write-Host "ocrCorpusSmoke: passed"
Write-Host "resultPath: $resultPath"
