use serde::{Deserialize, Serialize};
use sqlx::{mysql::MySqlPool, QueryBuilder, Row};
use std::collections::HashMap;

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
    if norm_file
        .to_lowercase()
        .starts_with(&norm_from.to_lowercase())
    {
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
    pub id: i64, // `ID` — primary key
    pub filename: String,
    pub songtype: String, // 'S'=Song, 'J'=Jingle, etc.
    pub status: i32,      // 0=disabled, 1=enabled
    pub weight: f64,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub genre: String,
    pub albumyear: String,
    pub duration: i32, // seconds
    pub bpm: i32,
    pub xfade: String, // crossfade preset name
    pub mood: String,
    pub mood_ai: Option<String>,
    pub rating: i32,
    pub count_played: i32,
    pub date_played: Option<String>, // MySQL datetime as String
    pub label: String,
    pub isrc: String, // `ISRC` column
    pub upc: String,  // `UPC` column (also used for Spotify ID)
    pub picture: Option<String>,
    pub overlay: String, // 'yes' | 'no'
}

/// Map a MySQL row to `SamSong` defensively.
///
/// Uses `try_get` for every field except the primary key so that the app
/// works with any SAM Broadcaster version, regardless of which optional
/// columns (mood_ai, ISRC, UPC, overlay, albumyear, …) are present.
fn row_to_sam_song(r: &sqlx::mysql::MySqlRow) -> SamSong {
    // Primary key: try "ID" first (modern SAM), fall back to "songID" (older SAM)
    let id = r
        .try_get::<i64, _>("ID")
        .or_else(|_| r.try_get::<i32, _>("ID").map(|v| v as i64))
        .or_else(|_| r.try_get::<i64, _>("songID"))
        .or_else(|_| r.try_get::<i32, _>("songID").map(|v| v as i64))
        .unwrap_or(0);

    SamSong {
        id,
        filename: r.try_get("filename").unwrap_or_default(),
        songtype: r.try_get("songtype").unwrap_or_else(|_| "S".to_string()),
        status: r
            .try_get::<i8, _>("status")
            .map(|v| v as i32)
            .or_else(|_| r.try_get::<i32, _>("status"))
            .unwrap_or(1),
        weight: r.try_get("weight").unwrap_or(1.0),
        artist: r.try_get("artist").unwrap_or_default(),
        title: r.try_get("title").unwrap_or_default(),
        album: r.try_get("album").unwrap_or_default(),
        genre: r.try_get("genre").unwrap_or_default(),
        albumyear: r.try_get("albumyear").unwrap_or_default(),
        duration: r.try_get::<i32, _>("duration").unwrap_or(0),
        bpm: r.try_get::<i32, _>("bpm").unwrap_or(0),
        xfade: r.try_get("xfade").unwrap_or_default(),
        mood: r.try_get("mood").unwrap_or_default(),
        mood_ai: r.try_get("mood_ai").ok(), // column absent in most SAM installs
        rating: r.try_get::<i32, _>("rating").unwrap_or(0),
        count_played: r.try_get::<i32, _>("count_played").unwrap_or(0),
        date_played: r.try_get("date_played").ok(),
        label: r.try_get("label").unwrap_or_default(),
        isrc: r.try_get("ISRC").unwrap_or_default(),
        upc: r.try_get("UPC").unwrap_or_default(),
        picture: r.try_get("picture").ok(),
        overlay: r.try_get("overlay").unwrap_or_default(),
    }
}

/// Fetch a single song by its SAM `ID`.
pub async fn get_song(pool: &MySqlPool, song_id: i64) -> Result<Option<SamSong>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM songlist WHERE ID = ?")
        .bind(song_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_sam_song))
}

