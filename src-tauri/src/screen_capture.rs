use screen_watch_core::{detect::RgbFrame, evidence::save_rgb_png};
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRegion {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("capture width and height must be positive")]
    EmptyRegion,
    #[error("{0}")]
    Platform(String),
    #[error("{0}")]
    Image(String),
}

pub fn capture_screen_region(region: CaptureRegion) -> Result<RgbFrame, CaptureError> {
    if region.width == 0 || region.height == 0 {
        return Err(CaptureError::EmptyRegion);
    }
    capture_screen_region_platform(region)
}

pub fn frame_to_png_bytes(frame: &RgbFrame) -> Result<Vec<u8>, CaptureError> {
    let mut out = Vec::new();
    {
        let mut cursor = Cursor::new(&mut out);
        write_rgb_png(&mut cursor, frame).map_err(|err| CaptureError::Image(err.to_string()))?;
    }
    Ok(out)
}

fn write_rgb_png(writer: impl std::io::Write, frame: &RgbFrame) -> std::io::Result<()> {
    let mut encoder = png::Encoder::new(writer, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&frame.pixels)?;
    Ok(())
}

#[allow(dead_code)]
pub fn save_capture_preview_png(
    path: impl AsRef<std::path::Path>,
    region: CaptureRegion,
) -> Result<RgbFrame, CaptureError> {
    let frame = capture_screen_region(region)?;
    save_rgb_png(path, &frame, &[]).map_err(|err| CaptureError::Image(err.to_string()))?;
    Ok(frame)
}

pub(crate) fn bgra_top_down_to_rgb(
    width: u32,
    height: u32,
    bgra: &[u8],
) -> Result<RgbFrame, CaptureError> {
    let expected = width as usize * height as usize * 4;
    if bgra.len() != expected {
        return Err(CaptureError::Image(format!(
            "expected {expected} BGRA bytes, got {}",
            bgra.len()
        )));
    }
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for pixel in bgra.chunks_exact(4) {
        rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
    }
    RgbFrame::new(width, height, rgb).map_err(CaptureError::Image)
}

#[cfg(windows)]
fn capture_screen_region_platform(region: CaptureRegion) -> Result<RgbFrame, CaptureError> {
    use std::{ffi::c_void, mem::size_of};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
        HBITMAP, HDC, HGDIOBJ, SRCCOPY,
    };

    let width_i32 = i32::try_from(region.width)
        .map_err(|_| CaptureError::Platform("capture width is too large".to_string()))?;
    let height_i32 = i32::try_from(region.height)
        .map_err(|_| CaptureError::Platform("capture height is too large".to_string()))?;

    unsafe {
        let screen_dc = ScreenDc::new()?;
        let memory_dc = MemoryDc::new(screen_dc.0)?;
        let bitmap = Bitmap::new(screen_dc.0, width_i32, height_i32)?;
        let selected = SelectedObject::new(memory_dc.0, bitmap.as_object())?;

        BitBlt(
            memory_dc.0,
            0,
            0,
            width_i32,
            height_i32,
            Some(screen_dc.0),
            region.left,
            region.top,
            SRCCOPY | CAPTUREBLT,
        )
        .map_err(|err| CaptureError::Platform(err.to_string()))?;

        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = size_of::<windows::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width_i32;
        info.bmiHeader.biHeight = -height_i32;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;

        let mut bgra = vec![0u8; region.width as usize * region.height as usize * 4];
        let scan_lines = GetDIBits(
            memory_dc.0,
            bitmap.0,
            0,
            region.height,
            Some(bgra.as_mut_ptr().cast::<c_void>()),
            &mut info,
            DIB_RGB_COLORS,
        );
        drop(selected);
        if scan_lines == 0 {
            return Err(CaptureError::Platform("GetDIBits failed".to_string()));
        }
        return bgra_top_down_to_rgb(region.width, region.height, &bgra);
    }

    struct ScreenDc(HDC);
    impl ScreenDc {
        unsafe fn new() -> Result<Self, CaptureError> {
            let dc = unsafe { GetDC(None) };
            if dc.is_invalid() {
                Err(CaptureError::Platform("GetDC failed".to_string()))
            } else {
                Ok(Self(dc))
            }
        }
    }
    impl Drop for ScreenDc {
        fn drop(&mut self) {
            unsafe {
                ReleaseDC(None, self.0);
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
fn capture_screen_region_platform(_region: CaptureRegion) -> Result<RgbFrame, CaptureError> {
    Err(CaptureError::Platform(
        "screen capture is currently implemented on Windows only".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        bgra_top_down_to_rgb, capture_screen_region, frame_to_png_bytes, CaptureError,
        CaptureRegion,
    };

    #[test]
    fn bgra_top_down_conversion_preserves_row_order_and_rgb_channels() {
        let frame = bgra_top_down_to_rgb(
            2,
            2,
            &[
                30, 20, 10, 255, 60, 50, 40, 255, //
                90, 80, 70, 255, 120, 110, 100, 255,
            ],
        )
        .unwrap();
        assert_eq!(frame.pixel(0, 0), Some([10, 20, 30]));
        assert_eq!(frame.pixel(1, 0), Some([40, 50, 60]));
        assert_eq!(frame.pixel(0, 1), Some([70, 80, 90]));
        assert_eq!(frame.pixel(1, 1), Some([100, 110, 120]));
    }

    #[test]
    fn bgra_conversion_rejects_wrong_buffer_size() {
        assert!(matches!(
            bgra_top_down_to_rgb(2, 1, &[0, 1, 2]),
            Err(CaptureError::Image(_))
        ));
    }

    #[test]
    fn capture_rejects_empty_regions_before_platform_call() {
        assert!(matches!(
            capture_screen_region(CaptureRegion {
                left: 0,
                top: 0,
                width: 0,
                height: 1,
            }),
            Err(CaptureError::EmptyRegion)
        ));
    }

    #[test]
    fn frame_to_png_bytes_writes_png_signature() {
        let frame = bgra_top_down_to_rgb(1, 1, &[3, 2, 1, 255]).unwrap();
        let bytes = frame_to_png_bytes(&frame).unwrap();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn captures_tiny_screen_region_on_windows_desktop() {
        let frame = capture_screen_region(CaptureRegion {
            left: 0,
            top: 0,
            width: 1,
            height: 1,
        })
        .unwrap();
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.pixels.len(), 3);
    }
}
