use std::{
    env,
    io::Cursor,
    sync::atomic::{AtomicBool, Ordering},
    sync::OnceLock,
    thread,
    time::Duration,
};

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuEvent},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime, Window, WindowEvent,
};

pub const TRAY_ID: &str = "screen-watch-ocr-tauri-main";
pub const TRAY_MENU_SHOW_ID: &str = "screen-watch-ocr-tauri-show";
pub const TRAY_MENU_EXIT_ID: &str = "screen-watch-ocr-tauri-exit";
pub const TRAY_MENU_SHOW_LABEL: &str = "Show Tauri";
pub const TRAY_MENU_EXIT_LABEL: &str = "Exit Tauri";
pub const START_MINIMIZED_ARG: &str = "--start-minimized";

const ICON_SIZE: u32 = 48;
const TRAY_SOURCE_SIZE: u32 = 128;
const TRAY_SOURCE_CROP_SIZE: u32 = 96;
const WINDOW_ICON_SIZE: u32 = 256;
const APP_ICON_ICO: &[u8] = include_bytes!("../icons/icon.ico");
const MONITORING_GREEN: [u8; 3] = [34, 197, 94];
const WHITE_GLYPH_CHANNEL_FLOOR: u8 = 180;

static BASE_TRAY_ICON_RGBA: OnceLock<Vec<u8>> = OnceLock::new();
static WINDOW_ICON_RGBA: OnceLock<Vec<u8>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    ShowMainWindow,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconAction {
    ShowMainWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMonitoringPresentation {
    pub tooltip: &'static str,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl TrayMonitoringPresentation {
    fn icon_image(&self) -> Image<'static> {
        Image::new_owned(self.rgba.clone(), self.width, self.height)
    }
}

#[derive(Debug, Default)]
pub struct TrayLifecycleState {
    available: AtomicBool,
}

impl TrayLifecycleState {
    pub fn set_available(&self, available: bool) {
        self.available.store(available, Ordering::SeqCst);
    }

    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::SeqCst)
    }
}

pub fn install_tray<R, M>(manager: &M) -> tauri::Result<()>
where
    R: Runtime,
    M: Manager<R>,
{
    let menu = MenuBuilder::new(manager)
        .text(TRAY_MENU_SHOW_ID, TRAY_MENU_SHOW_LABEL)
        .separator()
        .text(TRAY_MENU_EXIT_ID, TRAY_MENU_EXIT_LABEL)
        .build()?;
    let presentation = tray_monitoring_presentation(false);
    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(presentation.tooltip)
        .icon(presentation.icon_image())
        .show_menu_on_left_click(false)
        .build(manager)?;
    Ok(())
}

pub fn start_minimized_from_env_args() -> bool {
    has_start_minimized_arg(env::args())
}

pub fn has_start_minimized_arg<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .any(|arg| arg.as_ref() == START_MINIMIZED_ARG)
}

pub fn should_hide_on_start(start_minimized: bool, tray_available: bool) -> bool {
    start_minimized && tray_available
}

pub fn should_hide_close_to_tray(tray_available: bool) -> bool {
    tray_available
}

pub fn apply_startup_visibility(app: &AppHandle, start_minimized: bool, tray_available: bool) {
    let hide_on_start = should_hide_on_start(start_minimized, tray_available);
    apply_startup_visibility_once(app, hide_on_start);

    let app_for_thread = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        let app_for_main = app_for_thread.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            apply_startup_visibility_once(&app_for_main, hide_on_start);
        });
    });
}

fn apply_startup_visibility_once(app: &AppHandle, hide_on_start: bool) {
    if hide_on_start {
        hide_main_window(app);
    } else {
        show_main_window(app);
    }
}

pub fn handle_window_event(window: &Window, event: &WindowEvent, state: &TrayLifecycleState) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    if window.label() != "main" || !should_hide_close_to_tray(state.is_available()) {
        return;
    }
    api.prevent_close();
    let _ = window.hide();
}

pub fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match menu_action_for_id(event.id().as_ref()) {
        Some(TrayMenuAction::ShowMainWindow) => show_main_window(app),
        Some(TrayMenuAction::Exit) => app.exit(0),
        None => {}
    }
}

