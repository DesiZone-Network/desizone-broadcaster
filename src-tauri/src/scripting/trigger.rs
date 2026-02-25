/// `scripting/trigger.rs` — Script event types that map to Lua callbacks
///
/// The audio engine and other subsystems fire `ScriptEvent`s.
/// The `ScriptEngine` listens on an async channel and dispatches them
/// to all enabled scripts whose `trigger_type` matches.
use serde::{Deserialize, Serialize};

/// Events that can trigger script execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScriptEvent {
    /// Fired when a new track starts playing on any deck.
    TrackStart {
        id: i64,
        title: String,
        artist: String,
        album: Option<String>,
        duration_ms: u64,
        category: Option<String>,
    },
    /// Fired when a track ends naturally (not by skip).
    TrackEnd { id: i64, title: String },
    /// Fired when a crossfade begins.
    CrossfadeStart {
        outgoing_id: i64,
        outgoing_title: String,
        incoming_id: i64,
        incoming_title: String,
    },
    /// Fired when the queue becomes empty.
    QueueEmpty,
    /// Fired when a listener song request arrives.
    RequestReceived {
        song_id: i64,
        song_title: String,
        requester: String,
    },
    /// Fired at the start of each calendar hour (0-23).
    Hour { hour: u8 },
    /// Fired when an encoder connects successfully.
    EncoderConnect { encoder_id: i64 },
    /// Fired when an encoder disconnects.
    EncoderDisconnect { encoder_id: i64, reason: String },
    /// Manual trigger (user pressed "Run" in UI).
    Manual,
}

impl ScriptEvent {
    /// Returns the trigger_type string stored in the DB for this event.
    pub fn trigger_type(&self) -> &'static str {
        match self {
            ScriptEvent::TrackStart { .. } => "on_track_start",
            ScriptEvent::TrackEnd { .. } => "on_track_end",
            ScriptEvent::CrossfadeStart { .. } => "on_crossfade_start",
            ScriptEvent::QueueEmpty => "on_queue_empty",
            ScriptEvent::RequestReceived { .. } => "on_request_received",
            ScriptEvent::Hour { .. } => "on_hour",
            ScriptEvent::EncoderConnect { .. } => "on_encoder_connect",
            ScriptEvent::EncoderDisconnect { .. } => "on_encoder_disconnect",
            ScriptEvent::Manual => "manual",
        }
    }
}
