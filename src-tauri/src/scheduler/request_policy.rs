/// Request Policy Engine
///
/// Evaluates song requests against a configurable policy to auto-accept or
/// reject them with a typed reason.
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use chrono::Timelike;

// ── Policy ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestPolicy {
    // Song limits
    pub max_requests_per_song_per_day: u32,
    pub min_minutes_between_same_song: u32,

    // Artist limits
    pub max_requests_per_artist_per_hour: u32,
    pub min_minutes_between_same_artist: u32,

    // Album limits
    pub max_requests_per_album_per_day: u32,

    // Requester limits
    pub max_requests_per_requester_per_day: u32,
    pub max_requests_per_requester_per_hour: u32,

    // Queue position for accepted request
    pub queue_position: RequestQueuePosition,

    // Blacklists
    pub blacklisted_song_ids: Vec<i64>,
    pub blacklisted_categories: Vec<String>,

    // Hours when requests are accepted (start_hour, end_hour in 24h)
    pub active_hours: Option<(u8, u8)>,

    // Auto-accept if all checks pass
    pub auto_accept: bool,
}

impl Default for RequestPolicy {
    fn default() -> Self {
        Self {
            max_requests_per_song_per_day: 3,
            min_minutes_between_same_song: 60,
            max_requests_per_artist_per_hour: 2,
            min_minutes_between_same_artist: 30,
            max_requests_per_album_per_day: 5,
            max_requests_per_requester_per_day: 5,
            max_requests_per_requester_per_hour: 2,
            queue_position: RequestQueuePosition::End,
            blacklisted_song_ids: Vec::new(),
            blacklisted_categories: Vec::new(),
            active_hours: None,
            auto_accept: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestQueuePosition {
    Next,
    After(u32),
    End,
}

// ── Request log entry ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLogEntry {
    pub id: Option<i64>,
    pub song_id: i64,
    pub song_title: Option<String>,
    pub artist: Option<String>,
    pub requester_name: Option<String>,
    pub requester_platform: Option<String>,
    pub requester_ip: Option<String>,
    pub requested_at: i64, // Unix timestamp
    pub status: RequestStatus,
    pub rejection_reason: Option<String>,
    pub played_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Pending,
    Accepted,
    Rejected,
    Played,
}

impl RequestStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Played => "played",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "accepted" => Self::Accepted,
            "rejected" => Self::Rejected,
            "played" => Self::Played,
            _ => Self::Pending,
        }
    }
}

// ── Policy validation ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub rule: String,
    pub message: String,
}

/// Evaluate a new request against the current policy.
/// Returns Ok(()) if the request is allowed, Err(violation) if not.
pub async fn evaluate_request(
    pool: &SqlitePool,
    policy: &RequestPolicy,
    song_id: i64,
    song_artist: &str,
    song_category: &str,
    requester_name: &str,
    _requester_ip: Option<&str>,
) -> Result<(), PolicyViolation> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Check active hours
    if let Some((start_h, end_h)) = policy.active_hours {
        let hour = chrono::Local::now().hour() as u8;
        if hour < start_h || hour >= end_h {
            return Err(PolicyViolation {
                rule: "active_hours".to_string(),
                message: format!("Requests only accepted between {:02}:00 and {:02}:00", start_h, end_h),
            });
        }
    }

    // Blacklisted song
    if policy.blacklisted_song_ids.contains(&song_id) {
        return Err(PolicyViolation {
            rule: "blacklist_song".to_string(),
            message: "This song is not requestable.".to_string(),
        });
    }

    // Blacklisted category
    for cat in &policy.blacklisted_categories {
        if song_category.to_lowercase().contains(&cat.to_lowercase()) {
            return Err(PolicyViolation {
                rule: "blacklist_category".to_string(),
                message: format!("Category '{}' is not requestable.", cat),
            });
        }
    }

    let day_start = now - 86400;
    let hour_start = now - 3600;
    let song_min_gap = now - (policy.min_minutes_between_same_song as i64 * 60);
    let artist_min_gap = now - (policy.min_minutes_between_same_artist as i64 * 60);

    // Same song per day
    let song_day_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM request_log WHERE song_id = ? AND requested_at > ? AND status != 'rejected'"
    )
    .bind(song_id)
    .bind(day_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if song_day_count >= policy.max_requests_per_song_per_day as i64 {
        return Err(PolicyViolation {
            rule: "song_day_limit".to_string(),
            message: format!("This song has already been requested {} times today.", song_day_count),
        });
    }

    // Same song min gap
    let last_song_request: Option<i64> = sqlx::query_scalar(
        "SELECT requested_at FROM request_log WHERE song_id = ? AND status != 'rejected' ORDER BY requested_at DESC LIMIT 1"
    )
    .bind(song_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some(t) = last_song_request {
        if t > song_min_gap {
            let wait = (t + policy.min_minutes_between_same_song as i64 * 60 - now) / 60;
            return Err(PolicyViolation {
                rule: "song_min_gap".to_string(),
                message: format!("Please wait {} more minutes before requesting this song again.", wait),
            });
        }
    }

    // Artist per hour
    let artist_hour_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM request_log WHERE artist = ? AND requested_at > ? AND status != 'rejected'"
    )
    .bind(song_artist)
    .bind(hour_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if artist_hour_count >= policy.max_requests_per_artist_per_hour as i64 {
        return Err(PolicyViolation {
            rule: "artist_hour_limit".to_string(),
            message: format!("Too many requests for this artist this hour (max {}).", policy.max_requests_per_artist_per_hour),
        });
    }

    // Artist min gap
    let last_artist_request: Option<i64> = sqlx::query_scalar(
        "SELECT requested_at FROM request_log WHERE artist = ? AND status != 'rejected' ORDER BY requested_at DESC LIMIT 1"
    )
    .bind(song_artist)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some(t) = last_artist_request {
        if t > artist_min_gap {
            let wait = (t + policy.min_minutes_between_same_artist as i64 * 60 - now) / 60;
            return Err(PolicyViolation {
                rule: "artist_min_gap".to_string(),
                message: format!("Please wait {} more minutes before requesting this artist again.", wait),
            });
        }
    }

    // Requester per hour
    let req_hour_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM request_log WHERE requester_name = ? AND requested_at > ? AND status != 'rejected'"
    )
    .bind(requester_name)
    .bind(hour_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if req_hour_count >= policy.max_requests_per_requester_per_hour as i64 {
        return Err(PolicyViolation {
            rule: "requester_hour_limit".to_string(),
            message: format!("You can only request {} songs per hour.", policy.max_requests_per_requester_per_hour),
        });
    }

    // Requester per day
    let req_day_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM request_log WHERE requester_name = ? AND requested_at > ? AND status != 'rejected'"
    )
    .bind(requester_name)
    .bind(day_start)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if req_day_count >= policy.max_requests_per_requester_per_day as i64 {
        return Err(PolicyViolation {
            rule: "requester_day_limit".to_string(),
            message: format!("You can only request {} songs per day.", policy.max_requests_per_requester_per_day),
        });
    }

    Ok(())
}

