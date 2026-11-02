# JARVIS — Roadmap + Phase 0–1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first milestone: a talking JARVIS on macOS. Say "Jarvis" to start an
always-in-conversation session, interrupt it mid-sentence, have it launch apps, and
see it live in a full-screen holographic HUD that shrinks to an arc-reactor orb.

**Architecture:** A Cargo workspace with two crates. `jarvis-core` holds everything
non-visual: config, secrets, audio, echo cancellation, VAD, STT, LLM, TTS, memory,
tools and the session state machine. It runs on a tokio runtime in a background
thread. `jarvis-app` is the Bevy 0.20 binary (HUD, orb, settings). The two talk only
through two channels: `UiCommand` (app → core) and `CoreEvent` (core → app).
Providers are enums matched in one place, not trait-object hierarchies.

**Tech Stack:** Rust stable (2024 edition), Bevy 0.20, bevy_egui 0.43, bevy_hanabi 0.20,
tokio 1.54, cpal 0.18, rubato 5, webrtc-audio-processing 2.1, voice_activity_detector 0.2
(Silero), whisper-rs 0.16 (Metal), genai 0.7 (multi-provider LLM), reqwest 0.13,
tokio-tungstenite 0.30, rusqlite 0.41 (SQLCipher), keyring 4, serde/toml, sysinfo,
global-hotkey, cargo-packager.

**Spec:** `docs/superpowers/specs/2026-11-03-jarvis-design.md`

---

## Part A — Roadmap (all phases)

Each phase after Phase 1 gets its own detailed plan, written when that phase starts,
so it can build on what was actually learned.

| Phase | Goal | Key work | Biggest risk / spike first |
|---|---|---|---|
| **0. Setup** | Toolchain + empty workspace that builds and tests in CI | Task 0 below | none |
| **1. Talking JARVIS + HUD** | First milestone (this plan) | Tasks 1–15 below | Echo cancellation and the ~800 ms latency target |
| **2. Hands** | JARVIS acts on the computer safely | `Risk` levels (safe/confirm/forbidden) + voice/tap confirm gate in the tool dispatcher; audit log; full macOS control (shell, files, windows, clipboard, keyboard/mouse via Accessibility, volume, media); fast/strong model routing; vector recall over memory (sqlite-vec + fastembed) and "forget that"; MCP client + server (`rmcp`); web browsing/automation; coding-agent bridge over ACP (`agent-client-protocol`) with CLI fallbacks for Claude Code, Codex, Gemini CLI, OpenCode, Aider, Cursor CLI, Goose, Hermes; agent output panel in HUD | Accessibility permission UX; ACP adapter coverage per harness |
| **3. Life** | Email/calendar, messaging, home | Gmail/Google Calendar, Microsoft 365 (Graph), IMAP/CalDAV; Telegram, Slack, Discord official APIs; WhatsApp via the Business Cloud API or another ToS-compliant route; Home Assistant REST/WebSocket; GitHub; morning briefing; proactive notifications | OAuth flows in a desktop app; WhatsApp ToS |
| **4. Phone** | iOS + Android remote, store-ready | Bevy mobile app sharing HUD widgets; `iroh` P2P link with QR pairing and device revocation; voice streaming both ways; approvals; APNs/FCM wake pushes with no readable content; App Store + Play Store submission (privacy labels; OSINT shown as results only) | Background audio rules on iOS; store review |
| **5. God's-eye globe** | Full Rust port of gods-eye-view | (a) holographic globe with imagery + terrain; (b) feed services: aircraft, military, ships, satellites (SGP4), quakes, fires, weather/radar, launches, cameras, traffic; each source switchable with its license recorded; (c) track/trail/contacts/fly-to/sensor looks/director/share links; (d) a 3D Tiles streaming renderer (Google Photorealistic / Cesium ion): tile fetch, LOD, glTF/Draco, cache within provider terms; (e) voice tools | (d): no mature Rust 3D Tiles renderer exists, so spike it first and build it last |
| **6. OSINT** | Everything from cyber intel to competitor analysis | Case + entity/relationship graph store (data model has owner/case/user IDs for team mode later); authorized-scope list enforced for active collection; cyber (subdomains, DNS, CT logs, port/service scan, fingerprinting, vuln templates, Shodan/Censys/VirusTotal/breach lookups, threat feeds); company/competitor (GDELT, news APIs, SEC EDGAR, MCA India, hiring, site-change monitoring, pricing, social, tech stack); topic watches + alerts; people (username presence, public profiles; purpose recorded); graph + globe views; Markdown/PDF reports | Rate limits and ToS per source; Rust re-implementation effort for recon tools |
| **7. Reach** | Windows + Linux, Indian languages | OS-control implementations for Windows and Linux (X11 full, Wayland best-effort); Kokoro/Piper local TTS via sherpa-onnx to replace macOS `say`; Telugu/Hindi code-mixed STT (Sarvam, IndicConformer) and TTS (Sarvam, ElevenLabs, VoiceStudio); signed Windows installer, AppImage/Flatpak; performance pass | Wayland input/overlay limits |

---

## Part B — Phase 0–1 detailed plan

## Global Constraints

