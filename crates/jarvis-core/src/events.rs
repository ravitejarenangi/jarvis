//! Messages between the core and the UI.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Muted,
}

/// All values are measured from the end of the user's speech.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LatencyReport {
    pub stt_ms: u32,
    pub first_token_ms: u32,
    pub first_audio_ms: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreEvent {
    State(AssistantState),
    UserTranscript {
        text: String,
        is_final: bool,
    },
    AssistantText {
        delta: String,
    },
    AssistantTurnDone,
    AudioLevel {
        input: f32,
        output: f32,
    },
    ToolActivity {
        name: String,
        summary: String,
    },
    Error {
        message: String,
        open_settings: bool,
    },
    Latency(LatencyReport),
    Facts(Vec<(i64, String)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiCommand {
    StartSession,
    EndSession,
    ToggleMute,
    ReloadConfig,
    SetApiKey { provider: String, key: String },
    LaunchApp(String),
    ListFacts,
    DeleteFact(i64),
    Shutdown,
}