// ── DB helpers ────────────────────────────────────────────────────────────────

pub async fn load_policy(pool: &SqlitePool) -> Result<RequestPolicy, sqlx::Error> {
    let row = sqlx::query("SELECT policy_json FROM request_policy WHERE id = 1")
        .fetch_optional(pool)
        .await?;

    if let Some(r) = row {
        let json: String = r.get("policy_json");
        Ok(serde_json::from_str(&json).unwrap_or_default())
    } else {
        Ok(RequestPolicy::default())
    }
}

pub async fn save_policy(pool: &SqlitePool, policy: &RequestPolicy) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string(policy).unwrap_or_default();
    sqlx::query(
        "INSERT INTO request_policy (id, policy_json) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET policy_json = excluded.policy_json"
    )
    .bind(&json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_requests(
    pool: &SqlitePool,
    status: &str,
) -> Result<Vec<RequestLogEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, song_id, song_title, artist, requester_name, requester_platform, requester_ip, \
         requested_at, status, rejection_reason, played_at \
         FROM request_log WHERE status = ? ORDER BY requested_at DESC LIMIT 200"
    )
    .bind(status)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| RequestLogEntry {
            id: r.get("id"),
            song_id: r.get("song_id"),
            song_title: r.get("song_title"),
            artist: r.get("artist"),
            requester_name: r.get("requester_name"),
            requester_platform: r.get("requester_platform"),
            requester_ip: r.get("requester_ip"),
            requested_at: r.get("requested_at"),
            status: RequestStatus::from_str(r.get::<&str, _>("status")),
            rejection_reason: r.get("rejection_reason"),
            played_at: r.get("played_at"),
        })
        .collect())
}

pub async fn insert_request(pool: &SqlitePool, entry: &RequestLogEntry) -> Result<i64, sqlx::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let r = sqlx::query(
        "INSERT INTO request_log (song_id, song_title, artist, requester_name, requester_platform, requester_ip, requested_at, status, rejection_reason) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(entry.song_id)
    .bind(&entry.song_title)
    .bind(&entry.artist)
    .bind(&entry.requester_name)
    .bind(&entry.requester_platform)
    .bind(&entry.requester_ip)
    .bind(now)
    .bind(entry.status.as_str())
    .bind(&entry.rejection_reason)
    .execute(pool)
    .await?;
    Ok(r.last_insert_rowid())
}

pub async fn update_request_status(
    pool: &SqlitePool,
    id: i64,
    status: RequestStatus,
    reason: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE request_log SET status = ?, rejection_reason = ? WHERE id = ?"
    )
    .bind(status.as_str())
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_request_history(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<Vec<RequestLogEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, song_id, song_title, artist, requester_name, requester_platform, requester_ip, \
         requested_at, status, rejection_reason, played_at \
         FROM request_log ORDER BY requested_at DESC LIMIT ? OFFSET ?"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| RequestLogEntry {
            id: r.get("id"),
            song_id: r.get("song_id"),
            song_title: r.get("song_title"),
            artist: r.get("artist"),
            requester_name: r.get("requester_name"),
            requester_platform: r.get("requester_platform"),
            requester_ip: r.get("requester_ip"),
            requested_at: r.get("requested_at"),
            status: RequestStatus::from_str(r.get::<&str, _>("status")),
            rejection_reason: r.get("rejection_reason"),
            played_at: r.get("played_at"),
        })
        .collect())
}
