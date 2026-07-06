import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";

const windowsProjectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const exePath = path.join(
  windowsProjectRoot,
  "target",
  "release",
  "screen-watch-ocr-tauri.exe",
);
const evidenceDir = path.join(
  windowsProjectRoot,
  "docs",
  "manual-gate-evidence",
);
const evidenceLogDir = path.join(evidenceDir, "logs");
const buildInfoPath = path.join(
  windowsProjectRoot,
  "target",
  "release",
  "screen-watch-ocr-tauri.build-info.json",
);

const args = new Set(process.argv.slice(2));
const shouldWriteRecords = !args.has("--no-record");
const gateMode = valueArg("--gate") || "all";
const monitoringSoakMs = clampNumber(
  numberArg(
    "--soak-ms",
    Number(process.env.SCREENWATCH_MONITORING_SOAK_MS || 60000),
  ),
  10000,
  300000,
);
const runStamp = timestamp();
const runRoot = path.join(
  windowsProjectRoot,
  "target",
  "webview-visual-smoke",
  runStamp,
);
const localAppData = path.join(runRoot, "localappdata");
const inputDir = path.join(runRoot, "inputs");
const appLogPath = path.join(evidenceLogDir, `webview-visual-smoke-${runStamp}-app.log`);
const resultPath = path.join(
  evidenceLogDir,
  `webview-visual-smoke-${runStamp}-result.json`,
);

const cdpPort = 9300 + Math.floor(Math.random() * 500);
const singleInstancePort = 48000 + Math.floor(Math.random() * 1000);
const helperTitle = `Screen Watch OCR Visual Smoke Source ${runStamp}`;
const logs = [];
const screenshots = [];
const gateResults = {
  source: null,
  gallery: null,
  clipboard: null,
  scan: null,
  monitoring: null,
  monitoringSoak: null,
  layout: null,
};

let appProcess = null;
let helperProcess = null;
let clipboardSession = null;
let cdp = null;

function valueArg(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || index + 1 >= process.argv.length) {
    return "";
  }
  return process.argv[index + 1];
}

function numberArg(name, fallback) {
  const raw = valueArg(name);
  if (raw === "") {
    return fallback;
  }
  const value = Number(raw);
  return Number.isFinite(value) ? value : fallback;
}

function clampNumber(value, min, max) {
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return min;
  }
  return Math.min(max, Math.max(min, Math.trunc(number)));
}

function timestamp() {
  const date = new Date();
  const pad = (value) => String(value).padStart(2, "0");
  return [
    date.getFullYear(),
    pad(date.getMonth() + 1),
    pad(date.getDate()),
    "-",
    pad(date.getHours()),
    pad(date.getMinutes()),
    pad(date.getSeconds()),
  ].join("");
}

function log(message, extra = null) {
  const item = {
    time: new Date().toISOString(),
    message,
    ...(extra ? { extra } : {}),
  };
  logs.push(item);
  console.log(message);
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitFor(fn, description, timeoutMs = 20000, intervalMs = 250) {
  const started = Date.now();
  let lastError = null;
  while (Date.now() - started < timeoutMs) {
    try {
      const value = await fn();
      if (value) {
        return value;
      }
    } catch (error) {
      lastError = error;
    }
    await sleep(intervalMs);
  }
  const suffix = lastError ? ` Last error: ${lastError.message}` : "";
  throw new Error(`Timed out waiting for ${description}.${suffix}`);
}

function readBuildInfo() {
  if (!fs.existsSync(buildInfoPath)) {
    return {
      path: buildInfoPath,
      exists: false,
      executableSha256: "missing",
    };
  }
  const parsed = JSON.parse(fs.readFileSync(buildInfoPath, "utf8").replace(/^\uFEFF/, ""));
  return {
    path: buildInfoPath,
    exists: true,
    ...parsed,
  };
}

function encodeCommand(script) {
  return Buffer.from(script, "utf16le").toString("base64");
}

function startHelperWindow() {
  const script = `
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$form = New-Object System.Windows.Forms.Form
$form.Text = '${helperTitle.replaceAll("'", "''")}'
$form.StartPosition = 'Manual'
$form.Left = 80
$form.Top = 80
$form.Width = 420
$form.Height = 260
$form.BackColor = [System.Drawing.Color]::FromArgb(24, 68, 113)
$label = New-Object System.Windows.Forms.Label
$label.Dock = 'Fill'
$label.TextAlign = 'MiddleCenter'
$label.Font = New-Object System.Drawing.Font('Segoe UI', 18, [System.Drawing.FontStyle]::Bold)
$label.ForeColor = [System.Drawing.Color]::White
$label.Text = "WINDOW SOURCE\`r\`n${runStamp}"
$form.Controls.Add($label)
[System.Windows.Forms.Application]::Run($form)
`;
  const child = spawn(
    "powershell.exe",
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", encodeCommand(script)],
    {
      cwd: windowsProjectRoot,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: false,
    },
  );
  child.stdout?.on("data", (chunk) => {
    fs.appendFileSync(appLogPath, `[helper stdout] ${chunk}`);
  });
  child.stderr?.on("data", (chunk) => {
    fs.appendFileSync(appLogPath, `[helper stderr] ${chunk}`);
  });
  log("started helper window", { pid: child.pid, title: helperTitle });
  return child;
}

function startApp() {
  if (!fs.existsSync(exePath)) {
    throw new Error(`Missing release executable: ${exePath}`);
  }
  const env = {
    ...process.env,
    LOCALAPPDATA: localAppData,
    SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT: String(singleInstancePort),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${cdpPort}`,
  };
  const child = spawn(exePath, [], {
    cwd: windowsProjectRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: false,
  });
  child.stdout?.on("data", (chunk) => {
    fs.appendFileSync(appLogPath, `[app stdout] ${chunk}`);
  });
  child.stderr?.on("data", (chunk) => {
    fs.appendFileSync(appLogPath, `[app stderr] ${chunk}`);
  });
  log("started app", {
    pid: child.pid,
    cdpPort,
    singleInstancePort,
    localAppData,
  });
  return child;
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url} returned HTTP ${response.status}`);
  }
  return response.json();
}

async function waitForWebviewTarget() {
  return waitFor(async () => {
    const targets = await fetchJson(`http://127.0.0.1:${cdpPort}/json/list`);
    const pages = Array.isArray(targets) ? targets : [targets];
    return pages.find(
      (target) =>
        target.type === "page" &&
        String(target.url || "").startsWith("http://tauri.localhost/") &&
        target.webSocketDebuggerUrl,
    );
  }, "WebView2 CDP target", 20000, 300);
}

class CdpClient {
  constructor(url) {
    this.url = url;
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
  }

  async connect() {
    this.socket = new WebSocket(this.url);
    this.socket.addEventListener("message", (event) => this.onMessage(event));
    this.socket.addEventListener("close", () => {
      for (const { reject } of this.pending.values()) {
        reject(new Error("CDP socket closed"));
      }
      this.pending.clear();
    });
    await new Promise((resolve, reject) => {
      this.socket.addEventListener("open", resolve, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
  }

  close() {
    this.socket?.close();
  }

  onMessage(event) {
    const message = JSON.parse(String(event.data));
    if (message.id && this.pending.has(message.id)) {
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) {
        reject(new Error(`${message.error.message || "CDP error"}`));
      } else {
        resolve(message.result || {});
      }
      return;
    }
    if (message.method && this.listeners.has(message.method)) {
      for (const listener of this.listeners.get(message.method)) {
        listener(message.params || {});
      }
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(payload);
      setTimeout(() => {
        if (!this.pending.has(id)) {
          return;
        }
        this.pending.delete(id);
        reject(new Error(`CDP command timed out: ${method}`));
      }, 15000);
    });
  }

  once(method, timeoutMs = 15000) {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        const listeners = this.listeners.get(method) || [];
        this.listeners.set(
          method,
          listeners.filter((listener) => listener !== onEvent),
        );
        reject(new Error(`Timed out waiting for CDP event ${method}`));
      }, timeoutMs);
      const onEvent = (params) => {
        clearTimeout(timeout);
        const listeners = this.listeners.get(method) || [];
        this.listeners.set(
          method,
          listeners.filter((listener) => listener !== onEvent),
        );
        resolve(params);
      };
      const listeners = this.listeners.get(method) || [];
      listeners.push(onEvent);
      this.listeners.set(method, listeners);
    });
  }
}

async function evalJs(expression) {
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(
      result.exceptionDetails.text ||
        result.exceptionDetails.exception?.description ||
        "Runtime.evaluate failed",
    );
  }
  return result.result?.value;
}

async function clickSelector(selector) {
  return evalJs(`(() => {
    const item = document.querySelector(${JSON.stringify(selector)});
    if (!item) return false;
    item.click();
    return true;
  })()`);
}

