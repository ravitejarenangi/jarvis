//! User configuration (`config.toml`) and the persona's system prompt.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Config {
    pub persona: Persona,
    pub llm: LlmConfig,
    pub stt: SttConfig,
    pub tts: TtsConfig,
    pub conversation: ConversationConfig,
    pub hud: HudConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Persona {
    pub name: String,
    pub wake_phrase: String,
    pub wake_aliases: Vec<String>,
    pub address: String,
    pub personality: String,
    pub languages: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum SttChoice {
    LocalWhisper,
    OpenAiCompatible { base_url: String, model: String },
    Deepgram { model: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SttConfig {
    pub provider: SttChoice,
    pub whisper_model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum TtsChoice {
    ElevenLabs {
        voice_id: String,
        model: String,
    },
    OpenAiCompatible {
        base_url: String,
        model: String,
        voice: String,
    },
    MacSay {
        voice: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TtsConfig {
    pub provider: TtsChoice,
    pub fallback: TtsChoice,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ConversationConfig {
    pub end_of_turn_silence_ms: u32,
    pub session_idle_timeout_s: u32,
    pub end_phrases: Vec<String>,
    pub expand_hud_on_wake: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct HudConfig {
    pub favorite_apps: Vec<String>,
    pub orb_position: Option<(i32, i32)>,
    pub animation_intensity: f32,
    pub captions: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file {0}: {1}")]
    Io(PathBuf, io::Error),
    #[error("config file {0} is not valid TOML: {1}")]
    Parse(PathBuf, toml::de::Error),
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

impl Default for Persona {
    fn default() -> Self {
        Self {
            name: "JARVIS".into(),
            wake_phrase: "jarvis".into(),
            wake_aliases: strings(&["jervis", "jarvis.", "hey jarvis"]),
            address: "sir".into(),
            personality: "You are JARVIS, a calm, highly capable British AI assistant with dry wit. Address the user as {address}. Be concise: one or two sentences unless asked for detail. You are speaking aloud, so never use markdown, lists, code blocks or URLs in replies; describe them and say you've put them on screen.".into(),
            languages: strings(&["en", "hi", "te"]),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".into(),
            model: "claude-sonnet-5-5".into(),
            base_url: None,
        }
    }
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            provider: SttChoice::LocalWhisper,
            whisper_model: "ggml-base.bin".into(),
        }
    }
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider: TtsChoice::ElevenLabs {
                voice_id: String::new(),
                model: "eleven_flash_v2_5".into(),
            },
            fallback: TtsChoice::MacSay {
                voice: "Daniel".into(),
            },
        }
    }
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            end_of_turn_silence_ms: 500,
            session_idle_timeout_s: 60,
            end_phrases: strings(&[
                "that'll be all",
                "that will be all",
                "goodbye jarvis",
                "stand by",
            ]),
            expand_hud_on_wake: true,
        }
    }
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            favorite_apps: strings(&[
                "Safari",
                "Mail",
                "Calendar",
                "Music",
                "Visual Studio Code",
                "Terminal",
            ]),
            orb_position: None,
            animation_intensity: 1.0,
            captions: true,
        }
    }
}

/// The jarvis data dir: `$JARVIS_CONFIG_DIR` if set, else
/// `~/Library/Application Support/jarvis` on macOS.
/// Models live in `config_dir()/models`, the memory DB at `config_dir()/memory.db`.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("JARVIS_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    directories::ProjectDirs::from("", "", "jarvis")
        .map(|d| d.config_dir().to_path_buf())
        // No home dir: fall back to a relative dir rather than panicking.
        .unwrap_or_else(|| PathBuf::from("jarvis"))
}

impl Config {
    /// Loads `path`, filling missing fields with defaults. A missing file is all defaults.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(e) => return Err(ConfigError::Io(path.to_path_buf(), e)),
        };
        toml::from_str(&text).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))
    }

    /// Writes atomically (`.tmp` then rename), creating parent dirs.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let io_err = |e| ConfigError::Io(path.to_path_buf(), e);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| io_err(io::Error::new(io::ErrorKind::InvalidData, e)))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(io_err)?;
        std::fs::rename(&tmp, path).map_err(io_err)
    }

    pub fn default_path() -> PathBuf {
        config_dir().join("config.toml")
    }
}

impl Persona {
    pub fn system_prompt(&self, now: &str, active_app: Option<&str>, facts: &[String]) -> String {
        let mut s = self.personality.replace("{address}", &self.address);
        s.push_str(&format!("\n\nCurrent time: {now}"));
        if let Some(app) = active_app {
            s.push_str(&format!("\nActive app: {app}"));
        }
        if !facts.is_empty() {
            s.push_str("\n\nThings you know about the user:");
            for f in facts {
                s.push_str(&format!("\n- {f}"));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn missing_file_loads_defaults() {
        let dir = tmp();
        let cfg = Config::load(&dir.path().join("nope.toml")).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = tmp();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[persona]\nname = \"FRIDAY\"").unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.persona.name, "FRIDAY");
        assert_eq!(cfg.persona.wake_phrase, "jarvis");
        assert_eq!(cfg.llm.model, LlmConfig::default().model);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tmp();
        let path = dir.path().join("sub/config.toml");
        let mut cfg = Config::default();
        cfg.persona.address = "ma'am".into();
        cfg.llm.base_url = Some("http://localhost:1234".into());
        cfg.stt.provider = SttChoice::Deepgram {
            model: "nova-3".into(),
        };
        cfg.tts.provider = TtsChoice::OpenAiCompatible {
            base_url: "http://localhost:3900/v1".into(),
            model: "kokoro".into(),
            voice: "bm_george".into(),
        };
        cfg.hud.orb_position = Some((10, -20));
        cfg.conversation.end_of_turn_silence_ms = 700;
        cfg.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), cfg);
    }

    #[test]
    fn invalid_toml_error_names_the_file() {
        let dir = tmp();
        let path = dir.path().join("broken.toml");
        std::fs::write(&path, "[persona").unwrap();
        let err = Config::load(&path).unwrap_err();
        assert!(err.to_string().contains("broken.toml"), "{err}");
    }

    #[test]
    fn system_prompt_includes_address_time_app_and_facts() {
        let p = Persona::default();
        let facts = vec!["Likes tea".to_string(), "Lives in Hyderabad".to_string()];
        let s = p.system_prompt("2026-11-03 09:15", Some("Safari"), &facts);
        assert!(s.contains("sir"));
        assert!(!s.contains("{address}"));
        assert!(s.contains("2026-11-03 09:15"));
        assert!(s.contains("Safari"));
        for f in &facts {
            assert!(s.contains(f.as_str()));
        }
    }

    #[test]
    fn config_defaults_are_fully_local_capable() {
        let cfg = Config::default();
        assert_eq!(cfg.stt.provider, SttChoice::LocalWhisper);
        assert!(matches!(cfg.tts.fallback, TtsChoice::MacSay { .. }));
    }

    #[test]
    fn config_dir_env_override() {
        let dir = tmp();
        // Only this test touches JARVIS_CONFIG_DIR.
        unsafe { std::env::set_var("JARVIS_CONFIG_DIR", dir.path()) };
        let (cfg_dir, cfg_path) = (config_dir(), Config::default_path());
        unsafe { std::env::remove_var("JARVIS_CONFIG_DIR") };
        assert_eq!(cfg_dir, dir.path());
        assert_eq!(cfg_path, dir.path().join("config.toml"));
        assert!(config_dir().ends_with("jarvis"));
    }
}
