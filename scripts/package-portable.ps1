param(
    [ValidateSet("lite", "full")]
    [string]$Flavor = "lite",
    [string]$ProjectRoot = "",
    [string]$OutputDir = "",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $ProjectRoot) {
    $ProjectRoot = Join-Path $ScriptRoot ".."
}
$ProjectRootPath = (Resolve-Path $ProjectRoot).Path
if (-not $OutputDir) {
    $OutputDir = Join-Path $ProjectRootPath "target\portable"
}
$OutputDirPath = $OutputDir
New-Item -ItemType Directory -Force -Path $OutputDirPath | Out-Null

function Get-ZipEntry {
    param(
        [System.IO.Compression.ZipArchive]$Archive,
        [string]$PackageBaseName,
        [string]$Name
    )
    $prefix = "$PackageBaseName/"
    $altPrefix = "$PackageBaseName\"
    $Archive.Entries |
        Where-Object {
            $_.FullName -eq "$prefix$Name" -or $_.FullName -eq "$altPrefix$Name"
        } |
        Select-Object -First 1
}

function Read-ZipTextEntry {
    param([System.IO.Compression.ZipArchiveEntry]$Entry)
    $stream = $Entry.Open()
    try {
        $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8)
        try {
            return $reader.ReadToEnd()
        } finally {
            $reader.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
}

function Get-BuildInfoPath {
    param([string]$ExePath)
    return [IO.Path]::ChangeExtension($ExePath, ".build-info.json")
}

function Get-ExeSha256 {
    param([string]$ExePath)
    $getFileHashCommand = Get-Command Get-FileHash -ErrorAction SilentlyContinue
    if ($getFileHashCommand) {
        return (Get-FileHash -Algorithm SHA256 -LiteralPath $ExePath).Hash.ToLowerInvariant()
    }

    $stream = [IO.File]::OpenRead($ExePath)
    try {
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try {
            $bytes = $sha256.ComputeHash($stream)
            return -join ($bytes | ForEach-Object { $_.ToString("x2") })
        } finally {
            $sha256.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
}

function Write-ReleaseBuildInfo {
    param(
        [string]$ExePath,
        [string]$Flavor
    )

    $exeItem = Get-Item -LiteralPath $ExePath
    $buildInfo = [ordered]@{
        executable = $exeItem.Name
        flavor = $Flavor
        executableBytes = $exeItem.Length
        executableSha256 = Get-ExeSha256 $exeItem.FullName
        builtUtc = (Get-Date).ToUniversalTime().ToString("o")
        buildFlavorEnv = "SCREENWATCH_BUILD_FLAVOR"
        compiledBuildFlavorEnv = "SCREENWATCH_COMPILED_BUILD_FLAVOR"
    }
    $buildInfo |
        ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath (Get-BuildInfoPath $exeItem.FullName) -Encoding UTF8
}

function Assert-ReleaseBuildInfo {
    param(
        [string]$ExePath,
        [string]$Flavor
    )

    $buildInfoPath = Get-BuildInfoPath $ExePath
    if (-not (Test-Path -LiteralPath $buildInfoPath)) {
        throw "release build info is missing at $buildInfoPath; rebuild without -SkipBuild before packaging"
    }

    $exeItem = Get-Item -LiteralPath $ExePath
    $buildInfo = Get-Content -LiteralPath $buildInfoPath -Raw | ConvertFrom-Json
    if ($buildInfo.flavor -ne $Flavor) {
        throw "release build info flavor '$($buildInfo.flavor)' does not match requested portable flavor '$Flavor'"
    }
    if ($buildInfo.executable -ne $exeItem.Name) {
        throw "release build info executable mismatch"
    }
    if ([int64]$buildInfo.executableBytes -ne $exeItem.Length) {
        throw "release build info executableBytes mismatch"
    }
    $actualHash = Get-ExeSha256 $exeItem.FullName
    if ([string]$buildInfo.executableSha256 -ne $actualHash) {
        throw "release build info hash does not match current executable"
    }
    return $buildInfo
}

function Assert-PortablePackage {
    param(
        [string]$ZipPath,
        [string]$PackageBaseName,
        [string]$Flavor,
        [int64]$ExpectedExeBytes,
        [string]$ExpectedExeSha256,
        [string[]]$RequiredModels
    )

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        foreach ($entry in $archive.Entries) {
            if (-not ($entry.FullName.StartsWith("$PackageBaseName/") -or $entry.FullName.StartsWith("$PackageBaseName\"))) {
                throw "portable package contains unexpected root entry: $($entry.FullName)"
            }
            $entryName = [IO.Path]::GetFileName(($entry.FullName -replace "\\", "/"))
            if ($RequiredModels -contains $entryName) {
                throw "portable package must not bundle required OCR assets: $($entry.FullName)"
            }
            if ($entry.FullName -match "\.onnx$") {
                throw "portable package must not bundle OCR model files: $($entry.FullName)"
            }
        }

        $exeEntry = Get-ZipEntry $archive $PackageBaseName "screen-watch-ocr-tauri.exe"
        $buildInfoEntry = Get-ZipEntry $archive $PackageBaseName "screen-watch-ocr-tauri.build-info.json"
        $manifestEntry = Get-ZipEntry $archive $PackageBaseName "portable-manifest.json"
        $readmeEntry = Get-ZipEntry $archive $PackageBaseName "README-portable.txt"
        if (-not $exeEntry) { throw "portable package is missing screen-watch-ocr-tauri.exe" }
        if (-not $buildInfoEntry) { throw "portable package is missing screen-watch-ocr-tauri.build-info.json" }
        if (-not $manifestEntry) { throw "portable package is missing portable-manifest.json" }
        if (-not $readmeEntry) { throw "portable package is missing README-portable.txt" }
        if ($exeEntry.Length -ne $ExpectedExeBytes) {
            throw "portable package exe bytes $($exeEntry.Length) did not match expected $ExpectedExeBytes"
        }
        if ($readmeEntry.Length -le 0) {
            throw "portable package README is empty"
        }

        $manifest = Read-ZipTextEntry $manifestEntry | ConvertFrom-Json
        if ($manifest.packageName -ne $PackageBaseName) {
            throw "portable manifest packageName mismatch"
        }
        if ($manifest.flavor -ne $Flavor) {
            throw "portable manifest flavor mismatch"
        }
        if ($manifest.executable -ne "screen-watch-ocr-tauri.exe") {
            throw "portable manifest executable mismatch"
        }
        if ([int64]$manifest.executableBytes -ne $ExpectedExeBytes) {
            throw "portable manifest executableBytes mismatch"
        }
        if ($manifest.executableSha256 -ne $ExpectedExeSha256) {
            throw "portable manifest executableSha256 mismatch"
        }
        if ($manifest.buildInfo -ne "screen-watch-ocr-tauri.build-info.json") {
            throw "portable manifest buildInfo mismatch"
        }
        if ($manifest.appDataDirectoryName -ne "ScreenWatchOCR") {
            throw "portable manifest changed the app-data directory contract"
        }
        if ($manifest.ocrModelsBundled -ne $false) {
            throw "portable manifest must report external OCR models"
        }
        if ($manifest.ocrModelDirEnv -ne "SCREENWATCH_OCR_MODEL_DIR") {
            throw "portable manifest OCR model env mismatch"
        }
        $actualModels = @($manifest.requiredOcrModels)
        if ($actualModels.Count -ne $RequiredModels.Count) {
            throw "portable manifest required OCR model count mismatch"
        }
        for ($i = 0; $i -lt $RequiredModels.Count; $i++) {
            if ($actualModels[$i] -ne $RequiredModels[$i]) {
                throw "portable manifest required OCR model mismatch at index $i"
            }
        }

        $buildInfo = Read-ZipTextEntry $buildInfoEntry | ConvertFrom-Json
        if ($buildInfo.flavor -ne $Flavor) {
            throw "portable build info flavor mismatch"
        }
        if ($buildInfo.executable -ne "screen-watch-ocr-tauri.exe") {
            throw "portable build info executable mismatch"
        }
        if ([int64]$buildInfo.executableBytes -ne $ExpectedExeBytes) {
            throw "portable build info executableBytes mismatch"
        }
        if ($buildInfo.executableSha256 -ne $ExpectedExeSha256) {
            throw "portable build info executableSha256 mismatch"
        }
    } finally {
        $archive.Dispose()
    }
}

$packageJson = Get-Content (Join-Path $ProjectRootPath "package.json") -Raw | ConvertFrom-Json
$exePath = Join-Path $ProjectRootPath "target\release\screen-watch-ocr-tauri.exe"

if (-not $SkipBuild) {
    Push-Location $ProjectRootPath
    $oldFlavor = $env:SCREENWATCH_BUILD_FLAVOR
    try {
        $env:SCREENWATCH_BUILD_FLAVOR = $Flavor
        $buildArgs = @("tauri", "build", "--no-bundle", "--ci")
        if ($Flavor -eq "full") {
            $buildArgs += @("--features", "ocr")
        }
        & npx @buildArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri app build failed with exit code $LASTEXITCODE"
        }
        Write-ReleaseBuildInfo -ExePath $exePath -Flavor $Flavor
    } finally {
        $env:SCREENWATCH_BUILD_FLAVOR = $oldFlavor
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $exePath)) {
    throw "release executable was not found at $exePath"
}
$buildInfo = Assert-ReleaseBuildInfo -ExePath $exePath -Flavor $Flavor

$timestamp = (Get-Date).ToUniversalTime().ToString("yyyyMMdd-HHmmss")
$suffix = ([guid]::NewGuid().ToString("N")).Substring(0, 8)
$packageBaseName = "screen-watch-ocr-tauri-$Flavor-portable-$timestamp-$suffix"
$zipPath = Join-Path $OutputDirPath "$packageBaseName.zip"
$stagingRoot = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-tauri-package-$suffix"

New-Item -ItemType Directory -Path $stagingRoot | Out-Null
try {
    $packageDir = Join-Path $stagingRoot $packageBaseName
    New-Item -ItemType Directory -Path $packageDir | Out-Null

    $exeItem = Get-Item -LiteralPath $exePath
    Copy-Item -LiteralPath $exePath -Destination (Join-Path $packageDir "screen-watch-ocr-tauri.exe")
    Copy-Item -LiteralPath (Get-BuildInfoPath $exePath) -Destination (Join-Path $packageDir "screen-watch-ocr-tauri.build-info.json")

    $requiredModels = @(
        "det.onnx",
        "rec.onnx",
        "ppocrv5_dict.txt"
    )
    $manifest = [ordered]@{
        packageName = $packageBaseName
        appName = "Screen Watch OCR Tauri"
        version = $packageJson.version
        flavor = $Flavor
        executable = "screen-watch-ocr-tauri.exe"
        executableBytes = $exeItem.Length
        executableSha256 = $buildInfo.executableSha256
        buildInfo = "screen-watch-ocr-tauri.build-info.json"
        createdUtc = (Get-Date).ToUniversalTime().ToString("o")
        appDataDirectoryName = "ScreenWatchOCR"
        ocrModelsBundled = $false
        ocrModelDirEnv = "SCREENWATCH_OCR_MODEL_DIR"
        defaultWindowsOcrModelDir = "%LOCALAPPDATA%\ScreenWatchOCR\models\rapidocr"
        requiredOcrModels = $requiredModels
    }
    $manifest |
        ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath (Join-Path $packageDir "portable-manifest.json") -Encoding UTF8

    $readme = @"
Screen Watch OCR Tauri portable package

Flavor: $Flavor
Version: $($packageJson.version)

Run:
  screen-watch-ocr-tauri.exe

Compatibility:
  User data directory name remains ScreenWatchOCR.
  Existing Python-compatible profile/config JSON shapes are preserved.

OCR:
  OCR models are not bundled in this portable package.
  Default Windows model directory:
    %LOCALAPPDATA%\ScreenWatchOCR\models\rapidocr
  Override with:
    SCREENWATCH_OCR_MODEL_DIR

Required native OCR asset filenames:
  det.onnx
  rec.onnx
  ppocrv5_dict.txt

Notes:
  This package avoids NSIS/WiX installer downloads and is intended for portable
  smoke testing and size verification.
"@
    Set-Content -LiteralPath (Join-Path $packageDir "README-portable.txt") -Value $readme -Encoding UTF8

    Compress-Archive -LiteralPath $packageDir -DestinationPath $zipPath -CompressionLevel Optimal
} finally {
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force
    }
}

$zipItem = Get-Item -LiteralPath $zipPath
Assert-PortablePackage `
    -ZipPath $zipItem.FullName `
    -PackageBaseName $packageBaseName `
    -Flavor $Flavor `
    -ExpectedExeBytes $exeItem.Length `
    -ExpectedExeSha256 $buildInfo.executableSha256 `
    -RequiredModels $requiredModels
Write-Host "packagePath: $($zipItem.FullName)"
Write-Host "packageBytes: $($zipItem.Length)"
Write-Host "packageVerified: True"
