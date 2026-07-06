use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DwmPreviewRect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DwmPreviewSyncResult {
    pub active: bool,
    pub reused: bool,
    pub source_key: String,
    pub rect: DwmPreviewRect,
}

pub struct DwmPreviewState {
    entries: Mutex<HashMap<String, DwmPreviewEntry>>,
    backend: Arc<dyn DwmThumbnailBackend>,
}

struct DwmPreviewEntry {
    destination_hwnd: isize,
    source_hwnd: isize,
    rect: DwmPreviewRect,
    thumbnail: RegisteredThumbnail,
}

struct RegisteredThumbnail {
    id: isize,
    backend: Arc<dyn DwmThumbnailBackend>,
}

trait DwmThumbnailBackend: Send + Sync {
    fn register(&self, destination_hwnd: isize, source_hwnd: isize) -> Result<isize, String>;
    fn update(&self, thumbnail: isize, rect: DwmPreviewRect, visible: bool) -> Result<(), String>;
    fn unregister(&self, thumbnail: isize);
}

struct PlatformDwmThumbnailBackend;

impl Default for DwmPreviewState {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            backend: Arc::new(PlatformDwmThumbnailBackend),
        }
    }
}

impl DwmPreviewState {
    #[cfg(test)]
    fn with_backend(backend: Arc<dyn DwmThumbnailBackend>) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            backend,
        }
    }

    pub fn sync_css_rect(
        &self,
        source_key: String,
        destination_hwnd: isize,
        source_hwnd: isize,
        css_rect: CssPreviewRect,
        scale_factor: f64,
    ) -> Result<DwmPreviewSyncResult, String> {
        if source_key.trim().is_empty() {
            return Err("DWM preview source key is empty".to_string());
        }
        if destination_hwnd == 0 || source_hwnd == 0 {
            return Err("DWM preview window handle is unavailable".to_string());
        }
        let rect = DwmPreviewRect::from_css_rect(css_rect, scale_factor)?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "DWM preview state lock is poisoned".to_string())?;
        let reusable = entries.get(&source_key).is_some_and(|entry| {
            entry.destination_hwnd == destination_hwnd && entry.source_hwnd == source_hwnd
        });
        if !reusable {
            entries.remove(&source_key);
            let thumbnail = RegisteredThumbnail {
                id: self.backend.register(destination_hwnd, source_hwnd)?,
                backend: self.backend.clone(),
            };
            entries.insert(
                source_key.clone(),
                DwmPreviewEntry {
                    destination_hwnd,
                    source_hwnd,
                    rect,
                    thumbnail,
                },
            );
        }
        let entry = entries
            .get_mut(&source_key)
            .expect("DWM preview entry was inserted before update");
        self.backend.update(entry.thumbnail.id, rect, true)?;
        entry.rect = rect;
        Ok(DwmPreviewSyncResult {
            active: true,
            reused: reusable,
            source_key,
            rect,
        })
    }

    pub fn retain_keys<'a>(&self, source_keys: impl IntoIterator<Item = &'a str>) {
        let keep = source_keys
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<HashSet<String>>();
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|key, _entry| keep.contains(key));
        }
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries
            .lock()
            .map(|entries| entries.len())
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssPreviewRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl DwmPreviewRect {
    pub fn from_css_rect(rect: CssPreviewRect, scale_factor: f64) -> Result<Self, String> {
        let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        if !rect.width.is_finite()
            || !rect.height.is_finite()
            || rect.width < 1.0
            || rect.height < 1.0
        {
            return Err("DWM preview rect must have positive dimensions".to_string());
        }
        Ok(Self {
            left: scaled_i32(rect.left, scale, "left")?,
            top: scaled_i32(rect.top, scale, "top")?,
            width: scaled_u32(rect.width, scale, "width")?,
            height: scaled_u32(rect.height, scale, "height")?,
        })
    }
}

fn scaled_i32(value: f64, scale: f64, name: &str) -> Result<i32, String> {
    if !value.is_finite() {
        return Err(format!("DWM preview rect {name} is not finite"));
    }
    let scaled = (value * scale).round();
    if scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        return Err(format!("DWM preview rect {name} is out of range"));
    }
    Ok(scaled as i32)
}

fn scaled_u32(value: f64, scale: f64, name: &str) -> Result<u32, String> {
    if !value.is_finite() {
        return Err(format!("DWM preview rect {name} is not finite"));
    }
    let scaled = (value * scale).round();
    if scaled < 1.0 || scaled > u32::MAX as f64 {
        return Err(format!("DWM preview rect {name} is out of range"));
    }
    Ok(scaled as u32)
}

pub fn sync_window_preview(
    state: &DwmPreviewState,
    window: &tauri::Window,
    source_key: String,
    source_hwnd: isize,
    css_rect: CssPreviewRect,
) -> Result<DwmPreviewSyncResult, String> {
    sync_window_preview_platform(state, window, source_key, source_hwnd, css_rect)
}

impl Drop for RegisteredThumbnail {
    fn drop(&mut self) {
        if self.id != 0 {
            self.backend.unregister(self.id);
        }
    }
}

