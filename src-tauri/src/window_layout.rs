use screen_watch_core::{
    data_dir::user_data_dir,
    profile::{read_window_geometry_at, save_window_geometry_at, WindowGeometry},
};
use std::{io, path::Path};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Window, WindowEvent};

const TASKBAR_PLACEHOLDER_COORD: i32 = -30_000;
const MIN_VISIBLE_WIDTH: u32 = 200;
const MIN_VISIBLE_HEIGHT: u32 = 120;

pub fn apply_saved_window_geometry(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    match read_window_geometry_at(user_data_dir()) {
        Ok(Some(geometry)) => {
            let _ = window.set_size(PhysicalSize::new(geometry.width, geometry.height));
            let _ = window.set_position(PhysicalPosition::new(geometry.x, geometry.y));
        }
        Ok(None) => {}
        Err(err) => eprintln!("saved window geometry ignored: {err}"),
    }
}

pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    if matches!(
        event,
        WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. }
    ) {
        if let Err(err) = save_current_window_geometry(window) {
            eprintln!("window geometry was not saved: {err}");
        }
    }
}

fn save_current_window_geometry(window: &Window) -> io::Result<bool> {
    let visible = window.is_visible().unwrap_or(true);
    let minimized = window.is_minimized().unwrap_or(false);
    let size = window
        .outer_size()
        .map_err(|err| io::Error::other(err.to_string()))?;
    let position = window
        .outer_position()
        .map_err(|err| io::Error::other(err.to_string()))?;
    let geometry = geometry_from_window_parts(size, position)?;
    persist_window_geometry_if_visible(
        user_data_dir(),
        window.label(),
        visible,
        minimized,
        geometry,
    )
}

fn geometry_from_window_parts(
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
) -> io::Result<WindowGeometry> {
    WindowGeometry::new(size.width, size.height, position.x, position.y)
}

fn persist_window_geometry_if_visible(
    data_dir: impl AsRef<Path>,
    label: &str,
    visible: bool,
    minimized: bool,
    geometry: WindowGeometry,
) -> io::Result<bool> {
    if !should_save_window_geometry(label, visible, minimized, geometry) {
        return Ok(false);
    }
    save_window_geometry_at(data_dir, geometry)?;
    Ok(true)
}

fn should_save_window_geometry(
    label: &str,
    visible: bool,
    minimized: bool,
    geometry: WindowGeometry,
) -> bool {
    label == "main"
        && visible
        && !minimized
        && geometry.width >= MIN_VISIBLE_WIDTH
        && geometry.height >= MIN_VISIBLE_HEIGHT
        && geometry.x > TASKBAR_PLACEHOLDER_COORD
        && geometry.y > TASKBAR_PLACEHOLDER_COORD
}

#[cfg(test)]
mod tests {
    use super::{persist_window_geometry_if_visible, should_save_window_geometry};
    use screen_watch_core::profile::{read_window_geometry_at, state_path, WindowGeometry};
    use serde_json::{json, Value};
    use std::fs;

    #[test]
    fn saves_visible_main_window_geometry_without_losing_state_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            state_path(&data),
            serde_json::to_string_pretty(&json!({
                "last_profile": 3,
                "layout": {
                    "geometry": "980x680+0+0",
                    "main_ratio": 0.42
                },
                "future": true
            }))
            .unwrap(),
        )
        .unwrap();

        let saved = persist_window_geometry_if_visible(
            &data,
            "main",
            true,
            false,
            WindowGeometry::new(1280, 760, 30, 40).unwrap(),
        )
        .unwrap();

        assert!(saved);
        let stored: Value =
            serde_json::from_str(&fs::read_to_string(state_path(&data)).unwrap()).unwrap();
        assert_eq!(stored["last_profile"], json!(3));
        assert_eq!(stored["layout"]["geometry"], json!("1280x760+30+40"));
        assert_eq!(stored["layout"]["main_ratio"], json!(0.42));
        assert_eq!(stored["future"], json!(true));
        assert_eq!(
            read_window_geometry_at(&data).unwrap(),
            Some(WindowGeometry::new(1280, 760, 30, 40).unwrap())
        );
    }

    #[test]
    fn does_not_save_hidden_minimized_or_non_main_window_geometry() {
        let geometry = WindowGeometry::new(1280, 760, 30, 40).unwrap();
        assert!(!should_save_window_geometry("main", false, false, geometry));
        assert!(!should_save_window_geometry("main", true, true, geometry));
        assert!(!should_save_window_geometry(
            "secondary",
            true,
            false,
            geometry
        ));
    }

    #[test]
    fn does_not_save_taskbar_placeholder_geometry() {
        assert!(!should_save_window_geometry(
            "main",
            true,
            false,
            WindowGeometry::new(160, 28, -32_000, -32_000).unwrap(),
        ));
    }

    #[test]
    fn reports_no_change_when_geometry_should_not_be_saved() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let saved = persist_window_geometry_if_visible(
            &data,
            "main",
            true,
            true,
            WindowGeometry::new(1280, 760, 30, 40).unwrap(),
        )
        .unwrap();

        assert!(!saved);
        assert!(!state_path(&data).exists());
    }
}