/// Search songs with field-level filtering and optional song type filter.
///
/// - If all four field flags are `false`, defaults to searching artist + title.
/// - `song_type = None` means all types; `Some("S")` filters to that type.
/// - Shows ALL songs (any status) so the media library is never empty.
pub async fn search_songs(
    pool: &MySqlPool,
    query: &str,
    search_artist: bool,
    search_title: bool,
    search_album: bool,
    search_filename: bool,
    song_type: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SamSong>, sqlx::Error> {
    let pattern = format!("%{query}%");

    // Default to artist + title when no field is selected
    let (sa, st, sb, sf) = if !search_artist && !search_title && !search_album && !search_filename {
        (true, true, false, false)
    } else {
        (search_artist, search_title, search_album, search_filename)
    };

    let mut qb: QueryBuilder<sqlx::MySql> = QueryBuilder::new("SELECT * FROM songlist WHERE (");
    let mut first = true;

    let mut push_field = |qb: &mut QueryBuilder<sqlx::MySql>, col: &str, first: &mut bool| {
        if !*first {
            qb.push(" OR ");
        }
        qb.push(col).push(" LIKE ").push_bind(pattern.clone());
        *first = false;
    };

    if sa {
        push_field(&mut qb, "artist", &mut first);
    }
    if st {
        push_field(&mut qb, "title", &mut first);
    }
    if sb {
        push_field(&mut qb, "album", &mut first);
    }
    if sf {
        push_field(&mut qb, "filename", &mut first);
    }

    qb.push(")");

    if let Some(st) = song_type {
        qb.push(" AND songtype = ").push_bind(st.to_string());
    }

    qb.push(" ORDER BY artist, title LIMIT ").push_bind(limit);
    qb.push(" OFFSET ").push_bind(offset);

    let rows = qb.build().fetch_all(pool).await?;
    Ok(rows.iter().map(row_to_sam_song).collect())
}

// ── queuelist ────────────────────────────────────────────────────────────────

/// A row from SAM's `queuelist` table.
/// SAM queues use a floating-point `sortID` for ordering.
/// There is no `played` column — completed entries are simply deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: i64,               // `ID` — primary key
    pub song_id: i64,          // `songID`
    pub sort_id: f64,          // `sortID` — ordering key (float)
    pub requests: i32,         // number of listener requests for this slot
    pub request_id: i32,       // `requestID` — linked request (0 if none)
    pub plotw: i32,            // `PLOTW`: 0=Song/PLO, 1=VoiceBreak/TW
    pub dedication: i32,       // `dedication` flag
    pub song: Option<SamSong>, // hydrated from songlist by song_id
}

/// Fetch all pending queue entries ordered by sortID.
pub async fn get_queue(pool: &MySqlPool) -> Result<Vec<QueueEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT ID, songID, sortID, requests, requestID, PLOTW, dedication \
         FROM queuelist ORDER BY sortID ASC",
    )
    .fetch_all(pool)
    .await?;

    let mut entries: Vec<QueueEntry> = rows
        .into_iter()
        .map(|r| QueueEntry {
            id: r.get("ID"),
            song_id: r.get("songID"),
            sort_id: r.get("sortID"),
            requests: r.get::<i32, _>("requests"),
            request_id: r.get::<i32, _>("requestID"),
            plotw: r.get::<i8, _>("PLOTW") as i32,
            dedication: r.get::<i8, _>("dedication") as i32,
            song: None,
        })
        .collect();

    if entries.is_empty() {
        return Ok(entries);
    }

    let song_ids: Vec<i64> = entries.iter().map(|e| e.song_id).collect();
    let mut qb: QueryBuilder<sqlx::MySql> =
        QueryBuilder::new("SELECT * FROM songlist WHERE ID IN (");
    let mut separated = qb.separated(", ");
    for id in &song_ids {
        separated.push_bind(*id);
    }
    drop(separated);
    qb.push(")");

    let song_rows = qb.build().fetch_all(pool).await?;
    let songs_by_id: HashMap<i64, SamSong> = song_rows
        .iter()
        .map(row_to_sam_song)
        .map(|song| (song.id, song))
        .collect();

    for entry in &mut entries {
        entry.song = songs_by_id.get(&entry.song_id).cloned();
    }

    Ok(entries)
}

/// Append a song to the end of the queue.
/// The `sortID` is assigned as MAX(sortID)+1 so the new entry goes last.
pub async fn add_to_queue(pool: &MySqlPool, song_id: i64) -> Result<i64, sqlx::Error> {
    // Compute next sortID in a single round-trip
    let next_sort: f64 = sqlx::query_scalar("SELECT COALESCE(MAX(sortID), 0) + 1 FROM queuelist")
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
    pub song_id: i64, // `songID`
    pub filename: String,
    pub date_played: String, // MySQL datetime
    pub duration: i32,       // seconds
    pub artist: String,
    pub title: String,
    pub album: String,
    pub albumyear: String,
    pub listeners: i32,
    pub label: String,
    pub isrc: String, // `ISRC`
    pub upc: String,  // `UPC`
    pub songtype: String,
    pub request_id: i32, // `requestID`
    pub overlay: String, // 'yes' | 'no'
    pub songrights: String,
}

