param(
    [string]$EvidenceDir = "",
    [string]$Gate = "",
    [switch]$New,
    [switch]$List,
    [switch]$Status,
    [switch]$AllowNonPass,
    [switch]$Force,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $EvidenceDir) {
    $EvidenceDir = Join-Path $ProjectRootPath "docs\manual-gate-evidence"
}

$RequiredFields = @(
    "Gate",
    "Completion status",
    "Date/time",
    "Machine",
    "Worktree note",
    "Command(s) and exit code(s)",
    "Release build-info hash",
    "Model/image/evidence dirs",
    "Observed result",
    "Evidence files",
    "Cleanup performed",
    "Remaining risk"
)

function Get-ManualGateDefinitions {
    return @(
        [pscustomobject][ordered]@{
            Id = "baseline-before-manual-gates"
            Title = "Baseline Before Manual Gates"
        },
        [pscustomobject][ordered]@{
            Id = "desktop-backend-smoke"
            Title = "Desktop Backend Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "real-ocr-model-smoke"
            Title = "Real OCR Model Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "webview-source-preview-visual-smoke"
            Title = "WebView Source Preview Visual Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "template-gallery-visual-workflow-smoke"
            Title = "Template Gallery Visual Workflow Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "profile-clipboard-paste-smoke"
            Title = "Profile Clipboard Paste Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "profile-one-shot-scan-smoke"
            Title = "Profile One Shot Scan Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "profile-monitoring-restart-smoke"
            Title = "Profile Monitoring Restart Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "webview-layout-resize-smoke"
            Title = "WebView Layout Resize Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "packaged-app-smoke"
            Title = "Packaged App Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "packaged-tray-menu-and-icon-smoke"
            Title = "Packaged Tray Menu And Icon Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "installer-repeatability-smoke"
            Title = "Installer Repeatability Smoke"
        },
        [pscustomobject][ordered]@{
            Id = "production-template-performance-smoke"
            Title = "Production Template Performance Smoke"
        }
    )
}

function Resolve-ManualGate {
    param([string]$Value)

    $gates = Get-ManualGateDefinitions
    if (-not $Value) {
        return $null
    }

    $matches = @(
        $gates | Where-Object {
            $_.Id -eq $Value -or
            $_.Title -eq $Value -or
            $_.Title.Replace(" ", "-").ToLowerInvariant() -eq $Value.ToLowerInvariant()
        }
    )
    if ($matches.Count -ne 1) {
        $ids = ($gates | ForEach-Object { $_.Id }) -join ", "
        throw "Unknown manual gate '$Value'. Expected one of: $ids"
    }
    return $matches[0]
}

function Get-RecordPath {
    param(
        [string]$Directory,
        [object]$GateDefinition
    )

    return Join-Path $Directory "$($GateDefinition.Id).md"
}

function New-RecordText {
    param([object]$GateDefinition)

    return @"
Gate: $($GateDefinition.Title)
Completion status: blocked
Date/time:
Machine:
Worktree note:
Command(s) and exit code(s):
Release build-info hash:
Model/image/evidence dirs:
Observed result:
Evidence files:
Cleanup performed:
Remaining risk:
"@
}

function Write-ManualGateRecord {
    param(
        [string]$Directory,
        [object]$GateDefinition,
        [bool]$Overwrite
    )

    New-Item -ItemType Directory -Path $Directory -Force | Out-Null
    $path = Get-RecordPath $Directory $GateDefinition
    if ((Test-Path -LiteralPath $path) -and -not $Overwrite) {
        throw "Manual gate evidence record already exists: $path. Pass -Force to overwrite."
    }

    New-RecordText $GateDefinition | Set-Content -LiteralPath $path -Encoding UTF8
    Write-Host "created: $path"
}

function Read-RecordFields {
    param([string]$Path)

    $fields = @{}
    Get-Content -LiteralPath $Path | ForEach-Object {
        if ($_ -match "^([^:\r\n]+):\s*(.*)$") {
            $fields[$Matches[1].Trim()] = $Matches[2].Trim()
        }
    }
    return $fields
}

