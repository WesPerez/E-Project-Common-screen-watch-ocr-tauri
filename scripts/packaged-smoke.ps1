param(
    [string]$ExePath = "",
    [int]$StartupWaitSeconds = 18,
    [int]$CloseDelayMilliseconds = 750
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $ExePath) {
    $ExePath = Join-Path $ProjectRootPath "target\release\screen-watch-ocr-tauri.exe"
}
$ExePath = (Resolve-Path $ExePath).Path

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class ScreenWatchWindowProbe
{
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    public sealed class WindowInfo
    {
        public Int64 Hwnd;
        public string ClassName;
        public string Title;
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
        public int Width;
        public int Height;
        public string Summary;
    }

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int maxCount);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr hWnd, System.Text.StringBuilder text, int maxCount);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr hWnd, UInt32 msg, IntPtr wParam, IntPtr lParam);

    private const UInt32 WM_CLOSE = 0x0010;

    public static string[] VisibleTopLevelWindowSummariesForProcess(int processId)
    {
        List<string> summaries = new List<string>();
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            uint ownerPid;
            GetWindowThreadProcessId(hWnd, out ownerPid);
            if (ownerPid == processId && IsWindowVisible(hWnd))
            {
                summaries.Add(WindowSummary(hWnd));
            }
            return true;
        }, IntPtr.Zero);
        return summaries.ToArray();
    }

    public static string[] VisibleMainWindowSummariesForProcess(int processId, string expectedTitle)
    {
        List<string> summaries = new List<string>();
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            uint ownerPid;
            GetWindowThreadProcessId(hWnd, out ownerPid);
            if (ownerPid == processId && IsWindowVisible(hWnd))
            {
                string title = WindowTitle(hWnd);
                string className = WindowClassName(hWnd);
                if (title == expectedTitle || className == "Tauri Window")
                {
                    summaries.Add(WindowSummary(hWnd));
                }
            }
            return true;
        }, IntPtr.Zero);
        return summaries.ToArray();
    }

    public static WindowInfo[] VisibleMainWindowInfosForProcess(int processId, string expectedTitle)
    {
        List<WindowInfo> windows = new List<WindowInfo>();
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            uint ownerPid;
            GetWindowThreadProcessId(hWnd, out ownerPid);
            if (ownerPid == processId && IsWindowVisible(hWnd) && IsMainWindow(hWnd, expectedTitle))
            {
                windows.Add(WindowInfoFor(hWnd));
            }
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }

    public static bool CloseFirstVisibleMainWindowForProcess(int processId, string expectedTitle)
    {
        if (CloseFirstVisibleWindowForProcess(processId, expectedTitle, true))
        {
            return true;
        }
        return CloseFirstVisibleWindowForProcess(processId, expectedTitle, false);
    }

    private static bool CloseFirstVisibleWindowForProcess(int processId, string expectedTitle, bool exactTitleOnly)
    {
        bool posted = false;
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            if (posted)
            {
                return false;
            }

            uint ownerPid;
            GetWindowThreadProcessId(hWnd, out ownerPid);
            bool matches = exactTitleOnly
                ? WindowTitle(hWnd) == expectedTitle
                : IsMainWindow(hWnd, expectedTitle);
            if (ownerPid == processId && IsWindowVisible(hWnd) && matches)
            {
                posted = PostMessage(hWnd, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return posted;
    }

    private static string WindowSummary(IntPtr hWnd)
    {
        return WindowInfoFor(hWnd).Summary;
    }

    private static WindowInfo WindowInfoFor(IntPtr hWnd)
    {
        RECT rect;
        GetWindowRect(hWnd, out rect);
        string className = WindowClassName(hWnd);
        string title = WindowTitle(hWnd);
        WindowInfo info = new WindowInfo();
        info.Hwnd = hWnd.ToInt64();
        info.ClassName = className;
        info.Title = title;
        info.Left = rect.Left;
        info.Top = rect.Top;
        info.Right = rect.Right;
        info.Bottom = rect.Bottom;
        info.Width = rect.Right - rect.Left;
        info.Height = rect.Bottom - rect.Top;
        info.Summary = String.Format(
            "0x{0:x} class='{1}' title='{2}' rect={3},{4},{5},{6}",
            info.Hwnd,
            className,
            title,
            rect.Left,
            rect.Top,
            rect.Right,
            rect.Bottom
        );
        return info;
    }

    private static string WindowTitle(IntPtr hWnd)
    {
        System.Text.StringBuilder text = new System.Text.StringBuilder(512);
        GetWindowText(hWnd, text, text.Capacity);
        return text.ToString();
    }

    private static string WindowClassName(IntPtr hWnd)
    {
        System.Text.StringBuilder text = new System.Text.StringBuilder(256);
        GetClassName(hWnd, text, text.Capacity);
        return text.ToString();
    }

    private static bool IsMainWindow(IntPtr hWnd, string expectedTitle)
    {
        string title = WindowTitle(hWnd);
        string className = WindowClassName(hWnd);
        return title == expectedTitle || className == "Tauri Window";
    }
}
"@

function New-FreeLoopbackPort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Parse("127.0.0.1"), 0)
    $listener.Start()
    try {
        return ([Net.IPEndPoint]$listener.LocalEndpoint).Port
    } finally {
        $listener.Stop()
    }
}

