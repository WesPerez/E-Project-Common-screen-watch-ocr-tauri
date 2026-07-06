param(
    [string]$ExePath = "",
    [int]$StartupWaitSeconds = 8
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
if (-not $ExePath) {
    $ExePath = Join-Path $ProjectRootPath "target\release\screen-watch-ocr-tauri.exe"
}
$ExePath = (Resolve-Path $ExePath).Path

if (-not ([System.Management.Automation.PSTypeName]"ScreenWatchTrayMenuSmokeNative").Type) {
    Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class ScreenWatchTrayMenuSmokeNative
{
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    public delegate bool EnumChildProc(IntPtr hWnd, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct NOTIFYICONIDENTIFIER
    {
        public UInt32 cbSize;
        public IntPtr hWnd;
        public UInt32 uID;
        public Guid guidItem;
    }

    [DllImport("shell32.dll")]
    public static extern int Shell_NotifyIconGetRect(ref NOTIFYICONIDENTIFIER identifier, out RECT iconLocation);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lp);

    [DllImport("user32.dll")]
    public static extern bool EnumChildWindows(IntPtr hWnd, EnumChildProc cb, IntPtr lp);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetClassName(IntPtr hWnd, StringBuilder sb, int max);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder sb, int max);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extraInfo);

    public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
    public const uint MOUSEEVENTF_LEFTUP = 0x0004;

    public static string ClassName(IntPtr hWnd)
    {
        var sb = new StringBuilder(256);
        GetClassName(hWnd, sb, sb.Capacity);
        return sb.ToString();
    }

    public static string Title(IntPtr hWnd)
    {
        var sb = new StringBuilder(512);
        GetWindowText(hWnd, sb, sb.Capacity);
        return sb.ToString();
    }

    public static IntPtr FindPidWindowByClass(uint wantedPid, string className)
    {
        IntPtr found = IntPtr.Zero;
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == wantedPid && ClassName(hWnd) == className)
            {
                found = hWnd;
                return false;
            }

            EnumChildWindows(hWnd, delegate(IntPtr child, IntPtr childParam)
            {
                uint childPid;
                GetWindowThreadProcessId(child, out childPid);
                if (childPid == wantedPid && ClassName(child) == className)
                {
                    found = child;
                    return false;
                }
                return true;
            }, IntPtr.Zero);

            return found == IntPtr.Zero;
        }, IntPtr.Zero);
        return found;
    }

    public static object[] VisibleWindowsForPid(uint wantedPid)
    {
        var list = new List<object>();
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == wantedPid && IsWindowVisible(hWnd))
            {
                RECT rect;
                GetWindowRect(hWnd, out rect);
                list.Add(new
                {
                    Hwnd = hWnd.ToInt64(),
                    Pid = pid,
                    Class = ClassName(hWnd),
                    Title = Title(hWnd),
                    Left = rect.Left,
                    Top = rect.Top,
                    Right = rect.Right,
                    Bottom = rect.Bottom
                });
            }
            return true;
        }, IntPtr.Zero);
        return list.ToArray();
    }

    public static object[] TopWindows()
    {
        var list = new List<object>();
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam)
        {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (IsWindowVisible(hWnd))
            {
                RECT rect;
                GetWindowRect(hWnd, out rect);
                list.Add(new
                {
                    Hwnd = hWnd.ToInt64(),
                    Pid = pid,
                    Class = ClassName(hWnd),
                    Title = Title(hWnd),
                    Left = rect.Left,
                    Top = rect.Top,
                    Right = rect.Right,
                    Bottom = rect.Bottom
                });
            }
            return true;
        }, IntPtr.Zero);
        return list.ToArray();
    }

    public static void LeftClick(int x, int y)
    {
        SetCursorPos(x, y);
        mouse_event(MOUSEEVENTF_LEFTDOWN, (uint)x, (uint)y, 0, UIntPtr.Zero);
        mouse_event(MOUSEEVENTF_LEFTUP, (uint)x, (uint)y, 0, UIntPtr.Zero);
    }
}
"@
}

function New-FreeLoopbackPort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Parse("127.0.0.1"), 0)
    $listener.Start()
    try {
        return ([Net.IPEndPoint]$listener.LocalEndpoint).Port
    } finally {
        $listener.Stop()
    }
}

