use crate::{
    audio::AlarmBeepState,
    screen_capture::{capture_screen_region, CaptureRegion},
    window_capture::{capture_window_frame, WindowCaptureModeCache},
    window_sources,
};
use screen_watch_core::{
    config::{WatchConfig, WindowAppConfig, WindowConfig},
    evidence::safe_name,
    ocr::{create_ocr_backend, ocr_unavailable_reason_for_config, OcrSettings},
    scan::{ScanEngine, ScanFrameResult},
    sources::ResolvedRegion,
};
use serde::Serialize;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MIN_POLL_INTERVAL: Duration = Duration::from_millis(120);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(25);
const STOP_JOIN_GRACE: Duration = Duration::from_millis(75);
const START_STOP_JOIN_GRACE: Duration = Duration::from_millis(1200);
const WINDOW_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
pub const MONITOR_SESSION_EVENT: &str = "screen-watch://monitor-session";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSessionSnapshot {
    pub generation: u64,
    pub running: bool,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub last_tick: Option<String>,
    pub tick_count: u64,
    pub hit_count: u64,
    pub last_tick_match_count: u64,
    pub last_tick_hit_count: u64,
    pub last_tick_scan_ms: u64,
    pub error_count: u64,
    pub last_error: Option<String>,
    pub region_count: usize,
    pub window_count: usize,
    pub skipped_windows: usize,
    pub skipped_window_apps: usize,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MonitorSessionEventKind {
    Started,
    Tick,
    Stopped,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSessionEvent {
    pub kind: MonitorSessionEventKind,
    pub snapshot: MonitorSessionSnapshot,
    pub tick_match_count: u64,
    pub tick_hit_count: u64,
    pub tick_scan_ms: u64,
    pub tick_error: Option<String>,
}

pub trait MonitorSessionEventSink: Send + Sync + 'static {
    fn emit(&self, event: MonitorSessionEvent);
}

pub trait MonitorSessionHitSink: Send + Sync + 'static {
    fn record(&self, target_ids: &[String]) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct MonitorSessionState {
    worker: Mutex<Option<MonitorWorker>>,
    snapshot: Arc<Mutex<MonitorSessionSnapshot>>,
    active_generation: Arc<AtomicU64>,
}

#[derive(Debug)]
struct MonitorWorker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
    generation: u64,
}

impl Default for MonitorSessionSnapshot {
    fn default() -> Self {
        Self {
            generation: 0,
            running: false,
            started_at: None,
            stopped_at: None,
            last_tick: None,
            tick_count: 0,
            hit_count: 0,
            last_tick_match_count: 0,
            last_tick_hit_count: 0,
            last_tick_scan_ms: 0,
            error_count: 0,
            last_error: None,
            region_count: 0,
            window_count: 0,
            skipped_windows: 0,
            skipped_window_apps: 0,
            poll_interval_ms: MIN_POLL_INTERVAL.as_millis() as u64,
        }
    }
}

impl MonitorSessionState {
    pub fn start_sources_with_events(
        &self,
        config: WatchConfig,
        base_dir: PathBuf,
        regions: Vec<ResolvedRegion>,
        windows: Vec<WindowConfig>,
        window_apps: Vec<WindowAppConfig>,
        beeper: AlarmBeepState,
        hit_sink: Option<Arc<dyn MonitorSessionHitSink>>,
        event_sink: Arc<dyn MonitorSessionEventSink>,
    ) -> Result<MonitorSessionSnapshot, String> {
        if regions.is_empty() && windows.is_empty() && window_apps.is_empty() {
            return Err("monitoring session has no screen or window sources".to_string());
        }
        let settings = OcrSettings::from_env();
        if let Some(reason) = ocr_unavailable_reason_for_config(&config, &settings) {
            return Err(reason);
        }
        self.reap_finished_worker()?;
        self.request_stop_current_worker(START_STOP_JOIN_GRACE, false)?;

        let poll_interval = poll_interval_duration(config.poll_interval_seconds);
        let min_idle = min_idle_duration(&config);
        let mut engine = ScanEngine::new_with_ocr_backend(
            config,
            &base_dir,
            &base_dir,
            create_ocr_backend(&settings),
        )
        .map_err(|err| err.to_string())?;
        let mut sources = MonitorSources::new(regions, windows, window_apps);
        if let Err(err) = sources.refresh_remembered_windows() {
            sources.last_error = Some(err);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let snapshot = Arc::clone(&self.snapshot);
        let active_generation = Arc::clone(&self.active_generation);
        let generation = active_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let thread_stop = Arc::clone(&stop);
        let started_at = clock_now().time_text;
        let initial = MonitorSessionSnapshot {
            generation,
            running: true,
            started_at: Some(started_at),
            stopped_at: None,
            last_tick: None,
            tick_count: 0,
            hit_count: 0,
            last_tick_match_count: 0,
            last_tick_hit_count: 0,
            last_tick_scan_ms: 0,
            error_count: 0,
            last_error: sources.last_error.clone(),
            region_count: sources.regions.len(),
            window_count: sources.window_count(),
            skipped_windows: sources.skipped_windows(),
            skipped_window_apps: sources.skipped_window_apps,
            poll_interval_ms: poll_interval.as_millis() as u64,
        };
        *self
            .snapshot
            .lock()
            .map_err(|_| "monitor session snapshot is poisoned".to_string())? = initial.clone();
        event_sink.emit(started_event(initial.clone()));

        let handle = thread::spawn(move || {
            monitor_loop(
                &mut engine,
                sources,
                poll_interval,
                min_idle,
                thread_stop,
                snapshot,
                active_generation,
                generation,
                beeper,
                hit_sink,
                event_sink,
            );
        });

        *self
            .worker
            .lock()
            .map_err(|_| "monitor session worker is poisoned".to_string())? = Some(MonitorWorker {
            stop,
            handle,
            generation,
        });
        Ok(initial)
    }

    pub fn stop(&self) -> Result<MonitorSessionSnapshot, String> {
        self.request_stop_current_worker(STOP_JOIN_GRACE, true)?;
        self.snapshot()
    }

    pub fn snapshot(&self) -> Result<MonitorSessionSnapshot, String> {
        self.reap_finished_worker()?;
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| "monitor session snapshot is poisoned".to_string())
    }

