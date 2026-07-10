use crate::{
    config::AlarmConfig,
    detect::{Match, RgbFrame},
    profile::screenshots_dir,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

const ALERT_RED: [u8; 3] = [255, 0, 0];
const GLYPH_WIDTH: u32 = 5;
const GLYPH_HEIGHT: u32 = 7;
const GLYPH_SPACING: u32 = 1;
const MAX_LABEL_CHARS: usize = 32;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AlarmPaths {
    pub screenshot_dir: PathBuf,
    pub jsonl: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertEvent {
    pub time: String,
    pub region: String,
    pub matches: Vec<Match>,
    pub screenshot: String,
}

#[derive(Debug, Clone, Default)]
pub struct AlertCooldown {
    last_seen: HashMap<(String, String), f64>,
}

impl AlertCooldown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow(
        &mut self,
        region: &str,
        target: &str,
        now_seconds: f64,
        cooldown_seconds: f64,
    ) -> bool {
        let key = (region.to_string(), target.to_string());
        let last = self
            .last_seen
            .get(&key)
            .copied()
            .unwrap_or(f64::NEG_INFINITY);
        if now_seconds - last >= cooldown_seconds {
            self.last_seen.insert(key, now_seconds);
            true
        } else {
            false
        }
    }
}

pub fn alarm_paths(data_dir: impl AsRef<Path>, alarm: &AlarmConfig) -> AlarmPaths {
    let data_dir = data_dir.as_ref();
    let screenshot_dir = if alarm.save_dir == "screenshots" {
        screenshots_dir(data_dir)
    } else {
        data_dir.join(&alarm.save_dir)
    };
    AlarmPaths {
        screenshot_dir,
        jsonl: data_dir.join(&alarm.jsonl),
    }
}

pub fn safe_name(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-') {
            out.push(ch);
        } else {
            out.push('_');
        }
        if out.len() >= 80 {
            break;
        }
    }
    if out.is_empty() {
        "alert".to_string()
    } else {
        out
    }
}

pub fn write_alert_evidence(
    data_dir: impl AsRef<Path>,
    alarm: &AlarmConfig,
    region: &str,
    frame: &RgbFrame,
    matches: &[Match],
    time_text: &str,
    stamp: &str,
) -> io::Result<AlertEvent> {
    let paths = alarm_paths(data_dir, alarm);
    fs::create_dir_all(&paths.screenshot_dir)?;
    let image_path = paths
        .screenshot_dir
        .join(format!("{}-{}.png", stamp, safe_name(region)));
    save_rgb_png(&image_path, frame, matches)?;
    prune_alert_images(
        &paths.screenshot_dir,
        alarm.max_alerts.unwrap_or(50).max(1) as usize,
    )?;
    let screenshot = image_path
        .canonicalize()
        .unwrap_or_else(|_| image_path.clone())
        .display()
        .to_string();
    let event = AlertEvent {
        time: time_text.to_string(),
        region: region.to_string(),
        matches: matches.to_vec(),
        screenshot,
    };
    if let Some(parent) = paths.jsonl.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.jsonl)?;
    serde_json::to_writer(&mut file, &event)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    file.write_all(b"\n")?;
    Ok(event)
}

pub fn save_rgb_png(path: impl AsRef<Path>, frame: &RgbFrame, matches: &[Match]) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut pixels = frame.pixels.clone();
    for item in matches {
        draw_rect(&mut pixels, frame.width, frame.height, item.box_xyxy);
        draw_label(
            &mut pixels,
            frame.width,
            frame.height,
            item.box_xyxy,
            &item.target,
        );
    }
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.set_filter(png::FilterType::Paeth);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&pixels)?;
    Ok(())
}

pub fn prune_alert_images(path: impl AsRef<Path>, max_count: usize) -> io::Result<usize> {
    let max_count = max_count.max(1);
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|item| item.to_str()) != Some("png") {
            continue;
        }
        let modified = entry.metadata()?.modified().ok();
        let name = path
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or_default()
            .to_string();
        files.push((modified, name, path));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let remove_count = files.len().saturating_sub(max_count);
    for (_, _, path) in files.into_iter().take(remove_count) {
        fs::remove_file(path)?;
    }
    Ok(remove_count)
}

