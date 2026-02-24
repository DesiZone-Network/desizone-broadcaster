/// `scripting/api.rs` — registers all Lua global functions
///
/// Provides the full Phase 5 API surface to each script VM:
///   deck, queue, media, encoder, schedule, station, log, http, store

use mlua::{Lua, Result as LuaResult, Value};
use std::sync::{Arc, Mutex};

/// Per-script log output (log.info / log.warn / log.error calls).
#[derive(Debug, Clone)]
pub struct ScriptLogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: i64,
}

pub type ScriptLog = Arc<Mutex<Vec<ScriptLogEntry>>>;

/// Per-script key/value store (persisted to DB externally).
pub type ScriptStore = Arc<Mutex<std::collections::HashMap<String, serde_json::Value>>>;

/// Register all DesiZone Lua API globals on `lua`.
///
/// `log_sink` — entries written by log.info/warn/error land here.
/// `store`    — key/value store for the script (pre-loaded from DB).
pub fn register_all(
    lua: &Lua,
    script_id: i64,
    log_sink: ScriptLog,
    store: ScriptStore,
) -> LuaResult<()> {
    register_log(lua, script_id, log_sink)?;
    register_store(lua, store)?;
    register_deck(lua)?;
    register_queue(lua)?;
    register_media(lua)?;
    register_encoder(lua)?;
    register_schedule(lua)?;
    register_station(lua)?;
    register_http(lua)?;
    Ok(())
}

// ── log ───────────────────────────────────────────────────────────────────────

fn register_log(lua: &Lua, _script_id: i64, sink: ScriptLog) -> LuaResult<()> {
    let log_tbl = lua.create_table()?;

    macro_rules! log_fn {
        ($level:literal) => {{
            let sink = Arc::clone(&sink);
            lua.create_function(move |_, msg: String| {
                let entry = ScriptLogEntry {
                    level: $level.to_string(),
                    message: msg.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                };
                log::info!("[script][{}] {}", $level, msg);
                sink.lock().unwrap().push(entry);
                Ok(())
            })?
        }};
    }

    log_tbl.set("info", log_fn!("info"))?;
    log_tbl.set("warn", log_fn!("warn"))?;
    log_tbl.set("error", log_fn!("error"))?;
    lua.globals().set("log", log_tbl)?;
    Ok(())
}

// ── store ─────────────────────────────────────────────────────────────────────

fn register_store(lua: &Lua, store: ScriptStore) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    let store_set = {
        let store = Arc::clone(&store);
        lua.create_function(move |_, (key, val): (String, Value)| {
            let json = lua_value_to_json(val);
            store.lock().unwrap().insert(key, json);
            Ok(())
        })?
    };

    let store_get = {
        let store = Arc::clone(&store);
        lua.create_function(move |lua_ctx, key: String| {
            let map = store.lock().unwrap();
            match map.get(&key) {
                Some(v) => json_to_lua_value(lua_ctx, v),
                None => Ok(Value::Nil),
            }
        })?
    };

    let store_del = {
        let store = Arc::clone(&store);
        lua.create_function(move |_, key: String| {
            store.lock().unwrap().remove(&key);
            Ok(())
        })?
    };

    tbl.set("set", store_set)?;
    tbl.set("get", store_get)?;
    tbl.set("delete", store_del)?;
    lua.globals().set("store", tbl)?;
    Ok(())
}

// ── deck ──────────────────────────────────────────────────────────────────────

fn register_deck(lua: &Lua) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    // Stub implementations — in a full integration these would call into AppState.
    // Scripts can still call these; no-ops will be logged.
    tbl.set("play", lua.create_function(|_, deck_id: String| {
        log::info!("[script] deck.play({})", deck_id);
        Ok(())
    })?)?;
    tbl.set("stop", lua.create_function(|_, deck_id: String| {
        log::info!("[script] deck.stop({})", deck_id);
        Ok(())
    })?)?;
    tbl.set("load", lua.create_function(|_, (deck_id, song_id): (String, i64)| {
        log::info!("[script] deck.load({}, {})", deck_id, song_id);
        Ok(())
    })?)?;
    tbl.set("get_position", lua.create_function(|_, deck_id: String| {
        log::info!("[script] deck.get_position({})", deck_id);
        Ok(0u64)
    })?)?;

    lua.globals().set("deck", tbl)?;
    Ok(())
}

// ── queue ─────────────────────────────────────────────────────────────────────

fn register_queue(lua: &Lua) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    tbl.set("get", lua.create_function(|lua_ctx, ()| {
        lua_ctx.create_table() // empty table — full integration would call queue commands
    })?)?;
    tbl.set("add", lua.create_function(|_, song_id: i64| {
        log::info!("[script] queue.add({})", song_id);
        Ok(())
    })?)?;
    tbl.set("add_at", lua.create_function(|_, (song_id, pos): (i64, u32)| {
        log::info!("[script] queue.add_at({}, {})", song_id, pos);
        Ok(())
    })?)?;
    tbl.set("remove", lua.create_function(|_, pos: u32| {
        log::info!("[script] queue.remove({})", pos);
        Ok(())
    })?)?;
    tbl.set("clear", lua.create_function(|_, ()| {
        log::info!("[script] queue.clear()");
        Ok(())
    })?)?;
    tbl.set("add_playlist", lua.create_function(|_, playlist_id: i64| {
        log::info!("[script] queue.add_playlist({})", playlist_id);
        Ok(())
    })?)?;

    lua.globals().set("queue", tbl)?;
    Ok(())
}

// ── media ─────────────────────────────────────────────────────────────────────