    fn request_stop_current_worker(
        &self,
        join_grace: Duration,
        retain_stopping_worker: bool,
    ) -> Result<bool, String> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| "monitor session worker is poisoned".to_string())?
            .take();
        let mut still_stopping = false;
        if let Some(worker) = worker {
            worker.stop.store(true, Ordering::SeqCst);
            if let Some(worker) = join_worker_if_finished(worker, join_grace)? {
                still_stopping = true;
                if retain_stopping_worker {
                    *self
                        .worker
                        .lock()
                        .map_err(|_| "monitor session worker is poisoned".to_string())? =
                        Some(worker);
                }
            }
        }
        mark_stopped(&self.snapshot)?;
        Ok(still_stopping)
    }

    fn reap_finished_worker(&self) -> Result<(), String> {
        let should_reap = self
            .worker
            .lock()
            .map_err(|_| "monitor session worker is poisoned".to_string())?
            .as_ref()
            .map(|worker| worker.handle.is_finished())
            .unwrap_or(false);
        if should_reap {
            let worker = self
                .worker
                .lock()
                .map_err(|_| "monitor session worker is poisoned".to_string())?
                .take();
            if let Some(worker) = worker {
                let generation = worker.generation;
                worker
                    .handle
                    .join()
                    .map_err(|_| "monitor session thread panicked".to_string())?;
                mark_stopped_if_active(&self.snapshot, &self.active_generation, generation)?;
            }
        }
        Ok(())
    }
}

fn join_worker_if_finished(
    worker: MonitorWorker,
    join_grace: Duration,
) -> Result<Option<MonitorWorker>, String> {
    let started = Instant::now();
    loop {
        if worker.handle.is_finished() {
            worker
                .handle
                .join()
                .map_err(|_| "monitor session thread panicked".to_string())?;
            return Ok(None);
        }
        if started.elapsed() >= join_grace {
            return Ok(Some(worker));
        }
        thread::sleep(STOP_POLL_INTERVAL.min(join_grace.saturating_sub(started.elapsed())));
    }
}

fn monitor_loop(
    engine: &mut ScanEngine,
    mut sources: MonitorSources,
    poll_interval: Duration,
    min_idle: Duration,
    stop: Arc<AtomicBool>,
    snapshot: Arc<Mutex<MonitorSessionSnapshot>>,
    active_generation: Arc<AtomicU64>,
    generation: u64,
    beeper: AlarmBeepState,
    hit_sink: Option<Arc<dyn MonitorSessionHitSink>>,
    event_sink: Arc<dyn MonitorSessionEventSink>,
) {
    let mut window_modes = WindowCaptureModeCache::default();
    while !stop.load(Ordering::SeqCst) && active_generation.load(Ordering::SeqCst) == generation {
        let tick_started = Instant::now();
        let clock = clock_now();
        let mut tick_matches = 0usize;
        let mut tick_hits = 0usize;
        let mut tick_target_ids = Vec::new();
        let mut tick_error: Option<String> = None;
        if let Err(err) = sources.refresh_remembered_windows_if_due() {
            tick_error = Some(err);
        }
        for region in &sources.regions {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            match scan_one_region(engine, region, &clock) {
                Ok(result) => {
                    tick_matches += result.matches.len();
                    tick_target_ids.extend(alerted_target_ids(&result));
                    tick_hits += result.alerted_matches.len();
                }
                Err(err) => {
                    tick_error = Some(err);
                }
            }
        }
        for window in sources.concrete_windows() {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            match scan_one_window(engine, window, &clock, &mut window_modes) {
                Ok(Some(result)) => {
                    tick_matches += result.matches.len();
                    tick_target_ids.extend(alerted_target_ids(&result));
                    tick_hits += result.alerted_matches.len();
                }
                Ok(None) => {}
                Err(err) => {
                    tick_error = Some(err);
                }
            }
        }
        if stop.load(Ordering::SeqCst) || active_generation.load(Ordering::SeqCst) != generation {
            break;
        }
        if tick_hits > 0 {
            if let Some(hit_sink) = &hit_sink {
                if let Err(err) = hit_sink.record(&tick_target_ids) {
                    tick_error = Some(err);
                }
            }
            beeper.start_for_alarm(engine.alarm_config());
        }
        if let Some(event) = record_monitor_tick(
            &snapshot,
            &active_generation,
            generation,
            &stop,
            &clock,
            &sources,
            tick_matches as u64,
            tick_hits as u64,
            duration_ms(tick_started.elapsed()),
            tick_error,
        ) {
            event_sink.emit(event);
        }
        sleep_interruptibly(
            remaining_poll_interval(poll_interval, tick_started.elapsed()).max(min_idle),
            &stop,
        );
    }
    if let Ok(Some(snapshot)) = mark_stopped_if_active(&snapshot, &active_generation, generation) {
        event_sink.emit(stopped_event(snapshot));
    }
}

