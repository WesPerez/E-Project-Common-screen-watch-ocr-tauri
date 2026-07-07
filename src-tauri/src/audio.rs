use screen_watch_core::{
    audio::{beep_wave, clamp_volume, DEFAULT_BEEP_MILLISECONDS},
    config::AlarmConfig,
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Default)]
pub struct AlarmBeepState {
    state: Arc<Mutex<BeepRuntimeState>>,
}

#[derive(Debug, Default)]
struct BeepRuntimeState {
    beep_until: Option<Instant>,
}

impl AlarmBeepState {
    pub fn start_for_alarm(&self, alarm: &AlarmConfig) -> bool {
        if !alarm.beep {
            return false;
        }
        self.start(alarm.beep_seconds, alarm.beep_volume)
    }

    fn start(&self, seconds: f64, volume: i32) -> bool {
        let Some(duration) = duration_from_seconds(seconds) else {
            return false;
        };
        let now = Instant::now();
        if !self
            .state
            .lock()
            .map(|mut state| state.try_start(now, duration))
            .unwrap_or(false)
        {
            return false;
        }

        thread::spawn(move || play_beep_for(duration, volume));
        true
    }
}

impl BeepRuntimeState {
    fn try_start(&mut self, now: Instant, duration: Duration) -> bool {
        if self.beep_until.map(|until| now < until).unwrap_or(false) {
            return false;
        }
        self.beep_until = Some(now + duration);
        true
    }
}

fn duration_from_seconds(seconds: f64) -> Option<Duration> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    Some(Duration::from_secs_f64(seconds))
}

fn play_beep_for(duration: Duration, volume: i32) {
    let deadline = Instant::now() + duration;
    let step = Duration::from_millis(u64::from(DEFAULT_BEEP_MILLISECONDS));
    while Instant::now() < deadline {
        if clamp_volume(volume) == 0 {
            thread::sleep(step);
            continue;
        }
        play_wave_memory(&beep_wave(volume));
    }
}

#[cfg(windows)]
fn play_wave_memory(wav: &[u8]) {
    use windows::{
        core::PCWSTR,
        Win32::Media::Audio::{PlaySoundW, SND_MEMORY},
    };

    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY);
    }
}

#[cfg(not(windows))]
fn play_wave_memory(_wav: &[u8]) {
    thread::sleep(Duration::from_millis(u64::from(DEFAULT_BEEP_MILLISECONDS)));
}

#[cfg(test)]
mod tests {
    use super::{duration_from_seconds, AlarmBeepState, BeepRuntimeState};
    use screen_watch_core::config::AlarmConfig;
    use std::time::{Duration, Instant};

    #[test]
    fn rejects_invalid_beep_duration() {
        assert!(duration_from_seconds(0.0).is_none());
        assert!(duration_from_seconds(-1.0).is_none());
        assert!(duration_from_seconds(f64::NAN).is_none());
        assert_eq!(
            duration_from_seconds(0.25),
            Some(Duration::from_millis(250))
        );
    }

    #[test]
    fn runtime_throttle_does_not_restart_until_deadline() {
        let mut state = BeepRuntimeState::default();
        let now = Instant::now();
        assert!(state.try_start(now, Duration::from_secs(3)));
        assert!(!state.try_start(now + Duration::from_millis(2999), Duration::from_secs(1)));
        assert!(state.try_start(now + Duration::from_secs(3), Duration::from_secs(1)));
    }

    #[test]
    fn start_for_alarm_respects_disabled_alarm() {
        let beeper = AlarmBeepState::default();
        let alarm = AlarmConfig {
            beep: false,
            beep_seconds: 0.01,
            beep_volume: 100,
            ..Default::default()
        };

        assert!(!beeper.start_for_alarm(&alarm));
    }

    #[test]
    fn start_for_alarm_throttles_even_when_volume_is_zero() {
        let beeper = AlarmBeepState::default();
        let alarm = AlarmConfig {
            beep: true,
            beep_seconds: 0.01,
            beep_volume: 0,
            ..Default::default()
        };

        assert!(beeper.start_for_alarm(&alarm));
        assert!(!beeper.start_for_alarm(&alarm));
    }
}
