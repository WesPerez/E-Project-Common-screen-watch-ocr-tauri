param(
    [string]$PythonProject = "",
    [int]$MinimumPythonTests = 98,
    [int]$MinimumRustCoreTests = 120,
    [int]$MinimumTauriTests = 82,
    [int]$MinimumOcrFeatureTests = 23,
    [int]$MinimumFrontendTests = 103,
    [long]$MaxTauriLiteExeBytes = 15728640,
    [double]$MaxTauriToPythonExeRatio = 0.25,
    [switch]$SkipPython,
    [switch]$SkipFrontend,
    [switch]$SkipRelease,
    [switch]$IncludeDesktopSmoke,
    [switch]$IncludePortablePackage,
    [switch]$IncludeFullPortablePackage,
    [switch]$IncludeOcrSmoke,
    [string]$OcrModelDir = "",
    [string]$OcrSmokeImage = "",
    [string]$OcrSmokeExpect = "",
    [switch]$IncludeTemplateBenchmark,
    [int]$TemplateBenchmarkMaxMs = 0,
    [switch]$IncludePackagedSmoke,
    [int]$PackagedSmokeStartupWaitSeconds = 18
)

$ErrorActionPreference = "Stop"
$PSDefaultParameterValues["Get-Content:Encoding"] = "UTF8"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $PythonProject) {
    $PythonProject = Join-Path $ProjectRootPath "..\screen-watch-ocr"
}
$PythonProjectPath = (Resolve-Path $PythonProject).Path

function Invoke-CheckedStep {
    param(
        [string]$Name,
        [string]$WorkingDirectory,
        [scriptblock]$Script
    )

    Write-Host ""
    Write-Host "==> $Name"
    Push-Location $WorkingDirectory
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

function Invoke-CapturedStep {
    param(
        [string]$Name,
        [string]$WorkingDirectory,
        [scriptblock]$Script,
        [switch]$SuppressOutput
    )

    Write-Host ""
    Write-Host "==> $Name"
    Push-Location $WorkingDirectory
    $oldErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $output = & $Script 2>&1
        $exitCode = $LASTEXITCODE
        if (-not $SuppressOutput -or $exitCode -ne 0) {
            $output | ForEach-Object { Write-Host $_ }
        }
        if ($exitCode -ne 0) {
            throw "$Name failed with exit code $exitCode"
        }
        return ($output | Out-String)
    } finally {
        $ErrorActionPreference = $oldErrorActionPreference
        Pop-Location
    }
}

function Assert-MinimumCount {
    param(
        [string]$Name,
        [int]$Actual,
        [int]$Minimum
    )

    if ($Actual -lt $Minimum) {
        throw "$Name count $Actual is below required baseline $Minimum"
    }
}

function Assert-OutputContainsNames {
    param(
        [string]$Name,
        [string]$Output,
        [string[]]$RequiredNames
    )

    $missing = @($RequiredNames | Where-Object { -not $Output.Contains($_) })
    if ($missing.Count -gt 0) {
        throw "$Name output is missing required gate test names: $($missing -join ', ')"
    }
}

function Get-RequiredBaselineNames {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        throw "Missing Python baseline test snapshot at $Path"
    }

    return @(
        Get-Content -Path $Path |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ -and -not $_.StartsWith("#") }
    )
}

function Assert-RequiredBaselineNames {
    param(
        [string[]]$ActualNames,
        [string[]]$RequiredNames
    )

    if ($RequiredNames.Count -lt $MinimumPythonTests) {
        throw "Python baseline snapshot only contains $($RequiredNames.Count) tests, below required baseline $MinimumPythonTests"
    }

    $missing = @($RequiredNames | Where-Object { $ActualNames -cnotcontains $_ })
    if ($missing.Count -gt 0) {
        throw "Python baseline inventory is missing locked acceptance tests: $($missing -join ', ')"
    }
}

function Assert-LiteExeSize {
    param(
        [long]$TauriBytes,
        [object]$PythonBytes,
        [long]$MaxBytes,
        [double]$MaxRatio
    )

    if ($MaxBytes -gt 0 -and $TauriBytes -gt $MaxBytes) {
        throw "Tauri lite exe size $TauriBytes bytes exceeds guardrail $MaxBytes bytes"
    }

    if ($null -ne $PythonBytes -and $PythonBytes -gt 0 -and $MaxRatio -gt 0) {
        $ratio = [double]$TauriBytes / [double]$PythonBytes
        if ($ratio -gt $MaxRatio) {
            $percent = "{0:N2}" -f ($ratio * 100)
            $limitPercent = "{0:N2}" -f ($MaxRatio * 100)
            throw "Tauri lite exe is $percent% of Python exe, above guardrail $limitPercent%"
        }
    }
}

function Get-BuildInfoPath {
    param([string]$ExePath)
    return [IO.Path]::ChangeExtension($ExePath, ".build-info.json")
}

function Get-ExeSha256 {
    param([string]$ExePath)

    if (Get-Command Get-FileHash -ErrorAction SilentlyContinue) {
        return (Get-FileHash -Algorithm SHA256 -LiteralPath $ExePath).Hash.ToLowerInvariant()
    }

    $stream = [IO.File]::OpenRead($ExePath)
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
        $hashBytes = $sha256.ComputeHash($stream)
        return ([BitConverter]::ToString($hashBytes) -replace "-", "").ToLowerInvariant()
    } finally {
        $sha256.Dispose()
        $stream.Dispose()
    }
}

function Get-PeSubsystem {
    param([string]$ExePath)

    $bytes = [IO.File]::ReadAllBytes($ExePath)
    if ($bytes.Length -lt 0x100) {
        throw "PE file is too small to inspect: $ExePath"
    }

    $peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
    $optionalHeaderOffset = $peOffset + 24
    if ($optionalHeaderOffset + 0x46 -gt $bytes.Length) {
        throw "PE optional header is truncated: $ExePath"
    }

    return [BitConverter]::ToUInt16($bytes, $optionalHeaderOffset + 0x44)
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
        [string]$Flavor = ""
    )

    $buildInfoPath = Get-BuildInfoPath $ExePath
    if (-not (Test-Path -LiteralPath $buildInfoPath)) {
        throw "release build info is missing at $buildInfoPath"
    }

    $exeItem = Get-Item -LiteralPath $ExePath
    $buildInfo = Get-Content -LiteralPath $buildInfoPath -Raw | ConvertFrom-Json
    if ($Flavor -and $buildInfo.flavor -ne $Flavor) {
        throw "release build info flavor '$($buildInfo.flavor)' does not match expected '$Flavor'"
    }
    if ($buildInfo.executable -ne $exeItem.Name) {
        throw "release build info executable mismatch"
    }
    if ([int64]$buildInfo.executableBytes -ne $exeItem.Length) {
        throw "release build info executableBytes mismatch"
    }
    if ([string]$buildInfo.executableSha256 -ne (Get-ExeSha256 $exeItem.FullName)) {
        throw "release build info hash does not match current executable"
    }
    return $buildInfo
}

function Get-CargoMetadataPackage {
    param(
        [object]$Metadata,
        [string]$Name
    )

    $packages = @($Metadata.packages | Where-Object { $_.name -eq $Name })
    if ($packages.Count -ne 1) {
        throw "Expected exactly one Cargo package named $Name, found $($packages.Count)"
    }
    return $packages[0]
}

function Get-CargoMetadataFeature {
    param(
        [object]$Package,
        [string]$Name
    )

    $property = $Package.features.PSObject.Properties[$Name]
    if ($null -eq $property) {
        throw "Cargo package $($Package.name) is missing feature '$Name'"
    }
    return @($property.Value | ForEach-Object { [string]$_ })
}

function Get-CargoMetadataDependency {
    param(
        [object]$Package,
        [string]$Name
    )

    $dependencies = @($Package.dependencies | Where-Object { $_.name -eq $Name -and $null -eq $_.kind })
    if ($dependencies.Count -ne 1) {
        throw "Expected exactly one normal dependency named $Name in $($Package.name), found $($dependencies.Count)"
    }
    return $dependencies[0]
}

function Assert-CargoFeatureEmpty {
    param(
        [string]$PackageName,
        [string]$FeatureName,
        [string[]]$Entries
    )

    if ($Entries.Count -ne 0) {
        throw "Cargo package $PackageName feature '$FeatureName' must be empty, found: $($Entries -join ', ')"
    }
}

function Assert-CargoFeatureContains {
    param(
        [string]$PackageName,
        [string]$FeatureName,
        [string[]]$Entries,
        [string]$Expected
    )

    if ($Entries -notcontains $Expected) {
        throw "Cargo package $PackageName feature '$FeatureName' must include '$Expected', found: $($Entries -join ', ')"
    }
}

function Assert-OcrFeatureBoundary {
    param([string]$MetadataOutput)

    $jsonStart = $MetadataOutput.IndexOf("{")
    $jsonEnd = $MetadataOutput.LastIndexOf("}")
    if ($jsonStart -lt 0 -or $jsonEnd -lt $jsonStart) {
        throw "Cargo metadata output did not contain JSON"
    }

    $metadataJson = $MetadataOutput.Substring($jsonStart, $jsonEnd - $jsonStart + 1)
    $metadata = $metadataJson | ConvertFrom-Json

    $corePackage = Get-CargoMetadataPackage $metadata "screen-watch-core"
    $tauriPackage = Get-CargoMetadataPackage $metadata "screen-watch-ocr-tauri"

    $coreDefault = Get-CargoMetadataFeature $corePackage "default"
    Assert-CargoFeatureEmpty "screen-watch-core" "default" $coreDefault

    $coreOcr = Get-CargoMetadataFeature $corePackage "ocr"
    Assert-CargoFeatureContains "screen-watch-core" "ocr" $coreOcr "dep:pure-onnx-ocr"

    $pureOcrDependency = Get-CargoMetadataDependency $corePackage "pure-onnx-ocr"
    if (-not [bool]$pureOcrDependency.optional) {
        throw "screen-watch-core dependency pure-onnx-ocr must remain optional"
    }

    $tauriDefault = Get-CargoMetadataFeature $tauriPackage "default"
    Assert-CargoFeatureEmpty "screen-watch-ocr-tauri" "default" $tauriDefault

    $tauriOcr = Get-CargoMetadataFeature $tauriPackage "ocr"
    Assert-CargoFeatureContains "screen-watch-ocr-tauri" "ocr" $tauriOcr "screen-watch-core/ocr"

    $coreDependency = Get-CargoMetadataDependency $tauriPackage "screen-watch-core"
    $enabledCoreFeatures = @($coreDependency.features | ForEach-Object { [string]$_ })
    if ($enabledCoreFeatures -contains "ocr") {
        throw "screen-watch-ocr-tauri must not enable screen-watch-core/ocr from its default dependency declaration"
    }
}

function Assert-OcrDependencyTrees {
    param(
        [string]$LiteTreeOutput,
        [string]$FullTreeOutput,
        [string[]]$OcrCrateNames
    )

    foreach ($crateName in $OcrCrateNames) {
        if ($LiteTreeOutput.Contains($crateName)) {
            throw "Tauri lite dependency tree must not include OCR crate '$crateName'"
        }
        if (-not $FullTreeOutput.Contains($crateName)) {
            throw "Tauri full dependency tree must include OCR crate '$crateName'"
        }
    }
}

function Get-RustStringConst {
    param(
        [string]$Source,
        [string]$Name
    )

    $pattern = "pub\s+const\s+$([regex]::Escape($Name)):\s*&str\s*=\s*`"([^`"]+)`"\s*;"
    $match = [regex]::Match($Source, $pattern)
    if (-not $match.Success) {
        throw "Could not find Rust string constant $Name"
    }
    return $match.Groups[1].Value
}

function Get-JavaScriptStringConst {
    param(
        [string]$Source,
        [string]$Name
    )

    $pattern = "\bconst\s+$([regex]::Escape($Name))\s*=\s*['`"]([^'`"]+)['`"]\s*;"
    $match = [regex]::Match($Source, $pattern)
    if (-not $match.Success) {
        throw "Could not find JavaScript string constant $Name"
    }
    return $match.Groups[1].Value
}

function Get-RustStringArrayConst {
    param(
        [string]$Source,
        [string]$Name
    )

    $pattern = "pub\s+const\s+$([regex]::Escape($Name)):\s*\[&str;\s*\d+\]\s*=\s*\[(.*?)\];"
    $match = [regex]::Match($Source, $pattern, [System.Text.RegularExpressions.RegexOptions]::Singleline)
    if (-not $match.Success) {
        throw "Could not find Rust string array constant $Name"
    }
    return @(
        [regex]::Matches($match.Groups[1].Value, "`"([^`"]+)`"") |
            ForEach-Object { $_.Groups[1].Value }
    )
}

function Assert-TextContains {
    param(
        [string]$Name,
        [string]$Text,
        [string]$Expected
    )

    if (-not $Text.Contains($Expected)) {
        throw "$Name is missing expected contract text: $Expected"
    }
}

function Assert-PortableOcrContract {
    param([string]$ProjectRootPath)

    $dataDirSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\data_dir.rs") -Raw
    $buildSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\build.rs") -Raw
    $buildScriptSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\build.rs") -Raw
    $ocrSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\ocr.rs") -Raw
    $portableSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\package-portable.ps1") -Raw

    $appName = Get-RustStringConst $dataDirSource "APP_NAME"
    $buildFlavorEnv = Get-RustStringConst $buildSource "BUILD_FLAVOR_ENV"
    $compiledBuildFlavorEnv = Get-RustStringConst $buildSource "COMPILED_BUILD_FLAVOR_ENV"
    $ocrModelDirEnv = Get-RustStringConst $ocrSource "OCR_MODEL_DIR_ENV"
    $requiredModels = Get-RustStringArrayConst $ocrSource "REQUIRED_NATIVE_OCR_ASSETS"

    if ($buildSource.Contains("std::env::var(")) {
        throw "Build flavor must be packaged compile-time state, not a runtime environment override"
    }
    Assert-TextContains "build flavor compile-time env" $buildSource "option_env!(`"$compiledBuildFlavorEnv`")"
    Assert-TextContains "core build script env tracking" $buildScriptSource "cargo:rerun-if-env-changed=$buildFlavorEnv"
    Assert-TextContains "core build script compiled flavor" $buildScriptSource "cargo:rustc-env=$compiledBuildFlavorEnv="
    Assert-TextContains "portable package build-info sidecar" $portableSource "screen-watch-ocr-tauri.build-info.json"
    Assert-TextContains "portable package hash guard" $portableSource "executableSha256"
    if (-not $ocrSource.Contains('data_dir.join("models").join("rapidocr")')) {
        throw "Rust default OCR model directory must remain <app-data>\models\rapidocr"
    }

    $defaultWindowsModelDir = "%LOCALAPPDATA%\$appName\models\rapidocr"
    Assert-TextContains "portable package build flavor contract" $portableSource "`$env:$buildFlavorEnv = `$Flavor"
    Assert-TextContains "portable package manifest" $portableSource "appDataDirectoryName = `"$appName`""
    Assert-TextContains "portable package manifest" $portableSource "ocrModelDirEnv = `"$ocrModelDirEnv`""
    Assert-TextContains "portable package manifest" $portableSource "defaultWindowsOcrModelDir = `"$defaultWindowsModelDir`""
    Assert-TextContains "portable package README" $portableSource "User data directory name remains $appName"
    Assert-TextContains "portable package README" $portableSource $defaultWindowsModelDir
    Assert-TextContains "portable package README" $portableSource $ocrModelDirEnv

    foreach ($model in $requiredModels) {
        Assert-TextContains "portable package OCR model contract" $portableSource "`"$model`""
        Assert-TextContains "portable package README" $portableSource $model
    }
}