function Test-TcpPortBusy {
    param([int]$Port)
    $client = [Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync("127.0.0.1", $Port)
        if (-not $task.Wait(200)) {
            return $false
        }
        return $client.Connected
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Start-SmokeProcess {
    param(
        [string]$ExePath,
        [string]$LocalAppData,
        [int]$InstancePort,
        [string]$Arguments = ""
    )
    $psi = [Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $ExePath
    $psi.Arguments = $Arguments
    $psi.WorkingDirectory = Split-Path -Parent $ExePath
    $psi.UseShellExecute = $false
    $psi.EnvironmentVariables["LOCALAPPDATA"] = $LocalAppData
    $psi.EnvironmentVariables["SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT"] = [string]$InstancePort
    return [Diagnostics.Process]::Start($psi)
}

function Write-SmokeTextFile {
    param(
        [string]$Path,
        [string]$Value
    )

    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}

function Initialize-LegacyAppDataFixture {
    param(
        [string]$AppRoot,
        [string]$SourceExePath
    )

    New-Item -ItemType Directory -Path $AppRoot | Out-Null
    $stagedExePath = Join-Path $AppRoot "screen-watch-ocr-tauri.exe"
    Copy-Item -LiteralPath $SourceExePath -Destination $stagedExePath

    $legacyDir = Join-Path $AppRoot "app_data"
    New-Item -ItemType Directory -Force -Path (Join-Path $legacyDir "profiles") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $legacyDir "templates") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $legacyDir "alerts") | Out-Null
    Write-SmokeTextFile `
        -Path (Join-Path $legacyDir "profiles\profile_1.json") `
        -Value '{"targets":[],"legacySmokeProfile":true}'
    Write-SmokeTextFile `
        -Path (Join-Path $legacyDir "templates\legacy-smoke.png") `
        -Value "legacy-template"
    Write-SmokeTextFile `
        -Path (Join-Path $legacyDir "state.json") `
        -Value '{"last_profile":1,"layout":{"geometry":"980x680+20+30"},"legacySmokeState":true}'
    Write-SmokeTextFile `
        -Path (Join-Path $legacyDir "alerts.jsonl") `
        -Value '{"legacySmokeAlert":true}'
    Write-SmokeTextFile `
        -Path (Join-Path $legacyDir "alerts\legacy-alert.png") `
        -Value "legacy-alert"

    return [pscustomobject]@{
        ExePath = $stagedExePath
        LegacyDir = $legacyDir
    }
}

function Assert-LegacyAppDataMigrated {
    param(
        [string]$LegacyDir,
        [string]$SharedDataDir
    )

    $checks = @(
        "profiles\profile_1.json",
        "templates\legacy-smoke.png",
        "state.json",
        "alerts.jsonl",
        "screenshots\legacy-alert.png"
    )
    foreach ($relative in $checks) {
        $path = Join-Path $SharedDataDir $relative
        if (-not (Test-Path -LiteralPath $path)) {
            throw "legacy app_data migration did not create $relative under $SharedDataDir"
        }
    }
    $legacyProfile = Join-Path $LegacyDir "profiles\profile_1.json"
    if (-not (Test-Path -LiteralPath $legacyProfile)) {
        throw "legacy app_data migration should not delete the source profile"
    }
    $profileText = Get-Content -LiteralPath (Join-Path $SharedDataDir "profiles\profile_1.json") -Raw
    if ($profileText -notmatch "legacySmokeProfile") {
        throw "legacy profile contents were not copied into the shared data directory"
    }
    $stateText = Get-Content -LiteralPath (Join-Path $SharedDataDir "state.json") -Raw
    if ($stateText -notmatch "legacySmokeState") {
        throw "legacy state contents were not copied into the shared data directory"
    }
}

function Assert-ProcessRunning {
    param(
        [Diagnostics.Process]$Process,
        [string]$Scenario
    )
    $Process.Refresh()
    if ($Process.HasExited) {
        throw "$Scenario packaged app exited with code $($Process.ExitCode)"
    }
}

function Get-VisibleMainWindows {
    param([Diagnostics.Process]$Process)
    return [ScreenWatchWindowProbe]::VisibleMainWindowSummariesForProcess($Process.Id, "Screen Watch OCR Tauri")
}

function Get-VisibleMainWindowInfos {
    param([Diagnostics.Process]$Process)
    return [ScreenWatchWindowProbe]::VisibleMainWindowInfosForProcess($Process.Id, "Screen Watch OCR Tauri")
}

function Assert-MainWindowRestoredLegacyGeometry {
    param(
        [object]$WindowInfo,
        [int]$ExpectedLeft = 20,
        [int]$ExpectedTop = 30,
        [int]$ExpectedWidth = 980,
        [int]$ExpectedHeight = 680,
        [int]$PositionTolerance = 32,
        [int]$SizeTolerance = 96
    )

    if (-not $WindowInfo) {
        throw "legacy geometry restore could not inspect the main window"
    }

    $scales = @(1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0)
    foreach ($scale in $scales) {
        $scaledLeft = [Math]::Round($ExpectedLeft / $scale)
        $scaledTop = [Math]::Round($ExpectedTop / $scale)
        $scaledWidth = [Math]::Round($ExpectedWidth / $scale)
        $scaledHeight = [Math]::Round($ExpectedHeight / $scale)
        $leftDelta = [Math]::Abs([int]$WindowInfo.Left - $scaledLeft)
        $topDelta = [Math]::Abs([int]$WindowInfo.Top - $scaledTop)
        $widthDelta = [Math]::Abs([int]$WindowInfo.Width - $scaledWidth)
        $heightDelta = [Math]::Abs([int]$WindowInfo.Height - $scaledHeight)
        if (
            $leftDelta -le $PositionTolerance -and
            $topDelta -le $PositionTolerance -and
            $widthDelta -le $SizeTolerance -and
            $heightDelta -le $SizeTolerance
        ) {
            return $scale
        }
    }

    throw (
        "legacy geometry restore mismatch: expected about " +
        "${ExpectedWidth}x${ExpectedHeight}+${ExpectedLeft}+${ExpectedTop} " +
        "or a DPI-virtualized equivalent, got " +
        "$($WindowInfo.Width)x$($WindowInfo.Height)+$($WindowInfo.Left)+$($WindowInfo.Top) " +
        "from $($WindowInfo.Summary)"
    )
}

function Wait-ForSmokeCondition {
    param(
        [string]$Description,
        [scriptblock]$Condition,
        [int]$TimeoutSeconds = 6
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (& $Condition) {
            return
        }
        Start-Sleep -Milliseconds 100
    }
    throw "Timed out waiting for $Description"
}

function Wait-ForProcessExit {
    param(
        [Diagnostics.Process]$Process,
        [string]$Scenario,
        [int]$TimeoutSeconds = 6
    )
    if (-not $Process.WaitForExit($TimeoutSeconds * 1000)) {
        throw "$Scenario process did not exit within $TimeoutSeconds seconds"
    }
    $Process.Refresh()
    if ($Process.ExitCode -ne 0) {
        throw "$Scenario process exited with code $($Process.ExitCode)"
    }
}

function Stop-SmokeProcess {
    param([Diagnostics.Process]$Process)
    if (-not $Process) {
        return $false
    }
    $Process.Refresh()
    if ($Process.HasExited) {
        return $false
    }
    & taskkill.exe /PID $Process.Id /T /F | Out-Null
    $Process.WaitForExit(5000) | Out-Null
    return $true
}

function Get-PeSubsystem {
    param([string]$Path)

    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        $reader = [IO.BinaryReader]::new($stream)
        $stream.Seek(0x3c, [IO.SeekOrigin]::Begin) | Out-Null
        $peOffset = $reader.ReadInt32()
        if ($peOffset -le 0 -or $peOffset -gt ($stream.Length - 96)) {
            throw "invalid PE header offset $peOffset"
        }
        $stream.Seek($peOffset, [IO.SeekOrigin]::Begin) | Out-Null
        $signature = $reader.ReadUInt32()
        if ($signature -ne 0x00004550) {
            throw "invalid PE signature 0x$($signature.ToString('X8'))"
        }
        $optionalHeaderOffset = $peOffset + 24
        $stream.Seek($optionalHeaderOffset, [IO.SeekOrigin]::Begin) | Out-Null
        $magic = $reader.ReadUInt16()
        if ($magic -ne 0x10b -and $magic -ne 0x20b) {
            throw "unsupported PE optional-header magic 0x$($magic.ToString('X4'))"
        }
        $stream.Seek($optionalHeaderOffset + 68, [IO.SeekOrigin]::Begin) | Out-Null
        return [int]$reader.ReadUInt16()
    } finally {
        $stream.Dispose()
    }
}

