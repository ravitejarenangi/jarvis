//! WebRTC acoustic echo cancellation, so JARVIS doesn't hear itself.

use webrtc_audio_processing::Processor;
use webrtc_audio_processing::config::{
    Config, EchoCanceller as Aec3, HighPassFilter, NoiseSuppression, NoiseSuppressionLevel,
};

use crate::audio::{FRAME, SAMPLE_RATE};

#[derive(Debug, thiserror::Error)]
#[error("echo canceller: {0}")]
pub struct AecError(#[from] webrtc_audio_processing::Error);

/// Full AEC3 with moderate noise suppression, high-pass filter on, AGC off.
pub struct EchoCanceller {
    apm: Processor,
}

impl EchoCanceller {
    pub fn new() -> Result<Self, AecError> {
        let apm = Processor::new(SAMPLE_RATE)?;
        apm.set_config(Config {
            echo_canceller: Some(Aec3::Full {
                stream_delay_ms: None,
            }),
            noise_suppression: Some(NoiseSuppression {
                level: NoiseSuppressionLevel::Moderate,
                analyze_linear_aec_output: false,
            }),
            high_pass_filter: Some(HighPassFilter::default()),
            gain_controller: None,
            ..Config::default()
        });
        Ok(Self { apm })
    }

    /// What the speaker played (render tap frames).
    pub fn far_end(&mut self, frame: &[f32; FRAME]) {
        if let Err(e) = self.apm.analyze_render_frame([&frame[..]]) {
            tracing::warn!("AEC far end: {e}");
        }
    }

    /// Removes the echo of the far end from a microphone frame, in place.
    pub fn near_end(&mut self, frame: &mut [f32; FRAME]) {
        if let Err(e) = self.apm.process_capture_frame([&mut frame[..]]) {
            tracing::warn!("AEC near end: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{FRAME, rms};
    use crate::vad::fixture_frames;

    #[test]
    fn aec_attenuates_echo_by_10db() {
        let far = fixture_frames("open_safari.wav");
        let samples = far.as_flattened();
        // The microphone hears the speaker 40 ms late at 0.6 gain.
        let delay = 16 * 40;
        let near: Vec<f32> = (0..samples.len())
            .map(|i| {
                if i < delay {
                    0.0
                } else {
                    samples[i - delay] * 0.6
                }
            })
            .collect();

        let mut aec = EchoCanceller::new().unwrap();
        let converge = 30; // 300 ms
        let (mut input, mut output) = (Vec::new(), Vec::new());
        for (i, (f, n)) in far.iter().zip(near.as_chunks::<FRAME>().0).enumerate() {
            aec.far_end(f);
            let mut frame = *n;
            aec.near_end(&mut frame);
            if i >= converge {
                input.extend_from_slice(n);
                output.extend_from_slice(&frame);
            }
        }
        let db = 20.0 * (rms(&output) / rms(&input)).log10();
        assert!(db <= -10.0, "echo only attenuated by {db:.1} dB");
    }
}