function Assert-ExternalOcrAssetBoundaryContract {
    param([string]$ProjectRootPath)

    $ocrSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\ocr.rs") -Raw
    $requiredModels = Get-RustStringArrayConst $ocrSource "REQUIRED_NATIVE_OCR_ASSETS"
    $forbiddenNames = New-Object System.Collections.Generic.HashSet[string]([StringComparer]::OrdinalIgnoreCase)
    foreach ($model in $requiredModels) {
        $forbiddenNames.Add($model) | Out-Null
    }

    $excludedDirectories = New-Object System.Collections.Generic.HashSet[string]([StringComparer]::OrdinalIgnoreCase)
    foreach ($name in @(".git", "dist", "node_modules", "target")) {
        $excludedDirectories.Add($name) | Out-Null
    }

    $queue = New-Object System.Collections.Generic.Queue[System.IO.DirectoryInfo]
    foreach ($item in Get-ChildItem -LiteralPath $ProjectRootPath -Force) {
        if ($item.PSIsContainer) {
            if (-not $excludedDirectories.Contains($item.Name)) {
                $queue.Enqueue($item)
            }
        } elseif ($item.Extension -ieq ".onnx" -or $forbiddenNames.Contains($item.Name)) {
            throw "OCR model asset must stay external, but source tree contains file: $($item.FullName)"
        }
    }

    $violations = New-Object System.Collections.Generic.List[string]
    while ($queue.Count -gt 0) {
        $dir = $queue.Dequeue()
        $relativeDir = $dir.FullName.Substring($ProjectRootPath.Length).TrimStart([char]92, [char]47)
        if ($relativeDir -match '(^|[\\/])models[\\/]rapidocr($|[\\/])') {
            $violations.Add($dir.FullName) | Out-Null
            continue
        }

        foreach ($item in Get-ChildItem -LiteralPath $dir.FullName -Force) {
            if ($item.PSIsContainer) {
                if (-not $excludedDirectories.Contains($item.Name)) {
                    $queue.Enqueue($item)
                }
            } elseif ($item.Extension -ieq ".onnx" -or $forbiddenNames.Contains($item.Name)) {
                $violations.Add($item.FullName) | Out-Null
            }
        }
    }

    if ($violations.Count -gt 0) {
        throw "OCR model assets must stay external; unexpected source-tree asset(s): $($violations -join ', ')"
    }
}

function Assert-TauriBuildScriptContract {
    param([string]$ProjectRootPath)

    $packageJson = Get-Content -LiteralPath (Join-Path $ProjectRootPath "package.json") -Raw | ConvertFrom-Json
    $buildSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\build.rs") -Raw
    $buildScriptSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\build-tauri.mjs") -Raw

    $buildFlavorEnv = Get-RustStringConst $buildSource "BUILD_FLAVOR_ENV"
    $compiledBuildFlavorEnv = Get-RustStringConst $buildSource "COMPILED_BUILD_FLAVOR_ENV"

    if ($packageJson.scripts.'tauri:build:lite' -ne "node scripts/build-tauri.mjs lite") {
        throw "package.json tauri:build:lite must route through scripts/build-tauri.mjs lite"
    }
    if ($packageJson.scripts.'tauri:build:full' -ne "node scripts/build-tauri.mjs full") {
        throw "package.json tauri:build:full must route through scripts/build-tauri.mjs full"
    }

    Assert-TextContains "Tauri build script flavor default" $buildScriptSource 'const flavor = process.argv[2] === "full" ? "full" : "lite";'
    Assert-TextContains "Tauri build script command" $buildScriptSource 'const args = ["tauri", "build"];'
    Assert-TextContains "Tauri build script full OCR feature" $buildScriptSource 'args.push("--features", "ocr");'
    Assert-TextContains "Tauri build script npx tauri" $buildScriptSource 'spawnSync("npx", args'
    Assert-TextContains "Tauri build script packaged flavor env" $buildScriptSource "$($buildFlavorEnv): flavor"
    Assert-TextContains "Tauri build script release exe path" $buildScriptSource 'join("target", "release", "screen-watch-ocr-tauri.exe")'
    Assert-TextContains "Tauri build script build-info sidecar" $buildScriptSource 'screen-watch-ocr-tauri.build-info.json'
    Assert-TextContains "Tauri build script flavor build-info sidecar" $buildScriptSource 'screen-watch-ocr-tauri.${flavor}.build-info.json'
    Assert-TextContains "Tauri build script sha256" $buildScriptSource 'createHash("sha256")'
    Assert-TextContains "Tauri build script executable bytes" $buildScriptSource 'executableBytes: exe.size'
    Assert-TextContains "Tauri build script build flavor env metadata" $buildScriptSource "buildFlavorEnv: `"$buildFlavorEnv`""
    Assert-TextContains "Tauri build script compiled flavor env metadata" $buildScriptSource "compiledBuildFlavorEnv: `"$compiledBuildFlavorEnv`""
    Assert-TextContains "Tauri build script flavor installer copy" $buildScriptSource '`_x64-${flavor}-setup.exe`'
    Assert-TextContains "Tauri build script stale Python installer cleanup" $buildScriptSource 'Screen Watch OCR_\d+\.\d+\.\d+_x64'
    Assert-TextContains "Tauri build script removes stale Python installer artifacts" $buildScriptSource 'rmSync(join(nsisDir, name))'
}

function Assert-PackageScriptContract {
    param([string]$ProjectRootPath)

    $packageJson = Get-Content -LiteralPath (Join-Path $ProjectRootPath "package.json") -Raw | ConvertFrom-Json
    $expectedScripts = [ordered]@{
        "dev" = "vite --host 127.0.0.1"
        "build" = "vite build"
        "test:frontend" = "node --test src/*.test.js"
        "verify:migration" = "powershell -ExecutionPolicy Bypass -File scripts/verify-migration.ps1"
        "ocr:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/ocr-smoke.ps1"
        "ocr:corpus:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/ocr-corpus-smoke.ps1"
        "ocr:text:parity" = "powershell -ExecutionPolicy Bypass -File scripts/ocr-text-parity-smoke.ps1"
        "template:benchmark" = "powershell -ExecutionPolicy Bypass -File scripts/template-benchmark.ps1"
        "template:parity" = "powershell -ExecutionPolicy Bypass -File scripts/template-parity-benchmark.ps1"
        "production:template:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/production-template-performance-smoke.ps1"
        "packaged:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/packaged-smoke.ps1"
        "tray:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/tray-menu-smoke.ps1"
        "coexistence:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/coexistence-smoke.ps1"
        "python:profile:compat" = "powershell -ExecutionPolicy Bypass -File scripts/python-profile-compat-smoke.ps1"
        "manual:evidence" = "powershell -ExecutionPolicy Bypass -File scripts/manual-gate-evidence.ps1"
        "evidence:references" = "powershell -ExecutionPolicy Bypass -File scripts/evidence-reference-check.ps1"
        "webview:visual:smoke" = "node scripts/webview-visual-smoke.mjs"
        "webview:clipboard:smoke" = "node scripts/webview-visual-smoke.mjs --gate clipboard"
        "webview:scan:smoke" = "node scripts/webview-visual-smoke.mjs --gate scan"
        "webview:ocr-lite:smoke" = "node scripts/webview-visual-smoke.mjs --gate ocr-lite-boundary"
        "webview:monitoring:smoke" = "node scripts/webview-visual-smoke.mjs --gate monitoring"
        "webview:monitoring:soak" = "node scripts/webview-visual-smoke.mjs --gate monitoring-soak"
        "webview:legacy-profile:smoke" = "node scripts/webview-visual-smoke.mjs --gate legacy-profile"
        "webview:legacy-late-window:smoke" = "node scripts/webview-visual-smoke.mjs --gate legacy-late-window"
        "webview:layout:smoke" = "node scripts/webview-visual-smoke.mjs --gate layout"
        "package:portable:lite" = "powershell -ExecutionPolicy Bypass -File scripts/package-portable.ps1 -Flavor lite"
        "package:portable:full" = "powershell -ExecutionPolicy Bypass -File scripts/package-portable.ps1 -Flavor full"
        "tauri:dev" = "tauri dev"
        "tauri:build" = "tauri build"
        "tauri:build:lite" = "node scripts/build-tauri.mjs lite"
        "tauri:build:full" = "node scripts/build-tauri.mjs full"
    }

    foreach ($scriptName in $expectedScripts.Keys) {
        $property = $packageJson.scripts.PSObject.Properties[$scriptName]
        if ($null -eq $property) {
            throw "package.json is missing required script '$scriptName'"
        }
        if ([string]$property.Value -ne $expectedScripts[$scriptName]) {
            throw "package.json script '$scriptName' must remain '$($expectedScripts[$scriptName])', found '$($property.Value)'"
        }
    }

    foreach ($scriptPath in @(
            "scripts\verify-migration.ps1",
            "scripts\ocr-smoke.ps1",
            "scripts\ocr-corpus-smoke.ps1",
            "scripts\ocr-text-parity-smoke.ps1",
            "scripts\template-benchmark.ps1",
            "scripts\template-parity-benchmark.ps1",
            "scripts\production-template-performance-smoke.ps1",
            "scripts\packaged-smoke.ps1",
            "scripts\tray-menu-smoke.ps1",
            "scripts\coexistence-smoke.ps1",
            "scripts\python-profile-compat-smoke.ps1",
            "scripts\manual-gate-evidence.ps1",
            "scripts\evidence-reference-check.ps1",
            "scripts\webview-visual-smoke.mjs",
            "scripts\package-portable.ps1",
            "scripts\build-tauri.mjs"
        )) {
        if (-not (Test-Path -LiteralPath (Join-Path $ProjectRootPath $scriptPath))) {
            throw "package.json script contract references missing file $scriptPath"
        }
    }
}

function Assert-TauriBundleConfigContract {
    param([string]$ProjectRootPath)

    $configPath = Join-Path $ProjectRootPath "src-tauri\tauri.conf.json"
    $configSource = Get-Content -LiteralPath $configPath -Raw
    $config = $configSource | ConvertFrom-Json

    if ($config.productName -ne "Screen Watch OCR Tauri") {
        throw "Tauri productName must remain Screen Watch OCR Tauri"
    }
    if ($config.build.beforeBuildCommand -ne "npm run build") {
        throw "Tauri beforeBuildCommand must remain npm run build"
    }
    if ($config.build.frontendDist -ne "../dist") {
        throw "Tauri frontendDist must remain ../dist"
    }

    $mainWindows = @($config.app.windows | Where-Object { $_.label -eq "main" })
    if ($mainWindows.Count -ne 1) {
        throw "Tauri config must define exactly one main window"
    }
    $mainWindow = $mainWindows[0]
    if ($mainWindow.title -ne "Screen Watch OCR Tauri") {
        throw "Tauri main window title must remain Screen Watch OCR Tauri"
    }
    if ($mainWindow.visible -ne $false) {
        throw "Tauri main window must start hidden so startup/tray policy controls first visibility"
    }

    if ($config.bundle.active -ne $true) {
        throw "Tauri bundle.active must remain true for repeatable packaged builds"
    }
    $targets = @($config.bundle.targets)
    if ($targets.Count -ne 1 -or $targets[0] -ne "nsis") {
        throw "Tauri bundle targets must remain exactly nsis until another installer path is verified"
    }
    foreach ($propertyName in @("resources", "externalBin")) {
        if ($config.bundle.PSObject.Properties[$propertyName]) {
            $value = $config.bundle.$propertyName
            if ($null -ne $value -and @($value).Count -gt 0) {
                throw "Tauri bundle.$propertyName must remain empty so OCR models stay external"
            }
        }
    }
    foreach ($forbidden in @(".onnx", "ppocrv5_dict.txt", "models/rapidocr", "models\rapidocr")) {
        if ($configSource.Contains($forbidden)) {
            throw "Tauri config must not reference bundled OCR model asset '$forbidden'"
        }
    }
}

