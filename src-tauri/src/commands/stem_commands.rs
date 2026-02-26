use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    audio::analyzer::stems::separate_two_stems_vocals, db::local::StemAnalysis, state::AppState,
};

use super::audio_commands::parse_deck;

const PY_STANDALONE_RELEASE_API: &str =
    "https://api.github.com/repos/indygreg/python-build-standalone/releases/latest";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StemPlaybackSource {
    Original,
    Vocals,
    Instrumental,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckStemSourceResult {
    pub source: StemPlaybackSource,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StemsRuntimeStatus {
    pub ready: bool,
    pub runtime_dir: String,
    pub python_path: Option<String>,
    pub ffmpeg_available: bool,
    pub message: String,
}

#[tauri::command]
pub async fn get_stems_runtime_status() -> Result<StemsRuntimeStatus, String> {
    Ok(read_stems_runtime_status())
}

#[tauri::command]
pub async fn install_stems_runtime() -> Result<StemsRuntimeStatus, String> {
    tauri::async_runtime::spawn_blocking(install_stems_runtime_blocking)
        .await
        .map_err(|e| format!("Stems runtime installer join failed: {e}"))?
}

#[tauri::command]
pub async fn analyze_stems(
    song_id: i64,
    file_path: String,
    force_reanalyze: Option<bool>,
    state: State<'_, AppState>,
) -> Result<StemAnalysis, String> {
    let local = state
        .local_db
        .as_ref()
        .ok_or("Local DB not initialised")?
        .clone();
    let input_path = PathBuf::from(&file_path);
    if !input_path.exists() {
        return Err(format!("File not found: {file_path}"));
    }
    if !input_path.is_file() {
        return Err(format!("Path is not a file: {file_path}"));
    }

    let mtime_ms = file_mtime_ms(&input_path);
    let force = force_reanalyze.unwrap_or(false);
    if !force {
        if let Ok(Some(cached)) =
            crate::db::local::get_stem_analysis(&local, song_id, &file_path, mtime_ms).await
        {
            if Path::new(&cached.vocals_file_path).exists()
                && Path::new(&cached.instrumental_file_path).exists()
            {
                return Ok(cached);
            }
        }
    }

    let output_root = stem_output_root(song_id, mtime_ms);
    if force && output_root.exists() {
        let _ = fs::remove_dir_all(&output_root);
    }
    let separate_input = input_path.clone();
    let separate_output = output_root.clone();
    let preferred_python = resolve_runtime_python_bin();
    let computed = tauri::async_runtime::spawn_blocking(move || {
        separate_two_stems_vocals(
            &separate_input,
            &separate_output,
            preferred_python.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Stem worker join failed: {e}"))??;

    let analysis = StemAnalysis {
        song_id,
        source_file_path: file_path.clone(),
        source_mtime_ms: mtime_ms,
        vocals_file_path: computed.vocals_path.to_string_lossy().to_string(),
        instrumental_file_path: computed.instrumental_path.to_string_lossy().to_string(),
        model_name: computed.model_name,
        updated_at: None,
    };
    crate::db::local::save_stem_analysis(&local, &analysis)
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    crate::db::local::get_stem_analysis(&local, song_id, &file_path, mtime_ms)
        .await
        .map_err(|e| format!("DB error: {e}"))?
        .ok_or("Failed to read saved stem analysis".to_string())
}

#[tauri::command]
pub async fn get_stem_analysis(
    song_id: i64,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<Option<StemAnalysis>, String> {
    let local = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let input_path = PathBuf::from(&file_path);
    if !input_path.exists() || !input_path.is_file() {
        return Ok(None);
    }
    let mtime_ms = file_mtime_ms(&input_path);
    let row = crate::db::local::get_stem_analysis(local, song_id, &file_path, mtime_ms)
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    Ok(row.and_then(validate_stem_files))
}

#[tauri::command]
pub async fn get_latest_stem_analysis(
    song_id: i64,
    state: State<'_, AppState>,
) -> Result<Option<StemAnalysis>, String> {
    let local = state.local_db.as_ref().ok_or("Local DB not initialised")?;
    let row = crate::db::local::get_latest_stem_analysis_by_song_id(local, song_id)
        .await
        .map_err(|e| format!("DB error: {e}"))?;
    Ok(row.and_then(validate_stem_files))
}

#[tauri::command]
pub async fn set_deck_stem_source(
    deck: String,
    source: StemPlaybackSource,
    song_id: Option<i64>,
    original_file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<DeckStemSourceResult, String> {
    let deck_id = parse_deck(&deck)?;
    let latest = if let Some(local) = &state.local_db {
        if let Some(id) = song_id {
            crate::db::local::get_latest_stem_analysis_by_song_id(local, id)
                .await
                .map_err(|e| format!("DB error: {e}"))?
                .and_then(validate_stem_files)
        } else {
            None
        }
    } else {
        None
    };

    let current_loaded = {
        state
            .engine
            .lock()
            .unwrap()
            .get_deck_state(deck_id)
            .and_then(|s| s.file_path)
    };

    let target = match source {
        StemPlaybackSource::Original => original_file_path
            .filter(|p| !p.trim().is_empty())
            .or_else(|| latest.as_ref().map(|r| r.source_file_path.clone()))
            .or(current_loaded)
            .ok_or("No original track path available for this deck")?,
        StemPlaybackSource::Vocals => latest
            .as_ref()
            .map(|r| r.vocals_file_path.clone())
            .ok_or("No generated stems found. Run Generate Stems first.")?,
        StemPlaybackSource::Instrumental => latest
            .as_ref()
            .map(|r| r.instrumental_file_path.clone())
            .ok_or("No generated stems found. Run Generate Stems first.")?,
    };

    state
        .engine
        .lock()
        .unwrap()
        .switch_deck_track_source(deck_id, PathBuf::from(&target))?;

    Ok(DeckStemSourceResult {
        source,
        file_path: target,
    })
}

fn install_stems_runtime_blocking() -> Result<StemsRuntimeStatus, String> {
    let runtime_root = stems_runtime_root();
    fs::create_dir_all(&runtime_root).map_err(|e| {
        format!(
            "Failed to create runtime dir {}: {e}",
            runtime_root.display()
        )
    })?;

    if resolve_runtime_python_bin().is_none() {
        install_python_standalone(&runtime_root)?;
    }
    let python = resolve_runtime_python_bin().ok_or_else(|| {
        "Python runtime extraction completed but python executable was not found".to_string()
    })?;

    run_python_cmd(&python, &["-m", "ensurepip", "--upgrade"])?;
    run_python_cmd(&python, &["-m", "pip", "install", "--upgrade", "pip"])?;
    run_python_cmd(
        &python,
        &["-m", "pip", "install", "--upgrade", "demucs", "torchcodec"],
    )?;

    let status = read_stems_runtime_status();
    if status.ready {
        Ok(status)
    } else {
        Err(format!(
            "Stems runtime install finished but runtime is not ready: {}",
            status.message
        ))
    }
}

fn install_python_standalone(runtime_root: &Path) -> Result<(), String> {
    let pattern = platform_asset_pattern()
        .ok_or("Portable Python runtime installer is not available for this platform")?;
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let release_json: serde_json::Value = client
        .get(PY_STANDALONE_RELEASE_API)
        .header("User-Agent", "desizone-broadcaster")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Failed to query python-build-standalone release API: {e}"))?
        .json()
        .map_err(|e| format!("Failed to parse release API response: {e}"))?;

    let assets = release_json["assets"]
        .as_array()
        .ok_or("Release API response missing assets list")?;

    let mut selected: Option<(String, String)> = None;
    for asset in assets {
        let name = asset["name"].as_str().unwrap_or_default();
        let url = asset["browser_download_url"].as_str().unwrap_or_default();
        if name.contains("cpython-3.11")
            && name.contains(pattern)
            && name.ends_with(".tar.gz")
            && name.contains("install_only")
        {
            selected = Some((name.to_string(), url.to_string()));
            break;
        }
    }
    if selected.is_none() {
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or_default();
            let url = asset["browser_download_url"].as_str().unwrap_or_default();
            if name.contains(pattern) && name.ends_with(".tar.gz") && name.contains("install_only")
            {
                selected = Some((name.to_string(), url.to_string()));
                break;
            }
        }
    }
    let (asset_name, asset_url) = selected.ok_or_else(|| {
        format!("No matching portable Python asset found for platform pattern `{pattern}`")
    })?;

    let downloads_dir = runtime_root.join("downloads");
    fs::create_dir_all(&downloads_dir).map_err(|e| {
        format!(
            "Failed to create download dir {}: {e}",
            downloads_dir.display()
        )
    })?;
    let archive_path = downloads_dir.join(&asset_name);
    let mut response = client
        .get(asset_url)
        .header("User-Agent", "desizone-broadcaster")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Failed to download portable Python runtime: {e}"))?;
    let mut out = File::create(&archive_path).map_err(|e| {
        format!(
            "Failed to create archive file {}: {e}",
            archive_path.display()
        )
    })?;
    response
        .copy_to(&mut out)
        .map_err(|e| format!("Failed to write runtime archive: {e}"))?;

    let extract_dir = runtime_root.join("python-dist");
    if extract_dir.exists() {
        let _ = fs::remove_dir_all(&extract_dir);
    }
    fs::create_dir_all(&extract_dir).map_err(|e| {
        format!(
            "Failed to create extract dir {}: {e}",
            extract_dir.display()
        )
    })?;
    let tar_gz = File::open(&archive_path).map_err(|e| {
        format!(
            "Failed to open runtime archive {}: {e}",
            archive_path.display()
        )
    })?;
    let decoder = GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(&extract_dir)
        .map_err(|e| format!("Failed to extract portable Python runtime: {e}"))?;

    Ok(())
}

fn run_python_cmd(python: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new(python)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {} {:?}: {e}", python.display(), args))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = summarize_command_output(&format!("{stderr}\n{stdout}"));
    Err(format!(
        "{} {:?} failed (code {:?}): {}",
        python.display(),
        args,
        output.status.code(),
        detail
    ))
}

fn read_stems_runtime_status() -> StemsRuntimeStatus {
    let runtime_dir = stems_runtime_root();
    let python_bin = resolve_runtime_python_bin();
    let ready = python_bin
        .as_ref()
        .map(|p| check_python_modules(p))
        .unwrap_or(false);
    let ffmpeg_ok = ffmpeg_available();

    let message = if ready {
        if ffmpeg_ok {
            "Stems runtime ready".to_string()
        } else {
            "Stems runtime ready, but FFmpeg was not found in PATH".to_string()
        }
    } else if python_bin.is_none() {
        "Stems runtime not installed".to_string()
    } else {
        "Python runtime found, but Demucs dependencies are missing".to_string()
    };

    StemsRuntimeStatus {
        ready,
        runtime_dir: runtime_dir.to_string_lossy().to_string(),
        python_path: python_bin.map(|p| p.to_string_lossy().to_string()),
        ffmpeg_available: ffmpeg_ok,
        message,
    }
}

fn check_python_modules(python: &Path) -> bool {
    Command::new(python)
        .args(["-c", "import demucs, torchcodec"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn resolve_runtime_python_bin() -> Option<PathBuf> {
    let root = stems_runtime_root();
    let candidates = [
        root.join("python-dist")
            .join("python")
            .join("bin")
            .join("python3"),
        root.join("python-dist")
            .join("python")
            .join("bin")
            .join("python"),
        root.join("python-dist").join("bin").join("python3"),
        root.join("python-dist").join("bin").join("python"),
        root.join("python-dist").join("python.exe"),
    ];
    for candidate in candidates {
        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }
    find_python_recursive(&root.join("python-dist"), 0)
}

fn find_python_recursive(dir: &Path, depth: usize) -> Option<PathBuf> {
    if depth > 5 || !dir.exists() || !dir.is_dir() {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name == "python3" || name == "python" || name == "python.exe" {
                return Some(path);
            }
        }
    }
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_python_recursive(&path, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

fn summarize_command_output(raw: &str) -> String {
    let normalized = raw.replace('\r', "\n");
    let mut lines: Vec<&str> = normalized
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return "no output".to_string();
    }
    if lines.len() > 12 {
        lines = lines.split_off(lines.len() - 12);
    }
    lines.join(" | ")
}

fn platform_asset_pattern() -> Option<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Some("aarch64-apple-darwin");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Some("x86_64-apple-darwin");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Some("x86_64-unknown-linux-gnu");
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Some("x86_64-pc-windows-msvc");
    }
    #[allow(unreachable_code)]
    None
}

fn validate_stem_files(row: StemAnalysis) -> Option<StemAnalysis> {
    if Path::new(&row.vocals_file_path).exists() && Path::new(&row.instrumental_file_path).exists()
    {
        Some(row)
    } else {
        None
    }
}

fn file_mtime_ms(path: &Path) -> i64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn stem_output_root(song_id: i64, mtime_ms: i64) -> PathBuf {
    PathBuf::from(compute_app_data_dir())
        .join("stems")
        .join(format!("song_{song_id}_{mtime_ms}"))
}

fn stems_runtime_root() -> PathBuf {
    PathBuf::from(compute_app_data_dir()).join("stems_runtime")
}

fn compute_app_data_dir() -> String {
    const IDENTIFIER: &str = "com.minhaj.desizonebroadcaster";

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        return format!("{home}/Library/Application Support/{IDENTIFIER}");
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        return format!("{appdata}\\{IDENTIFIER}");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.config/{IDENTIFIER}")
    }
}