- All code in the repo is Rust. Crates that wrap C/C++ libraries and OS frameworks are allowed (spec §5).
- macOS (Apple Silicon, macOS 13+) is the only Phase 1 target. Keep OS-specific code in `apps.rs`/`audio.rs` so Phase 7 is a port, not a rewrite.
- No API key is ever written to a file or a log. Keys live in the OS keychain (service name `jarvis`, account = provider id). Env var `JARVIS_<PROVIDER>_API_KEY` overrides for dev/CI only.
- Audio never leaves the machine while no session is active: wake detection is always local (whisper). Cloud STT receives audio only during a session (spec §4, §11).
- Default persona: name `JARVIS`, wake phrase `jarvis`, address `sir`, calm, dry British wit, concise (spec §2.6).
- Latency target: first audio within ~800 ms of end of speech on cloud voice, ~1.2 s on local voice (spec §4). Measure it; don't guess.
- Memory DB is encrypted at rest (SQLCipher). Its key is 32 random bytes in the keychain under account `memory-key`.
- Transcripts are stored, raw audio is not (spec §15.2).
- Config file: `~/Library/Application Support/jarvis/config.toml`. Models: `~/Library/Application Support/jarvis/models/`. DB: `~/Library/Application Support/jarvis/memory.db`. (Use the `directories` crate's `ProjectDirs::from("", "", "jarvis")`.)
- Internal audio format: 16 kHz mono `f32` frames of 160 samples (10 ms). TTS output: 24 kHz mono `f32`.
- Tests that need network/keys or real hardware are `#[ignore = "<reason>"]` and are run explicitly in the task's verification step. Never leave them unrun.

## Review Focus

1. **JARVIS hears itself through the speakers.** Its own voice must not trigger barge-in or get transcribed. Covered by the AEC attenuation test (Task 4) and the session test `speaking_echo_below_threshold_is_not_barge_in` (Task 10).
2. **The provider fails mid-reply** (network drop, 401, 429, timeout). JARVIS must say a short line via the local fallback voice, show the error in the HUD, and return to Listening rather than hanging in Thinking. Session test `brain_error_speaks_fallback_and_returns_to_listening` (Task 10).
3. **First run with no API keys, or the wrong ones.** No panic: the HUD shows what's missing and opens Settings, and the local providers still work. Test `validate_reports_missing_keys` (Task 13) + `config_defaults_are_fully_local_capable` (Task 1).
4. **Mic permission denied, or no input device.** No panic: the HUD shows "Microphone access needed" with the System Settings path. Test `audio_open_error_maps_to_user_message` (Task 3).
5. **Speaking garbage aloud.** Markdown, code blocks, URLs and emoji read literally, or audio clicks from 16-bit samples split across HTTP chunks. Tests `speakable_*` (Task 6) and `pcm_decoder_handles_split_samples` (Task 7).

## Execution Notes (owner's answers for the autonomous run)

- **Open source:** public repo `ravitejarenangi/jarvis`, dual-licensed **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`, `license = "MIT OR Apache-2.0"` in every crate). No GPL/AGPL dependencies (App Store compatibility); CI enforces this with `cargo deny check licenses`.
- **Secrets in a public repo:** keys come only from env vars `JARVIS_ANTHROPIC_API_KEY` and `JARVIS_ELEVENLABS_API_KEY` (the owner provides both) or the keychain. CI runs `gitleaks` on every push. Never commit `.env`, keys, voice IDs tied to a private account, or recorded audio of the owner.
- **Keys available:** Anthropic and ElevenLabs, so their `#[ignore]` tests must be run. Deepgram/OpenAI tests stay ignored; list them in the owner checklist as "not run: no key".
- **ElevenLabs voice:** call `GET https://api.elevenlabs.io/v1/voices` and pick a calm British male premade voice (prefer "George", else "Daniel"). Store its `voice_id` as the default in `Config` and as `JARVIS_TEST_VOICE` for tests.
- **VoiceStudio:** install with `brew tap debpalash/tap && brew trust --cask debpalash/tap/voicestudio && brew install --cask debpalash/tap/voicestudio`, start it, and download its default model, so VoiceStudio tests run against `http://localhost:3900/v1`.
- **Human checkpoints are batched, not blocking.** Whenever a step says "manual", "owner look check" or needs speaking/permissions, do everything automatable, save screenshots to `docs/screens/`, append a checkbox item to `docs/owner-checklist.md` (what to do, expected result, where to report), and continue with the next task. Do not mark Phase 1 complete until the owner has ticked the checklist.
- **Apple signing:** the owner has a Developer account. Task 15 uses a `Developer ID Application` identity and a `notarytool` keychain profile; ask the owner for the identity name/profile when Task 15 starts (add it to the checklist if they're unavailable, and ship ad-hoc meanwhile).

---

## File Structure

```
jarvis/
  Cargo.toml                      # workspace, shared dependency versions
  rust-toolchain.toml             # stable
  .github/workflows/ci.yml        # macOS: fmt, clippy, test
  scripts/fetch-models.sh         # downloads whisper + silero models
  packaging/Info.plist.ext        # NSMicrophoneUsageDescription etc.
  Packager.toml                   # cargo-packager: .app + .dmg, signing
  crates/jarvis-core/
    src/lib.rs                    # pub mods + run_core()
    src/events.rs                 # UiCommand, CoreEvent, AssistantState, LatencyReport
    src/config.rs                 # Config + defaults + load/save + persona prompt
    src/secrets.rs                # keychain get/set/delete + env override
    src/audio.rs                  # cpal capture/playback, resample+frame, levels
    src/aec.rs                    # WebRTC echo canceller wrapper
    src/vad.rs                    # Silero wrapper + TurnDetector (pure)
    src/wake.rs                   # strip_wake_phrase, is_end_phrase (pure)
    src/stt.rs                    # SttProvider enum: LocalWhisper | OpenAiCompatible | Deepgram
    src/brain.rs                  # LLM streaming via genai, BrainEvent
    src/speech_text.rs            # SentenceChunker + speakable() (pure)
    src/tts.rs                    # TtsProvider enum: ElevenLabs | OpenAiCompatible | MacSay; PcmDecoder
    src/memory.rs                 # SQLCipher turns + facts
    src/tools.rs                  # tool specs + dispatch (list_apps, launch_app, remember)
    src/apps.rs                   # macOS app discovery/launch, best_app_match (pure)
    src/session.rs                # Session state machine (pure)
    src/runtime.rs                # wires everything; latency measurement
    tests/fixtures/               # wav files generated by `say`
  crates/jarvis-app/
    src/main.rs                   # Bevy app, windows, plugins
    src/link.rs                   # CoreLink resource: bridge channels <-> Bevy messages
    src/theme.rs                  # palette, fonts, bloom settings
    src/reactor.rs                # arc reactor + voice visualizer (+ orb_visual pure fn)
    src/hud.rs                    # captions, clock, system stats, status, launcher ring
    src/orb.rs                    # orb window, HUD<->orb switching, hotkeys
    src/settings.rs               # egui settings + memory browser
    assets/shaders/reactor.wgsl
    assets/fonts/                 # a free techno font with SIL OFL license (e.g. Rajdhani, Orbitron)
```

---

### Task 0: Toolchain, repo, workspace, CI

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `deny.toml`, `LICENSE-MIT`, `LICENSE-APACHE`, `README.md`, `docs/owner-checklist.md`, `crates/jarvis-core/Cargo.toml`, `crates/jarvis-core/src/lib.rs`, `crates/jarvis-app/Cargo.toml`, `crates/jarvis-app/src/main.rs`, `.github/workflows/ci.yml`, `scripts/fetch-models.sh`

**Interfaces:**
- Produces: a workspace where `cargo test --workspace` passes and `cargo run -p jarvis-app` opens an empty Bevy window.

- [ ] **Step 1: Install the toolchain and native build deps**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
xcode-select --install || true
brew install cmake meson ninja pkg-config gitleaks
cargo install cargo-packager cargo-deny --locked
```
Expected: `rustc --version` prints 1.9x stable; `cmake --version` and `meson --version` work.

- [ ] **Step 2: `git init`, workspace manifests, `rust-toolchain.toml` (channel = "stable"), `.gitignore` (`target/`, `*.bin`, `*.onnx`, `.DS_Store`)**

The workspace `Cargo.toml` declares `members = ["crates/*"]`, `resolver = "3"`, edition 2024, and pins every dependency version listed in Tech Stack under `[workspace.dependencies]`. Add `[profile.dev.package."*"] opt-level = 3` so Bevy, whisper and the audio code are fast in debug builds.

- [ ] **Step 3: `scripts/fetch-models.sh`** downloads into the models dir (from Global Constraints), skipping files already present:
  - `ggml-base.bin` (multilingual) from `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin`
  - `ggml-small.bin` (better accuracy, optional, `--all` flag)

  The Silero VAD model ships inside `voice_activity_detector`, so it needs no download.

- [ ] **Step 4: `jarvis-app/src/main.rs`**: a minimal Bevy app with `DefaultPlugins`; window title `JARVIS`.

- [ ] **Step 5: CI** `.github/workflows/ci.yml` on `macos-latest`: `gitleaks` scan, `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo deny check licenses` (`deny.toml` allows MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, OFL-1.1 for fonts; denies GPL/AGPL/LGPL), `scripts/fetch-models.sh`, `cargo test --workspace`. Cache `~/.cargo`, `target`, and the models dir.

- [ ] **Step 5b: Open-source files + public repo.** Add `LICENSE-MIT`, `LICENSE-APACHE`, a short `README.md` (what JARVIS is, status: Phase 1 in progress, build steps, license, credit to gods-eye-view (MIT) and VoiceStudio (used via API, AGPL, not bundled)), and `docs/owner-checklist.md` (empty checklist). Then `gh repo create ravitejarenangi/jarvis --public --source . --push`, only after `gitleaks detect` passes locally.

- [ ] **Step 6: Verify**

Run: `cargo test --workspace && cargo run -p jarvis-app`
Expected: tests pass (0 tests); an empty window titled JARVIS opens.

- [ ] **Step 7: Commit** `chore: workspace skeleton, toolchain, CI`

---

### Task 1: Config + persona

**Files:**
- Create: `crates/jarvis-core/src/config.rs`
- Test: same file (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `pub struct Config { pub persona: Persona, pub llm: LlmConfig, pub stt: SttConfig, pub tts: TtsConfig, pub conversation: ConversationConfig, pub hud: HudConfig }` (all `Serialize + Deserialize + Clone + Debug`, `#[serde(default)]` on every struct)
  - `pub struct Persona { pub name: String, pub wake_phrase: String, pub wake_aliases: Vec<String>, pub address: String, pub personality: String, pub languages: Vec<String> }`
  - `pub struct LlmConfig { pub provider: String, pub model: String, pub base_url: Option<String> }`
  - `pub enum SttChoice { LocalWhisper, OpenAiCompatible { base_url: String, model: String }, Deepgram { model: String } }` and `pub struct SttConfig { pub provider: SttChoice, pub whisper_model: String }`
  - `pub enum TtsChoice { ElevenLabs { voice_id: String, model: String }, OpenAiCompatible { base_url: String, model: String, voice: String }, MacSay { voice: String } }` and `pub struct TtsConfig { pub provider: TtsChoice, pub fallback: TtsChoice }`
  - `pub struct ConversationConfig { pub end_of_turn_silence_ms: u32, pub session_idle_timeout_s: u32, pub end_phrases: Vec<String>, pub expand_hud_on_wake: bool }`
  - `pub struct HudConfig { pub favorite_apps: Vec<String>, pub orb_position: Option<(i32, i32)>, pub animation_intensity: f32, pub captions: bool }`
  - `impl Config { pub fn load(path: &Path) -> Result<Config, ConfigError>; pub fn save(&self, path: &Path) -> Result<(), ConfigError>; pub fn default_path() -> PathBuf }`
  - `impl Persona { pub fn system_prompt(&self, now: &str, active_app: Option<&str>, facts: &[String]) -> String }`
  - `#[derive(thiserror::Error)] pub enum ConfigError { Io(PathBuf, io::Error), Parse(PathBuf, toml::de::Error) }`. The display string includes the file path.

Defaults (copy exactly):
- persona: name `JARVIS`, wake_phrase `jarvis`, wake_aliases `["jervis", "jarvis.", "hey jarvis"]`, address `sir`, languages `["en", "hi", "te"]`, personality: `"You are JARVIS, a calm, highly capable British AI assistant with dry wit. Address the user as {address}. Be concise: one or two sentences unless asked for detail. You are speaking aloud, so never use markdown, lists, code blocks or URLs in replies; describe them and say you've put them on screen."`
- llm: provider `anthropic`, model `claude-sonnet-5-5`, base_url `None`
- stt: `LocalWhisper`, whisper_model `ggml-base.bin`
- tts: provider `ElevenLabs { voice_id: "", model: "eleven_flash_v2_5" }`, fallback `MacSay { voice: "Daniel" }`
- conversation: end_of_turn_silence_ms `500`, session_idle_timeout_s `60`, end_phrases `["that'll be all", "that will be all", "goodbye jarvis", "stand by"]`, expand_hud_on_wake `true`
- hud: favorite_apps `["Safari", "Mail", "Calendar", "Music", "Visual Studio Code", "Terminal"]`, orb_position `None`, animation_intensity `1.0`, captions `true`

- [ ] **Step 1: Write failing tests**

```rust
#[test] fn missing_file_loads_defaults() { /* load(tmp/"nope.toml") == Ok(Config::default()) */ }
#[test] fn partial_file_fills_defaults() { /* "[persona]\nname = \"FRIDAY\"" -> name FRIDAY, wake_phrase "jarvis", llm.model default */ }
#[test] fn save_then_load_roundtrips() { /* modify several fields incl. enum variants, save, load, assert_eq */ }
#[test] fn invalid_toml_error_names_the_file() { /* "[persona" -> Err, err.to_string() contains file name */ }
#[test] fn system_prompt_includes_address_time_app_and_facts() { /* contains "sir", the time string, "Safari", each fact */ }
#[test] fn config_defaults_are_fully_local_capable() { /* default stt is LocalWhisper and tts.fallback is MacSay */ }
```

- [ ] **Step 2: Run** `cargo test -p jarvis-core config`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement** the types and functions above. `load` returns the defaults when the file doesn't exist. `save` creates parent dirs and writes atomically (write `.tmp`, then rename) so a crash can't corrupt the config. `system_prompt` substitutes `{address}` and appends sections `Current time: …`, `Active app: …`, `Things you know about the user:` + bullet facts.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): config and persona`

---

### Task 2: Secrets (keychain)

**Files:**
- Create: `crates/jarvis-core/src/secrets.rs`

**Interfaces:**
- Produces:
  - `pub fn get_key(provider: &str) -> Option<String>` (env `JARVIS_<PROVIDER_UPPER>_API_KEY` first, then keychain service `jarvis`, account `provider`)
  - `pub fn set_key(provider: &str, key: &str) -> Result<(), SecretError>`
  - `pub fn delete_key(provider: &str) -> Result<(), SecretError>`
  - `pub fn memory_key() -> Result<[u8; 32], SecretError>`: returns the stored key, or generates 32 random bytes (`getrandom`), stores them hex-encoded under account `memory-key`, and returns them.
  - Provider ids used everywhere: `anthropic`, `openai`, `gemini`, `openrouter`, `xai`, `groq`, `deepseek`, `mistral`, `elevenlabs`, `deepgram`, `voicestudio`.

- [ ] **Step 1: Write failing tests** using keyring's mock credential store (`keyring::set_default_credential_builder(mock::default_credential_builder())`):
  - `env_var_overrides_keychain`
  - `set_get_delete_roundtrip`
  - `memory_key_is_stable_across_calls` (two calls return the same 32 bytes)
  - `get_missing_key_is_none`
- [ ] **Step 2: Run** `cargo test -p jarvis-core secrets`. Expected: FAIL.
- [ ] **Step 3: Implement.** Never log key values. `SecretError` wraps `keyring::Error`.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): keychain secrets`

---

### Task 3: Audio I/O and events

**Files:**
- Create: `crates/jarvis-core/src/events.rs`, `crates/jarvis-core/src/audio.rs`, `crates/jarvis-core/examples/mic_loopback.rs`

**Interfaces:**
- Produces (`events.rs`):
  - `pub enum AssistantState { Idle, Listening, Thinking, Speaking, Muted }`
  - `pub struct LatencyReport { pub stt_ms: u32, pub first_token_ms: u32, pub first_audio_ms: u32 }` (all measured from end of user speech)
  - `pub enum CoreEvent { State(AssistantState), UserTranscript { text: String, is_final: bool }, AssistantText { delta: String }, AssistantTurnDone, AudioLevel { input: f32, output: f32 }, ToolActivity { name: String, summary: String }, Error { message: String, open_settings: bool }, Latency(LatencyReport), Facts(Vec<(i64, String)>) }`
  - `pub enum UiCommand { StartSession, EndSession, ToggleMute, ReloadConfig, SetApiKey { provider: String, key: String }, LaunchApp(String), ListFacts, DeleteFact(i64), Shutdown }`
- Produces (`audio.rs`):
  - `pub const FRAME: usize = 160;` (10 ms @ 16 kHz)
  - `pub struct Framer` with `pub fn new(input_rate: u32, channels: u16) -> Self` and `pub fn push(&mut self, interleaved: &[f32]) -> Vec<[f32; FRAME]>`: downmixes to mono, resamples to 16 kHz (rubato), and emits whole frames, keeping the remainder.
  - `pub fn rms(samples: &[f32]) -> f32`
  - `pub struct Mic` with `pub fn start(tx: std::sync::mpsc::SyncSender<[f32; FRAME]>) -> Result<Mic, AudioError>` (default input device; dropping it stops capture)
  - `pub struct Speaker` with `pub fn start() -> Result<Speaker, AudioError>`, `pub fn play(&self, pcm24k: Vec<f32>)`, `pub fn stop_now(&self)` (clears the queue within one callback, for barge-in), `pub fn is_playing(&self) -> bool`, `pub fn render_tap(&self) -> Receiver<[f32; FRAME]>` (what's being played, resampled to 16 kHz frames, for the AEC far-end)
  - `pub enum AudioError { NoInputDevice, NoOutputDevice, PermissionDenied, Backend(String) }` with `pub fn user_message(&self) -> String`

- [ ] **Step 1: Write failing tests**
  - `framer_48k_stereo_one_second_gives_100_frames` (push 48 000 stereo samples, possibly in uneven chunks → exactly 100 frames, ±1 for resampler delay)
  - `framer_keeps_remainder_between_pushes`
  - `rms_of_full_scale_sine_is_about_0_707`
  - `audio_open_error_maps_to_user_message`: `PermissionDenied.user_message()` contains `"System Settings > Privacy & Security > Microphone"`; `NoInputDevice` mentions connecting a microphone.
- [ ] **Step 2: Run** `cargo test -p jarvis-core audio`. Expected: FAIL.
- [ ] **Step 3: Implement.** cpal callbacks must not allocate or block: use a preallocated ring buffer (`ringbuf`) between the callback and a worker thread that runs `Framer`. The `Speaker` resamples 24 kHz to the device rate. Map cpal's build-stream errors on macOS to `PermissionDenied` when the device exists but the stream fails to open.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Manual hardware check.** `cargo run -p jarvis-core --example mic_loopback` prints the input RMS 10×/s and plays a 1 s 440 Hz tone. Expected: the numbers rise when you speak, and the tone is audible.
- [ ] **Step 6: Commit** `feat(core): audio capture, playback, framing`

---

### Task 4: Echo cancellation + VAD + turn detection  ⚠️ spike-sensitive

**Files:**
- Create: `crates/jarvis-core/src/aec.rs`, `crates/jarvis-core/src/vad.rs`

**Interfaces:**
- Consumes: `FRAME`, `rms` (Task 3)
- Produces:
  - `pub struct EchoCanceller` with `pub fn new() -> Result<Self, AecError>`, `pub fn far_end(&mut self, frame: &[f32; FRAME])`, `pub fn near_end(&mut self, frame: &mut [f32; FRAME])` (in place). WebRTC APM config: echo canceller on (full AEC, not mobile), noise suppression moderate, high-pass filter on, AGC off.
  - `pub struct Vad` with `pub fn new() -> Result<Self, VadError>` and `pub fn speech_prob(&mut self, frame: &[f32; FRAME]) -> f32` (Silero wants 512-sample chunks at 16 kHz, so buffer frames internally and return the latest probability)
  - `pub enum TurnEvent { SpeechStart, SpeechEnd }`
  - `pub struct TurnDetector` with `pub fn new(silence_ms: u32) -> Self` and `pub fn push(&mut self, prob: f32) -> Option<TurnEvent>`. Rules: `SpeechStart` after **20 ms** with prob ≥ 0.5 (2 consecutive frames); `SpeechEnd` after `silence_ms` with prob < 0.35 following a start. Hysteresis keeps it from flapping.

- [ ] **Step 1: Write failing tests**
  - `turn_detector_ignores_single_frame_blip`
  - `turn_detector_start_then_end_after_silence` (silence_ms 500 → `SpeechEnd` exactly at the 50th quiet frame)
  - `turn_detector_short_pause_does_not_end_turn` (300 ms dip inside speech → no `SpeechEnd`)
  - `vad_silence_is_low_and_speech_is_high`: `tests/fixtures/open_safari.wav` peaks > 0.5; 1 s of zeros stays < 0.2. Generate the fixture in this step: `say -v Daniel -o crates/jarvis-core/tests/fixtures/open_safari.wav --data-format=LEI16@16000 "Open Safari"`.
  - `aec_attenuates_echo_by_10db`: feed `open_safari.wav` as far-end, and the same signal delayed 40 ms and scaled 0.6 as near-end. After the first 300 ms of convergence, the output RMS is ≥ 10 dB below the input RMS.
- [ ] **Step 2: Run** `cargo test -p jarvis-core vad aec`. Expected: FAIL.
- [ ] **Step 3: Implement.** If `webrtc-audio-processing` (feature `bundled`) fails to build, **stop and report** before trying alternatives. The fallback to discuss is macOS's VoiceProcessingIO audio unit via `objc2-audio-toolbox`, which also cancels echo but changes Task 3's `Mic`.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): echo cancellation, VAD, turn detection`

---

### Task 5: Speech-to-text providers + wake phrase

**Files:**
- Create: `crates/jarvis-core/src/stt.rs`, `crates/jarvis-core/src/wake.rs`
- Test fixtures: `tests/fixtures/jarvis_open_safari.wav` (`say -v Daniel … "Jarvis, open Safari"`)

**Interfaces:**
- Consumes: `SttChoice`, `SttConfig` (Task 1), `get_key` (Task 2)
- Produces:
  - `pub struct LocalWhisper` with `pub fn load(model_path: &Path) -> Result<Self, SttError>` and `pub fn transcribe(&self, pcm16k: &[f32], language: Option<&str>) -> Result<String, SttError>` (Metal on; language `None` means auto-detect). It always exists, because wake detection uses it.
  - `pub async fn transcribe_utterance(choice: &SttChoice, whisper: &LocalWhisper, pcm16k: &[f32]) -> Result<String, SttError>`:
    - `LocalWhisper` → whisper
    - `OpenAiCompatible { base_url, model }` → `POST {base_url}/audio/transcriptions` as multipart with a WAV-encoded body and `model`; bearer key from `get_key("openai")`, or `get_key("voicestudio")` when `base_url` is localhost (where the key is optional). This one variant covers OpenAI and VoiceStudio (`http://localhost:3900/v1`).
    - `Deepgram { model }` → `POST https://api.deepgram.com/v1/listen?model={model}&smart_format=true&detect_language=true`, header `Authorization: Token <key>`, body WAV.
  - `pub fn strip_wake_phrase(text: &str, phrase: &str, aliases: &[String]) -> Option<String>`: case-insensitive; ignores leading "hey"/"ok" and the punctuation after the name. Returns the remaining command, trimmed (may be empty), or `None` if the text doesn't start with the phrase or an alias.
  - `pub fn is_end_phrase(text: &str, end_phrases: &[String]) -> bool` (normalized whole-utterance match)

  Phase 1 transcribes whole utterances, because one code path serves every provider. Streaming STT (Deepgram WS, VoiceStudio `WS /v1/audio/transcriptions/stream`) comes in only if Task 15's latency measurement shows the STT stage over 350 ms (`ponytail:` comment at the dispatch site).

- [ ] **Step 1: Write failing tests**
  - `strip_wake_cases`: `"Jarvis, open Safari."`→`Some("open Safari.")`; `"hey jarvis what's the time"`→`Some("what's the time")`; `"Jarvis."`→`Some("")`; `"Jervis open mail"` (alias)→`Some("open mail")`; `"I told jarvis to wait"`→`None`; `""`→`None`
  - `end_phrase_cases`: `"That'll be all."`→true; `"that'll be all for the report, then summarise it"`→false
  - `whisper_transcribes_fixture` (needs the model from fetch-models): transcription of `jarvis_open_safari.wav` lowercased contains `"safari"`, and `strip_wake_phrase` returns `Some` of it
  - `#[ignore = "needs DEEPGRAM key"] deepgram_transcribes_fixture`; `#[ignore = "needs VoiceStudio running on :3900"] voicestudio_transcribes_fixture`
- [ ] **Step 2: Run** `cargo test -p jarvis-core stt wake`. Expected: FAIL.
- [ ] **Step 3: Implement.** Map HTTP 401/403 to `SttError::BadKey(provider)` and everything else to `SttError::Provider(String)`.
- [ ] **Step 4: Run** the tests, then the ignored ones with keys/VoiceStudio available: `cargo test -p jarvis-core stt -- --ignored`. Expected: PASS. Report any ignored test you could not run, and why.
- [ ] **Step 5: Commit** `feat(core): STT providers and wake phrase`

---

### Task 6: LLM brain, sentence chunking, speakable text

**Files:**
- Create: `crates/jarvis-core/src/brain.rs`, `crates/jarvis-core/src/speech_text.rs`

**Interfaces:**
- Consumes: `LlmConfig` (Task 1), `get_key` (Task 2)
- Produces:
  - `pub struct ToolSpec { pub name: String, pub description: String, pub schema: serde_json::Value }`
  - `pub enum Role { User, Assistant, Tool { call_id: String } }` and `pub struct Turn { pub role: Role, pub text: String, pub tool_calls: Vec<ToolCall> }`
  - `pub struct ToolCall { pub id: String, pub name: String, pub args: serde_json::Value }`
  - `pub enum BrainEvent { TextDelta(String), ToolCalls(Vec<ToolCall>), Done }`
  - `pub fn stream_reply(cfg: &LlmConfig, system: String, history: Vec<Turn>, tools: Vec<ToolSpec>) -> impl Stream<Item = Result<BrainEvent, BrainError>>`. Uses `genai`: provider string → genai adapter (`anthropic`, `openai`, `gemini`, `openrouter`, `xai`, `groq`, `deepseek`, `mistral`, `ollama`), plus `openai-compatible` with `base_url` for LM Studio/vLLM/any endpoint. Keys come from `get_key(provider)` through genai's auth resolver. Dropping the stream cancels the request (barge-in relies on this).
  - `pub struct SentenceChunker` with `pub fn push(&mut self, delta: &str) -> Vec<String>` and `pub fn finish(&mut self) -> Option<String>`. It emits a chunk at `. ! ? … ।` or a newline, followed by whitespace, once the chunk is ≥ 12 chars (the first chunk of a reply: ≥ 4 chars, so speech starts fast). It does not split on `3.14`, `e.g.`, `Mr.`, `Dr.`, `vs.`, or `U.S.`
  - `pub fn speakable(text: &str) -> String`: strips markdown emphasis and headings; replaces fenced code blocks with `"I've put the code on screen."` and URLs with `"the link on screen"`; drops emoji; turns list bullets into plain sentences.

- [ ] **Step 1: Write failing tests**
  - `chunker_first_chunk_is_early`: deltas `"Yes"`, `", sir. "`, `"Opening it now."` → first output `"Yes, sir."`
  - `chunker_does_not_split_decimals_or_abbreviations`: `"Pi is 3.14 approx. Dr. Banner agrees. "` → `["Pi is 3.14 approx.", "Dr. Banner agrees."]`. Note "approx." ends that chunk; only the listed abbreviations are protected.
  - `chunker_handles_devanagari_danda`: `"नमस्ते। आप कैसे हैं? "` → two chunks
  - `chunker_finish_flushes_tail`
  - `speakable_strips_markdown`, `speakable_replaces_code_block`, `speakable_replaces_urls`, `speakable_drops_emoji`
  - `#[ignore = "needs ANTHROPIC key"] anthropic_streams_text_and_tool_call`: with a `get_time` tool spec and prompt "What time is it? Use the tool." → yields `ToolCalls` naming `get_time`
- [ ] **Step 2: Run** `cargo test -p jarvis-core brain speech_text`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests, plus the ignored one with a key. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): streaming LLM brain, sentence chunking, speakable text`

---

### Task 7: Text-to-speech providers

**Files:**
- Create: `crates/jarvis-core/src/tts.rs`

**Interfaces:**
- Consumes: `TtsChoice` (Task 1), `get_key` (Task 2)
- Produces:
  - `pub struct PcmDecoder` with `pub fn push(&mut self, bytes: &[u8]) -> Vec<f32>`: 16-bit little-endian to f32, carrying an odd trailing byte over to the next push.
  - `pub fn synthesize(choice: &TtsChoice, text: String) -> impl Stream<Item = Result<Vec<f32>, TtsError>>` yields 24 kHz mono chunks as they arrive:
    - `ElevenLabs { voice_id, model }` → `POST https://api.elevenlabs.io/v1/text-to-speech/{voice_id}/stream?output_format=pcm_24000`, header `xi-api-key`, JSON `{ "text", "model_id": model }`
    - `OpenAiCompatible { base_url, model, voice }` → `POST {base_url}/audio/speech` JSON `{ model, voice, input, response_format: "pcm" }` (OpenAI pcm = 24 kHz s16le). The same variant serves VoiceStudio at `http://localhost:3900/v1`. If VoiceStudio rejects `pcm`, request `wav` and strip the 44-byte header; check this in the ignored test and record which one works in a code comment.
    - `MacSay { voice }` → `say -v {voice} -o <tmp>.wav --data-format=LEI16@24000 <text>`, then read the samples. This is the offline fallback (`ponytail:` macOS only; Phase 7 replaces it with Kokoro via sherpa-onnx).
  - `pub async fn synthesize_with_fallback(cfg: &TtsConfig, text: String) -> Result<(futures::stream::BoxStream<'static, Result<Vec<f32>, TtsError>>, bool), TtsError>`: the bool is `used_fallback`. If the primary fails before its first chunk, it uses `cfg.fallback`.
  - `ElevenLabs` with an empty `voice_id` is a config error (`TtsError::NotConfigured`), which triggers the fallback.

