use crate::screen_capture::{
    bgra_top_down_to_rgb, capture_screen_region, CaptureError, CaptureRegion,
};
use screen_watch_core::detect::RgbFrame;
use std::collections::HashMap;

const VISIBLE_MODE_REPROBE_INTERVAL: u16 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Default)]
pub struct WindowCaptureModeCache {
    visible_hwnds: HashMap<isize, u16>,
}

impl WindowCaptureModeCache {
    #[cfg(test)]
    pub fn is_visible_mode(&self, hwnd: isize) -> bool {
        self.visible_hwnds.contains_key(&hwnd)
    }

    pub fn set_visible_mode(&mut self, hwnd: isize) {
        self.visible_hwnds.insert(hwnd, 0);
    }

    pub fn clear_visible_mode(&mut self, hwnd: isize) {
        self.visible_hwnds.remove(&hwnd);
    }

    fn should_capture_visible_first(&mut self, hwnd: isize) -> bool {
        let Some(ticks) = self.visible_hwnds.get_mut(&hwnd) else {
            return false;
        };
        *ticks += 1;
        if *ticks >= VISIBLE_MODE_REPROBE_INTERVAL {
            *ticks = 0;
            false
        } else {
            true
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FrameQuality {
    mostly_black: bool,
    black_fraction: f64,
    content_bounds: Option<(u32, u32, u32, u32)>,
}

pub fn mostly_black(frame: &RgbFrame) -> bool {
    analyze_frame(frame, 8).mostly_black
}

#[cfg(test)]
pub fn black_fraction(frame: &RgbFrame, threshold: u8) -> f64 {
    analyze_frame(frame, threshold).black_fraction
}

#[cfg(test)]
pub fn crop_black_padding(frame: &RgbFrame, threshold: u8) -> RgbFrame {
    crop_black_padding_owned(frame.clone(), threshold)
}

pub fn choose_window_frame(
    hwnd: isize,
    print_frame: Option<RgbFrame>,
    visible_frame: Option<RgbFrame>,
    mode_cache: Option<&mut WindowCaptureModeCache>,
) -> Option<RgbFrame> {
    let (prepared_print, print_quality, print_was_black) = prepare_print_frame(print_frame);
    choose_prepared_window_frame(
        hwnd,
        prepared_print,
        print_quality,
        print_was_black,
        visible_frame,
        mode_cache,
    )
}

fn choose_prepared_window_frame(
    hwnd: isize,
    prepared_print: Option<RgbFrame>,
    print_quality: Option<FrameQuality>,
    print_was_black: bool,
    visible_frame: Option<RgbFrame>,
    mode_cache: Option<&mut WindowCaptureModeCache>,
) -> Option<RgbFrame> {
    if print_quality.map(frame_quality_is_good).unwrap_or(false) {
        if let Some(cache) = mode_cache {
            cache.clear_visible_mode(hwnd);
        }
        return prepared_print;
    }

    let visible_quality = visible_frame.as_ref().map(|frame| analyze_frame(frame, 8));
    let visible_is_good = visible_quality
        .map(|quality| !quality.mostly_black)
        .unwrap_or(false);
    if visible_is_good {
        let should_use_visible = print_quality
            .map(|quality| {
                quality.mostly_black
                    || visible_quality.unwrap().black_fraction + 0.1 < quality.black_fraction
            })
            .unwrap_or(true);
        if should_use_visible {
            if let Some(cache) = mode_cache {
                if print_was_black {
                    cache.set_visible_mode(hwnd);
                }
            }
            return visible_frame;
        }
    }

    prepared_print.or(visible_frame)
}

pub fn capture_window_preview(hwnd: isize) -> Result<Option<RgbFrame>, CaptureError> {
    let visible = capture_window_visible(hwnd)?;
    if visible
        .as_ref()
        .map(|frame| !mostly_black(frame))
        .unwrap_or(false)
    {
        return Ok(visible);
    }
    capture_window_frame(hwnd, None)
}

pub fn capture_window_frame(
    hwnd: isize,
    mode_cache: Option<&mut WindowCaptureModeCache>,
) -> Result<Option<RgbFrame>, CaptureError> {
    capture_window_frame_with_sources(
        hwnd,
        mode_cache,
        capture_window_visible,
        capture_window_print,
    )
}

fn capture_window_frame_with_sources(
    hwnd: isize,
    mode_cache: Option<&mut WindowCaptureModeCache>,
    mut capture_visible: impl FnMut(isize) -> Result<Option<RgbFrame>, CaptureError>,
    mut capture_print: impl FnMut(isize) -> Result<Option<RgbFrame>, CaptureError>,
) -> Result<Option<RgbFrame>, CaptureError> {
    let mut mode_cache = mode_cache;
    if mode_cache
        .as_deref_mut()
        .map(|cache| cache.should_capture_visible_first(hwnd))
        .unwrap_or(false)
    {
        let visible_frame = capture_visible(hwnd)?;
        if visible_frame
            .as_ref()
            .map(|frame| !analyze_frame(frame, 8).mostly_black)
            .unwrap_or(false)
        {
            return Ok(visible_frame);
        }
        let print_frame = capture_print(hwnd)?;
        return Ok(choose_window_frame(
            hwnd,
            print_frame,
            visible_frame,
            mode_cache,
        ));
    }

    let print_frame = capture_print(hwnd)?;
    let (prepared_print, print_quality, print_was_black) = prepare_print_frame(print_frame);
    if print_quality.map(frame_quality_is_good).unwrap_or(false) {
        if let Some(cache) = mode_cache {
            cache.clear_visible_mode(hwnd);
        }
        return Ok(prepared_print);
    }
    let visible_frame = capture_visible(hwnd)?;
    Ok(choose_prepared_window_frame(
        hwnd,
        prepared_print,
        print_quality,
        print_was_black,
        visible_frame,
        mode_cache,
    ))
}

pub fn capture_window_visible(hwnd: isize) -> Result<Option<RgbFrame>, CaptureError> {
    let Some(rect) = visible_window_rect(hwnd)? else {
        return Ok(None);
    };
    capture_screen_region(CaptureRegion {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
    })
    .map(Some)
}

pub fn capture_window_print(hwnd: isize) -> Result<Option<RgbFrame>, CaptureError> {
    capture_window_print_platform(hwnd)
}

pub fn window_rect(hwnd: isize) -> Result<Option<WindowRect>, CaptureError> {
    window_rect_platform(hwnd)
}

fn visible_window_rect(hwnd: isize) -> Result<Option<WindowRect>, CaptureError> {
    visible_window_rect_platform(hwnd)
}

fn crop_frame(frame: &RgbFrame, left: u32, top: u32, width: u32, height: u32) -> RgbFrame {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
    for y in top..top + height {
        let start = ((y * frame.width + left) * 3) as usize;
        let end = start + width as usize * 3;
        pixels.extend_from_slice(&frame.pixels[start..end]);
    }
    RgbFrame::new(width, height, pixels).expect("cropped frame dimensions are valid")
}

fn analyze_frame(frame: &RgbFrame, threshold: u8) -> FrameQuality {
    let total = frame.pixels.len() / 3;
    if total == 0 {
        return FrameQuality {
            mostly_black: true,
            black_fraction: 1.0,
            content_bounds: None,
        };
    }
    let mut channel_sum = 0u64;
    let mut black = 0usize;
    let mut left = frame.width;
    let mut top = frame.height;
    let mut right = 0u32;
    let mut bottom = 0u32;
    for (index, pixel) in frame.pixels.chunks_exact(3).enumerate() {
        channel_sum += pixel.iter().map(|value| u64::from(*value)).sum::<u64>();
        let max = pixel.iter().copied().max().unwrap_or(0);
        if max < threshold {
            black += 1;
            continue;
        }
        let x = index as u32 % frame.width;
        let y = index as u32 / frame.width;
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
    }
    FrameQuality {
        mostly_black: channel_sum as f64 / (frame.pixels.len() as f64) < 8.0,
        black_fraction: black as f64 / (total as f64),
        content_bounds: (right > left && bottom > top).then_some((left, top, right, bottom)),
    }
}

fn frame_quality_is_good(quality: FrameQuality) -> bool {
    !quality.mostly_black && quality.black_fraction < 0.25
}

fn prepare_print_frame(frame: Option<RgbFrame>) -> (Option<RgbFrame>, Option<FrameQuality>, bool) {
    let Some(frame) = frame else {
        return (None, None, true);
    };
    let initial_quality = analyze_frame(&frame, 8);
    let print_was_black = initial_quality.mostly_black;
    let frame = if print_was_black {
        frame
    } else {
        crop_black_padding_owned_with_quality(frame, initial_quality)
    };
    let quality = analyze_frame(&frame, 8);
    (Some(frame), Some(quality), print_was_black)
}

#[cfg(test)]
fn crop_black_padding_owned(frame: RgbFrame, threshold: u8) -> RgbFrame {
    let quality = analyze_frame(&frame, threshold);
    crop_black_padding_owned_with_quality(frame, quality)
}

fn crop_black_padding_owned_with_quality(frame: RgbFrame, quality: FrameQuality) -> RgbFrame {
    if quality.black_fraction < 0.25 {
        return frame;
    }
    let Some((left, top, right, bottom)) = quality.content_bounds else {
        return frame;
    };
    let width = frame.width;
    let height = frame.height;
    let should_crop = left as f64 <= width as f64 * 0.05
        && top as f64 <= height as f64 * 0.05
        && right as f64 >= width as f64 * 0.35
        && bottom as f64 >= height as f64 * 0.35
        && ((right as f64) < width as f64 * 0.9 || (bottom as f64) < height as f64 * 0.9);
    if !should_crop {
        return frame;
    }
    crop_frame(&frame, left, top, right - left, bottom - top)
}

#[cfg(windows)]
fn visible_window_rect_platform(hwnd: isize) -> Result<Option<WindowRect>, CaptureError> {
    use std::{ffi::c_void, mem::size_of};
    use windows::Win32::{
        Foundation::{HWND, RECT},
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
        UI::WindowsAndMessaging::{GetWindowRect, IsIconic},
    };

    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        if IsIconic(hwnd).as_bool() {
            return Ok(None);
        }
        let mut rect = RECT::default();
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rect as *mut RECT).cast::<c_void>(),
            size_of::<RECT>() as u32,
        )
        .is_err()
            && GetWindowRect(hwnd, &mut rect).is_err()
        {
            return Ok(None);
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 2 || height < 2 {
            return Ok(None);
        }
        Ok(Some(WindowRect {
            left: rect.left,
            top: rect.top,
            width: width as u32,
            height: height as u32,
        }))
    }
}

#[cfg(not(windows))]
fn visible_window_rect_platform(_hwnd: isize) -> Result<Option<WindowRect>, CaptureError> {
    Ok(None)
}

#[cfg(windows)]
fn window_rect_platform(hwnd: isize) -> Result<Option<WindowRect>, CaptureError> {
    use std::{ffi::c_void, mem::size_of};
    use windows::Win32::{
        Foundation::{HWND, RECT},
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
        UI::WindowsAndMessaging::{GetWindowPlacement, GetWindowRect, IsIconic, WINDOWPLACEMENT},
    };

    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        let mut rect = RECT::default();
        if IsIconic(hwnd).as_bool() {
            let mut placement = WINDOWPLACEMENT {
                length: size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            if GetWindowPlacement(hwnd, &mut placement).is_ok() {
                rect = placement.rcNormalPosition;
            } else if GetWindowRect(hwnd, &mut rect).is_err() {
                return Ok(None);
            }
        } else if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rect as *mut RECT).cast::<c_void>(),
            size_of::<RECT>() as u32,
        )
        .is_err()
            && GetWindowRect(hwnd, &mut rect).is_err()
        {
            return Ok(None);
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 2 || height < 2 {
            return Ok(None);
        }
        Ok(Some(WindowRect {
            left: rect.left,
            top: rect.top,
            width: width as u32,
            height: height as u32,
        }))
    }
}

