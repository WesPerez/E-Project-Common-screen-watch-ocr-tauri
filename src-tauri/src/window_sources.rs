use screen_watch_core::{
    config::{WindowAppConfig, WindowConfig},
    evidence::safe_name,
    profile::window_key,
};
use serde::Serialize;
use serde_json::Map;
use std::collections::{HashMap, HashSet};

pub const MAX_WINDOW_ROWS: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawWindowRecord {
    pub hwnd: isize,
    pub title: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppWindow {
    pub hwnd: isize,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub ordinal: u32,
    pub key: String,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowSourceResolution {
    pub available_windows: Vec<AppWindow>,
    pub windows: Vec<WindowConfig>,
    pub missing_window_apps: Vec<WindowAppConfig>,
}

pub fn window_display(title: &str, ordinal: u32, duplicate: bool) -> String {
    if duplicate {
        format!("{title} #{ordinal}")
    } else {
        title.to_string()
    }
}

pub fn decorate_window_records(records: Vec<RawWindowRecord>) -> Vec<AppWindow> {
    let mut title_totals = HashMap::<String, usize>::new();
    for record in &records {
        *title_totals.entry(record.title.clone()).or_default() += 1;
    }

    let mut records = records;
    records.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| left.hwnd.cmp(&right.hwnd))
    });

    let mut counts = HashMap::<String, u32>::new();
    let mut seen = HashSet::<(isize, String)>::new();
    let mut out = Vec::new();
    for record in records {
        let ordinal = counts
            .entry(record.title.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        let key = (record.hwnd, record.title.clone());
        if !seen.insert(key) {
            continue;
        }
        let duplicate = title_totals.get(&record.title).copied().unwrap_or_default() > 1;
        out.push(AppWindow {
            hwnd: record.hwnd,
            title: record.title.clone(),
            width: record.width,
            height: record.height,
            ordinal: *ordinal,
            key: window_key(&record.title, *ordinal),
            display: window_display(&record.title, *ordinal, duplicate),
        });
        if out.len() >= MAX_WINDOW_ROWS {
            break;
        }
    }
    out
}

pub fn resolve_window_apps(
    selected_apps: &[WindowAppConfig],
    available_windows: &[AppWindow],
) -> WindowSourceResolution {
    let by_key = available_windows
        .iter()
        .map(|window| (window.key.as_str(), window))
        .collect::<HashMap<_, _>>();
    let mut windows = Vec::new();
    let mut missing_window_apps = Vec::new();
    for app in selected_apps {
        let key = window_key(&app.title, app.ordinal);
        if let Some(window) = by_key.get(key.as_str()) {
            windows.push(WindowConfig {
                name: format!("app-{}", safe_name(&window.display)),
                title: window.title.clone(),
                display: window.display.clone(),
                hwnd: Some(window.hwnd),
                extra: Map::new(),
            });
        } else {
            missing_window_apps.push(app.clone());
        }
    }
    WindowSourceResolution {
        available_windows: available_windows.to_vec(),
        windows,
        missing_window_apps,
    }
}

#[cfg(windows)]
pub fn list_app_windows() -> Result<Vec<AppWindow>, String> {
    windows_impl::list_app_windows()
}

#[cfg(not(windows))]
pub fn list_app_windows() -> Result<Vec<AppWindow>, String> {
    Ok(Vec::new())
}