- [ ] **Step 1: Write failing tests**
  - `pcm_decoder_handles_split_samples`: bytes of `[0x00,0x40, 0x00,0xC0]` pushed as `[0x00]`, `[0x40,0x00]`, `[0xC0]` → `[0.5, -0.5]` total
  - `mac_say_produces_audio` (macOS): non-empty, RMS > 0.01
  - `empty_elevenlabs_voice_falls_back`: `synthesize_with_fallback` with voice_id "" returns `used_fallback == true` and audio
  - `#[ignore = "needs ELEVENLABS key + voice id in JARVIS_TEST_VOICE"] elevenlabs_streams_audio`
  - `#[ignore = "needs VoiceStudio on :3900"] voicestudio_streams_audio`
- [ ] **Step 2: Run** `cargo test -p jarvis-core tts`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests and the ignored ones. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): TTS providers with fallback`

---

### Task 8: Encrypted memory

**Files:**
- Create: `crates/jarvis-core/src/memory.rs`

**Interfaces:**
- Consumes: `memory_key()` (Task 2)
- Produces:
  - `pub struct Memory` with:
    - `pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self, MemoryError>`: `PRAGMA key = "x'<hex>'"`, then creates the tables if missing
    - `pub fn append_turn(&self, session_id: &str, role: &str, text: &str) -> Result<()>`
    - `pub fn recent_turns(&self, session_id: &str, limit: usize) -> Result<Vec<(String, String)>>` (role, text), oldest first
    - `pub fn add_fact(&self, text: &str) -> Result<i64>`
    - `pub fn list_facts(&self) -> Result<Vec<(i64, String)>>`
    - `pub fn delete_fact(&self, id: i64) -> Result<bool>`
  - Schema: `turns(id INTEGER PK, session_id TEXT, role TEXT, text TEXT, ts INTEGER)`, `facts(id INTEGER PK, text TEXT UNIQUE, ts INTEGER)`
  - rusqlite feature: `bundled-sqlcipher-vendored-openssl`

  All facts are injected into the prompt (`ponytail:` no vector search; Phase 2 adds sqlite-vec recall once facts exceed ~200).

- [ ] **Step 1: Write failing tests**
  - `turns_roundtrip_in_order`
  - `duplicate_fact_is_not_added_twice` (returns the existing id)
  - `delete_fact_returns_false_for_unknown_id`
  - `wrong_key_cannot_read`: open with key A, write, close; open with key B → `Err`
  - `file_is_not_plaintext`: the raw file bytes don't contain the stored fact text
- [ ] **Step 2: Run** `cargo test -p jarvis-core memory`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): encrypted memory`