fn draw_rect(pixels: &mut [u8], width: u32, height: u32, box_xyxy: [u32; 4]) {
    if width == 0 || height == 0 {
        return;
    }
    if box_xyxy[2] <= box_xyxy[0] || box_xyxy[3] <= box_xyxy[1] {
        return;
    }
    let x1 = box_xyxy[0].min(width - 1);
    let y1 = box_xyxy[1].min(height - 1);
    let x2 = box_xyxy[2].saturating_sub(1).min(width - 1);
    let y2 = box_xyxy[3].saturating_sub(1).min(height - 1);
    if x2 < x1 || y2 < y1 {
        return;
    }
    for inset in 0..3 {
        let left = x1.saturating_add(inset).min(x2);
        let right = x2.saturating_sub(inset).max(left);
        let top = y1.saturating_add(inset).min(y2);
        let bottom = y2.saturating_sub(inset).max(top);
        for x in left..=right {
            set_pixel(pixels, width, x, top, ALERT_RED);
            set_pixel(pixels, width, x, bottom, ALERT_RED);
        }
        for y in top..=bottom {
            set_pixel(pixels, width, left, y, ALERT_RED);
            set_pixel(pixels, width, right, y, ALERT_RED);
        }
    }
}

fn draw_label(pixels: &mut [u8], width: u32, height: u32, box_xyxy: [u32; 4], text: &str) {
    if width == 0 || height == 0 || text.trim().is_empty() {
        return;
    }
    let x = box_xyxy[0].min(width - 1);
    let box_top = box_xyxy[1].min(height - 1);
    let y = box_top.saturating_sub(GLYPH_HEIGHT + 2);
    draw_text(pixels, width, height, x, y, text);
}

fn draw_text(pixels: &mut [u8], width: u32, height: u32, x: u32, y: u32, text: &str) {
    let mut cursor_x = x;
    for ch in text.chars().take(MAX_LABEL_CHARS) {
        if cursor_x >= width {
            break;
        }
        let glyph = glyph_rows(ch);
        for (row, bits) in glyph.iter().enumerate() {
            let py = y + row as u32;
            if py >= height {
                continue;
            }
            for col in 0..GLYPH_WIDTH {
                let mask = 1 << (GLYPH_WIDTH - 1 - col);
                if bits & mask != 0 {
                    let px = cursor_x + col;
                    if px < width {
                        set_pixel(pixels, width, px, py, ALERT_RED);
                    }
                }
            }
        }
        cursor_x = cursor_x.saturating_add(GLYPH_WIDTH + GLYPH_SPACING);
    }
}

fn glyph_rows(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        ' ' => [0; 7],
        _ => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
    }
}

