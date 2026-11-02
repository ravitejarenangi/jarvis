//! Hardware check: prints the microphone level 10x/s for 5 s and plays a
//! 1 s 440 Hz tone. `cargo run -p jarvis-core --example mic_loopback`

use std::sync::mpsc;
use std::time::{Duration, Instant};

use jarvis_core::audio::{Mic, Speaker, TTS_RATE, rms};

fn main() {
    let (tx, rx) = mpsc::sync_channel(100);
    let _mic = Mic::start(tx).unwrap_or_else(|e| exit(e.user_message()));
    let speaker = Speaker::start().unwrap_or_else(|e| exit(e.user_message()));

    let tone = (0..TTS_RATE)
        .map(|i| 0.3 * (i as f32 * 440.0 * std::f32::consts::TAU / TTS_RATE as f32).sin())
        .collect();
    speaker.play(tone);

    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        let deadline = Instant::now() + Duration::from_millis(100);
        let mut samples = Vec::new();
        while let Ok(frame) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            samples.extend_from_slice(&frame);
        }
        println!(
            "input rms {:.4}  playing {}",
            rms(&samples),
            speaker.is_playing()
        );
    }
}

fn exit(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}
