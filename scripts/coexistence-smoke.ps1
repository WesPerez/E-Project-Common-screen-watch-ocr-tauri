param(
    [string]$PythonExePath = "",
    [string]$TauriExePath = "",
    [string]$ResultPath = "",
    [int]$StartupWaitSeconds = 24
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $PythonExePath) {
    $PythonExePath = Join-Path $ProjectRootPath "..\screen-watch-ocr\dist\ScreenWatchOCR.exe"
}
if (-not $TauriExePath) {
    $TauriExePath = Join-Path $ProjectRootPath "release-single\ScreenWatchOCRTauri.exe"
}
if (-not $ResultPath) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $ResultPath = Join-Path $ProjectRootPath "docs\manual-gate-evidence\logs\coexistence-smoke-$stamp-result.json"
}

$PythonExePath = (Resolve-Path $PythonExePath).Path
$TauriExePath = (Resolve-Path $TauriExePath).Path
$ResultPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ResultPath)

$PythonPort = 47627
$TauriPort = 47628
$PythonCommand = [Text.Encoding]::ASCII.GetBytes("ScreenWatchOCR:show`n")
$TauriCommand = [Text.Encoding]::ASCII.GetBytes("ScreenWatchOCRTauri:show`n")
$Ack = [Text.Encoding]::ASCII.GetBytes("ok`n")

if (-not ([System.Management.Automation.PSTypeName]"ScreenWatchCoexistenceSmokeNative").Type) {
    Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class ScreenWatchCoexistenceSmokeNative
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

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr hWnd, StringBuilder text, int maxCount);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    public static object[] VisibleWindowsForPid(uint wantedPid)
    {
        var windows = new List<object>();
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == wantedPid && IsWindowVisible(hWnd))
            {
                RECT rect;
                GetWindowRect(hWnd, out rect);
                windows.Add(new
                {
                    Hwnd = hWnd.ToInt64(),
                    Pid = pid,
                    Class = ClassName(hWnd),
                    Title = Title(hWnd),
                    Left = rect.Left,
                    Top = rect.Top,
                    Right = rect.Right,
                    Bottom = rect.Bottom,
                    Width = rect.Right - rect.Left,
                    Height = rect.Bottom - rect.Top
                });
            }
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }

    private static string Title(IntPtr hWnd)
    {
        var text = new StringBuilder(512);
        GetWindowText(hWnd, text, text.Capacity);
        return text.ToString();
    }

    private static string ClassName(IntPtr hWnd)
    {
        var text = new StringBuilder(256);
        GetClassName(hWnd, text, text.Capacity);
        return text.ToString();
    }
}
"@
}

function Get-ExeSha256 {
    param([string]$Path)
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToUpperInvariant()
}

