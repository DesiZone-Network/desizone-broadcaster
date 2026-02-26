use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

#[derive(Debug, Clone)]
pub struct StemSeparationResult {
    pub model_name: String,
    pub vocals_path: PathBuf,
    pub instrumental_path: PathBuf,
}

pub fn separate_two_stems_vocals(
    input_file: &Path,
    output_root: &Path,
    preferred_python: Option<&Path>,
) -> Result<StemSeparationResult, String> {
    if !input_file.exists() {
        return Err(format!(
            "Input file does not exist: {}",
            input_file.display()
        ));
    }
    if !input_file.is_file() {
        return Err(format!(
            "Input path is not a file: {}",
            input_file.display()
        ));
    }
    fs::create_dir_all(output_root).map_err(|e| {
        format!(
            "Failed to create stem output dir {}: {e}",
            output_root.display()
        )
    })?;

    let mut errors = Vec::<String>::new();
    let out = output_root.to_string_lossy().to_string();
    let input = input_file.to_string_lossy().to_string();

    let demucs_args = vec![
        "--two-stems".to_string(),
        "vocals".to_string(),
        "-n".to_string(),
        "htdemucs".to_string(),
        "--out".to_string(),
        out.clone(),
        input.clone(),
    ];
    if let Err(e) = run_command("demucs", &demucs_args) {
        if let Ok(result) = resolve_generated_stems(output_root) {
            return Ok(result);
        }
        errors.push(format!("demucs: {e}"));
    } else {
        return resolve_generated_stems(output_root);
    }

    let py_args = vec![
        "-m".to_string(),
        "demucs.separate".to_string(),
        "--two-stems".to_string(),
        "vocals".to_string(),
        "-n".to_string(),
        "htdemucs".to_string(),
        "--out".to_string(),
        out.clone(),
        input.clone(),
    ];

    if let Some(py) = preferred_python {
        if let Err(e) = run_command_path(py, &py_args) {
            if let Ok(result) = resolve_generated_stems(output_root) {
                return Ok(result);
            }
            errors.push(format!("{} -m demucs.separate: {e}", py.display()));
        } else {
            return resolve_generated_stems(output_root);
        }
    }

    if let Err(e) = run_command("python3.11", &py_args) {
        if let Ok(result) = resolve_generated_stems(output_root) {
            return Ok(result);
        }
        errors.push(format!("python3.11 -m demucs.separate: {e}"));
    } else {
        return resolve_generated_stems(output_root);
    }

    if let Err(e) = run_command("python3", &py_args) {
        if let Ok(result) = resolve_generated_stems(output_root) {
            return Ok(result);
        }
        errors.push(format!("python3 -m demucs.separate: {e}"));
    } else {
        return resolve_generated_stems(output_root);
    }

    if let Err(e) = run_command("python", &py_args) {
        if let Ok(result) = resolve_generated_stems(output_root) {
            return Ok(result);
        }
        errors.push(format!("python -m demucs.separate: {e}"));
    } else {
        return resolve_generated_stems(output_root);
    }

    Err(format!(
        "Stem generation failed. Install Demucs (`pip install demucs`) and FFmpeg. Details: {}",
        errors.join(" | ")
    ))
}

fn run_command(command: &str, args: &[String]) -> Result<(), String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn command: {e}"))?;
    map_command_output(output)
}

fn run_command_path(command: &Path, args: &[String]) -> Result<(), String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn command: {e}"))?;
    map_command_output(output)
}

fn map_command_output(output: std::process::Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let raw = if !stderr.trim().is_empty() {
        stderr
    } else if !stdout.trim().is_empty() {
        stdout
    } else {
        "no output".to_string()
    };
    let detail = summarize_command_output(&raw);
    Err(format!("exit code {:?}: {}", output.status.code(), detail))
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

fn resolve_generated_stems(output_root: &Path) -> Result<StemSeparationResult, String> {
    let mut stack = vec![output_root.to_path_buf()];
    let mut best: Option<(PathBuf, PathBuf, SystemTime)> = None;

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mut vocals: Option<PathBuf> = None;
        let mut instrumental: Option<PathBuf> = None;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if name.starts_with("vocals.") {
                vocals = Some(path);
            } else if name.starts_with("no_vocals.") {
                instrumental = Some(path);
            }
        }

        if let (Some(v), Some(i)) = (vocals, instrumental) {
            let mtime = fs::metadata(&v)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let replace = best.as_ref().map(|(_, _, t)| mtime > *t).unwrap_or(true);
            if replace {
                best = Some((v, i, mtime));
            }
        }
    }

    let Some((vocals_path, instrumental_path, _)) = best else {
        return Err(format!(
            "Demucs completed but no vocals/no_vocals outputs were found under {}",
            output_root.display()
        ));
    };

    let model_name = vocals_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "htdemucs".to_string());

    Ok(StemSeparationResult {
        model_name,
        vocals_path,
        instrumental_path,
    })
}