async function pressCtrlV() {
  await evalJs(`(() => {
    document.activeElement?.blur?.();
    document.body.setAttribute('tabindex', '-1');
    document.body.focus();
    return document.activeElement === document.body;
  })()`);
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "Control",
    code: "ControlLeft",
    windowsVirtualKeyCode: 17,
    nativeVirtualKeyCode: 17,
    modifiers: 2,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "v",
    code: "KeyV",
    text: "v",
    windowsVirtualKeyCode: 86,
    nativeVirtualKeyCode: 86,
    modifiers: 2,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "v",
    code: "KeyV",
    windowsVirtualKeyCode: 86,
    nativeVirtualKeyCode: 86,
    modifiers: 2,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "Control",
    code: "ControlLeft",
    windowsVirtualKeyCode: 17,
    nativeVirtualKeyCode: 17,
    modifiers: 0,
  });
}

async function mouseDrag(from, to, steps = 8) {
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: from.x,
    y: from.y,
    button: "none",
    buttons: 0,
  });
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: from.x,
    y: from.y,
    button: "left",
    buttons: 1,
    clickCount: 1,
  });
  for (let index = 1; index <= steps; index += 1) {
    const ratio = index / steps;
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: from.x + (to.x - from.x) * ratio,
      y: from.y + (to.y - from.y) * ratio,
      button: "left",
      buttons: 1,
    });
    await sleep(35);
  }
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: to.x,
    y: to.y,
    button: "left",
    buttons: 0,
    clickCount: 1,
  });
  await sleep(350);
}

async function dragSelector(selector, delta) {
  const rect = await evalJs(`(() => {
    const item = document.querySelector(${JSON.stringify(selector)});
    if (!item) return null;
    const rect = item.getBoundingClientRect();
    return {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
      centerX: rect.left + rect.width / 2,
      centerY: rect.top + rect.height / 2,
    };
  })()`);
  if (!rect) {
    throw new Error(`Cannot drag missing selector ${selector}`);
  }
  await mouseDrag(
    { x: rect.centerX, y: rect.centerY },
    { x: rect.centerX + (delta.dx || 0), y: rect.centerY + (delta.dy || 0) },
  );
}

async function dragControlGroupResizeHandle(selector, delta) {
  const rect = await evalJs(`(() => {
    const item = document.querySelector(${JSON.stringify(selector)});
    if (!item) return null;
    const rect = item.getBoundingClientRect();
    return {
      x: rect.right - 4,
      y: rect.bottom - 4,
      width: rect.width,
      height: rect.height,
    };
  })()`);
  if (!rect) {
    throw new Error(`Cannot resize missing selector ${selector}`);
  }
  await mouseDrag(
    { x: rect.x, y: rect.y },
    { x: rect.x + (delta.dx || 0), y: rect.y + (delta.dy || 0) },
    10,
  );
}

async function resetStoredWorkbenchLayoutForSmoke() {
  await evalJs(`(() => {
    const key = "screen-watch-ocr-tauri:workbench-layout:v1";
    window.localStorage.removeItem(key);
    window.dispatchEvent(new Event("resize"));
    return {
      storedLayout: window.localStorage.getItem(key),
      viewport: { width: window.innerWidth, height: window.innerHeight },
    };
  })()`);
  await sleep(500);
}

async function captureScreenshot(name) {
  const file = path.join(evidenceLogDir, `${name}-${runStamp}.png`);
  try {
    await captureNativeWindow(file);
    screenshots.push(file);
    log("captured native window screenshot", { file });
    return file;
  } catch (error) {
    log("native window screenshot failed; falling back to CDP screenshot", {
      error: error.message,
    });
  }
  const result = await cdp.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  fs.writeFileSync(file, Buffer.from(result.data, "base64"));
  screenshots.push(file);
  log("captured screenshot", { file });
  return file;
}

function psHereString(value) {
  return `@'\n${String(value).replaceAll("'@", "' + \"@\" + '")}\n'@`;
}

async function captureNativeWindow(file) {
  const script = `
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class SmokeScreenshotUser32 {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT {
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;
  }
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr hWnd);
}
"@
$file = ${psHereString(file)}
$process = Get-Process -Id ${appProcess.pid} -ErrorAction Stop
$deadline = (Get-Date).AddSeconds(10)
while ($process.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 250
  $process.Refresh()
}
if ($process.MainWindowHandle -eq 0) {
  throw "main window handle unavailable"
}
[SmokeScreenshotUser32]::SetForegroundWindow($process.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 250
$rect = New-Object SmokeScreenshotUser32+RECT
if (-not [SmokeScreenshotUser32]::GetWindowRect($process.MainWindowHandle, [ref]$rect)) {
  throw "GetWindowRect failed"
}
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) {
  throw "invalid window rect $width x $height"
}
$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
try {
  $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
  $bitmap.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
} finally {
  $graphics.Dispose()
  $bitmap.Dispose()
}
`;
  await runPowershell(script, "capture native window screenshot");
}

async function resizeAppWindow(width, height) {
  const script = `
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class SmokeUser32 {
  [DllImport("user32.dll")]
  public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
}
"@
$process = Get-Process -Id ${appProcess.pid} -ErrorAction Stop
$deadline = (Get-Date).AddSeconds(10)
while ($process.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 250
  $process.Refresh()
}
if ($process.MainWindowHandle -eq 0) {
  throw "main window handle unavailable"
}
[SmokeUser32]::SetWindowPos($process.MainWindowHandle, [IntPtr]::Zero, 120, 80, ${width}, ${height}, 0x0040) | Out-Null
`;
  await runPowershell(script, "resize app window");
  log("resized app window", { width, height });
}

function runPowershell(script, label) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "powershell.exe",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", encodeCommand(script)],
      {
        cwd: windowsProjectRoot,
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      },
    );
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("exit", (code) => {
      fs.appendFileSync(
        appLogPath,
        `[${label}] exit=${code}\nstdout:\n${stdout}\nstderr:\n${stderr}\n`,
      );
      if (code === 0) {
        resolve(stdout);
      } else {
        reject(new Error(`${label} failed with exit ${code}: ${stderr || stdout}`));
      }
    });
  });
}

class ClipboardSession {
  constructor() {
    this.buffer = "";
    this.pending = [];
    this.ready = false;
    this.restored = false;
    this.readyPromise = null;
    this.resolveReady = null;
    this.rejectReady = null;
    this.child = null;
  }

  start() {
    if (this.readyPromise) {
      return this.readyPromise;
    }
    const script = `
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
function Invoke-ClipboardRetry([scriptblock]$Action) {
  $last = $null
  for ($i = 0; $i -lt 15; $i++) {
    try {
      return & $Action
    } catch {
      $last = $_
      Start-Sleep -Milliseconds 120
    }
  }
  throw $last
}
function Write-SmokeJson($Value) {
  $Value | ConvertTo-Json -Compress | Write-Output
}
function Get-ClipboardFormatNames {
  $data = Invoke-ClipboardRetry { [System.Windows.Forms.Clipboard]::GetDataObject() }
  if ($null -eq $data) {
    return @()
  }
  return @($data.GetFormats())
}
$oldClipboard = Invoke-ClipboardRetry { [System.Windows.Forms.Clipboard]::GetDataObject() }
Write-Output "READY"
while ($true) {
  $line = [Console]::In.ReadLine()
  if ($null -eq $line) {
    break
  }
  try {
    $cmd = $line | ConvertFrom-Json
    switch ([string]$cmd.action) {
      "image" {
        $image = New-Object System.Drawing.Bitmap([string]$cmd.path)
        $bmpStream = New-Object System.IO.MemoryStream
        $dibStream = $null
        try {
          $image.Save($bmpStream, [System.Drawing.Imaging.ImageFormat]::Bmp)
          $bmpBytes = $bmpStream.ToArray()
          $dibBytes = New-Object byte[] ($bmpBytes.Length - 14)
          [Array]::Copy($bmpBytes, 14, $dibBytes, 0, $dibBytes.Length)
          $dibStream = New-Object System.IO.MemoryStream(,$dibBytes)
          $dataObject = New-Object System.Windows.Forms.DataObject
          $dataObject.SetData([System.Windows.Forms.DataFormats]::Dib, $dibStream)
          $dataObject.SetData([System.Windows.Forms.DataFormats]::Bitmap, $image)
          Invoke-ClipboardRetry { [System.Windows.Forms.Clipboard]::SetDataObject($dataObject, $true) } | Out-Null
        } finally {
          if ($null -ne $dibStream) { $dibStream.Dispose() }
          $bmpStream.Dispose()
          $image.Dispose()
        }
        Write-SmokeJson @{
          ok = $true
          action = "image"
          formats = @(Get-ClipboardFormatNames)
        }
      }
      "files" {
        $list = New-Object System.Collections.Specialized.StringCollection
        foreach ($path in @($cmd.paths)) {
          [void]$list.Add([string]$path)
        }
        Invoke-ClipboardRetry { [System.Windows.Forms.Clipboard]::SetFileDropList($list) } | Out-Null
        Write-SmokeJson @{
          ok = $true
          action = "files"
          count = $list.Count
          formats = @(Get-ClipboardFormatNames)
        }
      }
      "restore" {
        Invoke-ClipboardRetry {
          if ($null -ne $oldClipboard) {
            [System.Windows.Forms.Clipboard]::SetDataObject($oldClipboard, $true)
          } else {
            [System.Windows.Forms.Clipboard]::Clear()
          }
        } | Out-Null
        Write-SmokeJson @{ ok = $true; action = "restore" }
        exit 0
      }
      default {
        throw "unknown clipboard action: $($cmd.action)"
      }
    }
  } catch {
    Write-SmokeJson @{ ok = $false; error = $_.Exception.Message }
  }
}
`;
    this.readyPromise = new Promise((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
      this.child = spawn(
        "powershell.exe",
        [
          "-NoProfile",
          "-STA",
          "-ExecutionPolicy",
          "Bypass",
          "-EncodedCommand",
          encodeCommand(script),
        ],
        {
          cwd: windowsProjectRoot,
          stdio: ["pipe", "pipe", "pipe"],
          windowsHide: true,
        },
      );
      this.child.stdout.on("data", (chunk) => this.onStdout(chunk));
      this.child.stderr.on("data", (chunk) => {
        fs.appendFileSync(appLogPath, `[clipboard stderr] ${chunk}`);
      });
      this.child.on("exit", (code) => {
        fs.appendFileSync(appLogPath, `[clipboard helper] exit=${code}\n`);
        if (!this.ready) {
          this.rejectReady?.(new Error(`clipboard helper exited before ready: ${code}`));
        }
        while (this.pending.length) {
          const pending = this.pending.shift();
          pending.reject(new Error(`clipboard helper exited: ${code}`));
        }
      });
    });
    return this.readyPromise;
  }