function Assert-TauriIdentitySeparationContract {
    param([string]$ProjectRootPath)

    $packageJson = Get-Content -LiteralPath (Join-Path $ProjectRootPath "package.json") -Raw | ConvertFrom-Json
    $cargoToml = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\Cargo.toml") -Raw
    $config = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
    $dataDirSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\data_dir.rs") -Raw
    $singleInstanceSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\single_instance.rs") -Raw
    $startupSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\startup.rs") -Raw
    $traySource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\tray.rs") -Raw
    $windowSourcesSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\window_sources.rs") -Raw
    $packagedSmokeSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\packaged-smoke.ps1") -Raw
    $coexistenceSmokeSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\coexistence-smoke.ps1") -Raw
    $webviewSmokeSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\webview-visual-smoke.mjs") -Raw

    if ($packageJson.name -ne "screen-watch-ocr-tauri") {
        throw "Tauri package.json name must stay distinct from the Python app"
    }
    if ($packageJson.scripts.'coexistence:smoke' -ne "powershell -ExecutionPolicy Bypass -File scripts/coexistence-smoke.ps1") {
        throw "package.json must expose the packaged Python/Tauri coexistence smoke"
    }
    if (-not $cargoToml.Contains('name = "screen-watch-ocr-tauri"')) {
        throw "Tauri Cargo package name must stay screen-watch-ocr-tauri"
    }
    if ($config.productName -ne "Screen Watch OCR Tauri") {
        throw "Tauri productName must stay distinct from Python Screen Watch OCR"
    }
    if ($config.identifier -ne "local.screenwatchocrtauri.tauri") {
        throw "Tauri bundle identifier must stay distinct from the Python app"
    }
    if ($config.bundle.publisher -ne "Screen Watch OCR Tauri") {
        throw "Tauri bundle publisher must stay distinct from the Python app"
    }
    $mainWindow = @($config.app.windows | Where-Object { $_.label -eq "main" })[0]
    if ($mainWindow.title -ne "Screen Watch OCR Tauri") {
        throw "Tauri main window title must stay Screen Watch OCR Tauri"
    }

    if ((Get-RustStringConst $dataDirSource "APP_NAME") -ne "ScreenWatchOCR") {
        throw "Shared app-data directory must remain ScreenWatchOCR for Python JSON/profile compatibility"
    }

    foreach ($required in @(
            'pub const INSTANCE_PORT: u16 = 47628;',
            'pub const INSTANCE_PORT_ENV: &str = "SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT";',
            'pub const INSTANCE_COMMAND: &[u8] = b"ScreenWatchOCRTauri:show\n";',
            '.name("screen-watch-tauri-single-instance".to_string())'
        )) {
        if (-not $singleInstanceSource.Contains($required)) {
            throw "Tauri single-instance identity is missing '$required'"
        }
    }
    foreach ($forbidden in @(
            'pub const INSTANCE_PORT: u16 = 47627;',
            'SCREENWATCH_SINGLE_INSTANCE_PORT',
            'b"ScreenWatchOCR:show\n"',
            '"screen-watch-single-instance"'
        )) {
        if ($singleInstanceSource.Contains($forbidden)) {
            throw "Tauri single-instance identity must not reuse Python value '$forbidden'"
        }
    }

    if (-not $startupSource.Contains('pub const STARTUP_LINK_NAME: &str = "屏幕监控OCR Tauri.lnk";')) {
        throw "Tauri startup shortcut must use the distinct Tauri link name"
    }
    if ($startupSource.Contains('pub const STARTUP_LINK_NAME: &str = "屏幕监控OCR.lnk";')) {
        throw "Tauri startup shortcut must not use the legacy Python link name"
    }

    foreach ($required in @(
            '"ScreenWatchOCR"',
            '"Screen Watch OCR"',
            '"Screen Watch OCR Tauri"',
            'GetCurrentProcessId',
            'if pid == unsafe { GetCurrentProcessId() }',
            'is_ignored_app_title(&title)'
        )) {
        if (-not $windowSourcesSource.Contains($required)) {
            throw "Tauri app-window list self-filter contract is missing '$required'"
        }
    }

    foreach ($required in @(
            '$PythonPort = 47627',
            '$TauriPort = 47628',
            'ScreenWatchOCR:show',
            'ScreenWatchOCRTauri:show',
            'Python and Tauri deliverables must not have the same exe name',
            'Python and Tauri process names must not match',
            'Python app accepted the Tauri single-instance command',
            'Tauri app accepted the Python single-instance command',
            'Get-ProcessTreeRecords',
            'WEBVIEW2_USER_DATA_FOLDER',
            'Python packaged app process tree did not expose a visible Screen Watch OCR window',
            'Tauri packaged app process tree did not expose a visible Screen Watch OCR Tauri window',
            'Tauri WebView2 child process did not use the smoke-owned user data folder',
            'refusing to touch an existing app'
        )) {
        if (-not $coexistenceSmokeSource.Contains($required)) {
            throw "Packaged coexistence smoke is missing '$required'"
        }
    }

    foreach ($required in @(
            'pub const TRAY_ID: &str = "screen-watch-ocr-tauri-main";',
            'pub const TRAY_MENU_SHOW_ID: &str = "screen-watch-ocr-tauri-show";',
            'pub const TRAY_MENU_EXIT_ID: &str = "screen-watch-ocr-tauri-exit";',
            'pub const TRAY_MENU_SHOW_LABEL: &str = "Show Tauri";',
            'pub const TRAY_MENU_EXIT_LABEL: &str = "Exit Tauri";',
            '"Screen Watch OCR Tauri - Ready"',
            '"Screen Watch OCR Tauri - Monitoring"'
        )) {
        if (-not $traySource.Contains($required)) {
            throw "Tauri tray identity is missing '$required'"
        }
    }
    foreach ($forbidden in @(
            'pub const TRAY_ID: &str = "screen-watch-ocr-main";',
            'pub const TRAY_MENU_SHOW_ID: &str = "screen-watch-ocr-show";',
            'pub const TRAY_MENU_EXIT_ID: &str = "screen-watch-ocr-exit";',
            '"Screen Watch OCR - Ready"',
            '"Screen Watch OCR - Monitoring"'
        )) {
        if ($traySource.Contains($forbidden)) {
            throw "Tauri tray identity must not reuse Python/old value '$forbidden'"
        }
    }

    foreach ($required in @(
            'SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT',
            'Test-TcpPortBusy 47628',
            '"Screen Watch OCR Tauri"'
        )) {
        if (-not $packagedSmokeSource.Contains($required)) {
            throw "Packaged smoke identity contract is missing '$required'"
        }
    }
    if ($packagedSmokeSource.Contains('SCREENWATCH_SINGLE_INSTANCE_PORT') -or
        $packagedSmokeSource.Contains('Test-TcpPortBusy 47627')) {
        throw "Packaged smoke must not use the old Python single-instance identity"
    }
    if (-not $webviewSmokeSource.Contains('SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT')) {
        throw "WebView visual smoke must use the Tauri single-instance env var"
    }
    if ($webviewSmokeSource.Contains('SCREENWATCH_SINGLE_INSTANCE_PORT')) {
        throw "WebView visual smoke must not use the old Python single-instance env var"
    }
    if (-not $webviewSmokeSource.Contains('buildInfoMatchesActual')) {
        throw "WebView visual smoke must report whether target build-info matches the supplied exe"
    }
    if (-not $webviewSmokeSource.Contains('not the supplied exe')) {
        throw "WebView visual smoke must distinguish target release build-info from a supplied final exe"
    }

    $docsToCheck = @(
        "docs\DECISION.md",
        "docs\FUNCTIONAL_ACCEPTANCE.md",
        "docs\ACCEPTANCE.md",
        "docs\MANUAL_GATES.md",
        "docs\manual-gate-evidence\installer-repeatability-smoke.md"
    )
    foreach ($doc in $docsToCheck) {
        $source = Get-Content -LiteralPath (Join-Path $ProjectRootPath $doc) -Raw
        foreach ($forbidden in @(
                'ScreenWatchOCR:show\n',
                '127.0.0.1:47627',
                'Screen Watch OCR_0.1.0_x64'
            )) {
            if ($source.Contains($forbidden)) {
                throw "Identity documentation must not preserve old app identity '$forbidden' in $doc"
            }
        }
    }
}

function Assert-OcrSmokeContract {
    param([string]$ProjectRootPath)

    $dataDirSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\data_dir.rs") -Raw
    $ocrSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\ocr.rs") -Raw
    $smokeSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\ocr-smoke.ps1") -Raw
    $corpusSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\ocr-corpus-smoke.ps1") -Raw

    $appName = Get-RustStringConst $dataDirSource "APP_NAME"
    $ocrModelDirEnv = Get-RustStringConst $ocrSource "OCR_MODEL_DIR_ENV"
    $requiredModels = Get-RustStringArrayConst $ocrSource "REQUIRED_NATIVE_OCR_ASSETS"

    if (-not $ocrSource.Contains('data_dir.join("models").join("rapidocr")')) {
        throw "Rust default OCR model directory must remain <app-data>\models\rapidocr"
    }

    Assert-TextContains "OCR smoke default Windows model directory" $smokeSource "$appName\models\rapidocr"
    Assert-TextContains "OCR smoke default Unix model directory" $smokeSource "$appName/models/rapidocr"
    Assert-TextContains "OCR smoke model env read" $smokeSource ('$env:' + $ocrModelDirEnv)
    Assert-TextContains "OCR smoke model env write" $smokeSource ('$env:' + $ocrModelDirEnv + ' = $effectiveModelDir')
    Assert-TextContains "OCR smoke model dir output" $smokeSource "modelDir:"
    Assert-TextContains "OCR smoke required model output" $smokeSource "requiredModels:"
    Assert-TextContains "OCR smoke ready output" $smokeSource "modelReady:"
    Assert-TextContains "OCR smoke missing output" $smokeSource "modelMissing:"
    Assert-TextContains "OCR smoke missing model error" $smokeSource "Missing external OCR model asset(s):"
    Assert-TextContains "OCR smoke model override hint" $smokeSource "-ModelDir"
    Assert-TextContains "OCR smoke model env hint" $smokeSource $ocrModelDirEnv
    Assert-TextContains "OCR smoke missing-model self-test switch" $smokeSource '[switch]$SelfTestMissingModels'
    Assert-TextContains "OCR smoke missing-model self-test output" $smokeSource "missingModelSelfTest: passed"
    Assert-TextContains "OCR smoke image env" $smokeSource "SCREENWATCH_OCR_SMOKE_IMAGE"
    Assert-TextContains "OCR smoke expected text env" $smokeSource "SCREENWATCH_OCR_SMOKE_EXPECT"
    Assert-TextContains "OCR smoke image validation" $smokeSource "OCR smoke image does not exist"
    Assert-TextContains "OCR smoke paired image args" $smokeSource "Pass both -Image and -Expect"

    foreach ($model in $requiredModels) {
        Assert-TextContains "OCR smoke required model contract" $smokeSource "`"$model`""
    }

    $preflightIndex = $smokeSource.IndexOf('Assert-OcrModelAssets $effectiveModelDir')
    $probeIndex = $smokeSource.IndexOf('native_ocr_real_model_probe_initializes_from_external_assets')
    if ($preflightIndex -lt 0 -or $probeIndex -lt 0 -or $preflightIndex -gt $probeIndex) {
        throw "OCR smoke script must preflight model assets before running the real-model Cargo probe"
    }

    foreach ($expected in @(
            "target\ocr-model-smoke\monkt-ppocrv5-english",
            "target\ocr-model-smoke\monkt-ppocrv5-chinese",
            "target\ocr-corpus-smoke",
            "ocr-corpus-smoke-`$stamp-result.json",
            "scripts\ocr-smoke.ps1",
            "english-ready",
            "READY",
            "english-alert-number",
            "ALERT 42",
            "english-ocr-test",
            "OCR TEST",
            "english-scan-complete",
            "SCAN COMPLETE",
            "english-error-percent",
            "ERROR 100%",
            "chinese-ready",
            "0x51C6",
            "0x5907",
            "chinese-monitor",
            "0x76D1",
            "0x63A7",
            "chinese-screen-monitor",
            "0x5C4F",
            "0x5E55",
            "chinese-alert",
            "0x53D1",
            "0x73B0",
            "0x5F02",
            "0x5E38",
            "does not bundle OCR models into the lite exe",
            "PP-OCRv6/RapidOCR-native"
        )) {
        Assert-TextContains "OCR corpus smoke contract" $corpusSource $expected
    }
}

function Assert-AcceptanceRealGateDocumentationContract {
    param(
        [string]$ProjectRootPath,
        [string[]]$WorkspaceGateTests,
        [string[]]$OcrGateTests
    )

    $acceptancePath = Join-Path $ProjectRootPath "docs\FUNCTIONAL_ACCEPTANCE.md"
    if (-not (Test-Path -LiteralPath $acceptancePath)) {
        throw "Missing functional acceptance checklist at $acceptancePath"
    }

    $acceptanceSource = Get-Content -LiteralPath $acceptancePath -Raw
    Assert-TextContains "functional acceptance completion rule" $acceptanceSource "## Required Completion Rule"
    Assert-TextContains "functional acceptance manual gate section" $acceptanceSource "## Tracked Real And Manual Gate Names"

    $requiredNames = @($WorkspaceGateTests) + @($OcrGateTests)
    $missing = @($requiredNames | Where-Object { -not $acceptanceSource.Contains($_) })
    if ($missing.Count -gt 0) {
        throw "Functional acceptance checklist is missing required real/manual gate test names: $($missing -join ', ')"
    }
}

function Assert-ManualGateRunbookContract {
    param([string]$ProjectRootPath)

    $runbookPath = Join-Path $ProjectRootPath "docs\MANUAL_GATES.md"
    if (-not (Test-Path -LiteralPath $runbookPath)) {
        throw "Missing manual gates runbook at $runbookPath"
    }

    $runbookSource = Get-Content -LiteralPath $runbookPath -Raw
    foreach ($section in @(
            "# Manual Gates Runbook",
            "## Evidence Record Template",
            "## Baseline Before Manual Gates",
            "## Desktop Backend Smoke",
            "## Real OCR Model Smoke",
            "## WebView Source Preview Visual Smoke",
            "## Template Gallery Visual Workflow Smoke",
            "## Profile Clipboard Paste Smoke",
            "## Profile One Shot Scan Smoke",
            "## OCR Lite Boundary Smoke",
            "## Legacy Profile End-to-End Smoke",
            "## Legacy Late-Start Window End-to-End Smoke",
            "## Python Read Tauri Profile Compat Smoke",
            "## Profile Monitoring Restart Smoke",
            "## Profile Monitoring Soak Smoke",
            "## WebView Layout Resize Smoke",
            "## Packaged App Smoke",
            "## Packaged Tray Menu And Icon Smoke",
            "## Packaged Python Tauri Coexistence Smoke",
            "## Installer Repeatability Smoke",
            "## Production Template Performance Smoke"
        )) {
        Assert-TextContains "manual gate runbook section" $runbookSource $section
    }

    foreach ($command in @(
            'powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease',
            'powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipPython -SkipFrontend -SkipRelease -IncludeDesktopSmoke',
            'powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeOcrSmoke -OcrModelDir "D:\Models\rapidocr"',
            'powershell -ExecutionPolicy Bypass -File scripts\verify-migration.ps1 -SkipRelease -IncludeOcrSmoke -OcrModelDir "D:\Models\rapidocr" -OcrSmokeImage ".\smoke.png" -OcrSmokeExpect "READY"',
            'npm run ocr:corpus:smoke',
            'npm run tauri:dev',
            'npm run webview:visual:smoke -- --gate source',
            'npm run webview:visual:smoke -- --gate gallery',
            'npm run webview:clipboard:smoke',
            'npm run webview:scan:smoke',
            'npm run webview:ocr-lite:smoke',
            'npm run webview:legacy-profile:smoke',
            'npm run webview:legacy-late-window:smoke',
            'npm run python:profile:compat',
            'npm run webview:monitoring:smoke',
            'npm run webview:monitoring:soak',
            'npm run webview:monitoring:soak -- --soak-ms 30000',
            'npm run webview:layout:smoke',
            'npm run tauri:build:lite',
            'npm run tauri:build:full',
            'npm run tray:smoke -- -ExePath target\release\screen-watch-ocr-tauri.exe',
            'npm run coexistence:smoke -- -TauriExePath target\release\screen-watch-ocr-tauri.exe',
            'npm run manual:evidence -- -New',
            'npm run manual:evidence -- -Status',
            'npm run manual:evidence',
            'npm run evidence:references',
            'powershell -ExecutionPolicy Bypass -File scripts\packaged-smoke.ps1 -ExePath target\release\screen-watch-ocr-tauri.exe -StartupWaitSeconds 18',
            'npm run template:parity',
            'powershell -ExecutionPolicy Bypass -File scripts\template-benchmark.ps1',
            'npm run production:template:smoke'
        )) {
        Assert-TextContains "manual gate runbook command" $runbookSource $command
    }

    foreach ($requiredText in @(
            'det.onnx',
            'rec.onnx',
            'ppocrv5_dict.txt',
            'OCR models are not bundled',
            'DWM-backed window previews',
            'CF_DIB bitmap paste and CF_HDROP image-file paste',
            'alerts.jsonl plus screenshot evidence',
            'lite build: OCR module disabled',
            'OCR target requires an available OCR backend',
            'Python-shaped profile_1.json',
            'late-start remembered app window',
            'Tauri-shaped `profile_1.json`',
            'unknown future fields survived old Python save',
            'start/stop/restart monitoring',
            'target/settings splitter',
            'packagedSmokeVerified: True',
            'real system tray menu',
            'target\release\bundle',
            'Actual packaged exe hash',
            'buildInfoMatchesActual',
            'No app process remains',
            'Production dataset description',
            'Completion status: pass | fail | blocked',
            'Command(s) and exit code(s):',
            'Evidence files:',
            'Cleanup performed:',
            'Remaining risk:',
            'manual-gate-evidence',
            'manualGateEvidenceStatus:'
        )) {
        Assert-TextContains "manual gate runbook evidence" $runbookSource $requiredText
    }
}

