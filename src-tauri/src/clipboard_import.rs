use screen_watch_core::detect::RgbFrame;
use std::{io::Cursor, path::PathBuf};

#[derive(Debug, Default)]
pub struct ClipboardTemplateImages {
    pub paths: Vec<PathBuf>,
    pub frames: Vec<RgbFrame>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClipboardImportError {
    #[error("剪贴板里没有图片；用截图工具复制后再按 Ctrl+V。")]
    NoImages,
    #[error("{0}")]
    Platform(String),
    #[error("cannot decode clipboard image: {0}")]
    Decode(String),
}

pub fn read_clipboard_template_images() -> Result<ClipboardTemplateImages, ClipboardImportError> {
    read_clipboard_template_images_platform()
}

#[cfg(windows)]
fn read_clipboard_template_images_platform() -> Result<ClipboardTemplateImages, ClipboardImportError>
{
    use windows::Win32::{
        System::{
            DataExchange::GetClipboardData,
            Ole::{CF_DIB, CF_DIBV5, CF_HDROP},
        },
        UI::Shell::HDROP,
    };

    open_clipboard_with_retry()?;
    let _guard = ClipboardGuard;

    let mut out = ClipboardTemplateImages::default();
    if clipboard_format_available(u32::from(CF_HDROP.0)) {
        let handle = unsafe { GetClipboardData(u32::from(CF_HDROP.0)) }.map_err(|err| {
            ClipboardImportError::Platform(format!("cannot read clipboard file list: {err}"))
        })?;
        out.paths = clipboard_file_paths(HDROP(handle.0))
            .into_iter()
            .filter(|path| is_supported_image_path(path))
            .collect();
    }

    if clipboard_format_available(u32::from(CF_DIBV5.0)) {
        if let Some(frame) = clipboard_dib_frame(u32::from(CF_DIBV5.0))? {
            out.frames.push(frame);
        }
    } else if clipboard_format_available(u32::from(CF_DIB.0)) {
        if let Some(frame) = clipboard_dib_frame(u32::from(CF_DIB.0))? {
            out.frames.push(frame);
        }
    }

    if out.paths.is_empty() && out.frames.is_empty() {
        return Err(ClipboardImportError::NoImages);
    }
    Ok(out)
}

#[cfg(windows)]
struct ClipboardGuard;

#[cfg(windows)]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::DataExchange::CloseClipboard();
        }
    }
}

#[cfg(windows)]
fn open_clipboard_with_retry() -> Result<(), ClipboardImportError> {
    use std::{thread, time::Duration};
    use windows::Win32::System::DataExchange::OpenClipboard;

    const ATTEMPTS: usize = 12;
    const RETRY_DELAY: Duration = Duration::from_millis(25);

    let mut last_error = None;
    for attempt in 0..ATTEMPTS {
        match unsafe { OpenClipboard(None) } {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = Some(err);
                if attempt + 1 < ATTEMPTS {
                    thread::sleep(RETRY_DELAY);
                }
            }
        }
    }

    Err(ClipboardImportError::Platform(format!(
        "cannot open clipboard: {}",
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "unknown error".to_string())
    )))
}

#[cfg(windows)]
fn clipboard_format_available(format: u32) -> bool {
    unsafe { windows::Win32::System::DataExchange::IsClipboardFormatAvailable(format).is_ok() }
}

#[cfg(windows)]
fn clipboard_file_paths(hdrop: windows::Win32::UI::Shell::HDROP) -> Vec<PathBuf> {
    use windows::Win32::UI::Shell::DragQueryFileW;

    let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
    let mut paths = Vec::new();
    for index in 0..count {
        let len = unsafe { DragQueryFileW(hdrop, index, None) };
        if len == 0 {
            continue;
        }
        let mut buffer = vec![0u16; len as usize + 1];
        let written = unsafe { DragQueryFileW(hdrop, index, Some(&mut buffer)) };
        if written == 0 {
            continue;
        }
        paths.push(PathBuf::from(String::from_utf16_lossy(
            &buffer[..written as usize],
        )));
    }
    paths
}

#[cfg(windows)]
fn clipboard_dib_frame(format: u32) -> Result<Option<RgbFrame>, ClipboardImportError> {
    use std::slice;
    use windows::Win32::{
        Foundation::HGLOBAL,
        System::{
            DataExchange::GetClipboardData,
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        },
    };

    let handle = unsafe { GetClipboardData(format) }.map_err(|err| {
        ClipboardImportError::Platform(format!("cannot read clipboard image: {err}"))
    })?;
    let hglobal = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(hglobal) };
    if size == 0 {
        return Err(ClipboardImportError::Platform(
            "clipboard image data is empty".to_string(),
        ));
    }
    let ptr = unsafe { GlobalLock(hglobal) };
    if ptr.is_null() {
        return Err(ClipboardImportError::Platform(
            "cannot lock clipboard image data".to_string(),
        ));
    }
    let bytes = unsafe { slice::from_raw_parts(ptr.cast::<u8>(), size) }.to_vec();
    unsafe {
        let _ = GlobalUnlock(hglobal);
    }
    dib_to_rgb_frame(&bytes).map(Some)
}

#[cfg(not(windows))]
fn read_clipboard_template_images_platform() -> Result<ClipboardTemplateImages, ClipboardImportError>
{
    Err(ClipboardImportError::Platform(
        "clipboard image import is only implemented on Windows".to_string(),
    ))
}

fn is_supported_image_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|item| item.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "bmp" | "webp"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn dib_to_rgb_frame(dib: &[u8]) -> Result<RgbFrame, ClipboardImportError> {
    let decoder = image::codecs::bmp::BmpDecoder::new_without_file_header(Cursor::new(dib))
        .map_err(|err| ClipboardImportError::Decode(err.to_string()))?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|err| ClipboardImportError::Decode(err.to_string()))?
        .to_rgb8();
    RgbFrame::new(image.width(), image.height(), image.into_raw())
        .map_err(ClipboardImportError::Decode)
}

#[cfg(test)]
mod tests {
    use super::{dib_to_rgb_frame, is_supported_image_path};
    use std::path::Path;

    #[test]
    fn supported_image_path_matches_python_clipboard_extensions() {
        for path in ["one.png", "two.JPG", "three.jpeg", "four.bmp", "five.webp"] {
            assert!(is_supported_image_path(Path::new(path)), "{path}");
        }
        assert!(!is_supported_image_path(Path::new("notes.txt")));
        assert!(!is_supported_image_path(Path::new("no-extension")));
    }

    #[test]
    fn dib_to_rgb_frame_decodes_bottom_up_24bpp_rows() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&16u32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&[30, 20, 10, 60, 50, 40, 0, 0]);
        dib.extend_from_slice(&[90, 80, 70, 120, 110, 100, 0, 0]);

        let frame = dib_to_rgb_frame(&dib).unwrap();

        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 2);
        assert_eq!(
            frame.pixels,
            vec![
                70, 80, 90, 100, 110, 120, //
                10, 20, 30, 40, 50, 60,
            ]
        );
    }
}