---

### Task 9: Tools: apps + remember

**Files:**
- Create: `crates/jarvis-core/src/apps.rs`, `crates/jarvis-core/src/tools.rs`

**Interfaces:**
- Consumes: `ToolSpec`, `ToolCall` (Task 6), `Memory` (Task 8)
- Produces:
  - `pub fn installed_apps() -> Vec<String>`: `.app` bundle names (without `.app`) from `/Applications`, `/Applications/Utilities`, `/System/Applications`, `/System/Applications/Utilities`, `~/Applications`; sorted, deduplicated
  - `pub fn best_app_match<'a>(query: &str, apps: &'a [String]) -> Option<&'a str>`: case-insensitive; exact > starts-with > all query words contained; ties go to the shortest name
  - `pub fn launch_app(name: &str) -> Result<(), String>` → `open -a <name>`; non-zero exit → `Err` with stderr
  - `pub fn frontmost_app() -> Option<String>` (`NSWorkspace.frontmostApplication` via `objc2-app-kit`)
  - `pub fn tool_specs() -> Vec<ToolSpec>`: `list_apps {}`, `launch_app { name: string }`, `remember { fact: string }`
  - `pub fn execute(call: &ToolCall, memory: &Memory) -> String`: always returns a string for the model; failures become `"Error: …"` text so the model can tell the user, never a panic