pub fn alerted_target_ids(result: &ScanFrameResult) -> Vec<String> {
    result
        .alerted_matches
        .iter()
        .map(|item| item.target_id.clone())
        .collect()
}

#[derive(Debug, Clone)]
struct MonitorSources {
    regions: Vec<ResolvedRegion>,
    direct_windows: Vec<WindowConfig>,
    window_apps: Vec<WindowAppConfig>,
    remembered_windows: Vec<WindowConfig>,
    skipped_window_apps: usize,
    last_window_refresh: Option<Instant>,
    last_error: Option<String>,
}

impl MonitorSources {
    fn new(
        regions: Vec<ResolvedRegion>,
        direct_windows: Vec<WindowConfig>,
        window_apps: Vec<WindowAppConfig>,
    ) -> Self {
        Self {
            regions,
            direct_windows,
            window_apps,
            remembered_windows: Vec::new(),
            skipped_window_apps: 0,
            last_window_refresh: None,
            last_error: None,
        }
    }

    fn refresh_remembered_windows_if_due(&mut self) -> Result<(), String> {
        if self
            .last_window_refresh
            .map(|last| last.elapsed() < WINDOW_REFRESH_INTERVAL)
            .unwrap_or(false)
        {
            return Ok(());
        }
        self.refresh_remembered_windows()
    }

    fn refresh_remembered_windows(&mut self) -> Result<(), String> {
        self.last_window_refresh = Some(Instant::now());
        if self.window_apps.is_empty() {
            self.remembered_windows.clear();
            self.skipped_window_apps = 0;
            return Ok(());
        }
        let available = window_sources::list_app_windows()?;
        let resolution = window_sources::resolve_window_apps(&self.window_apps, &available);
        self.remembered_windows = resolution.windows;
        self.skipped_window_apps = resolution.missing_window_apps.len();
        Ok(())
    }

    fn concrete_windows(&self) -> Vec<&WindowConfig> {
        let mut seen = HashSet::new();
        self.direct_windows
            .iter()
            .chain(self.remembered_windows.iter())
            .filter(|window| window.hwnd.is_some())
            .filter(|window| seen.insert(window.hwnd.unwrap()))
            .collect()
    }

    fn window_count(&self) -> usize {
        self.concrete_windows().len()
    }

    fn skipped_windows(&self) -> usize {
        self.direct_windows
            .iter()
            .filter(|window| window.hwnd.is_none())
            .count()
    }
}

fn scan_one_region(
    engine: &mut ScanEngine,
    region: &ResolvedRegion,
    clock: &MonitorClock,
) -> Result<ScanFrameResult, String> {
    let frame = capture_screen_region(CaptureRegion {
        left: region.bbox.left,
        top: region.bbox.top,
        width: region.bbox.width,
        height: region.bbox.height,
    })
    .map_err(|err| err.to_string())?;
    engine
        .scan_region_frame(
            &region.name,
            &frame,
            clock.now_seconds,
            &clock.time_text,
            &format!("{}-{}", clock.stamp, region.monitor),
        )
        .map_err(|err| err.to_string())
}

fn scan_one_window(
    engine: &mut ScanEngine,
    window: &WindowConfig,
    clock: &MonitorClock,
    mode_cache: &mut WindowCaptureModeCache,
) -> Result<Option<ScanFrameResult>, String> {
    let Some(hwnd) = window.hwnd else {
        return Ok(None);
    };
    let Some(frame) =
        capture_window_frame(hwnd, Some(mode_cache)).map_err(|err| err.to_string())?
    else {
        return Ok(None);
    };
    let name = window_source_name(window);
    let stamp = format!("{}-window-{}", clock.stamp, safe_name(&name));
    engine
        .scan_region_frame(&name, &frame, clock.now_seconds, &clock.time_text, &stamp)
        .map(Some)
        .map_err(|err| err.to_string())
}

pub(crate) fn window_source_name(window: &WindowConfig) -> String {
    if !window.name.trim().is_empty() {
        return window.name.clone();
    }
    if !window.display.trim().is_empty() {
        return format!("app-{}", safe_name(&window.display));
    }
    if !window.title.trim().is_empty() {
        return format!("app-{}", safe_name(&window.title));
    }
    "app-window".to_string()
}

fn mark_stopped(
    snapshot: &Arc<Mutex<MonitorSessionSnapshot>>,
) -> Result<Option<MonitorSessionSnapshot>, String> {
    let mut status = snapshot
        .lock()
        .map_err(|_| "monitor session snapshot is poisoned".to_string())?;
    if status.running {
        status.running = false;
        status.stopped_at = Some(clock_now().time_text);
        return Ok(Some(status.clone()));
    }
    Ok(None)
}

fn mark_stopped_if_active(
    snapshot: &Arc<Mutex<MonitorSessionSnapshot>>,
    active_generation: &AtomicU64,
    generation: u64,
) -> Result<Option<MonitorSessionSnapshot>, String> {
    if active_generation.load(Ordering::SeqCst) != generation {
        return Ok(None);
    }
    mark_stopped(snapshot)
}

fn started_event(snapshot: MonitorSessionSnapshot) -> MonitorSessionEvent {
    MonitorSessionEvent {
        kind: MonitorSessionEventKind::Started,
        snapshot,
        tick_match_count: 0,
        tick_hit_count: 0,
        tick_scan_ms: 0,
        tick_error: None,
    }
}

