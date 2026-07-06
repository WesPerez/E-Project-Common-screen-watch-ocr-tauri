param(
    [switch]$RustDebug
)

$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRootPath = (Resolve-Path (Join-Path $ScriptRoot "..")).Path
$PythonProjectPath = (Resolve-Path (Join-Path $ProjectRootPath "..\screen-watch-ocr")).Path
$oldPythonPath = $env:PYTHONPATH
$pythonScriptPath = Join-Path ([IO.Path]::GetTempPath()) "screen-watch-ocr-template-parity-$PID-$([guid]::NewGuid().ToString('N')).py"

$pythonBenchmark = @'
import tempfile
import time
from pathlib import Path

import cv2
import numpy as np

from screen_watch.core import Detector

frame_width = 2560
frame_height = 1440
template_count = 8
template_workers = 4

def seeded_textured_template(width, height, seed):
    template = np.zeros((height, width, 3), dtype=np.uint8)
    for y in range(height):
        for x in range(width):
            template[y, x, 0] = 30 + ((x * 37 + y * 11 + seed * 17) % 190)
            template[y, x, 1] = 20 + ((x * 13 + y * 41 + seed * 29) % 200)
            template[y, x, 2] = 40 + ((x * 23 + y * 29 + seed * 31) % 180)
    return template

def run_detector(base, label, frame, targets, expected_boxes, minimum_matches=None):
    detector = Detector(
        {
            "_base_dir": str(base),
            "template_workers": template_workers,
            "targets": targets,
        }
    )
    started = time.perf_counter()
    matches = detector.run(frame)
    elapsed_ms = int((time.perf_counter() - started) * 1000)

    expected_count = template_count if minimum_matches is None else minimum_matches
    assert len(matches) >= expected_count, (label, len(matches), matches)
    target_indexes = {target["id"]: index for index, target in enumerate(targets)}
    for item in matches:
        index = target_indexes[item["target_id"]]
        assert item["box"] == expected_boxes[index], (item, expected_boxes[index])
        assert item["score"] >= 0.99, item

    print(
        f"{label}={elapsed_ms} "
        f"frame={frame_width}x{frame_height} "
        f"templates={template_count} workers={template_workers} "
        f"matches={len(matches)} expected={template_count}"
    )

def run_flat(base):
    frame = np.full((frame_height, frame_width, 3), 3, dtype=np.uint8)
    targets = []
    expected_boxes = []

    for index in range(template_count):
        value = 40 + index * 23
        template = np.full((12, 12), value, dtype=np.uint8)
        file_name = f"target-{index}.png"
        cv2.imwrite(str(base / file_name), template)
        left = 137 + index * 211
        top = 193 + index * 97
        frame[top:top + 12, left:left + 12, :] = value
        expected_boxes.append([left, top, left + 12, top + 12])
        targets.append(
            {
                "kind": "template",
                "id": f"target-{index}",
                "name": f"target-{index}",
                "path": file_name,
                "threshold": 0.99,
                "scales": [1.0],
            }
        )

    run_detector(base, "pythonTemplateBenchmarkMs", frame, targets, expected_boxes)

def run_textured(base):
    frame = np.full((frame_height, frame_width, 3), 7, dtype=np.uint8)
    targets = []
    expected_boxes = []

    for index in range(template_count):
        template = seeded_textured_template(12, 12, index + 1)
        file_name = f"textured-target-{index}.png"
        cv2.imwrite(str(base / file_name), cv2.cvtColor(template, cv2.COLOR_RGB2BGR))
        left = 149 + index * 223
        top = 211 + index * 101
        frame[top:top + 12, left:left + 12, :] = template
        expected_boxes.append([left, top, left + 12, top + 12])
        targets.append(
            {
                "kind": "template",
                "id": f"textured-target-{index}",
                "name": f"textured-target-{index}",
                "path": file_name,
                "threshold": 0.99,
                "scales": [1.0],
            }
        )

    run_detector(
        base,
        "pythonTexturedTemplateBenchmarkMs",
        frame,
        targets,
        expected_boxes,
        minimum_matches=4,
    )

with tempfile.TemporaryDirectory() as tmp:
    base = Path(tmp)
    run_flat(base)
    run_textured(base)
'@

try {
    $env:PYTHONPATH = Join-Path $PythonProjectPath "src"
    [IO.File]::WriteAllText(
        $pythonScriptPath,
        $pythonBenchmark,
        [Text.UTF8Encoding]::new($false)
    )

    Push-Location $PythonProjectPath
    try {
        python $pythonScriptPath
        if ($LASTEXITCODE -ne 0) {
            throw "Python template benchmark failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    Push-Location $ProjectRootPath
    try {
        $filters = @(
            "benchmark_large_frame_many_template_scan",
            "benchmark_large_frame_textured_template_scan"
        )
        foreach ($filter in $filters) {
            $cargoArgs = @("test")
            if (-not $RustDebug) {
                $cargoArgs += "--release"
            }
            $cargoArgs += @(
                "-p",
                "screen-watch-core",
                $filter,
                "--",
                "--ignored",
                "--nocapture"
            )

            cargo @cargoArgs
            if ($LASTEXITCODE -ne 0) {
                throw "Rust template benchmark '$filter' failed with exit code $LASTEXITCODE"
            }
        }
    } finally {
        Pop-Location
    }
} finally {
    $env:PYTHONPATH = $oldPythonPath
    if (Test-Path -LiteralPath $pythonScriptPath) {
        Remove-Item -LiteralPath $pythonScriptPath -Force
    }
}