/// Fetch the most recent history entries.
pub async fn get_history(pool: &MySqlPool, limit: u32) -> Result<Vec<HistoryEntry>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT ID, songID, filename, date_played, duration,
                  artist, title, album, albumyear, listeners,
                  label, ISRC, UPC, songtype, requestID, overlay, songrights,
                  DATE_FORMAT(date_played, '%Y-%m-%dT%H:%i:%s') AS date_played_iso
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
            id: r
                .try_get::<i64, _>("ID")
                .or_else(|_| r.try_get::<i32, _>("ID").map(|v| v as i64))
                .unwrap_or(0),
            song_id: r
                .try_get::<i64, _>("songID")
                .or_else(|_| r.try_get::<i32, _>("songID").map(|v| v as i64))
                .unwrap_or(0),
            filename: r.try_get("filename").unwrap_or_default(),
            date_played: r
                .try_get::<String, _>("date_played_iso")
                .or_else(|_| r.try_get::<String, _>("date_played"))
                .unwrap_or_default(),
            duration: r
                .try_get::<i32, _>("duration")
                .or_else(|_| r.try_get::<i16, _>("duration").map(|v| v as i32))
                .unwrap_or(0),
            artist: r.try_get("artist").unwrap_or_default(),
            title: r.try_get("title").unwrap_or_default(),
            album: r.try_get("album").unwrap_or_default(),
            albumyear: r.try_get("albumyear").unwrap_or_default(),
            listeners: r
                .try_get::<i32, _>("listeners")
                .or_else(|_| r.try_get::<i16, _>("listeners").map(|v| v as i32))
                .unwrap_or(0),
            label: r.try_get("label").unwrap_or_default(),
            isrc: r.try_get("ISRC").unwrap_or_default(),
            upc: r.try_get("UPC").unwrap_or_default(),
            songtype: r.try_get("songtype").unwrap_or_default(),
            request_id: r
                .try_get::<i32, _>("requestID")
                .or_else(|_| r.try_get::<i16, _>("requestID").map(|v| v as i32))
                .unwrap_or(0),
            overlay: r.try_get("overlay").unwrap_or_default(),
            songrights: r.try_get("songrights").unwrap_or_default(),
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
    pub parent_id: i64,
    pub levelindex: i32,
    pub itemindex: i64,
}

async fn table_exists(pool: &MySqlPool, table_name: &str) -> bool {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table_name)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    exists > 0
}

async fn column_exists(pool: &MySqlPool, table_name: &str, column_name: &str) -> bool {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
    )
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    exists > 0
}

/// Fetch all SAM categories.
///
/// Supports both schemas:
/// - Modern/custom installs: `category(ID, name, parentID, levelindex, itemindex)`
/// - Legacy installs: `catlist(catID, catname)`
pub async fn get_categories(pool: &MySqlPool) -> Result<Vec<SamCategory>, sqlx::Error> {
    if table_exists(pool, "category").await {
        let rows = sqlx::query(
            r#"SELECT
                   ID AS id,
                   name AS catname,
                   parentID AS parent_id,
                   levelindex,
                   itemindex
               FROM category
               ORDER BY parentID, levelindex, itemindex, name"#,
        )
        .fetch_all(pool)
        .await?;

        return Ok(rows
            .into_iter()
            .map(|r| SamCategory {
                id: r
                    .try_get::<i64, _>("id")
                    .or_else(|_| r.try_get::<i32, _>("id").map(|v| v as i64))
                    .unwrap_or(0),
                catname: r.try_get("catname").unwrap_or_default(),
                parent_id: r
                    .try_get::<i64, _>("parent_id")
                    .or_else(|_| r.try_get::<i32, _>("parent_id").map(|v| v as i64))
                    .unwrap_or(0),
                levelindex: r
                    .try_get::<i32, _>("levelindex")
                    .or_else(|_| r.try_get::<i8, _>("levelindex").map(|v| v as i32))
                    .unwrap_or(0),
                itemindex: r
                    .try_get::<i64, _>("itemindex")
                    .or_else(|_| r.try_get::<i32, _>("itemindex").map(|v| v as i64))
                    .unwrap_or(0),
            })
            .collect());
    }

    if !table_exists(pool, "catlist").await {
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
            parent_id: 0,
            levelindex: 0,
            itemindex: 0,
        })
        .collect())
}