pub fn handle_tray_icon_event(app: &AppHandle, event: TrayIconEvent) {
    let action = match event {
        TrayIconEvent::DoubleClick { .. } => Some(TrayIconAction::ShowMainWindow),
        TrayIconEvent::Click {
            button,
            button_state,
            ..
        } => tray_icon_click_action(button, button_state),
        _ => None,
    };
    if matches!(action, Some(TrayIconAction::ShowMainWindow)) {
        show_main_window(app);
    }
}

pub fn menu_action_for_id(id: &str) -> Option<TrayMenuAction> {
    match id {
        TRAY_MENU_SHOW_ID => Some(TrayMenuAction::ShowMainWindow),
        TRAY_MENU_EXIT_ID => Some(TrayMenuAction::Exit),
        _ => None,
    }
}

pub fn tray_icon_click_action(
    button: MouseButton,
    button_state: MouseButtonState,
) -> Option<TrayIconAction> {
    if button == MouseButton::Left && button_state == MouseButtonState::Up {
        Some(TrayIconAction::ShowMainWindow)
    } else {
        None
    }
}

pub fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn update_monitoring_status(app: &AppHandle, running: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let presentation = tray_monitoring_presentation(running);
    let _ = tray.set_tooltip(Some(presentation.tooltip));
    let _ = tray.set_icon(Some(presentation.icon_image()));
}

pub fn tray_tooltip(monitoring: bool) -> &'static str {
    if monitoring {
        "Screen Watch OCR Tauri - Monitoring"
    } else {
        "Screen Watch OCR Tauri - Ready"
    }
}

pub fn tray_monitoring_presentation(monitoring: bool) -> TrayMonitoringPresentation {
    TrayMonitoringPresentation {
        tooltip: tray_tooltip(monitoring),
        rgba: tray_icon_rgba(monitoring),
        width: ICON_SIZE,
        height: ICON_SIZE,
    }
}

pub fn tray_icon_rgba(monitoring: bool) -> Vec<u8> {
    let mut rgba = base_tray_icon_rgba().to_vec();
    if !monitoring {
        return rgba;
    }

    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] > 0
            && pixel[0] >= WHITE_GLYPH_CHANNEL_FLOOR
            && pixel[1] >= WHITE_GLYPH_CHANNEL_FLOOR
            && pixel[2] >= WHITE_GLYPH_CHANNEL_FLOOR
        {
            pixel[..3].copy_from_slice(&MONITORING_GREEN);
        }
    }
    rgba
}

pub fn app_window_icon_image() -> Image<'static> {
    Image::new_owned(
        window_icon_rgba().to_vec(),
        WINDOW_ICON_SIZE,
        WINDOW_ICON_SIZE,
    )
}

fn base_tray_icon_rgba() -> &'static [u8] {
    BASE_TRAY_ICON_RGBA
        .get_or_init(|| {
            let source = decode_ico_png_layer(APP_ICON_ICO, TRAY_SOURCE_SIZE, TRAY_SOURCE_SIZE)
                .expect("embedded application icon must contain a valid 128x128 PNG layer");
            zoom_tray_icon_rgba(source).expect("tray icon crop and resize must remain valid")
        })
        .as_slice()
}

fn window_icon_rgba() -> &'static [u8] {
    WINDOW_ICON_RGBA
        .get_or_init(|| {
            decode_ico_png_layer(APP_ICON_ICO, WINDOW_ICON_SIZE, WINDOW_ICON_SIZE)
                .expect("embedded application icon must contain a valid 256x256 PNG layer")
        })
        .as_slice()
}

fn zoom_tray_icon_rgba(source: Vec<u8>) -> Result<Vec<u8>, String> {
    let source = image::RgbaImage::from_raw(TRAY_SOURCE_SIZE, TRAY_SOURCE_SIZE, source)
        .ok_or_else(|| "tray source icon has an unexpected RGBA length".to_string())?;
    let crop_offset = (TRAY_SOURCE_SIZE - TRAY_SOURCE_CROP_SIZE) / 2;
    let cropped = image::imageops::crop_imm(
        &source,
        crop_offset,
        crop_offset,
        TRAY_SOURCE_CROP_SIZE,
        TRAY_SOURCE_CROP_SIZE,
    )
    .to_image();
    Ok(image::imageops::resize(
        &cropped,
        ICON_SIZE,
        ICON_SIZE,
        image::imageops::FilterType::Lanczos3,
    )
    .into_raw())
}

