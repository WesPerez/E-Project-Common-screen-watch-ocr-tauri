use std::f32::consts::PI;

pub const DEFAULT_BEEP_SECONDS: f64 = 3.0;
pub const DEFAULT_BEEP_VOLUME: i32 = 100;
pub const DEFAULT_BEEP_MILLISECONDS: u32 = 180;
pub const DEFAULT_BEEP_FREQUENCY_HZ: u32 = 1200;
pub const DEFAULT_BEEP_SAMPLE_RATE: u32 = 22_050;

pub fn clamp_volume(value: i32) -> u8 {
    value.clamp(0, 100) as u8
}

pub fn beep_wave(volume: i32) -> Vec<u8> {
    beep_wave_with_params(
        volume,
        DEFAULT_BEEP_MILLISECONDS,
        DEFAULT_BEEP_FREQUENCY_HZ,
        DEFAULT_BEEP_SAMPLE_RATE,
    )
}

pub fn beep_wave_with_params(
    volume: i32,
    milliseconds: u32,
    frequency_hz: u32,
    sample_rate: u32,
) -> Vec<u8> {
    let volume = clamp_volume(volume);
    let frames = sample_rate.saturating_mul(milliseconds) / 1000;
    let data_len = frames.saturating_mul(2);
    let mut out = Vec::with_capacity(44 + data_len as usize);
    let riff_len = 36u32.saturating_add(data_len);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate.saturating_mul(2)).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());

    if sample_rate == 0 || frequency_hz == 0 {
        out.resize(out.len() + data_len as usize, 0);
        return out;
    }

    let amplitude = 32767.0f32 * (f32::from(volume) / 100.0);
    let frequency = frequency_hz as f32;
    let sample_rate_f = sample_rate as f32;
    for i in 0..frames {
        let phase = 2.0 * PI * frequency * (i as f32) / sample_rate_f;
        let sample = (amplitude * phase.sin()) as i16;
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct BeepThrottle {
    beep_until_seconds: f64,
}

impl Default for BeepThrottle {
    fn default() -> Self {
        Self {
            beep_until_seconds: 0.0,
        }
    }
}

impl BeepThrottle {
    pub fn try_start(&mut self, now_seconds: f64, seconds: f64) -> bool {
        if now_seconds < self.beep_until_seconds {
            return false;
        }
        self.beep_until_seconds = now_seconds + seconds.max(0.0);
        true
    }

    pub fn beep_until_seconds(&self) -> f64 {
        self.beep_until_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::{beep_wave_with_params, clamp_volume, BeepThrottle};

    #[test]
    fn volume_is_clamped_like_python_baseline() {
        assert_eq!(clamp_volume(-2), 0);
        assert_eq!(clamp_volume(0), 0);
        assert_eq!(clamp_volume(42), 42);
        assert_eq!(clamp_volume(150), 100);
    }

    #[test]
    fn beep_wave_is_pcm_wav_and_volume_changes_amplitude() {
        let quiet = beep_wave_with_params(10, 20, 1200, 22_050);
        let loud = beep_wave_with_params(100, 20, 1200, 22_050);

        assert_eq!(&loud[0..4], b"RIFF");
        assert_eq!(&loud[8..12], b"WAVE");
        assert_eq!(&loud[12..16], b"fmt ");
        assert_eq!(&loud[36..40], b"data");
        assert_eq!(u16::from_le_bytes([loud[20], loud[21]]), 1);
        assert_eq!(u16::from_le_bytes([loud[22], loud[23]]), 1);
        assert_eq!(u16::from_le_bytes([loud[34], loud[35]]), 16);
        assert!(sample_peak(&loud) > sample_peak(&quiet));
    }

    #[test]
    fn zero_volume_wave_is_silent_but_valid() {
        let silent = beep_wave_with_params(0, 20, 1200, 22_050);
        assert_eq!(&silent[0..4], b"RIFF");
        assert_eq!(sample_peak(&silent), 0);
    }

    #[test]
    fn throttle_does_not_restart_while_beeping() {
        let mut throttle = BeepThrottle::default();
        assert!(throttle.try_start(10.0, 3.0));
        assert_eq!(throttle.beep_until_seconds(), 13.0);
        assert!(!throttle.try_start(12.999, 3.0));
        assert_eq!(throttle.beep_until_seconds(), 13.0);
        assert!(throttle.try_start(13.0, 2.0));
        assert_eq!(throttle.beep_until_seconds(), 15.0);
    }

    fn sample_peak(wav: &[u8]) -> i16 {
        wav[44..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]).abs())
            .max()
            .unwrap_or(0)
    }
}
