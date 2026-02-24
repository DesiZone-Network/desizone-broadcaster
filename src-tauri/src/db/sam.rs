use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPool, Row};

/// Connect to a SAM Broadcaster MySQL database.
/// URL format: "mysql://user:password@host:port/database"
pub async fn connect(url: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPool::connect(url).await
}

/// Translate a SAM Windows-style file path to a local path.
/// If `from` is empty, the filename is returned unchanged.
/// Example: translate_path("C:\\Music\\track.mp3", "C:\\Music\\", "/Volumes/Music/")
///          → "/Volumes/Music/track.mp3"
pub fn translate_path(filename: &str, from: &str, to: &str) -> String {
    if from.is_empty() || filename.is_empty() {
        return filename.to_string();
    }
    // Normalise Windows backslashes for comparison
    let norm_file = filename.replace('\\', "/");
    let norm_from = from.replace('\\', "/");
    if norm_file.to_lowercase().starts_with(&norm_from.to_lowercase()) {
        let rest = &norm_file[norm_from.len()..];
        format!("{}{}", to.trim_end_matches('/'), rest)
    } else {
        filename.to_string()
    }
}

// ── songlist ─────────────────────────────────────────────────────────────────

/// A row from SAM's `songlist` table.
/// Column names match the real samdb schema exactly (primary key is `ID`).
/// Note: SAM does not store `intro`, `outro`, or `gain` in this schema version —
/// those are handled by DesiZone's local SQLite `cue_points` / `song_fade_overrides`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamSong {
    pub id: i64,                     // `ID` — primary key
    pub filename: String,
    pub songtype: String,            // 'S'=Song, 'J'=Jingle, etc.
    pub status: i32,                 // 0=disabled, 1=enabled
    pub weight: f64,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub genre: String,
    pub albumyear: String,
    pub duration: i32,               // seconds
    pub bpm: i32,
    pub xfade: String,               // crossfade preset name
    pub mood: String,
    pub mood_ai: Option<String>,
    pub rating: i32,
    pub count_played: i32,
    pub date_played: Option<String>, // MySQL datetime as String
    pub label: String,
    pub isrc: String,                // `ISRC` column
    pub upc: String,                 // `UPC` column (also used for Spotify ID)
    pub picture: Option<String>,
    pub overlay: String,             // 'yes' | 'no'
}

fn row_to_sam_song(r: &sqlx::mysql::MySqlRow) -> SamSong {
    SamSong {
        id: r.get::<i64, _>("ID"),
        filename: r.get("filename"),
        songtype: r.get("songtype"),
        status: r.get::<i8, _>("status") as i32,
        weight: r.get("weight"),
        artist: r.get("artist"),
        title: r.get("title"),
        album: r.get("album"),
        genre: r.get("genre"),
        albumyear: r.get("albumyear"),
        duration: r.get("duration"),
        bpm: r.get::<i32, _>("bpm"),
        xfade: r.get("xfade"),
        mood: r.get("mood"),
        mood_ai: r.get("mood_ai"),
        rating: r.get::<i32, _>("rating"),
        count_played: r.get::<i32, _>("count_played"),
        date_played: r.get("date_played"),
        label: r.get("label"),
        isrc: r.get("ISRC"),
        upc: r.get("UPC"),
        picture: r.get("picture"),
        overlay: r.get("overlay"),
    }
}

const SONG_COLUMNS: &str = r#"
    ID, filename, songtype, status, weight,
    artist, title, album, genre, albumyear,
    duration, bpm, xfade, mood, mood_ai,
    rating, count_played, date_played, label,
    ISRC, UPC, picture, overlay
"#;

/// Fetch a single song by its SAM `ID`.
pub async fn get_song(pool: &MySqlPool, song_id: i64) -> Result<Option<SamSong>, sqlx::Error> {
    let sql = format!("SELECT {SONG_COLUMNS} FROM songlist WHERE ID = ?");
    let row = sqlx::query(&sql)
        .bind(song_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_sam_song))
}

/// Full-text search against artist and title.
/// Only returns enabled songs (`status = 1`).
pub async fn search_songs(
    pool: &MySqlPool,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<SamSong>, sqlx::Error> {
    let pattern = format!("%{query}%");
    let sql = format!(
        r#"SELECT {SONG_COLUMNS}
           FROM songlist
           WHERE status = 1
             AND (artist LIKE ? OR title LIKE ? OR album LIKE ?)
           ORDER BY artist, title
           LIMIT ? OFFSET ?"#
    );
    let rows = sqlx::query(&sql)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(row_to_sam_song).collect())
}

// ── queuelist ────────────────────────────────────────────────────────────────

/// A row from SAM's `queuelist` table.
/// SAM queues use a floating-point `sortID` for ordering.
/// There is no `played` column — completed entries are simply deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: i64,            // `ID` — primary key
    pub song_id: i64,       // `songID`
    pub sort_id: f64,       // `sortID` — ordering key (float)
    pub requests: i32,      // number of listener requests for this slot
    pub request_id: i32,    // `requestID` — linked request (0 if none)
    pub plotw: i32,         // `PLOTW`: 0=Song/PLO, 1=VoiceBreak/TW
    pub dedication: i32,    // `dedication` flag
}

