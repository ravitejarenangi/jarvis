//! Microphone capture, speaker playback, resampling to 16 kHz frames, levels.
//!
//! cpal callbacks never allocate or block: they only move samples through
//! preallocated ring buffers and touch atomics. Resampling and framing run on
//! worker threads.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::SeqCst};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::ErrorKind;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

/// Samples per frame: 10 ms at 16 kHz.
pub const FRAME: usize = 160;
/// Internal sample rate.
pub const SAMPLE_RATE: u32 = 16_000;
/// Sample rate of the PCM handed to [`Speaker::play`].
pub const TTS_RATE: u32 = 24_000;

const WORKER_IDLE: Duration = Duration::from_millis(5);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("no input device")]
    NoInputDevice,
    #[error("no output device")]
    NoOutputDevice,
    #[error("microphone permission denied")]
    PermissionDenied,
    #[error("audio backend error: {0}")]
    Backend(String),
}

impl AudioError {
    /// Text for the HUD.
    pub fn user_message(&self) -> String {
        match self {
            Self::NoInputDevice => {
                "No microphone found. Connect a microphone, then try again.".into()
            }
            Self::NoOutputDevice => {
                "No speakers found. Connect speakers or headphones, then try again.".into()
            }
            Self::PermissionDenied => "Microphone access needed. Allow JARVIS in \
                 System Settings > Privacy & Security > Microphone, then restart JARVIS."
                .into(),
            Self::Backend(e) => format!("Audio error: {e}"),
        }
    }
}

/// Maps a cpal error on the input side. A device that exists but whose stream
/// will not open is, on macOS, almost always a denied microphone permission.
fn input_error(e: cpal::Error, opening_stream: bool) -> AudioError {
    match e.kind() {
        ErrorKind::DeviceNotAvailable => AudioError::NoInputDevice,
        ErrorKind::PermissionDenied => AudioError::PermissionDenied,
        _ if opening_stream => AudioError::PermissionDenied,
        _ => AudioError::Backend(e.to_string()),
    }
}

