use anyhow::Context;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub fn folder_size(path: &Path) -> anyhow::Result<u64> {
    let mut total = 0u64;

    if !path.exists() {
        return Ok(0);
    }

    for entry in std::fs::read_dir(path)
        .with_context(|| format!("Could not read directory {}", path.display()))?
    {
        let entry = entry?;
        let entry_path = entry.path();
        let metadata = entry.metadata()?;

        if metadata.is_file() {
            total += metadata.len();
        } else if metadata.is_dir() {
            total += folder_size(&entry_path)?;
        }
    }

    Ok(total)
}

pub fn modified_secs(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

pub fn newest_file_timestamp(path: &Path) -> Option<u64> {
    let mut newest = None;

    for entry in std::fs::read_dir(path).ok()? {
        let Ok(entry) = entry else { continue };
        let entry_path = entry.path();

        let ts = if entry_path.is_dir() {
            newest_file_timestamp(&entry_path)
        } else {
            modified_secs(&entry_path)
        };

        newest = match (newest, ts) {
            (None, ts) => ts,
            (Some(a), Some(b)) => Some(a.max(b)),
            (some, None) => some,
        };
    }

    newest
}

pub fn format_size(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

pub fn format_timestamp(ts: Option<u64>) -> String {
    match ts {
        None => "Unknown".to_string(),
        Some(secs) => {
            use chrono::{DateTime, Utc};
            let dt = DateTime::<Utc>::from_timestamp(secs as i64, 0).unwrap_or_default();
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
    }
}