function Get-RegisteredTauriCommands {
    param([string]$BackendSource)

    $handlerMatch = [regex]::Match(
        $BackendSource,
        "tauri::generate_handler!\s*\[(.*?)\]",
        [System.Text.RegularExpressions.RegexOptions]::Singleline
    )
    if (-not $handlerMatch.Success) {
        throw "Tauri command contract could not find tauri::generate_handler![...] in src-tauri\src\lib.rs"
    }

    $handlerText = $handlerMatch.Groups[1].Value
    $handlerText = [regex]::Replace($handlerText, "(?m)//.*$", "")
    $handlerText = [regex]::Replace($handlerText, "(?s)/\*.*?\*/", "")

    return @(
        [regex]::Matches($handlerText, "\b[A-Za-z_][A-Za-z0-9_]*\b") |
            ForEach-Object { $_.Value } |
            Sort-Object -Unique
    )
}

function Convert-SnakeToCamel {
    param([string]$Name)

    $parts = @($Name -split "_")
    if ($parts.Count -le 1) {
        return $Name
    }

    $out = $parts[0]
    foreach ($part in @($parts | Select-Object -Skip 1)) {
        if ($part.Length -eq 0) {
            continue
        }
        $out += $part.Substring(0, 1).ToUpperInvariant() + $part.Substring(1)
    }
    return $out
}

function Split-TopLevelComma {
    param(
        [string]$Text,
        [bool]$SingleQuoteStrings = $true
    )

    $items = New-Object System.Collections.Generic.List[string]
    $start = 0
    $roundDepth = 0
    $squareDepth = 0
    $curlyDepth = 0
    $angleDepth = 0
    $quote = $null
    $escape = $false
    for ($i = 0; $i -lt $Text.Length; $i++) {
        $ch = $Text[$i]
        if ($null -ne $quote) {
            if ($escape) {
                $escape = $false
                continue
            }
            if ($ch -eq [char]92) {
                $escape = $true
                continue
            }
            if ($ch -eq $quote) {
                $quote = $null
            }
            continue
        }
        if ($ch -eq [char]34 -or ($SingleQuoteStrings -and $ch -eq [char]39) -or $ch -eq [char]96) {
            $quote = $ch
            continue
        }

        switch ($ch) {
            "(" { $roundDepth += 1; continue }
            ")" { if ($roundDepth -gt 0) { $roundDepth -= 1 }; continue }
            "[" { $squareDepth += 1; continue }
            "]" { if ($squareDepth -gt 0) { $squareDepth -= 1 }; continue }
            "{" { $curlyDepth += 1; continue }
            "}" { if ($curlyDepth -gt 0) { $curlyDepth -= 1 }; continue }
            "<" { $angleDepth += 1; continue }
            ">" { if ($angleDepth -gt 0) { $angleDepth -= 1 }; continue }
            "," {
                if ($roundDepth -eq 0 -and $squareDepth -eq 0 -and $curlyDepth -eq 0 -and $angleDepth -eq 0) {
                    $items.Add($Text.Substring($start, $i - $start)) | Out-Null
                    $start = $i + 1
                }
            }
        }
    }
    $items.Add($Text.Substring($start)) | Out-Null
    return @($items)
}

function Find-JavaScriptObjectEnd {
    param(
        [string]$Source,
        [int]$OpenIndex
    )

    $depth = 0
    $quote = $null
    $escape = $false
    for ($i = $OpenIndex; $i -lt $Source.Length; $i++) {
        $ch = $Source[$i]
        if ($null -ne $quote) {
            if ($escape) {
                $escape = $false
                continue
            }
            if ($ch -eq [char]92) {
                $escape = $true
                continue
            }
            if ($ch -eq $quote) {
                $quote = $null
            }
            continue
        }
        if ($ch -eq [char]34 -or $ch -eq [char]39 -or $ch -eq [char]96) {
            $quote = $ch
            continue
        }
        if ($ch -eq "{") {
            $depth += 1
        } elseif ($ch -eq "}") {
            $depth -= 1
            if ($depth -eq 0) {
                return $i
            }
        }
    }
    throw "Could not find the end of a JavaScript object literal starting at offset $OpenIndex"
}

function Get-JavaScriptObjectKeys {
    param([string]$ObjectText)

    $trimmed = $ObjectText.Trim()
    if (-not $trimmed.StartsWith("{") -or -not $trimmed.EndsWith("}")) {
        throw "JavaScript invoke argument parser expected an object literal"
    }
    $body = $trimmed.Substring(1, $trimmed.Length - 2)
    return @(
        Split-TopLevelComma $body |
            ForEach-Object {
                $entry = $_.Trim()
                if (-not $entry -or $entry.StartsWith("...")) {
                    return
                }
                $keyMatch = [regex]::Match($entry, '^([A-Za-z_$][A-Za-z0-9_$]*)\s*:')
                if ($keyMatch.Success) {
                    $keyMatch.Groups[1].Value
                    return
                }
                $shorthandMatch = [regex]::Match($entry, '^([A-Za-z_$][A-Za-z0-9_$]*)$')
                if ($shorthandMatch.Success) {
                    $shorthandMatch.Groups[1].Value
                }
            } |
            Sort-Object -Unique
    )
}

function Get-FrontendInvokeArgumentMap {
    param([string]$FrontendSource)

    $map = @{}
    $invokeMatches = [regex]::Matches($FrontendSource, "invoke\(\s*['`"]([A-Za-z_][A-Za-z0-9_]*)['`"]")
    foreach ($match in $invokeMatches) {
        $command = $match.Groups[1].Value
        if (-not $map.ContainsKey($command)) {
            $map[$command] = New-Object System.Collections.Generic.HashSet[string]
        }

        $pos = $match.Index + $match.Length
        while ($pos -lt $FrontendSource.Length -and [char]::IsWhiteSpace($FrontendSource[$pos])) {
            $pos += 1
        }
        if ($pos -ge $FrontendSource.Length -or $FrontendSource[$pos] -ne ",") {
            continue
        }
        $pos += 1
        while ($pos -lt $FrontendSource.Length -and [char]::IsWhiteSpace($FrontendSource[$pos])) {
            $pos += 1
        }
        if ($pos -ge $FrontendSource.Length -or $FrontendSource[$pos] -ne "{") {
            continue
        }

        $end = Find-JavaScriptObjectEnd $FrontendSource $pos
        $objectText = $FrontendSource.Substring($pos, $end - $pos + 1)
        foreach ($key in Get-JavaScriptObjectKeys $objectText) {
            $map[$command].Add($key) | Out-Null
        }
    }

    $result = @{}
    foreach ($key in $map.Keys) {
        $result[$key] = @($map[$key] | Sort-Object)
    }
    return $result
}

function Get-TauriCommandParameterMap {
    param([string]$BackendSource)

    $map = @{}
    $commandMatches = [regex]::Matches(
        $BackendSource,
        '(?s)#\[\s*tauri::command(?:\([^\]]*\))?\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\((.*?)\)\s*(?:->|\{)'
    )
    foreach ($match in $commandMatches) {
        $command = $match.Groups[1].Value
        $params = New-Object System.Collections.Generic.List[string]
        foreach ($param in Split-TopLevelComma -Text $match.Groups[2].Value -SingleQuoteStrings $false) {
            $nameMatch = [regex]::Match($param.Trim(), '^([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(.+)$')
            if (-not $nameMatch.Success) {
                continue
            }
            $typeText = $nameMatch.Groups[2].Value.Trim()
            if ($typeText -match '\btauri::(Window|State|AppHandle)\b') {
                continue
            }
            $params.Add((Convert-SnakeToCamel $nameMatch.Groups[1].Value)) | Out-Null
        }
        $map[$command] = @($params | Sort-Object -Unique)
    }
    return $map
}

function Assert-FrontendCommandContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $backendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\lib.rs") -Raw

    $frontendCommands = @(
        [regex]::Matches($frontendSource, "invoke\(\s*['`"]([A-Za-z_][A-Za-z0-9_]*)['`"]") |
            ForEach-Object { $_.Groups[1].Value } |
            Sort-Object -Unique
    )
    if ($frontendCommands.Count -eq 0) {
        throw "Frontend command contract found no invoke() calls in src\main.js"
    }

    $backendCommands = Get-RegisteredTauriCommands $backendSource
    if ($backendCommands.Count -eq 0) {
        throw "Frontend command contract found no backend commands in generate_handler![...]"
    }

    $missing = @($frontendCommands | Where-Object { $backendCommands -cnotcontains $_ })
    if ($missing.Count -gt 0) {
        throw "Frontend invokes unregistered Tauri command(s): $($missing -join ', ')"
    }
}

function Assert-FrontendCommandArgumentContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $backendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\lib.rs") -Raw
    $frontendArguments = Get-FrontendInvokeArgumentMap $frontendSource
    $backendParameters = Get-TauriCommandParameterMap $backendSource

    foreach ($command in @($frontendArguments.Keys | Sort-Object)) {
        if (-not $backendParameters.ContainsKey($command)) {
            throw "Frontend command argument contract could not find backend parameters for '$command'"
        }
        $frontendKeys = @($frontendArguments[$command])
        $expectedKeys = @($backendParameters[$command])
        $missing = @($expectedKeys | Where-Object { $frontendKeys -cnotcontains $_ })
        $extra = @($frontendKeys | Where-Object { $expectedKeys -cnotcontains $_ })
        if ($missing.Count -gt 0 -or $extra.Count -gt 0) {
            $parts = @()
            if ($missing.Count -gt 0) {
                $parts += "missing frontend key(s): $($missing -join ', ')"
            }
            if ($extra.Count -gt 0) {
                $parts += "unexpected frontend key(s): $($extra -join ', ')"
            }
            throw "Frontend invoke argument mismatch for '$command' ($($parts -join '; '))"
        }
    }

    Assert-TextContains `
        "Frontend paste_profile_template_images numeric maxTemplates" `
        $frontendSource `
        "maxTemplates: profileImportLimitValue()"
    Assert-TextContains `
        "Frontend profile import limit coercion" `
        $frontendSource `
        "profileImportRequest([], profileImportLimitInput()).maxTemplates"
}

function Get-HtmlIds {
    param([string]$HtmlSource)

    return @(
        [regex]::Matches($HtmlSource, '\bid\s*=\s*["'']([^"'']+)["'']') |
            ForEach-Object { $_.Groups[1].Value } |
            Sort-Object -Unique
    )
}

function Get-HtmlAttribute {
    param(
        [string]$AttributeText,
        [string]$Name
    )

    $match = [regex]::Match($AttributeText, "(?i)\b$([regex]::Escape($Name))\s*=\s*[""']([^""']+)[""']")
    if ($match.Success) {
        return $match.Groups[1].Value
    }
    return $null
}

function Get-HtmlActionControls {
    param([string]$HtmlSource)

    $controls = @()
    foreach ($match in [regex]::Matches($HtmlSource, '(?is)<(button|select|input)\b([^>]*)>')) {
        $tag = $match.Groups[1].Value.ToLowerInvariant()
        $attributes = $match.Groups[2].Value
        $id = Get-HtmlAttribute $attributes "id"
        if (-not $id) {
            continue
        }

        $event = $null
        if ($tag -eq "button") {
            $event = "click"
        } elseif ($tag -eq "select") {
            $event = "change"
        } elseif ($tag -eq "input") {
            $type = Get-HtmlAttribute $attributes "type"
            if ($type -and $type.ToLowerInvariant() -eq "checkbox") {
                $event = "change"
            }
        }

        if ($event) {
            $controls += [pscustomobject]@{
                id = $id
                event = $event
                tag = $tag
            }
        }
    }
    return @($controls)
}

function Get-FrontendDomIdSelectors {
    param([string]$FrontendSource)

    return @(
        [regex]::Matches(
            $FrontendSource,
            'querySelector(?:All)?\(\s*["''`]#([A-Za-z][A-Za-z0-9_-]*)["''`]\s*\)'
        ) |
            ForEach-Object { $_.Groups[1].Value } |
            Sort-Object -Unique
    )
}

function Add-FrontendEventBinding {
    param(
        [hashtable]$Bindings,
        [string]$Event,
        [string]$Id
    )

    if (-not $Bindings.ContainsKey($Event)) {
        $Bindings[$Event] = New-Object System.Collections.Generic.HashSet[string]
    }
    $Bindings[$Event].Add($Id) | Out-Null
}

function Get-FrontendEventBindings {
    param([string]$FrontendSource)

    $bindings = @{}
    foreach ($match in [regex]::Matches(
            $FrontendSource,
            '(?s)querySelector\(\s*["''`]#([A-Za-z][A-Za-z0-9_-]*)["''`]\s*\)\s*\.addEventListener\(\s*["''`]([A-Za-z][A-Za-z0-9_-]*)["''`]'
        )) {
        Add-FrontendEventBinding $bindings $match.Groups[2].Value $match.Groups[1].Value
    }

    foreach ($match in [regex]::Matches(
            $FrontendSource,
            '(?s)\[(?<items>.*?)\]\s*\.forEach\(\s*\(\s*(?<var>[A-Za-z_$][A-Za-z0-9_$]*)\s*\)\s*=>\s*\{(?<body>.*?)document\.querySelector\(\s*\k<var>\s*\)\s*\.addEventListener\(\s*["''`](?<event>[A-Za-z][A-Za-z0-9_-]*)["''`]'
        )) {
        $event = $match.Groups["event"].Value
        foreach ($selectorMatch in [regex]::Matches($match.Groups["items"].Value, '["''`]#([A-Za-z][A-Za-z0-9_-]*)["''`]')) {
            Add-FrontendEventBinding $bindings $event $selectorMatch.Groups[1].Value
        }
    }

    return $bindings
}