fn output_error(e: cpal::Error) -> AudioError {
    match e.kind() {
        ErrorKind::DeviceNotAvailable => AudioError::NoOutputDevice,
        _ => AudioError::Backend(e.to_string()),
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<(), AudioError> {
    thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .map(drop)
        .map_err(|e| AudioError::Backend(e.to_string()))
}

/// Root mean square level of `samples` (0 for an empty slice).
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Streaming mono resampler working in 10 ms chunks.
struct Resample {
    /// `None` when the rates already match.
    fft: Option<Fft<f32>>,
    from: u32,
    to: u32,
    pending: Vec<f32>,
}

impl Resample {
    fn new(from: u32, to: u32) -> Self {
        let fft = (from != to).then(|| {
            let chunk = (from as usize / 100).max(1);
            Fft::new(from as usize, to as usize, chunk, 1, FixedSync::Both)
                .expect("sample rates are non-zero")
        });
        Self {
            fft,
            from,
            to,
            pending: Vec::new(),
        }
    }

    /// Resamples `mono` and appends the result to `out`. Input that doesn't
    /// fill a whole chunk waits for the next call.
    fn push(&mut self, mono: &[f32], out: &mut Vec<f32>) {
        let Some(fft) = &mut self.fft else {
            out.extend_from_slice(mono);
            return;
        };
        self.pending.extend_from_slice(mono);
        let mut used = 0;
        while self.pending.len() - used >= fft.input_frames_next() {
            let need = fft.input_frames_next();
            let input = InterleavedSlice::new(&self.pending[used..used + need], 1, need)
                .expect("slice holds `need` frames");
            let start = out.len();
            let room = fft.output_frames_next();
            out.resize(start + room, 0.0);
            let mut output = InterleavedSlice::new_mut(&mut out[start..], 1, room)
                .expect("slice holds `room` frames");
            let (_, written) = fft
                .process_into_buffer(&input, &mut output, None)
                .expect("buffers are sized from the resampler");
            out.truncate(start + written);
            used += need;
        }
        self.pending.drain(..used);
    }

    /// Pushes silence through so everything given so far comes out, then
    /// resets. Adds at most a few tens of ms of trailing silence.
    fn flush(&mut self, out: &mut Vec<f32>) {
        let Some(fft) = &self.fft else { return };
        let chunk = fft.input_frames_next();
        let delay_in = fft.output_delay() * self.from as usize / self.to as usize;
        let zeros = vec![0.0; 2 * chunk + delay_in];
        self.push(&zeros, out);
        self.reset();
    }

    fn reset(&mut self) {
        if let Some(fft) = &mut self.fft {
            fft.reset();
        }
        self.pending.clear();
    }
}

/// Turns interleaved device audio into 16 kHz mono frames of [`FRAME`] samples.
pub struct Framer {
    channels: usize,
    mono: Vec<f32>,
    resample: Resample,
    out: Vec<f32>,
}

impl Framer {
    pub fn new(input_rate: u32, channels: u16) -> Self {
        Self {
            channels: usize::from(channels.max(1)),
            mono: Vec::new(),
            resample: Resample::new(input_rate, SAMPLE_RATE),
            out: Vec::new(),
        }
    }

    /// Downmixes, resamples and returns every whole frame; the remainder is
    /// kept for the next call. `interleaved` must hold whole device frames.
    pub fn push(&mut self, interleaved: &[f32]) -> Vec<[f32; FRAME]> {
        self.mono.clear();
        self.mono.extend(
            interleaved
                .chunks_exact(self.channels)
                .map(|f| f.iter().sum::<f32>() / self.channels as f32),
        );
        self.resample.push(&self.mono, &mut self.out);
        let frames = self.out.as_chunks::<FRAME>().0.to_vec();
        self.out.drain(..frames.len() * FRAME);
        frames
    }
}

/// Default-input capture. Dropping it stops capture.
pub struct Mic {
    _stream: cpal::Stream,
    stop: Arc<AtomicBool>,
}

impl Mic {
    /// Starts capture; 16 kHz frames arrive on `tx`.
    pub fn start(tx: SyncSender<[f32; FRAME]>) -> Result<Mic, AudioError> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;
        let config = device
            .default_input_config()
            .map_err(|e| input_error(e, false))?
            .config();
        let (rate, channels) = (config.sample_rate, config.channels);
        let ch = usize::from(channels.max(1));
        // One second of headroom between the callback and the worker.
        let (mut prod, mut cons) = HeapRb::<f32>::new(rate as usize * ch).split();
        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Whole buffers only, so channels never get out of step.
                    if prod.vacant_len() >= data.len() {
                        prod.push_slice(data);
                    }
                },
                |e| tracing::warn!("microphone stream error: {e}"),
                None,
            )
            .map_err(|e| input_error(e, true))?;
        stream.play().map_err(|e| input_error(e, true))?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        spawn("jarvis-mic", move || {
            let mut framer = Framer::new(rate, channels);
            let mut buf = vec![0.0; rate as usize / 50 * ch];
            while !stop_worker.load(SeqCst) {
                let n = cons.pop_slice(&mut buf);
                if n == 0 {
                    thread::sleep(WORKER_IDLE);
                    continue;
                }
                for frame in framer.push(&buf[..n]) {
                    if tx.send(frame).is_err() {
                        return;
                    }
                }
            }
        })?;
        Ok(Mic {
            _stream: stream,
            stop,
        })
    }
}

impl Drop for Mic {
    fn drop(&mut self) {
        self.stop.store(true, SeqCst);
    }
}