- [ ] **Step 1: Write failing tests**
  - `best_match_cases` over `["Safari", "Visual Studio Code", "Calendar", "Calculator", "Code Runner"]`: `"safari"`→Safari; `"calc"`→Calculator (shortest of the two prefix matches); `"studio code"`→Visual Studio Code; `"zzz"`→None
  - `installed_apps_includes_safari` (macOS)
  - `execute_launch_unknown_app_returns_error_text`: `{"name":"NoSuchApp123"}` → starts with `"Error:"` and mentions the name
  - `execute_remember_stores_fact`
  - `execute_bad_args_returns_error_text`: `launch_app` with `{}` → `"Error:"`
- [ ] **Step 2: Run** `cargo test -p jarvis-core apps tools`. Expected: FAIL.
- [ ] **Step 3: Implement.** `launch_app` resolves through `best_app_match` first, so "open vs code" works when the model passes a loose name.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): app tools and remember`

---

### Task 10: Session state machine

**Files:**
- Create: `crates/jarvis-core/src/session.rs`

The heart of the conversation feel. It's pure logic with no I/O, so every behavior is tested here and `runtime.rs` (Task 11) just executes the actions.

**Interfaces:**
- Consumes: `AssistantState` (Task 3), `strip_wake_phrase`, `is_end_phrase` (Task 5), `ConversationConfig`, `Persona` (Task 1)
- Produces:
  - `pub enum Input { WakeTranscript(String), UserTranscript(String), SpeechStart { echo_suppressed_rms: f32 }, BrainText(String), BrainDone, BrainError { provider: String, message: String }, SpeakingFinished, Tick { now_ms: u64 }, ManualStart, ManualEnd, ToggleMute }`
  - `pub enum Action { SetState(AssistantState), TranscribeForWake, TranscribeUtterance, AskBrain { user_text: String }, Speak(String), StopSpeaking, CancelBrain, SpeakFallback(String), ExpandHud, Emit(CoreEvent) }`
  - `pub struct Session` with `pub fn new(cfg: ConversationConfig, persona: Persona) -> Self` and `pub fn handle(&mut self, input: Input, now_ms: u64) -> Vec<Action>`
  - `pub const BARGE_IN_MIN_RMS: f32 = 0.02;` (post-AEC speech energy needed to count as barge-in; tune in Task 15)

Rules (the tests pin these):
- **Idle:** a speech end leads to `TranscribeForWake`. A `WakeTranscript` that matches the wake phrase → state Listening, plus `ExpandHud` if `expand_hud_on_wake`; a non-empty command → `AskBrain`; an empty one → `Speak("Yes, {address}?")`. A non-match changes nothing.
- **Listening:** `UserTranscript`: an end phrase → `Speak("Very good, {address}.")` then Idle; empty text → ignore; otherwise `AskBrain` and Thinking.
- **Thinking/Speaking:** `BrainText` → `Speak(chunk)` and Speaking; `BrainDone` + `SpeakingFinished` → Listening.
- **Barge-in:** `SpeechStart` while Speaking or Thinking with `echo_suppressed_rms ≥ BARGE_IN_MIN_RMS` → `StopSpeaking`, `CancelBrain`, Listening.
- **Brain error** → `SpeakFallback("Apologies, {address}, I can't reach my {provider} systems right now.")`, an `Emit(Error)`, then Listening.
- **Idle timeout:** no user speech for `session_idle_timeout_s` while Listening → Idle (silently).
- **Muted:** ignores all audio inputs; `ToggleMute` toggles; `StopSpeaking` on entry.

- [ ] **Step 1: Write failing tests** (one per rule)
  - `wake_with_command_asks_brain`, `wake_alone_greets`, `non_wake_speech_is_ignored_when_idle`
  - `end_phrase_returns_to_idle`
  - `barge_in_stops_speech_and_cancels_brain`
  - `speaking_echo_below_threshold_is_not_barge_in`
  - `brain_error_speaks_fallback_and_returns_to_listening`
  - `idle_timeout_returns_to_idle`
  - `muted_ignores_wake`
  - `full_conversation_sequence`: wake → reply → user follow-up with no wake word → reply → "that'll be all" → Idle
- [ ] **Step 2: Run** `cargo test -p jarvis-core session`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): conversation session state machine`