function Assert-FrontendDomContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $htmlSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "index.html") -Raw
    $selectors = Get-FrontendDomIdSelectors $frontendSource
    $htmlIds = Get-HtmlIds $htmlSource
    if ($selectors.Count -eq 0) {
        throw "Frontend DOM contract found no #id querySelector calls in src\main.js"
    }
    if ($htmlIds.Count -eq 0) {
        throw "Frontend DOM contract found no id attributes in index.html"
    }

    $missing = @($selectors | Where-Object { $htmlIds -cnotcontains $_ })
    if ($missing.Count -gt 0) {
        throw "Frontend DOM selector(s) missing from index.html id attributes: $($missing -join ', ')"
    }
}

function Assert-FrontendActionBindingContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $htmlSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "index.html") -Raw
    $controls = Get-HtmlActionControls $htmlSource
    $bindings = Get-FrontendEventBindings $frontendSource

    if ($controls.Count -eq 0) {
        throw "Frontend action binding contract found no static action controls in index.html"
    }

    $missing = New-Object System.Collections.Generic.List[string]
    foreach ($control in $controls) {
        if (-not $bindings.ContainsKey($control.event) -or -not $bindings[$control.event].Contains($control.id)) {
            $missing.Add("$($control.id):$($control.event)") | Out-Null
        }
    }

    if ($missing.Count -gt 0) {
        throw "Static frontend action control(s) missing event binding in src\main.js: $($missing -join ', ')"
    }
}

function Assert-FrontendDynamicTargetContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw

    Assert-TextContains "frontend dynamic target rendering" $frontendSource "function renderProfileTarget(target, index)"
    Assert-TextContains "frontend target click selection/open path" $frontendSource 'item.addEventListener("click", () => clickProfileTarget(index))'
    Assert-TextContains "frontend target context menu binding" $frontendSource 'item.addEventListener("contextmenu", (event) => {'
    Assert-TextContains "frontend target context menu action" $frontendSource "showTargetContextMenu(event.clientX, event.clientY, target, index)"
    Assert-TextContains "frontend target drag start" $frontendSource 'item.addEventListener("dragstart", (event) => {'
    Assert-TextContains "frontend target drag source index" $frontendSource "draggedTargetIndex = index"
    Assert-TextContains "frontend target drag payload" $frontendSource 'event.dataTransfer.setData("text/plain", String(index))'
    Assert-TextContains "frontend target dragover midpoint" $frontendSource "const dropAfter = targetDropAfter(item, event.clientY)"
    Assert-TextContains "frontend target drop insert index" $frontendSource "const insertIndex = targetDropInsertIndex(index, dropAfter)"
    Assert-TextContains "frontend target drop reorder" $frontendSource "await reorderProfileTarget(draggedTargetIndex, insertIndex)"
    Assert-TextContains "frontend target dragend preview retry" $frontendSource "scheduleSourcePreviews(SOURCE_PREVIEW_BUSY_RETRY_MS)"
    Assert-TextContains "frontend target enabled checkbox" $frontendSource 'enabled.addEventListener("change", () =>'
    Assert-TextContains "frontend target enabled command path" $frontendSource "setProfileTargetEnabled(index, enabled.checked)"

    $requiredActions = @(
        @{ Label = "上移"; Function = "reorderProfileTarget(index, actionState.moveUpInsertIndex)" },
        @{ Label = "下移"; Function = "reorderProfileTarget(index, actionState.moveDownInsertIndex)" },
        @{ Label = "打开"; Function = "openProfileTarget(index)" },
        @{ Label = "清零"; Function = "clearProfileHitCount(actionState.targetId)" },
        @{ Label = "删除"; Function = "removeProfileTarget(index)" }
    )
    foreach ($action in $requiredActions) {
        Assert-TextContains "frontend target action label" $frontendSource "`"$($action.Label)`""
        Assert-TextContains "frontend target action command" $frontendSource $action.Function
    }

    Assert-TextContains "frontend target menu open action" $frontendSource 'targetMenuButton("打开图片", () => openProfileTarget(index), !menuState.canOpen)'
    Assert-TextContains "frontend target menu clear action" $frontendSource "clearProfileHitCount(target.id)"

    foreach ($command in @(
            "set_profile_target_enabled",
            "reorder_profile_target",
            "remove_profile_target",
            "clear_profile_targets",
            "clear_profile_target_hit_count",
            "open_profile_target_file",
            "add_profile_template_pngs",
            "capture_profile_source_template"
        )) {
        Assert-TextContains "frontend target backend command" $frontendSource "invoke(`"$command`""
    }
}

function Assert-LegacyVisibleWorkflowContract {
    param(
        [string]$ProjectRootPath,
        [string]$PythonProjectPath
    )

    $pythonSource = Get-Content -LiteralPath (Join-Path $PythonProjectPath "src\screen_watch\app.py") -Raw
    $htmlSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "index.html") -Raw
    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $frontendBehaviorSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\ui-behavior.js") -Raw
    $frontendTestSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\ui-behavior.test.js") -Raw
    $backendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\lib.rs") -Raw
    $profileSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\profile.rs") -Raw
    $backendCommands = Get-RegisteredTauriCommands $backendSource

    $workflows = @(
        @{
            Name = "profile slot"
            Legacy = @("profile_box = ttk.Combobox", 'profile_box.bind("<<ComboboxSelected>>", self.switch_profile)')
            HtmlIds = @("profile-number")
            Frontend = @('querySelector("#profile-number")', 'addEventListener("change", switchProfile)')
            Commands = @("load_profile", "save_last_profile")
        },
        @{
            Name = "startup toggle"
            Legacy = @("self.make_check(profile_bar, self.startup_enabled", "self.toggle_startup")
            HtmlIds = @("startup-toggle")
            Frontend = @('querySelector("#startup-toggle")', "setStartup(event.target.checked)")
            Commands = @("startup_status", "set_startup_enabled")
        },
        @{
            Name = "image upload"
            Legacy = @("command=self.add_files", "def add_files")
            HtmlIds = @("profile-select-pngs")
            Frontend = @('querySelector("#profile-select-pngs")', "selectProfilePngs", "importProfilePaths")
            Commands = @("select_profile_template_pngs", "add_profile_template_pngs")
        },
        @{
            Name = "clipboard paste"
            Legacy = @("command=self.paste_images", "def paste_images")
            HtmlIds = @("profile-paste-images")
            Frontend = @('querySelector("#profile-paste-images")', "pasteProfileImages")
            Commands = @("paste_profile_template_images")
        },
        @{
            Name = "capture source as target"
            Legacy = @("command=self.capture_as_target", "def capture_as_target")
            HtmlIds = @("profile-capture-target")
            Frontend = @('querySelector("#profile-capture-target")', "captureProfileTarget")
            Commands = @("capture_profile_source_template")
        },
        @{
            Name = "delete selected target"
            Legacy = @("command=self.remove_selected", "def remove_selected")
            HtmlIds = @("profile-delete-selected")
            Frontend = @('querySelector("#profile-delete-selected")', "removeSelectedProfileTarget")
            Commands = @("remove_profile_target")
        },
        @{
            Name = "clear all targets"
            Legacy = @("command=self.clear_targets", "def clear_targets")
            HtmlIds = @("profile-clear-all")
            Frontend = @('querySelector("#profile-clear-all")', "clearProfileTargets")
            Commands = @("clear_profile_targets")
        },
        @{
            Name = "target enable and invert"
            Legacy = @("command=self.toggle_all_targets", "def toggle_all_targets", "def toggle_target")
            HtmlIds = @("profile-toggle-all", "profile-targets")
            Frontend = @('querySelector("#profile-toggle-all")', "toggleProfileTargets", "setProfileTargetEnabled(index, enabled.checked)")
            Commands = @("toggle_all_profile_targets", "set_profile_target_enabled")
        },
        @{
            Name = "target open reorder and hit menu"
            Legacy = @("def open_target_file", "def reorder_target", "def clear_target_hit_count")
            HtmlIds = @("profile-targets")
            Frontend = @("clickProfileTarget", "openProfileTarget(index)", "reorderProfileTarget", "showTargetContextMenu", "clearProfileHitCount")
            Commands = @("open_profile_target_file", "reorder_profile_target", "clear_profile_target_hit_count")
        },
        @{
            Name = "screen source selection"
            Legacy = @("command=self.refresh_monitors", "def refresh_monitors", "def selected_regions")
            HtmlIds = @("monitors", "refresh")
            Frontend = @('querySelector("#refresh")', "renderMonitorList", "ensureDefaultMonitorSelection")
            Commands = @("list_monitors", "save_profile_sources")
        },
        @{
            Name = "window source selection"
            Legacy = @("self.window_combo", "def refresh_windows", "def selected_windows")
            HtmlIds = @("windows", "refresh-windows")
            Frontend = @('querySelector("#refresh-windows")', "refreshWindows", "rememberWindowApp")
            Commands = @("list_app_windows", "save_profile_sources")
        },
        @{
            Name = "source preview"
            Legacy = @("self.source_canvas", "def refresh_source_previews")
            HtmlIds = @("source-previews", "refresh-source-previews")
            Frontend = @('querySelector("#refresh-source-previews")', "refreshSourcePreviews", "renderSourcePreviewCards")
            Commands = @("capture_screen_region_preview_cached", "capture_window_preview_cached")
        },
        @{
            Name = "region and match settings"
            Legacy = @("def region_for", "def detector_config", "self.threshold", "self.interval_ms")
            HtmlIds = @("profile-region-left", "profile-region-top", "profile-region-width", "profile-region-height", "profile-threshold", "profile-scales", "profile-interval-ms", "profile-cooldown", "profile-beep", "profile-beep-seconds", "profile-beep-volume", "profile-max-templates", "profile-max-alerts")
            Frontend = @("buildProfileOptions", "profileRegionInputs", "profileRegion", "persistProfileSources", 'querySelector("#profile-threshold")', "legacyMaxAlerts = options.maxAlerts ?? legacyMaxAlerts")
            Commands = @("save_profile_sources", "build_profile_watch_config")
        },
        @{
            Name = "run scan monitor evidence"
            Legacy = @("command=self.toggle_monitoring", "command=self.scan_once", "command=self.open_evidence", "def toggle_monitoring", "def scan_once", "def open_evidence")
            HtmlIds = @("profile-monitor-start", "profile-scan-once", "open-evidence-dir", "event-log")
            Frontend = @('querySelector("#profile-monitor-start")', 'querySelector("#profile-scan-once")', 'querySelector("#open-evidence-dir")', "toggleProfileMonitoring", "scanProfileOnce", "openEvidenceDir", "appendLog")
            Commands = @("start_profile_monitoring_session", "stop_monitoring_session", "monitoring_session_status", "scan_profile_once", "open_evidence_dir")
        }
    )

    foreach ($workflow in $workflows) {
        foreach ($expected in $workflow.Legacy) {
            Assert-TextContains "legacy visible workflow '$($workflow.Name)' in Python app" $pythonSource $expected
        }
        foreach ($id in $workflow.HtmlIds) {
            Assert-TextContains "legacy visible workflow '$($workflow.Name)' HTML id" $htmlSource "id=`"$id`""
        }
        foreach ($expected in $workflow.Frontend) {
            Assert-TextContains "legacy visible workflow '$($workflow.Name)' frontend path" $frontendSource $expected
        }
        foreach ($command in $workflow.Commands) {
            Assert-TextContains "legacy visible workflow '$($workflow.Name)' frontend command" $frontendSource "invoke(`"$command`""
            if ($backendCommands -cnotcontains $command) {
                throw "Legacy visible workflow '$($workflow.Name)' backend command '$command' is not registered in tauri::generate_handler!"
            }
        }
    }

    Assert-TextContains "legacy visible workflow max_alerts backend state save" $backendSource "save_max_alerts_at(data_dir, max_alerts)"
    Assert-TextContains "legacy visible workflow max_alerts state helper" $profileSource "pub fn save_max_alerts_at"
    Assert-TextContains "legacy visible workflow max_alerts profile cleanup" $profileSource 'out.remove("max_alerts")'
    Assert-TextContains "legacy target select button starts as all-select" $pythonSource 'self.target_select_btn = ttk.Button(gallery_label, text="全选"'
    Assert-TextContains "legacy target select button switches to invert when all enabled" $pythonSource 'self.target_select_btn.configure(text="反选" if all_selected else "全选")'
    Assert-TextContains "frontend target select button starts as all-select" $htmlSource '<button id="profile-toggle-all" type="button">全选</button>'
    Assert-TextContains "frontend target select button uses parity helper" $frontendSource "profileToggleAllLabel(profile)"
    Assert-TextContains "frontend target select button parity helper" $frontendBehaviorSource "export function profileToggleAllLabel"
    Assert-TextContains "frontend target select button parity test" $frontendTestSource 'profileToggleAllLabel({ targets: [] }), "全选"'
    Assert-TextContains "frontend target select button parity test" $frontendTestSource 'profileToggleAllLabel({ targets: [{ enabled: true }, {}] })'
}

