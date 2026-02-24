use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RemoteDjCommand {
    LoadTrack {
        deck: String,
        song_id: i64,
    },
    PlayDeck {
        deck: String,
    },
    PauseDeck {
        deck: String,
    },
    SetVolume {
        channel: String,
        volume: f32,
    },
    AddToQueue {
        song_id: i64,
        position: Option<usize>,
    },
    RemoveFromQueue {
        queue_id: i64,
    },
    TriggerCrossfade,
    SetAutoPilot {
        enabled: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSession {
    pub session_id: String,
    pub user_id: String,
    pub display_name: Option<String>,
    pub connected_at: i64, // Unix timestamp ms
    pub commands_sent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DjPermissions {
    pub can_load_track: bool,
    pub can_play_pause: bool,
    pub can_seek: bool,
    pub can_set_volume: bool,
    pub can_queue_add: bool,
    pub can_queue_remove: bool,
    pub can_trigger_crossfade: bool,
    pub can_set_autopilot: bool,
}

impl Default for DjPermissions {
    fn default() -> Self {
        Self {
            can_load_track: false,
            can_play_pause: true,
            can_seek: false,
            can_set_volume: true,
            can_queue_add: true,
            can_queue_remove: false,
            can_trigger_crossfade: false,
            can_set_autopilot: false,
        }
    }
}

impl DjPermissions {
    /// Check if a command is allowed with these permissions
    pub fn allows_command(&self, command: &RemoteDjCommand) -> bool {
        match command {
            RemoteDjCommand::LoadTrack { .. } => self.can_load_track,
            RemoteDjCommand::PlayDeck { .. } | RemoteDjCommand::PauseDeck { .. } => {
                self.can_play_pause
            }
            RemoteDjCommand::SetVolume { .. } => self.can_set_volume,
            RemoteDjCommand::AddToQueue { .. } => self.can_queue_add,
            RemoteDjCommand::RemoveFromQueue { .. } => self.can_queue_remove,
            RemoteDjCommand::TriggerCrossfade => self.can_trigger_crossfade,
            RemoteDjCommand::SetAutoPilot { .. } => self.can_set_autopilot,
        }
    }
}

