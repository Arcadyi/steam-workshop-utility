use crate::steam::api::{WorkshopMetadata, fetch_workshop_metadata_batch};
use anyhow::{Context, Result};
use keyvalues_parser::Vdf;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::task;
use crate::steam::types::{GameEntry, ItemStatus, WorkshopItem};
use crate::utils::general::{folder_size, newest_file_timestamp};

// Collects all SteamApps library folders
pub fn get_steam_install_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let candidates = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];

    #[cfg(target_os = "linux")]
    let candidates = {
        // Steam can live in several places depending on how it was installed
        // (native package, Flatpak symlink, etc.)
        let home = dirs::home_dir().context("Could not determine home directory")?;
        vec![
            home.join(".steam/steam"),       // most common symlink
            home.join(".local/share/Steam"), // actual location for many installs
        ]
    };

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let candidates: Vec<PathBuf> = vec![];

    candidates
        .into_iter()
        .find(|p| p.exists())
        .context("Could not find Steam installation. Is Steam installed?")
}

fn extract_library_paths(contents: &str) -> Vec<PathBuf> {
    let Ok(vdf) = keyvalues_parser::parse(contents).map(Vdf::from) else {
        eprintln!("Warning: could not parse libraryfolders.vdf");
        return vec![];
    };

    let Some(libraries) = vdf.value.get_obj() else {
        return vec![];
    };

    libraries
        .values()
        .filter_map(|entries| {
            let obj = entries.first()?.get_obj()?;
            let path_val = obj.get("path")?.first()?;
            let path_str = path_val.get_str()?;
            Some(PathBuf::from(path_str).join("steamapps"))
        })
        .collect()
}

pub fn get_library_folders() -> Result<Vec<PathBuf>> {
    let steam_dir = get_steam_install_dir()?;
    let vdf_path = steam_dir.join("steamapps/libraryfolders.vdf");

    let contents = std::fs::read_to_string(&vdf_path)
        .with_context(|| format!("Could not read {}", vdf_path.display()))?;

    let mut seen = HashSet::new();
    let mut folders = vec![];

    let default = steam_dir.join("steamapps");
    let default = default.canonicalize().unwrap_or(default);
    seen.insert(default.clone());
    folders.push(default);

    for path in extract_library_paths(&contents) {
        let path = path.canonicalize().unwrap_or(path.clone());
        if seen.insert(path.clone()) {
            folders.push(path);
        }
    }

    Ok(folders)
}

fn parse_app_manifest(contents: &str, library: &Path) -> Option<GameEntry> {
    let vdf = keyvalues_parser::parse(contents)
        .map(Vdf::from)
        .map_err(|e| eprintln!("Warning: could not parse app manifest: {e}"))
        .ok()?;
    let obj = vdf.value.get_obj()?;
    let get =
        |key: &str| -> Option<String> { obj.get(key)?.first()?.get_str().map(|s| s.to_string()) };
    let appid = get("appid")?;
    let name = get("name");
    let install_dir = get("installdir")?;
    let build_id = get("buildid");

    Some(GameEntry {
        appid,
        name,
        path: library
            .join("common")
            .join(install_dir)
            .to_string_lossy()
            .to_string(),
        build_id,
    })
}

pub fn get_games() -> Result<Vec<GameEntry>> {
    let libraries = get_library_folders()?;
    let mut games = Vec::new();
    let mut seen_appids = std::collections::HashSet::new();

    for library in libraries {
        let entries = match std::fs::read_dir(&library) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };

            if !filename.starts_with("appmanifest_") || !filename.ends_with(".acf") {
                continue;
            }

            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Could not read {}", path.display()))?;

            match parse_app_manifest(&contents, &library) {
                Some(game) => {
                    if seen_appids.insert(game.appid.clone()) {
                        games.push(game);
                    }
                }
                None => eprintln!("Warning: could not parse app manifest: {}", path.display()),
            }
        }
    }

    Ok(games)
}

