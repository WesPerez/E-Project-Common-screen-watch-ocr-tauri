use crate::{
    config::{
        AlarmConfig, RegionConfig, ScaleSpec, TargetConfig, WatchConfig, WindowAppConfig,
        WindowConfig,
    },
    detect::{DetectError, RgbFrame},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{
    fs,
    io::{self, BufWriter},
    path::{Path, PathBuf},
};

pub const PROFILE_COUNT: u32 = 5;
pub const PROFILE_SOURCE_WORKERS: usize = 1;
pub const PROFILE_TEMPLATE_WORKERS: usize = 2;
pub const PROFILE_MIN_IDLE_SECONDS: f64 = 0.08;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddTemplateImagesResult {
    pub changed: bool,
    pub added_count: usize,
    pub pruned_count: usize,
    pub selected_index: Option<usize>,
    pub targets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTargetsEditResult {
    pub changed: bool,
    pub deleted_files: usize,
    pub selected_index: Option<usize>,
    pub targets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTargetsEnabledResult {
    pub changed: bool,
    pub enabled_count: usize,
    pub all_enabled: bool,
    pub targets: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileReadResult {
    pub exists: bool,
    pub enabled_count: usize,
    pub all_enabled: bool,
    pub targets: Vec<Value>,
    pub profile: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSourcesSaveResult {
    pub changed: bool,
    pub profile: ProfileReadResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStateResult {
    pub last_profile: u32,
    pub state: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowGeometry {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

impl WindowGeometry {
    pub fn new(width: u32, height: u32, x: i32, y: i32) -> io::Result<Self> {
        if width == 0 || height == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "window geometry width and height must be positive",
            ));
        }
        Ok(Self {
            width,
            height,
            x,
            y,
        })
    }

    pub fn to_python_geometry(self) -> String {
        format!(
            "{}x{}{}{}{}{}",
            self.width,
            self.height,
            if self.x < 0 { "" } else { "+" },
            self.x,
            if self.y < 0 { "" } else { "+" },
            self.y
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileWatchConfigOptions {
    #[serde(default)]
    pub regions: Vec<RegionConfig>,
    #[serde(default)]
    pub windows: Vec<WindowConfig>,
    #[serde(default)]
    pub window_apps: Vec<WindowAppConfig>,
    #[serde(default = "default_profile_threshold")]
    pub threshold: f32,
    #[serde(default = "default_profile_scales")]
    pub scales: ScaleSpec,
    #[serde(default = "default_profile_cooldown_seconds")]
    pub cooldown_seconds: f64,
    #[serde(default = "default_profile_poll_interval_seconds")]
    pub poll_interval_seconds: f64,
    #[serde(default = "default_profile_template_workers")]
    pub template_workers: usize,
    #[serde(default = "default_profile_source_workers")]
    pub source_workers: usize,
    #[serde(default = "default_profile_min_idle_seconds")]
    pub min_idle_seconds: f64,
    #[serde(default = "default_profile_alarm_beep")]
    pub beep: bool,
    #[serde(default = "default_profile_beep_seconds")]
    pub beep_seconds: f64,
    #[serde(default = "default_profile_beep_volume")]
    pub beep_volume: i32,
    #[serde(default = "default_profile_max_templates")]
    pub max_templates: usize,
    #[serde(default = "default_profile_max_alerts")]
    pub max_alerts: Option<u32>,
}

impl Default for ProfileWatchConfigOptions {
    fn default() -> Self {
        Self {
            regions: Vec::new(),
            windows: Vec::new(),
            window_apps: Vec::new(),
            threshold: default_profile_threshold(),
            scales: default_profile_scales(),
            cooldown_seconds: default_profile_cooldown_seconds(),
            poll_interval_seconds: default_profile_poll_interval_seconds(),
            template_workers: default_profile_template_workers(),
            source_workers: default_profile_source_workers(),
            min_idle_seconds: default_profile_min_idle_seconds(),
            beep: default_profile_alarm_beep(),
            beep_seconds: default_profile_beep_seconds(),
            beep_volume: default_profile_beep_volume(),
            max_templates: default_profile_max_templates(),
            max_alerts: default_profile_max_alerts(),
        }
    }
}

pub fn profiles_dir(data_dir: impl AsRef<Path>) -> PathBuf {
    data_dir.as_ref().join("profiles")
}

pub fn profile_path(data_dir: impl AsRef<Path>, number: u32) -> PathBuf {
    profiles_dir(data_dir).join(format!("profile_{number}.json"))
}

pub fn templates_dir(data_dir: impl AsRef<Path>) -> PathBuf {
    data_dir.as_ref().join("templates")
}

pub fn screenshots_dir(data_dir: impl AsRef<Path>) -> PathBuf {
    data_dir.as_ref().join("screenshots")
}

pub fn state_path(data_dir: impl AsRef<Path>) -> PathBuf {
    data_dir.as_ref().join("state.json")
}

pub fn template_name(profile: u32, count: u32, stamp: Option<&str>) -> String {
    format!(
        "{}-{}-{}",
        profile,
        count,
        stamp.map(str::to_string).unwrap_or_else(template_stamp)
    )
}

pub fn template_stamp() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    // The Python app uses a 20-digit timestamp. This keeps the same sortable
    // width without depending on local time formatting in the core crate.
    format!("{:014}{:06}", elapsed.as_secs(), elapsed.subsec_micros())
}

pub fn template_suffix(name_or_path: &str) -> Option<String> {
    let stem = Path::new(name_or_path)
        .file_stem()
        .and_then(|item| item.to_str())
        .unwrap_or(name_or_path);
    let mut parts = stem.splitn(3, '-');
    let profile = parts.next()?;
    let count = parts.next()?;
    let suffix = parts.next()?;
    if profile.parse::<u32>().is_ok() && count.parse::<u32>().is_ok() && !suffix.is_empty() {
        Some(suffix.to_string())
    } else {
        None
    }
}

pub fn target_identity(id: Option<&str>, path: Option<&str>, name: Option<&str>) -> String {
    if let Some(id) = id.filter(|item| !item.is_empty()) {
        return id.to_string();
    }
    for value in [path, name].into_iter().flatten() {
        if let Some(suffix) = template_suffix(value) {
            return suffix;
        }
    }
    name.and_then(|value| {
        Path::new(value)
            .file_stem()
            .and_then(|item| item.to_str())
            .map(str::to_string)
    })
    .filter(|value| !value.is_empty())
    .or_else(|| {
        path.and_then(|value| {
            Path::new(value)
                .file_stem()
                .and_then(|item| item.to_str())
                .map(str::to_string)
        })
    })
    .filter(|value| !value.is_empty())
    .unwrap_or_else(template_stamp)
}

pub fn window_key(title: &str, ordinal: u32) -> String {
    format!("{title}\0{ordinal}")
}

pub fn target_identity_from_record(target: &Map<String, Value>) -> String {
    target_identity(
        value_str(target, "id").as_deref(),
        value_str(target, "path").as_deref(),
        value_str(target, "name").as_deref(),
    )
}

pub fn target_enabled(target: &Map<String, Value>) -> bool {
    target
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

pub fn enabled_target_count(targets: &[Map<String, Value>]) -> usize {
    targets
        .iter()
        .filter(|target| target_enabled(target))
        .count()
}

pub fn profile_template_targets_for_detection(
    targets: &[Map<String, Value>],
    threshold: f32,
    scales: ScaleSpec,
) -> Vec<TargetConfig> {
    targets
        .iter()
        .filter(|target| target_enabled(target))
        .filter_map(|target| {
            let path = value_str(target, "path")?;
            let name = value_str(target, "name").unwrap_or_else(|| {
                Path::new(&path)
                    .file_stem()
                    .and_then(|item| item.to_str())
                    .unwrap_or("template")
                    .to_string()
            });
            Some(TargetConfig::Template {
                id: Some(target_identity_from_record(target)),
                name,
                path,
                threshold,
                scales: scales.clone(),
                extra: Map::new(),
            })
        })
        .collect()
}

pub fn profile_watch_config_from_targets(
    targets: &[Map<String, Value>],
    data_dir: impl AsRef<Path>,
    options: ProfileWatchConfigOptions,
) -> io::Result<WatchConfig> {
    if targets.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile.targets is empty",
        ));
    }
    if options.regions.is_empty() && options.windows.is_empty() && options.window_apps.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one screen region, window, or remembered app is required",
        ));
    }

    let detector_targets =
        profile_template_targets_for_detection(targets, options.threshold, options.scales.clone());
    if detector_targets.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one enabled profile target is required",
        ));
    }

    let mut extra = Map::new();
    extra.insert(
        "_base_dir".to_string(),
        Value::String(data_dir.as_ref().to_string_lossy().to_string()),
    );
    extra.insert(
        "source_workers".to_string(),
        Value::Number(Number::from(options.source_workers as u64)),
    );
    if let Some(value) = Number::from_f64(options.min_idle_seconds) {
        extra.insert("min_idle_seconds".to_string(), Value::Number(value));
    }

    let config = WatchConfig {
        poll_interval_seconds: options.poll_interval_seconds,
        cooldown_seconds: options.cooldown_seconds,
        template_workers: options.template_workers,
        regions: options.regions,
        windows: options.windows,
        window_apps: options.window_apps,
        targets: detector_targets,
        alarm: AlarmConfig {
            beep: options.beep,
            beep_seconds: options.beep_seconds,
            beep_volume: options.beep_volume,
            save_dir: "screenshots".to_string(),
            jsonl: "alerts.jsonl".to_string(),
            max_alerts: options.max_alerts,
            extra: Map::new(),
        },
        extra,
    };
    config
        .validate()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    Ok(config)
}

pub fn profile_watch_config_at(
    profile_path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
    options: ProfileWatchConfigOptions,
) -> io::Result<WatchConfig> {
    let targets = read_profile_targets(profile_path)?;
    profile_watch_config_from_targets(&targets, data_dir, options)
}

pub fn read_profile_at(profile_path: impl AsRef<Path>) -> io::Result<ProfileReadResult> {
    let profile_path = profile_path.as_ref();
    let text = match fs::read_to_string(profile_path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(profile_read_result(false, Value::Object(Map::new())));
        }
        Err(err) => return Err(err),
    };
    let data: Value = serde_json::from_str(&text)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(profile_read_result(true, data))
}

pub fn read_profile_state_at(data_dir: impl AsRef<Path>) -> io::Result<ProfileStateResult> {
    let state = read_profile_data(state_path(data_dir))?;
    Ok(profile_state_result(state))
}

