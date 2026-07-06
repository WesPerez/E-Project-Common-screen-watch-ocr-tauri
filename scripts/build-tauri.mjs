import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  readdirSync,
  rmSync,
  statSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";

const flavor = process.argv[2] === "full" ? "full" : "lite";
const args = ["tauri", "build"];
if (flavor === "full") {
  args.push("--features", "ocr");
}
const result = spawnSync("npx", args, {
  stdio: "inherit",
  shell: process.platform === "win32",
  env: {
    ...process.env,
    SCREENWATCH_BUILD_FLAVOR: flavor,
  },
});

if (result.status === 0) {
  const exePath = join("target", "release", "screen-watch-ocr-tauri.exe");
  const exe = statSync(exePath);
  const executableSha256 = createHash("sha256")
    .update(readFileSync(exePath))
    .digest("hex");
  const buildInfo = {
    executable: "screen-watch-ocr-tauri.exe",
    flavor,
    executableBytes: exe.size,
    executableSha256,
    builtUtc: new Date().toISOString(),
    buildFlavorEnv: "SCREENWATCH_BUILD_FLAVOR",
    compiledBuildFlavorEnv: "SCREENWATCH_COMPILED_BUILD_FLAVOR",
  };
  const buildInfoText = `${JSON.stringify(buildInfo, null, 2)}\n`;
  writeFileSync(join("target", "release", "screen-watch-ocr-tauri.build-info.json"), buildInfoText);
  writeFileSync(
    join("target", "release", `screen-watch-ocr-tauri.${flavor}.build-info.json`),
    buildInfoText,
  );

  const nsisDir = join("target", "release", "bundle", "nsis");
  if (existsSync(nsisDir)) {
    for (const name of readdirSync(nsisDir)) {
      if (/^Screen Watch OCR_\d+\.\d+\.\d+_x64(?:-(?:lite|full))?-setup\.exe$/.test(name)) {
        rmSync(join(nsisDir, name));
      }
    }
    const setupName = readdirSync(nsisDir).find(
      (name) => name.endsWith("_x64-setup.exe") && !name.includes(`-${flavor}-setup.exe`),
    );
    if (setupName) {
      const flavorSetupName = setupName.replace("_x64-setup.exe", `_x64-${flavor}-setup.exe`);
      copyFileSync(join(nsisDir, setupName), join(nsisDir, flavorSetupName));
    }
  }
}

process.exit(result.status ?? 1);