---

### Task 11: Core runtime wiring + latency measurement

**Files:**
- Create: `crates/jarvis-core/src/runtime.rs`
- Modify: `crates/jarvis-core/src/lib.rs`
- Create: `crates/jarvis-core/examples/headless.rs` (runs the core with a terminal printout, no UI)

**Interfaces:**
- Consumes: everything from Tasks 1–10
- Produces: `pub fn run_core(cmd_rx: tokio::sync::mpsc::UnboundedReceiver<UiCommand>, event_tx: tokio::sync::mpsc::UnboundedSender<CoreEvent>) -> std::thread::JoinHandle<()>` (spawns its own multi-thread tokio runtime; loads config, secrets, memory and the whisper model; never panics on provider/config errors and reports them as `CoreEvent::Error`)

Pipeline per 10 ms frame:
1. Speaker render tap → `EchoCanceller::far_end`
2. Mic frame → `near_end` → `Vad::speech_prob` → `TurnDetector`
3. Keep a rolling pre-roll buffer (300 ms) plus the current utterance buffer
4. On `SpeechEnd` → `TranscribeForWake` (local whisper) or `TranscribeUtterance` (configured STT), as the Session asks

The brain loop: build the system prompt (persona + now + `frontmost_app()` + `list_facts`); the history is the last 20 turns from memory; stream the reply; feed `TextDelta` through `SentenceChunker` → `speakable` → `Session` → `Speak`. Tool calls: run `tools::execute`, append the results, call `stream_reply` again (at most 5 rounds per user turn), and emit `ToolActivity`. Every user and assistant turn is appended to memory.