  onStdout(chunk) {
    this.buffer += String(chunk);
    const lines = this.buffer.split(/\r?\n/);
    this.buffer = lines.pop() || "";
    for (const raw of lines) {
      const line = raw.trim();
      if (!line) {
        continue;
      }
      fs.appendFileSync(appLogPath, `[clipboard stdout] ${line}\n`);
      if (line === "READY") {
        this.ready = true;
        this.resolveReady?.();
        continue;
      }
      const pending = this.pending.shift();
      if (!pending) {
        continue;
      }
      try {
        const parsed = JSON.parse(line);
        if (parsed.ok) {
          pending.resolve(parsed);
        } else {
          pending.reject(new Error(parsed.error || "clipboard helper failed"));
        }
      } catch (error) {
        pending.reject(error);
      }
    }
  }

  async request(payload) {
    await this.start();
    if (!this.child || !this.child.stdin.writable) {
      throw new Error("clipboard helper stdin is not writable");
    }
    return new Promise((resolve, reject) => {
      this.pending.push({ resolve, reject });
      this.child.stdin.write(`${JSON.stringify(payload)}\n`, "utf8");
    });
  }

  setImage(file) {
    return this.request({ action: "image", path: file });
  }

  setFiles(files) {
    return this.request({ action: "files", paths: files });
  }

  async restore() {
    if (this.restored) {
      return;
    }
    this.restored = true;
    try {
      await this.request({ action: "restore" });
    } finally {
      this.child?.stdin?.end?.();
    }
  }
}

async function ensureClipboardSession() {
  if (!clipboardSession) {
    clipboardSession = new ClipboardSession();
  }
  await clipboardSession.start();
  return clipboardSession;
}

async function waitForReadyStatus() {
  return waitFor(async () => {
    const status = await evalJs("document.querySelector('#status')?.textContent || ''");
    return status.startsWith("Ready") ? status : "";
  }, "Ready status", 25000, 300);
}

async function sourcePreviewState() {
  return evalJs(`(() => ({
    status: document.querySelector('#status')?.textContent || '',
    scrollY: window.scrollY,
    viewport: { width: window.innerWidth, height: window.innerHeight },
    cards: [...document.querySelectorAll('.source-preview-card')].map((card) => {
      const frame = card.querySelector('.source-preview-frame');
      const img = card.querySelector('img');
      const rect = frame?.getBoundingClientRect();
      return {
        key: card.dataset.sourceKey || '',
        title: card.querySelector('strong')?.textContent || '',
        meta: card.querySelector('small')?.textContent || '',
        usesDwm: card.classList.contains('uses-dwm'),
        isError: card.classList.contains('is-error'),
        hasImage: Boolean(img?.getAttribute('src')),
        message: card.querySelector('.source-preview-message')?.textContent || '',
        rect: rect ? {
          left: Math.round(rect.left),
          top: Math.round(rect.top),
          width: Math.round(rect.width),
          height: Math.round(rect.height)
        } : null,
      };
    })
  }))()`);
}

async function galleryState() {
  return evalJs(`(() => ({
    status: document.querySelector('#status')?.textContent || '',
    summary: document.querySelector('#profile-summary')?.textContent || '',
    selectedIndex: [...document.querySelectorAll('.target-card')].findIndex((card) => card.classList.contains('is-selected')),
    scrollbarState: document.querySelector('#profile-targets')?.dataset.scrollbarState || '',
    cards: [...document.querySelectorAll('.target-card')].map((card, index) => ({
      index,
      title: card.querySelector('.target-text strong')?.textContent || '',
      meta: card.querySelector('.target-text small')?.textContent || '',
      disabled: card.classList.contains('is-disabled'),
      selected: card.classList.contains('is-selected'),
      hasImage: card.querySelector('.target-thumb')?.classList.contains('has-image') || false,
      hasBottomBorder: getComputedStyle(card).borderBottomWidth !== '0px',
      hitText: card.querySelector('.target-hit-badge')?.textContent || '',
      hitCount: Number(card.querySelector('.target-hit-badge')?.textContent || 0),
      card: {
        width: Math.round(card.getBoundingClientRect().width || 0),
        height: Math.round(card.getBoundingClientRect().height || 0),
      },
      thumb: {
        width: Math.round(card.querySelector('.target-thumb')?.getBoundingClientRect().width || 0),
        height: Math.round(card.querySelector('.target-thumb')?.getBoundingClientRect().height || 0),
      },
    })),
    menuVisible: Boolean(document.querySelector('.target-menu')),
    menuText: document.querySelector('.target-menu')?.textContent || '',
  }))()`);
}

async function monitoringState() {
  return evalJs(`(() => {
    const resultText = document.querySelector('#scan-result')?.textContent || '';
    let parsed = null;
    try {
      parsed = resultText.trim() ? JSON.parse(resultText) : null;
    } catch (_error) {
      parsed = null;
    }
    const session = parsed?.snapshot && typeof parsed.snapshot === 'object'
      ? parsed.snapshot
      : parsed;
    const rows = [...document.querySelectorAll('#event-log tr')].map((row) => row.textContent || '');
    return {
      status: document.querySelector('#status')?.textContent || '',
      buttonText: document.querySelector('#profile-monitor-start')?.textContent || '',
      buttonDisabled: Boolean(document.querySelector('#profile-monitor-start')?.disabled),
      logRows: rows,
      progressRows: rows.filter((text) => text.includes('第 ') && text.includes('扫描')),
      startedRows: rows.filter((text) => text.includes('监控中')).length,
      stoppedRows: rows.filter((text) => text.includes('停止')).length,
      event: parsed?.snapshot ? parsed : null,
      session,
      generation: Number(session?.generation || session?.sessionGeneration || session?.session_generation || 0),
      tickCount: Number(session?.tickCount || session?.tick_count || 0),
      hitCount: Number(session?.hitCount || session?.hit_count || 0),
      running: Boolean(session?.running),
    };
  })()`);
}

async function scanState() {
  return evalJs(`(() => {
    const resultText = document.querySelector('#scan-result')?.textContent || '';
    let scan = null;
    try {
      scan = resultText.trim() ? JSON.parse(resultText) : null;
    } catch (_error) {
      scan = null;
    }
    const rows = [...document.querySelectorAll('#event-log tr')].map((row) => row.textContent || '');
    const cards = [...document.querySelectorAll('.target-card')].map((card, index) => ({
      index,
      title: card.querySelector('.target-text strong')?.textContent || '',
      hitText: card.querySelector('.target-hit-badge')?.textContent || '',
      hitCount: Number(card.querySelector('.target-hit-badge')?.textContent || 0),
      selected: card.classList.contains('is-selected'),
      hasImage: card.querySelector('.target-thumb')?.classList.contains('has-image') || false,
    }));
    return {
      status: document.querySelector('#status')?.textContent || '',
      logRows: rows,
      scanRows: rows.filter((text) => text.includes('Ready -') && text.includes('hits')),
      scan,
      hitCount: Number(scan?.hitCount || scan?.hit_count || 0),
      skippedWindows: Number(scan?.skippedWindows || scan?.skipped_windows || 0),
      skippedWindowApps: Number(scan?.skippedWindowApps || scan?.skipped_window_apps || 0),
      windowResultCount: Array.isArray(scan?.windows) ? scan.windows.length : 0,
      regionResultCount: Array.isArray(scan?.regions) ? scan.regions.length : 0,
      cards,
    };
  })()`);
}