impl DwmThumbnailBackend for PlatformDwmThumbnailBackend {
    fn register(&self, destination_hwnd: isize, source_hwnd: isize) -> Result<isize, String> {
        register_thumbnail(destination_hwnd, source_hwnd)
    }

    fn update(&self, thumbnail: isize, rect: DwmPreviewRect, visible: bool) -> Result<(), String> {
        update_thumbnail(thumbnail, rect, visible)
    }

    fn unregister(&self, thumbnail: isize) {
        unregister_thumbnail(thumbnail);
    }
}

#[cfg(windows)]
fn sync_window_preview_platform(
    state: &DwmPreviewState,
    window: &tauri::Window,
    source_key: String,
    source_hwnd: isize,
    css_rect: CssPreviewRect,
) -> Result<DwmPreviewSyncResult, String> {
    let destination_hwnd = window
        .hwnd()
        .map_err(|err| format!("DWM preview destination window is unavailable: {err}"))?
        .0 as isize;
    let scale_factor = window.scale_factor().map_err(|err| err.to_string())?;
    state.sync_css_rect(
        source_key,
        destination_hwnd,
        source_hwnd,
        css_rect,
        scale_factor,
    )
}

#[cfg(not(windows))]
fn sync_window_preview_platform(
    _state: &DwmPreviewState,
    _window: &tauri::Window,
    _source_key: String,
    _source_hwnd: isize,
    _css_rect: CssPreviewRect,
) -> Result<DwmPreviewSyncResult, String> {
    Err("DWM preview is only supported on Windows".to_string())
}

#[cfg(windows)]
fn register_thumbnail(destination_hwnd: isize, source_hwnd: isize) -> Result<isize, String> {
    use std::ffi::c_void;
    use windows::Win32::{Foundation::HWND, Graphics::Dwm::DwmRegisterThumbnail};

    unsafe {
        DwmRegisterThumbnail(
            HWND(destination_hwnd as *mut c_void),
            HWND(source_hwnd as *mut c_void),
        )
        .map_err(|err| format!("DWM thumbnail registration failed: {err}"))
    }
}

#[cfg(not(windows))]
fn register_thumbnail(_destination_hwnd: isize, _source_hwnd: isize) -> Result<isize, String> {
    Err("DWM preview is only supported on Windows".to_string())
}

#[cfg(windows)]
fn update_thumbnail(thumbnail: isize, rect: DwmPreviewRect, visible: bool) -> Result<(), String> {
    use windows::Win32::{
        Foundation::RECT,
        Graphics::Dwm::{
            DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
            DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
        },
    };

    let props = DWM_THUMBNAIL_PROPERTIES {
        dwFlags: DWM_TNP_RECTDESTINATION
            | DWM_TNP_VISIBLE
            | DWM_TNP_OPACITY
            | DWM_TNP_SOURCECLIENTAREAONLY,
        rcDestination: RECT {
            left: rect.left,
            top: rect.top,
            right: rect.left + rect.width as i32,
            bottom: rect.top + rect.height as i32,
        },
        opacity: 255,
        fVisible: visible.into(),
        fSourceClientAreaOnly: true.into(),
        ..Default::default()
    };
    unsafe { DwmUpdateThumbnailProperties(thumbnail, &props) }
        .map_err(|err| format!("DWM thumbnail update failed: {err}"))
}

#[cfg(not(windows))]
fn update_thumbnail(
    _thumbnail: isize,
    _rect: DwmPreviewRect,
    _visible: bool,
) -> Result<(), String> {
    Err("DWM preview is only supported on Windows".to_string())
}

#[cfg(windows)]
fn unregister_thumbnail(thumbnail: isize) {
    use windows::Win32::Graphics::Dwm::DwmUnregisterThumbnail;

    if thumbnail != 0 {
        let _ = unsafe { DwmUnregisterThumbnail(thumbnail) };
    }
}

#[cfg(not(windows))]
fn unregister_thumbnail(_thumbnail: isize) {}

#[cfg(test)]
mod tests {
    use super::{CssPreviewRect, DwmPreviewRect, DwmPreviewState, DwmThumbnailBackend};
    use std::sync::{
        atomic::{AtomicIsize, Ordering},
        Arc, Mutex,
    };

    #[test]
    fn css_preview_rect_scales_to_physical_pixels() {
        let rect = DwmPreviewRect::from_css_rect(
            CssPreviewRect {
                left: 10.2,
                top: 20.6,
                width: 100.2,
                height: 50.7,
            },
            1.5,
        )
        .unwrap();

        assert_eq!(
            rect,
            DwmPreviewRect {
                left: 15,
                top: 31,
                width: 150,
                height: 76
            }
        );
    }

    #[test]
    fn css_preview_rect_rejects_empty_dimensions() {
        let err = DwmPreviewRect::from_css_rect(
            CssPreviewRect {
                left: 0.0,
                top: 0.0,
                width: 0.0,
                height: 10.0,
            },
            1.0,
        )
        .unwrap_err();

        assert!(err.contains("positive dimensions"));
    }