#[cfg(not(windows))]
fn window_rect_platform(_hwnd: isize) -> Result<Option<WindowRect>, CaptureError> {
    Ok(None)
}

#[cfg(windows)]
fn capture_window_print_platform(hwnd: isize) -> Result<Option<RgbFrame>, CaptureError> {
    use std::{ffi::c_void, mem::size_of};
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
            GetWindowDC, ReleaseDC, SelectObject, BITMAPINFO, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
            HGDIOBJ, SRCCOPY,
        },
        Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS},
        UI::WindowsAndMessaging::PW_RENDERFULLCONTENT,
    };

    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        let Some(rect) = window_rect_platform(hwnd.0 as isize)? else {
            return Ok(None);
        };
        let width_i32 = i32::try_from(rect.width)
            .map_err(|_| CaptureError::Platform("window width is too large".to_string()))?;
        let height_i32 = i32::try_from(rect.height)
            .map_err(|_| CaptureError::Platform("window height is too large".to_string()))?;
        let window_dc = WindowDc::new(hwnd)?;
        let memory_dc = MemoryDc::new(window_dc.dc)?;
        let bitmap = Bitmap::new(window_dc.dc, width_i32, height_i32)?;
        let selected = SelectedObject::new(memory_dc.0, bitmap.as_object())?;

        let printed =
            PrintWindow(hwnd, memory_dc.0, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
        if !printed {
            BitBlt(
                memory_dc.0,
                0,
                0,
                width_i32,
                height_i32,
                Some(window_dc.dc),
                0,
                0,
                SRCCOPY,
            )
            .map_err(|err| CaptureError::Platform(err.to_string()))?;
        }

        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = size_of::<windows::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width_i32;
        info.bmiHeader.biHeight = -height_i32;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;

        let mut bgra = vec![0u8; rect.width as usize * rect.height as usize * 4];
        let scan_lines = GetDIBits(
            memory_dc.0,
            bitmap.0,
            0,
            rect.height,
            Some(bgra.as_mut_ptr().cast::<c_void>()),
            &mut info,
            DIB_RGB_COLORS,
        );
        drop(selected);
        if scan_lines == 0 {
            return Err(CaptureError::Platform("GetDIBits failed".to_string()));
        }
        return bgra_top_down_to_rgb(rect.width, rect.height, &bgra).map(Some);
    }

    struct WindowDc {
        hwnd: HWND,
        dc: HDC,
    }
    impl WindowDc {
        unsafe fn new(hwnd: HWND) -> Result<Self, CaptureError> {
            let dc = unsafe { GetWindowDC(Some(hwnd)) };
            if dc.is_invalid() {
                Err(CaptureError::Platform("GetWindowDC failed".to_string()))
            } else {
                Ok(Self { hwnd, dc })
            }
        }
    }
    impl Drop for WindowDc {
        fn drop(&mut self) {
            unsafe {
                ReleaseDC(Some(self.hwnd), self.dc);
            }
        }
    }

    struct MemoryDc(HDC);
    impl MemoryDc {
        unsafe fn new(source: HDC) -> Result<Self, CaptureError> {
            let dc = unsafe { CreateCompatibleDC(Some(source)) };
            if dc.is_invalid() {
                Err(CaptureError::Platform(
                    "CreateCompatibleDC failed".to_string(),
                ))
            } else {
                Ok(Self(dc))
            }
        }
    }
    impl Drop for MemoryDc {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }

    struct Bitmap(HBITMAP);
    impl Bitmap {
        unsafe fn new(source: HDC, width: i32, height: i32) -> Result<Self, CaptureError> {
            let bitmap = unsafe { CreateCompatibleBitmap(source, width, height) };
            if bitmap.is_invalid() {
                Err(CaptureError::Platform(
                    "CreateCompatibleBitmap failed".to_string(),
                ))
            } else {
                Ok(Self(bitmap))
            }
        }

        fn as_object(&self) -> HGDIOBJ {
            self.0.into()
        }
    }
    impl Drop for Bitmap {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }

    struct SelectedObject {
        dc: HDC,
        previous: HGDIOBJ,
    }
    impl SelectedObject {
        unsafe fn new(dc: HDC, object: HGDIOBJ) -> Result<Self, CaptureError> {
            let previous = unsafe { SelectObject(dc, object) };
            if previous.is_invalid() {
                Err(CaptureError::Platform("SelectObject failed".to_string()))
            } else {
                Ok(Self { dc, previous })
            }
        }
    }
    impl Drop for SelectedObject {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.previous);
            }
        }
    }
}

