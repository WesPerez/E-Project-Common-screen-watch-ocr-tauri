use crate::{
    config::{AlarmConfig, WatchConfig},
    detect::{DetectError, Match, PreparedDetector, RgbFrame},
    evidence::{write_alert_evidence, AlertCooldown, AlertEvent},
    ocr::{OcrBackend, OcrError, UnavailableOcrBackend},
};
use serde::Serialize;
use std::{
    io,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("{0}")]
    Detect(#[from] DetectError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Ocr(#[from] OcrError),
}

pub struct ScanEngine {
    config: WatchConfig,
    detector: PreparedDetector,
    data_dir: PathBuf,
    cooldown: AlertCooldown,
    ocr_backend: Box<dyn OcrBackend>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanFrameResult {
    pub region: String,
    pub matches: Vec<Match>,
    pub alerted_matches: Vec<Match>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alert: Option<AlertEvent>,
}

impl ScanEngine {
    pub fn new(
        config: WatchConfig,
        template_base_dir: impl AsRef<Path>,
        data_dir: impl AsRef<Path>,
    ) -> Result<Self, ScanError> {
        Self::new_with_ocr_backend(
            config,
            template_base_dir,
            data_dir,
            Box::new(UnavailableOcrBackend::disabled()),
        )
    }

    pub fn new_with_ocr_backend(
        config: WatchConfig,
        template_base_dir: impl AsRef<Path>,
        data_dir: impl AsRef<Path>,
        ocr_backend: Box<dyn OcrBackend>,
    ) -> Result<Self, ScanError> {
        let detector = PreparedDetector::from_config(&config, template_base_dir)?;
        Ok(Self {
            config,
            detector,
            data_dir: data_dir.as_ref().to_path_buf(),
            cooldown: AlertCooldown::new(),
            ocr_backend,
        })
    }

    pub fn scan_region_frame(
        &mut self,
        region: &str,
        frame: &RgbFrame,
        now_seconds: f64,
        time_text: &str,
        stamp: &str,
    ) -> Result<ScanFrameResult, ScanError> {
        let matches = if self.config.has_ocr_targets() {
            let rows = self.ocr_backend.recognize(frame)?;
            self.detector.run_with_ocr_rows(frame, &rows)
        } else {
            self.detector.run(frame)
        };
        let mut alerted_matches = Vec::new();
        for item in &matches {
            if self.cooldown.allow(
                region,
                &item.target_id,
                now_seconds,
                self.config.cooldown_seconds,
            ) {
                alerted_matches.push(item.clone());
            }
        }
        let alert = if alerted_matches.is_empty() {
            None
        } else {
            Some(write_alert_evidence(
                &self.data_dir,
                &self.config.alarm,
                region,
                frame,
                &alerted_matches,
                time_text,
                stamp,
            )?)
        };
        Ok(ScanFrameResult {
            region: region.to_string(),
            matches,
            alerted_matches,
            alert,
        })
    }

    pub fn alarm_config(&self) -> &AlarmConfig {
        &self.config.alarm
    }
}

#[cfg(test)]
mod tests {
    use super::ScanEngine;
    use crate::{
        config::{RegionConfig, ScaleSpec, WatchConfig},
        detect::{OcrTextRow, RgbFrame},
        evidence::AlertEvent,
        ocr::{OcrBackend, OcrError},
        profile::{
            profile_path, profile_watch_config_at, templates_dir, ProfileWatchConfigOptions,
        },
    };
    use serde_json::{json, Map};
    use std::fs;