function Assert-LegacyUiSurfaceContract {
    param(
        [string]$ProjectRootPath,
        [string]$PythonProjectPath
    )

    $pythonSource = Get-Content -LiteralPath (Join-Path $PythonProjectPath "src\screen_watch\app.py") -Raw
    $htmlSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "index.html") -Raw
    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $frontendBehaviorSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\ui-behavior.js") -Raw
    $frontendTestSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\ui-behavior.test.js") -Raw
    $tauriSurface = "$htmlSource`n$frontendSource`n$frontendBehaviorSource`n$frontendTestSource"

    $surfaceItems = @(
        @{
            Name = "profile slot and startup controls"
            Legacy = @('ttk.Label(profile_bar, text="配置位")', '"开机自启"', 'values=list(range(1, PROFILE_COUNT + 1))')
            Tauri = @('配置位', '开机自启', 'id="profile-number"', '<option value="1">1</option>', '<option value="5">5</option>', 'id="startup-toggle"')
        },
        @{
            Name = "template gallery toolbar"
            Legacy = @('"上传图片"', '"粘贴图片"', '"截图作模板"', '"删除选中"', '"清空"', '"匹配图片"')
            Tauri = @('上传图片', '粘贴图片', '截图作模板', '删除选中', '清空', '匹配图片', 'id="profile-select-pngs"', 'id="profile-paste-images"', 'id="profile-capture-target"', 'id="profile-delete-selected"', 'id="profile-clear-all"')
        },
        @{
            Name = "target selection and summary"
            Legacy = @('text="全选"', 'text="反选"', '当前 {len(self.targets)} 张模板，启用 {len(self.enabled_targets())} 张')
            Tauri = @('全选', '反选', '当前 ${totalCount} 张模板，启用 ${enabledCount} 张', 'id="profile-summary"', 'profileToggleAllLabel')
        },
        @{
            Name = "source and preview controls"
            Legacy = @('"监控屏幕"', '"监控应用"', '"刷新屏幕"', 'self.window_combo', '"来源预览"')
            Tauri = @('监控屏幕', '监控应用', '刷新应用', '来源预览', '刷新预览', '抓取预览', '抓取窗口', 'id="monitors"', 'id="windows"', 'id="source-previews"')
        },
        @{
            Name = "region fields"
            Legacy = @('"区域"', '("左", self.left)', '("上", self.top)', '("宽(空=全屏)", self.width)', '("高(空=全屏)", self.height)')
            Tauri = @('区域', '左', '上', '宽(空=全屏)', '高(空=全屏)', 'id="profile-region-left"', 'id="profile-region-top"', 'id="profile-region-width"', 'id="profile-region-height"')
        },
        @{
            Name = "match and retention settings"
            Legacy = @('"匹配"', '"阈值"', '"缩放"', '"间隔ms"', '"同图冷却秒"', '"蜂鸣秒"', '"蜂鸣音量"', '"模板最多张"', '"截图最多张"')
            Tauri = @('匹配', '阈值', '缩放', '间隔ms', '同图冷却秒', '蜂鸣秒', '蜂鸣音量', '模板最多张', '截图最多张', '命中蜂鸣', 'id="profile-threshold"', 'id="profile-scales"', 'id="profile-interval-ms"', 'id="profile-cooldown"', 'id="profile-beep-seconds"', 'id="profile-beep-volume"', 'id="profile-max-templates"', 'id="profile-max-alerts"')
        },
        @{
            Name = "run actions and evidence"
            Legacy = @('"运行"', '"开始监控"', '"停止监控"', '"扫描一次"', '"打开证据目录"')
            Tauri = @('运行', '开始监控', '停止监控', '扫描一次', '打开证据目录', 'id="profile-monitor-start"', 'id="profile-scan-once"', 'id="open-evidence-dir"')
        },
        @{
            Name = "status, log, and evidence feedback"
            Legacy = @('self.status = StringVar', '"报警与扫描日志"', 'columns=("time", "message")', 'self.log.heading("time", text="时间")', 'self.log.heading("message", text="事件")', 'self.log.insert("", 0', 'self.status.set')
            Tauri = @('id="status"', '报警与扫描日志', '<th>时间</th>', '<th>事件</th>', 'id="event-log"', 'appendLog', 'scanStatusText', 'monitoringStatusText')
        }
    )

    foreach ($item in $surfaceItems) {
        foreach ($expected in $item.Legacy) {
            Assert-TextContains "legacy UI surface '$($item.Name)' in Python app" $pythonSource $expected
        }
        foreach ($expected in $item.Tauri) {
            Assert-TextContains "legacy UI surface '$($item.Name)' in Tauri app" $tauriSurface $expected
        }
    }

    Assert-TextContains "legacy UI surface compact layout app grid" $htmlSource 'id="app-grid"'
    Assert-TextContains "legacy UI surface compact layout target/control split" $htmlSource 'data-splitter="targets-controls"'
    Assert-TextContains "legacy UI surface compact layout control/preview split" $htmlSource 'data-splitter="controls-preview"'
    Assert-TextContains "legacy UI surface compact layout target/log split" $htmlSource 'data-splitter="targets-log"'
    Assert-TextContains "legacy UI surface resizable layout tests" $frontendTestSource "resizeThreePaneLayout"
    Assert-TextContains "legacy UI surface resizable layout tests" $frontendTestSource "resizeStackedPaneLayout"
    Assert-TextContains "legacy UI surface resizable layout tests" $frontendTestSource "resizeMultiPaneLayout"
}

function Assert-LegacyProfilePersistenceContract {
    param(
        [string]$ProjectRootPath,
        [string]$PythonProjectPath
    )

    $pythonSource = Get-Content -LiteralPath (Join-Path $PythonProjectPath "src\screen_watch\app.py") -Raw
    $profileSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\profile.rs") -Raw
    $pythonCompatSmokeSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\python-profile-compat-smoke.ps1") -Raw
    $frontendBehaviorSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\ui-behavior.js") -Raw
    $frontendTestSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\ui-behavior.test.js") -Raw

    $saveStateMatch = [regex]::Match($pythonSource, "(?s)def save_state\(self\):.*?def current_window_geometry")
    if (-not $saveStateMatch.Success) {
        throw "Could not locate Python save_state block"
    }
    $saveState = $saveStateMatch.Value
    foreach ($expected in @(
            '"last_profile": self.current_profile',
            '"layout": {',
            '"geometry": geometry',
            '"main_ratio": self.main_ratio',
            '"right_ratio": self.right_ratio',
            '"left_ratio": self.left_ratio',
            '"max_alerts": self.max_alerts.get()'
        )) {
        Assert-TextContains "Python save_state persistence contract" $saveState $expected
    }

    $saveProfileMatch = [regex]::Match($pythonSource, "(?s)def save_current_profile\(self\):.*?def load_profile")
    if (-not $saveProfileMatch.Success) {
        throw "Could not locate Python save_current_profile block"
    }
    $saveProfile = $saveProfileMatch.Value
    foreach ($expected in @(
            '"targets": self.targets',
            '"monitors": [i for i, var in self.monitor_vars.items() if var.get()]',
            '"windows": self.selected_apps',
            '"region": {"left": self.left.get(), "top": self.top.get(), "width": self.width.get(), "height": self.height.get()}',
            '"threshold": self.threshold.get()',
            '"scales": self.scales.get()',
            '"interval_ms": self.interval_ms.get()',
            '"cooldown": self.cooldown.get()',
            '"beep": self.beep.get()',
            '"beep_seconds": self.beep_seconds.get()',
            '"beep_volume": self.beep_volume.get()',
            '"max_templates": self.max_templates.get()'
        )) {
        Assert-TextContains "Python profile persistence contract" $saveProfile $expected
    }
    if ($saveProfile.Contains('"max_alerts"')) {
        throw "Python profile persistence contract changed: max_alerts must remain global state, not profile match data"
    }

    foreach ($expected in @(
            "pub profile_region: Option<RegionConfig>",
            ".or_else(|| options.regions.first())",
            'out.remove("max_alerts")',
            "pub fn save_max_alerts_at"
        )) {
        Assert-TextContains "Tauri profile persistence contract" $profileSource $expected
    }
    Assert-TextContains "frontend profile region persistence contract" $frontendBehaviorSource "profileRegion: normalizedRegion(state.region)"
    Assert-TextContains "frontend window-only region persistence test" $frontendTestSource "profile source options keep region inputs even without selected monitors"

    foreach ($expected in @(
            'future_profile_preserved_after_python_save',
            'future_match_preserved_after_python_save',
            'future_state_preserved_after_python_save',
            'future_layout_preserved_after_python_save',
            'external_target_hit_count_preserved_after_python_stale_save',
            'external_state_max_alerts_preserved_after_python_stale_save',
            'external_layout_unknown_preserved_after_python_stale_save',
            'external_profile["targets"][0]["hit_count"] = 9',
            'external_state["max_alerts"] = 99',
            'assert loaded["selected_apps"] == [{"title": "Tauri Compatibility Window", "ordinal": 2}], loaded',
            'assert saved["profile_has_required_keys"], saved_profile',
            'assert saved["state_has_required_keys"], saved_state'
        )) {
        Assert-TextContains "Python read Tauri profile compatibility smoke contract" $pythonCompatSmokeSource $expected
    }
}

function Assert-AudioAlarmParityContract {
    param(
        [string]$ProjectRootPath,
        [string]$PythonProjectPath
    )

    $pythonSource = Get-Content -LiteralPath (Join-Path $PythonProjectPath "src\screen_watch\app.py") -Raw
    $coreAudioSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "crates\screen-watch-core\src\audio.rs") -Raw
    $tauriAudioSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\audio.rs") -Raw
    $backendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\lib.rs") -Raw
    $monitorSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\monitor_session.rs") -Raw

    foreach ($expected in @(
            "def beep_wave(volume, milliseconds=180, frequency=1200, sample_rate=22050):",
            "volume = parse_volume(volume)",
            "wav.setnchannels(1)",
            "wav.setsampwidth(2)",
            "wav.setframerate(sample_rate)",
            "winsound.PlaySound(beep_wave(level), winsound.SND_MEMORY)",
            "with self.beep_lock:",
            "if now < self.beep_until:",
            "threading.Thread(target=beep_for"
        )) {
        Assert-TextContains "Python audio alarm parity source" $pythonSource $expected
    }

    foreach ($expected in @(
            "pub const DEFAULT_BEEP_MILLISECONDS: u32 = 180;",
            "pub const DEFAULT_BEEP_FREQUENCY_HZ: u32 = 1200;",
            "pub const DEFAULT_BEEP_SAMPLE_RATE: u32 = 22_050;",
            "pub fn clamp_volume(value: i32) -> u8",
            "out.extend_from_slice(b`"RIFF`");",
            "out.extend_from_slice(b`"WAVE`");",
            "out.extend_from_slice(b`"fmt `");",
            "out.extend_from_slice(b`"data`");",
            "pub struct BeepThrottle",
            "fn beep_wave_is_pcm_wav_and_volume_changes_amplitude()",
            "fn throttle_does_not_restart_while_beeping()"
        )) {
        Assert-TextContains "Rust core audio alarm parity" $coreAudioSource $expected
    }

    foreach ($expected in @(
            "pub fn start_for_alarm(&self, alarm: &AlarmConfig) -> bool",
            "if !alarm.beep",
            "self.start(alarm.beep_seconds, alarm.beep_volume)",
            "thread::spawn(move || play_beep_for(duration, volume));",
            "if clamp_volume(volume) == 0",
            "PlaySoundW",
            "SND_MEMORY",
            "PCWSTR(wav.as_ptr() as *const u16)",
            "fn start_for_alarm_respects_disabled_alarm()",
            "fn start_for_alarm_throttles_even_when_volume_is_zero()"
        )) {
        Assert-TextContains "Tauri audio alarm runtime parity" $tauriAudioSource $expected
    }

    Assert-TextContains "one-shot scan triggers alarm beep" $backendSource "beeper.start_for_alarm(&alarm);"
    Assert-TextContains "monitoring tick triggers alarm beep" $monitorSource "beeper.start_for_alarm(engine.alarm_config());"
}