pub fn save_last_profile_at(
    data_dir: impl AsRef<Path>,
    profile_number: u32,
) -> io::Result<ProfileStateResult> {
    if !(1..=PROFILE_COUNT).contains(&profile_number) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("profile_number must be between 1 and {PROFILE_COUNT}"),
        ));
    }
    let path = state_path(data_dir);
    let mut state = read_profile_data(&path)?;
    let object = ensure_profile_object(&mut state)?;
    object.insert(
        "last_profile".to_string(),
        Value::Number(Number::from(profile_number as u64)),
    );
    write_profile_data(&path, &state)?;
    Ok(profile_state_result(state))
}

pub fn parse_window_geometry(value: &str) -> io::Result<WindowGeometry> {
    fn invalid() -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "window geometry must use WIDTHxHEIGHT+X+Y",
        )
    }

    let value = value.trim();
    let x_separator = value.find('x').ok_or_else(invalid)?;
    let width = value[..x_separator].parse::<u32>().map_err(|_| invalid())?;
    let rest = &value[x_separator + 1..];
    let coord_start = rest
        .char_indices()
        .find(|(index, ch)| *index > 0 && (*ch == '+' || *ch == '-'))
        .map(|(index, _)| index)
        .ok_or_else(invalid)?;
    let height = rest[..coord_start].parse::<u32>().map_err(|_| invalid())?;
    let coords = &rest[coord_start..];
    let y_start = coords
        .char_indices()
        .skip(1)
        .find(|(_, ch)| *ch == '+' || *ch == '-')
        .map(|(index, _)| index)
        .ok_or_else(invalid)?;
    let x = coords[..y_start].parse::<i32>().map_err(|_| invalid())?;
    let y = coords[y_start..].parse::<i32>().map_err(|_| invalid())?;
    WindowGeometry::new(width, height, x, y)
}

pub fn read_window_geometry_at(data_dir: impl AsRef<Path>) -> io::Result<Option<WindowGeometry>> {
    let state = read_profile_data(state_path(data_dir))?;
    let Some(geometry) = state
        .as_object()
        .and_then(|object| object.get("layout"))
        .and_then(Value::as_object)
        .and_then(|layout| layout.get("geometry"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    parse_window_geometry(geometry).map(Some).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid saved window geometry: {err}"),
        )
    })
}

pub fn save_window_geometry_at(
    data_dir: impl AsRef<Path>,
    geometry: WindowGeometry,
) -> io::Result<ProfileStateResult> {
    let path = state_path(data_dir);
    let mut state = read_profile_data(&path)?;
    let object = ensure_profile_object(&mut state)?;
    let layout = object
        .entry("layout".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !layout.is_object() {
        *layout = Value::Object(Map::new());
    }
    let layout_object = layout
        .as_object_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "layout is not an object"))?;
    layout_object.insert(
        "geometry".to_string(),
        Value::String(geometry.to_python_geometry()),
    );
    write_profile_data(&path, &state)?;
    Ok(profile_state_result(state))
}