async function layoutState() {
  return evalJs(`(() => {
    const rectFor = (selector) => {
      const item = document.querySelector(selector);
      if (!item) return null;
      const rect = item.getBoundingClientRect();
      return {
        left: Math.round(rect.left),
        top: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
        right: Math.round(rect.right),
        bottom: Math.round(rect.bottom),
      };
    };
    const styleFor = (selector) => {
      const item = document.querySelector(selector);
      if (!item) return null;
      const style = getComputedStyle(item);
      return {
        resize: style.resize,
        overflow: style.overflow,
        overflowX: style.overflowX,
        overflowY: style.overflowY,
        height: Math.round(item.getBoundingClientRect().height),
      };
    };
    const targetButtons = [...document.querySelectorAll('.target-toolbar button')].map((button) => ({
      text: button.textContent,
      clientWidth: button.clientWidth,
      scrollWidth: button.scrollWidth,
      clipped: button.scrollWidth > button.clientWidth + 1,
    }));
    const horizontalOverflow = document.documentElement.scrollWidth > window.innerWidth + 1 ||
      document.body.scrollWidth > window.innerWidth + 1;
    return {
      status: document.querySelector('#status')?.textContent || '',
      viewport: { width: window.innerWidth, height: window.innerHeight },
      appGrid: rectFor('#app-grid'),
      targetPanel: rectFor('.target-panel'),
      controlPanel: rectFor('.control-panel'),
      previewPanel: rectFor('.preview-panel'),
      targetList: rectFor('#profile-targets'),
      logPanel: rectFor('.log-panel'),
      splitters: {
        targetsControls: rectFor('[data-splitter="targets-controls"]'),
        controlsPreview: rectFor('[data-splitter="controls-preview"]'),
        targetsLog: rectFor('[data-splitter="targets-log"]'),
        control0: rectFor('[data-splitter="control-0"]'),
        control1: rectFor('[data-splitter="control-1"]'),
        control2: rectFor('[data-splitter="control-2"]'),
        control3: rectFor('[data-splitter="control-3"]'),
      },
      controlGroups: [...document.querySelectorAll('.control-panel .control-group')].map((item) => {
        const rect = item.getBoundingClientRect();
        const style = getComputedStyle(item);
        return {
          title: item.querySelector('h2')?.textContent || '',
          width: Math.round(rect.width),
          height: Math.round(rect.height),
          resize: style.resize,
          overflow: style.overflow,
          overflowX: style.overflowX,
          overflowY: style.overflowY,
        };
      }),
      firstControlGroup: styleFor('.control-panel .control-group:not(.run-group)'),
      targetButtons,
      horizontalOverflow,
      bodyResizeClass: document.body.classList.contains('is-resizing-layout'),
      storedLayout: (() => {
        try {
          return JSON.parse(localStorage.getItem('screen-watch-ocr-tauri:workbench-layout:v1') || '{}');
        } catch (_error) {
          return {};
        }
      })(),
    };
  })()`);
}

async function scrollProfileTargetsIntoView() {
  await evalJs(`(() => {
    const target = document.querySelector('#profile-targets') || document.querySelector('#profile-summary');
    target?.scrollIntoView({ block: 'center' });
    window.dispatchEvent(new Event('scroll'));
    return {
      scrollY: window.scrollY,
      targetTop: target?.getBoundingClientRect().top ?? null,
    };
  })()`);
  await sleep(500);
}

async function selectVisualSources() {
  return evalJs(`(() => {
    const monitorChecks = [...document.querySelectorAll('#monitors input[type="checkbox"]:not(:disabled)')];
    if (!monitorChecks.some((item) => item.checked) && monitorChecks[0]) {
      monitorChecks[0].checked = true;
      monitorChecks[0].dispatchEvent(new Event('change', { bubbles: true }));
    }
    const labels = [...document.querySelectorAll('#windows label')];
    const helperTitle = ${JSON.stringify(helperTitle)};
    const label = labels.find((item) => item.textContent.includes(helperTitle)) || labels[0];
    if (!label) {
      return { ok: false, reason: 'no window source labels' };
    }
    const checkbox = label.querySelector('input[type="checkbox"]');
    if (!checkbox.checked) {
      checkbox.checked = true;
      checkbox.dispatchEvent(new Event('change', { bubbles: true }));
    }
    return {
      ok: true,
      selectedWindow: label.textContent,
      monitorCount: monitorChecks.length,
    };
  })()`);
}

async function selectOnlyHelperWindowSource() {
  return evalJs(`(() => {
    const monitorChecks = [...document.querySelectorAll('#monitors input[type="checkbox"]:not(:disabled)')];
    for (const checkbox of monitorChecks) {
      if (checkbox.checked) {
        checkbox.checked = false;
        checkbox.dispatchEvent(new Event('change', { bubbles: true }));
      }
    }
    const remembered = document.querySelector('#profile-use-remembered-windows');
    if (remembered && !remembered.checked) {
      remembered.checked = true;
      remembered.dispatchEvent(new Event('change', { bubbles: true }));
    }
    const labels = [...document.querySelectorAll('#windows label')];
    const helperTitle = ${JSON.stringify(helperTitle)};
    const label = labels.find((item) => item.textContent.includes(helperTitle));
    if (!label) {
      return { ok: false, reason: 'helper window source not found' };
    }
    for (const item of labels) {
      const checkbox = item.querySelector('input[type="checkbox"]');
      const shouldCheck = item === label;
      if (checkbox && checkbox.checked !== shouldCheck) {
        checkbox.checked = shouldCheck;
        checkbox.dispatchEvent(new Event('change', { bubbles: true }));
      }
    }
    return {
      ok: true,
      selectedWindow: label.textContent,
      monitorCount: monitorChecks.length,
    };
  })()`);
}

async function runSourcePreviewGate() {
  log("running source preview visual gate");
  await waitForReadyStatus();
  await clickSelector("#refresh-windows");
  await waitForReadyStatus();
  const selected = await waitFor(async () => {
    const result = await selectVisualSources();
    return result.ok ? result : null;
  }, "helper window source in app window list", 20000, 500);
  log("selected visual sources", selected);
  await evalJs(`(() => {
    document.querySelector('#source-previews')?.scrollIntoView({ block: 'center' });
    window.dispatchEvent(new Event('scroll'));
    return window.scrollY;
  })()`);
  await sleep(900);
  await clickSelector("#refresh-source-previews");
  await waitFor(async () => {
    const state = await sourcePreviewState();
    const hasScreen = state.cards.some((card) => card.key.startsWith("screen:"));
    const hasWindow = state.cards.some((card) => card.key.startsWith("app:"));
    const hasError = state.cards.some((card) => card.isError);
    const rendered = state.cards.every((card) => card.hasImage || card.usesDwm);
    return hasScreen && hasWindow && !hasError && rendered
      ? state
      : null;
  }, "screen and window preview cards without errors", 25000, 500);
  const firstState = await sourcePreviewState();
  const initialScreenshot = await captureScreenshot("webview-source-preview-initial");

  await resizeAppWindow(900, 620);
  await sleep(750);
  await evalJs(`(() => {
    const target = document.querySelector('#source-previews');
    const y = target.getBoundingClientRect().top + window.scrollY + 48;
    window.scrollTo(0, Math.max(0, y));
    window.dispatchEvent(new Event('scroll'));
    return { scrollY: window.scrollY, top: target.getBoundingClientRect().top };
  })()`);
  await sleep(750);
  const partialState = await sourcePreviewState();
  const partialScreenshot = await captureScreenshot("webview-source-preview-partial-scroll");
  await evalJs(`(() => {
    document.querySelector('#source-previews')?.scrollIntoView({ block: 'center' });
    window.dispatchEvent(new Event('scroll'));
    return window.scrollY;
  })()`);
  await sleep(750);
  await clickSelector("#refresh-source-previews");
  const restoredState = await waitFor(async () => {
    const state = await sourcePreviewState();
    const hasError = state.cards.some((card) => card.isError);
    const rendered = state.cards.every((card) => card.hasImage || card.usesDwm);
    return !hasError && rendered ? state : null;
  }, "restored source previews without errors", 25000, 500);
  const restoredScreenshot = await captureScreenshot("webview-source-preview-restored");

  const result = {
    status: "pass",
    selected,
    firstState,
    partialState,
    restoredState,
    screenshots: [initialScreenshot, partialScreenshot, restoredScreenshot],
  };
  gateResults.source = result;
  return result;
}