function Assert-NonEmptyField {
    param(
        [string]$Path,
        [hashtable]$Fields,
        [string]$Name
    )

    if (-not $Fields.ContainsKey($Name)) {
        throw "Manual gate evidence record '$Path' is missing field '$Name'"
    }
    if (-not [string]$Fields[$Name]) {
        throw "Manual gate evidence record '$Path' has empty field '$Name'"
    }
}

function Assert-Record {
    param(
        [string]$Path,
        [object]$GateDefinition,
        [bool]$PermitNonPass
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Missing manual gate evidence record: $Path"
    }

    $fields = Read-RecordFields $Path
    foreach ($field in $RequiredFields) {
        Assert-NonEmptyField $Path $fields $field
    }

    if ($fields["Gate"] -ne $GateDefinition.Title) {
        throw "Manual gate evidence record '$Path' is for '$($fields["Gate"])', expected '$($GateDefinition.Title)'"
    }

    $status = $fields["Completion status"].ToLowerInvariant()
    if (@("pass", "fail", "blocked") -notcontains $status) {
        throw "Manual gate evidence record '$Path' has invalid status '$($fields["Completion status"])'"
    }
    if (-not $PermitNonPass -and $status -ne "pass") {
        throw "Manual gate evidence record '$Path' is '$status', expected 'pass'"
    }
}

function Get-RecordStatus {
    param(
        [string]$Path,
        [object]$GateDefinition
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return [pscustomobject][ordered]@{
            Id = $GateDefinition.Id
            Gate = $GateDefinition.Title
            Status = "missing"
            Record = $Path
            Detail = "record file does not exist"
        }
    }

    $fields = Read-RecordFields $Path
    $missingFields = @(
        $RequiredFields |
            Where-Object { -not $fields.ContainsKey($_) -or -not [string]$fields[$_] }
    )
    if ($missingFields.Count -gt 0) {
        return [pscustomobject][ordered]@{
            Id = $GateDefinition.Id
            Gate = $GateDefinition.Title
            Status = "incomplete"
            Record = $Path
            Detail = "missing/empty fields: $($missingFields -join ', ')"
        }
    }

    if ($fields["Gate"] -ne $GateDefinition.Title) {
        return [pscustomobject][ordered]@{
            Id = $GateDefinition.Id
            Gate = $GateDefinition.Title
            Status = "invalid"
            Record = $Path
            Detail = "record gate '$($fields["Gate"])' does not match"
        }
    }

    $status = $fields["Completion status"].ToLowerInvariant()
    if (@("pass", "fail", "blocked") -notcontains $status) {
        return [pscustomobject][ordered]@{
            Id = $GateDefinition.Id
            Gate = $GateDefinition.Title
            Status = "invalid"
            Record = $Path
            Detail = "invalid completion status '$($fields["Completion status"])'"
        }
    }

    return [pscustomobject][ordered]@{
        Id = $GateDefinition.Id
        Gate = $GateDefinition.Title
        Status = $status
        Record = $Path
        Detail = $fields["Observed result"]
    }
}

function Invoke-Status {
    param(
        [string]$Directory,
        [string]$OnlyGate
    )

    $gates = if ($OnlyGate) {
        @(Resolve-ManualGate $OnlyGate)
    } else {
        Get-ManualGateDefinitions
    }

    $records = @(
        foreach ($gateDefinition in $gates) {
            Get-RecordStatus (Get-RecordPath $Directory $gateDefinition) $gateDefinition
        }
    )

    $records |
        Select-Object Id, Status, Detail |
        Format-Table -AutoSize |
        Out-String |
        Write-Host

    $counts = @{}
    foreach ($record in $records) {
        if (-not $counts.ContainsKey($record.Status)) {
            $counts[$record.Status] = 0
        }
        $counts[$record.Status] += 1
    }

    $summaryParts = @(
        "pass",
        "blocked",
        "fail",
        "missing",
        "incomplete",
        "invalid"
    ) | ForEach-Object {
        "$_=$([int]$counts[$_])"
    }
    Write-Host "manualGateEvidenceStatus: $($summaryParts -join ', ')"
}

