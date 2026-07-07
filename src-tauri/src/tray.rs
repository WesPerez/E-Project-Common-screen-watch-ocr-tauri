use std::{
    env,
    sync::atomic::{AtomicBool, Ordering},
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

const ICON_SIZE: u32 = 32;

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
    let mut rgba = vec![0; (ICON_SIZE * ICON_SIZE * 4) as usize];
    let accent = if monitoring {
        [39, 174, 96, 255]
    } else {
        [78, 91, 110, 255]
    };
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            let idx = ((y * ICON_SIZE + x) * 4) as usize;
            if dx * dx + dy * dy <= 14 * 14 {
                let shade = if (8..=23).contains(&x) && (8..=23).contains(&y) {
                    accent
                } else {
                    [24, 30, 38, 255]
                };
                rgba[idx..idx + 4].copy_from_slice(&shade);
            }
            if monitoring {
                let dot_dx = x as i32 - 23;
                let dot_dy = y as i32 - 9;
                if dot_dx * dot_dx + dot_dy * dot_dy <= 5 * 5 {
                    rgba[idx..idx + 4].copy_from_slice(&[212, 255, 225, 255]);
                }
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::{
        has_start_minimized_arg, menu_action_for_id, should_hide_close_to_tray,
        should_hide_on_start, tray_icon_click_action, tray_icon_rgba, tray_monitoring_presentation,
        tray_tooltip, START_MINIMIZED_ARG, TRAY_MENU_EXIT_ID, TRAY_MENU_EXIT_LABEL,
        TRAY_MENU_SHOW_ID, TRAY_MENU_SHOW_LABEL,
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
        assert_eq!(ready.len(), 32 * 32 * 4);
        assert_eq!(monitoring.len(), ready.len());
        assert_ne!(ready, monitoring);
        assert!(ready.chunks_exact(4).any(|pixel| pixel[3] == 255));
        assert!(monitoring
            .chunks_exact(4)
            .any(|pixel| pixel == [212, 255, 225, 255]));
    }

    #[test]
    fn monitoring_presentation_couples_tooltip_icon_and_dimensions() {
        let ready = tray_monitoring_presentation(false);
        let monitoring = tray_monitoring_presentation(true);

        assert_eq!(ready.tooltip, "Screen Watch OCR Tauri - Ready");
        assert_eq!(monitoring.tooltip, "Screen Watch OCR Tauri - Monitoring");
        assert_eq!((ready.width, ready.height), (32, 32));
        assert_eq!((monitoring.width, monitoring.height), (32, 32));
        assert_eq!(ready.rgba.len(), 32 * 32 * 4);
        assert_eq!(monitoring.rgba.len(), 32 * 32 * 4);
        assert_ne!(ready.rgba, monitoring.rgba);
    }
}