function Get-PeSubsystemName {
    param([int]$Subsystem)

    switch ($Subsystem) {
        2 { return "WindowsGui" }
        3 { return "WindowsConsole" }
        default { return "Other($Subsystem)" }
    }
}

function Assert-WindowsGuiSubsystem {
    param(
        [string]$Path,
        [string]$Label
    )

    $subsystem = Get-PeSubsystem $Path
    $name = Get-PeSubsystemName $subsystem
    if ($subsystem -ne 2) {
        throw "$Label must be a Windows GUI subsystem executable, got $name ($subsystem). A console subsystem build can show an unwanted console window."
    }
    return [pscustomobject][ordered]@{
        Path = $Path
        Subsystem = $subsystem
        Name = $name
    }
}

$smokeId = ([guid]::NewGuid().ToString("N")).Substring(0, 8)
$smokeAppRoot = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-tauri-packaged-smoke-app-$smokeId"
$smokeLocalAppData = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-tauri-packaged-smoke-$smokeId"
$sharedDataDir = Join-Path $smokeLocalAppData "ScreenWatchOCR"
$startMinimizedPort = New-FreeLoopbackPort
$closeToTrayPort = New-FreeLoopbackPort
$process = $null
$closeProcess = $null
$secondInstanceProcess = $null
$stoppedProcess = $false
$stoppedCloseProcess = $false
$stoppedSecondInstanceProcess = $false
$removedAppData = $false
$removedAppRoot = $false
$fixture = $null

