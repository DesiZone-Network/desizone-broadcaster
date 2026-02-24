/// `broadcaster.rs` — Fan-out PCM ring buffer dispatcher
///
/// The master audio engine writes f32 PCM samples into a single producer ring
/// buffer.  The Broadcaster reads that ring buffer and copies each frame into
/// every active EncoderSlot consumer ring buffer.
///
/// One Tokio task runs the broadcast loop; each encoder is given its own
/// HeapProd/HeapCons pair.  EncoderSlots are added/removed at runtime without
/// stopping the loop (guarded by a Mutex over the slot Vec).
use std::sync::{Arc, Mutex};

use ringbuf::{traits::{Consumer as _, Producer as _, Split}, HeapRb};
use serde::{Deserialize, Serialize};

/// Unique ID for a broadcaster slot (= encoder id in DB)
pub type SlotId = i64;

/// One per active encoder — the Broadcaster writes into `prod`,
/// the encoder task reads from `cons`.
struct BroadcastSlot {
    id: SlotId,
    prod: ringbuf::HeapProd<f32>,
}

/// Shared, cloneable handle used by the rest of the app.
#[derive(Clone)]
pub struct Broadcaster {
    slots: Arc<Mutex<Vec<BroadcastSlot>>>,
}

impl Broadcaster {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a new encoder slot; returns the `HeapCons` end.
    /// Buffer size: 5 s worth of stereo f32 @ 44100 Hz
    pub fn add_slot(&self, id: SlotId) -> ringbuf::HeapCons<f32> {
        const BUF: usize = 44100 * 2 * 5;
        let rb = HeapRb::<f32>::new(BUF);
        let (prod, cons) = rb.split();
        self.slots.lock().unwrap().push(BroadcastSlot { id, prod });
        cons
    }

    /// Remove a slot (encoder stopped / deleted).
    pub fn remove_slot(&self, id: SlotId) {
        self.slots.lock().unwrap().retain(|s| s.id != id);
    }

    /// Distribute samples from the master ring buffer into all slots.
    /// Called in a tight loop on the broadcaster task.
    pub fn distribute(&self, master: &mut ringbuf::HeapCons<f32>) {
        // Collect all available samples in one pass
        let mut samples: Vec<f32> = Vec::with_capacity(8192);
        while let Some(s) = master.try_pop() {
            samples.push(s);
        }
        if samples.is_empty() {
            return;
        }
        let mut guard = self.slots.lock().unwrap();
        for slot in guard.iter_mut() {
            for &s in &samples {
                let _ = slot.prod.try_push(s);
            }
        }
    }

    /// Number of active slots.
    pub fn slot_count(&self) -> usize {
        self.slots.lock().unwrap().len()
    }
}

// ── Encoder status (used by both Rust and the Tauri layer) ───────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncoderStatus {
    Disabled,
    Connecting,
    Streaming,
    Retrying { attempt: u32, max: u32 },
    Failed,
    Recording, // file output only
}

impl Default for EncoderStatus {
    fn default() -> Self {
        Self::Disabled
    }
}

/// Runtime state for one encoder — stored in EncoderManager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderRuntimeState {
    pub id: SlotId,
    pub status: EncoderStatus,
    pub listeners: Option<u32>,
    pub uptime_secs: u64,
    pub bytes_sent: u64,
    pub current_bitrate_kbps: Option<u32>,
    pub error: Option<String>,
    pub recording_file: Option<String>,
}
