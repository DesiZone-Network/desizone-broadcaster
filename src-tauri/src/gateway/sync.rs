use std::time::Duration;
use tokio::time::interval;

use super::client::{GatewayClient, GatewayMessage, QueueItem};

/// State sync configuration
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub sync_queue: bool,
    pub sync_vu: bool,
    pub vu_throttle_ms: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_queue: true,
            sync_vu: true,
            vu_throttle_ms: 200,
        }
    }
}

/// State syncer — pushes state from desktop to gateway
pub struct StateSyncer {
    client: GatewayClient,
    config: SyncConfig,
}

impl StateSyncer {
    pub fn new(client: GatewayClient, config: SyncConfig) -> Self {
        Self { client, config }
    }

    /// Push "now playing" update to gateway
    pub async fn push_now_playing(
        &self,
        song_id: i64,
        title: String,
        artist: String,
        duration_ms: u32,
    ) -> Result<(), String> {
        let msg = GatewayMessage::NowPlaying {
            song_id,
            title,
            artist,
            duration_ms,
        };
        self.client.send(msg).await
    }

    /// Push deck state update to gateway
    pub async fn push_deck_state(
        &self,
        deck: String,
        state: String,
        position_ms: u32,
        duration_ms: u32,
    ) -> Result<(), String> {
        let msg = GatewayMessage::DeckState {
            deck,
            state,
            position_ms,
            duration_ms,
        };
        self.client.send(msg).await
    }

    /// Push queue update to gateway
    pub async fn push_queue(&self, queue: Vec<QueueItem>) -> Result<(), String> {
        if !self.config.sync_queue {
            return Ok(());
        }
        let msg = GatewayMessage::QueueUpdated { queue };
        self.client.send(msg).await
    }

    /// Push VU meter reading to gateway
    pub async fn push_vu_meter(
        &self,
        channel: String,
        left_db: f32,
        right_db: f32,
    ) -> Result<(), String> {
        if !self.config.sync_vu {
            return Ok(());
        }
        let msg = GatewayMessage::VuMeter {
            channel,
            left_db,
            right_db,
        };
        self.client.send(msg).await
    }

    /// Push crossfade progress to gateway
    pub async fn push_crossfade_progress(
        &self,
        progress: f32,
        outgoing: String,
        incoming: String,
    ) -> Result<(), String> {
        let msg = GatewayMessage::CrossfadeProgress {
            progress,
            outgoing,
            incoming,
        };
        self.client.send(msg).await
    }

    /// Push stream status to gateway
    pub async fn push_stream_status(
        &self,
        connected: bool,
        mount: String,
        listeners: u32,
    ) -> Result<(), String> {
        let msg = GatewayMessage::StreamStatus {
            connected,
            mount,
            listeners,
        };
        self.client.send(msg).await
    }

    /// Start VU meter sync loop (throttled)
    pub async fn start_vu_sync_loop<F>(self, mut get_vu: F)
    where
        F: FnMut() -> Vec<(String, f32, f32)> + Send + 'static,
    {
        let mut ticker = interval(Duration::from_millis(self.config.vu_throttle_ms));

        loop {
            ticker.tick().await;

            if !self.client.is_connected() {
                break;
            }

            let readings = get_vu();
            for (channel, left_db, right_db) in readings {
                let _ = self.push_vu_meter(channel, left_db, right_db).await;
            }
        }
    }
}

