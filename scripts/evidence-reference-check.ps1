param(
    [string]$ProjectRoot = ""
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $ProjectRoot) {
    $ProjectRoot = Join-Path $ScriptRoot ".."
}
$ProjectRootPath = (Resolve-Path $ProjectRoot).Path

$docFiles = @()
$verificationDocs = Get-ChildItem -Path $ProjectRootPath -Filter "docs\VERIFICATION_RUN_*.md" -File -ErrorAction SilentlyContinue
$manualEvidenceDir = Join-Path $ProjectRootPath "docs\manual-gate-evidence"
if (Test-Path -LiteralPath $manualEvidenceDir) {
    $docFiles += Get-ChildItem -Path $manualEvidenceDir -Filter "*.md" -File
}
$docFiles += $verificationDocs

$patterns = @(
    "docs\\manual-gate-evidence\\logs\\[^`\s;,]+",
    "target\\[^`\s;,]+",
    "release-single\\ScreenWatchOCRTauri\.exe"
)

$missing = @()
foreach ($doc in $docFiles) {
    $text = Get-Content -LiteralPath $doc.FullName -Raw
    foreach ($pattern in $patterns) {
        foreach ($match in [regex]::Matches($text, $pattern)) {
            $reference = $match.Value.TrimEnd(".", ",", ";", ")", ":", [char]0x60)

            # Historical installer smoke install roots are summarized in the
            # evidence record but intentionally not required as live artifacts.
            if ($reference -match "^target\\installer-smoke") {
                continue
            }

            $path = Join-Path $ProjectRootPath $reference
            if (-not (Test-Path -LiteralPath $path)) {
                $missing += [pscustomobject]@{
                    Doc = $doc.Name
                    Reference = $reference
                    Path = $path
                }
            }
        }
    }
}

if ($missing.Count -gt 0) {
    $missing |
        Sort-Object Doc, Reference -Unique |
        Format-Table -AutoSize |
        Out-String |
        Write-Host
    throw "evidenceReferenceCheck found $($missing.Count) missing local reference(s)"
}

Write-Host "evidenceReferenceCheck: all parsed current local references exist"