fn decode_ico_png_layer(
    ico: &[u8],
    target_width: u32,
    target_height: u32,
) -> Result<Vec<u8>, String> {
    if ico.len() < 6 || read_u16_le(ico, 0)? != 0 || read_u16_le(ico, 2)? != 1 {
        return Err("invalid ICO header".to_string());
    }

    let entry_count = read_u16_le(ico, 4)? as usize;
    let mut selected: Option<(u16, usize, usize)> = None;
    for entry_index in 0..entry_count {
        let entry_offset = 6 + entry_index * 16;
        if entry_offset + 16 > ico.len() {
            return Err("truncated ICO directory".to_string());
        }
        let width = ico_dimension(ico[entry_offset]);
        let height = ico_dimension(ico[entry_offset + 1]);
        if width != target_width || height != target_height {
            continue;
        }

        let bit_count = read_u16_le(ico, entry_offset + 6)?;
        let image_size = read_u32_le(ico, entry_offset + 8)? as usize;
        let image_offset = read_u32_le(ico, entry_offset + 12)? as usize;
        let image_end = image_offset
            .checked_add(image_size)
            .ok_or_else(|| "ICO image range overflow".to_string())?;
        if image_end > ico.len() {
            return Err("ICO image range exceeds file size".to_string());
        }
        if !ico[image_offset..image_end].starts_with(b"\x89PNG\r\n\x1a\n") {
            continue;
        }
        if selected.is_none_or(|(selected_bits, _, _)| bit_count > selected_bits) {
            selected = Some((bit_count, image_offset, image_end));
        }
    }

    let (_, image_offset, image_end) = selected.ok_or_else(|| {
        format!("ICO does not contain a {target_width}x{target_height} PNG layer")
    })?;
    decode_png_rgba(&ico[image_offset..image_end], target_width, target_height)
}

fn decode_png_rgba(
    png_bytes: &[u8],
    expected_width: u32,
    expected_height: u32,
) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("failed to read PNG metadata: {error}"))?;
    let mut decoded = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut decoded)
        .map_err(|error| format!("failed to decode PNG: {error}"))?;
    if info.width != expected_width || info.height != expected_height {
        return Err(format!(
            "unexpected PNG dimensions: {}x{}",
            info.width, info.height
        ));
    }
    let decoded = &decoded[..info.buffer_size()];
    let pixel_count = (info.width * info.height) as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(decoded),
        png::ColorType::Rgb => {
            for pixel in decoded.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in decoded.chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for &value in decoded {
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        png::ColorType::Indexed => {
            return Err("PNG palette was not expanded during decoding".to_string());
        }
    }
    if rgba.len() != pixel_count * 4 {
        return Err("decoded PNG buffer has an unexpected length".to_string());
    }
    Ok(rgba)
}

fn ico_dimension(value: u8) -> u32 {
    if value == 0 {
        256
    } else {
        value as u32
    }
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| "truncated ICO integer".to_string())?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "truncated ICO integer".to_string())?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

#[cfg(test)]
mod tests {
    use super::{
        has_start_minimized_arg, menu_action_for_id, should_hide_close_to_tray,
        should_hide_on_start, tray_icon_click_action, tray_icon_rgba, tray_monitoring_presentation,
        tray_tooltip, window_icon_rgba, START_MINIMIZED_ARG, TRAY_MENU_EXIT_ID,
        TRAY_MENU_EXIT_LABEL, TRAY_MENU_SHOW_ID, TRAY_MENU_SHOW_LABEL,
    };
    use tauri::tray::{MouseButton, MouseButtonState};

    #[test]
    fn start_minimized_flag_matches_shared_startup_argument() {
        assert!(has_start_minimized_arg([
            "screen-watch-ocr-tauri.exe",
            START_MINIMIZED_ARG
        ]));
        assert!(!has_start_minimized_arg([
            "screen-watch-ocr-tauri.exe",
            "--start-hidden"
        ]));
    }

