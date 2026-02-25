use chrono::{Datelike, TimeZone};
/// Show Scheduler
///
/// Runs as a Tokio background task. Reads the schedule from the local DB
/// every second, fires show actions at the correct times, emits Tauri events.
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DayOfWeek {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// Actions a show can trigger when it fires
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShowAction {
    PlayPlaylist { playlist_id: i64 },
    PlaySong { song_id: i64 },
    StartStream { encoder_id: String },
    StopStream { encoder_id: String },
    SetVolume { channel: String, volume: f32 },
    SwitchMode { mode: String },
    PlayJingle { song_id: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Show {
    pub id: Option<i64>,
    pub name: String,
    /// Days this show recurs on (empty = one-time)
    pub days: Vec<DayOfWeek>,
    /// HH:MM in 24h local time
    pub start_time: String,
    /// Duration in minutes (0 = run until next show)
    pub duration_minutes: u32,
    pub actions: Vec<ShowAction>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledEvent {
    pub show_id: i64,
    pub show_name: String,
    /// ISO-8601 datetime string of when this fires next
    pub fires_at: String,
    pub actions: Vec<ShowAction>,
}

// ── DB helpers ────────────────────────────────────────────────────────────────

pub async fn get_shows(pool: &SqlitePool) -> Result<Vec<Show>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, days_json, start_time, duration_minutes, actions_json, enabled FROM scheduled_shows ORDER BY start_time"
    )
    .fetch_all(pool)
    .await?;

    let mut shows = Vec::new();
    for r in rows {
        let days: Vec<DayOfWeek> =
            serde_json::from_str(r.get::<&str, _>("days_json")).unwrap_or_default();
        let actions: Vec<ShowAction> =
            serde_json::from_str(r.get::<&str, _>("actions_json")).unwrap_or_default();
        shows.push(Show {
            id: r.get("id"),
            name: r.get("name"),
            days,
            start_time: r.get("start_time"),
            duration_minutes: r.get::<i64, _>("duration_minutes") as u32,
            actions,
            enabled: r.get::<i64, _>("enabled") != 0,
        });
    }
    Ok(shows)
}

pub async fn upsert_show(pool: &SqlitePool, show: &Show) -> Result<i64, sqlx::Error> {
    let days_json = serde_json::to_string(&show.days).unwrap_or_default();
    let actions_json = serde_json::to_string(&show.actions).unwrap_or_default();

    let result = if let Some(id) = show.id {
        sqlx::query(
            "UPDATE scheduled_shows SET name=?, days_json=?, start_time=?, duration_minutes=?, actions_json=?, enabled=? WHERE id=?"
        )
        .bind(&show.name)
        .bind(&days_json)
        .bind(&show.start_time)
        .bind(show.duration_minutes as i64)
        .bind(&actions_json)
        .bind(show.enabled as i64)
        .bind(id)
        .execute(pool)
        .await?;
        id
    } else {
        let r = sqlx::query(
            "INSERT INTO scheduled_shows (name, days_json, start_time, duration_minutes, actions_json, enabled) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&show.name)
        .bind(&days_json)
        .bind(&show.start_time)
        .bind(show.duration_minutes as i64)
        .bind(&actions_json)
        .bind(show.enabled as i64)
        .execute(pool)
        .await?;
        r.last_insert_rowid()
    };
    Ok(result)
}

pub async fn delete_show(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM scheduled_shows WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Return upcoming scheduled events within the next `hours` hours
pub async fn get_upcoming_events(
    pool: &SqlitePool,
    hours: u32,
) -> Result<Vec<ScheduledEvent>, sqlx::Error> {
    let shows = get_shows(pool).await?;
    let now = chrono::Local::now();
    let window = chrono::Duration::hours(hours as i64);
    let mut events = Vec::new();

    for show in shows.into_iter().filter(|s| s.enabled) {
        // Parse show time
        let parts: Vec<u32> = show
            .start_time
            .split(':')
            .filter_map(|p| p.parse().ok())
            .collect();
        if parts.len() < 2 {
            continue;
        }
        let (h, m) = (parts[0], parts[1]);

        if show.days.is_empty() {
            // One-time: try today
            let candidate = now.date_naive().and_hms_opt(h, m, 0);
            if let Some(dt) = candidate {
                let fire_at = chrono::Local
                    .from_local_datetime(&dt)
                    .single()
                    .unwrap_or_else(|| chrono::Local::now());
                if fire_at > now && fire_at < now + window {
                    events.push(ScheduledEvent {
                        show_id: show.id.unwrap_or(0),
                        show_name: show.name.clone(),
                        fires_at: fire_at.to_rfc3339(),
                        actions: show.actions.clone(),
                    });
                }
            }
        } else {
            // Recurring: check each day in range
            for day_offset in 0..=(hours / 24 + 1) {
                let candidate_date = now.date_naive() + chrono::Duration::days(day_offset as i64);
                let weekday = candidate_date.weekday();
                let matches = show.days.iter().any(|d| match d {
                    DayOfWeek::Monday => weekday == chrono::Weekday::Mon,
                    DayOfWeek::Tuesday => weekday == chrono::Weekday::Tue,
                    DayOfWeek::Wednesday => weekday == chrono::Weekday::Wed,
                    DayOfWeek::Thursday => weekday == chrono::Weekday::Thu,
                    DayOfWeek::Friday => weekday == chrono::Weekday::Fri,
                    DayOfWeek::Saturday => weekday == chrono::Weekday::Sat,
                    DayOfWeek::Sunday => weekday == chrono::Weekday::Sun,
                });
                if !matches {
                    continue;
                }
                let candidate = candidate_date.and_hms_opt(h, m, 0);
                if let Some(dt) = candidate {
                    let fire_at = chrono::Local
                        .from_local_datetime(&dt)
                        .single()
                        .unwrap_or_else(|| chrono::Local::now());
                    if fire_at > now && fire_at < now + window {
                        events.push(ScheduledEvent {
                            show_id: show.id.unwrap_or(0),
                            show_name: show.name.clone(),
                            fires_at: fire_at.to_rfc3339(),
                            actions: show.actions.clone(),
                        });
                    }
                }
            }
        }
    }

    events.sort_by(|a, b| a.fires_at.cmp(&b.fires_at));
    Ok(events)
}