/// Fetch all songs belonging to a SAM category via the `categorylist` join table.
/// Returns an empty Vec if `categorylist` doesn't exist in this SAM version.
pub async fn get_songs_in_category(
    pool: &MySqlPool,
    category_id: i64,
    limit: u32,
    offset: u32,
) -> Result<Vec<SamSong>, sqlx::Error> {
    if !table_exists(pool, "categorylist").await {
        return Ok(vec![]);
    }

    let category_key_col = if column_exists(pool, "categorylist", "categoryID").await {
        "categoryID"
    } else if column_exists(pool, "categorylist", "catID").await {
        "catID"
    } else {
        return Ok(vec![]);
    };

    let order_by = if column_exists(pool, "categorylist", "sortID").await {
        "cl.sortID, s.artist, s.title"
    } else {
        "s.artist, s.title"
    };

    let sql = format!(
        r#"SELECT s.*
           FROM songlist s
           INNER JOIN categorylist cl ON cl.songID = s.ID
           WHERE cl.{category_key_col} = ?
           ORDER BY {order_by}
           LIMIT ? OFFSET ?"#,
    );

    let rows = sqlx::query(&sql)
        .bind(category_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(row_to_sam_song).collect())
}

/// Fetch songs whose weight falls in [min_weight, max_weight).
/// Used for the Weighted Rotation sidebar folders (Power Hit, Heavy, Medium, etc.).
pub async fn get_songs_by_weight_range(
    pool: &MySqlPool,
    min_weight: f64,
    max_weight: f64,
    limit: u32,
    offset: u32,
) -> Result<Vec<SamSong>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT * FROM songlist
           WHERE weight >= ? AND weight < ?
           ORDER BY weight DESC, artist, title
           LIMIT ? OFFSET ?"#,
    )
    .bind(min_weight)
    .bind(max_weight)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_sam_song).collect())
}

/// Return all distinct `songtype` values present in the library, sorted.
/// Used to populate the Song Types sidebar section dynamically.
pub async fn get_distinct_song_types(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SELECT DISTINCT songtype FROM songlist ORDER BY songtype")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .map(|r| r.try_get::<String, _>("songtype").unwrap_or_default())
        .collect())
}

// ── Song update ───────────────────────────────────────────────────────────────

/// Editable fields for a song in `songlist`.
/// All fields are `Option<T>` — only provided fields are written to the DB.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct SongUpdateFields {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub albumyear: Option<String>,
    pub songtype: Option<String>,
    pub weight: Option<f64>,
    pub bpm: Option<i32>,
    pub mood: Option<String>,
    pub rating: Option<i32>,
    pub label: Option<String>,
    pub isrc: Option<String>,
    pub upc: Option<String>,
    pub overlay: Option<String>,
    pub status: Option<i32>,
}

/// Update a song in `songlist`.  Only fields present in `fields` are written.
/// Returns `true` if a row was actually modified, `false` if no fields were
/// provided or the song ID did not exist.
pub async fn update_song(
    pool: &MySqlPool,
    song_id: i64,
    fields: SongUpdateFields,
) -> Result<bool, sqlx::Error> {
    let mut qb: QueryBuilder<sqlx::MySql> = QueryBuilder::new("UPDATE songlist SET ");
    let mut first = true;

    // Helper closure — borrowing qb and first mutably
    macro_rules! push_field {
        ($col:expr, $val:expr) => {
            if let Some(v) = $val {
                if !first {
                    qb.push(", ");
                }
                qb.push(concat!($col, " = ")).push_bind(v);
                first = false;
            }
        };
    }

    push_field!("artist", fields.artist);
    push_field!("title", fields.title);
    push_field!("album", fields.album);
    push_field!("genre", fields.genre);
    push_field!("albumyear", fields.albumyear);
    push_field!("songtype", fields.songtype);
    push_field!("mood", fields.mood);
    push_field!("label", fields.label);
    push_field!("overlay", fields.overlay);

    // Numeric fields
    if let Some(v) = fields.weight {
        if !first {
            qb.push(", ");
        }
        qb.push("weight = ").push_bind(v);
        first = false;
    }
    if let Some(v) = fields.bpm {
        if !first {
            qb.push(", ");
        }
        qb.push("bpm = ").push_bind(v);
        first = false;
    }
    if let Some(v) = fields.rating {
        if !first {
            qb.push(", ");
        }
        qb.push("rating = ").push_bind(v);
        first = false;
    }
    if let Some(v) = fields.status {
        if !first {
            qb.push(", ");
        }
        qb.push("status = ").push_bind(v);
        first = false;
    }
    // Uppercase SAM column names
    if let Some(v) = fields.isrc {
        if !first {
            qb.push(", ");
        }
        qb.push("ISRC = ").push_bind(v);
        first = false;
    }
    if let Some(v) = fields.upc {
        if !first {
            qb.push(", ");
        }
        qb.push("UPC = ").push_bind(v);
        first = false;
    }

    if first {
        return Ok(false); // nothing to update
    }

    qb.push(" WHERE ID = ").push_bind(song_id);
    let result = qb.build().execute(pool).await?;
    Ok(result.rows_affected() > 0)
}
