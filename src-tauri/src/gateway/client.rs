use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use super::remote_dj::RemoteDjCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub queue_id: i64,
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum GatewayMessage {
    // Desktop → Gateway (state push)
    NowPlaying {
        song_id: i64,
        title: String,
        artist: String,
        duration_ms: u32,
    },
    QueueUpdated {
        queue: Vec<QueueItem>,
    },
    DeckState {
        deck: String,
        state: String,
        position_ms: u32,
        duration_ms: u32,
    },
    VuMeter {
        channel: String,
        left_db: f32,
        right_db: f32,
    },
    ListenerCount {
        count: u32,
    },
    CrossfadeProgress {
        progress: f32,
        outgoing: String,
        incoming: String,
    },
    StreamStatus {
        connected: bool,
        mount: String,
        listeners: u32,
    },

    // Gateway → Desktop (remote commands)
    RemoteCommand {
        session_id: String,
        command: RemoteDjCommand,
    },
    RemoteDjJoined {
        session_id: String,
        user_id: String,
        display_name: String,
    },
    RemoteDjLeft {
        session_id: String,
    },
    RequestReceived {
        song_id: i64,
        requested_by: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub connected: bool,
    pub url: String,
    pub reconnecting: bool,
    pub last_error: Option<String>,
}

pub struct GatewayClient {
    url: String,
    token: String,
    connected: Arc<AtomicBool>,
    tx: Option<mpsc::UnboundedSender<GatewayMessage>>,
    status: Arc<tokio::sync::Mutex<GatewayStatus>>,
}

impl Clone for GatewayClient {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            token: self.token.clone(),
            connected: self.connected.clone(),
            tx: self.tx.clone(),
            status: self.status.clone(),
        }
    }
}

impl GatewayClient {
    pub fn new(url: String, token: String) -> Self {
        let status = GatewayStatus {
            connected: false,
            url: url.clone(),
            reconnecting: false,
            last_error: None,
        };

        Self {
            url,
            token,
            connected: Arc::new(AtomicBool::new(false)),
            tx: None,
            status: Arc::new(tokio::sync::Mutex::new(status)),
        }
    }

    /// Connect to the gateway WebSocket
    pub async fn connect(
        &mut self,
        on_message: impl Fn(GatewayMessage) + Send + 'static,
    ) -> Result<(), String> {
        let ws_url = format!("{}/desktop-bridge?token={}", self.url, self.token);

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| format!("WebSocket connection failed: {}", e))?;

        let (mut write, mut read) = ws_stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<GatewayMessage>();

        self.connected.store(true, Ordering::SeqCst);
        self.tx = Some(tx);

        // Update status
        {
            let mut status = self.status.lock().await;
            status.connected = true;
            status.last_error = None;
        }

        let connected = self.connected.clone();
        let status = self.status.clone();

        // Spawn task to send messages to gateway
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let json = serde_json::to_string(&msg).unwrap();
                if write.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        });

        let connected_clone = connected.clone();
        let status_clone = status.clone();

        // Spawn task to receive messages from gateway
        tokio::spawn(async move {
            while let Some(Ok(msg)) = read.next().await {
                match msg {
                    Message::Text(text) => {
                        if let Ok(gateway_msg) = serde_json::from_str::<GatewayMessage>(&text) {
                            on_message(gateway_msg);
                        }
                    }
                    Message::Close(_) => {
                        connected_clone.store(false, Ordering::SeqCst);
                        let mut s = status_clone.lock().await;
                        s.connected = false;
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// Send a message to the gateway
    pub async fn send(&self, message: GatewayMessage) -> Result<(), String> {
        if let Some(tx) = &self.tx {
            tx.send(message)
                .map_err(|e| format!("Failed to send message: {}", e))?;
            Ok(())
        } else {
            Err("Not connected".to_string())
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Get status
    pub async fn get_status(&self) -> GatewayStatus {
        self.status.lock().await.clone()
    }

    /// Disconnect from gateway
    pub async fn disconnect(&mut self) {
        self.tx = None;
        self.connected.store(false, Ordering::SeqCst);
        let mut status = self.status.lock().await;
        status.connected = false;
    }
}