/// State shared by the speaker handle, its callback and its workers.
#[derive(Default)]
struct SpeakerShared {
    /// Bumped by `stop_now`; queued audio from an older epoch is dropped.
    epoch: AtomicU64,
    /// Asks the callback to empty the ring buffer.
    flush: AtomicBool,
    /// `play` calls not yet moved into the ring buffer.
    queued: AtomicUsize,
    /// Samples sitting in the ring buffer.
    buffered: AtomicUsize,
    stop: AtomicBool,
    tap: Mutex<Option<SyncSender<[f32; FRAME]>>>,
}

/// Default-output playback of 24 kHz mono PCM.
pub struct Speaker {
    _stream: cpal::Stream,
    tx: Sender<(u64, Vec<f32>)>,
    shared: Arc<SpeakerShared>,
}

impl Speaker {
    pub fn start() -> Result<Speaker, AudioError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;
        let config = device
            .default_output_config()
            .map_err(output_error)?
            .config();
        let (rate, channels) = (config.sample_rate, usize::from(config.channels.max(1)));
        let shared = Arc::new(SpeakerShared::default());

        // Device-rate mono: 2 s queued for playback, 1 s for the render tap.
        let (mut play_prod, mut play_cons) = HeapRb::<f32>::new(rate as usize * 2).split();
        let (mut tap_prod, mut tap_cons) = HeapRb::<f32>::new(rate as usize).split();

        let cb = shared.clone();
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if cb.flush.swap(false, SeqCst) {
                        let n = play_cons.clear();
                        cb.buffered.fetch_sub(n, SeqCst);
                    }
                    let mut popped = 0;
                    for frame in data.chunks_mut(channels) {
                        let s = play_cons.try_pop();
                        popped += usize::from(s.is_some());
                        let s = s.unwrap_or(0.0);
                        frame.fill(s);
                        // Dropped if the tap worker lags; the AEC copes.
                        let _ = tap_prod.try_push(s);
                    }
                    cb.buffered.fetch_sub(popped, SeqCst);
                },
                |e| tracing::warn!("speaker stream error: {e}"),
                None,
            )
            .map_err(output_error)?;
        stream.play().map_err(output_error)?;

        // Feeder: resample queued 24 kHz PCM into the playback ring buffer.
        let (tx, rx) = mpsc::channel::<(u64, Vec<f32>)>();
        let feed = shared.clone();
        spawn("jarvis-speaker", move || {
            let mut resample = Resample::new(TTS_RATE, rate);
            let mut out = Vec::new();
            // Epoch of the audio still inside the resampler, if any.
            let mut held: Option<u64> = None;
            while !feed.stop.load(SeqCst) {
                let current = feed.epoch.load(SeqCst);
                if held.is_some_and(|e| e != current) {
                    resample.reset();
                    held = None;
                }
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok((epoch, pcm)) => {
                        if epoch == current {
                            resample.push(&pcm, &mut out);
                            held = Some(epoch);
                            if !feed_ring(&mut play_prod, &mut out, epoch, &feed) {
                                resample.reset();
                                held = None;
                            }
                        }
                        feed.queued.fetch_sub(1, SeqCst);
                    }
                    // Nothing new and the ring is about to run dry: play the tail.
                    Err(RecvTimeoutError::Timeout)
                        if held.is_some() && play_prod.occupied_len() < rate as usize / 50 =>
                    {
                        resample.flush(&mut out);
                        held = None;
                        feed_ring(&mut play_prod, &mut out, current, &feed);
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        })?;

        // Render tap: what was actually played, as 16 kHz frames.
        let tap = shared.clone();
        spawn("jarvis-render-tap", move || {
            let mut framer = Framer::new(rate, 1);
            let mut buf = vec![0.0; rate as usize / 50];
            while !tap.stop.load(SeqCst) {
                let n = tap_cons.pop_slice(&mut buf);
                if n == 0 {
                    thread::sleep(WORKER_IDLE);
                    continue;
                }
                let frames = framer.push(&buf[..n]);
                let mut sender = tap.tap.lock().expect("tap lock");
                if let Some(tx) = sender.as_ref() {
                    for frame in frames {
                        if let Err(TrySendError::Disconnected(_)) = tx.try_send(frame) {
                            *sender = None;
                            break;
                        }
                    }
                }
            }
        })?;

        Ok(Speaker {
            _stream: stream,
            tx,
            shared,
        })
    }

    /// Queues 24 kHz mono PCM for playback.
    pub fn play(&self, pcm24k: Vec<f32>) {
        self.shared.queued.fetch_add(1, SeqCst);
        if self
            .tx
            .send((self.shared.epoch.load(SeqCst), pcm24k))
            .is_err()
        {
            self.shared.queued.fetch_sub(1, SeqCst);
        }
    }

    /// Drops everything queued; silence within one callback (barge-in).
    pub fn stop_now(&self) {
        self.shared.epoch.fetch_add(1, SeqCst);
        self.shared.flush.store(true, SeqCst);
    }

    pub fn is_playing(&self) -> bool {
        self.shared.queued.load(SeqCst) > 0 || self.shared.buffered.load(SeqCst) > 0
    }

    /// What's being played, as 16 kHz frames, for the echo canceller's far
    /// end. A new call replaces the previous receiver.
    pub fn render_tap(&self) -> Receiver<[f32; FRAME]> {
        let (tx, rx) = mpsc::sync_channel(100);
        *self.shared.tap.lock().expect("tap lock") = Some(tx);
        rx
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        self.shared.stop.store(true, SeqCst);
    }
}

