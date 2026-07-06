use crate::audio::{DEFAULT_BEEP_SECONDS, DEFAULT_BEEP_VOLUME};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;
use thiserror::Error;

pub const MAX_SCALE_COUNT: usize = 120;
pub const DEFAULT_TEMPLATE_WORKERS: usize = 8;

#[derive(Debug, Error, PartialEq)]
pub enum ConfigError {
    #[error("invalid json: {0}")]
    InvalidJson(String),
    #[error("config.targets is empty")]
    EmptyTargets,
    #[error("at least one screen region, window, or remembered app is required")]
    EmptySources,
    #[error("unknown target kind {0}")]
    UnknownTargetKind(String),
    #[error("scale must be > 0")]
    NonPositiveScale,
    #[error("scale range {0:?} needs a step, for example 0.5-2.0:0.1")]
    MissingScaleStep(String),
    #[error("too many scales; keep it <= {MAX_SCALE_COUNT}")]
    TooManyScales,
    #[error("scales is empty")]
    EmptyScales,
    #[error("invalid scale {0:?}")]
    InvalidScale(String),
    #[error("alarm.beep_seconds must be > 0")]
    NonPositiveBeepSeconds,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchConfig {
    #[serde(default = "default_poll_interval_seconds")]
    pub poll_interval_seconds: f64,
    #[serde(default = "default_cooldown_seconds")]
    pub cooldown_seconds: f64,
    #[serde(default = "default_template_workers")]
    pub template_workers: usize,
    #[serde(default)]
    pub regions: Vec<RegionConfig>,
    #[serde(default)]
    pub windows: Vec<WindowConfig>,
    #[serde(default)]
    pub window_apps: Vec<WindowAppConfig>,
    #[serde(default)]
    pub targets: Vec<TargetConfig>,
    #[serde(default)]
    pub alarm: AlarmConfig,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegionConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_monitor")]
    pub monitor: i32,
    #[serde(default)]
    pub left: i32,
    #[serde(default)]
    pub top: i32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub display: String,
    pub hwnd: Option<isize>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowAppConfig {
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_ordinal")]
    pub ordinal: u32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum TargetConfig {
    #[serde(rename = "template")]
    Template {
        name: String,
        path: String,
        #[serde(default = "default_template_threshold")]
        threshold: f32,
        #[serde(default = "default_scales")]
        scales: ScaleSpec,
        #[serde(default)]
        id: Option<String>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    #[serde(rename = "pixel")]
    Pixel {
        name: String,
        x: u32,
        y: u32,
        rgb: [u8; 3],
        #[serde(default = "default_pixel_tolerance")]
        tolerance: u8,
        #[serde(default)]
        id: Option<String>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    #[serde(rename = "ocr_text")]
    OcrText {
        name: String,
        text: String,
        #[serde(default)]
        min_score: f32,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        id: Option<String>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ScaleSpec {
    Text(String),
    Number(f64),
    List(Vec<ScaleSpec>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlarmConfig {
    #[serde(default = "default_alarm_beep")]
    pub beep: bool,
    #[serde(default = "default_beep_seconds")]
    pub beep_seconds: f64,
    #[serde(default = "default_beep_volume")]
    pub beep_volume: i32,
    #[serde(default = "default_save_dir")]
    pub save_dir: String,
    #[serde(default = "default_jsonl")]
    pub jsonl: String,
    pub max_alerts: Option<u32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for AlarmConfig {
    fn default() -> Self {
        Self {
            beep: default_alarm_beep(),
            beep_seconds: default_beep_seconds(),
            beep_volume: default_beep_volume(),
            save_dir: default_save_dir(),
            jsonl: default_jsonl(),
            max_alerts: None,
            extra: Map::new(),
        }
    }
}

impl WatchConfig {
    pub fn from_json_str(text: &str) -> Result<Self, ConfigError> {
        serde_json::from_str(text).map_err(|err| ConfigError::InvalidJson(err.to_string()))
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| ConfigError::InvalidJson(err.to_string()))?;
        Self::from_json_str(&text)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.targets.is_empty() {
            return Err(ConfigError::EmptyTargets);
        }
        if !self.alarm.beep_seconds.is_finite() || self.alarm.beep_seconds <= 0.0 {
            return Err(ConfigError::NonPositiveBeepSeconds);
        }
        for target in &self.targets {
            match target {
                TargetConfig::Unknown => {
                    return Err(ConfigError::UnknownTargetKind("unknown".to_string()))
                }
                TargetConfig::Template { scales, .. } => {
                    parse_scales(scales)?;
                }
                TargetConfig::Pixel { .. } | TargetConfig::OcrText { .. } => {}
            }
        }
        Ok(())
    }

    pub fn template_worker_limit(&self) -> usize {
        self.template_workers.max(1)
    }

    pub fn has_ocr_targets(&self) -> bool {
        self.targets
            .iter()
            .any(|target| matches!(target, TargetConfig::OcrText { .. }))
    }
}

impl Default for ScaleSpec {
    fn default() -> Self {
        ScaleSpec::Number(1.0)
    }
}

pub fn parse_scales(spec: &ScaleSpec) -> Result<Vec<f64>, ConfigError> {
    let mut values = Vec::new();
    collect_scales(spec, &mut values)?;
    let mut out = Vec::new();
    for value in values {
        if value <= 0.0 {
            return Err(ConfigError::NonPositiveScale);
        }
        let key = (value * 1_000_000.0).round() / 1_000_000.0;
        if !out
            .iter()
            .any(|old: &f64| (*old - key).abs() < f64::EPSILON)
        {
            out.push(key);
        }
        if out.len() > MAX_SCALE_COUNT {
            return Err(ConfigError::TooManyScales);
        }
    }
    if out.is_empty() {
        return Err(ConfigError::EmptyScales);
    }
    Ok(out)
}

fn collect_scales(spec: &ScaleSpec, values: &mut Vec<f64>) -> Result<(), ConfigError> {
    match spec {
        ScaleSpec::Number(value) => values.push(*value),
        ScaleSpec::Text(text) => {
            for part in text
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                collect_scale_token(part, values)?;
            }
        }
        ScaleSpec::List(items) => {
            for item in items {
                collect_scales(item, values)?;
            }
        }
    }
    Ok(())
}

fn collect_scale_token(token: &str, values: &mut Vec<f64>) -> Result<(), ConfigError> {
    if !token.contains('-') {
        let value = token
            .parse::<f64>()
            .map_err(|_| ConfigError::InvalidScale(token.to_string()))?;
        values.push(value);
        return Ok(());
    }

    let (span, step_text) = token
        .split_once(':')
        .ok_or_else(|| ConfigError::MissingScaleStep(token.to_string()))?;
    let (start_text, end_text) = span
        .split_once('-')
        .ok_or_else(|| ConfigError::InvalidScale(token.to_string()))?;
    let start = start_text
        .parse::<f64>()
        .map_err(|_| ConfigError::InvalidScale(token.to_string()))?;
    let end = end_text
        .parse::<f64>()
        .map_err(|_| ConfigError::InvalidScale(token.to_string()))?;
    if start <= 0.0 || end <= 0.0 {
        return Err(ConfigError::NonPositiveScale);
    }
    values.extend(scale_range(start, end, step_text.trim())?);
    Ok(())
}

fn scale_range(start: f64, end: f64, step_text: &str) -> Result<Vec<f64>, ConfigError> {
    let percent = step_text.ends_with('%');
    let raw_step = if percent {
        &step_text[..step_text.len() - 1]
    } else {
        step_text
    };
    let mut step = raw_step
        .parse::<f64>()
        .map_err(|_| ConfigError::InvalidScale(step_text.to_string()))?;
    if step <= 0.0 {
        return Err(ConfigError::NonPositiveScale);
    }

    let mut values = Vec::new();
    if percent {
        let factor = 1.0 + step / 100.0;
        let mut current = start;
        if start <= end {
            while current <= end * 1.0000001 {
                push_scale(&mut values, current)?;
                current *= factor;
            }
        } else {
            while current >= end / 1.0000001 {
                push_scale(&mut values, current)?;
                current /= factor;
            }
        }
    } else {
        let direction = if end >= start { 1.0 } else { -1.0 };
        step *= direction;
        let mut current = start;
        if direction > 0.0 {
            while current <= end + step.abs() / 1_000_000.0 {
                push_scale(&mut values, current)?;
                current += step;
            }
        } else {
            while current >= end - step.abs() / 1_000_000.0 {
                push_scale(&mut values, current)?;
                current += step;
            }
        }
    }

    if values
        .last()
        .map(|value| (value - end).abs() > 1e-6)
        .unwrap_or(false)
    {
        push_scale(&mut values, end)?;
    }

    Ok(values)
}

fn push_scale(values: &mut Vec<f64>, value: f64) -> Result<(), ConfigError> {
    values.push(value);
    if values.len() > MAX_SCALE_COUNT {
        return Err(ConfigError::TooManyScales);
    }
    Ok(())
}

fn default_poll_interval_seconds() -> f64 {
    0.3
}

fn default_cooldown_seconds() -> f64 {
    3.0
}

fn default_template_workers() -> usize {
    DEFAULT_TEMPLATE_WORKERS
}

fn default_monitor() -> i32 {
    1
}

fn default_ordinal() -> u32 {
    1
}

fn default_template_threshold() -> f32 {
    0.9
}

fn default_scales() -> ScaleSpec {
    ScaleSpec::Number(1.0)
}

fn default_pixel_tolerance() -> u8 {
    8
}

fn default_alarm_beep() -> bool {
    true
}

fn default_beep_seconds() -> f64 {
    DEFAULT_BEEP_SECONDS
}

fn default_beep_volume() -> i32 {
    DEFAULT_BEEP_VOLUME
}

fn default_save_dir() -> String {
    "evidence/alerts".to_string()
}

fn default_jsonl() -> String {
    "evidence/alerts.jsonl".to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_scales, ScaleSpec, TargetConfig, WatchConfig};
    use crate::audio::{DEFAULT_BEEP_SECONDS, DEFAULT_BEEP_VOLUME};
    use crate::config::ConfigError;

    #[test]
    fn parses_python_style_config_and_preserves_extra_fields() {
        let text = r#"{
          "poll_interval_seconds": 0.25,
          "cooldown_seconds": 2,
          "template_workers": 2,
          "regions": [{"name": "left-top", "monitor": 1, "left": 0, "top": 0, "width": 640, "height": 360}],
          "targets": [
            {"name": "boss-avatar", "kind": "template", "path": "templates/boss.png", "threshold": 0.91, "scales": [0.9, 1.0, 1.1]},
            {"name": "red-dot", "kind": "pixel", "x": 50, "y": 50, "rgb": [255, 0, 0], "tolerance": 12},
            {"name": "warning-text", "kind": "ocr_text", "text": "WARNING", "min_score": 0.4}
          ],
          "alarm": {"beep": true, "beep_seconds": 5.5, "beep_volume": 42, "save_dir": "evidence/alerts", "jsonl": "evidence/alerts.jsonl"},
          "future_field": {"kept": true}
        }"#;
        let config = WatchConfig::from_json_str(text).unwrap();
        config.validate().unwrap();
        assert_eq!(config.template_worker_limit(), 2);
        assert_eq!(config.alarm.beep_seconds, 5.5);
        assert_eq!(config.alarm.beep_volume, 42);
        assert!(config.extra.contains_key("future_field"));
        assert!(matches!(config.targets[2], TargetConfig::OcrText { .. }));
    }

    #[test]
    fn alarm_beep_settings_default_and_validate_like_python_baseline() {
        let config = WatchConfig::from_json_str(
            r#"{"targets":[{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]}"#,
        )
        .unwrap();
        assert_eq!(config.alarm.beep_seconds, DEFAULT_BEEP_SECONDS);
        assert_eq!(config.alarm.beep_volume, DEFAULT_BEEP_VOLUME);
        config.validate().unwrap();

        let config = WatchConfig::from_json_str(
            r#"{"alarm":{"beep_seconds":0},"targets":[{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]}"#,
        )
        .unwrap();
        assert_eq!(config.validate(), Err(ConfigError::NonPositiveBeepSeconds));
    }

    #[test]
    fn template_worker_limit_defaults_and_clamps_to_one() {
        let config = WatchConfig::from_json_str(
            r#"{"targets":[{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]}"#,
        )
        .unwrap();
        assert_eq!(config.template_workers, super::DEFAULT_TEMPLATE_WORKERS);
        assert_eq!(
            config.template_worker_limit(),
            super::DEFAULT_TEMPLATE_WORKERS
        );

        let config = WatchConfig::from_json_str(
            r#"{"template_workers":0,"targets":[{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]}"#,
        )
        .unwrap();
        assert_eq!(config.template_workers, 0);
        assert_eq!(config.template_worker_limit(), 1);
    }

    #[test]
    fn parses_scale_syntax_like_python_baseline() {
        assert_eq!(
            parse_scales(&ScaleSpec::Text("1, 0.9,1.1".to_string())).unwrap(),
            vec![1.0, 0.9, 1.1]
        );
        assert_eq!(
            parse_scales(&ScaleSpec::Text("0.5-0.7:0.1,1".to_string())).unwrap(),
            vec![0.5, 0.6, 0.7, 1.0]
        );
        assert_eq!(
            parse_scales(&ScaleSpec::Text("0.1-0.13:10%".to_string())).unwrap(),
            vec![0.1, 0.11, 0.121, 0.13]
        );
    }

    #[test]
    fn rejects_empty_targets() {
        let config = WatchConfig::from_json_str(r#"{"targets":[]}"#).unwrap();
        assert_eq!(config.validate(), Err(ConfigError::EmptyTargets));
    }

    #[test]
    fn accepts_target_only_config_because_regions_default_to_physical_monitors() {
        let config = WatchConfig::from_json_str(
            r#"{"targets":[{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]}"#,
        )
        .unwrap();
        config.validate().unwrap();
    }
}