TTS: a single-consumer queue keeps sentences in order; synthesis of sentence N+1 starts while N plays. `StopSpeaking` clears the queue and calls `Speaker::stop_now`.

Latency: timestamp `SpeechEnd`, STT done, first `TextDelta`, and first audio sample played. Emit `CoreEvent::Latency` and log via `tracing` each turn.

- [ ] **Step 1: Write a failing integration test** `runtime_reports_missing_mic_or_key_without_panicking`. Run the core with an empty config dir (`JARVIS_CONFIG_DIR` env override, which `Config::default_path` and the memory/models paths must honor) and the anthropic key unset. Within 10 s, the event stream contains `CoreEvent::Error { open_settings: true, .. }` or `State(Idle)`, and the thread is still alive.
- [ ] **Step 2: Run** `cargo test -p jarvis-core runtime`. Expected: FAIL.
- [ ] **Step 3: Implement** `run_core`. Add the `JARVIS_CONFIG_DIR` override to `config.rs` (Task 1 test file gets one more test: `config_dir_env_override`).
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Manual end-to-end (headless).** `cargo run -p jarvis-core --example headless` with an Anthropic key set. Say "Jarvis, open Safari". Expected: Safari opens, JARVIS confirms aloud, and the terminal prints the latency report. Then talk for 2 minutes over speakers, with interruptions. Expected: no self-triggering, and barge-in stops speech promptly.
- [ ] **Step 6: Commit** `feat(core): runtime pipeline with latency reporting`

---

### Task 12: Bevy shell, core link, arc reactor

**Files:**
- Modify: `crates/jarvis-app/src/main.rs`
- Create: `crates/jarvis-app/src/link.rs`, `src/theme.rs`, `src/reactor.rs`, `assets/shaders/reactor.wgsl`, `assets/fonts/<font>.ttf` (+ its OFL license file)

**Interfaces:**
- Consumes: `run_core`, `UiCommand`, `CoreEvent`, `AssistantState` (Tasks 3, 11)
- Produces:
  - `#[derive(Resource)] pub struct CoreLink { pub tx: UnboundedSender<UiCommand> }`, plus a system that drains the core's receiver every frame and re-sends each `CoreEvent` as a Bevy message `CoreMsg(CoreEvent)`
  - `#[derive(Resource, Default)] pub struct AssistantView { pub state: AssistantState, pub input_level: f32, pub output_level: f32 }`, updated from `CoreMsg`
  - `pub struct OrbVisual { pub scale: f32, pub glow: f32, pub color: Color, pub ring_speed: f32 }`
  - `pub fn orb_visual(state: AssistantState, input: f32, output: f32, t: f32, intensity: f32) -> OrbVisual`: Idle = dim cyan with a slow breathing pulse; Listening = bright cyan, scale follows input level; Thinking = cyan→amber rotating rings, faster; Speaking = scale and glow follow the output level; Muted = grey, static.
  - `pub fn send(link: &CoreLink, cmd: UiCommand)`
- Theme (copy exactly): background `#02070D` at 92% alpha; primary cyan `#3CE7FF`; secondary amber `#FFB547`; alert red `#FF4D5E`; text `#CFF7FF`; bloom intensity 0.35 (HDR camera + `Bloom`).

The main window is borderless and fullscreen on the primary monitor, with transparency enabled (`composite_alpha_mode` post-multiplied on macOS). The reactor is concentric ring meshes with a WGSL material driven by `OrbVisual` uniforms, plus `bevy_hanabi` sparks while Speaking.

- [ ] **Step 1: Write failing tests** (pure, in `reactor.rs`)
  - `orb_idle_breathes_within_bounds`: for t in 0..10 s, scale stays in [0.95, 1.05]
  - `orb_speaking_scales_with_output`: output 0.0 vs 0.8 → the scale difference is ≥ 0.15
  - `orb_muted_is_static`: same visual at t = 0 and t = 3
  - `orb_zero_intensity_is_static`: intensity 0.0 while Speaking at output 0.8 → identical visual at t = 0 and t = 3
  - `orb_thinking_color_is_between_cyan_and_amber`
- [ ] **Step 2: Run** `cargo test -p jarvis-app reactor`. Expected: FAIL.
- [ ] **Step 3: Implement** link, theme, reactor and main wiring (start `run_core` in `Startup` and store `CoreLink`; send `UiCommand::Shutdown` on `AppExit`).
- [ ] **Step 4: Run** the tests, then `cargo run -p jarvis-app`. Expected: tests PASS; a fullscreen dark HUD with a glowing reactor that pulses when you talk to it.
- [ ] **Step 5: Owner look check.** Save a screenshot (Bevy `Screenshot`) to `docs/screens/phase1-reactor.png` and show it to the owner. Adjust the visuals until they approve.
- [ ] **Step 6: Commit** `feat(app): Bevy shell, core link, arc reactor`

---

### Task 13: HUD panels + settings

**Files:**
- Create: `crates/jarvis-app/src/hud.rs`, `crates/jarvis-app/src/settings.rs`
- Modify: `crates/jarvis-core/src/config.rs` (add `validate`)

**Interfaces:**
- Consumes: `CoreMsg`, `CoreLink`, `send`, theme (Task 12); `Config`, `get_key`/`set_key` (Tasks 1, 2)
- Produces:
  - `pub struct Captions { lines: VecDeque<(Who, String)> }` with `pub fn push_delta(&mut self, who: Who, delta: &str)`, `pub fn finish_turn(&mut self)` and `pub fn visible(&self) -> &VecDeque<…>` (keeps the last 6 lines; a delta appends to the current line); `pub enum Who { User, Jarvis }`
  - `pub fn launcher_positions(n: usize, radius: f32, start_angle: f32) -> Vec<Vec2>` (evenly spaced on a ring)
  - in `jarvis-core::config`: `pub fn validate(cfg: &Config, key_present: impl Fn(&str) -> bool) -> Vec<String>`: human-readable problems, e.g. `"ElevenLabs is selected but no ElevenLabs API key is set."`, `"ElevenLabs voice id is empty."`, `"No API key for anthropic."`; an empty list means OK
