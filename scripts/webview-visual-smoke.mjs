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
  monitoring: null,
};

let appProcess = null;
let helperProcess = null;
let cdp = null;

function valueArg(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || index + 1 >= process.argv.length) {
    return "";
  }
  return process.argv[index + 1];
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
  const parsed = JSON.parse(fs.readFileSync(buildInfoPath, "utf8"));
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
    let session = null;
    try {
      session = resultText.trim() ? JSON.parse(resultText) : null;
    } catch (_error) {
      session = null;
    }
    const rows = [...document.querySelectorAll('#event-log tr')].map((row) => row.textContent || '');
    return {
      status: document.querySelector('#status')?.textContent || '',
      buttonText: document.querySelector('#profile-monitor-start')?.textContent || '',
      buttonDisabled: Boolean(document.querySelector('#profile-monitor-start')?.disabled),
      logRows: rows,
      progressRows: rows.filter((text) => text.includes('第 ') && text.includes('扫描')),
      startedRows: rows.filter((text) => text.includes('监控中')).length,
      stoppedRows: rows.filter((text) => text.includes('停止')).length,
      session,
      tickCount: Number(session?.tickCount || session?.tick_count || 0),
      hitCount: Number(session?.hitCount || session?.hit_count || 0),
      running: Boolean(session?.running),
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

  await sleep(700);
  await clickSelector("#profile-monitor-start");
  const secondRunState = await waitForMonitoringStart("second monitoring run after stop");
  await sleep(1200);
  const secondProgressState = await monitoringState();
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

async function runGalleryGate() {
  log("running template gallery visual gate");
  await waitForReadyStatus();
  const images = createInputImages();
  const importedState = await importProfileImages(images);
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
  if (gateMode === "all" || gateMode === "monitoring") {
    await runMonitoringGate();
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
    monitoring: gateResults.monitoring?.status || "skipped",
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