    #[test]
    fn scan_region_frame_detects_matches_and_writes_evidence_after_cooldown() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "cooldown_seconds": 10,
              "alarm": {"save_dir":"screenshots", "jsonl":"alerts.jsonl", "max_alerts": 5},
              "targets": [
                {"kind":"pixel", "id":"red-id", "name":"red", "x":1, "y":1, "rgb":[255,0,0], "tolerance":0}
              ]
            }"#,
        )
        .unwrap();
        let mut engine = ScanEngine::new(config, tmp.path(), tmp.path()).unwrap();
        let frame = frame_with_pixels(&[(1, 1, [255, 0, 0])]);

        let first = engine
            .scan_region_frame("monitor 1", &frame, 100.0, "t1", "stamp-1")
            .unwrap();
        assert_eq!(first.matches.len(), 1);
        assert_eq!(first.alerted_matches[0].target_id, "red-id");
        assert!(first.alert.is_some());
        assert_eq!(jsonl_events(tmp.path()).len(), 1);

        let cooled = engine
            .scan_region_frame("monitor 1", &frame, 105.0, "t2", "stamp-2")
            .unwrap();
        assert_eq!(cooled.matches.len(), 1);
        assert!(cooled.alerted_matches.is_empty());
        assert!(cooled.alert.is_none());
        assert_eq!(jsonl_events(tmp.path()).len(), 1);

        let later = engine
            .scan_region_frame("monitor 1", &frame, 111.0, "t3", "stamp-3")
            .unwrap();
        assert_eq!(later.alerted_matches.len(), 1);
        assert!(later.alert.is_some());
        assert_eq!(jsonl_events(tmp.path()).len(), 2);
    }

    #[test]
    fn scan_cooldown_is_scoped_by_region_and_target_id() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "cooldown_seconds": 30,
              "alarm": {"save_dir":"screenshots", "jsonl":"alerts.jsonl"},
              "targets": [
                {"kind":"pixel", "id":"left-id", "name":"left", "x":0, "y":0, "rgb":[255,0,0], "tolerance":0},
                {"kind":"pixel", "id":"right-id", "name":"right", "x":2, "y":0, "rgb":[0,0,255], "tolerance":0}
              ]
            }"#,
        )
        .unwrap();
        let mut engine = ScanEngine::new(config, tmp.path(), tmp.path()).unwrap();

        let left = frame_with_pixels(&[(0, 0, [255, 0, 0])]);
        let first = engine
            .scan_region_frame("screen", &left, 1.0, "t1", "left")
            .unwrap();
        assert_eq!(target_ids(&first.alerted_matches), vec!["left-id"]);

        let right = frame_with_pixels(&[(2, 0, [0, 0, 255])]);
        let second = engine
            .scan_region_frame("screen", &right, 2.0, "t2", "right")
            .unwrap();
        assert_eq!(target_ids(&second.alerted_matches), vec!["right-id"]);

        let cooled_left = engine
            .scan_region_frame("screen", &left, 3.0, "t3", "left-again")
            .unwrap();
        assert!(cooled_left.alerted_matches.is_empty());

        let other_region = engine
            .scan_region_frame("other-screen", &left, 4.0, "t4", "left-other")
            .unwrap();
        assert_eq!(target_ids(&other_region.alerted_matches), vec!["left-id"]);
        assert_eq!(jsonl_events(tmp.path()).len(), 3);
    }

    #[test]
    fn scan_without_matches_does_not_write_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "alarm": {"save_dir":"screenshots", "jsonl":"alerts.jsonl"},
              "targets": [
                {"kind":"pixel", "name":"missing", "x":1, "y":1, "rgb":[255,0,0], "tolerance":0}
              ]
            }"#,
        )
        .unwrap();
        let mut engine = ScanEngine::new(config, tmp.path(), tmp.path()).unwrap();
        let frame = frame_with_pixels(&[]);
        let result = engine
            .scan_region_frame("screen", &frame, 1.0, "t1", "none")
            .unwrap();
        assert!(result.matches.is_empty());
        assert!(result.alert.is_none());
        assert!(!tmp.path().join("alerts.jsonl").exists());
        assert!(!tmp.path().join("screenshots").exists());
    }

    #[test]
    fn scan_with_ocr_target_requires_an_ocr_backend() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "alarm": {"save_dir":"screenshots", "jsonl":"alerts.jsonl"},
              "targets": [
                {"kind":"ocr_text", "id":"ready-id", "name":"ready", "text":"READY", "min_score":0.5}
              ]
            }"#,
        )
        .unwrap();
        let mut engine = ScanEngine::new(config, tmp.path(), tmp.path()).unwrap();
        let frame = frame_with_pixels(&[]);

        let err = engine
            .scan_region_frame("screen", &frame, 1.0, "t1", "ocr-missing")
            .unwrap_err();

        assert!(err.to_string().contains("OCR backend disabled"));
        assert!(!tmp.path().join("alerts.jsonl").exists());
    }

    #[test]
    fn scan_with_ocr_backend_matches_text_and_writes_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "cooldown_seconds": 10,
              "alarm": {"save_dir":"screenshots", "jsonl":"alerts.jsonl"},
              "targets": [
                {"kind":"ocr_text", "id":"ready-id", "name":"ready", "text":"ready", "min_score":0.5}
              ]
            }"#,
        )
        .unwrap();
        let backend = FixedOcrBackend {
            rows: vec![OcrTextRow {
                text: "READY NOW".to_string(),
                score: 0.9,
                box_points: Some(vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]]),
            }],
        };
        let mut engine =
            ScanEngine::new_with_ocr_backend(config, tmp.path(), tmp.path(), Box::new(backend))
                .unwrap();
        let frame = frame_with_pixels(&[]);

        let result = engine
            .scan_region_frame("screen", &frame, 1.0, "t1", "ocr-hit")
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].target_id, "ready-id");
        assert_eq!(result.matches[0].text.as_deref(), Some("READY NOW"));
        assert_eq!(result.matches[0].box_xyxy, [1, 1, 3, 3]);
        assert!(result.alert.is_some());
        assert_eq!(jsonl_events(tmp.path()).len(), 1);
    }

    #[test]
    fn profile_watch_config_scans_scaled_template_through_engine() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let templates = templates_dir(&data_dir);
        fs::create_dir_all(&templates).unwrap();

        let template = textured_template(4, 4);
        let template_path = templates.join("1-1-scaled-widget.png");
        write_png(&template_path, &template);

        let profile = profile_path(&data_dir, 1);
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "futureProfileField": true,
                "targets": [{
                    "id": "scaled-widget-id",
                    "name": "scaled-widget",
                    "path": "templates/1-1-scaled-widget.png",
                    "enabled": true
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let config = profile_watch_config_at(
            &profile,
            &data_dir,
            ProfileWatchConfigOptions {
                regions: vec![RegionConfig {
                    name: "monitor-1".to_string(),
                    monitor: 1,
                    left: 0,
                    top: 0,
                    width: Some(24),
                    height: Some(18),
                    extra: Map::new(),
                }],
                threshold: 0.99,
                scales: ScaleSpec::Text("1.0-2.0:1.0".to_string()),
                cooldown_seconds: 0.0,
                template_workers: 2,
                ..ProfileWatchConfigOptions::default()
            },
        )
        .unwrap();
        let mut engine = ScanEngine::new(config, &data_dir, &data_dir).unwrap();

        let scaled = scale_nearest_rgb(&template, 2.0);
        let mut frame_pixels = vec![0; 24 * 18 * 3];
        paste(&mut frame_pixels, 24, 9, 6, &scaled);
        let frame = RgbFrame::new(24, 18, frame_pixels).unwrap();

        let result = engine
            .scan_region_frame("profile monitor", &frame, 1.0, "t1", "profile-scaled")
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].target_id, "scaled-widget-id");
        assert_eq!(result.matches[0].scale, Some(2.0));
        assert_eq!(result.matches[0].box_xyxy, [9, 6, 17, 14]);
        assert_eq!(result.alerted_matches.len(), 1);
        assert!(result.alert.is_some());
        assert_eq!(jsonl_events(&data_dir).len(), 1);
        assert!(data_dir.join("screenshots").exists());
    }

    fn frame_with_pixels(items: &[(u32, u32, [u8; 3])]) -> RgbFrame {
        let width = 3;
        let height = 3;
        let mut pixels = vec![0; width as usize * height as usize * 3];
        for (x, y, rgb) in items {
            let index = ((y * width + x) * 3) as usize;
            pixels[index..index + 3].copy_from_slice(rgb);
        }
        RgbFrame::new(width, height, pixels).unwrap()
    }

    fn jsonl_events(path: &std::path::Path) -> Vec<AlertEvent> {
        fs::read_to_string(path.join("alerts.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn target_ids(matches: &[crate::detect::Match]) -> Vec<&str> {
        matches.iter().map(|item| item.target_id.as_str()).collect()
    }

    fn textured_template(width: u32, height: u32) -> RgbFrame {
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                pixels.extend([
                    30 + ((x * 37 + y * 11) % 190) as u8,
                    20 + ((x * 13 + y * 41) % 200) as u8,
                    40 + ((x * 23 + y * 29) % 180) as u8,
                ]);
            }
        }
        RgbFrame::new(width, height, pixels).unwrap()
    }

    fn scale_nearest_rgb(image: &RgbFrame, scale: f64) -> RgbFrame {
        let width = ((f64::from(image.width) * scale) as u32).max(1);
        let height = ((f64::from(image.height) * scale) as u32).max(1);
        let mut pixels = vec![0; width as usize * height as usize * 3];
        for y in 0..height {
            for x in 0..width {
                let src_x = ((f64::from(x) / scale).floor() as u32).min(image.width - 1);
                let src_y = ((f64::from(y) / scale).floor() as u32).min(image.height - 1);
                let target = ((y * width + x) * 3) as usize;
                let rgb = image.pixel(src_x, src_y).unwrap();
                pixels[target..target + 3].copy_from_slice(&rgb);
            }
        }
        RgbFrame::new(width, height, pixels).unwrap()
    }

    fn paste(frame: &mut [u8], frame_width: u32, left: u32, top: u32, image: &RgbFrame) {
        for y in 0..image.height {
            for x in 0..image.width {
                let target = (((top + y) * frame_width + left + x) * 3) as usize;
                let source = ((y * image.width + x) * 3) as usize;
                frame[target..target + 3].copy_from_slice(&image.pixels[source..source + 3]);
            }
        }
    }

    fn write_png(path: &std::path::Path, image: &RgbFrame) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, image.width, image.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&image.pixels).unwrap();
    }

    #[derive(Debug)]
    struct FixedOcrBackend {
        rows: Vec<OcrTextRow>,
    }

    impl OcrBackend for FixedOcrBackend {
        fn recognize(&mut self, _frame: &RgbFrame) -> Result<Vec<OcrTextRow>, OcrError> {
            Ok(self.rows.clone())
        }
    }
}
