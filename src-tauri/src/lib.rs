mod audio;
mod clipboard_import;
mod dwm_preview;
mod monitor_session;
mod preview_cache;
mod screen_capture;
mod single_instance;
mod startup;
mod tray;
mod window_capture;
mod window_layout;
mod window_sources;

use audio::AlarmBeepState;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use dwm_preview::{CssPreviewRect, DwmPreviewState, DwmPreviewSyncResult};
use monitor_session::{
    alerted_target_ids, MonitorSessionEvent, MonitorSessionEventSink, MonitorSessionHitSink,
    MonitorSessionSnapshot, MonitorSessionState, MONITOR_SESSION_EVENT,
};
use preview_cache::{
    screen_preview_key, screen_preview_signature, window_preview_key, window_preview_signature,
    PreviewCacheState,
};
use screen_capture::{capture_screen_region, frame_to_png_bytes, CaptureRegion};
use screen_watch_core::{
    build::BuildFlavor,
    config::WatchConfig,
    data_dir::{legacy_data_dir_from_app_root, migrate_legacy_data_at, user_data_dir},
    evidence::safe_name,
    ocr::{
        create_ocr_backend, ocr_unavailable_reason_for_config, probe_ocr_backend, OcrAvailability,
        OcrProbeResult, OcrSettings,
    },
    profile::{
        add_profile_template_frames_at, add_profile_template_pngs_at,
        clear_profile_target_hit_count_at, clear_profile_targets_at, normalize_profile_file_at,
        profile_path, profile_watch_config_at, read_profile_at, read_profile_state_at,
        record_profile_hits_at, remove_profile_target_at, reorder_profile_target_at,
        save_last_profile_at, save_max_alerts_at, save_profile_sources_at, screenshots_dir,
        set_profile_target_enabled_at, toggle_all_profile_targets_at, AddTemplateImagesResult,
        ProfileReadResult, ProfileSourcesSaveResult, ProfileStateResult, ProfileTargetsEditResult,
        ProfileTargetsEnabledResult, ProfileWatchConfigOptions, PROFILE_COUNT,
    },
    scan::{ScanEngine, ScanFrameResult},
    sources::{resolve_sources, MonitorInfo, ResolvedRegion, ResolvedSources},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use startup::{unsupported_status, StartupManager, StartupStatus};
use std::{
    path::Path,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
use window_sources::{AppWindow, WindowSourceResolution};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    build_flavor: String,
    data_dir: String,
    ocr: OcrAvailability,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipboardImagePayload {
    name: Option<String>,
    mime_type: Option<String>,
    data_base64: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ListedMonitor {
    index: i32,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    name: Option<String>,
    scale_factor: f64,
    is_virtual: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapturePreview {
    width: u32,
    height: u32,
    data_url: String,
    source_signature: String,
    cached: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OneShotScanResult {
    regions: Vec<ScanFrameResult>,
    windows: Vec<ScanFrameResult>,
    hit_count: usize,
    skipped_windows: usize,
    skipped_window_apps: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenTargetFileResult {
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileTargetThumbnail {
    path: String,
    data_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileCaptureSources {
    regions: Vec<ResolvedRegion>,
    windows: Vec<screen_watch_core::config::WindowConfig>,
    skipped_windows: usize,
    skipped_window_apps: usize,
}

#[tauri::command]
fn app_info() -> AppInfo {
    let data_dir = user_data_dir();
    let build_flavor = BuildFlavor::from_env();
    let ocr = OcrSettings::from_env_and_data_dir(build_flavor, data_dir.clone()).availability();
    AppInfo {
        build_flavor: build_flavor.as_str().to_string(),
        data_dir: data_dir.display().to_string(),
        ocr,
    }
}

#[tauri::command]
fn ocr_backend_probe() -> OcrProbeResult {
    probe_ocr_backend(&OcrSettings::from_env())
}

#[tauri::command]
fn list_monitors(window: tauri::Window) -> Result<Vec<ListedMonitor>, String> {
    monitors_for_window(&window)
}

#[tauri::command]
fn list_app_windows() -> Result<Vec<AppWindow>, String> {
    window_sources::list_app_windows()
}

#[tauri::command]
fn resolve_config_text_sources(
    window: tauri::Window,
    text: String,
) -> Result<ResolvedSources, String> {
    let config = WatchConfig::from_json_str(&text).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    let monitors = monitors_for_window(&window)?;
    let core_monitors = core_monitors_from_listed(&monitors);
    resolve_sources(&config, &core_monitors).map_err(|err| err.to_string())
}

#[tauri::command]
fn resolve_config_text_window_sources(text: String) -> Result<WindowSourceResolution, String> {
    let config = WatchConfig::from_json_str(&text).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    let available = window_sources::list_app_windows()?;
    Ok(window_sources::resolve_window_apps(
        &config.window_apps,
        &available,
    ))
}

#[tauri::command]
fn scan_config_text_once(
    window: tauri::Window,
    beep_state: tauri::State<'_, AlarmBeepState>,
    text: String,
    base_dir: Option<String>,
) -> Result<OneShotScanResult, String> {
    let config = WatchConfig::from_json_str(&text).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    let base_dir = base_dir.map(PathBuf::from).unwrap_or_else(user_data_dir);
    scan_watch_config_once(&window, beep_state.inner(), config, base_dir)
}

fn scan_watch_config_once(
    window: &tauri::Window,
    beeper: &AlarmBeepState,
    config: WatchConfig,
    base_dir: PathBuf,
) -> Result<OneShotScanResult, String> {
    let alarm = config.alarm.clone();
    let monitors = monitors_for_window(window)?;
    let sources = resolve_sources(&config, &core_monitors_from_listed(&monitors))
        .map_err(|err| err.to_string())?;
    let (windows, skipped_windows, skipped_window_apps) = concrete_windows_from_sources(&sources)?;
    if sources.regions.is_empty() && windows.is_empty() {
        if skipped_windows > 0 || skipped_window_apps > 0 {
            return Ok(OneShotScanResult {
                regions: Vec::new(),
                windows: Vec::new(),
                hit_count: 0,
                skipped_windows,
                skipped_window_apps,
            });
        }
        return Err("scan has no resolved screen or window sources".into());
    }

    let clock = ScanClock::now();
    let (region_results, window_results) =
        scan_resolved_sources_once(config, &base_dir, &sources.regions, &windows, &clock)?;
    let hit_count = region_results
        .iter()
        .chain(window_results.iter())
        .map(|result| result.alerted_matches.len())
        .sum();
    if hit_count > 0 {
        beeper.start_for_alarm(&alarm);
    }
    Ok(OneShotScanResult {
        regions: region_results,
        windows: window_results,
        hit_count,
        skipped_windows,
        skipped_window_apps,
    })
}

fn scan_engine_for(
    config: WatchConfig,
    template_base_dir: &Path,
    data_dir: &Path,
) -> Result<ScanEngine, String> {
    let settings = OcrSettings::from_env();
    if let Some(reason) = ocr_unavailable_reason_for_config(&config, &settings) {
        return Err(reason);
    }
    ScanEngine::new_with_ocr_backend(
        config,
        template_base_dir,
        data_dir,
        create_ocr_backend(&settings),
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn start_monitoring_session(
    window: tauri::Window,
    state: tauri::State<'_, MonitorSessionState>,
    beep_state: tauri::State<'_, AlarmBeepState>,
    text: String,
    base_dir: Option<String>,
) -> Result<MonitorSessionSnapshot, String> {
    let config = WatchConfig::from_json_str(&text).map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())?;
    let base_dir = base_dir.map(PathBuf::from).unwrap_or_else(user_data_dir);
    start_monitoring_with_config(window, state, beep_state, config, base_dir)
}

fn start_monitoring_with_config(
    window: tauri::Window,
    state: tauri::State<'_, MonitorSessionState>,
    beep_state: tauri::State<'_, AlarmBeepState>,
    config: WatchConfig,
    base_dir: PathBuf,
) -> Result<MonitorSessionSnapshot, String> {
    start_monitoring_with_config_and_hit_sink(window, state, beep_state, config, base_dir, None)
}

fn start_monitoring_with_config_and_hit_sink(
    window: tauri::Window,
    state: tauri::State<'_, MonitorSessionState>,
    beep_state: tauri::State<'_, AlarmBeepState>,
    config: WatchConfig,
    base_dir: PathBuf,
    hit_sink: Option<Arc<dyn MonitorSessionHitSink>>,
) -> Result<MonitorSessionSnapshot, String> {
    let monitors = monitors_for_window(&window)?;
    let sources = resolve_sources(&config, &core_monitors_from_listed(&monitors))
        .map_err(|err| err.to_string())?;
    state.start_sources_with_events(
        config,
        base_dir,
        sources.regions,
        sources.windows,
        sources.window_apps,
        beep_state.inner().clone(),
        hit_sink,
        Arc::new(TauriMonitorEventSink {
            app: window.app_handle().clone(),
            window,
        }),
    )
}

#[tauri::command]
fn stop_monitoring_session(
    window: tauri::Window,
    state: tauri::State<'_, MonitorSessionState>,
) -> Result<MonitorSessionSnapshot, String> {
    let snapshot = state.stop()?;
    tray::update_monitoring_status(window.app_handle(), snapshot.running);
    Ok(snapshot)
}

#[tauri::command]
fn monitoring_session_status(
    state: tauri::State<'_, MonitorSessionState>,
) -> Result<MonitorSessionSnapshot, String> {
    state.snapshot()
}

#[tauri::command]
fn startup_status() -> Result<StartupStatus, String> {
    match StartupManager::current()? {
        Some(manager) => manager.status(),
        None => Ok(unsupported_status()),
    }
}

#[tauri::command]
fn set_startup_enabled(enabled: bool) -> Result<StartupStatus, String> {
    match StartupManager::current()? {
        Some(manager) => manager.set_enabled(enabled),
        None if enabled => Err("startup shortcut is only supported on Windows".to_string()),
        None => Ok(unsupported_status()),
    }
}

#[cfg(test)]
fn scan_resolved_screen_regions_once(
    config: WatchConfig,
    base_dir: &Path,
    regions: &[ResolvedRegion],
    clock: &ScanClock,
) -> Result<Vec<ScanFrameResult>, String> {
    let mut engine = scan_engine_for(config, base_dir, base_dir)?;
    let mut results = Vec::with_capacity(regions.len());
    for region in regions {
        let frame = capture_screen_region(CaptureRegion {
            left: region.bbox.left,
            top: region.bbox.top,
            width: region.bbox.width,
            height: region.bbox.height,
        })
        .map_err(|err| err.to_string())?;
        let result = engine
            .scan_region_frame(
                &region.name,
                &frame,
                clock.now_seconds,
                &clock.time_text,
                &format!("{}-{}", clock.stamp, region.monitor),
            )
            .map_err(|err| err.to_string())?;
        results.push(result);
    }
    Ok(results)
}

fn scan_resolved_sources_once(
    config: WatchConfig,
    base_dir: &Path,
    regions: &[ResolvedRegion],
    windows: &[screen_watch_core::config::WindowConfig],
    clock: &ScanClock,
) -> Result<(Vec<ScanFrameResult>, Vec<ScanFrameResult>), String> {
    let mut engine = scan_engine_for(config, base_dir, base_dir)?;
    let mut region_results = Vec::with_capacity(regions.len());
    for region in regions {
        let frame = capture_screen_region(CaptureRegion {
            left: region.bbox.left,
            top: region.bbox.top,
            width: region.bbox.width,
            height: region.bbox.height,
        })
        .map_err(|err| err.to_string())?;
        let result = engine
            .scan_region_frame(
                &region.name,
                &frame,
                clock.now_seconds,
                &clock.time_text,
                &format!("{}-{}", clock.stamp, region.monitor),
            )
            .map_err(|err| err.to_string())?;
        region_results.push(result);
    }

    let mut window_results = Vec::with_capacity(windows.len());
    let mut mode_cache = window_capture::WindowCaptureModeCache::default();
    for window in windows {
        let Some(hwnd) = window.hwnd else {
            continue;
        };
        let Some(frame) = window_capture::capture_window_frame(hwnd, Some(&mut mode_cache))
            .map_err(|err| err.to_string())?
        else {
            continue;
        };
        let name = monitor_session::window_source_name(window);
        let result = engine
            .scan_region_frame(
                &name,
                &frame,
                clock.now_seconds,
                &clock.time_text,
                &format!("{}-window-{}", clock.stamp, safe_name(&name)),
            )
            .map_err(|err| err.to_string())?;
        window_results.push(result);
    }

    Ok((region_results, window_results))
}

fn concrete_windows_from_sources(
    sources: &ResolvedSources,
) -> Result<(Vec<screen_watch_core::config::WindowConfig>, usize, usize), String> {
    let skipped_windows = sources
        .windows
        .iter()
        .filter(|window| window.hwnd.is_none())
        .count();
    let mut windows = sources
        .windows
        .iter()
        .filter(|window| window.hwnd.is_some())
        .cloned()
        .collect::<Vec<_>>();
    let mut skipped_window_apps = 0;
    if !sources.window_apps.is_empty() {
        let available = window_sources::list_app_windows()?;
        let resolution = window_sources::resolve_window_apps(&sources.window_apps, &available);
        skipped_window_apps = resolution.missing_window_apps.len();
        windows.extend(resolution.windows);
    }
    Ok((windows, skipped_windows, skipped_window_apps))
}

#[tauri::command]
fn capture_screen_region_preview(
    left: i32,
    top: i32,
    width: u32,
    height: u32,
) -> Result<CapturePreview, String> {
    let signature = screen_preview_signature(left, top, width, height);
    let frame = capture_screen_region(CaptureRegion {
        left,
        top,
        width,
        height,
    })
    .map_err(|err| err.to_string())?;
    capture_preview_from_frame(frame, signature, false)
}

#[tauri::command]
fn capture_screen_region_preview_cached(
    preview_cache: tauri::State<'_, PreviewCacheState>,
    source_key: Option<String>,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
) -> Result<CapturePreview, String> {
    let key = screen_preview_key(source_key, left, top, width, height);
    let signature = screen_preview_signature(left, top, width, height);
    let capture_region = CaptureRegion {
        left,
        top,
        width,
        height,
    };
    let (frame, cached) = preview_cache.frame_for(key, signature.clone(), || {
        capture_screen_region(capture_region).map_err(|err| err.to_string())
    })?;
    capture_preview_from_frame(frame, signature, cached)
}

#[tauri::command]
fn capture_window_preview(hwnd: isize) -> Result<CapturePreview, String> {
    let rect = window_capture::window_rect(hwnd)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "window preview is unavailable".to_string())?;
    let signature = window_preview_signature(hwnd, rect.left, rect.top, rect.width, rect.height);
    let frame = window_capture::capture_window_preview(hwnd)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "window preview is unavailable".to_string())?;
    capture_preview_from_frame(frame, signature, false)
}

#[tauri::command]
fn capture_window_preview_cached(
    preview_cache: tauri::State<'_, PreviewCacheState>,
    source_key: Option<String>,
    hwnd: isize,
) -> Result<CapturePreview, String> {
    let rect = window_capture::window_rect(hwnd)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "window preview is unavailable".to_string())?;
    let key = window_preview_key(source_key, hwnd);
    let signature = window_preview_signature(hwnd, rect.left, rect.top, rect.width, rect.height);
    let (frame, cached) = preview_cache.frame_for(key, signature.clone(), || {
        window_capture::capture_window_preview(hwnd)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "window preview is unavailable".to_string())
    })?;
    capture_preview_from_frame(frame, signature, cached)
}

#[tauri::command]
fn retain_cached_preview_sources(
    preview_cache: tauri::State<'_, PreviewCacheState>,
    source_keys: Vec<String>,
) -> Result<(), String> {
    preview_cache.retain_keys(source_keys.iter().map(String::as_str))
}

#[tauri::command]
fn sync_dwm_preview(
    window: tauri::Window,
    dwm_preview: tauri::State<'_, DwmPreviewState>,
    source_key: String,
    hwnd: isize,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
) -> Result<DwmPreviewSyncResult, String> {
    dwm_preview::sync_window_preview(
        dwm_preview.inner(),
        &window,
        source_key,
        hwnd,
        CssPreviewRect {
            left,
            top,
            width,
            height,
        },
    )
}

#[tauri::command]
fn retain_dwm_preview_sources(
    dwm_preview: tauri::State<'_, DwmPreviewState>,
    source_keys: Vec<String>,
) -> Result<(), String> {
    dwm_preview.retain_keys(source_keys.iter().map(String::as_str));
    Ok(())
}

#[tauri::command]
fn clear_dwm_previews(dwm_preview: tauri::State<'_, DwmPreviewState>) -> Result<(), String> {
    dwm_preview.clear();
    Ok(())
}

fn capture_preview_from_frame(
    frame: screen_watch_core::detect::RgbFrame,
    source_signature: String,
    cached: bool,
) -> Result<CapturePreview, String> {
    let png = frame_to_png_bytes(&frame).map_err(|err| err.to_string())?;
    Ok(CapturePreview {
        width: frame.width,
        height: frame.height,
        data_url: format!("data:image/png;base64,{}", BASE64_STANDARD.encode(png)),
        source_signature,
        cached,
    })
}

#[tauri::command]
fn validate_config_text(text: String) -> Result<(), String> {
    let config = screen_watch_core::config::WatchConfig::from_json_str(&text)
        .map_err(|err| err.to_string())?;
    config.validate().map_err(|err| err.to_string())
}

#[tauri::command]
fn normalize_profile(profile_number: u32) -> Result<bool, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    normalize_profile_file_at(path, data_dir, profile_number).map_err(|err| err.to_string())
}

#[tauri::command]
fn load_profile(profile_number: u32) -> Result<ProfileReadResult, String> {
    let (_, path) = checked_profile_path(profile_number)?;
    read_profile_at(path).map_err(|err| err.to_string())
}

#[tauri::command]
fn load_profile_state() -> Result<ProfileStateResult, String> {
    read_profile_state_at(user_data_dir()).map_err(|err| err.to_string())
}

#[tauri::command]
fn save_last_profile(profile_number: u32) -> Result<ProfileStateResult, String> {
    checked_profile_path(profile_number)?;
    save_last_profile_at(user_data_dir(), profile_number).map_err(|err| err.to_string())
}

#[tauri::command]
fn save_profile_sources(
    profile_number: u32,
    options: ProfileWatchConfigOptions,
) -> Result<ProfileSourcesSaveResult, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    let max_alerts = options.max_alerts;
    let result = save_profile_sources_at(path, options).map_err(|err| err.to_string())?;
    if let Some(max_alerts) = max_alerts {
        save_max_alerts_at(data_dir, max_alerts).map_err(|err| err.to_string())?;
    }
    Ok(result)
}

#[tauri::command]
fn record_profile_hits(profile_number: u32, target_ids: Vec<String>) -> Result<bool, String> {
    let (_, path) = checked_profile_path(profile_number)?;
    record_profile_hits_at(path, &target_ids).map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_profile_target_hit_count(
    profile_number: u32,
    target_id: String,
) -> Result<ProfileTargetsEditResult, String> {
    let (_, path) = checked_profile_path(profile_number)?;
    clear_profile_target_hit_count_at(path, &target_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn add_profile_template_pngs(
    profile_number: u32,
    image_paths: Vec<String>,
    max_templates: usize,
) -> Result<AddTemplateImagesResult, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    let image_paths = image_paths
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    add_profile_template_pngs_at(path, data_dir, profile_number, &image_paths, max_templates)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn select_profile_template_pngs() -> Result<Vec<String>, String> {
    select_png_files_with_dialog().map(|paths| {
        paths
            .into_iter()
            .map(|path| path.display().to_string())
            .collect()
    })
}

#[tauri::command]
fn paste_profile_template_images(
    profile_number: u32,
    max_templates: usize,
) -> Result<AddTemplateImagesResult, String> {
    let clipboard =
        clipboard_import::read_clipboard_template_images().map_err(|err| err.to_string())?;
    let mut frames = clipboard.frames;
    for path in clipboard.paths {
        let frame = screen_watch_core::detect::RgbFrame::from_image_path(&path)
            .map_err(|err| err.to_string())?;
        frames.push(frame);
    }
    if frames.is_empty() {
        return Err("剪贴板里没有图片；用截图工具复制后再按 Ctrl+V。".to_string());
    }
    let (data_dir, path) = checked_profile_path(profile_number)?;
    add_profile_template_frames_at(path, data_dir, profile_number, &frames, max_templates)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn add_profile_template_clipboard_images(
    profile_number: u32,
    images: Vec<ClipboardImagePayload>,
    max_templates: usize,
) -> Result<AddTemplateImagesResult, String> {
    if images.is_empty() {
        return Err("剪贴板里没有图片；用截图工具复制后再按 Ctrl+V。".to_string());
    }
    let mut frames = Vec::with_capacity(images.len());
    for (index, image) in images.into_iter().enumerate() {
        let bytes = BASE64_STANDARD
            .decode(image.data_base64.as_bytes())
            .map_err(|err| format!("cannot decode clipboard image data: {err}"))?;
        let fallback = image
            .mime_type
            .as_deref()
            .and_then(extension_from_mime_type)
            .map(|ext| format!("clipboard-{}.{}", index + 1, ext))
            .unwrap_or_else(|| format!("clipboard-{}", index + 1));
        let label = image
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&fallback);
        frames.push(
            screen_watch_core::detect::RgbFrame::from_image_bytes(label, &bytes)
                .map_err(|err| err.to_string())?,
        );
    }
    let (data_dir, path) = checked_profile_path(profile_number)?;
    add_profile_template_frames_at(path, data_dir, profile_number, &frames, max_templates)
        .map_err(|err| err.to_string())
}

fn extension_from_mime_type(mime_type: &str) -> Option<&'static str> {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/bmp" | "image/x-ms-bmp" => Some("bmp"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

#[tauri::command]
fn capture_profile_source_template(
    window: tauri::Window,
    profile_number: u32,
    options: ProfileWatchConfigOptions,
) -> Result<AddTemplateImagesResult, String> {
    let frame = capture_profile_source_frame(&window, &options)?;
    let (data_dir, path) = checked_profile_path(profile_number)?;
    add_profile_template_frames_at(
        path,
        data_dir,
        profile_number,
        std::slice::from_ref(&frame),
        options.max_templates,
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn reorder_profile_target(
    profile_number: u32,
    from_index: usize,
    insert_index: usize,
) -> Result<ProfileTargetsEditResult, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    reorder_profile_target_at(path, data_dir, profile_number, from_index, insert_index)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn remove_profile_target(
    profile_number: u32,
    index: usize,
) -> Result<ProfileTargetsEditResult, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    remove_profile_target_at(path, data_dir, profile_number, index).map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_profile_targets(profile_number: u32) -> Result<ProfileTargetsEditResult, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    clear_profile_targets_at(path, data_dir).map_err(|err| err.to_string())
}

#[tauri::command]
fn set_profile_target_enabled(
    profile_number: u32,
    index: usize,
    enabled: bool,
) -> Result<ProfileTargetsEnabledResult, String> {
    let (_, path) = checked_profile_path(profile_number)?;
    set_profile_target_enabled_at(path, index, enabled).map_err(|err| err.to_string())
}

#[tauri::command]
fn toggle_all_profile_targets(profile_number: u32) -> Result<ProfileTargetsEnabledResult, String> {
    let (_, path) = checked_profile_path(profile_number)?;
    toggle_all_profile_targets_at(path).map_err(|err| err.to_string())
}

#[tauri::command]
fn open_profile_target_file(
    profile_number: u32,
    index: usize,
) -> Result<OpenTargetFileResult, String> {
    let (_, path) = checked_profile_path(profile_number)?;
    let target_path = profile_target_file_to_open(path, index)?;
    open_path_with_default_app(&target_path)?;
    Ok(OpenTargetFileResult {
        path: target_path.display().to_string(),
    })
}

#[tauri::command]
fn open_evidence_dir() -> Result<OpenTargetFileResult, String> {
    open_evidence_dir_at(user_data_dir(), open_path_with_default_app)
}

fn open_evidence_dir_at(
    data_dir: impl AsRef<Path>,
    opener: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<OpenTargetFileResult, String> {
    let path = screenshots_dir(data_dir);
    std::fs::create_dir_all(&path).map_err(|err| err.to_string())?;
    opener(&path)?;
    Ok(OpenTargetFileResult {
        path: path.display().to_string(),
    })
}

#[tauri::command]
fn profile_target_thumbnail(
    profile_number: u32,
    index: usize,
) -> Result<ProfileTargetThumbnail, String> {
    let (data_dir, _) = checked_profile_path(profile_number)?;
    profile_target_thumbnail_at(data_dir, profile_number, index)
}

fn profile_target_thumbnail_at(
    data_dir: impl AsRef<Path>,
    profile_number: u32,
    index: usize,
) -> Result<ProfileTargetThumbnail, String> {
    let path = profile_path(data_dir, profile_number);
    let target_path = profile_target_file_to_open(path, index)?;
    let bytes = std::fs::read(&target_path)
        .map_err(|err| format!("failed to read profile target image: {err}"))?;
    Ok(ProfileTargetThumbnail {
        path: target_path.display().to_string(),
        data_url: format!("data:image/png;base64,{}", BASE64_STANDARD.encode(bytes)),
    })
}

#[tauri::command]
fn build_profile_watch_config(
    profile_number: u32,
    options: ProfileWatchConfigOptions,
) -> Result<WatchConfig, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    profile_watch_config_at(path, data_dir, options).map_err(|err| err.to_string())
}

#[tauri::command]
fn scan_profile_once(
    window: tauri::Window,
    beep_state: tauri::State<'_, AlarmBeepState>,
    profile_number: u32,
    options: ProfileWatchConfigOptions,
) -> Result<OneShotScanResult, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    let config =
        profile_watch_config_at(&path, &data_dir, options).map_err(|err| err.to_string())?;
    let result = scan_watch_config_once(&window, beep_state.inner(), config, data_dir)?;
    let target_ids = one_shot_alerted_target_ids(&result);
    if !target_ids.is_empty() {
        record_profile_hits_at(path, &target_ids).map_err(|err| err.to_string())?;
    }
    Ok(result)
}

#[tauri::command]
fn start_profile_monitoring_session(
    window: tauri::Window,
    state: tauri::State<'_, MonitorSessionState>,
    beep_state: tauri::State<'_, AlarmBeepState>,
    profile_number: u32,
    options: ProfileWatchConfigOptions,
) -> Result<MonitorSessionSnapshot, String> {
    let (data_dir, path) = checked_profile_path(profile_number)?;
    let config =
        profile_watch_config_at(&path, &data_dir, options).map_err(|err| err.to_string())?;
    start_monitoring_with_config_and_hit_sink(
        window,
        state,
        beep_state,
        config,
        data_dir,
        Some(Arc::new(ProfileHitSink { path })),
    )
}

fn one_shot_alerted_target_ids(result: &OneShotScanResult) -> Vec<String> {
    result
        .regions
        .iter()
        .chain(result.windows.iter())
        .flat_map(alerted_target_ids)
        .collect()
}

fn capture_profile_source_frame(
    window: &tauri::Window,
    options: &ProfileWatchConfigOptions,
) -> Result<screen_watch_core::detect::RgbFrame, String> {
    let monitors = monitors_for_window(window)?;
    let core_monitors = core_monitors_from_listed(&monitors);
    let available_windows = if options.window_apps.is_empty() {
        Vec::new()
    } else {
        window_sources::list_app_windows()?
    };
    let sources =
        profile_capture_sources_for_monitors(options, &core_monitors, &available_windows)?;

    capture_profile_source_frame_from_sources(&sources)
}

fn capture_profile_source_frame_from_sources(
    sources: &ProfileCaptureSources,
) -> Result<screen_watch_core::detect::RgbFrame, String> {
    if let Some(region) = sources.regions.first() {
        return capture_screen_region(CaptureRegion {
            left: region.bbox.left,
            top: region.bbox.top,
            width: region.bbox.width,
            height: region.bbox.height,
        })
        .map_err(|err| err.to_string());
    }

    let mut mode_cache = window_capture::WindowCaptureModeCache::default();
    for window in &sources.windows {
        let Some(hwnd) = window.hwnd else {
            continue;
        };
        if let Some(frame) = window_capture::capture_window_frame(hwnd, Some(&mut mode_cache))
            .map_err(|err| err.to_string())?
        {
            return Ok(frame);
        }
    }

    if !sources.windows.is_empty() {
        return Err("应用窗口截图为空或黑屏，请把窗口露出后重试。".to_string());
    }
    Err("当前没有可截图来源，请先勾选一个屏幕或选择一个已启动的应用。".to_string())
}

fn profile_capture_sources_for_monitors(
    options: &ProfileWatchConfigOptions,
    monitors: &[MonitorInfo],
    available_windows: &[AppWindow],
) -> Result<ProfileCaptureSources, String> {
    let regions = if options.regions.is_empty() {
        Vec::new()
    } else {
        let config = WatchConfig {
            poll_interval_seconds: options.poll_interval_seconds,
            cooldown_seconds: options.cooldown_seconds,
            template_workers: options.template_workers,
            regions: options.regions.clone(),
            windows: Vec::new(),
            window_apps: Vec::new(),
            targets: Vec::new(),
            alarm: Default::default(),
            extra: Default::default(),
        };
        screen_watch_core::sources::resolve_regions(&config, monitors)
            .map_err(|err| err.to_string())?
    };

    let skipped_windows = options
        .windows
        .iter()
        .filter(|window| window.hwnd.is_none())
        .count();
    let mut windows = options
        .windows
        .iter()
        .filter(|window| window.hwnd.is_some())
        .cloned()
        .collect::<Vec<_>>();
    let resolution = window_sources::resolve_window_apps(&options.window_apps, available_windows);
    let skipped_window_apps = resolution.missing_window_apps.len();
    windows.extend(resolution.windows);

    if regions.is_empty() && windows.is_empty() {
        return Err("当前没有可截图来源，请先勾选一个屏幕或选择一个已启动的应用。".to_string());
    }

    Ok(ProfileCaptureSources {
        regions,
        windows,
        skipped_windows,
        skipped_window_apps,
    })
}

fn core_monitors_from_listed(monitors: &[ListedMonitor]) -> Vec<MonitorInfo> {
    monitors
        .iter()
        .map(|monitor| MonitorInfo {
            index: monitor.index,
            left: monitor.left,
            top: monitor.top,
            width: monitor.width,
            height: monitor.height,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
struct ScanClock {
    now_seconds: f64,
    time_text: String,
    stamp: String,
}

impl ScanClock {
    fn now() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis();
        Self {
            now_seconds: secs as f64 + f64::from(millis) / 1000.0,
            time_text: format!("unix-{secs}.{millis:03}"),
            stamp: format!("unix-{secs}-{millis:03}"),
        }
    }
}

fn monitors_for_window(window: &tauri::Window) -> Result<Vec<ListedMonitor>, String> {
    let physical = window
        .available_monitors()
        .map_err(|err| err.to_string())?
        .into_iter()
        .enumerate()
        .map(|(offset, monitor)| {
            let position = monitor.position();
            let size = monitor.size();
            ListedMonitor {
                index: offset as i32 + 1,
                left: position.x,
                top: position.y,
                width: size.width,
                height: size.height,
                name: monitor.name().cloned(),
                scale_factor: monitor.scale_factor(),
                is_virtual: false,
            }
        })
        .collect::<Vec<_>>();
    Ok(with_virtual_monitor(physical))
}

#[cfg(all(test, windows))]
fn windows_desktop_monitors() -> Result<Vec<ListedMonitor>, String> {
    Ok(with_virtual_monitor(windows_physical_monitors()?))
}

#[cfg(all(test, windows))]
fn windows_physical_monitors() -> Result<Vec<ListedMonitor>, String> {
    use std::mem::size_of;
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{LPARAM, RECT},
            Graphics::Gdi::{
                EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
            },
        },
    };

    struct EnumState {
        monitors: Vec<ListedMonitor>,
        error: Option<String>,
    }

    unsafe extern "system" fn enum_monitor(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let state = unsafe { &mut *(data.0 as *mut EnumState) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        let ok = unsafe {
            GetMonitorInfoW(
                monitor,
                &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
            )
        };
        if !ok.as_bool() {
            state.error = Some(format!(
                "GetMonitorInfoW failed: {}",
                std::io::Error::last_os_error()
            ));
            return false.into();
        }

        let rect = info.monitorInfo.rcMonitor;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            state.error = Some(format!(
                "monitor reported non-positive bounds: left={} top={} right={} bottom={}",
                rect.left, rect.top, rect.right, rect.bottom
            ));
            return false.into();
        }

        state.monitors.push(ListedMonitor {
            index: state.monitors.len() as i32 + 1,
            left: rect.left,
            top: rect.top,
            width: width as u32,
            height: height as u32,
            name: utf16_nul_terminated_to_string(&info.szDevice),
            scale_factor: 1.0,
            is_virtual: false,
        });
        true.into()
    }

    let mut state = EnumState {
        monitors: Vec::new(),
        error: None,
    };
    let ok = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor),
            LPARAM(&mut state as *mut _ as isize),
        )
    };
    if !ok.as_bool() {
        if let Some(error) = state.error {
            return Err(error);
        }
        return Err(format!(
            "EnumDisplayMonitors failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(state.monitors)
}

#[cfg(all(test, windows))]
fn windows_virtual_screen_metrics() -> ListedMonitor {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    ListedMonitor {
        index: 0,
        left: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        top: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        width: unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(0) as u32,
        height: unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(0) as u32,
        name: Some("virtual".to_string()),
        scale_factor: 1.0,
        is_virtual: true,
    }
}

#[cfg(all(test, windows))]
fn utf16_nul_terminated_to_string(value: &[u16]) -> Option<String> {
    let end = value
        .iter()
        .position(|code| *code == 0)
        .unwrap_or(value.len());
    if end == 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&value[..end]))
    }
}

fn with_virtual_monitor(physical: Vec<ListedMonitor>) -> Vec<ListedMonitor> {
    if physical.is_empty() {
        return Vec::new();
    }
    let left = physical
        .iter()
        .map(|monitor| monitor.left)
        .min()
        .unwrap_or(0);
    let top = physical
        .iter()
        .map(|monitor| monitor.top)
        .min()
        .unwrap_or(0);
    let right = physical
        .iter()
        .map(|monitor| i64::from(monitor.left) + i64::from(monitor.width))
        .max()
        .unwrap_or(i64::from(left));
    let bottom = physical
        .iter()
        .map(|monitor| i64::from(monitor.top) + i64::from(monitor.height))
        .max()
        .unwrap_or(i64::from(top));
    let virtual_monitor = ListedMonitor {
        index: 0,
        left,
        top,
        width: (right - i64::from(left)).max(0) as u32,
        height: (bottom - i64::from(top)).max(0) as u32,
        name: Some("virtual".to_string()),
        scale_factor: 1.0,
        is_virtual: true,
    };
    let mut out = Vec::with_capacity(physical.len() + 1);
    out.push(virtual_monitor);
    out.extend(physical);
    out
}

fn checked_profile_path(profile_number: u32) -> Result<(PathBuf, PathBuf), String> {
    if !(1..=PROFILE_COUNT).contains(&profile_number) {
        return Err(format!(
            "profile_number must be between 1 and {PROFILE_COUNT}"
        ));
    }
    let data_dir = user_data_dir();
    let path = profile_path(&data_dir, profile_number);
    Ok((data_dir, path))
}

fn parse_open_file_name_buffer(buffer: &[u16]) -> Vec<PathBuf> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    for (index, value) in buffer.iter().enumerate() {
        if *value != 0 {
            continue;
        }
        if index == start {
            break;
        }
        parts.push(String::from_utf16_lossy(&buffer[start..index]));
        start = index + 1;
    }

    if parts.len() <= 1 {
        return parts.into_iter().map(PathBuf::from).collect();
    }

    let dir = PathBuf::from(&parts[0]);
    parts
        .into_iter()
        .skip(1)
        .map(|name| dir.join(name))
        .collect()
}

fn select_png_files_with_dialog() -> Result<Vec<PathBuf>, String> {
    #[cfg(windows)]
    {
        use windows::{
            core::{PCWSTR, PWSTR},
            Win32::{
                Foundation::HWND,
                UI::Controls::Dialogs::{
                    CommDlgExtendedError, GetOpenFileNameW, OFN_ALLOWMULTISELECT, OFN_EXPLORER,
                    OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_PATHMUSTEXIST, OPENFILENAMEW,
                },
            },
        };

        let filter =
            widestr("Images\0*.png;*.PNG;*.jpg;*.JPG;*.jpeg;*.JPEG;*.bmp;*.BMP;*.webp;*.WEBP\0All files\0*.*\0");
        let title = widestr("Select template images");
        let def_ext = widestr("png");
        let mut buffer = vec![0u16; 65_536];
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: HWND::default(),
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrDefExt: PCWSTR(def_ext.as_ptr()),
            Flags: OFN_EXPLORER
                | OFN_ALLOWMULTISELECT
                | OFN_FILEMUSTEXIST
                | OFN_PATHMUSTEXIST
                | OFN_HIDEREADONLY,
            ..OPENFILENAMEW::default()
        };

        let ok = unsafe { GetOpenFileNameW(&mut ofn).as_bool() };
        if ok {
            return Ok(parse_open_file_name_buffer(&buffer));
        }

        let err = unsafe { CommDlgExtendedError() };
        if err.0 == 0 {
            return Ok(Vec::new());
        }
        Err(format!(
            "file selection failed with common dialog error {}",
            err.0
        ))
    }

    #[cfg(not(windows))]
    {
        Err("native image selection is only implemented on Windows".to_string())
    }
}

fn profile_target_file_to_open(
    profile_path: impl AsRef<Path>,
    index: usize,
) -> Result<PathBuf, String> {
    let profile = read_profile_at(profile_path).map_err(|err| err.to_string())?;
    let target = profile
        .targets
        .get(index)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("profile target index {index} is unavailable"))?;
    let path = target
        .get("path")
        .and_then(Value::as_str)
        .filter(|item| !item.is_empty())
        .ok_or_else(|| "profile target has no image path".to_string())?;
    let path = PathBuf::from(path);
    let path = path
        .canonicalize()
        .map_err(|err| format!("profile target image is unavailable: {err}"))?;
    if !path.is_file() {
        return Err("profile target image path is not a file".to_string());
    }
    let is_png = path
        .extension()
        .and_then(|item| item.to_str())
        .map(|item| item.eq_ignore_ascii_case("png"))
        .unwrap_or(false);
    if !is_png {
        return Err("profile target image must be a PNG file".to_string());
    }
    Ok(path)
}

