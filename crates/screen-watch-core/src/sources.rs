use crate::config::{RegionConfig, WatchConfig, WindowAppConfig, WindowConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorInfo {
    pub index: i32,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BBox {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedRegion {
    pub name: String,
    pub monitor: i32,
    pub bbox: BBox,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedSources {
    pub regions: Vec<ResolvedRegion>,
    pub windows: Vec<WindowConfig>,
    pub window_apps: Vec<WindowAppConfig>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceError {
    #[error("unknown monitor {0}; run list-monitors")]
    UnknownMonitor(i32),
    #[error("no screen or window sources are available")]
    EmptySources,
}

pub fn resolve_sources(
    config: &WatchConfig,
    monitors: &[MonitorInfo],
) -> Result<ResolvedSources, SourceError> {
    let has_window_sources = !config.windows.is_empty() || !config.window_apps.is_empty();
    let regions = if config.regions.is_empty() && has_window_sources {
        Vec::new()
    } else {
        resolve_regions(config, monitors)?
    };
    if regions.is_empty() && !has_window_sources {
        return Err(SourceError::EmptySources);
    }
    Ok(ResolvedSources {
        regions,
        windows: config.windows.clone(),
        window_apps: config.window_apps.clone(),
    })
}

pub fn resolve_regions(
    config: &WatchConfig,
    monitors: &[MonitorInfo],
) -> Result<Vec<ResolvedRegion>, SourceError> {
    let regions = if config.regions.is_empty() {
        monitors
            .iter()
            .filter(|monitor| monitor.index != 0)
            .map(default_region_for_monitor)
            .collect()
    } else {
        config.regions.clone()
    };

    regions
        .iter()
        .map(|region| resolve_region(region, monitors))
        .collect()
}

fn default_region_for_monitor(monitor: &MonitorInfo) -> RegionConfig {
    RegionConfig {
        name: format!("monitor-{}", monitor.index),
        monitor: monitor.index,
        left: 0,
        top: 0,
        width: None,
        height: None,
        extra: Default::default(),
    }
}

fn resolve_region(
    region: &RegionConfig,
    monitors: &[MonitorInfo],
) -> Result<ResolvedRegion, SourceError> {
    let monitor_id = region.monitor;
    let monitor = monitors
        .iter()
        .find(|item| item.index == monitor_id && item.index != 0)
        .ok_or(SourceError::UnknownMonitor(monitor_id))?;
    Ok(ResolvedRegion {
        name: if region.name.is_empty() {
            format!("monitor-{monitor_id}")
        } else {
            region.name.clone()
        },
        monitor: monitor_id,
        bbox: BBox {
            left: monitor.left + region.left,
            top: monitor.top + region.top,
            width: region.width.unwrap_or(monitor.width),
            height: region.height.unwrap_or(monitor.height),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{resolve_regions, resolve_sources, BBox, MonitorInfo, SourceError};
    use crate::config::WatchConfig;

    fn monitors() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo {
                index: 0,
                left: 0,
                top: 0,
                width: 5760,
                height: 2160,
            },
            MonitorInfo {
                index: 1,
                left: 0,
                top: 0,
                width: 1920,
                height: 1080,
            },
            MonitorInfo {
                index: 2,
                left: 1920,
                top: 0,
                width: 3840,
                height: 2160,
            },
        ]
    }

    #[test]
    fn empty_regions_default_to_physical_monitors_only() {
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]
            }"#,
        )
        .unwrap();
        let regions = resolve_regions(&config, &monitors()).unwrap();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].name, "monitor-1");
        assert_eq!(regions[1].bbox.width, 3840);
    }

    #[test]
    fn region_bbox_is_relative_to_monitor_origin() {
        let config = WatchConfig::from_json_str(
            r#"{
              "regions": [{"name":"right-crop","monitor":2,"left":100,"top":50,"width":800,"height":600}],
              "targets": [{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]
            }"#,
        )
        .unwrap();
        let regions = resolve_regions(&config, &monitors()).unwrap();
        assert_eq!(
            regions[0].bbox,
            BBox {
                left: 2020,
                top: 50,
                width: 800,
                height: 600,
            }
        );
    }

    #[test]
    fn monitor_zero_and_unknown_monitor_are_rejected() {
        let config = WatchConfig::from_json_str(
            r#"{
              "regions": [{"name":"virtual","monitor":0}],
              "targets": [{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]
            }"#,
        )
        .unwrap();
        assert_eq!(
            resolve_regions(&config, &monitors()),
            Err(SourceError::UnknownMonitor(0))
        );

        let config = WatchConfig::from_json_str(
            r#"{
              "regions": [{"name":"missing","monitor":9}],
              "targets": [{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]
            }"#,
        )
        .unwrap();
        assert_eq!(
            resolve_regions(&config, &monitors()),
            Err(SourceError::UnknownMonitor(9))
        );
    }

    #[test]
    fn resolve_sources_keeps_window_only_configs_without_defaulting_to_monitors() {
        let config = WatchConfig::from_json_str(
            r#"{
              "windows": [{"title":"Demo", "display":"Demo", "hwnd":123}],
              "window_apps": [{"title":"Demo", "ordinal":1}],
              "targets": [{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]
            }"#,
        )
        .unwrap();
        let sources = resolve_sources(&config, &monitors()).unwrap();
        assert!(sources.regions.is_empty());
        assert_eq!(sources.windows[0].title, "Demo");
        assert_eq!(sources.window_apps[0].ordinal, 1);
    }

    #[test]
    fn resolve_sources_defaults_to_monitors_when_no_explicit_sources_exist() {
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]
            }"#,
        )
        .unwrap();
        let sources = resolve_sources(&config, &monitors()).unwrap();
        assert_eq!(sources.regions.len(), 2);
        assert!(sources.windows.is_empty());
        assert!(sources.window_apps.is_empty());
    }

    #[test]
    fn resolve_sources_reports_missing_source_without_monitors_or_windows() {
        let config = WatchConfig::from_json_str(
            r#"{
              "targets": [{"kind":"pixel","name":"p","x":1,"y":1,"rgb":[1,2,3]}]
            }"#,
        )
        .unwrap();
        assert_eq!(
            resolve_sources(&config, &[]),
            Err(SourceError::EmptySources)
        );
    }
}