#[cfg(not(windows))]
fn capture_window_print_platform(_hwnd: isize) -> Result<Option<RgbFrame>, CaptureError> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{
        black_fraction, capture_window_frame, capture_window_frame_with_sources,
        capture_window_preview, choose_window_frame, crop_black_padding, mostly_black, window_rect,
        WindowCaptureModeCache,
    };
    use screen_watch_core::detect::RgbFrame;
    use std::cell::Cell;

    #[test]
    fn mostly_black_detects_blank_preview_like_python_baseline() {
        assert!(mostly_black(&solid(4, 4, [0, 0, 0])));
        assert!(!mostly_black(&solid(4, 4, [80, 80, 80])));
    }

    #[test]
    fn black_fraction_and_crop_black_padding_match_python_shape_behavior() {
        let mut pixels = Vec::new();
        for _y in 0..4 {
            for x in 0..4 {
                if x < 2 {
                    pixels.extend_from_slice(&[80, 80, 80]);
                } else {
                    pixels.extend_from_slice(&[0, 0, 0]);
                }
            }
        }
        let padded = RgbFrame::new(4, 4, pixels).unwrap();
        assert!(black_fraction(&padded, 8) > 0.4);
        let cropped = crop_black_padding(&padded, 8);
        assert_eq!((cropped.width, cropped.height), (2, 4));
        assert_eq!(cropped, solid(2, 4, [80, 80, 80]));
    }

    #[test]
    fn choose_window_frame_falls_back_to_visible_when_printwindow_is_black() {
        let mut cache = WindowCaptureModeCache::default();
        let frame = choose_window_frame(
            123,
            Some(solid(2, 2, [0, 0, 0])),
            Some(solid(2, 2, [10, 20, 30])),
            Some(&mut cache),
        )
        .unwrap();
        assert_eq!(frame, solid(2, 2, [10, 20, 30]));
        assert!(cache.is_visible_mode(123));
    }

    #[test]
    fn choose_window_frame_crops_printwindow_black_padding_before_visible_fallback() {
        let mut pixels = Vec::new();
        for _y in 0..4 {
            for x in 0..4 {
                if x < 2 {
                    pixels.extend_from_slice(&[80, 80, 80]);
                } else {
                    pixels.extend_from_slice(&[0, 0, 0]);
                }
            }
        }
        let padded = RgbFrame::new(4, 4, pixels).unwrap();
        let frame =
            choose_window_frame(123, Some(padded), Some(solid(4, 4, [60, 60, 60])), None).unwrap();
        assert_eq!(frame, solid(2, 4, [80, 80, 80]));
    }

    #[test]
    fn capture_window_frame_prefers_printwindow_before_visible_desktop_pixels() {
        let mut cache = WindowCaptureModeCache::default();
        let visible_calls = Cell::new(0);
        let frame = capture_window_frame_with_sources(
            123,
            Some(&mut cache),
            |_| {
                visible_calls.set(visible_calls.get() + 1);
                Ok(Some(solid(2, 2, [10, 20, 30])))
            },
            |_| Ok(Some(solid(2, 2, [40, 50, 60]))),
        )
        .unwrap()
        .unwrap();

        assert_eq!(frame, solid(2, 2, [40, 50, 60]));
        assert_eq!(visible_calls.get(), 0);
        assert!(!cache.is_visible_mode(123));
    }

    #[test]
    fn cached_visible_mode_skips_printwindow_until_reprobe() {
        let mut cache = WindowCaptureModeCache::default();
        cache.set_visible_mode(123);
        let print_calls = Cell::new(0);

        let frame = capture_window_frame_with_sources(
            123,
            Some(&mut cache),
            |_| Ok(Some(solid(2, 2, [10, 20, 30]))),
            |_| {
                print_calls.set(print_calls.get() + 1);
                Ok(Some(solid(2, 2, [40, 50, 60])))
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(frame, solid(2, 2, [10, 20, 30]));
        assert_eq!(print_calls.get(), 0);
        assert!(cache.is_visible_mode(123));
    }

    #[test]
    fn capture_window_frame_falls_back_to_printwindow_when_visible_is_black() {
        let mut cache = WindowCaptureModeCache::default();
        cache.set_visible_mode(123);
        let frame = capture_window_frame_with_sources(
            123,
            Some(&mut cache),
            |_| Ok(Some(solid(2, 2, [0, 0, 0]))),
            |_| Ok(Some(solid(2, 2, [40, 50, 60]))),
        )
        .unwrap()
        .unwrap();

        assert_eq!(frame, solid(2, 2, [40, 50, 60]));
        assert!(!cache.is_visible_mode(123));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop with at least one selectable window"]
    fn capture_first_app_window_preview_on_windows_desktop() {
        let windows = crate::window_sources::list_app_windows().unwrap();
        let Some(window) = windows.first() else {
            return;
        };
        let rect = window_rect(window.hwnd).unwrap().unwrap();
        assert!(rect.width >= 2);
        assert!(rect.height >= 2);
        let frame = capture_window_preview(window.hwnd).unwrap().unwrap();
        assert!(frame.width >= 2);
        assert!(frame.height >= 2);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop with at least one selectable window"]
    fn capture_first_app_window_frame_on_windows_desktop() {
        let windows = crate::window_sources::list_app_windows().unwrap();
        let Some(window) = windows.first() else {
            return;
        };
        let mut cache = WindowCaptureModeCache::default();
        let frame = capture_window_frame(window.hwnd, Some(&mut cache))
            .unwrap()
            .unwrap();
        assert!(frame.width >= 2);
        assert!(frame.height >= 2);
    }

    fn solid(width: u32, height: u32, rgb: [u8; 3]) -> RgbFrame {
        let mut pixels = Vec::new();
        for _ in 0..width * height {
            pixels.extend_from_slice(&rgb);
        }
        RgbFrame::new(width, height, pixels).unwrap()
    }
}