    #[test]
    fn start_minimized_only_hides_when_tray_is_available() {
        assert!(should_hide_on_start(true, true));
        assert!(!should_hide_on_start(true, false));
        assert!(!should_hide_on_start(false, true));
    }

    #[test]
    fn close_hides_to_tray_only_when_tray_is_available() {
        assert!(should_hide_close_to_tray(true));
        assert!(!should_hide_close_to_tray(false));
    }

    #[test]
    fn tray_menu_ids_route_to_show_and_exit_actions() {
        assert_eq!(
            menu_action_for_id(TRAY_MENU_SHOW_ID),
            Some(super::TrayMenuAction::ShowMainWindow)
        );
        assert_eq!(
            menu_action_for_id(TRAY_MENU_EXIT_ID),
            Some(super::TrayMenuAction::Exit)
        );
        assert_eq!(menu_action_for_id("unknown"), None);
        assert_eq!(TRAY_MENU_SHOW_LABEL, "Show Tauri");
        assert_eq!(TRAY_MENU_EXIT_LABEL, "Exit Tauri");
    }

    #[test]
    fn tray_left_click_up_routes_to_show_action_only() {
        assert_eq!(
            tray_icon_click_action(MouseButton::Left, MouseButtonState::Up),
            Some(super::TrayIconAction::ShowMainWindow)
        );
        assert_eq!(
            tray_icon_click_action(MouseButton::Left, MouseButtonState::Down),
            None
        );
        assert_eq!(
            tray_icon_click_action(MouseButton::Right, MouseButtonState::Up),
            None
        );
    }

    #[test]
    fn tooltip_reflects_monitoring_state() {
        assert_eq!(tray_tooltip(false), "Screen Watch OCR Tauri - Ready");
        assert_eq!(tray_tooltip(true), "Screen Watch OCR Tauri - Monitoring");
    }

    #[test]
    fn tray_icon_pixels_change_with_monitoring_state() {
        let ready = tray_icon_rgba(false);
        let monitoring = tray_icon_rgba(true);
        assert_eq!(ready.len(), 48 * 48 * 4);
        assert_eq!(monitoring.len(), ready.len());
        assert_ne!(ready, monitoring);
        assert!(ready.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(ready.chunks_exact(4).any(|pixel| pixel[3] == 255));
        assert!(monitoring
            .chunks_exact(4)
            .any(|pixel| pixel == [34, 197, 94, 255]));

        let changed_pixels: Vec<usize> = ready
            .chunks_exact(4)
            .zip(monitoring.chunks_exact(4))
            .enumerate()
            .filter_map(|(index, (ready_pixel, monitoring_pixel))| {
                (ready_pixel != monitoring_pixel).then_some(index)
            })
            .collect();
        assert!(changed_pixels.len() > 150);
        assert!(changed_pixels.iter().any(|index| index % 48 < 18));
        assert!(changed_pixels.iter().any(|index| index % 48 > 30));
        assert!(changed_pixels.iter().any(|index| index / 48 < 18));
        assert!(changed_pixels.iter().any(|index| index / 48 > 30));
        assert!(ready
            .chunks_exact(4)
            .zip(monitoring.chunks_exact(4))
            .all(|(ready_pixel, monitoring_pixel)| ready_pixel[3] == monitoring_pixel[3]));
    }

    #[test]
    fn monitoring_presentation_couples_tooltip_icon_and_dimensions() {
        let ready = tray_monitoring_presentation(false);
        let monitoring = tray_monitoring_presentation(true);

        assert_eq!(ready.tooltip, "Screen Watch OCR Tauri - Ready");
        assert_eq!(monitoring.tooltip, "Screen Watch OCR Tauri - Monitoring");
        assert_eq!((ready.width, ready.height), (48, 48));
        assert_eq!((monitoring.width, monitoring.height), (48, 48));
        assert_eq!(ready.rgba.len(), 48 * 48 * 4);
        assert_eq!(monitoring.rgba.len(), 48 * 48 * 4);
        assert_ne!(ready.rgba, monitoring.rgba);
    }

    #[test]
    fn taskbar_icon_uses_native_256_layer() {
        let rgba = window_icon_rgba();
        assert_eq!(rgba.len(), 256 * 256 * 4);
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }
}