fn register_media(lua: &Lua) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    tbl.set("search", lua.create_function(|lua_ctx, _query: String| {
        lua_ctx.create_table() // stub
    })?)?;
    tbl.set("get", lua.create_function(|lua_ctx, _id: i64| {
        lua_ctx.create_table() // stub
    })?)?;
    tbl.set("get_random", lua.create_function(|lua_ctx, _category: Value| {
        lua_ctx.create_table() // stub
    })?)?;

    lua.globals().set("media", tbl)?;
    Ok(())
}

// ── encoder ───────────────────────────────────────────────────────────────────

fn register_encoder(lua: &Lua) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    tbl.set("start", lua.create_function(|_, id: i64| {
        log::info!("[script] encoder.start({})", id);
        Ok(())
    })?)?;
    tbl.set("stop", lua.create_function(|_, id: i64| {
        log::info!("[script] encoder.stop({})", id);
        Ok(())
    })?)?;
    tbl.set("set_stream_title", lua.create_function(|_, (id, title): (i64, String)| {
        log::info!("[script] encoder.set_stream_title({}, '{}')", id, title);
        Ok(())
    })?)?;
    tbl.set("get_listeners", lua.create_function(|_, id: i64| {
        log::info!("[script] encoder.get_listeners({})", id);
        Ok(0u32)
    })?)?;

    lua.globals().set("encoder", tbl)?;
    Ok(())
}

// ── schedule ──────────────────────────────────────────────────────────────────

fn register_schedule(lua: &Lua) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    tbl.set("add_once", lua.create_function(|_, (_dt, _fn): (String, Value)| {
        log::info!("[script] schedule.add_once()");
        Ok(0i64) // returns schedule id
    })?)?;
    tbl.set("add_cron", lua.create_function(|_, (_expr, _fn): (String, Value)| {
        log::info!("[script] schedule.add_cron()");
        Ok(0i64)
    })?)?;
    tbl.set("remove", lua.create_function(|_, id: i64| {
        log::info!("[script] schedule.remove({})", id);
        Ok(())
    })?)?;

    lua.globals().set("schedule", tbl)?;
    Ok(())
}

// ── station ───────────────────────────────────────────────────────────────────

fn register_station(lua: &Lua) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    tbl.set("set_mode", lua.create_function(|_, mode: String| {
        log::info!("[script] station.set_mode('{}')", mode);
        Ok(())
    })?)?;
    tbl.set("emergency_stop", lua.create_function(|_, ()| {
        log::warn!("[script] station.emergency_stop() invoked");
        Ok(())
    })?)?;

    lua.globals().set("station", tbl)?;
    Ok(())
}

// ── http ──────────────────────────────────────────────────────────────────────

fn register_http(lua: &Lua) -> LuaResult<()> {
    let tbl = lua.create_table()?;

    // http.get(url) — synchronous (blocking) HTTP GET for script use
    tbl.set("get", lua.create_function(|lua_ctx, url: String| {
        match reqwest::blocking::get(&url) {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().unwrap_or_default();
                let t = lua_ctx.create_table()?;
                t.set("status", status)?;
                t.set("body", body)?;
                Ok(t)
            }
            Err(e) => {
                let t = lua_ctx.create_table()?;
                t.set("status", 0u16)?;
                t.set("body", e.to_string())?;
                Ok(t)
            }
        }
    })?)?;

    // http.post(url, body_json)
    tbl.set("post", lua.create_function(|lua_ctx, (url, body): (String, String)| {
        let client = reqwest::blocking::Client::new();
        match client.post(&url).header("Content-Type", "application/json").body(body).send() {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().unwrap_or_default();
                let t = lua_ctx.create_table()?;
                t.set("status", status)?;
                t.set("body", body)?;
                Ok(t)
            }
            Err(e) => {
                let t = lua_ctx.create_table()?;
                t.set("status", 0u16)?;
                t.set("body", e.to_string())?;
                Ok(t)
            }
        }
    })?)?;

    lua.globals().set("http", tbl)?;
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn lua_value_to_json(val: Value) -> serde_json::Value {
    match val {
        Value::Nil => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(b),
        Value::Integer(i) => serde_json::json!(i),
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::Value::String(s.to_string_lossy()),
        Value::Table(t) => {
            // Array heuristic: check if keys are sequential integers
            let pairs: Vec<_> = t.clone().pairs::<Value, Value>().filter_map(|p| p.ok()).collect();
            let is_array = pairs.iter().enumerate().all(|(i, (k, _))| {
                matches!(k, Value::Integer(n) if *n == (i as i64 + 1))
            });
            if is_array {
                serde_json::Value::Array(pairs.into_iter().map(|(_, v)| lua_value_to_json(v)).collect())
            } else {
                let mut map = serde_json::Map::new();
                for (k, v) in pairs {
                    if let Value::String(ks) = k {
                        map.insert(ks.to_string_lossy(), lua_value_to_json(v));
                    }
                }
                serde_json::Value::Object(map)
            }
        }
        _ => serde_json::Value::Null,
    }
}

fn json_to_lua_value(lua: &Lua, val: &serde_json::Value) -> LuaResult<Value> {
    match val {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i))
            } else {
                Ok(Value::Number(n.as_f64().unwrap_or(0.0)))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(lua.create_string(s.as_str())?)),
        serde_json::Value::Array(arr) => {
            let t = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                t.set(i + 1, json_to_lua_value(lua, v)?)?;
            }
            Ok(Value::Table(t))
        }
        serde_json::Value::Object(obj) => {
            let t = lua.create_table()?;
            for (k, v) in obj {
                t.set(k.as_str(), json_to_lua_value(lua, v)?)?;
            }
            Ok(Value::Table(t))
        }
    }
}