/// Moves `out` into the playback ring, waiting for room. Returns false (and
/// discards the rest) if `stop_now` was called meanwhile.
fn feed_ring(
    prod: &mut impl Producer<Item = f32>,
    out: &mut Vec<f32>,
    epoch: u64,
    shared: &SpeakerShared,
) -> bool {
    while !out.is_empty() {
        if shared.stop.load(SeqCst) || shared.epoch.load(SeqCst) != epoch {
            out.clear();
            return false;
        }
        // Let the callback finish a pending flush before adding new audio.
        if shared.flush.load(SeqCst) {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        // Count before pushing so the callback never subtracts first.
        let n = prod.vacant_len().min(out.len());
        shared.buffered.fetch_add(n, SeqCst);
        prod.push_slice(&out[..n]);
        out.drain(..n);
        // stop_now may have raced with this push: flush again.
        if shared.epoch.load(SeqCst) != epoch {
            shared.flush.store(true, SeqCst);
        }
        if n == 0 {
            thread::sleep(WORKER_IDLE);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framer_48k_stereo_one_second_gives_100_frames() {
        let mut framer = Framer::new(48_000, 2);
        let input: Vec<f32> = (0..48_000 * 2)
            .map(|i| ((i / 2) as f32 * 0.01).sin())
            .collect();
        let mut frames = 0;
        for chunk in input.chunks(2 * 333) {
            frames += framer.push(chunk).len();
        }
        assert!((99..=101).contains(&frames), "got {frames} frames");
    }

    #[test]
    fn framer_keeps_remainder_between_pushes() {
        let mut framer = Framer::new(16_000, 1);
        assert!(framer.push(&[0.5; 100]).is_empty());
        let frames = framer.push(&[0.5; 100]);
        assert_eq!(frames.len(), 1);
        assert!(frames[0].iter().all(|&s| s == 0.5));
        // 40 samples left over; 120 more completes the next frame.
        assert_eq!(framer.push(&[0.5; 120]).len(), 1);
    }

    #[test]
    fn rms_of_full_scale_sine_is_about_0_707() {
        let sine: Vec<f32> = (0..16_000)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 16_000.0).sin())
            .collect();
        assert!((rms(&sine) - 0.707).abs() < 0.01);
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn audio_open_error_maps_to_user_message() {
        assert!(
            AudioError::PermissionDenied
                .user_message()
                .contains("System Settings > Privacy & Security > Microphone")
        );
        assert!(
            AudioError::NoInputDevice
                .user_message()
                .to_lowercase()
                .contains("connect a microphone")
        );
    }
}