#[cfg(windows)]
mod windows_impl {
    use super::{decorate_window_records, AppWindow, RawWindowRecord};
    use std::{ffi::c_void, mem::size_of};
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{HWND, LPARAM, RECT},
            Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
            System::Threading::GetCurrentProcessId,
            UI::WindowsAndMessaging::{
                EnumWindows, GetClassNameW, GetWindow, GetWindowLongW, GetWindowRect,
                GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
                GWL_EXSTYLE, GW_OWNER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
            },
        },
    };

    const APP_TITLES: &[&str] = &[
        "ScreenWatchOCR",
        "Screen Watch OCR",
        "Screen Watch OCR Tauri",
        "Program Manager",
    ];
    const SKIPPED_CLASSES: &[&str] = &[
        "Windows.UI.Core.CoreWindow",
        "ApplicationFrameInputSinkWindow",
    ];

    pub fn list_app_windows() -> Result<Vec<AppWindow>, String> {
        let mut records = Vec::<RawWindowRecord>::new();
        unsafe {
            EnumWindows(
                Some(enum_window_proc),
                LPARAM((&mut records as *mut Vec<RawWindowRecord>) as isize),
            )
            .map_err(|err| err.to_string())?;
        }
        Ok(decorate_window_records(records))
    }

    unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let records = &mut *(lparam.0 as *mut Vec<RawWindowRecord>);
        if let Some(record) = unsafe { record_for_window(hwnd) } {
            records.push(record);
        }
        true.into()
    }

    unsafe fn record_for_window(hwnd: HWND) -> Option<RawWindowRecord> {
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return None;
        }
        let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
        let has_owner = unsafe { GetWindow(hwnd, GW_OWNER) }
            .ok()
            .map(|owner| !owner.0.is_null())
            .unwrap_or(false);
        if has_owner && ex_style & WS_EX_APPWINDOW.0 == 0 {
            return None;
        }
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }
        if unsafe { is_cloaked(hwnd) } {
            return None;
        }
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid == unsafe { GetCurrentProcessId() } {
            return None;
        }
        let title = unsafe { window_title(hwnd) }?;
        if title.is_empty() || APP_TITLES.contains(&title.as_str()) {
            return None;
        }
        let class_name = unsafe { window_class(hwnd) };
        if SKIPPED_CLASSES.contains(&class_name.as_str()) {
            return None;
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            return None;
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 40 || height < 40 {
            return None;
        }
        Some(RawWindowRecord {
            hwnd: hwnd.0 as isize,
            title,
            width: width as u32,
            height: height as u32,
        })
    }

    unsafe fn window_title(hwnd: HWND) -> Option<String> {
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len <= 0 {
            return None;
        }
        let mut buffer = vec![0u16; len as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
        if copied <= 0 {
            return None;
        }
        Some(
            String::from_utf16_lossy(&buffer[..copied as usize])
                .trim()
                .to_string(),
        )
    }

    unsafe fn window_class(hwnd: HWND) -> String {
        let mut buffer = vec![0u16; 256];
        let copied = unsafe { GetClassNameW(hwnd, &mut buffer) };
        if copied <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..copied as usize])
    }

    unsafe fn is_cloaked(hwnd: HWND) -> bool {
        let mut cloaked = 0i32;
        unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                (&mut cloaked as *mut i32).cast::<c_void>(),
                size_of::<i32>() as u32,
            )
        }
        .is_ok()
            && cloaked != 0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decorate_window_records, resolve_window_apps, window_display, RawWindowRecord,
        MAX_WINDOW_ROWS,
    };
    use screen_watch_core::{config::WindowAppConfig, profile::window_key};

    #[test]
    fn window_display_matches_legacy_duplicate_labeling() {
        assert_eq!(window_display("Demo", 1, false), "Demo");
        assert_eq!(window_display("Demo", 2, true), "Demo #2");
    }

    #[test]
    fn decorate_window_records_sorts_by_title_and_assigns_legacy_keys() {
        let windows = decorate_window_records(vec![
            raw(300, "Beta", 100, 80),
            raw(200, "Alpha", 100, 80),
            raw(100, "Beta", 120, 90),
        ]);

        assert_eq!(windows[0].title, "Alpha");
        assert_eq!(windows[0].ordinal, 1);
        assert_eq!(windows[0].display, "Alpha");
        assert_eq!(windows[0].key, window_key("Alpha", 1));
        assert_eq!(windows[1].hwnd, 100);
        assert_eq!(windows[1].ordinal, 1);
        assert_eq!(windows[1].display, "Beta #1");
        assert_eq!(windows[2].hwnd, 300);
        assert_eq!(windows[2].ordinal, 2);
        assert_eq!(windows[2].display, "Beta #2");
    }

    #[test]
    fn decorate_window_records_dedupes_hwnd_title_and_caps_rows() {
        let mut records = vec![raw(1, "Same", 80, 80), raw(1, "Same", 80, 80)];
        for index in 0..MAX_WINDOW_ROWS + 5 {
            records.push(raw(index as isize + 10, format!("Window {index}"), 80, 80));
        }

        let windows = decorate_window_records(records);

        assert_eq!(windows.len(), MAX_WINDOW_ROWS);
        assert_eq!(
            windows
                .iter()
                .filter(|window| window.title == "Same")
                .count(),
            1
        );
    }

    #[test]
    fn resolve_window_apps_returns_available_windows_and_missing_remembered_apps() {
        let available =
            decorate_window_records(vec![raw(10, "Demo", 200, 100), raw(20, "Demo", 220, 100)]);
        let resolution = resolve_window_apps(
            &[
                WindowAppConfig {
                    title: "Demo".to_string(),
                    ordinal: 2,
                    extra: Default::default(),
                },
                WindowAppConfig {
                    title: "Missing".to_string(),
                    ordinal: 1,
                    extra: Default::default(),
                },
            ],
            &available,
        );

        assert_eq!(resolution.windows.len(), 1);
        assert_eq!(resolution.windows[0].name, "app-Demo__2");
        assert_eq!(resolution.windows[0].display, "Demo #2");
        assert_eq!(resolution.windows[0].hwnd, Some(20));
        assert_eq!(resolution.missing_window_apps.len(), 1);
        assert_eq!(resolution.missing_window_apps[0].title, "Missing");
        assert_eq!(resolution.available_windows.len(), 2);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn list_app_windows_enumerates_without_panic_on_windows_desktop() {
        let windows = super::list_app_windows().unwrap();
        for window in windows {
            assert!(!window.title.trim().is_empty());
            assert!(!window.key.is_empty());
            assert!(window.width >= 40);
            assert!(window.height >= 40);
        }
    }

    fn raw(hwnd: isize, title: impl Into<String>, width: u32, height: u32) -> RawWindowRecord {
        RawWindowRecord {
            hwnd,
            title: title.into(),
            width,
            height,
        }
    }
}