fn set_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, rgb: [u8; 3]) {
    let index = ((y * width + x) * 3) as usize;
    if index + 2 < pixels.len() {
        pixels[index..index + 3].copy_from_slice(&rgb);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        alarm_paths, prune_alert_images, safe_name, save_rgb_png, write_alert_evidence,
        AlertCooldown, AlertEvent,
    };
    use crate::config::AlarmConfig;
    use crate::detect::{Match, RgbFrame};
    use std::{fs, time::Instant};

    #[test]
    fn alarm_paths_match_app_data_conventions() {
        let tmp = tempfile::tempdir().unwrap();
        let alarm = AlarmConfig {
            save_dir: "screenshots".to_string(),
            jsonl: "alerts.jsonl".to_string(),
            ..Default::default()
        };
        let paths = alarm_paths(tmp.path(), &alarm);
        assert_eq!(paths.screenshot_dir, tmp.path().join("screenshots"));
        assert_eq!(paths.jsonl, tmp.path().join("alerts.jsonl"));

        let alarm = AlarmConfig {
            save_dir: "evidence/alerts".to_string(),
            jsonl: "evidence/alerts.jsonl".to_string(),
            ..Default::default()
        };
        let paths = alarm_paths(tmp.path(), &alarm);
        assert_eq!(paths.screenshot_dir, tmp.path().join("evidence/alerts"));
        assert_eq!(paths.jsonl, tmp.path().join("evidence/alerts.jsonl"));
    }

    #[test]
    fn safe_name_matches_legacy_alert_filename_sanitizing() {
        assert_eq!(safe_name("monitor 1/left"), "monitor_1_left");
        assert_eq!(safe_name(""), "alert");
        assert_eq!(safe_name("abc.DEF-123"), "abc.DEF-123");
    }

    #[test]
    fn alert_cooldown_is_scoped_by_region_and_target() {
        let mut cooldown = AlertCooldown::new();
        assert!(cooldown.allow("screen", "target", 10.0, 3.0));
        assert!(!cooldown.allow("screen", "target", 12.0, 3.0));
        assert!(cooldown.allow("screen", "other", 12.0, 3.0));
        assert!(cooldown.allow("screen", "target", 13.1, 3.0));
    }

    #[test]
    fn prune_alert_images_keeps_newest_by_sort_order_and_ignores_non_png() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["0.png", "1.png", "2.png", "3.png", "4.png", "note.txt"] {
            fs::write(tmp.path().join(name), b"x").unwrap();
        }
        assert_eq!(prune_alert_images(tmp.path(), 2).unwrap(), 3);
        let mut kept = fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        kept.sort();
        assert_eq!(kept, vec!["3.png", "4.png", "note.txt"]);
    }

    #[test]
    fn save_rgb_png_draws_red_match_boxes() {
        let tmp = tempfile::tempdir().unwrap();
        let frame = RgbFrame::new(5, 5, vec![20; 5 * 5 * 3]).unwrap();
        let path = tmp.path().join("hit.png");
        save_rgb_png(&path, &frame, &[sample_match([1, 1, 4, 4])]).unwrap();
        let decoded = RgbFrame::from_png_path(&path).unwrap();
        assert_eq!(decoded.pixel(1, 1), Some([255, 0, 0]));
        assert_eq!(decoded.pixel(3, 3), Some([255, 0, 0]));
        assert_eq!(decoded.pixel(0, 0), Some([20, 20, 20]));
    }

    #[test]
    fn save_rgb_png_draws_match_label_above_box() {
        let tmp = tempfile::tempdir().unwrap();
        let frame = RgbFrame::new(32, 24, vec![20; 32 * 24 * 3]).unwrap();
        let path = tmp.path().join("labeled-hit.png");
        let mut hit = sample_match([2, 12, 12, 20]);
        hit.target = "A1".to_string();

        save_rgb_png(&path, &frame, &[hit]).unwrap();

        let decoded = RgbFrame::from_png_path(&path).unwrap();
        assert_eq!(decoded.pixel(3, 3), Some([255, 0, 0]));
        assert_eq!(decoded.pixel(10, 3), Some([255, 0, 0]));
        assert_eq!(decoded.pixel(2, 3), Some([20, 20, 20]));
    }

    #[test]
    fn write_alert_evidence_saves_screenshot_appends_jsonl_and_prunes_old_images() {
        let tmp = tempfile::tempdir().unwrap();
        let alarm = AlarmConfig {
            save_dir: "screenshots".to_string(),
            jsonl: "alerts.jsonl".to_string(),
            max_alerts: Some(2),
            ..Default::default()
        };
        let screenshots = tmp.path().join("screenshots");
        fs::create_dir_all(&screenshots).unwrap();
        for name in ["old-1.png", "old-2.png"] {
            fs::write(screenshots.join(name), b"x").unwrap();
        }
        let frame = RgbFrame::new(4, 4, vec![0; 4 * 4 * 3]).unwrap();
        let event = write_alert_evidence(
            tmp.path(),
            &alarm,
            "monitor 1",
            &frame,
            &[sample_match([1, 1, 3, 3])],
            "2026-07-05T12:00:00+0800",
            "20260705-120000-000000001",
        )
        .unwrap();
        assert_eq!(event.region, "monitor 1");
        assert!(event
            .screenshot
            .ends_with("20260705-120000-000000001-monitor_1.png"));

        let mut screenshots = fs::read_dir(tmp.path().join("screenshots"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        screenshots.sort();
        assert_eq!(screenshots.len(), 2);
        assert!(screenshots
            .iter()
            .any(|name| name.ends_with("monitor_1.png")));

        let lines = fs::read_to_string(tmp.path().join("alerts.jsonl")).unwrap();
        let parsed: AlertEvent = serde_json::from_str(lines.trim()).unwrap();
        assert_eq!(parsed.matches[0].target_id, "stable-id");
        assert_eq!(parsed.screenshot, event.screenshot);
    }

    #[test]
    #[ignore = "performance gate; run explicitly in release mode"]
    fn benchmark_4k_alert_png_write() {
        let tmp = tempfile::tempdir().unwrap();
        let width = 3840;
        let height = 2160;
        let mut pixels = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                pixels.push(((x * 17 + y * 11) % 256) as u8);
                pixels.push(((x * 7 + y * 19) % 256) as u8);
                pixels.push(((x * 23 + y * 5) % 256) as u8);
            }
        }
        let frame = RgbFrame::new(width as u32, height as u32, pixels).unwrap();
        let path = tmp.path().join("4k-alert.png");

        let started = Instant::now();
        save_rgb_png(&path, &frame, &[sample_match([100, 100, 600, 300])]).unwrap();
        let elapsed_ms = started.elapsed().as_millis();
        let bytes = path.metadata().unwrap().len();

        println!("alertPngBenchmarkMs={elapsed_ms} bytes={bytes}");
        let decoded = RgbFrame::from_png_path(&path).unwrap();
        assert_eq!(
            (decoded.width, decoded.height),
            (width as u32, height as u32)
        );
    }

    fn sample_match(box_xyxy: [u32; 4]) -> Match {
        Match {
            target: "target".to_string(),
            target_id: "stable-id".to_string(),
            kind: "template".to_string(),
            score: 0.99,
            box_xyxy,
            scale: Some(1.0),
            text: None,
        }
    }
}