fn open_path_with_default_app(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            core::PCWSTR,
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        };

        let operation = widestr("open");
        let file = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(operation.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err(format!(
                "failed to open profile target image, ShellExecuteW code {}",
                result.0 as isize
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
}

#[cfg(windows)]
fn widestr(text: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[derive(Clone)]
struct TauriMonitorEventSink {
    app: tauri::AppHandle,
    window: tauri::Window,
}

#[derive(Clone)]
struct ProfileHitSink {
    path: PathBuf,
}

impl MonitorSessionEventSink for TauriMonitorEventSink {
    fn emit(&self, event: MonitorSessionEvent) {
        tray::update_monitoring_status(&self.app, event.snapshot.running);
        let _ = self.window.emit(MONITOR_SESSION_EVENT, event);
    }
}

impl MonitorSessionHitSink for ProfileHitSink {
    fn record(&self, target_ids: &[String]) -> Result<(), String> {
        record_profile_hits_at(&self.path, target_ids)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
}

pub fn run() {
    let wake_target = Arc::new(Mutex::new(None::<tauri::AppHandle>));
    let wake_target_for_listener = Arc::clone(&wake_target);
    let single_instance = match single_instance::claim_single_instance(
        std::time::Duration::from_millis(500),
        move || {
            if let Ok(guard) = wake_target_for_listener.lock() {
                if let Some(app) = guard.as_ref() {
                    tray::show_main_window(app);
                }
            }
        },
    ) {
        single_instance::ClaimResult::NotifiedExisting => return,
        single_instance::ClaimResult::Listening(guard) => Some(guard),
        single_instance::ClaimResult::Unavailable(err) => {
            eprintln!("single-instance listener unavailable: {err}");
            None
        }
    };

    migrate_legacy_data_for_current_exe();

    let tray_state = Arc::new(tray::TrayLifecycleState::default());
    let start_minimized = tray::start_minimized_from_env_args();
    let wake_target_for_setup = Arc::clone(&wake_target);
    let tray_state_for_setup = Arc::clone(&tray_state);
    let tray_state_for_window_event = Arc::clone(&tray_state);
    let mut builder = tauri::Builder::default()
        .manage(MonitorSessionState::default())
        .manage(AlarmBeepState::default())
        .manage(PreviewCacheState::default())
        .manage(DwmPreviewState::default())
        .setup(move |app| {
            if let Ok(mut guard) = wake_target_for_setup.lock() {
                *guard = Some(app.handle().clone());
            }
            window_layout::apply_saved_window_geometry(app.handle());
            let tray_available = match tray::install_tray(app.handle()) {
                Ok(()) => {
                    tray_state_for_setup.set_available(true);
                    true
                }
                Err(err) => {
                    tray_state_for_setup.set_available(false);
                    eprintln!("tray unavailable: {err}");
                    false
                }
            };
            tray::apply_startup_visibility(app.handle(), start_minimized, tray_available);
            Ok(())
        })
        .on_window_event(move |window, event| {
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) {
                window.state::<DwmPreviewState>().clear();
            }
            window_layout::handle_window_event(window, event);
            tray::handle_window_event(window, event, &tray_state_for_window_event);
        })
        .on_menu_event(tray::handle_menu_event)
        .on_tray_icon_event(tray::handle_tray_icon_event)
        .invoke_handler(tauri::generate_handler![
            app_info,
            ocr_backend_probe,
            list_monitors,
            list_app_windows,
            resolve_config_text_sources,
            resolve_config_text_window_sources,
            scan_config_text_once,
            start_monitoring_session,
            stop_monitoring_session,
            monitoring_session_status,
            startup_status,
            set_startup_enabled,
            capture_screen_region_preview,
            capture_screen_region_preview_cached,
            capture_window_preview,
            capture_window_preview_cached,
            retain_cached_preview_sources,
            sync_dwm_preview,
            retain_dwm_preview_sources,
            clear_dwm_previews,
            validate_config_text,
            normalize_profile,
            load_profile,
            load_profile_state,
            save_last_profile,
            save_profile_sources,
            record_profile_hits,
            clear_profile_target_hit_count,
            add_profile_template_pngs,
            select_profile_template_pngs,
            paste_profile_template_images,
            add_profile_template_clipboard_images,
            capture_profile_source_template,
            reorder_profile_target,
            remove_profile_target,
            clear_profile_targets,
            set_profile_target_enabled,
            toggle_all_profile_targets,
            open_profile_target_file,
            open_evidence_dir,
            profile_target_thumbnail,
            build_profile_watch_config,
            scan_profile_once,
            start_profile_monitoring_session
        ]);
    if let Some(guard) = single_instance {
        builder = builder.manage(guard);
    }
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn migrate_legacy_data_for_current_exe() {
    let Some(app_root) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    else {
        return;
    };
    let legacy_dir = legacy_data_dir_from_app_root(app_root);
    match migrate_legacy_data_at(legacy_dir, user_data_dir()) {
        Ok(result) if result.migrated => {
            eprintln!(
                "legacy app_data migration copied {} files across {} directories, skipped {} existing files",
                result.copied_files, result.copied_dirs, result.skipped_existing_files
            );
        }
        Ok(_) => {}
        Err(err) => eprintln!("legacy app_data migration skipped: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        capture_profile_source_frame_from_sources, concrete_windows_from_sources,
        core_monitors_from_listed, one_shot_alerted_target_ids, open_evidence_dir_at,
        parse_open_file_name_buffer, profile_capture_sources_for_monitors,
        profile_target_file_to_open, profile_target_thumbnail_at,
        scan_resolved_screen_regions_once, with_virtual_monitor, AppInfo, ListedMonitor,
        OneShotScanResult, ProfileCaptureSources, ProfileHitSink, ScanClock,
    };
    use crate::audio::AlarmBeepState;
    use crate::monitor_session::{
        MonitorSessionEvent, MonitorSessionEventSink, MonitorSessionHitSink, MonitorSessionState,
    };
    use crate::screen_capture::CaptureRegion;
    use crate::window_sources::AppWindow;
    use screen_watch_core::{
        build::BuildFlavor,
        config::{RegionConfig, WatchConfig, WindowConfig},
        detect::{Match, RgbFrame},
        evidence::save_rgb_png,
        ocr::OcrSettings,
        profile::{
            add_profile_template_frames_at, add_profile_template_pngs_at, clear_profile_targets_at,
            profile_path, profile_watch_config_at, read_profile_at, record_profile_hits_at,
            remove_profile_target_at, reorder_profile_target_at, screenshots_dir,
            set_profile_target_enabled_at, templates_dir, toggle_all_profile_targets_at,
            ProfileTargetsEditResult, ProfileWatchConfigOptions,
        },
        scan::ScanFrameResult,
        sources::{BBox, MonitorInfo, ResolvedRegion, ResolvedSources},
    };
    use serde_json::json;
    use std::{fs, sync::Arc, thread, time::Duration};

    #[derive(Debug, Default)]
    struct TestEventSink;

    impl MonitorSessionEventSink for TestEventSink {
        fn emit(&self, _event: MonitorSessionEvent) {}
    }

    #[test]
    fn virtual_monitor_matches_combined_physical_bounds() {
        let monitors = with_virtual_monitor(vec![
            monitor(1, 0, 0, 1920, 1080),
            monitor(2, -1280, 100, 1280, 720),
        ]);
        assert_eq!(monitors[0].index, 0);
        assert_eq!(monitors[0].left, -1280);
        assert_eq!(monitors[0].top, 0);
        assert_eq!(monitors[0].width, 3200);
        assert_eq!(monitors[0].height, 1080);
        assert!(monitors[0].is_virtual);
        assert_eq!(monitors[1].index, 1);
        assert_eq!(monitors[2].index, 2);
    }

    #[test]
    fn virtual_monitor_is_omitted_when_no_physical_monitors_are_available() {
        assert!(with_virtual_monitor(Vec::new()).is_empty());
    }

    #[test]
    fn listed_monitors_convert_to_core_monitor_shape() {
        let monitors = core_monitors_from_listed(&[monitor(3, -10, 20, 30, 40)]);
        assert_eq!(monitors[0].index, 3);
        assert_eq!(monitors[0].left, -10);
        assert_eq!(monitors[0].top, 20);
        assert_eq!(monitors[0].width, 30);
        assert_eq!(monitors[0].height, 40);
    }

    #[test]
    fn app_info_serializes_frontend_ocr_contract_fields_as_camel_case() {
        let tmp = tempfile::tempdir().unwrap();
        let ocr = OcrSettings::from_sources_with_module(
            BuildFlavor::Full,
            tmp.path().to_path_buf(),
            None,
            false,
        )
        .availability();
        let value = serde_json::to_value(AppInfo {
            build_flavor: "full".to_string(),
            data_dir: tmp.path().display().to_string(),
            ocr,
        })
        .unwrap();

        assert_eq!(value["buildFlavor"], "full");
        assert!(value.get("build_flavor").is_none());
        assert!(value.get("dataDir").is_some());
        assert!(value.get("data_dir").is_none());

        let ocr = value["ocr"].as_object().unwrap();
        for field in [
            "enabled",
            "available",
            "moduleCompiled",
            "modelsReady",
            "backendReady",
            "backendName",
            "modelProfile",
            "modelDir",
            "requiredModels",
            "referenceModels",
            "missingModels",
            "reason",
        ] {
            assert!(ocr.contains_key(field), "missing OCR field {field}");
        }
        for snake_case_field in [
            "module_compiled",
            "models_ready",
            "backend_ready",
            "backend_name",
            "model_profile",
            "model_dir",
            "required_models",
            "reference_models",
            "missing_models",
        ] {
            assert!(
                !ocr.contains_key(snake_case_field),
                "unexpected snake_case OCR field {snake_case_field}"
            );
        }

        let required_models = ocr["requiredModels"].as_array().unwrap();
        assert_eq!(required_models.len(), 3);
        for model in required_models {
            let model = model.as_object().unwrap();
            for field in ["name", "path", "exists", "bytes"] {
                assert!(model.contains_key(field), "missing model field {field}");
            }
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore]
    fn real_windows_monitor_listing_matches_python_mss_indexing_on_desktop() {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CMONITORS};

        let monitors = super::windows_desktop_monitors().unwrap();
        assert!(!monitors.is_empty());
        let virtual_monitor = &monitors[0];
        let system_virtual = super::windows_virtual_screen_metrics();
        assert_eq!(virtual_monitor.index, 0);
        assert!(virtual_monitor.is_virtual);
        assert_eq!(virtual_monitor.left, system_virtual.left);
        assert_eq!(virtual_monitor.top, system_virtual.top);
        assert_eq!(virtual_monitor.width, system_virtual.width);
        assert_eq!(virtual_monitor.height, system_virtual.height);

        let physical = monitors
            .iter()
            .filter(|monitor| !monitor.is_virtual)
            .collect::<Vec<_>>();
        assert!(!physical.is_empty());
        assert_eq!(physical.len() as i32, unsafe {
            GetSystemMetrics(SM_CMONITORS)
        });
        for (offset, monitor) in physical.iter().enumerate() {
            assert_eq!(monitor.index, offset as i32 + 1);
            assert_ne!(monitor.index, 0);
            assert!(monitor.width > 0);
            assert!(monitor.height > 0);
            assert!(!monitor.is_virtual);
        }
    }

    #[test]
    fn scan_clock_uses_filename_safe_stamp() {
        let clock = ScanClock::now();
        assert!(clock.now_seconds > 0.0);
        assert!(clock.time_text.starts_with("unix-"));
        assert!(clock.stamp.starts_with("unix-"));
        assert!(!clock.stamp.contains(':'));
    }

    #[test]
    fn concrete_windows_from_sources_keeps_direct_handles_and_counts_missing() {
        let sources = ResolvedSources {
            regions: Vec::new(),
            windows: vec![
                WindowConfig {
                    name: "ready".to_string(),
                    title: "Ready".to_string(),
                    display: "Ready".to_string(),
                    hwnd: Some(99),
                    extra: Default::default(),
                },
                WindowConfig {
                    name: "missing".to_string(),
                    title: "Missing".to_string(),
                    display: "Missing".to_string(),
                    hwnd: None,
                    extra: Default::default(),
                },
            ],
            window_apps: Vec::new(),
        };

        let (windows, skipped_windows, skipped_window_apps) =
            concrete_windows_from_sources(&sources).unwrap();

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].hwnd, Some(99));
        assert_eq!(skipped_windows, 1);
        assert_eq!(skipped_window_apps, 0);
    }

    #[test]
    fn profile_capture_sources_allow_empty_targets_and_prefer_screen_regions() {
        let options = ProfileWatchConfigOptions {
            regions: vec![RegionConfig {
                name: "crop".to_string(),
                monitor: 1,
                left: 5,
                top: 6,
                width: Some(7),
                height: Some(8),
                extra: Default::default(),
            }],
            windows: vec![WindowConfig {
                name: "window".to_string(),
                title: "Window".to_string(),
                display: "Window".to_string(),
                hwnd: Some(44),
                extra: Default::default(),
            }],
            max_templates: 3,
            ..ProfileWatchConfigOptions::default()
        };

        let sources = profile_capture_sources_for_monitors(
            &options,
            &[MonitorInfo {
                index: 1,
                left: 100,
                top: 200,
                width: 300,
                height: 400,
            }],
            &[],
        )
        .unwrap();

        assert_eq!(sources.regions.len(), 1);
        assert_eq!(sources.regions[0].bbox.left, 105);
        assert_eq!(sources.regions[0].bbox.top, 206);
        assert_eq!(sources.regions[0].bbox.width, 7);
        assert_eq!(sources.windows.len(), 1);
        assert_eq!(sources.skipped_windows, 0);
        assert_eq!(sources.skipped_window_apps, 0);
    }

    #[test]
    fn profile_capture_sources_resolve_remembered_apps_and_report_missing_sources() {
        let options = ProfileWatchConfigOptions {
            window_apps: vec![
                screen_watch_core::config::WindowAppConfig {
                    title: "Logs".to_string(),
                    ordinal: 1,
                    extra: Default::default(),
                },
                screen_watch_core::config::WindowAppConfig {
                    title: "Missing".to_string(),
                    ordinal: 1,
                    extra: Default::default(),
                },
            ],
            ..ProfileWatchConfigOptions::default()
        };
        let windows = [AppWindow {
            hwnd: 99,
            title: "Logs".to_string(),
            width: 320,
            height: 240,
            ordinal: 1,
            key: screen_watch_core::profile::window_key("Logs", 1),
            display: "Logs".to_string(),
        }];

        let sources = profile_capture_sources_for_monitors(&options, &[], &windows).unwrap();

        assert!(sources.regions.is_empty());
        assert_eq!(sources.windows.len(), 1);
        assert_eq!(sources.windows[0].hwnd, Some(99));
        assert_eq!(sources.skipped_window_apps, 1);
    }

    #[test]
    fn profile_capture_sources_report_no_screenshot_source() {
        let err = profile_capture_sources_for_monitors(
            &ProfileWatchConfigOptions::default(),
            &[MonitorInfo {
                index: 1,
                left: 0,
                top: 0,
                width: 100,
                height: 100,
            }],
            &[],
        )
        .unwrap_err();

        assert!(err.contains("当前没有可截图来源"));
    }

    #[test]
    fn one_shot_alerted_target_ids_uses_cooldown_filtered_matches() {
        let result = OneShotScanResult {
            regions: vec![scan_result("region-alert")],
            windows: vec![scan_result("window-alert")],
            hit_count: 2,
            skipped_windows: 0,
            skipped_window_apps: 0,
        };

        assert_eq!(
            one_shot_alerted_target_ids(&result),
            vec!["region-alert", "window-alert"]
        );
    }

    #[test]
    fn profile_hit_sink_records_target_ids_to_profile_file() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [
                    {"id": "a", "hit_count": 1},
                    {"id": "b"}
                ],
                "future": true
            }))
            .unwrap(),
        )
        .unwrap();
        let sink = ProfileHitSink {
            path: profile.clone(),
        };

        sink.record(&["a".to_string(), "b".to_string(), "a".to_string()])
            .unwrap();

        let stored: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["targets"][0]["hit_count"], json!(3));
        assert_eq!(stored["targets"][1]["hit_count"], json!(1));
        assert_eq!(stored["future"], json!(true));
    }

    #[test]
    fn profile_targets_edit_result_serializes_selected_index_for_frontend() {
        let result = ProfileTargetsEditResult {
            changed: true,
            deleted_files: 0,
            selected_index: Some(1),
            targets: vec![json!({"id": "a"}), json!({"id": "b"})],
        };

        let value = serde_json::to_value(result).unwrap();

        assert_eq!(value["selectedIndex"], json!(1));
        assert!(value.get("selected_index").is_none());
    }

    #[test]
    fn open_evidence_dir_uses_python_compatible_screenshots_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let expected = screenshots_dir(&data_dir);
        let mut opened = None;

        let result = open_evidence_dir_at(&data_dir, |path| {
            opened = Some(path.to_path_buf());
            Ok(())
        })
        .unwrap();

        assert_eq!(opened.as_deref(), Some(expected.as_path()));
        assert_eq!(result.path, expected.display().to_string());
        assert!(expected.is_dir());
        assert!(!tmp.path().join("alerts").exists());
    }

    #[test]
    fn open_evidence_dir_reports_shell_open_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let expected = screenshots_dir(&data_dir);

        let err = open_evidence_dir_at(&data_dir, |_path| {
            Err("shell open failed for test".to_string())
        })
        .unwrap_err();

        assert!(err.contains("shell open failed for test"));
        assert!(expected.is_dir());
    }

    #[test]
    fn parse_open_file_name_buffer_reads_single_selected_file() {
        let buffer = wide_buffer(r"C:\images\one.png", &[]);

        assert_eq!(
            parse_open_file_name_buffer(&buffer),
            vec![std::path::PathBuf::from(r"C:\images\one.png")]
        );
    }

    #[test]
    fn parse_open_file_name_buffer_expands_multi_select_files() {
        let buffer = wide_buffer(r"C:\images", &["one.png", "two.png"]);

        assert_eq!(
            parse_open_file_name_buffer(&buffer),
            vec![
                std::path::PathBuf::from(r"C:\images").join("one.png"),
                std::path::PathBuf::from(r"C:\images").join("two.png")
            ]
        );
    }

    #[test]
    fn parse_open_file_name_buffer_treats_cancelled_dialog_as_empty() {
        assert!(parse_open_file_name_buffer(&[0, 0, 0]).is_empty());
    }

    #[test]
    fn profile_target_file_to_open_accepts_existing_png_target() {
        let tmp = tempfile::tempdir().unwrap();
        let image = tmp.path().join("target.PNG");
        let profile = tmp.path().join("profile.json");
        fs::write(&image, b"not-decoded-here").unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [{"name": "one", "path": image}]
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            profile_target_file_to_open(&profile, 0).unwrap(),
            image.canonicalize().unwrap()
        );
    }

    #[test]
    fn profile_target_thumbnail_returns_png_data_url_for_existing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let profiles = data_dir.join("profiles");
        let image = data_dir.join("templates").join("target.png");
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        fs::create_dir_all(&profiles).unwrap();
        fs::write(&image, &[0x89, b'P', b'N', b'G']).unwrap();
        fs::write(
            profiles.join("profile_1.json"),
            serde_json::to_string_pretty(&json!({
                "targets": [{"name": "one", "path": image}]
            }))
            .unwrap(),
        )
        .unwrap();

        let result = profile_target_thumbnail_at(&data_dir, 1, 0).unwrap();

        assert!(result.data_url.starts_with("data:image/png;base64,"));
        assert_eq!(
            result.path,
            image.canonicalize().unwrap().display().to_string()
        );
    }

    #[test]
    fn profile_target_file_to_open_rejects_unavailable_index() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile.json");
        fs::write(&profile, r#"{"targets":[]}"#).unwrap();

        let err = profile_target_file_to_open(&profile, 1).unwrap_err();

        assert!(err.contains("index 1"));
    }

    #[test]
    fn profile_target_file_to_open_rejects_non_png_files() {
        let tmp = tempfile::tempdir().unwrap();
        let image = tmp.path().join("target.txt");
        let profile = tmp.path().join("profile.json");
        fs::write(&image, b"text").unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "targets": [{"name": "one", "path": image}]
            }))
            .unwrap(),
        )
        .unwrap();

        let err = profile_target_file_to_open(&profile, 0).unwrap_err();

        assert!(err.contains("PNG"));
    }

    #[test]
    fn profile_gallery_edit_workflow_preserves_file_boundaries() {
        fn solid_frame(rgb: [u8; 3]) -> RgbFrame {
            let mut pixels = Vec::new();
            for _ in 0..4 {
                pixels.extend_from_slice(&rgb);
            }
            RgbFrame::new(2, 2, pixels).unwrap()
        }

        fn target_ids(targets: &[serde_json::Value]) -> Vec<String> {
            targets
                .iter()
                .map(|target| target["id"].as_str().unwrap().to_string())
                .collect()
        }

        fn target_path(target: &serde_json::Value) -> std::path::PathBuf {
            std::path::PathBuf::from(target["path"].as_str().unwrap())
        }

        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let profile = profile_path(&data_dir, 1);
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::write(
            &profile,
            serde_json::to_string_pretty(&json!({
                "future": true,
                "targets": []
            }))
            .unwrap(),
        )
        .unwrap();
        let frames = [
            solid_frame([240, 20, 20]),
            solid_frame([20, 200, 60]),
            solid_frame([20, 80, 240]),
        ];

        let imported = add_profile_template_frames_at(&profile, &data_dir, 1, &frames, 10).unwrap();

        assert!(imported.changed);
        assert_eq!(imported.added_count, 3);
        assert_eq!(imported.pruned_count, 0);
        assert_eq!(imported.selected_index, Some(2));
        assert_eq!(imported.targets.len(), 3);
        let template_root = templates_dir(&data_dir).canonicalize().unwrap();
        for target in &imported.targets {
            let path = target_path(target);
            assert!(path.exists());
            assert!(path.canonicalize().unwrap().starts_with(&template_root));
            assert_eq!(target["enabled"], json!(true));
        }

        let disabled = set_profile_target_enabled_at(&profile, 1, false).unwrap();

        assert!(disabled.changed);
        assert_eq!(disabled.enabled_count, 2);
        assert!(!disabled.all_enabled);
        assert_eq!(disabled.targets[1]["enabled"], json!(false));

        let selected_all = toggle_all_profile_targets_at(&profile).unwrap();

        assert!(selected_all.changed);
        assert_eq!(selected_all.enabled_count, 3);
        assert!(selected_all.all_enabled);

        let cleared_all = toggle_all_profile_targets_at(&profile).unwrap();

        assert!(cleared_all.changed);
        assert_eq!(cleared_all.enabled_count, 0);
        assert!(!cleared_all.all_enabled);
        assert!(cleared_all
            .targets
            .iter()
            .all(|target| target["enabled"] == json!(false)));

        let before_reorder = read_profile_at(&profile).unwrap();
        let before_reorder_ids = target_ids(&before_reorder.targets);

        let reordered = reorder_profile_target_at(&profile, &data_dir, 1, 0, 3).unwrap();

        assert!(reordered.changed);
        assert_eq!(reordered.deleted_files, 0);
        assert_eq!(reordered.selected_index, Some(2));
        let reordered_ids = target_ids(&reordered.targets);
        assert_eq!(
            reordered_ids,
            vec![
                before_reorder_ids[1].clone(),
                before_reorder_ids[2].clone(),
                before_reorder_ids[0].clone()
            ]
        );
        for target in &reordered.targets {
            let path = target_path(target);
            assert!(path.exists());
            assert!(path.canonicalize().unwrap().starts_with(&template_root));
        }

        let removed_target = reordered.targets[1].clone();
        let removed_id = removed_target["id"].as_str().unwrap().to_string();
        let removed_path = target_path(&removed_target);

        let removed = remove_profile_target_at(&profile, &data_dir, 1, 1).unwrap();

        assert!(removed.changed);
        assert_eq!(removed.deleted_files, 1);
        assert_eq!(removed.selected_index, None);
        assert_eq!(removed.targets.len(), 2);
        assert!(!removed_path.exists());
        assert!(!target_ids(&removed.targets).contains(&removed_id));

        let external = tmp.path().join("external.png");
        fs::write(&external, b"external image placeholder").unwrap();
        let mut stored: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        stored["targets"].as_array_mut().unwrap().push(json!({
            "id": "external",
            "name": "external",
            "path": external,
            "enabled": true
        }));
        fs::write(&profile, serde_json::to_string_pretty(&stored).unwrap()).unwrap();
        let before_clear = read_profile_at(&profile).unwrap();
        let template_paths_before_clear = before_clear
            .targets
            .iter()
            .map(target_path)
            .filter(|path| path.canonicalize().unwrap().starts_with(&template_root))
            .collect::<Vec<_>>();
        assert_eq!(template_paths_before_clear.len(), 2);

        let cleared = clear_profile_targets_at(&profile, &data_dir).unwrap();

        assert!(cleared.changed);
        assert_eq!(cleared.deleted_files, template_paths_before_clear.len());
        assert!(cleared.targets.is_empty());
        for path in template_paths_before_clear {
            assert!(!path.exists());
        }
        assert!(external.exists());
        let stored: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&profile).unwrap()).unwrap();
        assert_eq!(stored["future"], json!(true));
        assert!(stored["targets"].as_array().unwrap().is_empty());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn one_shot_scan_captures_screen_region_and_writes_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "cooldown_seconds": 3,
              "targets": [
                {"kind":"pixel","id":"desktop-pixel","name":"desktop-pixel","x":0,"y":0,"rgb":[0,0,0],"tolerance":255}
              ],
              "alarm": {"beep": false, "save_dir": "screenshots", "jsonl": "alerts.jsonl", "max_alerts": 2}
            }"#,
        )
        .unwrap();
        let regions = [ResolvedRegion {
            name: "desktop".to_string(),
            monitor: 1,
            bbox: BBox {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            },
        }];
        let clock = ScanClock {
            now_seconds: 1.0,
            time_text: "manual".to_string(),
            stamp: "manual".to_string(),
        };
        let results =
            scan_resolved_screen_regions_once(config, tmp.path(), &regions, &clock).unwrap();
        assert_eq!(results[0].matches[0].target_id, "desktop-pixel");
        assert_eq!(results[0].alerted_matches.len(), 1);
        assert!(tmp.path().join("alerts.jsonl").exists());
        assert_eq!(
            fs::read_dir(tmp.path().join("screenshots"))
                .unwrap()
                .count(),
            1
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn profile_screen_scan_workflow_records_template_hit_on_windows_desktop() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let template_source = tmp.path().join("profile-source.png");
        let captured = crate::screen_capture::capture_screen_region(CaptureRegion {
            left: 0,
            top: 0,
            width: 32,
            height: 32,
        })
        .unwrap();
        let template = crop_rgb_frame(&captured, 4, 4, 12, 12);
        save_rgb_png(&template_source, &template, &[]).unwrap();

        let profile = profile_path(&data_dir, 1);
        let added = add_profile_template_pngs_at(
            &profile,
            &data_dir,
            1,
            std::slice::from_ref(&template_source),
            5,
        )
        .unwrap();
        assert_eq!(added.added_count, 1);

        let options = ProfileWatchConfigOptions {
            regions: vec![RegionConfig {
                name: "profile-screen-smoke".to_string(),
                monitor: 1,
                left: 0,
                top: 0,
                width: Some(32),
                height: Some(32),
                extra: Default::default(),
            }],
            threshold: 0.99,
            cooldown_seconds: 0.0,
            max_alerts: Some(2),
            beep: false,
            ..ProfileWatchConfigOptions::default()
        };
        let config = profile_watch_config_at(&profile, &data_dir, options).unwrap();
        let regions = [ResolvedRegion {
            name: "profile-screen-smoke".to_string(),
            monitor: 1,
            bbox: BBox {
                left: 0,
                top: 0,
                width: 32,
                height: 32,
            },
        }];
        let clock = ScanClock {
            now_seconds: 1.0,
            time_text: "profile-smoke".to_string(),
            stamp: "profile-smoke".to_string(),
        };

        let results =
            scan_resolved_screen_regions_once(config, &data_dir, &regions, &clock).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].alerted_matches.is_empty());
        let target_ids = results[0]
            .alerted_matches
            .iter()
            .map(|item| item.target_id.clone())
            .collect::<Vec<_>>();
        assert!(record_profile_hits_at(&profile, &target_ids).unwrap());

        let stored = read_profile_at(&profile).unwrap();
        assert!(stored.exists);
        assert_eq!(stored.targets[0]["hit_count"], json!(1));
        assert!(data_dir.join("alerts.jsonl").exists());
        assert_eq!(
            fs::read_dir(data_dir.join("screenshots")).unwrap().count(),
            1
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn profile_screen_capture_template_writes_real_desktop_frame_on_windows_desktop() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let profile = profile_path(&data_dir, 1);
        let sources = ProfileCaptureSources {
            regions: vec![ResolvedRegion {
                name: "capture-template-smoke".to_string(),
                monitor: 1,
                bbox: BBox {
                    left: 0,
                    top: 0,
                    width: 32,
                    height: 32,
                },
            }],
            windows: Vec::new(),
            skipped_windows: 0,
            skipped_window_apps: 0,
        };

        let frame = capture_profile_source_frame_from_sources(&sources).unwrap();
        assert_eq!(frame.width, 32);
        assert_eq!(frame.height, 32);

        let added =
            add_profile_template_frames_at(&profile, &data_dir, 1, std::slice::from_ref(&frame), 5)
                .unwrap();
        assert!(added.changed);
        assert_eq!(added.added_count, 1);
        assert_eq!(added.pruned_count, 0);
        assert_eq!(added.selected_index, Some(0));

        let stored = read_profile_at(&profile).unwrap();
        assert!(stored.exists);
        assert_eq!(stored.targets.len(), 1);
        let target_path = std::path::PathBuf::from(stored.targets[0]["path"].as_str().unwrap());
        assert!(target_path.starts_with(templates_dir(&data_dir)));
        let saved = RgbFrame::from_image_path(&target_path).unwrap();
        assert_eq!(saved.width, 32);
        assert_eq!(saved.height, 32);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn profile_window_capture_template_writes_real_window_frame_on_windows_desktop() {
        let smoke_window = TestWindow::new(
            windows::core::w!("Screen Watch OCR profile window capture template smoke"),
            160,
            160,
            360,
            240,
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let profile = profile_path(&data_dir, 1);
        let sources = ProfileCaptureSources {
            regions: Vec::new(),
            windows: vec![WindowConfig {
                name: "capture-window-template-smoke".to_string(),
                title: "Screen Watch OCR profile window capture template smoke".to_string(),
                display: "Screen Watch OCR profile window capture template smoke".to_string(),
                hwnd: Some(smoke_window.hwnd()),
                extra: Default::default(),
            }],
            skipped_windows: 0,
            skipped_window_apps: 0,
        };

        let frame = capture_profile_source_frame_from_sources(&sources).unwrap();
        assert!(frame.width > 0);
        assert!(frame.height > 0);

        let added =
            add_profile_template_frames_at(&profile, &data_dir, 1, std::slice::from_ref(&frame), 5)
                .unwrap();
        assert!(added.changed);
        assert_eq!(added.added_count, 1);
        assert_eq!(added.pruned_count, 0);
        assert_eq!(added.selected_index, Some(0));

        let stored = read_profile_at(&profile).unwrap();
        assert!(stored.exists);
        assert_eq!(stored.targets.len(), 1);
        let target_path = std::path::PathBuf::from(stored.targets[0]["path"].as_str().unwrap());
        assert!(target_path.starts_with(templates_dir(&data_dir)));
        let saved = RgbFrame::from_image_path(&target_path).unwrap();
        assert_eq!(saved.width, frame.width);
        assert_eq!(saved.height, frame.height);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn profile_remembered_window_capture_template_resolves_and_writes_frame_on_windows_desktop() {
        const TITLE: &str = "000 Screen Watch OCR remembered app capture template smoke";
        let _form = TestFormProcess::new(TITLE).unwrap();
        let (remembered_window, available_windows) = wait_for_listed_app_window(TITLE).unwrap();
        let options = ProfileWatchConfigOptions {
            window_apps: vec![screen_watch_core::config::WindowAppConfig {
                title: TITLE.to_string(),
                ordinal: remembered_window.ordinal,
                extra: Default::default(),
            }],
            ..ProfileWatchConfigOptions::default()
        };

        let sources = profile_capture_sources_for_monitors(&options, &[], &available_windows)
            .expect("remembered app window should resolve to a concrete source");
        assert!(sources.regions.is_empty());
        assert_eq!(sources.windows.len(), 1);
        assert_eq!(sources.windows[0].hwnd, Some(remembered_window.hwnd));
        assert_eq!(sources.skipped_window_apps, 0);

        let frame = capture_profile_source_frame_from_sources(&sources).unwrap();
        assert!(frame.width > 0);
        assert!(frame.height > 0);

        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let profile = profile_path(&data_dir, 1);
        let added =
            add_profile_template_frames_at(&profile, &data_dir, 1, std::slice::from_ref(&frame), 5)
                .unwrap();
        assert!(added.changed);
        assert_eq!(added.added_count, 1);
        assert_eq!(added.selected_index, Some(0));

        let stored = read_profile_at(&profile).unwrap();
        assert!(stored.exists);
        let target_path = std::path::PathBuf::from(stored.targets[0]["path"].as_str().unwrap());
        assert!(target_path.starts_with(templates_dir(&data_dir)));
        let saved = RgbFrame::from_image_path(&target_path).unwrap();
        assert_eq!(saved.width, frame.width);
        assert_eq!(saved.height, frame.height);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn profile_monitoring_session_records_template_hit_on_windows_desktop() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let template_source = tmp.path().join("profile-monitor-source.png");
        let captured = crate::screen_capture::capture_screen_region(CaptureRegion {
            left: 0,
            top: 0,
            width: 32,
            height: 32,
        })
        .unwrap();
        let template = crop_rgb_frame(&captured, 4, 4, 12, 12);
        save_rgb_png(&template_source, &template, &[]).unwrap();

        let profile = profile_path(&data_dir, 1);
        let added = add_profile_template_pngs_at(
            &profile,
            &data_dir,
            1,
            std::slice::from_ref(&template_source),
            5,
        )
        .unwrap();
        assert_eq!(added.added_count, 1);

        let options = ProfileWatchConfigOptions {
            regions: vec![RegionConfig {
                name: "profile-monitor-smoke".to_string(),
                monitor: 1,
                left: 0,
                top: 0,
                width: Some(32),
                height: Some(32),
                extra: Default::default(),
            }],
            threshold: 0.99,
            cooldown_seconds: 0.0,
            poll_interval_seconds: 0.12,
            max_alerts: Some(3),
            beep: false,
            ..ProfileWatchConfigOptions::default()
        };
        let config = profile_watch_config_at(&profile, &data_dir, options).unwrap();
        let session = MonitorSessionState::default();
        let started = session
            .start_sources_with_events(
                config,
                data_dir.clone(),
                vec![ResolvedRegion {
                    name: "profile-monitor-smoke".to_string(),
                    monitor: 1,
                    bbox: BBox {
                        left: 0,
                        top: 0,
                        width: 32,
                        height: 32,
                    },
                }],
                Vec::new(),
                Vec::new(),
                AlarmBeepState::default(),
                Some(Arc::new(ProfileHitSink {
                    path: profile.clone(),
                })),
                Arc::new(TestEventSink),
            )
            .unwrap();
        assert!(started.running);

        for _ in 0..30 {
            if session.snapshot().unwrap().hit_count > 0 {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }

        let stopped = session.stop().unwrap();
        assert!(!stopped.running);
        assert!(stopped.tick_count > 0);
        assert!(stopped.hit_count > 0);

        let stored = read_profile_at(&profile).unwrap();
        assert!(stored.exists);
        assert!(stored.targets[0]["hit_count"].as_u64().unwrap_or(0) > 0);
        assert!(data_dir.join("alerts.jsonl").exists());
        assert!(fs::read_dir(data_dir.join("screenshots"))
            .unwrap()
            .next()
            .is_some());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn profile_window_scan_workflow_records_template_hit_on_windows_desktop() {
        let smoke_window = TestWindow::new(
            windows::core::w!("Screen Watch OCR profile window smoke"),
            120,
            120,
            360,
            240,
        )
        .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("ScreenWatchOCR");
        let template_source = tmp.path().join("profile-window-source.png");
        let mut capture_modes = crate::window_capture::WindowCaptureModeCache::default();
        let captured = crate::window_capture::capture_window_frame(
            smoke_window.hwnd(),
            Some(&mut capture_modes),
        )
        .unwrap()
        .expect("profile window smoke capture should return a frame");
        assert!(captured.width >= 32);
        assert!(captured.height >= 32);
        let left = captured.width.saturating_sub(12) / 2;
        let top = captured.height.saturating_sub(12) / 2;
        let template = crop_rgb_frame(&captured, left, top, 12, 12);
        save_rgb_png(&template_source, &template, &[]).unwrap();

        let profile = profile_path(&data_dir, 1);
        let added = add_profile_template_pngs_at(
            &profile,
            &data_dir,
            1,
            std::slice::from_ref(&template_source),
            5,
        )
        .unwrap();
        assert_eq!(added.added_count, 1);

        let window = WindowConfig {
            name: "profile-window-smoke".to_string(),
            title: "Screen Watch OCR profile window smoke".to_string(),
            display: "Screen Watch OCR profile window smoke".to_string(),
            hwnd: Some(smoke_window.hwnd()),
            extra: Default::default(),
        };
        let options = ProfileWatchConfigOptions {
            windows: vec![window.clone()],
            threshold: 0.9,
            cooldown_seconds: 0.0,
            max_alerts: Some(2),
            beep: false,
            ..ProfileWatchConfigOptions::default()
        };
        let config = profile_watch_config_at(&profile, &data_dir, options).unwrap();
        assert!(config.regions.is_empty());
        assert_eq!(config.windows.len(), 1);
        let clock = ScanClock {
            now_seconds: 1.0,
            time_text: "profile-window-smoke".to_string(),
            stamp: "profile-window-smoke".to_string(),
        };

        let (region_results, window_results) =
            super::scan_resolved_sources_once(config, &data_dir, &[], &[window], &clock).unwrap();

        assert!(region_results.is_empty());
        assert_eq!(window_results.len(), 1);
        assert!(!window_results[0].alerted_matches.is_empty());
        let target_ids = window_results[0]
            .alerted_matches
            .iter()
            .map(|item| item.target_id.clone())
            .collect::<Vec<_>>();
        assert!(record_profile_hits_at(&profile, &target_ids).unwrap());

        let stored = read_profile_at(&profile).unwrap();
        assert!(stored.exists);
        assert_eq!(stored.targets[0]["hit_count"], json!(1));
        assert!(data_dir.join("alerts.jsonl").exists());
        assert!(fs::read_dir(data_dir.join("screenshots"))
            .unwrap()
            .next()
            .is_some());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop with at least one selectable window"]
    fn one_shot_scan_captures_window_and_writes_evidence() {
        let Some(window) = crate::window_sources::list_app_windows()
            .unwrap()
            .into_iter()
            .next()
        else {
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let config = WatchConfig::from_json_str(
            r#"{
              "cooldown_seconds": 0,
              "targets": [
                {"kind":"pixel","id":"window-pixel","name":"window-pixel","x":0,"y":0,"rgb":[0,0,0],"tolerance":255}
              ],
              "alarm": {"beep": false, "save_dir": "screenshots", "jsonl": "alerts.jsonl", "max_alerts": 2}
            }"#,
        )
        .unwrap();
        let windows = [WindowConfig {
            name: "app-window".to_string(),
            title: window.title,
            display: window.display,
            hwnd: Some(window.hwnd),
            extra: Default::default(),
        }];
        let clock = ScanClock {
            now_seconds: 1.0,
            time_text: "manual".to_string(),
            stamp: "manual".to_string(),
        };

        let (region_results, window_results) =
            super::scan_resolved_sources_once(config, tmp.path(), &[], &windows, &clock).unwrap();

        assert!(region_results.is_empty());
        assert_eq!(window_results.len(), 1);
        assert_eq!(window_results[0].matches[0].target_id, "window-pixel");
        assert_eq!(window_results[0].alerted_matches.len(), 1);
        assert!(tmp.path().join("alerts.jsonl").exists());
    }

    fn monitor(index: i32, left: i32, top: i32, width: u32, height: u32) -> ListedMonitor {
        ListedMonitor {
            index,
            left,
            top,
            width,
            height,
            name: None,
            scale_factor: 1.0,
            is_virtual: false,
        }
    }

    fn scan_result(alerted_target_id: &str) -> ScanFrameResult {
        ScanFrameResult {
            region: "source".to_string(),
            matches: vec![scan_match("raw-only")],
            alerted_matches: vec![scan_match(alerted_target_id)],
            alert: None,
        }
    }

    fn scan_match(target_id: &str) -> Match {
        Match {
            target: target_id.to_string(),
            target_id: target_id.to_string(),
            kind: "template".to_string(),
            score: 1.0,
            box_xyxy: [0, 0, 1, 1],
            scale: None,
            text: None,
        }
    }

    fn crop_rgb_frame(frame: &RgbFrame, left: u32, top: u32, width: u32, height: u32) -> RgbFrame {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
        for y in top..top + height {
            let start = ((y * frame.width + left) * 3) as usize;
            let end = start + width as usize * 3;
            pixels.extend_from_slice(&frame.pixels[start..end]);
        }
        RgbFrame::new(width, height, pixels).unwrap()
    }

    fn wide_buffer(first: &str, rest: &[&str]) -> Vec<u16> {
        let mut values = Vec::new();
        for part in std::iter::once(first).chain(rest.iter().copied()) {
            values.extend(part.encode_utf16());
            values.push(0);
        }
        values.push(0);
        values
    }

    #[cfg(windows)]
    struct TestWindow {
        hwnd: windows::Win32::Foundation::HWND,
    }

    #[cfg(windows)]
    impl TestWindow {
        fn new(
            title: windows::core::PCWSTR,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        ) -> Result<Self, String> {
            use windows::Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, ShowWindow, SW_SHOW, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
                WS_VISIBLE,
            };

            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    windows::core::w!("STATIC"),
                    title,
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    x,
                    y,
                    width,
                    height,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .map_err(|err| format!("failed to create profile window smoke window: {err}"))?;
            if hwnd.0.is_null() {
                return Err("failed to create profile window smoke window".to_string());
            }
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            Ok(Self { hwnd })
        }

        fn hwnd(&self) -> isize {
            self.hwnd.0 as isize
        }
    }

    #[cfg(windows)]
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
            }
        }
    }

    #[cfg(windows)]
    struct TestFormProcess {
        child: std::process::Child,
    }

    #[cfg(windows)]
    impl TestFormProcess {
        fn new(title: &str) -> Result<Self, String> {
            let escaped_title = title.replace('\'', "''");
            let script = format!(
                "$ErrorActionPreference='Stop'; \
                 Add-Type -AssemblyName System.Windows.Forms; \
                 $form = New-Object System.Windows.Forms.Form; \
                 $form.Text = '{escaped_title}'; \
                 $form.Width = 360; $form.Height = 240; \
                 $form.StartPosition = 'Manual'; $form.Left = 180; $form.Top = 180; \
                 $form.TopMost = $true; \
                 $timer = New-Object System.Windows.Forms.Timer; \
                 $timer.Interval = 15000; \
                 $timer.Add_Tick({{ $form.Close() }}); \
                 $timer.Start(); \
                 [System.Windows.Forms.Application]::Run($form);"
            );
            let child = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-STA",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &script,
                ])
                .spawn()
                .map_err(|err| format!("failed to start remembered app smoke window: {err}"))?;
            Ok(Self { child })
        }
    }

    #[cfg(windows)]
    impl Drop for TestFormProcess {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[cfg(windows)]
    fn wait_for_listed_app_window(title: &str) -> Result<(AppWindow, Vec<AppWindow>), String> {
        for _ in 0..50 {
            let windows = crate::window_sources::list_app_windows()?;
            if let Some(window) = windows.iter().find(|window| window.title == title) {
                return Ok((window.clone(), windows));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Err(format!(
            "remembered app smoke window {title:?} was not listed"
        ))
    }
}