pub fn save_profile_sources_at(
    profile_path: impl AsRef<Path>,
    options: ProfileWatchConfigOptions,
) -> io::Result<ProfileSourcesSaveResult> {
    let profile_path = profile_path.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let original = data.clone();
    let object = ensure_profile_object(&mut data)?;
    object.insert(
        "monitors".to_string(),
        Value::Array(
            options
                .regions
                .iter()
                .map(|region| Value::Number(Number::from(region.monitor as i64)))
                .collect(),
        ),
    );
    if let Some(region) = options.regions.first() {
        object.insert("region".to_string(), Value::Object(profile_region(region)));
    }
    object.insert(
        "windows".to_string(),
        Value::Array(
            profile_window_apps(&options)
                .into_iter()
                .map(profile_window_app)
                .collect(),
        ),
    );
    let match_value = object
        .entry("match".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    *match_value = Value::Object(profile_match(match_value.as_object(), &options)?);

    let changed = data != original;
    if changed {
        write_profile_data(profile_path, &data)?;
    }

    Ok(ProfileSourcesSaveResult {
        changed,
        profile: profile_read_result(profile_path.exists(), data),
    })
}

pub fn ensure_target_metadata(mut target: Map<String, Value>) -> (Map<String, Value>, bool) {
    let mut changed = false;
    if value_str(&target, "id")
        .filter(|item| !item.is_empty())
        .is_none()
    {
        target.insert(
            "id".to_string(),
            Value::String(target_identity_from_record(&target)),
        );
        changed = true;
    }
    let count = parse_hit_count(target.get("hit_count"));
    let normalized = Value::Number(Number::from(count));
    if target.get("hit_count") != Some(&normalized) {
        target.insert("hit_count".to_string(), normalized);
        changed = true;
    }
    (target, changed)
}

pub fn normalize_target_record(target: Map<String, Value>) -> (Map<String, Value>, bool) {
    let (mut target, metadata_changed) = ensure_target_metadata(target);
    let thumb_removed = target.remove("thumb").is_some();
    (target, metadata_changed || thumb_removed)
}

pub fn delete_target_files(
    target: &Map<String, Value>,
    data_dir: impl AsRef<Path>,
) -> io::Result<usize> {
    let Some(path) = value_str(target, "path") else {
        return Ok(0);
    };
    let path = PathBuf::from(path);
    let templates = templates_dir(data_dir);
    if !is_under_existing(&path, &templates) {
        return Ok(0);
    }
    if path.exists() {
        fs::remove_file(path)?;
        Ok(1)
    } else {
        Ok(0)
    }
}

pub fn rename_target(
    target: Map<String, Value>,
    data_dir: impl AsRef<Path>,
    profile: u32,
    count: u32,
) -> io::Result<(Map<String, Value>, bool)> {
    let data_dir = data_dir.as_ref();
    let (mut target, metadata_changed) = ensure_target_metadata(target);
    let path = value_str(&target, "path").map(PathBuf::from);
    let Some(path) = path else {
        let thumb_removed = target.remove("thumb").is_some();
        return Ok((target, metadata_changed || thumb_removed));
    };
    let templates = templates_dir(data_dir);
    if !path.exists() || !is_under_existing(&path, &templates) {
        let thumb_removed = target.remove("thumb").is_some();
        return Ok((target, metadata_changed || thumb_removed));
    }

    let old_name = value_str(&target, "name");
    let old_path = path.canonicalize()?;
    let suffix = path
        .file_name()
        .and_then(|item| item.to_str())
        .and_then(template_suffix)
        .unwrap_or_else(template_stamp);
    let stem = available_template_stem(data_dir, profile, count, &suffix, Some(&old_path))?;
    let new_path = templates.join(format!("{stem}.png"));
    let changed_path = old_path != new_path.canonicalize().unwrap_or_else(|_| new_path.clone());
    let thumb_removed = target.remove("thumb").is_some();
    if changed_path {
        fs::create_dir_all(&templates)?;
        fs::rename(&path, &new_path)?;
        target.insert(
            "path".to_string(),
            Value::String(new_path.to_string_lossy().to_string()),
        );
    } else {
        target.insert(
            "path".to_string(),
            Value::String(path.to_string_lossy().to_string()),
        );
    }
    target.insert("name".to_string(), Value::String(stem.clone()));
    let changed = changed_path
        || old_name.as_deref() != Some(stem.as_str())
        || metadata_changed
        || thumb_removed;
    Ok((target, changed))
}

pub fn normalize_target_names(
    targets: Vec<Map<String, Value>>,
    data_dir: impl AsRef<Path>,
    profile: u32,
) -> io::Result<(Vec<Map<String, Value>>, bool)> {
    let data_dir = data_dir.as_ref();
    let mut changed = false;
    let mut renamed = Vec::with_capacity(targets.len());
    for (index, target) in targets.into_iter().enumerate() {
        let (target, did_change) = rename_target(target, data_dir, profile, index as u32 + 1)?;
        changed |= did_change;
        renamed.push(target);
    }
    Ok((renamed, changed))
}

pub fn normalize_profile_file_at(
    profile_path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
    profile: u32,
) -> io::Result<bool> {
    let profile_path = profile_path.as_ref();
    let text = match fs::read_to_string(profile_path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    let mut data: Value = serde_json::from_str(&text)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let Some(object) = data.as_object_mut() else {
        return Ok(false);
    };
    let original_targets = object
        .get("targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut kept = Vec::new();
    let mut record_changed = false;
    for target in &original_targets {
        if let Some(target) = target.as_object() {
            if value_str(target, "path")
                .map(|path| PathBuf::from(path).exists())
                .unwrap_or(false)
            {
                let (target, did_change) = normalize_target_record(target.clone());
                record_changed |= did_change;
                kept.push(target);
            }
        }
    }
    let (kept, renamed_changed) = normalize_target_names(kept, data_dir, profile)?;
    let changed = record_changed || renamed_changed || kept.len() != original_targets.len();
    if changed {
        object.insert(
            "targets".to_string(),
            Value::Array(kept.into_iter().map(Value::Object).collect()),
        );
        let serialized = serde_json::to_string_pretty(&data)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        if let Some(parent) = profile_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(profile_path, format!("{serialized}\n"))?;
    }
    Ok(changed)
}

pub fn add_profile_template_pngs_at(
    profile_path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
    profile: u32,
    source_paths: &[PathBuf],
    max_templates: usize,
) -> io::Result<AddTemplateImagesResult> {
    let frames = source_paths
        .iter()
        .map(|source_path| {
            RgbFrame::from_image_path(source_path)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        })
        .collect::<io::Result<Vec<_>>>()?;

    add_profile_template_frames_at(profile_path, data_dir, profile, &frames, max_templates)
}

pub fn add_profile_template_frames_at(
    profile_path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
    profile: u32,
    frames: &[RgbFrame],
    max_templates: usize,
) -> io::Result<AddTemplateImagesResult> {
    if max_templates < 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_templates must be >= 1",
        ));
    }
    if frames.is_empty() {
        let targets = read_profile_targets(profile_path)?;
        return Ok(AddTemplateImagesResult {
            changed: false,
            added_count: 0,
            pruned_count: 0,
            selected_index: targets.len().checked_sub(1),
            targets: targets.into_iter().map(Value::Object).collect(),
        });
    }

    let profile_path = profile_path.as_ref();
    let data_dir = data_dir.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let object = ensure_profile_object(&mut data)?;
    let raw_targets = object
        .get("targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut targets = raw_targets
        .into_iter()
        .filter_map(|item| item.as_object().cloned())
        .collect::<Vec<_>>();
    let changed = true;
    let mut pruned_count = 0usize;

    for frame in frames {
        pruned_count += prune_targets(&mut targets, data_dir, max_templates.saturating_sub(1))?;
        let (normalized, _) = normalize_target_names(targets, data_dir, profile)?;
        targets = normalized;

        let stem = available_template_stem(
            data_dir,
            profile,
            targets.len() as u32 + 1,
            &template_stamp(),
            None,
        )?;
        let target_path = templates_dir(data_dir).join(format!("{stem}.png"));
        write_rgb_png(&target_path, frame)?;

        let target = object_from_pairs([
            ("name", Value::String(stem)),
            (
                "path",
                Value::String(target_path.to_string_lossy().to_string()),
            ),
            (
                "size",
                Value::String(format!("{}x{}", frame.width, frame.height)),
            ),
            ("enabled", Value::Bool(true)),
        ]);
        let (target, _) = ensure_target_metadata(target);
        targets.push(target);
    }

    object.insert(
        "targets".to_string(),
        Value::Array(targets.iter().cloned().map(Value::Object).collect()),
    );
    write_profile_data(profile_path, &data)?;

    Ok(AddTemplateImagesResult {
        changed,
        added_count: frames.len(),
        pruned_count,
        selected_index: targets.len().checked_sub(1),
        targets: targets.into_iter().map(Value::Object).collect(),
    })
}

pub fn reorder_profile_target_at(
    profile_path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
    profile: u32,
    from_index: usize,
    insert_index: usize,
) -> io::Result<ProfileTargetsEditResult> {
    let profile_path = profile_path.as_ref();
    let data_dir = data_dir.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let targets = profile_targets_from_data(&data);
    if from_index >= targets.len() {
        return Ok(ProfileTargetsEditResult {
            changed: false,
            deleted_files: 0,
            selected_index: None,
            targets: targets.into_iter().map(Value::Object).collect(),
        });
    }
    let insert_index = insert_index.min(targets.len());
    if insert_index == from_index || insert_index == from_index + 1 {
        return Ok(ProfileTargetsEditResult {
            changed: false,
            deleted_files: 0,
            selected_index: Some(from_index),
            targets: targets.into_iter().map(Value::Object).collect(),
        });
    }

    let mut reordered = targets;
    let target = reordered.remove(from_index);
    let selected_index = if from_index < insert_index {
        insert_index - 1
    } else {
        insert_index
    };
    reordered.insert(selected_index, target);
    let (targets, _) = normalize_target_names(reordered, data_dir, profile)?;
    write_targets_to_profile(profile_path, &mut data, &targets)?;

    Ok(ProfileTargetsEditResult {
        changed: true,
        deleted_files: 0,
        selected_index: Some(selected_index),
        targets: targets.into_iter().map(Value::Object).collect(),
    })
}

pub fn remove_profile_target_at(
    profile_path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
    profile: u32,
    index: usize,
) -> io::Result<ProfileTargetsEditResult> {
    let profile_path = profile_path.as_ref();
    let data_dir = data_dir.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let mut targets = profile_targets_from_data(&data);
    if index >= targets.len() {
        return Ok(ProfileTargetsEditResult {
            changed: false,
            deleted_files: 0,
            selected_index: None,
            targets: targets.into_iter().map(Value::Object).collect(),
        });
    }

    let removed = targets.remove(index);
    let deleted_files = delete_target_files(&removed, data_dir)?;
    let (targets, _) = normalize_target_names(targets, data_dir, profile)?;
    write_targets_to_profile(profile_path, &mut data, &targets)?;

    Ok(ProfileTargetsEditResult {
        changed: true,
        deleted_files,
        selected_index: None,
        targets: targets.into_iter().map(Value::Object).collect(),
    })
}

pub fn clear_profile_targets_at(
    profile_path: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
) -> io::Result<ProfileTargetsEditResult> {
    let profile_path = profile_path.as_ref();
    let data_dir = data_dir.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let targets = profile_targets_from_data(&data);
    if targets.is_empty() {
        return Ok(ProfileTargetsEditResult {
            changed: false,
            deleted_files: 0,
            selected_index: None,
            targets: Vec::new(),
        });
    }

    let mut deleted_files = 0usize;
    for target in &targets {
        deleted_files += delete_target_files(target, data_dir)?;
    }
    write_targets_to_profile(profile_path, &mut data, &[])?;

    Ok(ProfileTargetsEditResult {
        changed: true,
        deleted_files,
        selected_index: None,
        targets: Vec::new(),
    })
}

pub fn set_profile_target_enabled_at(
    profile_path: impl AsRef<Path>,
    index: usize,
    enabled: bool,
) -> io::Result<ProfileTargetsEnabledResult> {
    let profile_path = profile_path.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let mut targets = profile_targets_from_data(&data);
    let mut changed = false;
    if let Some(target) = targets.get_mut(index) {
        let enabled_value = Value::Bool(enabled);
        if target.get("enabled") != Some(&enabled_value) {
            target.insert("enabled".to_string(), enabled_value);
            changed = true;
        }
    }
    if changed {
        write_targets_to_profile(profile_path, &mut data, &targets)?;
    }
    Ok(profile_targets_enabled_result(changed, targets))
}

pub fn toggle_all_profile_targets_at(
    profile_path: impl AsRef<Path>,
) -> io::Result<ProfileTargetsEnabledResult> {
    let profile_path = profile_path.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let mut targets = profile_targets_from_data(&data);
    let all_selected = !targets.is_empty() && targets.iter().all(target_enabled);
    let next_enabled = !all_selected;
    let mut changed = false;
    for target in &mut targets {
        let enabled_value = Value::Bool(next_enabled);
        if target.get("enabled") != Some(&enabled_value) {
            target.insert("enabled".to_string(), enabled_value);
            changed = true;
        }
    }
    if changed {
        write_targets_to_profile(profile_path, &mut data, &targets)?;
    }
    Ok(profile_targets_enabled_result(changed, targets))
}

pub fn record_target_hits(targets: &mut [Map<String, Value>], target_ids: &[String]) -> bool {
    let mut counts: HashMap<&str, u64> = HashMap::new();
    for target_id in target_ids {
        *counts.entry(target_id.as_str()).or_insert(0) += 1;
    }
    if counts.is_empty() {
        return false;
    }

    let mut changed = false;
    for target in targets {
        let key = target_identity_from_record(target);
        let Some(delta) = counts.get(key.as_str()).copied() else {
            continue;
        };
        let current = parse_hit_count(target.get("hit_count"));
        target.insert(
            "hit_count".to_string(),
            Value::Number(Number::from(current.saturating_add(delta))),
        );
        changed = true;
    }
    changed
}

pub fn record_profile_hits_at(
    profile_path: impl AsRef<Path>,
    target_ids: &[String],
) -> io::Result<bool> {
    update_profile_targets(profile_path, |targets| {
        record_target_hits(targets, target_ids)
    })
}

pub fn clear_target_hit_count(targets: &mut [Map<String, Value>], target_id: &str) -> bool {
    let mut changed = false;
    for target in targets {
        if target_identity_from_record(target) == target_id {
            let current = parse_hit_count(target.get("hit_count"));
            if current != 0 {
                target.insert("hit_count".to_string(), Value::Number(Number::from(0)));
                changed = true;
            }
        }
    }
    changed
}

pub fn clear_profile_target_hit_count_at(
    profile_path: impl AsRef<Path>,
    target_id: &str,
) -> io::Result<ProfileTargetsEditResult> {
    let profile_path = profile_path.as_ref();
    let mut data = read_profile_data(profile_path)?;
    let mut targets = profile_targets_from_data(&data);
    let selected_index = targets
        .iter()
        .position(|target| target_identity_from_record(target) == target_id);
    let changed = clear_target_hit_count(&mut targets, target_id);
    if changed {
        write_targets_to_profile(profile_path, &mut data, &targets)?;
    }

    Ok(ProfileTargetsEditResult {
        changed,
        deleted_files: 0,
        selected_index,
        targets: targets.into_iter().map(Value::Object).collect(),
    })
}

fn update_profile_targets(
    profile_path: impl AsRef<Path>,
    update: impl FnOnce(&mut [Map<String, Value>]) -> bool,
) -> io::Result<bool> {
    let profile_path = profile_path.as_ref();
    let text = match fs::read_to_string(profile_path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    let mut data: Value = serde_json::from_str(&text)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let Some(object) = data.as_object_mut() else {
        return Ok(false);
    };
    let Some(items) = object.get("targets").and_then(Value::as_array).cloned() else {
        return Ok(false);
    };
    let mut targets = items
        .into_iter()
        .filter_map(|item| item.as_object().cloned())
        .collect::<Vec<_>>();
    let changed = update(&mut targets);
    if changed {
        object.insert(
            "targets".to_string(),
            Value::Array(targets.into_iter().map(Value::Object).collect()),
        );
        let serialized = serde_json::to_string_pretty(&data)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(profile_path, format!("{serialized}\n"))?;
    }
    Ok(changed)
}

fn prune_targets(
    targets: &mut Vec<Map<String, Value>>,
    data_dir: &Path,
    keep_count: usize,
) -> io::Result<usize> {
    if targets.len() <= keep_count {
        return Ok(0);
    }
    let remove_count = targets.len() - keep_count;
    let removed = targets.drain(..remove_count).collect::<Vec<_>>();
    for target in &removed {
        delete_target_files(target, data_dir)?;
    }
    Ok(remove_count)
}

fn read_profile_targets(profile_path: impl AsRef<Path>) -> io::Result<Vec<Map<String, Value>>> {
    let data = read_profile_data(profile_path)?;
    Ok(profile_targets_from_data(&data))
}

fn profile_targets_from_data(data: &Value) -> Vec<Map<String, Value>> {
    data.as_object()
        .and_then(|object| object.get("targets"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_object().cloned())
        .collect()
}

fn profile_targets_enabled_result(
    changed: bool,
    targets: Vec<Map<String, Value>>,
) -> ProfileTargetsEnabledResult {
    let enabled_count = enabled_target_count(&targets);
    let all_enabled = !targets.is_empty() && enabled_count == targets.len();
    ProfileTargetsEnabledResult {
        changed,
        enabled_count,
        all_enabled,
        targets: targets.into_iter().map(Value::Object).collect(),
    }
}

fn profile_read_result(exists: bool, profile: Value) -> ProfileReadResult {
    let targets = profile_targets_from_data(&profile);
    let enabled_count = enabled_target_count(&targets);
    let all_enabled = !targets.is_empty() && enabled_count == targets.len();
    ProfileReadResult {
        exists,
        enabled_count,
        all_enabled,
        targets: targets.into_iter().map(Value::Object).collect(),
        profile,
    }
}

fn profile_state_result(state: Value) -> ProfileStateResult {
    let last_profile = state
        .as_object()
        .and_then(|object| object.get("last_profile"))
        .and_then(Value::as_u64)
        .map(|value| value as u32)
        .filter(|value| (1..=PROFILE_COUNT).contains(value))
        .unwrap_or(1);
    ProfileStateResult {
        last_profile,
        state,
    }
}

fn profile_region(region: &RegionConfig) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert(
        "left".to_string(),
        Value::Number(Number::from(region.left as i64)),
    );
    out.insert(
        "top".to_string(),
        Value::Number(Number::from(region.top as i64)),
    );
    if let Some(width) = region.width {
        out.insert(
            "width".to_string(),
            Value::Number(Number::from(width as u64)),
        );
    }
    if let Some(height) = region.height {
        out.insert(
            "height".to_string(),
            Value::Number(Number::from(height as u64)),
        );
    }
    out
}

fn profile_match(
    existing: Option<&Map<String, Value>>,
    options: &ProfileWatchConfigOptions,
) -> io::Result<Map<String, Value>> {
    let mut out = existing.cloned().unwrap_or_default();
    out.insert(
        "threshold".to_string(),
        finite_number(round_six(f64::from(options.threshold)), "threshold")?,
    );
    out.insert(
        "scales".to_string(),
        serde_json::to_value(&options.scales)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?,
    );
    out.insert(
        "interval_ms".to_string(),
        Value::Number(Number::from(
            (options.poll_interval_seconds * 1000.0).round().max(0.0) as u64,
        )),
    );
    out.insert(
        "cooldown".to_string(),
        finite_number(options.cooldown_seconds, "cooldown")?,
    );
    out.insert("beep".to_string(), Value::Bool(options.beep));
    out.insert(
        "beep_seconds".to_string(),
        finite_number(options.beep_seconds, "beep_seconds")?,
    );
    out.insert(
        "beep_volume".to_string(),
        Value::Number(Number::from(options.beep_volume as i64)),
    );
    out.insert(
        "max_templates".to_string(),
        Value::Number(Number::from(options.max_templates as u64)),
    );
    if let Some(max_alerts) = options.max_alerts {
        out.insert(
            "max_alerts".to_string(),
            Value::Number(Number::from(max_alerts as u64)),
        );
    }
    Ok(out)
}

fn round_six(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn profile_window_apps(options: &ProfileWatchConfigOptions) -> Vec<WindowAppConfig> {
    if !options.window_apps.is_empty() {
        return options.window_apps.clone();
    }
    options
        .windows
        .iter()
        .filter(|window| !window.title.trim().is_empty())
        .map(|window| WindowAppConfig {
            title: window.title.clone(),
            ordinal: window
                .extra
                .get("ordinal")
                .and_then(Value::as_u64)
                .map(|value| value.max(1) as u32)
                .unwrap_or(1),
            extra: Map::new(),
        })
        .collect()
}

fn profile_window_app(app: WindowAppConfig) -> Value {
    let mut out = app.extra;
    out.insert("title".to_string(), Value::String(app.title));
    out.insert(
        "ordinal".to_string(),
        Value::Number(Number::from(app.ordinal.max(1) as u64)),
    );
    Value::Object(out)
}

fn finite_number(value: f64, field: &str) -> io::Result<Value> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{field} is invalid")))
}

fn read_profile_data(profile_path: impl AsRef<Path>) -> io::Result<Value> {
    let profile_path = profile_path.as_ref();
    let text = match fs::read_to_string(profile_path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Value::Object(Map::new())),
        Err(err) => return Err(err),
    };
    serde_json::from_str(&text).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn ensure_profile_object(data: &mut Value) -> io::Result<&mut Map<String, Value>> {
    if !data.is_object() {
        *data = Value::Object(Map::new());
    }
    data.as_object_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "profile JSON is not an object"))
}

