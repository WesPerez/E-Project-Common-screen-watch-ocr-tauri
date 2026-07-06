param(
    [string]$PythonProject = "",
    [int]$MinimumPythonTests = 98,
    [int]$MinimumRustCoreTests = 117,
    [int]$MinimumTauriTests = 82,
    [int]$MinimumOcrFeatureTests = 23,
    [int]$MinimumFrontendTests = 89,
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
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $ExePath).Hash.ToLowerInvariant()
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
        "template:benchmark" = "powershell -ExecutionPolicy Bypass -File scripts/template-benchmark.ps1"
        "template:parity" = "powershell -ExecutionPolicy Bypass -File scripts/template-parity-benchmark.ps1"
        "production:template:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/production-template-performance-smoke.ps1"
        "packaged:smoke" = "powershell -ExecutionPolicy Bypass -File scripts/packaged-smoke.ps1"
        "manual:evidence" = "powershell -ExecutionPolicy Bypass -File scripts/manual-gate-evidence.ps1"
        "webview:visual:smoke" = "node scripts/webview-visual-smoke.mjs"
        "webview:monitoring:smoke" = "node scripts/webview-visual-smoke.mjs --gate monitoring"
        "webview:monitoring:soak" = "node scripts/webview-visual-smoke.mjs --gate monitoring-soak"
        "webview:legacy-profile:smoke" = "node scripts/webview-visual-smoke.mjs --gate legacy-profile"
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
            "scripts\template-benchmark.ps1",
            "scripts\template-parity-benchmark.ps1",
            "scripts\production-template-performance-smoke.ps1",
            "scripts\packaged-smoke.ps1",
            "scripts\manual-gate-evidence.ps1",
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
    $packagedSmokeSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\packaged-smoke.ps1") -Raw
    $webviewSmokeSource = Get-Content -LiteralPath (Join-Path $ProjectRootPath "scripts\webview-visual-smoke.mjs") -Raw

    if ($packageJson.name -ne "screen-watch-ocr-tauri") {
        throw "Tauri package.json name must stay distinct from the Python app"
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
            "## Legacy Profile End-to-End Smoke",
            "## Profile Monitoring Restart Smoke",
            "## Profile Monitoring Soak Smoke",
            "## WebView Layout Resize Smoke",
            "## Packaged App Smoke",
            "## Packaged Tray Menu And Icon Smoke",
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
            'npm run tauri:dev',
            'npm run webview:visual:smoke -- --gate source',
            'npm run webview:visual:smoke -- --gate gallery',
            'npm run webview:clipboard:smoke',
            'npm run webview:scan:smoke',
            'npm run webview:legacy-profile:smoke',
            'npm run webview:monitoring:smoke',
            'npm run webview:monitoring:soak',
            'npm run webview:monitoring:soak -- --soak-ms 30000',
            'npm run webview:layout:smoke',
            'npm run tauri:build:lite',
            'npm run tauri:build:full',
            'npm run tray:smoke -- -ExePath target\release\screen-watch-ocr-tauri.exe',
            'npm run manual:evidence -- -New',
            'npm run manual:evidence -- -Status',
            'npm run manual:evidence',
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
            'Python-shaped profile_1.json',
            'start/stop/restart monitoring',
            'target/settings splitter',
            'packagedSmokeVerified: True',
            'real system tray menu',
            'target\release\bundle',
            'Release exe build-info hash',
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
    frontendCommandContract = $null
    frontendCommandArgumentContract = $null
    frontendDomContract = $null
    frontendActionBindingContract = $null
    frontendDynamicTargetContract = $null
    frontendOcrReadinessContract = $null
    frontendSourcePreviewContract = $null
    backendCommandContract = $null
    monitorSessionEventContract = $null
    trayMonitoringStatusContract = $null
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