/// Fetch all pending queue entries ordered by sortID.
pub async fn get_queue(pool: &MySqlPool) -> Result<Vec<QueueEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT ID, songID, sortID, requests, requestID, PLOTW, dedication \
         FROM queuelist ORDER BY sortID ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| QueueEntry {
            id: r.get("ID"),
            song_id: r.get("songID"),
            sort_id: r.get("sortID"),
            requests: r.get::<i32, _>("requests"),
            request_id: r.get::<i32, _>("requestID"),
            plotw: r.get::<i8, _>("PLOTW") as i32,
            dedication: r.get::<i8, _>("dedication") as i32,
        })
        .collect())
}

/// Append a song to the end of the queue.
/// The `sortID` is assigned as MAX(sortID)+1 so the new entry goes last.
pub async fn add_to_queue(pool: &MySqlPool, song_id: i64) -> Result<i64, sqlx::Error> {
    // Compute next sortID in a single round-trip
    let next_sort: f64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sortID), 0) + 1 FROM queuelist",
    )
    .fetch_one(pool)
    .await?;

    let result = sqlx::query(
        "INSERT INTO queuelist (songID, sortID, requests, requestID, PLOTW, dedication) \
         VALUES (?, ?, 0, 0, 0, 0)",
    )
    .bind(song_id)
    .bind(next_sort)
    .execute(pool)
    .await?;

    Ok(result.last_insert_id() as i64)
}

/// Delete a queue entry (called after the track has been played / skipped).
pub async fn remove_from_queue(pool: &MySqlPool, queue_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM queuelist WHERE ID = ?")
        .bind(queue_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── historylist ──────────────────────────────────────────────────────────────

/// A row from SAM's `historylist` table.
/// SAM stores a full metadata snapshot at the time of play for royalty-compliance
/// (so the record is correct even if the song is later edited or deleted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub song_id: i64,        // `songID`
    pub filename: String,
    pub date_played: String, // MySQL datetime
    pub duration: i32,       // seconds
    pub artist: String,
    pub title: String,
    pub album: String,
    pub albumyear: String,
    pub listeners: i32,
    pub label: String,
    pub isrc: String,        // `ISRC`
    pub upc: String,         // `UPC`
    pub songtype: String,
    pub request_id: i32,     // `requestID`
    pub overlay: String,     // 'yes' | 'no'
    pub songrights: String,
}

/// Fetch the most recent history entries.
pub async fn get_history(
    pool: &MySqlPool,
    limit: u32,
) -> Result<Vec<HistoryEntry>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT ID, songID, filename, date_played, duration,
                  artist, title, album, albumyear, listeners,
                  label, ISRC, UPC, songtype, requestID, overlay, songrights
           FROM historylist
           ORDER BY date_played DESC
           LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| HistoryEntry {
            id: r.get("ID"),
            song_id: r.get("songID"),
            filename: r.get("filename"),
            date_played: r.get::<String, _>("date_played"),
            duration: r.get("duration"),
            artist: r.get("artist"),
            title: r.get("title"),
            album: r.get("album"),
            albumyear: r.get("albumyear"),
            listeners: r.get::<i32, _>("listeners"),
            label: r.get("label"),
            isrc: r.get("ISRC"),
            upc: r.get("UPC"),
            songtype: r.get("songtype"),
            request_id: r.get::<i32, _>("requestID"),
            overlay: r.get("overlay"),
            songrights: r.get("songrights"),
        })
        .collect())
}

/// Write a full metadata snapshot to `historylist`.
/// Call this when a track finishes playing. Copies metadata from `song` so the
/// history record is correct even if the song is later edited in SAM.
pub async fn add_to_history(pool: &MySqlPool, song: &SamSong) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO historylist
           (songID, filename, date_played, duration,
            artist, title, album, albumyear, listeners,
            label, ISRC, UPC, songtype, requestID, overlay, songrights)
           VALUES (?, ?, NOW(), ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, 0, ?, 'broadcast')"#,
    )
    .bind(song.id)
    .bind(&song.filename)
    .bind(song.duration)
    .bind(&song.artist)
    .bind(&song.title)
    .bind(&song.album)
    .bind(&song.albumyear)
    .bind(&song.label)
    .bind(&song.isrc)
    .bind(&song.upc)
    .bind(&song.songtype)
    .bind(&song.overlay)
    .execute(pool)
    .await?;
    Ok(())
}

/// Atomically complete a queue entry: removes it from `queuelist` and writes
/// a history record to `historylist`.  Best-effort (no distributed transaction).
pub async fn complete_track(
    pool: &MySqlPool,
    queue_id: i64,
    song: &SamSong,
) -> Result<(), sqlx::Error> {
    remove_from_queue(pool, queue_id).await?;
    add_to_history(pool, song).await?;
    Ok(())
}

// ── catlist ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamCategory {
    pub id: i64,
    pub catname: String,
}

/// Fetch all SAM categories.  Returns an empty Vec if the `catlist` table
/// does not exist in this version of the SAM database.
pub async fn get_categories(pool: &MySqlPool) -> Result<Vec<SamCategory>, sqlx::Error> {
    // Gracefully handle the case where catlist doesn't exist in this SAM version
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = 'catlist'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if exists == 0 {
        return Ok(vec![]);
    }

    let rows = sqlx::query("SELECT catID as id, catname FROM catlist ORDER BY catname")
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| SamCategory {
            id: r.get("id"),
            catname: r.get("catname"),
        })
        .collect())
}