fn write_profile_data(profile_path: impl AsRef<Path>, data: &Value) -> io::Result<()> {
    let profile_path = profile_path.as_ref();
    if let Some(parent) = profile_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(data)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(profile_path, format!("{serialized}\n"))
}

fn write_targets_to_profile(
    profile_path: impl AsRef<Path>,
    data: &mut Value,
    targets: &[Map<String, Value>],
) -> io::Result<()> {
    let object = ensure_profile_object(data)?;
    object.insert(
        "targets".to_string(),
        Value::Array(targets.iter().cloned().map(Value::Object).collect()),
    );
    write_profile_data(profile_path, data)
}

fn write_rgb_png(path: &Path, frame: &RgbFrame) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&frame.pixels)?;
    Ok(())
}

fn object_from_pairs<const N: usize>(pairs: [(&str, Value); N]) -> Map<String, Value> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

impl From<DetectError> for io::Error {
    fn from(err: DetectError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

fn available_template_stem(
    data_dir: &Path,
    profile: u32,
    count: u32,
    suffix: &str,
    current_path: Option<&Path>,
) -> io::Result<String> {
    let templates = templates_dir(data_dir);
    let mut suffix = suffix.to_string();
    loop {
        let stem = template_name(profile, count, Some(&suffix));
        let candidate = templates.join(format!("{stem}.png"));
        let path_ok = if !candidate.exists() {
            true
        } else if let Some(current_path) = current_path {
            candidate.canonicalize()? == current_path
        } else {
            false
        };
        if path_ok {
            return Ok(stem);
        }
        suffix = template_stamp();
    }
}

fn value_str(target: &Map<String, Value>, key: &str) -> Option<String> {
    target.get(key).and_then(Value::as_str).map(str::to_string)
}

fn parse_hit_count(value: Option<&Value>) -> u64 {
    let parsed = match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .map(|item| item.max(0) as u64)
            .or_else(|| number.as_u64())
            .or_else(|| number.as_f64().map(|item| item.max(0.0) as u64)),
        Some(Value::String(text)) => text.parse::<i64>().ok().map(|item| item.max(0) as u64),
        _ => None,
    };
    parsed.unwrap_or(0)
}

fn is_under_existing(path: &Path, parent: &Path) -> bool {
    match (path.canonicalize(), parent.canonicalize()) {
        (Ok(path), Ok(parent)) => path.starts_with(parent),
        _ => false,
    }
}

fn default_profile_threshold() -> f32 {
    0.90
}

fn default_profile_scales() -> ScaleSpec {
    ScaleSpec::Text("1.0".to_string())
}

fn default_profile_cooldown_seconds() -> f64 {
    1.0
}

fn default_profile_poll_interval_seconds() -> f64 {
    0.25
}

fn default_profile_template_workers() -> usize {
    PROFILE_TEMPLATE_WORKERS
}

fn default_profile_source_workers() -> usize {
    PROFILE_SOURCE_WORKERS
}

fn default_profile_min_idle_seconds() -> f64 {
    PROFILE_MIN_IDLE_SECONDS
}

fn default_profile_alarm_beep() -> bool {
    true
}

fn default_profile_beep_seconds() -> f64 {
    3.0
}

fn default_profile_beep_volume() -> i32 {
    100
}

fn default_profile_max_templates() -> usize {
    100
}

fn default_profile_max_alerts() -> Option<u32> {
    Some(50)
}

#[cfg(test)]
mod tests {
    use super::{
        add_profile_template_frames_at, add_profile_template_pngs_at,
        clear_profile_target_hit_count_at, clear_profile_targets_at, clear_target_hit_count,
        delete_target_files, ensure_target_metadata, normalize_profile_file_at,
        normalize_target_names, parse_window_geometry, profile_path,
        profile_template_targets_for_detection, profile_watch_config_at,
        profile_watch_config_from_targets, read_profile_at, read_profile_state_at,
        read_profile_targets, read_window_geometry_at, record_profile_hits_at, record_target_hits,
        remove_profile_target_at, reorder_profile_target_at, save_last_profile_at,
        save_profile_sources_at, save_window_geometry_at, screenshots_dir,
        set_profile_target_enabled_at, state_path, target_enabled, target_identity, template_name,
        template_stamp, template_suffix, templates_dir, toggle_all_profile_targets_at, window_key,
        write_rgb_png, ProfileWatchConfigOptions, WindowGeometry, PROFILE_COUNT,
        PROFILE_MIN_IDLE_SECONDS, PROFILE_SOURCE_WORKERS, PROFILE_TEMPLATE_WORKERS,
    };
    use crate::config::{
        RegionConfig, ScaleSpec, TargetConfig, WatchConfig, WindowAppConfig, WindowConfig,
    };
    use crate::detect::{PreparedDetector, RgbFrame};
    use serde_json::{json, Map, Value};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    #[test]
    fn profile_paths_match_python_app_data_layout() {
        let data = PathBuf::from(r"C:\Users\Wes\AppData\Local\ScreenWatchOCR");
        assert_eq!(
            profile_path(&data, 3),
            data.join("profiles").join("profile_3.json")
        );
        assert_eq!(templates_dir(&data), data.join("templates"));
        assert_eq!(screenshots_dir(&data), data.join("screenshots"));
        assert_eq!(state_path(&data), data.join("state.json"));
        assert_eq!(PROFILE_COUNT, 5);
    }

    #[test]
    fn profile_state_preserves_unknown_layout_and_updates_last_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            state_path(&data),
            serde_json::to_string_pretty(&json!({
                "last_profile": 2,
                "layout": {"geometry": "800x600+1+2"},
                "future": true
            }))
            .unwrap(),
        )
        .unwrap();

        let before = read_profile_state_at(&data).unwrap();
        assert_eq!(before.last_profile, 2);

        let saved = save_last_profile_at(&data, 5).unwrap();

