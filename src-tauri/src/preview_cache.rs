use screen_watch_core::detect::RgbFrame;
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

#[derive(Debug, Clone)]
struct CachedPreview {
    signature: String,
    frame: RgbFrame,
}

#[derive(Debug, Default)]
pub struct PreviewCacheState {
    previews: Mutex<HashMap<String, CachedPreview>>,
}

impl PreviewCacheState {
    pub fn frame_for(
        &self,
        key: impl Into<String>,
        signature: impl Into<String>,
        capture: impl FnOnce() -> Result<RgbFrame, String>,
    ) -> Result<(RgbFrame, bool), String> {
        let key = key.into();
        let signature = signature.into();
        let mut previews = self
            .previews
            .lock()
            .map_err(|_| "preview cache lock is poisoned".to_string())?;
        if let Some(cached) = previews.get(&key) {
            if cached.signature == signature {
                return Ok((cached.frame.clone(), true));
            }
        }

        let frame = capture()?;
        previews.insert(
            key,
            CachedPreview {
                signature,
                frame: frame.clone(),
            },
        );
        Ok((frame, false))
    }

    pub fn refresh_frame(
        &self,
        key: impl Into<String>,
        signature: impl Into<String>,
        capture: impl FnOnce() -> Result<RgbFrame, String>,
    ) -> Result<(RgbFrame, bool), String> {
        let key = key.into();
        let signature = signature.into();
        let frame = capture()?;
        let mut previews = self
            .previews
            .lock()
            .map_err(|_| "preview cache lock is poisoned".to_string())?;
        previews.insert(
            key,
            CachedPreview {
                signature,
                frame: frame.clone(),
            },
        );
        Ok((frame, false))
    }

    #[allow(dead_code)]
    pub fn retain_keys<'a>(&self, keys: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
        let keys = keys.into_iter().collect::<HashSet<_>>();
        let mut previews = self
            .previews
            .lock()
            .map_err(|_| "preview cache lock is poisoned".to_string())?;
        previews.retain(|key, _| keys.contains(key.as_str()));
        Ok(())
    }
}

pub fn screen_preview_key(
    source_key: Option<String>,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
) -> String {
    source_key.unwrap_or_else(|| screen_preview_signature(left, top, width, height))
}

pub fn screen_preview_signature(left: i32, top: i32, width: u32, height: u32) -> String {
    format!("screen:{left}:{top}:{width}:{height}")
}

pub fn window_preview_key(source_key: Option<String>, hwnd: isize) -> String {
    source_key.unwrap_or_else(|| format!("window:{hwnd}"))
}

pub fn window_preview_signature(
    hwnd: isize,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
) -> String {
    format!("window:{hwnd}:{left}:{top}:{width}:{height}")
}

#[cfg(test)]
mod tests {
    use super::{
        screen_preview_key, screen_preview_signature, window_preview_key, window_preview_signature,
        PreviewCacheState,
    };
    use screen_watch_core::detect::RgbFrame;
    use std::cell::Cell;

    #[test]
    fn cache_reuses_frame_when_key_and_signature_match() {
        let cache = PreviewCacheState::default();
        let captures = Cell::new(0);
        let (first, cached) = cache
            .frame_for("screen:monitor-1", "screen:1:0:0:10:10", || {
                captures.set(captures.get() + 1);
                Ok(solid(1, [10, 20, 30]))
            })
            .unwrap();
        assert!(!cached);

        let (second, cached) = cache
            .frame_for("screen:monitor-1", "screen:1:0:0:10:10", || {
                captures.set(captures.get() + 1);
                Ok(solid(1, [200, 200, 200]))
            })
            .unwrap();
        assert!(cached);
        assert_eq!(captures.get(), 1);
        assert_eq!(second, first);
    }

    #[test]
    fn cache_refreshes_frame_when_signature_changes_for_same_key() {
        let cache = PreviewCacheState::default();
        cache
            .frame_for("screen:monitor-2", "screen:2:0:0:10:10", || {
                Ok(solid(1, [10, 20, 30]))
            })
            .unwrap();

        let (frame, cached) = cache
            .frame_for("screen:monitor-2", "screen:2:0:0:20:20", || {
                Ok(solid(1, [90, 80, 70]))
            })
            .unwrap();
        assert!(!cached);
        assert_eq!(frame, solid(1, [90, 80, 70]));
    }

    #[test]
    fn refresh_frame_replaces_same_signature_cache() {
        let cache = PreviewCacheState::default();
        cache
            .frame_for("screen:monitor-1", "screen:1:0:0:10:10", || {
                Ok(solid(1, [10, 20, 30]))
            })
            .unwrap();

        let (frame, cached) = cache
            .refresh_frame("screen:monitor-1", "screen:1:0:0:10:10", || {
                Ok(solid(1, [90, 80, 70]))
            })
            .unwrap();

        assert!(!cached);
        assert_eq!(frame, solid(1, [90, 80, 70]));
        let (frame, cached) = cache
            .frame_for("screen:monitor-1", "screen:1:0:0:10:10", || {
                Ok(solid(1, [1, 2, 3]))
            })
            .unwrap();
        assert!(cached);
        assert_eq!(frame, solid(1, [90, 80, 70]));
    }

    #[test]
    fn cache_is_scoped_by_source_key() {
        let cache = PreviewCacheState::default();
        cache
            .frame_for("screen:monitor-1", "screen:1:0:0:10:10", || {
                Ok(solid(1, [10, 20, 30]))
            })
            .unwrap();
        let (frame, cached) = cache
            .frame_for("screen:monitor-2", "screen:1:0:0:10:10", || {
                Ok(solid(1, [1, 2, 3]))
            })
            .unwrap();
        assert!(!cached);
        assert_eq!(frame, solid(1, [1, 2, 3]));
    }

    #[test]
    fn retain_keys_drops_sources_that_are_no_longer_selected() {
        let cache = PreviewCacheState::default();
        cache
            .frame_for("screen:monitor-1", "screen:1:0:0:10:10", || {
                Ok(solid(1, [10, 20, 30]))
            })
            .unwrap();
        cache
            .frame_for("screen:monitor-2", "screen:2:0:0:10:10", || {
                Ok(solid(1, [1, 2, 3]))
            })
            .unwrap();
        cache.retain_keys(["screen:monitor-2"]).unwrap();

        let (_, cached) = cache
            .frame_for("screen:monitor-1", "screen:1:0:0:10:10", || {
                Ok(solid(1, [4, 5, 6]))
            })
            .unwrap();
        assert!(!cached);
        let (_, cached) = cache
            .frame_for("screen:monitor-2", "screen:2:0:0:10:10", || {
                Ok(solid(1, [7, 8, 9]))
            })
            .unwrap();
        assert!(cached);
    }

    #[test]
    fn preview_signatures_encode_source_geometry() {
        assert_eq!(
            screen_preview_key(Some("screen:monitor-2".to_string()), 1, 2, 3, 4),
            "screen:monitor-2"
        );
        assert_eq!(screen_preview_signature(1, 2, 3, 4), "screen:1:2:3:4");
        assert_eq!(window_preview_key(None, 42), "window:42");
        assert_eq!(
            window_preview_signature(42, 10, 20, 300, 200),
            "window:42:10:20:300:200"
        );
    }

    fn solid(size: u32, rgb: [u8; 3]) -> RgbFrame {
        let mut pixels = Vec::new();
        for _ in 0..size * size {
            pixels.extend_from_slice(&rgb);
        }
        RgbFrame::new(size, size, pixels).unwrap()
    }
}