function Assert-FrontendOcrReadinessContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $htmlSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "index.html") -Raw

    foreach ($id in @(
            "ocr",
            "ocr-flags",
            "model-dir",
            "ocr-models",
            "ocr-probe",
            "ocr-probe-result"
        )) {
        Assert-TextContains "frontend OCR readiness DOM" $htmlSource "id=`"$id`""
    }

    Assert-TextContains "frontend OCR status renderer" $frontendSource "function renderOcrStatus(ocr)"
    foreach ($required in @(
            'document.querySelector("#ocr").textContent',
            'document.querySelector("#ocr-flags").textContent',
            'document.querySelector("#model-dir").textContent = ocr.modelDir',
            'const models = Array.isArray(ocr.requiredModels) ? ocr.requiredModels : []',
            'const modelList = document.querySelector("#ocr-models")',
            'ocr.modelsReady',
            'ocr.backendReady',
            'ocr.backendName',
            'ocr.modelProfile',
            'model.exists',
            'model.name',
            'model.bytes',
            'model.path',
            'formatBytes(model.bytes)'
        )) {
        Assert-TextContains "frontend OCR readiness renderer" $frontendSource $required
    }

    Assert-TextContains "frontend OCR probe function" $frontendSource "async function probeOcrBackend()"
    Assert-TextContains "frontend OCR probe command" $frontendSource 'invoke("ocr_backend_probe")'
    Assert-TextContains "frontend OCR probe binding" $frontendSource 'document.querySelector("#ocr-probe").addEventListener("click", probeOcrBackend)'
    Assert-TextContains "frontend OCR probe result renderer" $frontendSource "function renderOcrProbeResult(probe)"
    foreach ($required in @(
            "attempted: Boolean(probe.attempted)",
            "initialized: Boolean(probe.initialized)",
            "reason: probe.reason || `"-`"",
            "error: probe.error || null",
            "backend: availability.backendName || `"-`"",
            "profile: availability.modelProfile || `"-`"",
            "modelDir: availability.modelDir || `"-`""
        )) {
        Assert-TextContains "frontend OCR probe result" $frontendSource $required
    }
}

function Assert-FrontendSourcePreviewContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $behaviorSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\ui-behavior.js") -Raw

    Assert-TextContains "frontend source preview visible rect import" $frontendSource "visiblePreviewRect,"
    Assert-TextContains "frontend source preview visible rect helper" $behaviorSource "export function visiblePreviewRect"
    Assert-TextContains "frontend source preview visible rect sync" $frontendSource "const rect = visiblePreviewRect(card.frame)"
    Assert-TextContains "frontend source preview DWM sync command" $frontendSource 'invoke("sync_dwm_preview"'
    foreach ($argument in @("sourceKey: source.key", "hwnd: source.hwnd", "left: rect.left", "top: rect.top", "width: rect.width", "height: rect.height")) {
        Assert-TextContains "frontend source preview DWM sync argument" $frontendSource $argument
    }
    Assert-TextContains "frontend source preview DWM class on success" $frontendSource 'card.card.classList.add("uses-dwm")'
    Assert-TextContains "frontend source preview DWM class on invisible/failure" $frontendSource 'card.card.classList.remove("uses-dwm")'
    Assert-TextContains "frontend source preview bitmap fallback presentation" $frontendSource "sourcePreviewCardPresentation({ dwmActive, error })"
    Assert-TextContains "frontend source preview frame clear after refresh" $frontendSource "clearSourcePreviewFrames(card?.frame)"
    Assert-TextContains "frontend source preview retain selected DWM keys" $frontendSource 'invoke("retain_dwm_preview_sources", { sourceKeys })'
}

function Assert-BackendCommandContract {
    param([string]$ProjectRootPath)

    $backendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\lib.rs") -Raw
    $registeredCommands = Get-RegisteredTauriCommands $backendSource
    $commandFunctions = @(
        [regex]::Matches(
            $backendSource,
            '(?s)#\[\s*tauri::command(?:\([^\]]*\))?\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)'
        ) |
            ForEach-Object { $_.Groups[1].Value } |
            Sort-Object -Unique
    )
    if ($commandFunctions.Count -eq 0) {
        throw "Backend command contract found no #[tauri::command] functions in src-tauri\src\lib.rs"
    }

    $missing = @($commandFunctions | Where-Object { $registeredCommands -cnotcontains $_ })
    if ($missing.Count -gt 0) {
        throw "Backend #[tauri::command] function(s) are missing from generate_handler![...]: $($missing -join ', ')"
    }

    $extra = @($registeredCommands | Where-Object { $commandFunctions -cnotcontains $_ })
    if ($extra.Count -gt 0) {
        throw "generate_handler![...] registers function(s) without #[tauri::command]: $($extra -join ', ')"
    }
}

function Assert-MonitorSessionEventContract {
    param([string]$ProjectRootPath)

    $frontendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src\main.js") -Raw
    $monitorSessionSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\monitor_session.rs") -Raw
    $backendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\lib.rs") -Raw

    $frontendEvent = Get-JavaScriptStringConst $frontendSource "MONITOR_SESSION_EVENT"
    $backendEvent = Get-RustStringConst $monitorSessionSource "MONITOR_SESSION_EVENT"
    if ($frontendEvent -cne $backendEvent) {
        throw "Monitor session event mismatch: frontend '$frontendEvent' but backend '$backendEvent'"
    }
    if (-not [regex]::IsMatch($frontendSource, "listen\(\s*MONITOR_SESSION_EVENT\s*,")) {
        throw "Frontend monitor session event contract must listen with MONITOR_SESSION_EVENT"
    }
    if (-not [regex]::IsMatch($backendSource, "\.emit\(\s*MONITOR_SESSION_EVENT\s*,")) {
        throw "Backend monitor session event contract must emit with MONITOR_SESSION_EVENT"
    }
    if (-not [regex]::IsMatch($backendSource, "\bMONITOR_SESSION_EVENT\b")) {
        throw "Backend monitor session event contract must import/use MONITOR_SESSION_EVENT"
    }
}

function Assert-TrayMonitoringStatusContract {
    param([string]$ProjectRootPath)

    $backendSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\lib.rs") -Raw
    $traySource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "src-tauri\src\tray.rs") -Raw

    $updateCall = "tray::update_monitoring_status(&self.app, event.snapshot.running);"
    $emitCall = "self.window.emit(MONITOR_SESSION_EVENT, event)"
    $updateIndex = $backendSource.IndexOf($updateCall)
    $emitIndex = $backendSource.IndexOf($emitCall)
    if ($updateIndex -lt 0) {
        throw "Monitor session tray contract must update tray status from event.snapshot.running"
    }
    if ($emitIndex -lt 0) {
        throw "Monitor session tray contract could not find frontend event emit call"
    }
    if ($updateIndex -gt $emitIndex) {
        throw "Monitor session tray contract must update tray status before emitting the frontend event"
    }

    foreach ($required in @(
            "let presentation = tray_monitoring_presentation(false);",
            "let presentation = tray_monitoring_presentation(running);",
            "tray.set_tooltip(Some(presentation.tooltip))",
            "tray.set_icon(Some(presentation.icon_image()))"
        )) {
        if (-not $traySource.Contains($required)) {
            throw "Tray monitoring status contract is missing '$required'"
        }
    }

    if (-not [regex]::IsMatch($traySource, "pub\s+fn\s+tray_monitoring_presentation\s*\(\s*monitoring:\s*bool\s*\)\s*->\s*TrayMonitoringPresentation")) {
        throw "Tray monitoring status contract must expose a testable presentation helper"
    }
}

function Assert-SingleFileDeliverableContract {
    param([string]$ProjectRootPath)

    $deliverablePath = Join-Path $ProjectRootPath "release-single\ScreenWatchOCRTauri.exe"
    if (-not (Test-Path -LiteralPath $deliverablePath)) {
        return "not present"
    }

    $exeItem = Get-Item -LiteralPath $deliverablePath
    $sha256 = (Get-ExeSha256 $exeItem.FullName).ToUpperInvariant()
    $subsystem = Get-PeSubsystem $exeItem.FullName
    if ($subsystem -ne 2) {
        throw "single-file deliverable must be a Windows GUI executable, found PE subsystem $subsystem"
    }

    $auditPath = Join-Path $ProjectRootPath "docs\COMPARISON_AUDIT.md"
    $audit = Get-Content -LiteralPath $auditPath -Raw
    Assert-TextContains "comparison audit current deliverable size" $audit "- Size: $($exeItem.Length) bytes"
    Assert-TextContains "comparison audit current deliverable hash" $audit "- SHA-256: ``$sha256``"
    Assert-TextContains `
        "comparison audit current full verifier summary" `
        $audit `
        "Current rerun passed Python 98, Rust core 121 / 3 ignored, Tauri 88 / 16 ignored, OCR feature 25 / 2 ignored, frontend 103"
    Assert-TextContains `
        "comparison audit manual evidence status" `
        $audit `
        "| Manual evidence status | 19 pass, 0 blocked, 0 fail, 0 missing, 0 incomplete, 0 invalid |"

    return "$($exeItem.Length) bytes, $sha256, WindowsGui"
}

function Get-CargoLibTestCounts {
    param(
        [string]$Output,
        [string]$CrateExeName
    )

    $escaped = [regex]::Escape($CrateExeName)
    $pattern = "(?s)Running unittests src\\lib\.rs \(target\\debug\\deps\\$escaped-[^)]*\.exe\).*?test result: ok\. (\d+) passed; \d+ failed; (\d+) ignored;"
    $match = [regex]::Match($Output, $pattern)
    if (-not $match.Success) {
        throw "Could not find cargo test summary for $CrateExeName"
    }
    return [pscustomobject]@{
        Passed = [int]$match.Groups[1].Value
        Ignored = [int]$match.Groups[2].Value
    }
}

function Get-FirstCargoPassedCount {
    param([string]$Output)

    $match = [regex]::Match($Output, "test result: ok\. (\d+) passed;")
    if (-not $match.Success) {
        throw "Could not find cargo passed test count"
    }
    return [int]$match.Groups[1].Value
}

function Get-NodeTestPassedCount {
    param([string]$Output)

    $match = [regex]::Match($Output, "(?m)^\S*\s*pass\s+(\d+)\s*$")
    if (-not $match.Success) {
        throw "Could not find frontend passed test count"
    }
    return [int]$match.Groups[1].Value
}

$summary = [ordered]@{
    projectRoot = $ProjectRootPath
    pythonProject = $PythonProjectPath
    pythonTestInventory = $null
    pythonBaselineNames = if ($SkipPython) { "skipped" } else { $null }
    pythonTests = $null
    rustCoreTests = $null
    tauriTests = $null
    ocrFeatureTests = $null
    ocrSmoke = if ($IncludeOcrSmoke) { "requested" } else { "skipped" }
    templateBenchmark = if ($IncludeTemplateBenchmark) { "requested" } else { "skipped" }
    packagedSmoke = if ($IncludePackagedSmoke) { "requested" } else { "skipped" }
    frontendTests = if ($SkipFrontend) { "skipped" } else { $null }
    desktopSmoke = if ($IncludeDesktopSmoke) { "requested" } else { "skipped" }
    portablePackage = if ($IncludePortablePackage) { "requested" } else { "skipped" }
    fullPortablePackage = if ($IncludeFullPortablePackage) { "requested" } else { "skipped" }
    buildFlavorContract = $null
    externalOcrAssetBoundary = $null
    packageScriptContract = $null
    tauriBuildScriptContract = $null
    tauriBundleConfigContract = $null
    tauriIdentitySeparationContract = $null
    portableOcrContract = $null
    ocrSmokeContract = $null
    ocrSmokeMissingModelSelfTest = $null
    acceptanceRealGateContract = $null
    manualGateRunbookContract = $null
    manualGateEvidenceSelfTest = $null
    evidenceReferenceContract = $null
    frontendCommandContract = $null
    frontendCommandArgumentContract = $null
    frontendDomContract = $null
    frontendActionBindingContract = $null
    frontendDynamicTargetContract = $null
    legacyVisibleWorkflowContract = $null
    legacyUiSurfaceContract = $null
    legacyProfilePersistenceContract = $null
    audioAlarmParityContract = $null
    frontendOcrReadinessContract = $null
    frontendSourcePreviewContract = $null
    backendCommandContract = $null
    monitorSessionEventContract = $null
    trayMonitoringStatusContract = $null
    singleFileDeliverableContract = $null
    releaseBuildInfo = $null
    ocrFeatureBoundary = $null
    ocrDependencyTree = $null
    requiredRealGates = $null
    liteSizeGate = "skipped"
    frontendBuild = -not $SkipFrontend
    releaseBuild = -not $SkipRelease
    pythonExeBytes = $null
    liteExeBytes = $null
    tauriExeBytes = $null
}

$pythonExeBytes = $null
$pythonExe = Join-Path $PythonProjectPath "dist\ScreenWatchOCR.exe"
if (Test-Path $pythonExe) {
    $pythonExeBytes = (Get-Item $pythonExe).Length
    $summary.pythonExeBytes = $pythonExeBytes
}

$tauriExe = Join-Path $ProjectRootPath "target\release\screen-watch-ocr-tauri.exe"
$pythonBaselineTestsPath = Join-Path $ProjectRootPath "docs\PYTHON_BASELINE_TESTS.txt"
$liteSizeGatePassed = $false
$desktopSmokeGates = @(
    @{
        Name = "Desktop smoke: screen capture"
        Filter = "captures_tiny_screen_region_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: monitor listing"
        Filter = "real_windows_monitor_listing_matches_python_mss_indexing_on_desktop"
    },
    @{
        Name = "Desktop smoke: one-shot screen scan"
        Filter = "one_shot_scan_captures_screen_region_and_writes_evidence"
    },
    @{
        Name = "Desktop smoke: profile screen workflow"
        Filter = "profile_screen_scan_workflow_records_template_hit_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: profile screen capture template"
        Filter = "profile_screen_capture_template_writes_real_desktop_frame_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: profile window capture template"
        Filter = "profile_window_capture_template_writes_real_window_frame_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: profile remembered window capture template"
        Filter = "profile_remembered_window_capture_template_resolves_and_writes_frame_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: profile monitoring workflow"
        Filter = "profile_monitoring_session_records_template_hit_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: profile window workflow"
        Filter = "profile_window_scan_workflow_records_template_hit_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: one-shot window scan"
        Filter = "one_shot_scan_captures_window_and_writes_evidence"
    },
    @{
        Name = "Desktop smoke: monitoring session"
        Filter = "session_start_runs_ticks_and_stop_joins_worker"
    },
    @{
        Name = "Desktop smoke: window monitoring session"
        Filter = "session_start_scans_window_source_and_writes_evidence"
    },
    @{
        Name = "Desktop smoke: app-window enumeration"
        Filter = "list_app_windows_enumerates_without_panic_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: app-window preview capture"
        Filter = "capture_first_app_window_preview_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: app-window frame capture"
        Filter = "capture_first_app_window_frame_on_windows_desktop"
    },
    @{
        Name = "Desktop smoke: real DWM thumbnail"
        Filter = "real_dwm_thumbnail_registers_updates_and_clears_on_windows_desktop"
    }
)
$desktopSmokeGateFilters = @($desktopSmokeGates | ForEach-Object { [string]$_["Filter"] })
$requiredWorkspaceGateTests = @(
    "benchmark_large_frame_many_template_scan",
    "benchmark_large_frame_textured_template_scan",
    "benchmark_production_profile_template_scan"
) + $desktopSmokeGateFilters
$requiredOcrFeatureGateTests = @(
    "native_ocr_real_model_probe_initializes_from_external_assets",
    "native_ocr_real_model_recognizes_smoke_png"
)
$ocrDependencyTreeCrates = @("pure-onnx-ocr", "tract-onnx")

if (-not $SkipPython) {
    $pythonTestInventoryScript = @'
import ast
import json
import pathlib

root = pathlib.Path("tests")
items = []

class Visitor(ast.NodeVisitor):
    def __init__(self, path):
        self.path = path
        self.class_stack = []
        self.function_depth = 0

    def visit_ClassDef(self, node):
        self.class_stack.append(node.name)
        self.generic_visit(node)
        self.class_stack.pop()

    def visit_FunctionDef(self, node):
        if self.function_depth == 0 and node.name.startswith("test"):
            parts = self.class_stack + [node.name]
            test_id = f"{self.path.as_posix()}::{'.'.join(parts)}"
            items.append({
                "file": self.path.as_posix(),
                "name": ".".join(parts),
                "id": test_id,
                "line": node.lineno,
            })
        self.function_depth += 1
        self.generic_visit(node)
        self.function_depth -= 1

    visit_AsyncFunctionDef = visit_FunctionDef

for path in sorted(root.rglob("*.py")):
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    Visitor(path).visit(tree)

items.sort(key=lambda item: item["id"])
print(json.dumps({"count": len(items), "items": items}, ensure_ascii=False))
'@
    $inventoryOutput = Invoke-CapturedStep `
        -Name "Python baseline test inventory" `
        -WorkingDirectory $PythonProjectPath `
        -Script { $pythonTestInventoryScript | python - } `
        -SuppressOutput
    $inventory = $inventoryOutput | ConvertFrom-Json
    $inventoryCount = [int]$inventory.count
    if ($inventoryCount -lt $MinimumPythonTests) {
        throw "Python static test inventory count $inventoryCount is below required baseline $MinimumPythonTests"
    }
    $inventoryNames = @($inventory.items | ForEach-Object { [string]$_.id })
    $baselineNames = Get-RequiredBaselineNames $pythonBaselineTestsPath
    Assert-RequiredBaselineNames $inventoryNames $baselineNames
    $summary.pythonTestInventory = $inventoryCount
    $summary.pythonBaselineNames = "$($baselineNames.Count) locked"

    $oldPythonPath = $env:PYTHONPATH
    try {
        $env:PYTHONPATH = "src"
        $pythonOutput = Invoke-CapturedStep `
            -Name "Python baseline unittest suite" `
            -WorkingDirectory $PythonProjectPath `
            -Script { python -m unittest discover -s tests -t . -v }
    } finally {
        $env:PYTHONPATH = $oldPythonPath
    }

    if ($pythonOutput -notmatch "Ran\s+(\d+)\s+tests") {
        throw "Could not find Python unittest count in output"
    }
    $count = [int]$Matches[1]
    if ($count -lt $MinimumPythonTests) {
        throw "Python unittest count $count is below required baseline $MinimumPythonTests"
    }
    if ($count -ne $inventoryCount) {
        throw "Python unittest ran $count tests, but static inventory found $inventoryCount test functions"
    }
    $summary.pythonTests = $count
}

Invoke-CheckedStep `
    -Name "Rust formatting" `
    -WorkingDirectory $ProjectRootPath `
    -Script { cargo fmt -- --check }

$cargoMetadataOutput = Invoke-CapturedStep `
    -Name "Cargo OCR feature boundary" `
    -WorkingDirectory $ProjectRootPath `
    -Script { cargo metadata --format-version 1 --no-default-features --no-deps } `
    -SuppressOutput
Assert-OcrFeatureBoundary $cargoMetadataOutput
$summary.ocrFeatureBoundary = "passed"

Invoke-CapturedStep `
    -Name "Portable OCR model contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-PortableOcrContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.buildFlavorContract = "passed"