pub fn get_workshop_entries(game_entry: &GameEntry) -> Result<Vec<WorkshopItem>> {
    let game_path = Path::new(&game_entry.path);

    // game path is expected to be: .../steamapps/common/<game>
    let steam_apps = game_path
        .parent()
        .and_then(|p| p.parent())
        .context("Game path is not under steamapps/common")?;

    let workshop_root = steam_apps
        .join("workshop")
        .join("content")
        .join(&game_entry.appid);

    if !workshop_root.exists() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();

    for entry in std::fs::read_dir(&workshop_root)
        .with_context(|| format!("Could not read {}", workshop_root.display()))?
    {
        let entry = entry?;
        let item_path = entry.path();

        if !item_path.is_dir() {
            continue;
        }

        let disk_size = folder_size(&item_path)?;

        let item_id = item_path
            .file_name()
            .and_then(|s| s.to_str())
            .with_context(|| {
                format!(
                    "Workshop item folder name was not valid UTF-8: {}",
                    item_path.display()
                )
            })?
            .to_string();

        items.push(WorkshopItem {
            item_id,
            name: None,
            path: item_path.clone(),
            local_timestamp: newest_file_timestamp(&item_path),
            remote_timestamp: None,
            disk_size,
            status: ItemStatus::Unknown,
            incompatible: false,
            supported_versions: Vec::new(),
            preview_url: None,
            selected: false,
        });
    }

    items.sort_by_key(|item| item.path.clone());

    Ok(items)
}

pub async fn enrich_workshop_items_for_game(
    client: &reqwest::Client,
    items: Vec<WorkshopItem>,
) -> Result<Vec<WorkshopItem>> {
    if items.is_empty() {
        return Ok(items);
    }
    let ids: Vec<String> = items.iter().map(|item| item.item_id.clone()).collect();
    const BATCH_SIZE: usize = 100;

    let batch_futures: Vec<_> = ids
        .chunks(BATCH_SIZE)
        .map(|batch| {
            let client = client.clone();
            let batch_ids: Vec<String> = batch.to_vec();
            task::spawn(async move { fetch_workshop_metadata_batch(&client, &batch_ids).await })
        })
        .collect();

    let mut metadata_map: HashMap<String, WorkshopMetadata> = HashMap::new();
    for fut in batch_futures {
        let batch_map = fut.await.context("Batch task panicked")??;
        metadata_map.extend(batch_map);
    }

    let mut out = Vec::with_capacity(items.len());
    for mut item in items {
        if let Some(metadata) = metadata_map.get(&item.item_id) {
            if item.name.is_none() {
                item.name = metadata.title.clone();
            }
            item.remote_timestamp = metadata.time_updated;
            item.incompatible = metadata.incompatible;
            item.preview_url = metadata.preview_url.clone();
            item.supported_versions = metadata
                .tags
                .iter()
                .filter(|t| {
                    // Must start with a digit
                    let starts_with_digit = t
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false);
                    // Must contain a dot (e.g. "1.49", "2.6.1") to exclude "1920", "2024" etc.
                    let has_dot = t.contains('.');
                    // Must be reasonably short to exclude long strings
                    let is_short = t.len() <= 12;
                    starts_with_digit && has_dot && is_short
                })
                .cloned()
                .collect();
        }
        item.status = match (item.local_timestamp, item.remote_timestamp) {
            (_, None) => ItemStatus::Unknown,
            (None, _) => ItemStatus::Unknown,
            (Some(local), Some(remote)) if local >= remote => ItemStatus::UpToDate,
            _ => ItemStatus::OutOfDate,
        };
        out.push(item);
    }
    Ok(out)
}

pub fn find_acf_path(game_entry: &GameEntry) -> Result<PathBuf> {
    let game_path = Path::new(&game_entry.path);

    let steam_apps = game_path
        .parent()
        .and_then(|p| p.parent())
        .context("Game path is not under steamapps/common")?;

    let acf_path = steam_apps
        .join("workshop")
        .join(format!("appworkshop_{}.acf", game_entry.appid));

    if acf_path.exists() {
        Ok(acf_path)
    } else {
        anyhow::bail!("Could not find appworkshop_{}.acf", game_entry.appid)
    }
}