New-Item -ItemType Directory -Path $smokeLocalAppData | Out-Null
try {
    $sourceSubsystem = Assert-WindowsGuiSubsystem -Path $ExePath -Label "source exe"
    $fixture = Initialize-LegacyAppDataFixture -AppRoot $smokeAppRoot -SourceExePath $ExePath
    $stagedSubsystem = Assert-WindowsGuiSubsystem -Path $fixture.ExePath -Label "staged smoke exe"
    $defaultPortBusy = Test-TcpPortBusy 47628
    $process = Start-SmokeProcess `
        -ExePath $fixture.ExePath `
        -LocalAppData $smokeLocalAppData `
        -InstancePort $startMinimizedPort `
        -Arguments "--start-minimized"

    Start-Sleep -Seconds $StartupWaitSeconds
    Assert-ProcessRunning $process "start-minimized"
    Assert-LegacyAppDataMigrated -LegacyDir $fixture.LegacyDir -SharedDataDir $sharedDataDir

    $visibleTopLevelWindows = [ScreenWatchWindowProbe]::VisibleTopLevelWindowSummariesForProcess($process.Id)
    $visibleMainWindows = [ScreenWatchWindowProbe]::VisibleMainWindowSummariesForProcess($process.Id, "Screen Watch OCR Tauri")
    if ($visibleMainWindows.Count -gt 0) {
        $handles = $visibleMainWindows -join "; "
        throw "start-minimized app still has visible main window(s): $handles"
    }

    Write-Host "exePath: $($fixture.ExePath)"
    Write-Host "sourceExePath: $ExePath"
    Write-Host "sourceExePeSubsystem: $($sourceSubsystem.Name) ($($sourceSubsystem.Subsystem))"
    Write-Host "stagedExePeSubsystem: $($stagedSubsystem.Name) ($($stagedSubsystem.Subsystem))"
    Write-Host "smokeAppRoot: $smokeAppRoot"
    Write-Host "defaultInstancePortBusy: $defaultPortBusy"
    Write-Host "isolatedStartMinimizedPort: $startMinimizedPort"
    Write-Host "smokeLocalAppData: $smokeLocalAppData"
    Write-Host "sharedDataDir: $sharedDataDir"
    Write-Host "legacyAppDataDir: $($fixture.LegacyDir)"
    Write-Host "processId: $($process.Id)"
    Write-Host "visibleTopLevelWindows: $($visibleTopLevelWindows.Count)"
    Write-Host "visibleMainWindows: $($visibleMainWindows.Count)"
    Write-Host "processRunning: True"
    Write-Host "legacyMigrationSmokeVerified: True"
    Write-Host "startMinimizedSmokeVerified: True"
    Write-Host "windowsGuiSubsystemSmokeVerified: True"

    $stoppedProcess = Stop-SmokeProcess $process
    $process = $null

    $closeProcess = Start-SmokeProcess `
        -ExePath $fixture.ExePath `
        -LocalAppData $smokeLocalAppData `
        -InstancePort $closeToTrayPort

    Wait-ForSmokeCondition "normal startup main window visibility" {
        Assert-ProcessRunning $closeProcess "close-to-tray"
        (Get-VisibleMainWindows $closeProcess).Count -gt 0
    } $StartupWaitSeconds
    $initialMainWindows = Get-VisibleMainWindows $closeProcess
    $initialMainWindowInfos = Get-VisibleMainWindowInfos $closeProcess
    $legacyGeometryScale = Assert-MainWindowRestoredLegacyGeometry $initialMainWindowInfos[0]

    if ($CloseDelayMilliseconds -gt 0) {
        Start-Sleep -Milliseconds $CloseDelayMilliseconds
    }
    if (-not [ScreenWatchWindowProbe]::CloseFirstVisibleMainWindowForProcess($closeProcess.Id, "Screen Watch OCR Tauri")) {
        throw "could not post WM_CLOSE to the packaged app main window"
    }
    try {
        Wait-ForSmokeCondition "close-to-tray main window hiding" {
            Assert-ProcessRunning $closeProcess "close-to-tray"
            (Get-VisibleMainWindows $closeProcess).Count -eq 0
        } $StartupWaitSeconds
    } catch {
        $remainingMainWindows = Get-VisibleMainWindows $closeProcess
        $remainingTopLevelWindows = [ScreenWatchWindowProbe]::VisibleTopLevelWindowSummariesForProcess($closeProcess.Id)
        throw "$($_.Exception.Message); remaining main windows: $($remainingMainWindows -join '; '); remaining top-level windows: $($remainingTopLevelWindows -join '; ')"
    }
    $afterCloseMainWindows = Get-VisibleMainWindows $closeProcess

    Start-Sleep -Milliseconds 500
    $secondInstanceProcess = Start-SmokeProcess `
        -ExePath $fixture.ExePath `
        -LocalAppData $smokeLocalAppData `
        -InstancePort $closeToTrayPort
    $secondInstanceProcessId = $secondInstanceProcess.Id
    Wait-ForProcessExit $secondInstanceProcess "second instance wake" $StartupWaitSeconds
    $secondInstanceExitCode = $secondInstanceProcess.ExitCode
    Wait-ForSmokeCondition "single-instance wake showing main window" {
        Assert-ProcessRunning $closeProcess "single-instance wake"
        (Get-VisibleMainWindows $closeProcess).Count -gt 0
    } $StartupWaitSeconds
    $afterWakeMainWindows = Get-VisibleMainWindows $closeProcess

    Write-Host "closeToTrayInitialVisibleMainWindows: $($initialMainWindows.Count)"
    Write-Host "legacyGeometryMainWindowRect: $($initialMainWindowInfos[0].Width)x$($initialMainWindowInfos[0].Height)+$($initialMainWindowInfos[0].Left)+$($initialMainWindowInfos[0].Top)"
    Write-Host "legacyGeometryProbeScale: $legacyGeometryScale"
    Write-Host "legacyGeometryRestoreSmokeVerified: True"
    Write-Host "closeDelayMilliseconds: $CloseDelayMilliseconds"
    Write-Host "isolatedCloseToTrayPort: $closeToTrayPort"
    Write-Host "closeToTrayAfterCloseVisibleMainWindows: $($afterCloseMainWindows.Count)"
    Write-Host "secondInstanceProcessId: $secondInstanceProcessId"
    Write-Host "secondInstanceExitCode: $secondInstanceExitCode"
    Write-Host "secondInstanceExited: True"
    Write-Host "singleInstanceWakeVisibleMainWindows: $($afterWakeMainWindows.Count)"
    Write-Host "closeToTraySmokeVerified: True"
    Write-Host "packagedSmokeVerified: True"
} finally {
    $stoppedProcess = $stoppedProcess -or (Stop-SmokeProcess $process)
    $stoppedCloseProcess = $stoppedCloseProcess -or (Stop-SmokeProcess $closeProcess)
    $stoppedSecondInstanceProcess = $stoppedSecondInstanceProcess -or (Stop-SmokeProcess $secondInstanceProcess)
    if (Test-Path -LiteralPath $smokeLocalAppData) {
        Remove-Item -LiteralPath $smokeLocalAppData -Recurse -Force
        $removedAppData = $true
    }
    if (Test-Path -LiteralPath $smokeAppRoot) {
        Remove-Item -LiteralPath $smokeAppRoot -Recurse -Force
        $removedAppRoot = $true
    }
    Write-Host "cleanupStoppedProcess: $stoppedProcess"
    Write-Host "cleanupStoppedCloseToTrayProcess: $stoppedCloseProcess"
    Write-Host "cleanupStoppedSecondInstanceProcess: $stoppedSecondInstanceProcess"
    Write-Host "cleanupRemovedAppData: $removedAppData"
    Write-Host "cleanupRemovedAppRoot: $removedAppRoot"
}