- HUD panels (Bevy UI, themed): captions (bottom center); clock + date (top left); CPU/RAM/network (top right, `sysinfo`, refreshed 1 Hz); state badge + mute indicator (top center: "LISTENING" / "THINKING" / "SPEAKING" / "STANDBY" / "MUTED"); tool activity ticker; launcher ring of `favorite_apps` around the reactor (click → `UiCommand::LaunchApp`); last latency report (small, bottom right, toggled by `F3`).
- Settings (bevy_egui with a custom dark-cyan `egui::Visuals`), opened with `Cmd+,` or the gear icon, or automatically on `CoreEvent::Error { open_settings: true }`:
  - **Brain:** a provider dropdown (the provider ids from Task 2 + `ollama` + `openai-compatible`), model text field, base URL
  - **Voice:** TTS provider, voice id/name, model; STT provider; a "Test voice" button that speaks a sample line
  - **Keys:** a masked field per provider; Save → `UiCommand::SetApiKey`; shows "set"/"not set", never the value
  - **Persona:** name, wake phrase, aliases, address, personality text, languages
  - **Conversation:** silence ms (slider 300–1200), idle timeout, end phrases, expand-HUD-on-wake
  - **Display:** animation intensity slider 0–100% (spec §8 accessibility; stored as `HudConfig.animation_intensity: f32`, default 1.0; multiplies pulse/ring/spark amplitudes in `orb_visual`), captions on/off
  - **Memory:** a fact list with a delete button per row (`ListFacts` / `DeleteFact`)
  - **Problems:** the `validate` output shown in red at the top
  - Save writes the config → `UiCommand::ReloadConfig`

- [ ] **Step 1: Write failing tests**
  - `captions_keep_last_six_lines`, `captions_delta_appends_to_current_line`
  - `launcher_positions_are_evenly_spaced` (n = 6 → adjacent distances equal within 1e-3)
  - `validate_reports_missing_keys` (ElevenLabs selected, no key → the exact message above)
  - `validate_ok_for_default_with_anthropic_key` → only the ElevenLabs voice id problem
- [ ] **Step 2: Run** `cargo test --workspace hud settings validate`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests, then `cargo run -p jarvis-app`. Expected: PASS. Then by hand: enter an ElevenLabs key + voice id, click "Test voice", and hear it; delete a fact and watch it disappear.
- [ ] **Step 5: Owner look check.** Screenshot to `docs/screens/phase1-hud.png`; owner approves.
- [ ] **Step 6: Commit** `feat(app): HUD panels and settings`

---

### Task 14: Orb mode + hotkeys

**Files:**
- Create: `crates/jarvis-app/src/orb.rs`

**Interfaces:**
- Consumes: `orb_visual`, `AssistantView` (Task 12), `HudConfig.orb_position` (Task 1), `CoreLink`
- Produces:
  - `pub enum DisplayMode { Hud, Orb }` (a Bevy `State`)
  - `pub fn next_mode(current: DisplayMode, ev: ModeEvent) -> DisplayMode` with `pub enum ModeEvent { HudLostFocus, OrbClicked, Hotkey, WakeWithExpand, SettingsOpened }`. HudLostFocus → Orb, except while settings are open; OrbClicked/Hotkey toggle; WakeWithExpand → Hud.
- The orb is a second Bevy window: 180×180, transparent, no decorations, `WindowLevel::AlwaysOnTop`, positioned at `orb_position` or the bottom-right of the primary monitor with a 40 px margin. It's draggable (`start_drag_move` on mouse down), and the position is saved to the config on release. It renders the same reactor at small scale through its own camera, and is click-through (`hit_test = false`) while Idle.
- Switching: Orb mode hides the HUD window (`visible = false`) and shows the orb; Hud mode does the reverse and focuses the HUD.
- Global hotkeys (`global-hotkey` crate): `Option+Space` toggles HUD/orb; `Option+M` sends `UiCommand::ToggleMute`; `Option+J` sends `UiCommand::StartSession` (push-to-start, an alternative to saying the name).

- [ ] **Step 1: Write failing tests** `next_mode_cases` (each transition above, including "lost focus while settings open stays Hud")
- [ ] **Step 2: Run** `cargo test -p jarvis-app orb`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests, then check by hand. Click into Safari: the HUD vanishes and the orb appears bottom-right. Say "Jarvis, what time is it?": the HUD expands, it answers, and you click away again. Drag the orb, restart the app, and the orb keeps its position. `Option+M`: the orb turns grey and wake is ignored.
- [ ] **Step 5: Commit** `feat(app): orb mode and global hotkeys`

---

### Task 15: Packaging, signing, Phase 1 acceptance

**Files:**
- Create: `Packager.toml`, `packaging/Info.plist.ext`, `docs/phase1-acceptance.md`

**Interfaces:**
- Consumes: the whole app
- Produces: a signed, notarized `JARVIS.app` + `.dmg`; `docs/phase1-acceptance.md` with measured results

- `Info.plist` keys: `NSMicrophoneUsageDescription` = `"JARVIS listens for its name and your voice. Audio is processed on this Mac until you start a conversation."`, `NSAppleEventsUsageDescription` = `"JARVIS opens and controls apps you ask it to."`, `LSUIElement` = false, bundle id `com.alviongs.jarvis` (`ponytail:` change if the owner prefers another reverse-DNS id).
- Signing: `Developer ID Application` identity from the owner's Apple Developer account; notarize with `xcrun notarytool` using a keychain profile. If the account isn't ready yet, use ad-hoc signing (`codesign -s -`) and note that macOS will re-ask for mic permission after each rebuild.

- [ ] **Step 1: Build** `cargo packager --release`. Expected: `target/release/bundle/macos/JARVIS.app` and a `.dmg`.
- [ ] **Step 2: Sign and notarize.** Expected: `spctl -a -vv JARVIS.app` says `accepted, source=Notarized Developer ID` (or note that ad-hoc signing was used).
- [ ] **Step 3: Acceptance run.** Install from the dmg, launch, and grant the mic. Record each result in `docs/phase1-acceptance.md`:
  1. "Jarvis" → "Yes, sir?" within about 1 s.
  2. A 5-minute conversation over speakers with ≥ 5 interruptions: no self-triggering, every interruption stops speech.
  3. "Jarvis, open Visual Studio Code" opens it. "Open NoSuchApp" gets a polite spoken failure.
  4. "Remember that my sister's name is Priya", restart the app, then "What's my sister's name?" → Priya.
  5. Wifi off mid-conversation → fallback voice apology, HUD error, and it recovers when wifi returns.
  6. Latency: the median of 10 turns for each provider pair (ElevenLabs + Anthropic; VoiceStudio + Anthropic; MacSay + local whisper) against the 800 ms / 1.2 s targets. **If the targets are missed, record which stage is slow** (`stt_ms`, `first_token_ms`, `first_audio_ms`) and propose the fix: streaming STT, ElevenLabs WebSocket input streaming, a faster model, or a lower silence threshold.
  7. Switching to another app collapses the HUD to the orb; the hotkey brings it back.
  8. Changing the persona name to FRIDAY with wake phrase "friday" works without a rebuild.
  9. A Hindi sentence ("Jarvis, aaj mausam kaisa hai?") gets a reasonable response. Telugu/Hindi quality is tuned in Phase 7, so just record the result.
- [ ] **Step 4: Commit** `chore: packaging, signing, Phase 1 acceptance results`

---

## After Phase 1

Write `docs/superpowers/plans/<date>-jarvis-phase-2-hands.md`, informed by the acceptance results: latency fixes go first if the targets were missed, then the Phase 2 scope in the roadmap table.