$summary.portableOcrContract = "passed"

Invoke-CapturedStep `
    -Name "External OCR asset boundary contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-ExternalOcrAssetBoundaryContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.externalOcrAssetBoundary = "passed"

Invoke-CapturedStep `
    -Name "Package script contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-PackageScriptContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.packageScriptContract = "passed"

Invoke-CapturedStep `
    -Name "Tauri build script contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-TauriBuildScriptContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.tauriBuildScriptContract = "passed"

Invoke-CapturedStep `
    -Name "Tauri bundle config contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-TauriBundleConfigContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.tauriBundleConfigContract = "passed"

Invoke-CapturedStep `
    -Name "Tauri identity separation contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-TauriIdentitySeparationContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.tauriIdentitySeparationContract = "passed"

Invoke-CapturedStep `
    -Name "OCR smoke script contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-OcrSmokeContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.ocrSmokeContract = "passed"

Invoke-CapturedStep `
    -Name "OCR smoke missing-model self-test" `
    -WorkingDirectory $ProjectRootPath `
    -Script { powershell -ExecutionPolicy Bypass -File scripts\ocr-smoke.ps1 -SelfTestMissingModels } `
    -SuppressOutput | Out-Null
$summary.ocrSmokeMissingModelSelfTest = "passed"

Invoke-CapturedStep `
    -Name "Acceptance real/manual gate documentation contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-AcceptanceRealGateDocumentationContract $ProjectRootPath $requiredWorkspaceGateTests $requiredOcrFeatureGateTests } `
    -SuppressOutput | Out-Null
$summary.acceptanceRealGateContract = "passed"

Invoke-CapturedStep `
    -Name "Manual gate runbook contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-ManualGateRunbookContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.manualGateRunbookContract = "passed"

Invoke-CapturedStep `
    -Name "Manual gate evidence self-test" `
    -WorkingDirectory $ProjectRootPath `
    -Script { powershell -ExecutionPolicy Bypass -File scripts\manual-gate-evidence.ps1 -SelfTest } `
    -SuppressOutput | Out-Null
$summary.manualGateEvidenceSelfTest = "passed"

Invoke-CapturedStep `
    -Name "Evidence reference contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { powershell -ExecutionPolicy Bypass -File scripts\evidence-reference-check.ps1 } `
    -SuppressOutput | Out-Null
$summary.evidenceReferenceContract = "passed"

Invoke-CapturedStep `
    -Name "Frontend command contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-FrontendCommandContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.frontendCommandContract = "passed"

Invoke-CapturedStep `
    -Name "Frontend command argument contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-FrontendCommandArgumentContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.frontendCommandArgumentContract = "passed"

Invoke-CapturedStep `
    -Name "Frontend DOM contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-FrontendDomContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.frontendDomContract = "passed"

Invoke-CapturedStep `
    -Name "Frontend action binding contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-FrontendActionBindingContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.frontendActionBindingContract = "passed"

Invoke-CapturedStep `
    -Name "Frontend dynamic target action contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-FrontendDynamicTargetContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.frontendDynamicTargetContract = "passed"

Invoke-CapturedStep `
    -Name "Legacy visible workflow contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-LegacyVisibleWorkflowContract $ProjectRootPath $PythonProjectPath } `
    -SuppressOutput | Out-Null
$summary.legacyVisibleWorkflowContract = "passed"

Invoke-CapturedStep `
    -Name "Legacy UI surface contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-LegacyUiSurfaceContract $ProjectRootPath $PythonProjectPath } `
    -SuppressOutput | Out-Null
$summary.legacyUiSurfaceContract = "passed"

Invoke-CapturedStep `
    -Name "Legacy profile persistence contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-LegacyProfilePersistenceContract $ProjectRootPath $PythonProjectPath } `
    -SuppressOutput | Out-Null
$summary.legacyProfilePersistenceContract = "passed"

Invoke-CapturedStep `
    -Name "Audio alarm parity contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-AudioAlarmParityContract $ProjectRootPath $PythonProjectPath } `
    -SuppressOutput | Out-Null
$summary.audioAlarmParityContract = "passed"

Invoke-CapturedStep `
    -Name "Frontend OCR readiness/probe contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-FrontendOcrReadinessContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.frontendOcrReadinessContract = "passed"

Invoke-CapturedStep `
    -Name "Frontend source preview contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-FrontendSourcePreviewContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.frontendSourcePreviewContract = "passed"

Invoke-CapturedStep `
    -Name "Backend command contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-BackendCommandContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.backendCommandContract = "passed"

Invoke-CapturedStep `
    -Name "Monitor session event contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-MonitorSessionEventContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.monitorSessionEventContract = "passed"

Invoke-CapturedStep `
    -Name "Tray monitoring status contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-TrayMonitoringStatusContract $ProjectRootPath } `
    -SuppressOutput | Out-Null
$summary.trayMonitoringStatusContract = "passed"

$singleFileDeliverableContract = Invoke-CapturedStep `
    -Name "Single-file deliverable contract" `
    -WorkingDirectory $ProjectRootPath `
    -Script { Assert-SingleFileDeliverableContract $ProjectRootPath } `
    -SuppressOutput
$summary.singleFileDeliverableContract = $singleFileDeliverableContract.Trim()

$rustWorkspaceOutput = Invoke-CapturedStep `
    -Name "Rust workspace tests" `
    -WorkingDirectory $ProjectRootPath `
    -Script { cargo test --workspace }
$coreCounts = Get-CargoLibTestCounts $rustWorkspaceOutput "screen_watch_core"
$tauriCounts = Get-CargoLibTestCounts $rustWorkspaceOutput "screen_watch_ocr_tauri"
Assert-MinimumCount "Rust core test" $coreCounts.Passed $MinimumRustCoreTests
Assert-MinimumCount "Tauri shell/backend test" $tauriCounts.Passed $MinimumTauriTests
Assert-OutputContainsNames "Rust workspace manual/real gate" $rustWorkspaceOutput $requiredWorkspaceGateTests
$summary.rustCoreTests = "$($coreCounts.Passed) passed, $($coreCounts.Ignored) ignored"
$summary.tauriTests = "$($tauriCounts.Passed) passed, $($tauriCounts.Ignored) ignored"

if ($IncludeTemplateBenchmark) {
    $templateBenchmarkArgs = @(
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts\template-benchmark.ps1"
    )
    if ($TemplateBenchmarkMaxMs -gt 0) {
        $templateBenchmarkArgs += @("-MaxMs", [string]$TemplateBenchmarkMaxMs)
    }
    Invoke-CheckedStep `
        -Name "Template benchmark" `
        -WorkingDirectory $ProjectRootPath `
        -Script { powershell @templateBenchmarkArgs }
    $summary.templateBenchmark = if ($TemplateBenchmarkMaxMs -gt 0) {
        "ran with max ${TemplateBenchmarkMaxMs}ms"
    } else {
        "ran"
    }
}

if ($IncludeDesktopSmoke) {
    foreach ($gate in $desktopSmokeGates) {
        $filter = $gate["Filter"]
        $name = $gate["Name"]
        Invoke-CheckedStep `
            -Name $name `
            -WorkingDirectory $ProjectRootPath `
            -Script { cargo test -p screen-watch-ocr-tauri $filter -- --ignored }
    }
    $summary.desktopSmoke = "$($desktopSmokeGates.Count) gates"
}

$ocrOutput = Invoke-CapturedStep `
    -Name "Rust OCR feature tests" `
    -WorkingDirectory $ProjectRootPath `
    -Script { cargo test -p screen-watch-core --features ocr ocr }
$ocrPassed = Get-FirstCargoPassedCount $ocrOutput
Assert-MinimumCount "Rust OCR feature test" $ocrPassed $MinimumOcrFeatureTests
Assert-OutputContainsNames "Rust OCR real-model gate" $ocrOutput $requiredOcrFeatureGateTests
$summary.ocrFeatureTests = "$ocrPassed passed"
$summary.requiredRealGates = "$($requiredWorkspaceGateTests.Count) workspace gates, $($requiredOcrFeatureGateTests.Count) OCR gates"

Invoke-CheckedStep `
    -Name "Tauri full-feature check" `
    -WorkingDirectory $ProjectRootPath `
    -Script { cargo check -p screen-watch-ocr-tauri --features ocr }

$liteDependencyTreeOutput = Invoke-CapturedStep `
    -Name "Tauri lite OCR dependency tree" `
    -WorkingDirectory $ProjectRootPath `
    -Script { cargo tree -p screen-watch-ocr-tauri --no-default-features } `
    -SuppressOutput
$fullDependencyTreeOutput = Invoke-CapturedStep `
    -Name "Tauri full OCR dependency tree" `
    -WorkingDirectory $ProjectRootPath `
    -Script { cargo tree -p screen-watch-ocr-tauri --features ocr } `
    -SuppressOutput
Assert-OcrDependencyTrees $liteDependencyTreeOutput $fullDependencyTreeOutput $ocrDependencyTreeCrates
$summary.ocrDependencyTree = "lite excludes, full includes"

if ($IncludeOcrSmoke) {
    $ocrSmokeArgs = @(
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts\ocr-smoke.ps1"
    )
    if ($OcrModelDir) {
        $ocrSmokeArgs += @("-ModelDir", $OcrModelDir)
    }
    if ($OcrSmokeImage) {
        $ocrSmokeArgs += @("-Image", $OcrSmokeImage)
    }
    if ($OcrSmokeExpect) {
        $ocrSmokeArgs += @("-Expect", $OcrSmokeExpect)
    }
    Invoke-CheckedStep `
        -Name "OCR real model smoke" `
        -WorkingDirectory $ProjectRootPath `
        -Script { powershell @ocrSmokeArgs }
    $summary.ocrSmoke = if ($OcrSmokeImage -and $OcrSmokeExpect) {
        "probe and recognition"
    } else {
        "probe"
    }
}

if (-not $SkipFrontend) {
    $frontendTestOutput = Invoke-CapturedStep `
        -Name "Frontend unit tests" `
        -WorkingDirectory $ProjectRootPath `
        -Script { npm run test:frontend }
    $frontendPassed = Get-NodeTestPassedCount $frontendTestOutput
    Assert-MinimumCount "Frontend unit test" $frontendPassed $MinimumFrontendTests
    $summary.frontendTests = "$frontendPassed passed"

    Invoke-CheckedStep `
        -Name "Frontend production build" `
        -WorkingDirectory $ProjectRootPath `
        -Script { npm run build }
}

if (-not $SkipRelease) {
    Invoke-CheckedStep `
        -Name "Tauri lite app build" `
        -WorkingDirectory $ProjectRootPath `
        -Script {
            $oldFlavor = $env:SCREENWATCH_BUILD_FLAVOR
            try {
                $env:SCREENWATCH_BUILD_FLAVOR = "lite"
                npx tauri build --no-bundle --ci
            } finally {
                $env:SCREENWATCH_BUILD_FLAVOR = $oldFlavor
            }
        }
    if (-not (Test-Path $tauriExe)) {
        throw "Tauri lite build did not produce release executable at $tauriExe"
    }
    Write-ReleaseBuildInfo -ExePath $tauriExe -Flavor "lite"
    $buildInfo = Assert-ReleaseBuildInfo -ExePath $tauriExe -Flavor "lite"
    $summary.releaseBuildInfo = "$($buildInfo.flavor), sha256 recorded"
    $liteExeBytes = (Get-Item $tauriExe).Length
    Assert-LiteExeSize `
        -TauriBytes $liteExeBytes `
        -PythonBytes $pythonExeBytes `
        -MaxBytes $MaxTauriLiteExeBytes `
        -MaxRatio $MaxTauriToPythonExeRatio
    $summary.liteExeBytes = $liteExeBytes
    $summary.liteSizeGate = "passed"
    $liteSizeGatePassed = $true
}

if ($IncludePackagedSmoke) {
    $packagedSmokeExe = Join-Path $ProjectRootPath "target\release\screen-watch-ocr-tauri.exe"
    $packagedSmokeArgs = @(
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts\packaged-smoke.ps1",
        "-ExePath",
        $packagedSmokeExe,
        "-StartupWaitSeconds",
        [string]$PackagedSmokeStartupWaitSeconds
    )
    Invoke-CheckedStep `
        -Name "Packaged tray/start-minimized smoke" `
        -WorkingDirectory $ProjectRootPath `
        -Script { powershell @packagedSmokeArgs }
    $summary.packagedSmoke = "ran"
}

if ($IncludePortablePackage) {
    $packageOutput = Invoke-CapturedStep `
        -Name "Tauri lite portable package" `
        -WorkingDirectory $ProjectRootPath `
        -Script {
            powershell -ExecutionPolicy Bypass -File scripts\package-portable.ps1 -Flavor lite -SkipBuild
        }
    if ($packageOutput -notmatch "packagePath:\s*(.+)") {
        throw "Could not find portable package path in output"
    }
    $summary.portablePackage = $Matches[1].Trim()
}

if ($IncludeFullPortablePackage) {
    $packageOutput = Invoke-CapturedStep `
        -Name "Tauri full portable package" `
        -WorkingDirectory $ProjectRootPath `
        -Script {
            powershell -ExecutionPolicy Bypass -File scripts\package-portable.ps1 -Flavor full
        }
    if ($packageOutput -notmatch "packagePath:\s*(.+)") {
        throw "Could not find full portable package path in output"
    }
    $summary.fullPortablePackage = $Matches[1].Trim()
}

if (Test-Path $tauriExe) {
    $tauriExeBytes = (Get-Item $tauriExe).Length
    $summary.tauriExeBytes = $tauriExeBytes
    $currentBuildInfo = Assert-ReleaseBuildInfo -ExePath $tauriExe
    if (-not $summary.releaseBuildInfo) {
        $summary.releaseBuildInfo = "$($currentBuildInfo.flavor), sha256 recorded"
    }
    if (-not $liteSizeGatePassed) {
        if ($currentBuildInfo.flavor -eq "full") {
            $summary.liteSizeGate = "skipped (current release exe is full)"
        } elseif ($currentBuildInfo.flavor -eq "lite") {
            Assert-LiteExeSize `
                -TauriBytes $tauriExeBytes `
                -PythonBytes $pythonExeBytes `
                -MaxBytes $MaxTauriLiteExeBytes `
                -MaxRatio $MaxTauriToPythonExeRatio
            $summary.liteExeBytes = $tauriExeBytes
            $summary.liteSizeGate = "passed"
        } else {
            throw "release build info has unknown flavor '$($currentBuildInfo.flavor)'"
        }
    }
}

Write-Host ""
Write-Host "==> Migration verification summary"
$summary.GetEnumerator() | ForEach-Object {
    Write-Host ("{0}: {1}" -f $_.Key, $_.Value)
}