function Wait-ForSmokeCondition {
    param(
        [string]$Description,
        [scriptblock]$Condition,
        [int]$TimeoutSeconds = 8
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

function Start-SmokeProcess {
    param(
        [string]$ExePath,
        [string]$LocalAppData,
        [int]$InstancePort
    )

    $psi = [Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $ExePath
    $psi.Arguments = "--start-minimized"
    $psi.WorkingDirectory = Split-Path -Parent $ExePath
    $psi.UseShellExecute = $false
    $psi.EnvironmentVariables["LOCALAPPDATA"] = $LocalAppData
    $psi.EnvironmentVariables["SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT"] = [string]$InstancePort
    return [Diagnostics.Process]::Start($psi)
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

function Stop-SmokeProcess {
    param([Diagnostics.Process]$Process)

    if (-not $Process) {
        return $false
    }
    $Process.Refresh()
    if ($Process.HasExited) {
        return $false
    }
    $Process.Kill()
    $Process.WaitForExit(5000) | Out-Null
    return $true
}

function Get-VisibleMainWindows {
    param([Diagnostics.Process]$Process)

    return @(
        [ScreenWatchTrayMenuSmokeNative]::VisibleWindowsForPid([uint32]$Process.Id) |
            Where-Object { $_.Title -eq "Screen Watch OCR Tauri" -or $_.Class -eq "Tauri Window" }
    )
}

function Get-TauriTrayRect {
    param([Diagnostics.Process]$Process)

    $hwnd = [ScreenWatchTrayMenuSmokeNative]::FindPidWindowByClass([uint32]$Process.Id, "tray_icon_app")
    if ($hwnd -eq [IntPtr]::Zero) {
        throw "Could not find Tauri tray hidden window class tray_icon_app for PID $($Process.Id)"
    }

    for ($uid = 0; $uid -le 16; $uid++) {
        $identifier = New-Object ScreenWatchTrayMenuSmokeNative+NOTIFYICONIDENTIFIER
        $identifier.cbSize = [Runtime.InteropServices.Marshal]::SizeOf([type]"ScreenWatchTrayMenuSmokeNative+NOTIFYICONIDENTIFIER")
        $identifier.hWnd = $hwnd
        $identifier.uID = [uint32]$uid
        $identifier.guidItem = [Guid]::Empty
        $rect = New-Object ScreenWatchTrayMenuSmokeNative+RECT
        $hresult = [ScreenWatchTrayMenuSmokeNative]::Shell_NotifyIconGetRect([ref]$identifier, [ref]$rect)
        if ($hresult -eq 0) {
            return [pscustomobject][ordered]@{
                Hwnd = $hwnd
                Uid = $uid
                Left = $rect.Left
                Top = $rect.Top
                Right = $rect.Right
                Bottom = $rect.Bottom
                CenterX = [int](($rect.Left + $rect.Right) / 2)
                CenterY = [int](($rect.Top + $rect.Bottom) / 2)
            }
        }
    }

    throw "Shell_NotifyIconGetRect could not find a notification icon for Tauri PID $($Process.Id)"
}

function Open-TauriTrayMenu {
    param([object]$TrayRect)

    [ScreenWatchTrayMenuSmokeNative]::SetCursorPos($TrayRect.CenterX, $TrayRect.CenterY) | Out-Null
    [ScreenWatchTrayMenuSmokeNative]::PostMessage(
        $TrayRect.Hwnd,
        6002,
        [IntPtr]$TrayRect.Uid,
        [IntPtr]0x0205
    ) | Out-Null
    Start-Sleep -Milliseconds 500
}

function Get-TauriNativeMenuWindow {
    param([Diagnostics.Process]$Process)

    $menus = @(
        [ScreenWatchTrayMenuSmokeNative]::TopWindows() |
            Where-Object { $_.Pid -eq [uint32]$Process.Id -and $_.Class -eq "#32768" } |
            Sort-Object Top, Left
    )
    if ($menus.Count -eq 0) {
        return $null
    }
    return $menus[0]
}

function Assert-TauriMenuWindow {
    param(
        [Diagnostics.Process]$Process,
        [string]$Scenario
    )

    $menu = Get-TauriNativeMenuWindow $Process
    if (-not $menu) {
        throw "$Scenario did not expose a native Tauri tray menu window"
    }
    return $menu
}

function Invoke-FirstMenuItem {
    param([object]$Menu)

    $height = [int]($Menu.Bottom - $Menu.Top)
    $itemHeight = [Math]::Max(18, [int]($height / 3))
    $x = [int](($Menu.Left + $Menu.Right) / 2)
    $y = [int]($Menu.Top + ($itemHeight / 2))
    [ScreenWatchTrayMenuSmokeNative]::LeftClick($x, $y)
    return "$x,$y"
}

function Invoke-LastMenuItem {
    param([object]$Menu)

    $height = [int]($Menu.Bottom - $Menu.Top)
    $itemHeight = [Math]::Max(18, [int]($height / 3))
    $x = [int](($Menu.Left + $Menu.Right) / 2)
    $y = [int]($Menu.Bottom - ($itemHeight / 2))
    [ScreenWatchTrayMenuSmokeNative]::LeftClick($x, $y)
    return "$x,$y"
}

$smokeId = ([guid]::NewGuid().ToString("N")).Substring(0, 8)
$smokeLocalAppData = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-tauri-tray-menu-smoke-$smokeId"
$instancePort = New-FreeLoopbackPort
$process = $null
$stoppedProcess = $false
$removedAppData = $false

New-Item -ItemType Directory -Path $smokeLocalAppData | Out-Null
try {
    $process = Start-SmokeProcess -ExePath $ExePath -LocalAppData $smokeLocalAppData -InstancePort $instancePort
    Start-Sleep -Seconds $StartupWaitSeconds
    Assert-ProcessRunning $process "tray-menu"

    $initialMainWindows = Get-VisibleMainWindows $process
    if ($initialMainWindows.Count -ne 0) {
        throw "start-minimized tray smoke expected no visible main window, found $($initialMainWindows.Count)"
    }

    $trayRect = Get-TauriTrayRect $process
    Open-TauriTrayMenu $trayRect
    $showMenu = Assert-TauriMenuWindow $process "Show Tauri"
    $showClick = Invoke-FirstMenuItem $showMenu

    Wait-ForSmokeCondition "Show Tauri revealing the main window" {
        Assert-ProcessRunning $process "show-menu"
        (Get-VisibleMainWindows $process).Count -gt 0
    } $StartupWaitSeconds
    $visibleAfterShow = Get-VisibleMainWindows $process

    $trayRectAfterShow = Get-TauriTrayRect $process
    Open-TauriTrayMenu $trayRectAfterShow
    $exitMenu = Assert-TauriMenuWindow $process "Exit Tauri"
    $exitClick = Invoke-LastMenuItem $exitMenu

    if (-not $process.WaitForExit($StartupWaitSeconds * 1000)) {
        throw "Tauri process did not exit after the tray Exit Tauri menu item"
    }
    if ($process.ExitCode -ne 0) {
        throw "Tauri process exited with code $($process.ExitCode) after the tray Exit Tauri menu item"
    }

    Write-Host "exePath: $ExePath"
    Write-Host "smokeLocalAppData: $smokeLocalAppData"
    Write-Host "isolatedInstancePort: $instancePort"
    Write-Host "processId: $($process.Id)"
    Write-Host "trayHiddenWindowClass: tray_icon_app"
    Write-Host "trayHiddenHwnd: $($trayRect.Hwnd.ToInt64())"
    Write-Host "trayUid: $($trayRect.Uid)"
    Write-Host "trayRect: $($trayRect.Left),$($trayRect.Top),$($trayRect.Right),$($trayRect.Bottom)"
    Write-Host "showMenuHwnd: $($showMenu.Hwnd)"
    Write-Host "showMenuPid: $($showMenu.Pid)"
    Write-Host "showMenuClass: $($showMenu.Class)"
    Write-Host "showClick: $showClick"
    Write-Host "visibleMainWindowsAfterShow: $($visibleAfterShow.Count)"
    Write-Host "exitMenuHwnd: $($exitMenu.Hwnd)"
    Write-Host "exitMenuPid: $($exitMenu.Pid)"
    Write-Host "exitMenuClass: $($exitMenu.Class)"
    Write-Host "exitClick: $exitClick"
    Write-Host "processExitedAfterExit: True"
    Write-Host "exitCode: $($process.ExitCode)"
    Write-Host "trayMenuSmokeVerified: True"
} finally {
    $stoppedProcess = $stoppedProcess -or (Stop-SmokeProcess $process)
    if (Test-Path -LiteralPath $smokeLocalAppData) {
        Remove-Item -LiteralPath $smokeLocalAppData -Recurse -Force
        $removedAppData = $true
    }
    Write-Host "cleanupStoppedProcess: $stoppedProcess"
    Write-Host "cleanupRemovedAppData: $removedAppData"
}