function Invoke-Validate {
    param(
        [string]$Directory,
        [bool]$PermitNonPass,
        [string]$OnlyGate
    )

    $gates = if ($OnlyGate) {
        @(Resolve-ManualGate $OnlyGate)
    } else {
        Get-ManualGateDefinitions
    }

    foreach ($gateDefinition in $gates) {
        Assert-Record (Get-RecordPath $Directory $gateDefinition) $gateDefinition $PermitNonPass
    }

    Write-Host "manualGateEvidence: validated $(@($gates).Count) record(s)"
}

function Invoke-SelfTest {
    $tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("screen-watch-manual-gate-evidence-" + [Guid]::NewGuid().ToString("N"))
    try {
        foreach ($gateDefinition in Get-ManualGateDefinitions) {
            Write-ManualGateRecord $tempRoot $gateDefinition $true | Out-Null
            $path = Get-RecordPath $tempRoot $gateDefinition
            $text = Get-Content -LiteralPath $path -Raw
            $text = $text -replace "(?m)^Completion status:\s*blocked\s*$", "Completion status: pass"
            $text = $text -replace "(?m)^Date/time:\s*$", "Date/time: 2026-07-06T00:00:00Z"
            $text = $text -replace "(?m)^Machine:\s*$", "Machine: self-test"
            $text = $text -replace "(?m)^Worktree note:\s*$", "Worktree note: self-test"
            $text = $text -replace "(?m)^Command\(s\) and exit code\(s\):\s*$", "Command(s) and exit code(s): self-test command exited 0"
            $text = $text -replace "(?m)^Release build-info hash:\s*$", "Release build-info hash: n/a"
            $text = $text -replace "(?m)^Model/image/evidence dirs:\s*$", "Model/image/evidence dirs: n/a"
            $text = $text -replace "(?m)^Observed result:\s*$", "Observed result: self-test pass"
            $text = $text -replace "(?m)^Evidence files:\s*$", "Evidence files: inline self-test record"
            $text = $text -replace "(?m)^Cleanup performed:\s*$", "Cleanup performed: temporary self-test directory removed"
            $text = $text -replace "(?m)^Remaining risk:\s*$", "Remaining risk: none"
            Set-Content -LiteralPath $path -Value $text -Encoding UTF8
        }

        Invoke-Validate $tempRoot $false "" | Out-Null
        Invoke-Status $tempRoot "" | Out-Null

        $firstGate = (Get-ManualGateDefinitions)[0]
        $firstPath = Get-RecordPath $tempRoot $firstGate
        $firstText = Get-Content -LiteralPath $firstPath -Raw
        $firstText = $firstText.Replace("Completion status: pass", "Completion status: blocked")
        Set-Content -LiteralPath $firstPath -Value $firstText -Encoding UTF8

        $failedAsExpected = $false
        try {
            Invoke-Validate $tempRoot $false "" | Out-Null
        } catch {
            $failedAsExpected = $true
        }
        if (-not $failedAsExpected) {
            throw "Manual gate evidence self-test expected pass-only validation to fail on blocked status"
        }

        Invoke-Validate $tempRoot $true "" | Out-Null
        Write-Host "manualGateEvidenceSelfTest: passed"
    } finally {
        if (Test-Path -LiteralPath $tempRoot) {
            Remove-Item -LiteralPath $tempRoot -Recurse -Force
        }
    }
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if ($List) {
    Get-ManualGateDefinitions | ForEach-Object {
        Write-Host "$($_.Id) - $($_.Title)"
    }
    exit 0
}

if ($Status) {
    Invoke-Status $EvidenceDir $Gate
    exit 0
}

if ($New) {
    $gates = if ($Gate) {
        @(Resolve-ManualGate $Gate)
    } else {
        Get-ManualGateDefinitions
    }
    foreach ($gateDefinition in $gates) {
        Write-ManualGateRecord $EvidenceDir $gateDefinition ([bool]$Force)
    }
    exit 0
}

Invoke-Validate $EvidenceDir ([bool]$AllowNonPass) $Gate