function Get-PeSubsystem {
    param([string]$Path)

    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        $reader = [IO.BinaryReader]::new($stream)
        $stream.Seek(0x3c, [IO.SeekOrigin]::Begin) | Out-Null
        $peOffset = $reader.ReadInt32()
        if ($peOffset -le 0 -or $peOffset -gt ($stream.Length - 96)) {
            throw "invalid PE header offset $peOffset for $Path"
        }
        $stream.Seek($peOffset, [IO.SeekOrigin]::Begin) | Out-Null
        $signature = $reader.ReadUInt32()
        if ($signature -ne 0x00004550) {
            throw "invalid PE signature 0x$($signature.ToString('X8')) for $Path"
        }
        $optionalHeaderOffset = $peOffset + 24
        $stream.Seek($optionalHeaderOffset, [IO.SeekOrigin]::Begin) | Out-Null
        $magic = $reader.ReadUInt16()
        if ($magic -ne 0x10b -and $magic -ne 0x20b) {
            throw "unsupported PE optional-header magic 0x$($magic.ToString('X4')) for $Path"
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

function Test-TcpPortBusy {
    param([int]$Port)

    $client = [Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync("127.0.0.1", $Port)
        if (-not $task.Wait(250)) {
            return $false
        }
        return $client.Connected
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Wait-ForSmokeCondition {
    param(
        [string]$Description,
        [scriptblock]$Condition,
        [int]$TimeoutSeconds = 12
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (& $Condition) {
            return
        }
        Start-Sleep -Milliseconds 150
    }
    throw "Timed out waiting for $Description"
}

function Send-InstanceCommand {
    param(
        [int]$Port,
        [byte[]]$Command,
        [int]$TimeoutMilliseconds = 700
    )

    $client = [Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync("127.0.0.1", $Port)
        if (-not $task.Wait($TimeoutMilliseconds)) {
            return $false
        }
        if (-not $client.Connected) {
            return $false
        }
        $client.ReceiveTimeout = $TimeoutMilliseconds
        $client.SendTimeout = $TimeoutMilliseconds
        $stream = $client.GetStream()
        $stream.Write($Command, 0, $Command.Length)
        $stream.Flush()
        $buffer = New-Object byte[] $Ack.Length
        $offset = 0
        while ($offset -lt $buffer.Length) {
            $read = $stream.Read($buffer, $offset, $buffer.Length - $offset)
            if ($read -le 0) {
                return $false
            }
            $offset += $read
        }
        for ($i = 0; $i -lt $Ack.Length; $i++) {
            if ($buffer[$i] -ne $Ack[$i]) {
                return $false
            }
        }
        return $true
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Start-SmokeProcess {
    param(
        [string]$ExePath,
        [string]$Arguments,
        [string]$LocalAppData,
        [string]$AppData
    )

    $psi = [Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $ExePath
    $psi.Arguments = $Arguments
    $psi.WorkingDirectory = Split-Path -Parent $ExePath
    $psi.UseShellExecute = $false
    $psi.EnvironmentVariables["LOCALAPPDATA"] = $LocalAppData
    $psi.EnvironmentVariables["APPDATA"] = $AppData
    return [Diagnostics.Process]::Start($psi)
}

function Assert-ProcessRunning {
    param(
        [Diagnostics.Process]$Process,
        [string]$Label
    )

    $Process.Refresh()
    if ($Process.HasExited) {
        throw "$Label exited unexpectedly with code $($Process.ExitCode)"
    }
}

function Wait-ForProcessExit {
    param(
        [Diagnostics.Process]$Process,
        [string]$Label,
        [int]$TimeoutSeconds = 10
    )

    if (-not $Process.WaitForExit($TimeoutSeconds * 1000)) {
        throw "$Label did not exit within $TimeoutSeconds seconds"
    }
    if ($Process.ExitCode -ne 0) {
        throw "$Label exited with code $($Process.ExitCode)"
    }
}

function Get-ProcessRecord {
    param([Diagnostics.Process]$Process)

    $Process.Refresh()
    $cim = Get-CimInstance Win32_Process -Filter "ProcessId = $($Process.Id)" -ErrorAction SilentlyContinue
    return [ordered]@{
        processId = $Process.Id
        processName = $Process.ProcessName
        hasExited = $Process.HasExited
        commandLine = if ($cim) { $cim.CommandLine } else { $null }
        parentProcessId = if ($cim) { $cim.ParentProcessId } else { $null }
    }
}

function Get-VisibleWindowsForProcess {
    param([Diagnostics.Process]$Process)
    return @([ScreenWatchCoexistenceSmokeNative]::VisibleWindowsForPid([uint32]$Process.Id))
}

function Get-MainWindowCount {
    param(
        [Diagnostics.Process]$Process,
        [string]$Title
    )
    return @((Get-VisibleWindowsForProcess $Process) | Where-Object { $_.Title -eq $Title }).Count
}

function Stop-OwnedProcess {
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

function Remove-OwnedDirectory {
    param([string]$Path)

    if ($Path -and (Test-Path -LiteralPath $Path)) {
        Remove-Item -LiteralPath $Path -Recurse -Force
        return $true
    }
    return $false
}

$smokeId = ([guid]::NewGuid().ToString("N")).Substring(0, 8)
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-coexistence-smoke-$smokeId"
$localAppData = Join-Path $tempRoot "localappdata"
$appData = Join-Path $tempRoot "appdata"
$sharedDataDir = Join-Path $localAppData "ScreenWatchOCR"
$pythonProcess = $null
$tauriProcess = $null
$pythonSecondProcess = $null
$tauriSecondProcess = $null
$failure = $null

$result = [ordered]@{
    status = "running"
    timestamp = (Get-Date).ToString("o")
    machine = $env:COMPUTERNAME
    python = [ordered]@{}
    tauri = [ordered]@{}
    environment = [ordered]@{
        tempRoot = $tempRoot
        localAppData = $localAppData
        appData = $appData
        sharedDataDir = $sharedDataDir
    }
    ports = [ordered]@{
        python = $PythonPort
        tauri = $TauriPort
    }
    protocol = [ordered]@{}
    windows = [ordered]@{}
    cleanup = [ordered]@{}
}

try {
    $pythonItem = Get-Item -LiteralPath $PythonExePath
    $tauriItem = Get-Item -LiteralPath $TauriExePath
    $pythonSubsystem = Get-PeSubsystem $PythonExePath
    $tauriSubsystem = Get-PeSubsystem $TauriExePath
    $result.python = [ordered]@{
        exePath = $PythonExePath
        exeName = $pythonItem.Name
        bytes = $pythonItem.Length
        sha256 = Get-ExeSha256 $PythonExePath
        subsystem = $pythonSubsystem
        subsystemName = Get-PeSubsystemName $pythonSubsystem
        expectedPort = $PythonPort
        expectedCommand = "ScreenWatchOCR:show\n"
        expectedWindowTitle = "Screen Watch OCR"
    }
    $result.tauri = [ordered]@{
        exePath = $TauriExePath
        exeName = $tauriItem.Name
        bytes = $tauriItem.Length
        sha256 = Get-ExeSha256 $TauriExePath
        subsystem = $tauriSubsystem
        subsystemName = Get-PeSubsystemName $tauriSubsystem
        expectedPort = $TauriPort
        expectedCommand = "ScreenWatchOCRTauri:show\n"
        expectedWindowTitle = "Screen Watch OCR Tauri"
    }

    if ($pythonItem.Name -eq $tauriItem.Name) {
        throw "Python and Tauri deliverables must not have the same exe name"
    }
    if (Test-TcpPortBusy $PythonPort) {
        throw "Python default single-instance port $PythonPort is already busy; refusing to touch an existing app"
    }
    if (Test-TcpPortBusy $TauriPort) {
        throw "Tauri default single-instance port $TauriPort is already busy; refusing to touch an existing app"
    }

    New-Item -ItemType Directory -Force -Path $localAppData, $appData | Out-Null

    $pythonProcess = Start-SmokeProcess -ExePath $PythonExePath -Arguments "" -LocalAppData $localAppData -AppData $appData
    Wait-ForSmokeCondition "Python app binding port $PythonPort" { Test-TcpPortBusy $PythonPort } $StartupWaitSeconds
    Assert-ProcessRunning $pythonProcess "Python packaged app"
    Wait-ForSmokeCondition "Python app acknowledging its own single-instance command" {
        Assert-ProcessRunning $pythonProcess "Python packaged app"
        Send-InstanceCommand -Port $PythonPort -Command $PythonCommand
    } $StartupWaitSeconds

    $tauriProcess = Start-SmokeProcess -ExePath $TauriExePath -Arguments "" -LocalAppData $localAppData -AppData $appData
    Wait-ForSmokeCondition "Tauri app binding port $TauriPort" { Test-TcpPortBusy $TauriPort } $StartupWaitSeconds
    Assert-ProcessRunning $tauriProcess "Tauri packaged app"
    Wait-ForSmokeCondition "Tauri app acknowledging its own single-instance command" {
        Assert-ProcessRunning $tauriProcess "Tauri packaged app"
        Send-InstanceCommand -Port $TauriPort -Command $TauriCommand
    } $StartupWaitSeconds

    $result.python.process = Get-ProcessRecord $pythonProcess
    $result.tauri.process = Get-ProcessRecord $tauriProcess

    if ($result.python.process.processName -eq $result.tauri.process.processName) {
        throw "Python and Tauri process names must not match"
    }

    $tauriCommandToPython = Send-InstanceCommand -Port $PythonPort -Command $TauriCommand
    $pythonCommandToTauri = Send-InstanceCommand -Port $TauriPort -Command $PythonCommand
    if ($tauriCommandToPython) {
        throw "Python app accepted the Tauri single-instance command"
    }
    if ($pythonCommandToTauri) {
        throw "Tauri app accepted the Python single-instance command"
    }

    $pythonOwnCommandAccepted = Send-InstanceCommand -Port $PythonPort -Command $PythonCommand
    $tauriOwnCommandAccepted = Send-InstanceCommand -Port $TauriPort -Command $TauriCommand
    if (-not $pythonOwnCommandAccepted) {
        throw "Python app did not acknowledge its own single-instance command"
    }
    if (-not $tauriOwnCommandAccepted) {
        throw "Tauri app did not acknowledge its own single-instance command"
    }

    $pythonSecondProcess = Start-SmokeProcess -ExePath $PythonExePath -Arguments "" -LocalAppData $localAppData -AppData $appData
    Wait-ForProcessExit $pythonSecondProcess "Python second instance" $StartupWaitSeconds
    $tauriSecondProcess = Start-SmokeProcess -ExePath $TauriExePath -Arguments "" -LocalAppData $localAppData -AppData $appData
    Wait-ForProcessExit $tauriSecondProcess "Tauri second instance" $StartupWaitSeconds

    Assert-ProcessRunning $pythonProcess "Python packaged app after Tauri second instance"
    Assert-ProcessRunning $tauriProcess "Tauri packaged app after Python second instance"

    $pythonWindows = @(Get-VisibleWindowsForProcess $pythonProcess)
    $tauriWindows = @(Get-VisibleWindowsForProcess $tauriProcess)
    $result.windows = [ordered]@{
        pythonVisibleWindows = $pythonWindows
        tauriVisibleWindows = $tauriWindows
        pythonMainWindowCount = @($pythonWindows | Where-Object { $_.Title -eq "Screen Watch OCR" }).Count
        tauriMainWindowCount = @($tauriWindows | Where-Object { $_.Title -eq "Screen Watch OCR Tauri" }).Count
    }
    $result.protocol = [ordered]@{
        tauriCommandToPythonAccepted = $tauriCommandToPython
        pythonCommandToTauriAccepted = $pythonCommandToTauri
        pythonOwnCommandAccepted = $pythonOwnCommandAccepted
        tauriOwnCommandAccepted = $tauriOwnCommandAccepted
        pythonSecondInstanceExitCode = $pythonSecondProcess.ExitCode
        tauriSecondInstanceExitCode = $tauriSecondProcess.ExitCode
    }
    $result.status = "pass"

    Write-Host "pythonExePath: $PythonExePath"
    Write-Host "tauriExePath: $TauriExePath"
    Write-Host "pythonExeName: $($pythonItem.Name)"
    Write-Host "tauriExeName: $($tauriItem.Name)"
    Write-Host "pythonBytes: $($pythonItem.Length)"
    Write-Host "tauriBytes: $($tauriItem.Length)"
    Write-Host "pythonSha256: $($result.python.sha256)"
    Write-Host "tauriSha256: $($result.tauri.sha256)"
    Write-Host "pythonSubsystem: $($result.python.subsystemName) ($pythonSubsystem)"
    Write-Host "tauriSubsystem: $($result.tauri.subsystemName) ($tauriSubsystem)"
    Write-Host "sharedIsolatedLocalAppData: $localAppData"
    Write-Host "sharedIsolatedScreenWatchOCR: $sharedDataDir"
    Write-Host "pythonProcessId: $($pythonProcess.Id)"
    Write-Host "tauriProcessId: $($tauriProcess.Id)"
    Write-Host "pythonProcessName: $($result.python.process.processName)"
    Write-Host "tauriProcessName: $($result.tauri.process.processName)"
    Write-Host "pythonDefaultPortBusy: $(Test-TcpPortBusy $PythonPort)"
    Write-Host "tauriDefaultPortBusy: $(Test-TcpPortBusy $TauriPort)"
    Write-Host "tauriCommandToPythonAccepted: $tauriCommandToPython"
    Write-Host "pythonCommandToTauriAccepted: $pythonCommandToTauri"
    Write-Host "pythonOwnCommandAccepted: $pythonOwnCommandAccepted"
    Write-Host "tauriOwnCommandAccepted: $tauriOwnCommandAccepted"
    Write-Host "pythonMainWindowCount: $($result.windows.pythonMainWindowCount)"
    Write-Host "tauriMainWindowCount: $($result.windows.tauriMainWindowCount)"
    Write-Host "pythonSecondInstanceExitCode: $($pythonSecondProcess.ExitCode)"
    Write-Host "tauriSecondInstanceExitCode: $($tauriSecondProcess.ExitCode)"
    Write-Host "coexistenceSmokeVerified: True"
} catch {
    $failure = $_
    $result.status = "fail"
    $result.error = $_.Exception.Message
} finally {
    $result.cleanup.pythonSecondStopped = Stop-OwnedProcess $pythonSecondProcess
    $result.cleanup.tauriSecondStopped = Stop-OwnedProcess $tauriSecondProcess
    $result.cleanup.pythonStopped = Stop-OwnedProcess $pythonProcess
    $result.cleanup.tauriStopped = Stop-OwnedProcess $tauriProcess
    $result.cleanup.tempRootRemoved = Remove-OwnedDirectory $tempRoot
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $ResultPath) | Out-Null
    $result.resultPath = $ResultPath
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $ResultPath -Encoding UTF8
    Write-Host "resultPath: $ResultPath"
    Write-Host "cleanupPythonSecondStopped: $($result.cleanup.pythonSecondStopped)"
    Write-Host "cleanupTauriSecondStopped: $($result.cleanup.tauriSecondStopped)"
    Write-Host "cleanupPythonStopped: $($result.cleanup.pythonStopped)"
    Write-Host "cleanupTauriStopped: $($result.cleanup.tauriStopped)"
    Write-Host "cleanupTempRootRemoved: $($result.cleanup.tempRootRemoved)"
}

if ($failure) {
    throw $failure
}