        assert_eq!(saved.last_profile, 5);
        let stored: Value =
            serde_json::from_str(&fs::read_to_string(state_path(&data)).unwrap()).unwrap();
        assert_eq!(stored["last_profile"], json!(5));
        assert_eq!(stored["layout"]["geometry"], json!("800x600+1+2"));
        assert_eq!(stored["future"], json!(true));
    }

    #[test]
    fn profile_state_result_serializes_frontend_contract_without_changing_state_file_shape() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            state_path(&data),
            serde_json::to_string_pretty(&json!({
                "last_profile": 3,
                "layout": {"geometry": "1024x640+10+20"},
                "future": {"kept": true}
            }))
            .unwrap(),
        )
        .unwrap();

        let result = read_profile_state_at(&data).unwrap();
        let value = serde_json::to_value(result).unwrap();

        assert_eq!(value["lastProfile"], 3);
        assert!(value.get("last_profile").is_none());
        assert_eq!(value["state"]["last_profile"], json!(3));
        assert_eq!(value["state"]["layout"]["geometry"], "1024x640+10+20");
        assert_eq!(value["state"]["future"], json!({"kept": true}));
    }

    #[test]
    fn window_geometry_parses_and_formats_python_state_values() {
        assert_eq!(
            parse_window_geometry("1200x720+30+40").unwrap(),
            WindowGeometry {
                width: 1200,
                height: 720,
                x: 30,
                y: 40,
            }
        );
        assert_eq!(
            parse_window_geometry("980x680-120+80").unwrap(),
            WindowGeometry {
                width: 980,
                height: 680,
                x: -120,
                y: 80,
            }
        );
        assert_eq!(
            WindowGeometry::new(1400, 900, 120, -80)
                .unwrap()
                .to_python_geometry(),
            "1400x900+120-80"
        );
    }

    #[test]
    fn window_geometry_rejects_invalid_or_zero_sized_values() {
        assert!(parse_window_geometry("1200x720").is_err());
        assert!(parse_window_geometry("1200x720+30").is_err());
        assert!(parse_window_geometry("0x720+30+40").is_err());
        assert!(parse_window_geometry("1200x0+30+40").is_err());
    }

    #[test]
    fn window_geometry_state_updates_layout_without_losing_unknown_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            state_path(&data),
            serde_json::to_string_pretty(&json!({
                "last_profile": 4,
                "layout": {
                    "geometry": "800x600+1+2",
                    "main_ratio": 0.42,
                    "right_ratio": 0.25,
                    "future_layout": true
                },
                "future": {"kept": true}
            }))
            .unwrap(),
        )
        .unwrap();

        let saved =
            save_window_geometry_at(&data, WindowGeometry::new(1280, 760, 44, -12).unwrap())
                .unwrap();

        assert_eq!(saved.last_profile, 4);
        let stored: Value =
            serde_json::from_str(&fs::read_to_string(state_path(&data)).unwrap()).unwrap();
        assert_eq!(stored["layout"]["geometry"], json!("1280x760+44-12"));
        assert_eq!(stored["layout"]["main_ratio"], json!(0.42));
        assert_eq!(stored["layout"]["right_ratio"], json!(0.25));
        assert_eq!(stored["layout"]["future_layout"], json!(true));
        assert_eq!(stored["future"], json!({"kept": true}));
        assert_eq!(
            read_window_geometry_at(&data).unwrap(),
            Some(WindowGeometry {
                width: 1280,
                height: 760,
                x: 44,
                y: -12,
            })
        );
    }

    #[test]
    fn window_geometry_state_creates_python_layout_shape_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");

        let saved =
            save_window_geometry_at(&data, WindowGeometry::new(980, 680, 0, 0).unwrap()).unwrap();

        assert_eq!(saved.last_profile, 1);
        let stored: Value =
            serde_json::from_str(&fs::read_to_string(state_path(&data)).unwrap()).unwrap();
        assert_eq!(stored["layout"]["geometry"], json!("980x680+0+0"));
        assert!(read_window_geometry_at(&data).unwrap().is_some());
    }

    #[test]
    fn profile_sources_save_python_compatible_shape_and_preserve_profile_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [{"id": "target-a", "enabled": false}],
                "match": {"threshold": 0.77, "future_match": true},
                "future": {"kept": true}
            }))
            .unwrap(),
        )
        .unwrap();
        let options = ProfileWatchConfigOptions {
            regions: vec![
                RegionConfig {
                    name: "left".to_string(),
                    monitor: 2,
                    left: 11,
                    top: 22,
                    width: Some(333),
                    height: Some(444),
                    extra: Map::new(),
                },
                RegionConfig {
                    name: "right".to_string(),
                    monitor: 3,
                    left: 0,
                    top: 0,
                    width: None,
                    height: None,
                    extra: Map::new(),
                },
            ],
            window_apps: vec![WindowAppConfig {
                title: "Demo".to_string(),
                ordinal: 2,
                extra: Map::new(),
            }],
            threshold: 0.81,
            scales: ScaleSpec::Text("0.9,1.0,1.1".to_string()),
            poll_interval_seconds: 0.4,
            cooldown_seconds: 2.5,
            beep: false,
            beep_seconds: 4.0,
            beep_volume: 42,
            max_templates: 12,
            max_alerts: Some(7),
            ..ProfileWatchConfigOptions::default()
        };

        let result = save_profile_sources_at(&profile, options).unwrap();

        assert!(result.changed);
        assert_eq!(result.profile.enabled_count, 0);
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["targets"][0]["id"], json!("target-a"));
        assert_eq!(stored["targets"][0]["enabled"], json!(false));
        assert_eq!(stored["match"]["threshold"], json!(0.81));
        assert_eq!(stored["match"]["scales"], json!("0.9,1.0,1.1"));
        assert_eq!(stored["match"]["interval_ms"], json!(400));
        assert_eq!(stored["match"]["cooldown"], json!(2.5));
        assert_eq!(stored["match"]["beep"], json!(false));
        assert_eq!(stored["match"]["beep_seconds"], json!(4.0));
        assert_eq!(stored["match"]["beep_volume"], json!(42));
        assert_eq!(stored["match"]["max_templates"], json!(12));
        assert_eq!(stored["match"]["max_alerts"], json!(7));
        assert_eq!(stored["match"]["future_match"], json!(true));
        assert_eq!(stored["future"]["kept"], json!(true));
        assert_eq!(stored["monitors"], json!([2, 3]));
        assert_eq!(
            stored["region"],
            json!({"left": 11, "top": 22, "width": 333, "height": 444})
        );
        assert_eq!(stored["windows"], json!([{"title": "Demo", "ordinal": 2}]));
    }

    #[test]
    fn profile_sources_can_persist_no_selected_monitors_and_concrete_windows() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "monitors": [1],
                "region": {"left": 9, "top": 8},
                "windows": [{"title": "Old", "ordinal": 1}],
                "targets": []
            }))
            .unwrap(),
        )
        .unwrap();
        let mut extra = Map::new();
        extra.insert("ordinal".to_string(), json!(3));
        let options = ProfileWatchConfigOptions {
            windows: vec![WindowConfig {
                name: "app-Demo".to_string(),
                title: "Demo".to_string(),
                display: "Demo #3".to_string(),
                hwnd: Some(123),
                extra,
            }],
            ..ProfileWatchConfigOptions::default()
        };

        save_profile_sources_at(&profile, options).unwrap();

        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["monitors"], json!([]));
        assert_eq!(stored["region"], json!({"left": 9, "top": 8}));
        assert_eq!(stored["windows"], json!([{"title": "Demo", "ordinal": 3}]));
    }

    #[test]
    fn profile_sources_prefer_remembered_window_apps_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [{"id": "target-a"}],
                "windows": [{"title": "Old", "ordinal": 1}],
                "future": true
            }))
            .unwrap(),
        )
        .unwrap();
        let mut concrete_extra = Map::new();
        concrete_extra.insert("ordinal".to_string(), json!(9));
        let mut remembered_extra = Map::new();
        remembered_extra.insert("futureWindow".to_string(), json!("kept"));
        let options = ProfileWatchConfigOptions {
            windows: vec![WindowConfig {
                name: "concrete".to_string(),
                title: "Concrete".to_string(),
                display: "Concrete #9".to_string(),
                hwnd: Some(456),
                extra: concrete_extra,
            }],
            window_apps: vec![WindowAppConfig {
                title: "Remembered".to_string(),
                ordinal: 2,
                extra: remembered_extra,
            }],
            ..ProfileWatchConfigOptions::default()
        };

        save_profile_sources_at(&profile, options).unwrap();

        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(
            stored["windows"],
            json!([{"title": "Remembered", "ordinal": 2, "futureWindow": "kept"}])
        );
        assert_eq!(stored["future"], json!(true));
    }

    #[test]
    fn profile_options_deserialize_frontend_camel_case_and_window_ordinals() {
        let options: ProfileWatchConfigOptions = serde_json::from_value(json!({
            "regions": [],
            "windows": [
                {
                    "name": "Demo #4",
                    "title": "Demo",
                    "display": "Demo #4",
                    "hwnd": 123,
                    "ordinal": 4
                }
            ],
            "windowApps": [{"title": "Remembered", "ordinal": 2}],
            "pollIntervalSeconds": 0.2,
            "cooldownSeconds": 5.5,
            "templateWorkers": 3,
            "sourceWorkers": 2,
            "minIdleSeconds": 0.75,
            "beepSeconds": 1.5,
            "beepVolume": 30,
            "maxTemplates": 33,
            "maxAlerts": 9
        }))
        .unwrap();

        assert_eq!(options.windows.len(), 1);
        assert_eq!(options.windows[0].title, "Demo");
        assert_eq!(options.windows[0].hwnd, Some(123));
        assert_eq!(options.windows[0].extra["ordinal"], json!(4));
        assert_eq!(
            options.window_apps,
            vec![WindowAppConfig {
                title: "Remembered".to_string(),
                ordinal: 2,
                extra: Map::new(),
            }]
        );
        assert_eq!(options.poll_interval_seconds, 0.2);
        assert_eq!(options.cooldown_seconds, 5.5);
        assert_eq!(options.template_workers, 3);
        assert_eq!(options.source_workers, 2);
        assert_eq!(options.min_idle_seconds, 0.75);
        assert_eq!(options.beep_seconds, 1.5);
        assert_eq!(options.beep_volume, 30);
        assert_eq!(options.max_templates, 33);
        assert_eq!(options.max_alerts, Some(9));
    }

    #[test]
    fn template_names_keep_profile_count_suffix_shape() {
        assert_eq!(template_name(1, 11, Some("20260701")), "1-11-20260701");
        assert_eq!(template_stamp().len(), 20);
        assert!(template_stamp().chars().all(|item| item.is_ascii_digit()));
    }

    #[test]
    fn template_suffix_and_identity_follow_legacy_names() {
        assert_eq!(
            template_suffix(r"C:\data\templates\1-2-20260701194530123456.png"),
            Some("20260701194530123456".to_string())
        );
        assert_eq!(
            target_identity(None, Some("1-2-old-a.png"), Some("ignored")),
            "old-a"
        );
        assert_eq!(
            target_identity(Some("stable"), Some("1-2-old-a.png"), Some("ignored")),
            "stable"
        );
        assert_eq!(
            target_identity(None, None, Some("plain-name")),
            "plain-name"
        );
    }

    #[test]
    fn window_key_uses_the_legacy_nul_separator() {
        assert_eq!(window_key("Demo", 2), "Demo\02");
    }

    #[test]
    fn target_enabled_defaults_to_true_like_python_profiles() {
        assert!(target_enabled(&object(json!({"name": "implicit"}))));
        assert!(target_enabled(&object(json!({"enabled": true}))));
        assert!(!target_enabled(&object(json!({"enabled": false}))));
    }

    #[test]
    fn profile_template_targets_for_detection_uses_only_enabled_targets() {
        let targets = vec![
            object(json!({
                "id": "one-id",
                "name": "one",
                "path": "templates/one.png",
                "enabled": true,
                "future": "kept-in-profile-only"
            })),
            object(json!({
                "id": "two-id",
                "name": "two",
                "path": "templates/two.png",
                "enabled": false
            })),
            object(json!({
                "name": "implicit",
                "path": "templates/1-3-implicit.png"
            })),
        ];

        let config_targets =
            profile_template_targets_for_detection(&targets, 0.88, ScaleSpec::Text("1.0".into()));

        assert_eq!(config_targets.len(), 2);
        assert!(matches!(
            &config_targets[0],
            TargetConfig::Template { id, name, path, threshold, scales, .. }
                if id.as_deref() == Some("one-id")
                    && name == "one"
                    && path == "templates/one.png"
                    && (*threshold - 0.88).abs() < f32::EPSILON
                    && matches!(scales, ScaleSpec::Text(value) if value == "1.0")
        ));
        assert!(matches!(
            &config_targets[1],
            TargetConfig::Template { id, name, path, .. }
                if id.as_deref() == Some("implicit")
                    && name == "implicit"
                    && path == "templates/1-3-implicit.png"
        ));
    }

    #[test]
    fn profile_watch_config_uses_enabled_targets_and_gui_defaults() {
        let data = PathBuf::from(r"C:\Users\Wes\AppData\Local\ScreenWatchOCR");
        let targets = vec![
            object(json!({
                "id": "one-id",
                "name": "one",
                "path": "templates/one.png",
                "enabled": true
            })),
            object(json!({
                "id": "two-id",
                "name": "two",
                "path": "templates/two.png",
                "enabled": false
            })),
            object(json!({
                "name": "implicit",
                "path": "templates/1-3-implicit.png"
            })),
        ];
        let options = ProfileWatchConfigOptions {
            regions: vec![RegionConfig {
                name: "monitor-1".to_string(),
                monitor: 1,
                left: 10,
                top: 20,
                width: Some(300),
                height: Some(200),
                extra: Map::new(),
            }],
            ..ProfileWatchConfigOptions::default()
        };

        let config = profile_watch_config_from_targets(&targets, &data, options).unwrap();

        assert_eq!(config.targets.len(), 2);
        assert!(matches!(
            &config.targets[0],
            TargetConfig::Template { id, name, path, threshold, scales, .. }
                if id.as_deref() == Some("one-id")
                    && name == "one"
                    && path == "templates/one.png"
                    && (*threshold - 0.90).abs() < f32::EPSILON
                    && matches!(scales, ScaleSpec::Text(value) if value == "1.0")
        ));
        assert!(matches!(
            &config.targets[1],
            TargetConfig::Template { id, name, path, .. }
                if id.as_deref() == Some("implicit")
                    && name == "implicit"
                    && path == "templates/1-3-implicit.png"
        ));
        assert_eq!(config.poll_interval_seconds, 0.25);
        assert_eq!(config.cooldown_seconds, 1.0);
        assert_eq!(config.template_workers, PROFILE_TEMPLATE_WORKERS);
        assert_eq!(config.alarm.save_dir, "screenshots");
        assert_eq!(config.alarm.jsonl, "alerts.jsonl");
        assert_eq!(config.alarm.beep_seconds, 3.0);
        assert_eq!(config.alarm.beep_volume, 100);
        assert_eq!(config.alarm.max_alerts, Some(50));
        assert_eq!(
            config.extra["_base_dir"],
            json!(data.to_string_lossy().to_string())
        );
        assert_eq!(
            config.extra["source_workers"],
            json!(PROFILE_SOURCE_WORKERS)
        );
        assert!(
            (config.extra["min_idle_seconds"].as_f64().unwrap() - PROFILE_MIN_IDLE_SECONDS).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn profile_watch_config_keeps_concrete_and_remembered_window_sources_distinct() {
        let data = PathBuf::from(r"C:\Users\Wes\AppData\Local\ScreenWatchOCR");
        let targets = vec![object(json!({
            "id": "one-id",
            "name": "one",
            "path": "templates/one.png",
            "enabled": true
        }))];
        let mut window_extra = Map::new();
        window_extra.insert("ordinal".to_string(), json!(3));
        let options = ProfileWatchConfigOptions {
            windows: vec![WindowConfig {
                name: "Demo #3".to_string(),
                title: "Demo".to_string(),
                display: "Demo #3".to_string(),
                hwnd: Some(321),
                extra: window_extra,
            }],
            window_apps: vec![WindowAppConfig {
                title: "Remembered".to_string(),
                ordinal: 2,
                extra: Map::new(),
            }],
            source_workers: 4,
            min_idle_seconds: 0.6,
            ..ProfileWatchConfigOptions::default()
        };

        let config = profile_watch_config_from_targets(&targets, &data, options).unwrap();

        assert_eq!(config.regions, Vec::<RegionConfig>::new());
        assert_eq!(config.windows.len(), 1);
        assert_eq!(config.windows[0].title, "Demo");
        assert_eq!(config.windows[0].display, "Demo #3");
        assert_eq!(config.windows[0].hwnd, Some(321));
        assert_eq!(config.windows[0].extra["ordinal"], json!(3));
        assert_eq!(
            config.window_apps,
            vec![WindowAppConfig {
                title: "Remembered".to_string(),
                ordinal: 2,
                extra: Map::new(),
            }]
        );
        assert_eq!(config.extra["source_workers"], json!(4));
        assert_eq!(config.extra["min_idle_seconds"], json!(0.6));
        config.validate().unwrap();
    }

    #[test]
    fn profile_watch_config_at_reads_profile_and_reports_gui_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let profile = profile_path(&data, 1);
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let mut options = ProfileWatchConfigOptions {
            regions: vec![RegionConfig {
                name: "monitor-1".to_string(),
                monitor: 1,
                left: 0,
                top: 0,
                width: None,
                height: None,
                extra: Map::new(),
            }],
            ..ProfileWatchConfigOptions::default()
        };

        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({"targets": []})).unwrap(),
        )
        .unwrap();
        let err = profile_watch_config_at(&profile, &data, options.clone()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("profile.targets is empty"));

        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [
                    {"id": "a", "name": "a", "path": "templates/a.png", "enabled": false}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let err = profile_watch_config_at(&profile, &data, options.clone()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err
            .to_string()
            .contains("at least one enabled profile target"));

        options.regions.clear();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [
                    {"id": "a", "name": "a", "path": "templates/a.png", "enabled": true}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let err = profile_watch_config_at(&profile, &data, options).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err
            .to_string()
            .contains("at least one screen region, window, or remembered app"));
    }

    #[test]
    fn read_profile_reports_missing_file_without_creating_it() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile_1.json");

        let result = read_profile_at(&profile).unwrap();

        assert!(!result.exists);
        assert_eq!(result.enabled_count, 0);
        assert!(!result.all_enabled);
        assert!(result.targets.is_empty());
        assert_eq!(result.profile, json!({}));
        assert!(!profile.exists());
    }

    #[test]
    fn read_profile_preserves_profile_json_and_counts_enabled_targets() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile_1.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "future": {"kept": true},
                "targets": [
                    {"id": "a", "enabled": true},
                    {"id": "b", "enabled": false},
                    {"id": "c"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let result = read_profile_at(&profile).unwrap();

        assert!(result.exists);
        assert_eq!(result.enabled_count, 2);
        assert!(!result.all_enabled);
        assert_eq!(result.targets.len(), 3);
        assert_eq!(result.profile["future"]["kept"], json!(true));
    }

    #[test]
    fn set_profile_target_enabled_writes_profile_and_preserves_unknown_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let profile = profile_path(&data, 1);
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "future": true,
                "targets": [
                    {"id": "a", "name": "a", "enabled": true},
                    {"id": "b", "name": "b", "futureTarget": 1}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let result = set_profile_target_enabled_at(&profile, 1, false).unwrap();

        assert!(result.changed);
        assert_eq!(result.enabled_count, 1);
        assert!(!result.all_enabled);
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["future"], json!(true));
        assert_eq!(stored["targets"][1]["futureTarget"], json!(1));
        assert_eq!(stored["targets"][1]["enabled"], json!(false));
        let unchanged = set_profile_target_enabled_at(&profile, 99, true).unwrap();
        assert!(!unchanged.changed);
        assert_eq!(unchanged.enabled_count, 1);
    }

    #[test]
    fn toggle_all_profile_targets_switches_between_select_all_and_invert() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let profile = profile_path(&data, 1);
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [
                    {"id": "a", "enabled": true},
                    {"id": "b", "enabled": false},
                    {"id": "c"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let selected = toggle_all_profile_targets_at(&profile).unwrap();
        assert!(selected.changed);
        assert_eq!(selected.enabled_count, 3);
        assert!(selected.all_enabled);
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(
            stored["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|target| target["enabled"].as_bool().unwrap())
                .collect::<Vec<_>>(),
            vec![true, true, true]
        );

        let inverted = toggle_all_profile_targets_at(&profile).unwrap();
        assert!(inverted.changed);
        assert_eq!(inverted.enabled_count, 0);
        assert!(!inverted.all_enabled);
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(
            stored["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|target| target["enabled"].as_bool().unwrap())
                .collect::<Vec<_>>(),
            vec![false, false, false]
        );
    }

    #[test]
    fn ensure_target_metadata_fills_id_and_normalizes_hit_count() {
        let target = object(json!({
            "name": "1-2-old-a",
            "path": "ignored.png",
            "hit_count": "-3"
        }));
        let (target, changed) = ensure_target_metadata(target);
        assert!(changed);
        assert_eq!(target.get("id"), Some(&json!("old-a")));
        assert_eq!(target.get("hit_count"), Some(&json!(0)));

        let target = object(json!({
            "id": "stable",
            "name": "target",
            "hit_count": 4
        }));
        let (target, changed) = ensure_target_metadata(target);
        assert!(!changed);
        assert_eq!(target.get("id"), Some(&json!("stable")));
    }

    #[test]
    fn delete_target_files_only_removes_files_under_templates() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let template = data.join("templates").join("one.png");
        fs::create_dir_all(template.parent().unwrap()).unwrap();
        fs::write(&template, b"x").unwrap();
        let outside = tmp.path().join("outside.png");
        fs::write(&outside, b"x").unwrap();

        assert_eq!(
            delete_target_files(&object(json!({"path": outside})), &data).unwrap(),
            0
        );
        assert!(outside.exists());
        assert_eq!(
            delete_target_files(&object(json!({"path": template})), &data).unwrap(),
            1
        );
        assert!(!template.exists());
    }

    #[test]
    fn normalize_target_names_fills_deleted_number_gap_and_preserves_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let templates = data.join("templates");
        fs::create_dir_all(&templates).unwrap();
        let mut targets = Vec::new();
        for stem in ["1-1-old-a", "1-3-old-b"] {
            let path = templates.join(format!("{stem}.png"));
            fs::write(&path, b"x").unwrap();
            targets.push(object(json!({
                "name": stem,
                "path": path,
                "thumb": "legacy-thumb.png"
            })));
        }

        let (targets, changed) = normalize_target_names(targets, &data, 1).unwrap();
        assert!(changed);
        assert_eq!(
            targets
                .iter()
                .map(|target| target.get("id").unwrap().as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["old-a", "old-b"]
        );
        assert_eq!(
            targets
                .iter()
                .map(|target| target.get("name").unwrap().as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["1-1-old-a", "1-2-old-b"]
        );
        assert!(templates.join("1-2-old-b.png").exists());
        assert!(!templates.join("1-3-old-b.png").exists());
        assert!(targets.iter().all(|target| !target.contains_key("thumb")));
    }

    #[test]
    fn normalize_target_names_leaves_external_or_missing_paths_untouched_except_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        fs::create_dir_all(data.join("templates")).unwrap();
        let external = tmp.path().join("external.png");
        fs::write(&external, b"x").unwrap();
        let missing = data.join("templates").join("missing.png");
        let targets = vec![
            object(json!({"name":"external","path":external,"thumb":"old"})),
            object(json!({"name":"missing","path":missing,"thumb":"old"})),
        ];

        let (targets, changed) = normalize_target_names(targets, &data, 2).unwrap();
        assert!(changed);
        assert!(external.exists());
        assert_eq!(targets[0].get("name"), Some(&json!("external")));
        assert_eq!(targets[1].get("name"), Some(&json!("missing")));
        assert!(targets.iter().all(|target| !target.contains_key("thumb")));
        assert!(targets.iter().all(|target| target.contains_key("id")));
    }

    #[test]
    fn normalize_profile_file_drops_missing_refs_and_rewrites_existing_targets() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let templates = data.join("templates");
        let profiles = data.join("profiles");
        fs::create_dir_all(&templates).unwrap();
        fs::create_dir_all(&profiles).unwrap();
        let one = templates.join("1-2-old-a.png");
        fs::write(&one, b"x").unwrap();
        let missing = templates.join("missing.png");
        let profile = profiles.join("profile_1.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "threshold": 0.9,
                "targets": [
                    {"name":"old","path":one,"thumb":"legacy"},
                    {"name":"missing","path":missing}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(normalize_profile_file_at(&profile, &data, 1).unwrap());
        let data: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        let targets = data.get("targets").unwrap().as_array().unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].get("name"), Some(&json!("1-1-old-a")));
        assert_eq!(targets[0].get("id"), Some(&json!("old-a")));
        assert_eq!(targets[0].get("hit_count"), Some(&json!(0)));
        assert!(targets[0].get("thumb").is_none());
        assert!(templates.join("1-1-old-a.png").exists());
        assert!(!templates.join("1-2-old-a.png").exists());
        assert_eq!(data.get("threshold"), Some(&json!(0.9)));
    }

    #[test]
    fn normalize_profile_file_writes_metadata_even_when_names_are_already_current() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let templates = data.join("templates");
        let profiles = data.join("profiles");
        fs::create_dir_all(&templates).unwrap();
        fs::create_dir_all(&profiles).unwrap();
        let one = templates.join("1-1-old-a.png");
        fs::write(&one, b"x").unwrap();
        let profile = profiles.join("profile_1.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [
                    {"name":"1-1-old-a","path":one,"thumb":"legacy","hit_count":"2"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(normalize_profile_file_at(&profile, &data, 1).unwrap());
        let data: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        let target = &data.get("targets").unwrap().as_array().unwrap()[0];
        assert_eq!(target.get("id"), Some(&json!("old-a")));
        assert_eq!(target.get("hit_count"), Some(&json!(2)));
        assert!(target.get("thumb").is_none());
    }

    #[test]
    fn record_target_hits_updates_matching_template_counts_by_identity() {
        let mut targets = vec![
            object(json!({"id":"a","name":"1-1-a","hit_count":2})),
            object(json!({"id":"b","name":"1-2-b"})),
            object(json!({"name":"1-3-c","hit_count":"bad"})),
        ];
        assert!(record_target_hits(
            &mut targets,
            &[
                "a".to_string(),
                "b".to_string(),
                "a".to_string(),
                "c".to_string(),
                "missing".to_string()
            ]
        ));
        assert_eq!(targets[0].get("hit_count"), Some(&json!(4)));
        assert_eq!(targets[1].get("hit_count"), Some(&json!(1)));
        assert_eq!(targets[2].get("hit_count"), Some(&json!(1)));
    }

    #[test]
    fn record_profile_hits_preserves_unknown_fields_and_writes_only_on_match() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "match": {"threshold": 0.9},
                "targets": [
                    {"id":"a","name":"1-1-a","hit_count":2,"future":true},
                    {"id":"b","name":"1-2-b","hit_count":0}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(record_profile_hits_at(&profile, &["a".to_string(), "a".to_string()]).unwrap());
        let data: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        let targets = data.get("targets").unwrap().as_array().unwrap();
        assert_eq!(targets[0].get("hit_count"), Some(&json!(4)));
        assert_eq!(targets[0].get("future"), Some(&json!(true)));
        assert_eq!(
            data.get("match").unwrap().get("threshold"),
            Some(&json!(0.9))
        );
        assert!(!record_profile_hits_at(&profile, &["missing".to_string()]).unwrap());
    }

    #[test]
    fn clear_target_hit_count_resets_matching_identity_only() {
        let mut targets = vec![
            object(json!({"id":"a","hit_count":7})),
            object(json!({"id":"b","hit_count":2})),
        ];
        assert!(clear_target_hit_count(&mut targets, "b"));
        assert_eq!(targets[0].get("hit_count"), Some(&json!(7)));
        assert_eq!(targets[1].get("hit_count"), Some(&json!(0)));
        assert!(!clear_target_hit_count(&mut targets, "b"));
    }

    #[test]
    fn clear_profile_target_hit_count_updates_profile_file() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [{"id":"a","hit_count":3}]
            }))
            .unwrap(),
        )
        .unwrap();
        let result = clear_profile_target_hit_count_at(&profile, "a").unwrap();

        assert!(result.changed);
        assert_eq!(result.deleted_files, 0);
        assert_eq!(result.selected_index, Some(0));
        assert_eq!(result.targets[0]["hit_count"], json!(0));
        let data: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(data["targets"][0].get("hit_count"), Some(&json!(0)));
    }

    #[test]
    fn clear_profile_target_hit_count_selects_zero_count_target_without_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        let profile_text = serde_json::to_string_pretty(&json!({
            "targets": [
                {"id":"a","hit_count":5},
                {"id":"b","hit_count":0}
            ]
        }))
        .unwrap();
        fs::write(&profile, &profile_text).unwrap();

        let result = clear_profile_target_hit_count_at(&profile, "b").unwrap();

        assert!(!result.changed);
        assert_eq!(result.selected_index, Some(1));
        assert_eq!(result.targets[1]["hit_count"], json!(0));
        assert_eq!(fs::read_to_string(&profile).unwrap(), profile_text);
        let missing = clear_profile_target_hit_count_at(&profile, "missing").unwrap();
        assert!(!missing.changed);
        assert_eq!(missing.selected_index, None);
        assert_eq!(missing.targets.len(), 2);
    }

    #[test]
    fn add_profile_template_pngs_prunes_to_limit_before_naming() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let profile = profile_path(&data, 1);
        let images = tmp.path().join("images");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let one = images.join("one.png");
        let two = images.join("two.png");
        let three = images.join("three.png");
        write_test_png(&one, [255, 0, 0], 12, 10);
        write_test_png(&two, [0, 0, 255], 12, 10);
        write_test_png(&three, [0, 255, 0], 12, 10);
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "future": {"kept": true},
                "targets": []
            }))
            .unwrap(),
        )
        .unwrap();

        add_profile_template_pngs_at(&profile, &data, 1, &[one], 2).unwrap();
        add_profile_template_pngs_at(&profile, &data, 1, &[two], 2).unwrap();
        let result = add_profile_template_pngs_at(&profile, &data, 1, &[three], 2).unwrap();

        assert!(result.changed);
        assert_eq!(result.added_count, 1);
        assert_eq!(result.pruned_count, 1);
        assert_eq!(result.selected_index, Some(1));
        assert_eq!(result.targets.len(), 2);
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["future"]["kept"], json!(true));
        let targets = stored["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 2);
        assert!(targets[0]["name"].as_str().unwrap().starts_with("1-1-"));
        assert!(targets[1]["name"].as_str().unwrap().starts_with("1-2-"));
        assert_eq!(targets[0]["size"], json!("12x10"));
        assert_eq!(targets[1]["enabled"], json!(true));
        assert_eq!(targets[1]["hit_count"], json!(0));
        assert!(targets.iter().all(|target| target.get("thumb").is_none()));
        assert_eq!(
            fs::read_dir(templates_dir(&data)).unwrap().count(),
            2,
            "only the retained target and the newly added target should remain"
        );
    }

    #[test]
    fn add_profile_template_pngs_converts_common_images_to_png_templates() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let profile = profile_path(&data, 1);
        let images = tmp.path().join("images");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let jpg = images.join("camera.jpg");
        let bmp = images.join("dialog.bmp");
        write_test_image(&jpg, [200, 30, 20], 9, 7, image::ImageFormat::Jpeg);
        write_test_image(&bmp, [10, 120, 240], 5, 4, image::ImageFormat::Bmp);

        let result = add_profile_template_pngs_at(&profile, &data, 1, &[jpg, bmp], 5).unwrap();

        assert!(result.changed);
        assert_eq!(result.added_count, 2);
        assert_eq!(result.selected_index, Some(1));
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        let targets = stored["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 2);
        assert!(targets
            .iter()
            .all(|target| target["path"].as_str().unwrap().ends_with(".png")));
        assert_eq!(targets[0]["size"], json!("9x7"));
        assert_eq!(targets[1]["size"], json!("5x4"));
        assert_eq!(
            RgbFrame::from_png_path(targets[0]["path"].as_str().unwrap())
                .unwrap()
                .width,
            9
        );
        assert_eq!(
            RgbFrame::from_png_path(targets[1]["path"].as_str().unwrap())
                .unwrap()
                .height,
            4
        );
    }

    #[test]
    fn add_profile_template_frames_writes_clipboard_frame_as_png_template() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let profile = profile_path(&data, 1);
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let pixels = vec![
            255, 0, 0, 0, 255, 0, 0, 0, 255, //
            8, 9, 10, 11, 12, 13, 14, 15, 16,
        ];
        let frame = RgbFrame::new(3, 2, pixels.clone()).unwrap();

        let result = add_profile_template_frames_at(&profile, &data, 1, &[frame], 5).unwrap();

        assert!(result.changed);
        assert_eq!(result.added_count, 1);
        assert_eq!(result.pruned_count, 0);
        assert_eq!(result.selected_index, Some(0));
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        let target = &stored["targets"].as_array().unwrap()[0];
        assert_eq!(target["size"], json!("3x2"));
        assert!(target["path"].as_str().unwrap().ends_with(".png"));
        let saved = RgbFrame::from_png_path(target["path"].as_str().unwrap()).unwrap();
        assert_eq!(saved.width, 3);
        assert_eq!(saved.height, 2);
        assert_eq!(saved.pixels, pixels);
    }

    #[test]
    fn add_profile_template_pngs_rejects_zero_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source.png");
        write_test_png(&source, [1, 2, 3], 2, 2);

        let err = add_profile_template_pngs_at(
            tmp.path().join("profile.json"),
            tmp.path(),
            1,
            &[source],
            0,
        )
        .unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("max_templates"));
    }

    #[test]
    fn reorder_profile_target_renames_files_by_new_position() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let templates = templates_dir(&data);
        let profile = profile_path(&data, 1);
        fs::create_dir_all(&templates).unwrap();
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let mut targets = Vec::new();
        for stem in ["1-1-a", "1-2-b", "1-3-c"] {
            let path = templates.join(format!("{stem}.png"));
            fs::write(&path, b"x").unwrap();
            targets.push(json!({
                "id": stem.rsplit('-').next().unwrap(),
                "name": stem,
                "path": path,
                "thumb": format!("{stem}.legacy.png")
            }));
        }
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "future": true,
                "targets": targets
            }))
            .unwrap(),
        )
        .unwrap();

        let result = reorder_profile_target_at(&profile, &data, 1, 0, 3).unwrap();

        assert!(result.changed);
        assert_eq!(result.selected_index, Some(2));
        assert_eq!(result.deleted_files, 0);
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["future"], json!(true));
        let targets = stored["targets"].as_array().unwrap();
        assert_eq!(
            targets
                .iter()
                .map(|target| target["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["b", "c", "a"]
        );
        assert_eq!(
            targets
                .iter()
                .map(|target| {
                    PathBuf::from(target["path"].as_str().unwrap())
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                })
                .collect::<Vec<_>>(),
            vec!["1-1-b", "1-2-c", "1-3-a"]
        );
        assert!(targets.iter().all(|target| target.get("thumb").is_none()));
        assert!(templates.join("1-1-b.png").exists());
        assert!(templates.join("1-2-c.png").exists());
        assert!(templates.join("1-3-a.png").exists());
    }

    #[test]
    fn remove_profile_target_deletes_template_file_and_preserves_external_file() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let templates = templates_dir(&data);
        let profile = profile_path(&data, 1);
        fs::create_dir_all(&templates).unwrap();
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let one = templates.join("1-1-a.png");
        let external = tmp.path().join("external.png");
        fs::write(&one, b"x").unwrap();
        fs::write(&external, b"x").unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "future": true,
                "targets": [
                    {"id": "a", "name": "1-1-a", "path": one},
                    {"id": "external", "name": "external", "path": external}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let result = remove_profile_target_at(&profile, &data, 1, 0).unwrap();

        assert!(result.changed);
        assert_eq!(result.deleted_files, 1);
        assert_eq!(result.selected_index, None);
        assert!(!one.exists());
        assert!(external.exists());
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["future"], json!(true));
        let targets = stored["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0]["id"], json!("external"));
    }

    #[test]
    fn clear_profile_targets_deletes_only_template_files() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let templates = templates_dir(&data);
        let profile = profile_path(&data, 1);
        fs::create_dir_all(&templates).unwrap();
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let one = templates.join("1-1-a.png");
        let external = tmp.path().join("external.png");
        fs::write(&one, b"x").unwrap();
        fs::write(&external, b"x").unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [
                    {"id": "a", "name": "1-1-a", "path": one},
                    {"id": "external", "name": "external", "path": external}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        let result = clear_profile_targets_at(&profile, &data).unwrap();

        assert!(result.changed);
        assert_eq!(result.deleted_files, 1);
        assert!(result.targets.is_empty());
        assert!(!one.exists());
        assert!(external.exists());
        let stored: Value = serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert!(stored["targets"].as_array().unwrap().is_empty());
    }

    #[test]
    #[ignore = "production performance gate; run through scripts\\production-template-performance-smoke.ps1"]
    fn benchmark_production_profile_template_scan() {
        let profile_path = PathBuf::from(
            std::env::var("SCREENWATCH_PRODUCTION_PROFILE")
                .expect("SCREENWATCH_PRODUCTION_PROFILE must point to a profile_*.json file"),
        );
        let data_dir = std::env::var("SCREENWATCH_PRODUCTION_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_data_dir_from_profile(&profile_path));
        let frame_spec =
            std::env::var("SCREENWATCH_PRODUCTION_FRAME").unwrap_or_else(|_| "2560x1440".into());
        let (frame_width, frame_height) = parse_frame_spec(&frame_spec);
        let scale_spec =
            std::env::var("SCREENWATCH_PRODUCTION_SCALES").unwrap_or_else(|_| "1.0".into());
        let threshold = std::env::var("SCREENWATCH_PRODUCTION_THRESHOLD")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or_else(super::default_profile_threshold);
        let template_workers = std::env::var("SCREENWATCH_PRODUCTION_TEMPLATE_WORKERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(PROFILE_TEMPLATE_WORKERS);

        let raw_targets = read_profile_targets(&profile_path).unwrap();
        let enabled_count = super::enabled_target_count(&raw_targets);
        let detector_targets = profile_template_targets_for_detection(
            &raw_targets,
            threshold,
            ScaleSpec::Text(scale_spec.clone()),
        );
        assert!(
            !detector_targets.is_empty(),
            "profile must contain at least one enabled template target"
        );

        let config = WatchConfig {
            poll_interval_seconds: super::default_profile_poll_interval_seconds(),
            cooldown_seconds: super::default_profile_cooldown_seconds(),
            template_workers,
            regions: Vec::new(),
            windows: Vec::new(),
            window_apps: Vec::new(),
            targets: detector_targets,
            alarm: Default::default(),
            extra: Map::new(),
        };
        let expected_ids = template_target_ids(&config.targets);
        let mut frame = synthetic_background(frame_width, frame_height);
        let placements = place_production_templates(&mut frame, &config.targets, &data_dir);

        let detector = PreparedDetector::from_config(&config, &data_dir).unwrap();
        let started = Instant::now();
        let matches = detector.run(&frame);
        let elapsed_ms = started.elapsed().as_millis();
        let mut actual_ids = matches
            .iter()
            .map(|item| item.target_id.clone())
            .collect::<Vec<_>>();
        actual_ids.sort();

        assert_eq!(matches.len(), expected_ids.len(), "{matches:#?}");
        assert_eq!(actual_ids, expected_ids);

        println!(
            "productionTemplateBenchmarkMs={} frame={}x{} profile={} dataDir={} rawTargets={} enabledTargets={} templateTargets={} templateWorkers={} threshold={:.2} scales={} matches={} placements={}",
            elapsed_ms,
            frame_width,
            frame_height,
            profile_path.display(),
            data_dir.display(),
            raw_targets.len(),
            enabled_count,
            config.targets.len(),
            detector.template_worker_limit(),
            threshold,
            scale_spec,
            matches.len(),
            placements
        );
    }

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    fn default_data_dir_from_profile(profile_path: &Path) -> PathBuf {
        let profiles_dir = profile_path
            .parent()
            .expect("profile path must have a parent directory");
        profiles_dir
            .parent()
            .expect("profile path must be inside a data directory")
            .to_path_buf()
    }

    fn parse_frame_spec(value: &str) -> (u32, u32) {
        let (width, height) = value
            .split_once('x')
            .or_else(|| value.split_once('X'))
            .expect("SCREENWATCH_PRODUCTION_FRAME must look like 2560x1440");
        let width = width
            .trim()
            .parse::<u32>()
            .expect("frame width must be a positive integer");
        let height = height
            .trim()
            .parse::<u32>()
            .expect("frame height must be a positive integer");
        assert!(width > 0 && height > 0, "frame dimensions must be positive");
        (width, height)
    }

    fn template_target_ids(targets: &[TargetConfig]) -> Vec<String> {
        let mut out = targets
            .iter()
            .filter_map(|target| match target {
                TargetConfig::Template { name, id, .. } => {
                    Some(id.as_deref().unwrap_or(name).to_string())
                }
                TargetConfig::Pixel { .. }
                | TargetConfig::OcrText { .. }
                | TargetConfig::Unknown => None,
            })
            .collect::<Vec<_>>();
        out.sort();
        out
    }

    fn synthetic_background(width: u32, height: u32) -> RgbFrame {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
        for y in 0..height {
            for x in 0..width {
                pixels.push(3 + ((x * 17 + y * 11) % 29) as u8);
                pixels.push(5 + ((x * 13 + y * 19) % 31) as u8);
                pixels.push(7 + ((x * 23 + y * 7) % 37) as u8);
            }
        }
        RgbFrame::new(width, height, pixels).unwrap()
    }

    fn place_production_templates(
        frame: &mut RgbFrame,
        targets: &[TargetConfig],
        data_dir: &Path,
    ) -> String {
        let mut cursor_x = 32;
        let mut cursor_y = 32;
        let mut row_height = 0;
        let margin = 19;
        let mut placements = Vec::new();

        for target in targets {
            let TargetConfig::Template { name, path, id, .. } = target else {
                continue;
            };
            let template_path = resolve_template_path(data_dir, path);
            let template = RgbFrame::from_image_path(&template_path).unwrap_or_else(|err| {
                panic!(
                    "failed to load production template {}: {err}",
                    template_path.display()
                )
            });
            assert!(
                template.width + margin < frame.width && template.height + margin < frame.height,
                "template {} is too large for {}x{} benchmark frame",
                template_path.display(),
                frame.width,
                frame.height
            );
            if cursor_x + template.width + margin > frame.width {
                cursor_x = 32;
                cursor_y += row_height + margin;
                row_height = 0;
            }
            assert!(
                cursor_y + template.height + margin <= frame.height,
                "not enough benchmark frame space for all production templates"
            );

            paste_rgb(frame, &template, cursor_x, cursor_y);
            let target_id = id.as_deref().unwrap_or(name);
            placements.push(format!(
                "{}@{},{},{}x{}",
                target_id, cursor_x, cursor_y, template.width, template.height
            ));
            cursor_x += template.width + margin;
            row_height = row_height.max(template.height);
        }

        placements.join(";")
    }

    fn resolve_template_path(data_dir: &Path, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            data_dir.join(path)
        }
    }

    fn paste_rgb(frame: &mut RgbFrame, template: &RgbFrame, left: u32, top: u32) {
        for y in 0..template.height {
            for x in 0..template.width {
                let src = ((y * template.width + x) * 3) as usize;
                let dst = (((top + y) * frame.width + (left + x)) * 3) as usize;
                frame.pixels[dst..dst + 3].copy_from_slice(&template.pixels[src..src + 3]);
            }
        }
    }

    fn write_test_png(path: &std::path::Path, rgb: [u8; 3], width: u32, height: u32) {
        let frame = RgbFrame::new(
            width,
            height,
            (0..width * height).flat_map(|_| rgb).collect(),
        )
        .unwrap();
        write_rgb_png(path, &frame).unwrap();
    }

    fn write_test_image(
        path: &std::path::Path,
        rgb: [u8; 3],
        width: u32,
        height: u32,
        format: image::ImageFormat,
    ) {
        let image = image::RgbImage::from_fn(width, height, |_x, _y| image::Rgb(rgb));
        image.save_with_format(path, format).unwrap();
    }
}