    #[test]
    fn clear_on_empty_state_is_safe() {
        let state = DwmPreviewState::default();

        state.clear();
        state.retain_keys(["missing"].iter().copied());

        assert_eq!(state.len(), 0);
    }

    #[test]
    fn sync_reuses_thumbnail_for_same_destination_and_source() {
        let backend = Arc::new(FakeDwmBackend::default());
        let state = DwmPreviewState::with_backend(backend.clone());
        let first = CssPreviewRect {
            left: 1.0,
            top: 2.0,
            width: 30.0,
            height: 40.0,
        };
        let second = CssPreviewRect {
            left: 5.0,
            top: 6.0,
            width: 70.0,
            height: 80.0,
        };

        let result = state
            .sync_css_rect("app:demo".to_string(), 10, 20, first, 1.0)
            .unwrap();
        assert!(!result.reused);
        assert_eq!(result.rect.left, 1);

        let result = state
            .sync_css_rect("app:demo".to_string(), 10, 20, second, 1.0)
            .unwrap();
        assert!(result.reused);
        assert_eq!(result.rect.left, 5);
        assert_eq!(state.len(), 1);
        assert_eq!(
            backend.calls(),
            vec![
                "register:10:20:1",
                "update:1:1:2:30:40:true",
                "update:1:5:6:70:80:true"
            ]
        );
    }

    #[test]
    fn sync_replaces_stale_thumbnail_and_retain_clear_unregister() {
        let backend = Arc::new(FakeDwmBackend::default());
        let state = DwmPreviewState::with_backend(backend.clone());
        let rect = CssPreviewRect {
            left: 0.0,
            top: 0.0,
            width: 10.0,
            height: 10.0,
        };

        state
            .sync_css_rect("app:demo".to_string(), 10, 20, rect, 1.0)
            .unwrap();
        let replacement = state
            .sync_css_rect("app:demo".to_string(), 10, 30, rect, 1.0)
            .unwrap();
        assert!(!replacement.reused);
        assert_eq!(state.len(), 1);
        assert_eq!(
            backend.calls(),
            vec![
                "register:10:20:1",
                "update:1:0:0:10:10:true",
                "unregister:1",
                "register:10:30:2",
                "update:2:0:0:10:10:true"
            ]
        );

        state.retain_keys(["other"].iter().copied());
        assert_eq!(state.len(), 0);
        assert!(backend.calls().contains(&"unregister:2".to_string()));

        state
            .sync_css_rect("app:demo".to_string(), 10, 40, rect, 1.0)
            .unwrap();
        state.clear();
        assert_eq!(state.len(), 0);
        assert!(backend.calls().contains(&"unregister:3".to_string()));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop with DWM enabled"]
    fn real_dwm_thumbnail_registers_updates_and_clears_on_windows_desktop() {
        let destination = TestWindow::new(
            windows::core::w!("Screen Watch OCR DWM destination"),
            80,
            80,
            320,
            240,
        )
        .unwrap();
        let source = TestWindow::new(
            windows::core::w!("Screen Watch OCR DWM source"),
            440,
            80,
            320,
            240,
        )
        .unwrap();
        let state = DwmPreviewState::default();

        let first = state
            .sync_css_rect(
                "window:dwm-smoke".to_string(),
                destination.hwnd(),
                source.hwnd(),
                CssPreviewRect {
                    left: 12.0,
                    top: 18.0,
                    width: 120.0,
                    height: 90.0,
                },
                1.0,
            )
            .unwrap();
        assert!(first.active);
        assert!(!first.reused);
        assert_eq!(state.len(), 1);

        let second = state
            .sync_css_rect(
                "window:dwm-smoke".to_string(),
                destination.hwnd(),
                source.hwnd(),
                CssPreviewRect {
                    left: 24.0,
                    top: 30.0,
                    width: 160.0,
                    height: 100.0,
                },
                1.0,
            )
            .unwrap();
        assert!(second.reused);
        assert_eq!(second.rect.width, 160);

        state.clear();
        assert_eq!(state.len(), 0);
    }

    #[derive(Default)]
    struct FakeDwmBackend {
        next_id: AtomicIsize,
        calls: Mutex<Vec<String>>,
    }

    impl FakeDwmBackend {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn push(&self, call: String) {
            self.calls.lock().unwrap().push(call);
        }
    }

    impl DwmThumbnailBackend for FakeDwmBackend {
        fn register(&self, destination_hwnd: isize, source_hwnd: isize) -> Result<isize, String> {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
            self.push(format!("register:{destination_hwnd}:{source_hwnd}:{id}"));
            Ok(id)
        }

        fn update(
            &self,
            thumbnail: isize,
            rect: DwmPreviewRect,
            visible: bool,
        ) -> Result<(), String> {
            self.push(format!(
                "update:{}:{}:{}:{}:{}:{}",
                thumbnail, rect.left, rect.top, rect.width, rect.height, visible
            ));
            Ok(())
        }

        fn unregister(&self, thumbnail: isize) {
            self.push(format!("unregister:{thumbnail}"));
        }
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
            .map_err(|err| format!("failed to create DWM smoke window: {err}"))?;
            if hwnd.0.is_null() {
                return Err("failed to create DWM smoke window".to_string());
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
}