fn stopped_event(snapshot: MonitorSessionSnapshot) -> MonitorSessionEvent {
    MonitorSessionEvent {
        kind: MonitorSessionEventKind::Stopped,
        snapshot,
        tick_match_count: 0,
        tick_hit_count: 0,
        tick_scan_ms: 0,
        tick_error: None,
    }
}

fn record_monitor_tick(
    snapshot: &Arc<Mutex<MonitorSessionSnapshot>>,
    active_generation: &AtomicU64,
    generation: u64,
    stop: &AtomicBool,
    clock: &MonitorClock,
    sources: &MonitorSources,
    tick_matches: u64,
    tick_hits: u64,
    tick_scan_ms: u64,
    tick_error: Option<String>,
) -> Option<MonitorSessionEvent> {
    if active_generation.load(Ordering::SeqCst) != generation {
        return None;
    }
    let mut status = snapshot.lock().ok()?;
    if active_generation.load(Ordering::SeqCst) != generation {
        return None;
    }
    status.running = !stop.load(Ordering::SeqCst);
    status.last_tick = Some(clock.time_text.clone());
    status.tick_count = status.tick_count.saturating_add(1);
    status.hit_count = status.hit_count.saturating_add(tick_hits);
    status.last_tick_match_count = tick_matches;
    status.last_tick_hit_count = tick_hits;
    status.last_tick_scan_ms = tick_scan_ms;
    status.region_count = sources.regions.len();
    status.window_count = sources.window_count();
    status.skipped_windows = sources.skipped_windows();
    status.skipped_window_apps = sources.skipped_window_apps;
    if let Some(err) = tick_error.clone() {
        status.error_count = status.error_count.saturating_add(1);
        status.last_error = Some(err);
    }
    Some(MonitorSessionEvent {
        kind: MonitorSessionEventKind::Tick,
        snapshot: status.clone(),
        tick_match_count: tick_matches,
        tick_hit_count: tick_hits,
        tick_scan_ms,
        tick_error,
    })
}

fn sleep_interruptibly(duration: Duration, stop: &AtomicBool) {
    let mut slept = Duration::ZERO;
    while slept < duration && !stop.load(Ordering::SeqCst) {
        let remaining = duration.saturating_sub(slept);
        let step = remaining.min(STOP_POLL_INTERVAL);
        thread::sleep(step);
        slept += step;
    }
}