function pngBuffer(width, height, color) {
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    const row = y * (width * 4 + 1);
    raw[row] = 0;
    for (let x = 0; x < width; x += 1) {
      const offset = row + 1 + x * 4;
      raw[offset] = color[0];
      raw[offset + 1] = color[1];
      raw[offset + 2] = color[2];
      raw[offset + 3] = 255;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from("89504e470d0a1a0a", "hex"),
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", zlib.deflateSync(raw)),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

function pngChunk(type, data) {
  const typeBuffer = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

let crcTable = null;
function crc32(buffer) {
  if (!crcTable) {
    crcTable = Array.from({ length: 256 }, (_, index) => {
      let c = index;
      for (let k = 0; k < 8; k += 1) {
        c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      }
      return c >>> 0;
    });
  }
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = crcTable[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function createInputImages() {
  ensureDir(inputDir);
  const specs = [
    ["target-red.png", [210, 62, 62]],
    ["target-green.png", [45, 150, 95]],
    ["target-blue.png", [55, 107, 210]],
    ["target-gold.png", [218, 162, 48]],
  ];
  return specs.map(([name, color]) => {
    const file = path.join(inputDir, name);
    fs.writeFileSync(file, pngBuffer(96, 64, color));
    return file;
  });
}

async function importProfileImages(imagePaths) {
  const pathText = imagePaths.join("\n");
  await evalJs(`(() => {
    const input = document.querySelector('#profile-import-paths');
    input.value = ${JSON.stringify(pathText)};
    input.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('#profile-add-paths').click();
    return true;
  })()`);
  return waitFor(async () => {
    const state = await galleryState();
    return state.cards.length >= imagePaths.length && state.status.startsWith("Ready -")
      ? state
      : null;
  }, "imported profile image targets", 25000, 500);
}

function profilePath() {
  return path.join(localAppData, "ScreenWatchOCR", "profiles", "profile_1.json");
}

function alertEvidenceState() {
  const dataDir = path.join(localAppData, "ScreenWatchOCR");
  const alertsPath = path.join(dataDir, "alerts.jsonl");
  const screenshotsDir = path.join(dataDir, "screenshots");
  const alertLines = fs.existsSync(alertsPath)
    ? fs.readFileSync(alertsPath, "utf8").split(/\r?\n/).filter(Boolean)
    : [];
  const screenshots = fs.existsSync(screenshotsDir)
    ? fs.readdirSync(screenshotsDir).filter((name) => name.toLowerCase().endsWith(".png"))
    : [];
  let profileTargets = [];
  if (fs.existsSync(profilePath())) {
    try {
      const profile = JSON.parse(fs.readFileSync(profilePath(), "utf8"));
      profileTargets = Array.isArray(profile.targets) ? profile.targets : [];
    } catch (_error) {
      profileTargets = [];
    }
  }
  return {
    alertsPath,
    screenshotsDir,
    alertLineCount: alertLines.length,
    alertLines,
    screenshotCount: screenshots.length,
    screenshots,
    profileHitCounts: profileTargets.map((target) =>
      Number(target.hit_count ?? target.hitCount ?? 0),
    ),
  };
}

function setFirstTargetHitCount(count) {
  const file = profilePath();
  const profile = JSON.parse(fs.readFileSync(file, "utf8"));
  if (Array.isArray(profile.targets) && profile.targets[0]) {
    profile.targets[0].hit_count = count;
  }
  fs.writeFileSync(file, `${JSON.stringify(profile, null, 2)}\n`, "utf8");
  return file;
}

async function waitForCardCount(count) {
  return waitFor(async () => {
    const state = await galleryState();
    return state.cards.length === count ? state : null;
  }, `${count} profile target card(s)`, 25000, 500);
}

async function setMonitoringSmokeInputs() {
  await evalJs(`(() => {
    const setValue = (selector, value) => {
      const input = document.querySelector(selector);
      if (!input) return;
      input.value = value;
      input.dispatchEvent(new Event('change', { bubbles: true }));
    };
    setValue('#profile-interval-ms', '500');
    setValue('#profile-cooldown', '0');
    setValue('#profile-threshold', '0.9');
    setValue('#profile-scales', '1.0');
    setValue('#profile-max-templates', '10');
    setValue('#profile-max-alerts', '10');
    const beep = document.querySelector('#profile-beep');
    if (beep?.checked) {
      beep.checked = false;
      beep.dispatchEvent(new Event('change', { bubbles: true }));
    }
    return true;
  })()`);
}

async function clearAllProfileTargetsForSmoke() {
  await evalJs(`(() => {
    window.confirm = () => true;
    document.querySelector('#profile-clear-all')?.click();
    return true;
  })()`);
  await waitForCardCount(0);
}

async function waitForMonitoringStart(description) {
  return waitFor(async () => {
    const state = await monitoringState();
    return state.buttonText === "停止监控" &&
      !state.buttonDisabled &&
      state.running &&
      state.generation > 0 &&
      state.tickCount > 0 &&
      state.hitCount > 0 &&
      state.progressRows.length > 0
      ? state
      : null;
  }, description, 35000, 500);
}

async function stopMonitoringFromUi(description) {
  await clickSelector("#profile-monitor-start");
  return waitFor(async () => {
    const state = await monitoringState();
    return state.buttonText === "开始监控" && !state.buttonDisabled && !state.running
      ? state
      : null;
  }, description, 20000, 300);
}

async function runOneShotScanGate() {
  log("running profile one-shot scan gate");
  await waitForReadyStatus();
  await clickSelector("#refresh-windows");
  await waitForReadyStatus();
  const selected = await waitFor(async () => {
    const result = await selectOnlyHelperWindowSource();
    return result.ok ? result : null;
  }, "exclusive helper window source for one-shot scan", 20000, 500);
  await setMonitoringSmokeInputs();
  await clearAllProfileTargetsForSmoke();
  await clickSelector("#profile-capture-target");
  const capturedTargetState = await waitForCardCount(1);
  await scrollProfileTargetsIntoView();
  const preparedScreenshot = await captureScreenshot("profile-one-shot-scan-prepared");

  await clickSelector("#profile-scan-once");
  const hitState = await waitFor(async () => {
    const state = await scanState();
    const cardHit = state.cards.some((card) => card.hitCount > 0);
    return state.hitCount > 0 &&
      state.windowResultCount > 0 &&
      state.skippedWindows === 0 &&
      state.skippedWindowApps === 0 &&
      state.scanRows.length > 0 &&
      cardHit
      ? state
      : null;
  }, "profile one-shot scan hit through visible UI", 30000, 500);

  const evidenceState = await waitFor(async () => {
    const state = alertEvidenceState();
    const profileHit = state.profileHitCounts.some((count) => count > 0);
    return state.alertLineCount > 0 && state.screenshotCount > 0 && profileHit
      ? state
      : null;
  }, "one-shot scan alert evidence files", 10000, 250);
  await scrollProfileTargetsIntoView();
  const hitScreenshot = await captureScreenshot("profile-one-shot-scan-hit");

  const result = {
    status: "pass",
    selected,
    capturedTargetState,
    hitState,
    evidenceState,
    screenshots: [preparedScreenshot, hitScreenshot],
  };
  gateResults.scan = result;
  return result;
}

async function runMonitoringGate() {
  log("running profile monitoring restart gate");
  await waitForReadyStatus();
  await clickSelector("#refresh-windows");
  await waitForReadyStatus();
  const selected = await waitFor(async () => {
    const result = await selectOnlyHelperWindowSource();
    return result.ok ? result : null;
  }, "exclusive helper window source", 20000, 500);
  await setMonitoringSmokeInputs();
  await clearAllProfileTargetsForSmoke();
  await clickSelector("#profile-capture-target");
  const capturedTargetState = await waitForCardCount(1);
  await scrollProfileTargetsIntoView();
  const preparedScreenshot = await captureScreenshot("profile-monitoring-prepared");

  await clickSelector("#profile-monitor-start");
  const firstRunState = await waitForMonitoringStart("first monitoring run with hits");
  await sleep(2200);
  const firstProgressState = await waitFor(async () => {
    const state = await monitoringState();
    return state.progressRows.length >= 2 && state.tickCount > firstRunState.tickCount
      ? state
      : null;
  }, "heartbeat progress log rows during first run", 15000, 500);
  const firstRunScreenshot = await captureScreenshot("profile-monitoring-running");
  const firstStopState = await stopMonitoringFromUi("first monitoring stop");

  await sleep(100);
  await clickSelector("#profile-monitor-start");
  const secondRunState = await waitForMonitoringStart("second monitoring run after stop");
  if (secondRunState.generation <= firstRunState.generation) {
    throw new Error(`monitoring restart reused stale generation ${secondRunState.generation} after ${firstRunState.generation}`);
  }
  const secondProgressState = await waitFor(async () => {
    const state = await monitoringState();
    return state.progressRows.length > firstProgressState.progressRows.length &&
      state.tickCount > secondRunState.tickCount
      ? state
      : null;
  }, "heartbeat progress log rows during second run", 15000, 500);
  const secondStopState = await stopMonitoringFromUi("second monitoring stop");

  const result = {
    status: "pass",
    selected,
    capturedTargetState,
    firstRunState,
    firstProgressState,
    firstStopState,
    secondRunState,
    secondProgressState,
    secondStopState,
    screenshots: [preparedScreenshot, firstRunScreenshot],
  };
  gateResults.monitoring = result;
  return result;
}

function monitoringSample(elapsedMs, state) {
  return {
    elapsedMs,
    generation: state.generation,
    hitCount: state.hitCount,
    progressRows: state.progressRows.length,
    status: state.status,
    tickCount: state.tickCount,
  };
}

async function runMonitoringSoakGate() {
  log(`running profile monitoring soak gate for ${monitoringSoakMs}ms`);
  await waitForReadyStatus();
  await clickSelector("#refresh-windows");
  await waitForReadyStatus();
  const selected = await waitFor(async () => {
    const result = await selectOnlyHelperWindowSource();
    return result.ok ? result : null;
  }, "exclusive helper window source for monitoring soak", 20000, 500);
  await setMonitoringSmokeInputs();
  await clearAllProfileTargetsForSmoke();
  await clickSelector("#profile-capture-target");
  const capturedTargetState = await waitForCardCount(1);
  await scrollProfileTargetsIntoView();
  const preparedScreenshot = await captureScreenshot("profile-monitoring-soak-prepared");

  let monitorStarted = false;
  let stoppedState = null;
  try {
    await clickSelector("#profile-monitor-start");
    const startState = await waitForMonitoringStart("monitoring soak start with hits");
    monitorStarted = true;
    const startedAt = Date.now();
    const samples = [monitoringSample(0, startState)];
    let midScreenshot = null;
    let capturedMidpoint = false;
    while (Date.now() - startedAt < monitoringSoakMs) {
      const remaining = monitoringSoakMs - (Date.now() - startedAt);
      await sleep(Math.max(250, Math.min(2000, remaining)));
      const state = await monitoringState();
      const elapsedMs = Date.now() - startedAt;
      if (
        state.buttonText !== "停止监控" ||
        state.buttonDisabled ||
        !state.running ||
        state.generation !== startState.generation
      ) {
        throw new Error(`monitoring soak became unhealthy at ${elapsedMs}ms: ${JSON.stringify(state)}`);
      }
      samples.push(monitoringSample(elapsedMs, state));
      if (!capturedMidpoint && elapsedMs >= monitoringSoakMs / 2) {
        midScreenshot = await captureScreenshot("profile-monitoring-soak-mid");
        capturedMidpoint = true;
      }
    }

    const endState = await monitoringState();
    const tickDelta = endState.tickCount - startState.tickCount;
    const hitDelta = endState.hitCount - startState.hitCount;
    const progressDelta = endState.progressRows.length - startState.progressRows.length;
    const distinctTickCounts = new Set(samples.map((sample) => sample.tickCount)).size;
    const minTickDelta = Math.max(6, Math.floor(monitoringSoakMs / 3000));
    const minDistinctTicks = Math.max(4, Math.floor(monitoringSoakMs / 6000));
    if (tickDelta < minTickDelta) {
      throw new Error(`monitoring soak tick delta ${tickDelta} below expected ${minTickDelta}`);
    }
    if (hitDelta < minTickDelta) {
      throw new Error(`monitoring soak hit delta ${hitDelta} below expected ${minTickDelta}`);
    }
    if (distinctTickCounts < minDistinctTicks) {
      throw new Error(`monitoring soak distinct tick samples ${distinctTickCounts} below expected ${minDistinctTicks}`);
    }
    if (progressDelta < Math.min(8, minTickDelta)) {
      throw new Error(`monitoring soak progress log delta ${progressDelta} below expected ${Math.min(8, minTickDelta)}`);
    }
    const runningScreenshot = await captureScreenshot("profile-monitoring-soak-running");
    stoppedState = await stopMonitoringFromUi("monitoring soak stop");
    monitorStarted = false;

    const result = {
      status: "pass",
      selected,
      capturedTargetState,
      durationMs: monitoringSoakMs,
      startState,
      endState,
      stoppedState,
      tickDelta,
      hitDelta,
      progressDelta,
      distinctTickCounts,
      samples,
      screenshots: [
        preparedScreenshot,
        ...(midScreenshot ? [midScreenshot] : []),
        runningScreenshot,
      ],
    };
    gateResults.monitoringSoak = result;
    return result;
  } finally {
    if (monitorStarted) {
      try {
        await stopMonitoringFromUi("monitoring soak cleanup stop");
      } catch (error) {
        log(`monitoring soak cleanup stop failed: ${error.message || error}`);
      }
    }
  }
}

async function runLayoutGate() {
  log("running resizable layout visual gate");
  await waitForReadyStatus();
  await resizeAppWindow(1120, 820);
  await resetStoredWorkbenchLayoutForSmoke();
  await sleep(700);
  const initialState = await layoutState();
  if (initialState.viewport.width < 961) {
    throw new Error(`layout gate requires desktop viewport, got ${initialState.viewport.width}`);
  }
  if (initialState.horizontalOverflow) {
    throw new Error("initial layout has horizontal overflow");
  }
  const missingSplitters = Object.entries(initialState.splitters)
    .filter(([_name, rect]) => !rect || rect.width <= 0 || rect.height <= 0)
    .map(([name]) => name);
  if (missingSplitters.length > 0) {
    throw new Error(`expected all workbench splitters to be present, missing ${missingSplitters.join(", ")}`);
  }

  await dragSelector('[data-splitter="targets-controls"]', { dx: 78, dy: 0 });
  const afterTargetsControls = await waitFor(async () => {
    const state = await layoutState();
    return state.targetPanel.width >= initialState.targetPanel.width + 45 &&
      state.controlPanel.width <= initialState.controlPanel.width - 35 &&
      !state.horizontalOverflow
      ? state
      : null;
  }, "targets/settings splitter drag changed column widths", 8000, 250);

  await dragSelector('[data-splitter="controls-preview"]', { dx: 56, dy: 0 });
  const afterControlsPreview = await waitFor(async () => {
    const state = await layoutState();
    return state.controlPanel.width >= afterTargetsControls.controlPanel.width + 25 &&
      state.previewPanel.width <= afterTargetsControls.previewPanel.width - 20 &&
      !state.horizontalOverflow
      ? state
      : null;
  }, "settings/preview splitter drag changed column widths", 8000, 250);

  await dragSelector('[data-splitter="targets-log"]', { dx: 0, dy: 54 });
  const afterTargetsLog = await waitFor(async () => {
    const state = await layoutState();
    return state.targetList.height >= afterControlsPreview.targetList.height + 35 &&
      state.logPanel.height <= afterControlsPreview.logPanel.height - 30 &&
      !state.horizontalOverflow
      ? state
      : null;
  }, "target list/log splitter drag changed row heights", 8000, 250);

  const firstControlBefore = afterTargetsLog.controlGroups[0];
  const secondControlBefore = afterTargetsLog.controlGroups[1];
  await dragSelector('[data-splitter="control-0"]', { dx: 0, dy: 42 });
  const afterControlSplitter = await waitFor(async () => {
    const state = await layoutState();
    const first = state.controlGroups[0];
    const second = state.controlGroups[1];
    return first.height >= firstControlBefore.height + 25 &&
      second.height <= secondControlBefore.height - 20 &&
      !state.horizontalOverflow
      ? state
      : null;
  }, "control panel splitter drag changed group heights", 8000, 250);

  const screenshot = await captureScreenshot("webview-layout-resized");
  const result = {
    status: "pass",
    initialState,
    afterTargetsControls,
    afterControlsPreview,
    afterTargetsLog,
    afterControlSplitter,
    measurements: {
      targetPanelWidthDelta:
        afterTargetsControls.targetPanel.width - initialState.targetPanel.width,
      controlPanelWidthDelta:
        afterControlsPreview.controlPanel.width - afterTargetsControls.controlPanel.width,
      targetListHeightDelta:
        afterTargetsLog.targetList.height - afterControlsPreview.targetList.height,
      firstControlGroupHeightDelta:
        afterControlSplitter.controlGroups[0].height - firstControlBefore.height,
      secondControlGroupHeightDelta:
        afterControlSplitter.controlGroups[1].height - secondControlBefore.height,
    },
    screenshots: [screenshot],
  };
  gateResults.layout = result;
  return result;
}

async function runGalleryGate() {
  log("running template gallery visual gate");
  await waitForReadyStatus();
  const images = createInputImages();
  const importedState = await importProfileImages(images);
  const oversizedCards = importedState.cards.filter((card) =>
    card.card.width > 52 ||
    card.card.height > 56 ||
    card.thumb.height > 28
  );
  if (oversizedCards.length > 0) {
    throw new Error(`target cards are too large: ${JSON.stringify(oversizedCards)}`);
  }
  setFirstTargetHitCount(7);
  await clickSelector("#profile-load");
  await waitForCardCount(4);
  await scrollProfileTargetsIntoView();
  const importedScreenshot = await captureScreenshot("template-gallery-imported");

  await evalJs(`document.querySelector('.target-card .target-enable-check')?.click()`);
  const toggledOffState = await waitFor(async () => {
    const state = await galleryState();
    return state.cards.some((card) => card.disabled) ? state : null;
  }, "one disabled target", 20000, 500);

  await clickSelector("#profile-toggle-all");
  await waitFor(async () => {
    const state = await galleryState();
    return state.cards.length === 4 && state.status.includes("启用") ? state : null;
  }, "toggle-all profile status", 20000, 500);

  await evalJs(`(() => {
    const cards = [...document.querySelectorAll('.target-card')];
    const firstDown = cards[0]?.querySelectorAll('.target-actions button')[1];
    firstDown?.click();
    return true;
  })()`);
  await sleep(800);
  const rowButtonState = await galleryState();

  await evalJs(`(() => {
    const cards = [...document.querySelectorAll('.target-card')];
    const from = cards[0];
    const to = cards[cards.length - 1];
    const data = new DataTransfer();
    from.dispatchEvent(new DragEvent('dragstart', { bubbles: true, dataTransfer: data }));
    const rect = to.getBoundingClientRect();
    to.dispatchEvent(new DragEvent('dragover', { bubbles: true, dataTransfer: data, clientY: rect.bottom - 3 }));
    to.dispatchEvent(new DragEvent('drop', { bubbles: true, dataTransfer: data, clientY: rect.bottom - 3 }));
    from.dispatchEvent(new DragEvent('dragend', { bubbles: true, dataTransfer: data }));
    return true;
  })()`);
  await sleep(1000);
  const dragDropState = await galleryState();
  await scrollProfileTargetsIntoView();
  const reorderedScreenshot = await captureScreenshot("template-gallery-reordered");

  await scrollProfileTargetsIntoView();
  await evalJs(`(() => {
    const card = document.querySelector('.target-card');
    const rect = card.getBoundingClientRect();
    card.dispatchEvent(new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: rect.left + 24,
      clientY: rect.top + 24,
    }));
    return true;
  })()`);
  const contextState = await waitFor(async () => {
    const state = await galleryState();
    return state.menuVisible ? state : null;
  }, "target context menu", 10000, 250);
  const contextScreenshot = await captureScreenshot("template-gallery-context-menu");

  await evalJs(`(() => {
    const buttons = [...document.querySelectorAll('.target-menu button')];
    const clear = buttons.find((button) => button.textContent.includes('清零'));
    clear?.click();
    return Boolean(clear);
  })()`);
  await sleep(800);
  const clearedHitsState = await galleryState();

  await evalJs(`(() => {
    window.confirm = () => true;
    const card = document.querySelector('.target-card');
    const buttons = [...card.querySelectorAll('.target-actions button')];
    const del = buttons.find((button) => button.textContent === '删除');
    del?.click();
    return Boolean(del);
  })()`);
  const afterDeleteState = await waitForCardCount(3);

  await evalJs(`(() => {
    window.confirm = () => true;
    document.querySelector('#profile-clear-all')?.click();
    return true;
  })()`);
  const afterClearState = await waitForCardCount(0);
  await scrollProfileTargetsIntoView();
  const clearedScreenshot = await captureScreenshot("template-gallery-cleared");

  await selectVisualSources();
  await clickSelector("#profile-capture-target");
  const capturedTargetState = await waitForCardCount(1);
  await scrollProfileTargetsIntoView();
  const capturedScreenshot = await captureScreenshot("template-gallery-captured-source");

  const result = {
    status: "pass",
    images,
    profilePath: profilePath(),
    importedState,
    toggledOffState,
    rowButtonState,
    dragDropState,
    contextState,
    clearedHitsState,
    afterDeleteState,
    afterClearState,
    capturedTargetState,
    screenshots: [
      importedScreenshot,
      reorderedScreenshot,
      contextScreenshot,
      clearedScreenshot,
      capturedScreenshot,
    ],
  };
  gateResults.gallery = result;
  return result;
}

async function runClipboardPasteGate() {
  log("running clipboard paste visual gate");
  await waitForReadyStatus();
  const images = createInputImages();
  const clipboard = await ensureClipboardSession();
  try {
    await clearAllProfileTargetsForSmoke();
    const imageClipboardState = await clipboard.setImage(images[0]);
    log("set clipboard bitmap image", imageClipboardState);
    const imageClickResult = await clickSelector("#profile-paste-images");
    log("clicked paste-images button", { ok: imageClickResult });
    let imagePasteState = null;
    try {
      imagePasteState = await waitFor(async () => {
        const state = await galleryState();
        if (
          state.status.includes("剪贴板") ||
          state.status.includes("cannot decode") ||
          state.status.includes("cannot open clipboard")
        ) {
          throw new Error(`clipboard bitmap paste failed with UI state: ${JSON.stringify(state)}`);
        }
        const card = state.cards[0];
        return state.cards.length === 1 &&
          state.status.includes("导入 1 张") &&
          card?.hasImage &&
          card?.thumb?.width > 0 &&
          card?.thumb?.height > 0
          ? state
          : null;
      }, "clipboard bitmap image paste through visible button", 25000, 500);
    } catch (error) {
      const lastState = await galleryState();
      throw new Error(`${error.message}; last gallery state: ${JSON.stringify(lastState)}`);
    }
    await scrollProfileTargetsIntoView();
    const imagePasteScreenshot = await captureScreenshot("profile-clipboard-image-paste");

    await clearAllProfileTargetsForSmoke();
    const fileClipboardState = await clipboard.setFiles([images[1]]);
    log("set clipboard file list", fileClipboardState);
    await pressCtrlV();
    let fileDropPasteState = null;
    try {
      fileDropPasteState = await waitFor(async () => {
        const state = await galleryState();
        if (
          state.status.includes("剪贴板") ||
          state.status.includes("cannot decode") ||
          state.status.includes("cannot open clipboard")
        ) {
          throw new Error(`clipboard file-list paste failed with UI state: ${JSON.stringify(state)}`);
        }
        const card = state.cards[0];
        return state.cards.length === 1 &&
          state.status.includes("导入 1 张") &&
          card?.hasImage &&
          card?.thumb?.width > 0 &&
          card?.thumb?.height > 0
          ? state
          : null;
      }, "clipboard file-list paste through Ctrl+V", 25000, 500);
    } catch (error) {
      const lastState = await galleryState();
      throw new Error(`${error.message}; last gallery state: ${JSON.stringify(lastState)}`);
    }
    await scrollProfileTargetsIntoView();
    const fileDropPasteScreenshot = await captureScreenshot("profile-clipboard-file-paste");

    const result = {
      status: "pass",
      images,
      imageClipboardState,
      imageClickResult,
      imagePasteState,
      fileClipboardState,
      fileDropPasteState,
      screenshots: [imagePasteScreenshot, fileDropPasteScreenshot],
    };
    gateResults.clipboard = result;
    return result;
  } finally {
    await clipboard.restore();
    clipboardSession = null;
  }
}

function relativeList(files) {
  return files.map((file) => path.relative(windowsProjectRoot, file)).join("; ");
}

function evidenceRecord({ gateTitle, status, observed, evidenceFiles, remainingRisk }) {
  const buildInfo = readBuildInfo();
  const releaseHash = buildInfo.executableSha256
    ? `executableSha256=${buildInfo.executableSha256}; buildInfo=${path.relative(windowsProjectRoot, buildInfo.path)}`
    : `build-info unavailable at ${buildInfo.path}`;
  return [
    `Gate: ${gateTitle}`,
    `Completion status: ${status}`,
    `Date/time: ${new Date().toISOString()}`,
    `Machine: ${os.hostname()}`,
    "Worktree note: screen-watch-ocr-tauri is not a git repository",
    `Command(s) and exit code(s): node scripts/webview-visual-smoke.mjs --gate ${gateMode}; exit 0`,
    `Release build-info hash: ${releaseHash}`,
    `Model/image/evidence dirs: inputDir=${inputDir}; localAppData=${localAppData}; evidenceLogDir=${evidenceLogDir}`,
    `Observed result: ${observed}`,
    `Evidence files: ${relativeList(evidenceFiles)}`,
    "Cleanup performed: stopped the test-owned app process and helper window process; retained isolated target/webview-visual-smoke run directory and evidence screenshots/logs for audit",
    `Remaining risk: ${remainingRisk}`,
    "",
  ].join("\n");
}

function writeEvidenceRecords(summary) {
  ensureDir(evidenceDir);
  if (summary.gates.source?.status === "pass") {
    fs.writeFileSync(
      path.join(evidenceDir, "webview-source-preview-visual-smoke.md"),
      evidenceRecord({
        gateTitle: "WebView Source Preview Visual Smoke",
        status: "pass",
        observed:
          "automated real WebView2/CDP smoke selected a physical screen and visible helper app-window source, refreshed source cards without unexpected failed previews, captured bitmap/DWM-backed cards, scrolled a preview card partially offscreen, resized the app window, restored the preview area, and refreshed again without stale/error cards",
        evidenceFiles: [
          resultPath,
          appLogPath,
          ...summary.gates.source.screenshots,
        ],
        remainingRisk:
          "proves the current packaged WebView2/runtime path on this interactive desktop; it does not prove every possible monitor topology or every third-party app window",
      }),
      "utf8",
    );
  }
  if (summary.gates.gallery?.status === "pass") {
    fs.writeFileSync(
      path.join(evidenceDir, "template-gallery-visual-workflow-smoke.md"),
      evidenceRecord({
        gateTitle: "Template Gallery Visual Workflow Smoke",
        status: "pass",
        observed:
          "automated real WebView2/CDP smoke imported four generated PNG templates into an isolated profile, preserved thumbnail geometry and selection, toggled target enablement, exercised select-all/invert, used row-button reorder, exercised drag/drop reorder, opened the hit-count context menu and cleared hits, deleted one target, cleared all targets, and captured the current source as a new template",
        evidenceFiles: [
          resultPath,
          appLogPath,
          ...summary.gates.gallery.screenshots,
        ],
        remainingRisk:
          "proves the current packaged WebView2/gallery workflow against isolated generated images and a real screen capture source; it does not prove every user image codec or long-running manual editing session",
      }),
      "utf8",
    );
  }
  if (summary.gates.clipboard?.status === "pass") {
    fs.writeFileSync(
      path.join(evidenceDir, "profile-clipboard-paste-smoke.md"),
      evidenceRecord({
        gateTitle: "Profile Clipboard Paste Smoke",
        status: "pass",
        observed:
          "automated real WebView2/CDP smoke saved the user's clipboard object, pasted a generated bitmap through the visible paste-images button, pasted a generated image file list through Ctrl+V, verified each paste created a selected template card with rendered thumbnail geometry, and restored the saved clipboard object before exit",
        evidenceFiles: [
          resultPath,
          appLogPath,
          ...summary.gates.clipboard.screenshots,
        ],
        remainingRisk:
          "proves CF_DIB bitmap paste and CF_HDROP image-file paste on this Windows interactive desktop; it does not exhaustively prove every clipboard producer application or every image codec",
      }),
      "utf8",
    );
  }
  if (summary.gates.scan?.status === "pass") {
    fs.writeFileSync(
      path.join(evidenceDir, "profile-one-shot-scan-smoke.md"),
      evidenceRecord({
        gateTitle: "Profile One Shot Scan Smoke",
        status: "pass",
        observed:
          "automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, clicked the visible profile scan-once button, observed a positive hit count and log row in the UI, verified the target hit badge/profile hit_count updated, and confirmed alerts.jsonl plus screenshot evidence were written",
        evidenceFiles: [
          resultPath,
          appLogPath,
          ...summary.gates.scan.screenshots,
        ],
        remainingRisk:
          "proves the packaged visible one-shot scan path on this Windows interactive desktop with a generated stable window source; it does not prove every third-party window capture implementation or every production image corpus",
      }),
      "utf8",
    );
  }
  if (summary.gates.monitoring?.status === "pass") {
    fs.writeFileSync(
      path.join(evidenceDir, "profile-monitoring-restart-smoke.md"),
      evidenceRecord({
        gateTitle: "Profile Monitoring Restart Smoke",
        status: "pass",
        observed:
          "automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, started profile monitoring, observed ticking progress log rows and positive hit counts, stopped monitoring through the main run button, started monitoring again, observed a second running session, and stopped cleanly with the button restored to start",
        evidenceFiles: [
          resultPath,
          appLogPath,
          ...summary.gates.monitoring.screenshots,
        ],
        remainingRisk:
          "proves start/stop/restart and progress logging on this Windows interactive desktop with a generated stable window source; it does not prove every third-party window capture implementation or every long-running production workload",
      }),
      "utf8",
    );
  }
  if (summary.gates.monitoringSoak?.status === "pass") {
    fs.writeFileSync(
      path.join(evidenceDir, "profile-monitoring-soak-smoke.md"),
      evidenceRecord({
        gateTitle: "Profile Monitoring Soak Smoke",
        status: "pass",
        observed:
          `automated real WebView2/CDP smoke selected only the generated helper app-window source, captured that source as a template, ran profile monitoring for ${summary.gates.monitoringSoak.durationMs}ms, sampled ${summary.gates.monitoringSoak.samples.length} UI states, observed tick delta ${summary.gates.monitoringSoak.tickDelta}, hit delta ${summary.gates.monitoringSoak.hitDelta}, progress-log delta ${summary.gates.monitoringSoak.progressDelta}, and stopped cleanly with the button restored to start`,
        evidenceFiles: [
          resultPath,
          appLogPath,
          ...summary.gates.monitoringSoak.screenshots,
        ],
        remainingRisk:
          "proves a sustained packaged WebView2 monitoring run on this Windows interactive desktop with a generated stable window source; it is still not a multi-hour production soak or an exhaustive third-party window capture matrix",
      }),
      "utf8",
    );
  }
  if (summary.gates.layout?.status === "pass") {
    fs.writeFileSync(
      path.join(evidenceDir, "webview-layout-resize-smoke.md"),
      evidenceRecord({
        gateTitle: "WebView Layout Resize Smoke",
        status: "pass",
        observed:
          "automated real WebView2/CDP smoke dragged the target/settings splitter, settings/preview splitter, target-list/log splitter, and a control-panel group splitter; each drag produced measured dimension changes without horizontal overflow",
        evidenceFiles: [
          resultPath,
          appLogPath,
          ...summary.gates.layout.screenshots,
        ],
        remainingRisk:
          "proves the current packaged WebView2 layout resize path on this desktop viewport; it does not exhaustively cover every DPI scale or very narrow mobile layout",
      }),
      "utf8",
    );
  }
}

async function main() {
  if (process.platform !== "win32") {
    throw new Error("webview visual smoke requires Windows/WebView2");
  }
  ensureDir(evidenceLogDir);
  ensureDir(runRoot);
  ensureDir(localAppData);
  fs.writeFileSync(appLogPath, `webview visual smoke ${runStamp}\n`, "utf8");

  helperProcess = startHelperWindow();
  appProcess = startApp();
  await sleep(1000);
  const target = await waitForWebviewTarget();
  cdp = new CdpClient(target.webSocketDebuggerUrl);
  await cdp.connect();
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  await cdp.send("Page.bringToFront");
  await resizeAppWindow(1120, 820);
  await waitForReadyStatus();

  if (gateMode === "all" || gateMode === "source") {
    await runSourcePreviewGate();
  }
  if (gateMode === "all" || gateMode === "gallery") {
    await runGalleryGate();
  }
  if (gateMode === "all" || gateMode === "clipboard") {
    await runClipboardPasteGate();
  }
  if (gateMode === "all" || gateMode === "scan") {
    await runOneShotScanGate();
  }
  if (gateMode === "all" || gateMode === "monitoring") {
    await runMonitoringGate();
  }
  if (gateMode === "monitoring-soak" || gateMode === "soak") {
    await runMonitoringSoakGate();
  }
  if (gateMode === "all" || gateMode === "layout") {
    await runLayoutGate();
  }

  const summary = {
    runStamp,
    exePath,
    buildInfo: readBuildInfo(),
    cdpPort,
    singleInstancePort,
    helperTitle,
    runRoot,
    localAppData,
    appLogPath,
    resultPath,
    screenshots,
    gates: gateResults,
    logs,
  };
  fs.writeFileSync(resultPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  if (shouldWriteRecords) {
    writeEvidenceRecords(summary);
  }
  console.log(JSON.stringify({
    webviewVisualSmoke: "passed",
    resultPath,
    source: gateResults.source?.status || "skipped",
    gallery: gateResults.gallery?.status || "skipped",
    clipboard: gateResults.clipboard?.status || "skipped",
    scan: gateResults.scan?.status || "skipped",
    monitoring: gateResults.monitoring?.status || "skipped",
    monitoringSoak: gateResults.monitoringSoak?.status || "skipped",
    layout: gateResults.layout?.status || "skipped",
  }, null, 2));
}

main()
  .catch((error) => {
    const failure = {
      runStamp,
      error: error.stack || error.message,
      gates: gateResults,
      logs,
      screenshots,
      appLogPath,
    };
    ensureDir(evidenceLogDir);
    fs.writeFileSync(resultPath, `${JSON.stringify(failure, null, 2)}\n`, "utf8");
    console.error(error.stack || error.message);
    process.exitCode = 1;
  })
  .finally(async () => {
    if (clipboardSession) {
      try {
        await clipboardSession.restore();
      } catch (error) {
        fs.appendFileSync(
          appLogPath,
          `[clipboard restore failed] ${error.stack || error.message}\n`,
        );
      }
    }
    try {
      cdp?.close();
    } catch (_error) {
      // best effort
    }
    if (appProcess && !appProcess.killed) {
      appProcess.kill("SIGKILL");
    }
    if (helperProcess && !helperProcess.killed) {
      helperProcess.kill("SIGKILL");
    }
    await sleep(300);
  });
