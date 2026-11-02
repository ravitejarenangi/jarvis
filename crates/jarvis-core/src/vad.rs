//! Silero voice activity detection and turn detection.

use voice_activity_detector::VoiceActivityDetector;

use crate::audio::{FRAME, SAMPLE_RATE};

/// Silero's chunk size at 16 kHz.
const CHUNK: usize = 512;
/// A frame at or above this probability counts as speech.
const SPEECH_PROB: f32 = 0.5;
/// A frame below this probability counts as silence.
const SILENCE_PROB: f32 = 0.35;
/// Speech frames in a row (20 ms) before a turn starts.
const START_FRAMES: u32 = 2;

#[derive(Debug, thiserror::Error)]
#[error("voice activity detector: {0}")]
pub struct VadError(#[from] voice_activity_detector::Error);

pub struct Vad {
    detector: VoiceActivityDetector,
    pending: Vec<f32>,
    prob: f32,
}

impl Vad {
    pub fn new() -> Result<Self, VadError> {
        let detector = VoiceActivityDetector::builder()
            .sample_rate(SAMPLE_RATE)
            .chunk_size(CHUNK)
            .build()?;
        Ok(Self {
            detector,
            pending: Vec::with_capacity(CHUNK + FRAME),
            prob: 0.0,
        })
    }

    /// Speech probability of the latest complete 512-sample chunk.
    pub fn speech_prob(&mut self, frame: &[f32; FRAME]) -> f32 {
        self.pending.extend_from_slice(frame);
        if self.pending.len() >= CHUNK {
            self.prob = self.detector.predict(self.pending.drain(..CHUNK));
        }
        self.prob
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnEvent {
    SpeechStart,
    SpeechEnd,
}

/// Turns per-frame speech probabilities into start/end of a user turn.
/// Probabilities between the two thresholds count as neither, so a turn
/// doesn't flap on borderline frames.
pub struct TurnDetector {
    end_frames: u32,
    in_speech: bool,
    run: u32,
}

impl TurnDetector {
    pub fn new(silence_ms: u32) -> Self {
        Self {
            end_frames: (silence_ms / 10).max(1),
            in_speech: false,
            run: 0,
        }
    }

    pub fn push(&mut self, prob: f32) -> Option<TurnEvent> {
        let (counts, needed, event) = if self.in_speech {
            (prob < SILENCE_PROB, self.end_frames, TurnEvent::SpeechEnd)
        } else {
            (prob >= SPEECH_PROB, START_FRAMES, TurnEvent::SpeechStart)
        };
        self.run = if counts { self.run + 1 } else { 0 };
        if self.run < needed {
            return None;
        }
        self.run = 0;
        self.in_speech = !self.in_speech;
        Some(event)
    }
}

#[cfg(test)]
pub(crate) fn fixture_frames(name: &str) -> Vec<[f32; crate::audio::FRAME]> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let mut reader = hound::WavReader::open(&path).expect("fixture exists");
    assert_eq!(reader.spec().sample_rate, 16_000);
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| f32::from(s.expect("valid sample")) / 32_768.0)
        .collect();
    samples.as_chunks().0.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::FRAME;

    fn feed(det: &mut TurnDetector, prob: f32, frames: usize) -> Vec<TurnEvent> {
        (0..frames).filter_map(|_| det.push(prob)).collect()
    }

    #[test]
    fn turn_detector_ignores_single_frame_blip() {
        let mut det = TurnDetector::new(500);
        assert!(feed(&mut det, 0.0, 10).is_empty());
        assert_eq!(det.push(0.9), None);
        assert!(feed(&mut det, 0.0, 100).is_empty());
    }

    #[test]
    fn turn_detector_start_then_end_after_silence() {
        let mut det = TurnDetector::new(500);
        assert_eq!(det.push(0.9), None);
        assert_eq!(det.push(0.9), Some(TurnEvent::SpeechStart));
        assert!(feed(&mut det, 0.9, 30).is_empty());
        assert!(feed(&mut det, 0.1, 49).is_empty());
        assert_eq!(det.push(0.1), Some(TurnEvent::SpeechEnd));
        assert!(feed(&mut det, 0.1, 100).is_empty());
    }

    #[test]
    fn turn_detector_short_pause_does_not_end_turn() {
        let mut det = TurnDetector::new(500);
        assert_eq!(feed(&mut det, 0.9, 20), vec![TurnEvent::SpeechStart]);
        assert!(feed(&mut det, 0.1, 30).is_empty());
        // Between the thresholds counts as neither speech nor silence.
        assert!(feed(&mut det, 0.4, 5).is_empty());
        assert!(feed(&mut det, 0.9, 20).is_empty());
        assert!(feed(&mut det, 0.1, 30).is_empty());
    }

    #[test]
    fn vad_silence_is_low_and_speech_is_high() {
        let mut vad = Vad::new().unwrap();
        let peak = fixture_frames("open_safari.wav")
            .iter()
            .map(|f| vad.speech_prob(f))
            .fold(0.0, f32::max);
        assert!(peak > 0.5, "speech peak {peak}");

        let mut vad = Vad::new().unwrap();
        let peak = (0..100)
            .map(|_| vad.speech_prob(&[0.0; FRAME]))
            .fold(0.0, f32::max);
        assert!(peak < 0.2, "silence peak {peak}");
    }
}