fn remaining_poll_interval(poll_interval: Duration, elapsed: Duration) -> Duration {
    poll_interval.saturating_sub(elapsed)
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn poll_interval_duration(seconds: f64) -> Duration {
    if !seconds.is_finite() || seconds <= 0.0 {
        return MIN_POLL_INTERVAL;
    }
    Duration::from_secs_f64(seconds).max(MIN_POLL_INTERVAL)
}

fn min_idle_duration(config: &WatchConfig) -> Duration {
    config
        .extra
        .get("min_idle_seconds")
        .and_then(|value| value.as_f64())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(Duration::from_secs_f64)
        .unwrap_or(Duration::ZERO)
}

#[derive(Debug, Clone, PartialEq)]
struct MonitorClock {
    now_seconds: f64,
    time_text: String,
    stamp: String,
}

fn clock_now() -> MonitorClock {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = elapsed.as_secs();
    let millis = elapsed.subsec_millis();
    MonitorClock {
        now_seconds: secs as f64 + f64::from(millis) / 1000.0,
        time_text: format!("unix-{secs}.{millis:03}"),
        stamp: format!("unix-{secs}-{millis:03}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        alerted_target_ids, mark_stopped, min_idle_duration, poll_interval_duration,
        record_monitor_tick, remaining_poll_interval, started_event, stopped_event,
        window_source_name, AlarmBeepState, MonitorClock, MonitorSessionEventKind,
        MonitorSessionSnapshot, MonitorSessionState, MonitorSources, MonitorWorker,
        MIN_POLL_INTERVAL, START_STOP_JOIN_GRACE,
    };
    use screen_watch_core::{
        config::{WatchConfig, WindowAppConfig, WindowConfig},
        detect::Match,
        scan::ScanFrameResult,
        sources::{BBox, ResolvedRegion},
    };
    use std::{
        fs,
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    #[derive(Debug, Default)]
    struct TestEventSink;

    impl super::MonitorSessionEventSink for TestEventSink {
        fn emit(&self, _event: super::MonitorSessionEvent) {}
    }

    #[test]
    fn poll_interval_has_safe_floor() {
        assert_eq!(poll_interval_duration(0.001), MIN_POLL_INTERVAL);
        assert_eq!(poll_interval_duration(-1.0), MIN_POLL_INTERVAL);
        assert_eq!(poll_interval_duration(f64::NAN), MIN_POLL_INTERVAL);
        assert_eq!(poll_interval_duration(0.25), Duration::from_millis(250));
    }

    #[test]
    fn remaining_poll_interval_subtracts_scan_time_from_sleep() {
        assert_eq!(
            remaining_poll_interval(Duration::from_millis(1200), Duration::from_millis(450)),
            Duration::from_millis(750)
        );
    }

    #[test]
    fn remaining_poll_interval_skips_sleep_when_scan_exceeds_interval() {
        assert_eq!(
            remaining_poll_interval(Duration::from_millis(1200), Duration::from_millis(1500)),
            Duration::ZERO
        );
    }

    #[test]
    fn profile_min_idle_is_applied_from_config_extra() {
        let config =
            WatchConfig::from_json_str(r#"{"targets":[],"min_idle_seconds":0.08}"#).unwrap();
        assert_eq!(min_idle_duration(&config), Duration::from_millis(80));
    }

    #[test]
    fn stop_without_running_session_is_safe() {
        let session = MonitorSessionState::default();
        let status = session.stop().unwrap();
        assert!(!status.running);
        assert_eq!(status.tick_count, 0);
    }

    #[test]
    fn stop_keeps_slow_stopping_worker_for_later_reap() {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }
            thread::sleep(Duration::from_millis(150));
        });
        let session = MonitorSessionState {
            worker: Mutex::new(Some(MonitorWorker {
                stop,
                handle,
                generation: 1,
            })),
            snapshot: Arc::new(Mutex::new(MonitorSessionSnapshot {
                running: true,
                started_at: Some("start".to_string()),
                ..MonitorSessionSnapshot::default()
            })),
            active_generation: Arc::new(AtomicU64::new(1)),
        };

        let started = Instant::now();
        let status = session.stop().unwrap();

        assert!(started.elapsed() < Duration::from_millis(100));
        assert!(!status.running);
        assert!(session.worker.lock().unwrap().is_some());
        for _ in 0..20 {
            if session.snapshot().unwrap().tick_count == 0
                && session.worker.lock().unwrap().is_none()
            {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(session.worker.lock().unwrap().is_none());
    }

    #[test]
    fn start_replaces_previous_worker_that_is_still_stopping() {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }
            thread::sleep(START_STOP_JOIN_GRACE + Duration::from_millis(75));
        });
        let session = MonitorSessionState {
            worker: Mutex::new(Some(MonitorWorker {
                stop,
                handle,
                generation: 1,
            })),
            snapshot: Arc::new(Mutex::new(MonitorSessionSnapshot {
                running: true,
                started_at: Some("start".to_string()),
                ..MonitorSessionSnapshot::default()
            })),
            active_generation: Arc::new(AtomicU64::new(1)),
        };
        let config = screen_watch_core::config::WatchConfig::from_json_str(
            r#"{"targets":[{"kind":"pixel","name":"p","x":0,"y":0,"rgb":[0,0,0]}]}"#,
        )
        .unwrap();

        let status = session
            .start_sources_with_events(
                config,
                std::env::temp_dir(),
                Vec::new(),
                Vec::new(),
                vec![WindowAppConfig {
                    title: "missing app window".to_string(),
                    ordinal: 1,
                    extra: Default::default(),
                }],
                AlarmBeepState::default(),
                None,
                Arc::new(TestEventSink),
            )
            .unwrap();

        assert!(status.running);
        assert!(status.generation > 1);
        assert!(session.worker.lock().unwrap().is_some());
        assert!(!session.stop().unwrap().running);
    }

    #[test]
    fn stale_worker_tick_does_not_overwrite_new_session_snapshot() {
        let snapshot = Arc::new(Mutex::new(MonitorSessionSnapshot {
            running: true,
            started_at: Some("new-start".to_string()),
            tick_count: 7,
            hit_count: 2,
            ..MonitorSessionSnapshot::default()
        }));
        let active_generation = AtomicU64::new(2);
        let sources = MonitorSources::new(vec![resolved_region("old")], Vec::new(), Vec::new());
        let stop = AtomicBool::new(true);
        let clock = MonitorClock {
            now_seconds: 8.0,
            time_text: "old-tick".to_string(),
            stamp: "old-stamp".to_string(),
        };

        let event = record_monitor_tick(
            &snapshot,
            &active_generation,
            1,
            &stop,
            &clock,
            &sources,
            101,
            99,
            1234,
            Some("old error".to_string()),
        );

        assert_eq!(event, None);
        let status = snapshot.lock().unwrap().clone();
        assert!(status.running);
        assert_eq!(status.started_at.as_deref(), Some("new-start"));
        assert_eq!(status.tick_count, 7);
        assert_eq!(status.hit_count, 2);
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn start_rejects_empty_screen_regions() {
        let session = MonitorSessionState::default();
        let config = screen_watch_core::config::WatchConfig::from_json_str(
            r#"{"targets":[{"kind":"pixel","name":"p","x":0,"y":0,"rgb":[0,0,0]}]}"#,
        )
        .unwrap();
        let err = session
            .start_sources_with_events(
                config,
                std::env::temp_dir(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                AlarmBeepState::default(),
                None,
                Arc::new(TestEventSink),
            )
            .unwrap_err();
        assert!(err.contains("no screen or window sources"));
        assert!(!session.snapshot().unwrap().running);
    }

    #[cfg(not(feature = "ocr"))]
    #[test]
    fn start_rejects_ocr_targets_before_starting_lite_worker() {
        let session = MonitorSessionState::default();
        let config = screen_watch_core::config::WatchConfig::from_json_str(
            r#"{"targets":[{"kind":"ocr_text","name":"ready","text":"READY"}]}"#,
        )
        .unwrap();

        let err = session
            .start_sources_with_events(
                config,
                std::env::temp_dir(),
                vec![ResolvedRegion {
                    name: "screen".to_string(),
                    monitor: 1,
                    bbox: BBox {
                        left: 0,
                        top: 0,
                        width: 16,
                        height: 16,
                    },
                }],
                Vec::new(),
                Vec::new(),
                AlarmBeepState::default(),
                None,
                Arc::new(TestEventSink),
            )
            .unwrap_err();

        assert!(err.contains("OCR target requires an available OCR backend"));
        assert!(err.contains("lite build: OCR module disabled"));
        assert!(!session.snapshot().unwrap().running);
        assert!(session.worker.lock().unwrap().is_none());
    }

    #[test]
    fn record_monitor_tick_updates_snapshot_and_builds_event_payload() {
        let snapshot = Arc::new(Mutex::new(MonitorSessionSnapshot {
            running: true,
            started_at: Some("start".to_string()),
            region_count: 2,
            window_count: 1,
            ..MonitorSessionSnapshot::default()
        }));
        let sources = MonitorSources::new(
            vec![resolved_region("one"), resolved_region("two")],
            vec![window("Demo", Some(123))],
            Vec::new(),
        );
        let stop = AtomicBool::new(false);
        let clock = MonitorClock {
            now_seconds: 7.0,
            time_text: "tick-time".to_string(),
            stamp: "tick-stamp".to_string(),
        };

        let event = record_monitor_tick(
            &snapshot,
            &AtomicU64::new(0),
            0,
            &stop,
            &clock,
            &sources,
            5,
            3,
            456,
            Some("capture failed".to_string()),
        )
        .unwrap();

        assert_eq!(event.kind, MonitorSessionEventKind::Tick);
        assert_eq!(event.tick_match_count, 5);
        assert_eq!(event.tick_hit_count, 3);
        assert_eq!(event.tick_scan_ms, 456);
        assert_eq!(event.tick_error.as_deref(), Some("capture failed"));
        assert!(event.snapshot.running);
        assert_eq!(event.snapshot.last_tick.as_deref(), Some("tick-time"));
        assert_eq!(event.snapshot.tick_count, 1);
        assert_eq!(event.snapshot.hit_count, 3);
        assert_eq!(event.snapshot.last_tick_match_count, 5);
        assert_eq!(event.snapshot.last_tick_hit_count, 3);
        assert_eq!(event.snapshot.last_tick_scan_ms, 456);
        assert_eq!(event.snapshot.error_count, 1);
        assert_eq!(event.snapshot.last_error.as_deref(), Some("capture failed"));
        assert_eq!(event.snapshot.region_count, 2);
        assert_eq!(event.snapshot.window_count, 1);
    }

    #[test]
    fn record_monitor_tick_marks_snapshot_not_running_when_stop_is_set() {
        let snapshot = Arc::new(Mutex::new(MonitorSessionSnapshot {
            running: true,
            ..MonitorSessionSnapshot::default()
        }));
        let sources = MonitorSources::new(Vec::new(), Vec::new(), Vec::new());
        let stop = AtomicBool::new(true);
        let clock = MonitorClock {
            now_seconds: 8.0,
            time_text: "tick-time".to_string(),
            stamp: "tick-stamp".to_string(),
        };

        let event = record_monitor_tick(
            &snapshot,
            &AtomicU64::new(0),
            0,
            &stop,
            &clock,
            &sources,
            0,
            0,
            0,
            None,
        )
        .unwrap();

        assert_eq!(event.kind, MonitorSessionEventKind::Tick);
        assert!(!event.snapshot.running);
        assert_eq!(event.snapshot.tick_count, 1);
    }

    #[test]
    fn record_monitor_tick_updates_skipped_source_counts_and_keeps_last_error() {
        let snapshot = Arc::new(Mutex::new(MonitorSessionSnapshot {
            running: true,
            error_count: 1,
            last_error: Some("previous capture failure".to_string()),
            ..MonitorSessionSnapshot::default()
        }));
        let mut sources = MonitorSources::new(
            vec![resolved_region("screen")],
            vec![window("ready", Some(7)), window("missing", None)],
            Vec::new(),
        );
        sources.remembered_windows = vec![window("remembered", Some(8))];
        sources.skipped_window_apps = 2;
        let stop = AtomicBool::new(false);
        let clock = MonitorClock {
            now_seconds: 9.0,
            time_text: "tick-time".to_string(),
            stamp: "tick-stamp".to_string(),
        };

        let event = record_monitor_tick(
            &snapshot,
            &AtomicU64::new(0),
            0,
            &stop,
            &clock,
            &sources,
            0,
            0,
            0,
            None,
        )
        .unwrap();

        assert_eq!(event.snapshot.region_count, 1);
        assert_eq!(event.snapshot.window_count, 2);
        assert_eq!(event.snapshot.skipped_windows, 1);
        assert_eq!(event.snapshot.skipped_window_apps, 2);
        assert_eq!(event.snapshot.error_count, 1);
        assert_eq!(
            event.snapshot.last_error.as_deref(),
            Some("previous capture failure")
        );
        assert_eq!(event.tick_error, None);
    }

    #[test]
    fn monitor_session_event_serializes_frontend_status_contract_as_camel_case() {
        let snapshot = Arc::new(Mutex::new(MonitorSessionSnapshot {
            running: true,
            started_at: Some("start-time".to_string()),
            poll_interval_ms: 250,
            ..MonitorSessionSnapshot::default()
        }));
        let mut sources = MonitorSources::new(
            vec![resolved_region("screen")],
            vec![window("ready", Some(7)), window("missing", None)],
            Vec::new(),
        );
        sources.remembered_windows = vec![window("remembered", Some(8))];
        sources.skipped_window_apps = 2;
        let stop = AtomicBool::new(false);
        let clock = MonitorClock {
            now_seconds: 9.0,
            time_text: "tick-time".to_string(),
            stamp: "tick-stamp".to_string(),
        };

        let event = record_monitor_tick(
            &snapshot,
            &AtomicU64::new(0),
            0,
            &stop,
            &clock,
            &sources,
            6,
            4,
            789,
            Some("capture failed".to_string()),
        )
        .unwrap();
        let value = serde_json::to_value(event).unwrap();

        assert_eq!(value["kind"], "tick");
        assert_eq!(value["tickMatchCount"], 6);
        assert_eq!(value["tickHitCount"], 4);
        assert_eq!(value["tickScanMs"], 789);
        assert_eq!(value["tickError"], "capture failed");
        assert!(value.get("tick_match_count").is_none());
        assert!(value.get("tick_hit_count").is_none());
        assert!(value.get("tick_scan_ms").is_none());
        assert!(value.get("tick_error").is_none());

        let snapshot = value["snapshot"].as_object().unwrap();
        for field in [
            "generation",
            "running",
            "startedAt",
            "stoppedAt",
            "lastTick",
            "tickCount",
            "hitCount",
            "lastTickMatchCount",
            "lastTickHitCount",
            "lastTickScanMs",
            "errorCount",
            "lastError",
            "regionCount",
            "windowCount",
            "skippedWindows",
            "skippedWindowApps",
            "pollIntervalMs",
        ] {
            assert!(
                snapshot.contains_key(field),
                "missing snapshot field {field}"
            );
        }
        for snake_case_field in [
            "started_at",
            "stopped_at",
            "last_tick",
            "tick_count",
            "hit_count",
            "last_tick_match_count",
            "last_tick_hit_count",
            "last_tick_scan_ms",
            "error_count",
            "last_error",
            "region_count",
            "window_count",
            "skipped_windows",
            "skipped_window_apps",
            "poll_interval_ms",
        ] {
            assert!(
                !snapshot.contains_key(snake_case_field),
                "unexpected snake_case snapshot field {snake_case_field}"
            );
        }
        assert_eq!(snapshot["startedAt"], "start-time");
        assert_eq!(snapshot["generation"], 0);
        assert_eq!(snapshot["lastTick"], "tick-time");
        assert_eq!(snapshot["tickCount"], 1);
        assert_eq!(snapshot["hitCount"], 4);
        assert_eq!(snapshot["lastTickMatchCount"], 6);
        assert_eq!(snapshot["lastTickHitCount"], 4);
        assert_eq!(snapshot["lastTickScanMs"], 789);
        assert_eq!(snapshot["errorCount"], 1);
        assert_eq!(snapshot["lastError"], "capture failed");
        assert_eq!(snapshot["regionCount"], 1);
        assert_eq!(snapshot["windowCount"], 2);
        assert_eq!(snapshot["skippedWindows"], 1);
        assert_eq!(snapshot["skippedWindowApps"], 2);
        assert_eq!(snapshot["pollIntervalMs"], 250);
    }

    #[test]
    fn monitor_sources_counts_direct_windows_and_missing_handles() {
        let sources = MonitorSources::new(
            vec![resolved_region("screen")],
            vec![window("ready", Some(7)), window("missing", None)],
            Vec::new(),
        );
        assert_eq!(sources.window_count(), 1);
        assert_eq!(sources.skipped_windows(), 1);
    }

    #[test]
    fn alerted_target_ids_uses_cooldown_filtered_matches() {
        let result = ScanFrameResult {
            region: "screen".to_string(),
            matches: vec![scan_match("raw")],
            alerted_matches: vec![scan_match("alert-a"), scan_match("alert-b")],
            alert: None,
        };

        assert_eq!(alerted_target_ids(&result), vec!["alert-a", "alert-b"]);
    }

    #[test]
    fn window_source_name_prefers_name_then_display_then_title() {
        assert_eq!(window_source_name(&window("Demo", Some(1))), "app-Demo");
        let mut with_display = window("Demo", Some(1));
        with_display.display = "Demo #2".to_string();
        assert_eq!(window_source_name(&with_display), "app-Demo__2");
        with_display.name = "custom".to_string();
        assert_eq!(window_source_name(&with_display), "custom");
    }

    #[test]
    fn mark_stopped_emits_a_single_state_transition_snapshot() {
        let snapshot = Arc::new(Mutex::new(MonitorSessionSnapshot {
            running: true,
            ..MonitorSessionSnapshot::default()
        }));

        let stopped = mark_stopped(&snapshot).unwrap().unwrap();
        let second = mark_stopped(&snapshot).unwrap();

        assert!(!stopped.running);
        assert!(stopped.stopped_at.is_some());
        assert!(second.is_none());
    }

    #[test]
    fn start_and_stop_events_use_zero_tick_transition_payloads() {
        let started_snapshot = MonitorSessionSnapshot {
            running: true,
            started_at: Some("start".to_string()),
            ..MonitorSessionSnapshot::default()
        };

        let started = started_event(started_snapshot.clone());

        assert_eq!(started.kind, MonitorSessionEventKind::Started);
        assert_eq!(started.snapshot, started_snapshot);
        assert_eq!(started.tick_hit_count, 0);
        assert_eq!(started.tick_error, None);

        let stopped_snapshot = MonitorSessionSnapshot {
            running: false,
            started_at: Some("start".to_string()),
            stopped_at: Some("stop".to_string()),
            tick_count: 2,
            hit_count: 1,
            ..MonitorSessionSnapshot::default()
        };

        let stopped = stopped_event(stopped_snapshot.clone());

        assert_eq!(stopped.kind, MonitorSessionEventKind::Stopped);
        assert_eq!(stopped.snapshot, stopped_snapshot);
        assert_eq!(stopped.tick_hit_count, 0);
        assert_eq!(stopped.tick_error, None);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop"]
    fn session_start_runs_ticks_and_stop_joins_worker() {
        let tmp = tempfile::tempdir().unwrap();
        let session = MonitorSessionState::default();
        let config = WatchConfig::from_json_str(
            r#"{
              "poll_interval_seconds": 0.12,
              "cooldown_seconds": 0,
              "targets": [
                {"kind":"pixel","id":"desktop-pixel","name":"desktop-pixel","x":0,"y":0,"rgb":[0,0,0],"tolerance":255}
              ],
              "alarm": {"beep": false, "save_dir": "screenshots", "jsonl": "alerts.jsonl", "max_alerts": 3}
            }"#,
        )
        .unwrap();
        let regions = vec![ResolvedRegion {
            name: "desktop".to_string(),
            monitor: 1,
            bbox: BBox {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            },
        }];
        let started = session
            .start_sources_with_events(
                config,
                tmp.path().to_path_buf(),
                regions,
                Vec::new(),
                Vec::new(),
                AlarmBeepState::default(),
                None,
                Arc::new(TestEventSink),
            )
            .unwrap();
        assert!(started.running);
        for _ in 0..20 {
            if session.snapshot().unwrap().tick_count > 0 {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let stopped = session.stop().unwrap();
        assert!(!stopped.running);
        assert!(stopped.tick_count > 0);
        assert!(stopped.hit_count > 0);
        assert!(tmp.path().join("alerts.jsonl").exists());
        assert!(fs::read_dir(tmp.path().join("screenshots"))
            .unwrap()
            .next()
            .is_some());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows desktop with at least one selectable window"]
    fn session_start_scans_window_source_and_writes_evidence() {
        const TITLE: &str = "000 Screen Watch OCR monitoring window smoke";
        let _form = TestFormProcess::new(TITLE).unwrap();
        let window = wait_for_listed_app_window(TITLE).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let session = MonitorSessionState::default();
        let config = WatchConfig::from_json_str(
            r#"{
              "poll_interval_seconds": 0.12,
              "cooldown_seconds": 0,
              "targets": [
                {"kind":"pixel","id":"window-pixel","name":"window-pixel","x":0,"y":0,"rgb":[0,0,0],"tolerance":255}
              ],
              "alarm": {"beep": false, "save_dir": "screenshots", "jsonl": "alerts.jsonl", "max_alerts": 3}
            }"#,
        )
        .unwrap();
        let started = session
            .start_sources_with_events(
                config,
                tmp.path().to_path_buf(),
                Vec::new(),
                vec![WindowConfig {
                    name: "app-window".to_string(),
                    title: window.title,
                    display: window.display,
                    hwnd: Some(window.hwnd),
                    extra: Default::default(),
                }],
                Vec::new(),
                AlarmBeepState::default(),
                None,
                Arc::new(TestEventSink),
            )
            .unwrap();
        assert_eq!(started.region_count, 0);
        assert_eq!(started.window_count, 1);
        for _ in 0..20 {
            if session.snapshot().unwrap().tick_count > 0 {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let stopped = session.stop().unwrap();
        assert!(stopped.tick_count > 0);
        assert!(stopped.hit_count > 0);
        assert!(tmp.path().join("alerts.jsonl").exists());
    }

    #[cfg(windows)]
    struct TestFormProcess {
        child: std::process::Child,
    }

    #[cfg(windows)]
    impl TestFormProcess {
        fn new(title: &str) -> Result<Self, String> {
            let escaped_title = title.replace('\'', "''");
            let script = format!(
                "$ErrorActionPreference='Stop'; \
                 Add-Type -AssemblyName System.Windows.Forms; \
                 $form = New-Object System.Windows.Forms.Form; \
                 $form.Text = '{escaped_title}'; \
                 $form.Width = 360; $form.Height = 240; \
                 $form.StartPosition = 'Manual'; $form.Left = 180; $form.Top = 180; \
                 $form.TopMost = $true; \
                 $timer = New-Object System.Windows.Forms.Timer; \
                 $timer.Interval = 15000; \
                 $timer.Add_Tick({{ $form.Close() }}); \
                 $timer.Start(); \
                 [System.Windows.Forms.Application]::Run($form);"
            );
            let child = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-STA",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &script,
                ])
                .spawn()
                .map_err(|err| format!("failed to start monitoring smoke window: {err}"))?;
            Ok(Self { child })
        }
    }

    #[cfg(windows)]
    impl Drop for TestFormProcess {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[cfg(windows)]
    fn wait_for_listed_app_window(title: &str) -> Result<crate::window_sources::AppWindow, String> {
        for _ in 0..50 {
            let windows = crate::window_sources::list_app_windows()?;
            if let Some(window) = windows.into_iter().find(|window| window.title == title) {
                return Ok(window);
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(format!("monitoring smoke window {title:?} was not listed"))
    }

    fn resolved_region(name: &str) -> ResolvedRegion {
        ResolvedRegion {
            name: name.to_string(),
            monitor: 1,
            bbox: BBox {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            },
        }
    }

    fn window(title: &str, hwnd: Option<isize>) -> WindowConfig {
        WindowConfig {
            name: String::new(),
            title: title.to_string(),
            display: String::new(),
            hwnd,
            extra: Default::default(),
        }
    }

    fn scan_match(target_id: &str) -> Match {
        Match {
            target: target_id.to_string(),
            target_id: target_id.to_string(),
            kind: "template".to_string(),
            score: 1.0,
            box_xyxy: [0, 0, 1, 1],
            scale: None,
            text: None,
        }
    }
}