pub fn zero_acf_entries(acf_path: &Path, item_ids: &[String]) -> Result<()> {
    let contents = std::fs::read_to_string(acf_path)
        .with_context(|| format!("Could not read {}", acf_path.display()))?;

    // Write backup before touching anything
    let backup_path = acf_path.with_extension("acf.bak");
    std::fs::write(&backup_path, &contents)
        .with_context(|| format!("Could not write backup to {}", backup_path.display()))?;

    let mut output = String::with_capacity(contents.len());
    let mut in_workshop_items = false;
    let mut current_item_id: Option<String> = None;
    let mut depth: usize = 0;
    let mut should_zero = false;

    for line in contents.lines() {
        let trimmed = line.trim();

        // Detect entry into the WorkshopItemsInstalled / WorkshopItemDetails blocks
        if trimmed == "\"WorkshopItemsInstalled\"" || trimmed == "\"WorkshopItemDetails\"" {
            in_workshop_items = true;
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if in_workshop_items {
            if trimmed == "{" {
                depth += 1;
                // depth 1 = outer block, depth 2 = item id block
                if depth == 2 {
                    should_zero = current_item_id
                        .as_deref()
                        .map(|id| item_ids.contains(&id.to_string()))
                        .unwrap_or(false);
                }
                output.push_str(line);
                output.push('\n');
                continue;
            }

            if trimmed == "}" {
                if depth == 1 {
                    in_workshop_items = false;
                    current_item_id = None;
                    should_zero = false;
                }
                depth = depth.saturating_sub(1);
                output.push_str(line);
                output.push('\n');
                continue;
            }

            // Capture item IDs at depth 1 (they're the keys of the map)
            if depth == 1 && trimmed.starts_with('"') {
                let parts: Vec<&str> = trimmed.split('"').collect();
                if parts.len() >= 2 {
                    current_item_id = Some(parts[1].to_string());
                }
            }

            // Zero out timeupdated and size for selected items
            if should_zero && depth == 2 {
                let parts: Vec<&str> = trimmed.split('"').collect();
                if parts.len() >= 4 {
                    let key = parts[1];
                    if key == "timeupdated" || key == "size" {
                        // Rebuild the line with value zeroed
                        let leading_whitespace = &line[..line.len() - line.trim_start().len()];
                        output.push_str(&format!("{}\"{}\"\t\t\"0\"\n", leading_whitespace, key));
                        continue;
                    }
                }
            }
        }

        output.push_str(line);
        output.push('\n');
    }

    std::fs::write(acf_path, output)
        .with_context(|| format!("Could not write patched ACF to {}", acf_path.display()))?;

    Ok(())
}

pub fn get_logo_path_from_appinfo(appid: &str) -> Option<String> {
    let appinfo_path = dirs::home_dir()?
        .join(".steam/steam/appcache/appinfo.vdf");

    let data = std::fs::read(&appinfo_path).ok()?;
    let appid_num: u32 = appid.parse().ok()?;

    // The binary format has a marker before each app block.
    // Search for the appid as a little-endian u32, then scan for the logo key.
    // We look for the string "library_logo" followed eventually by "logo.png" path.

    // Find appid bytes in the file
    let id_bytes = appid_num.to_le_bytes();

    let mut pos = 0;
    while pos + 4 < data.len() {
        if data[pos..pos + 4] == id_bytes {
            // Scan forward up to 64KB for "library_logo"
            let search_end = (pos + 65536).min(data.len());
            let chunk = &data[pos..search_end];

            if let Some(logo_offset) = find_bytes(chunk, b"library_logo") {
                // After "library_logo\0" key, scan for "logo.png" path
                let after_key = logo_offset + b"library_logo".len() + 1;
                if let Some(path_offset) = find_bytes(&chunk[after_key..], b"logo.png") {
                    // Walk back to find the start of the hash string (null-terminated string before logo.png)
                    let abs = after_key + path_offset;
                    // Find the preceding null byte to get string start
                    let start = chunk[..abs]
                        .iter()
                        .rposition(|&b| b == 0)
                        .map(|i| i + 1)
                        .unwrap_or(abs);
                    let path = std::str::from_utf8(&chunk[start..abs + "logo.png".len()]).ok()?;
                    return Some(path.to_string());
                }
            }
        }
        pos += 1;
    }

    None
}

pub fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
