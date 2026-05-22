use std::collections::HashMap;

use anyhow::{Context, Result};
use cosmic::widget;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GetPublishedFileDetailsResponse {
    response: PublishedFileDetailsResponse,
}

#[derive(Debug, Deserialize)]
struct PublishedFileDetailsResponse {
    publishedfiledetails: Vec<PublishedFileDetailsEntry>,
}

#[derive(Debug, Deserialize)]
struct PublishedFileTag {
    tag: String,
}

#[derive(Debug, Deserialize)]
struct PublishedFileDetailsEntry {
    publishedfileid: Option<String>,
    title: Option<String>,
    time_updated: Option<u64>,
    incompatible: Option<bool>,
    tags: Option<Vec<PublishedFileTag>>,
    preview_url: Option<String>,
    hcontent_preview: Option<String>,
}

pub struct WorkshopMetadata {
    pub title: Option<String>,
    pub time_updated: Option<u64>,
    pub incompatible: bool,
    pub tags: Vec<String>,
    pub preview_url: Option<String>,
}

pub async fn fetch_workshop_metadata_batch(
    client: &reqwest::Client,
    ids: &[String],
) -> Result<HashMap<String, WorkshopMetadata>> {
    let mut form: Vec<(String, String)> = Vec::new();
    form.push(("itemcount".into(), ids.len().to_string()));

    for (i, id) in ids.iter().enumerate() {
        form.push((format!("publishedfileids[{i}]"), id.clone()));
    }

    form.push(("format".into(), "json".into()));

    let resp: GetPublishedFileDetailsResponse = client
        .post("https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/")
        .form(&form)
        .send()
        .await
        .context("Steam API request failed")?
        .error_for_status()
        .context("Steam API returned error status")?
        .json()
        .await
        .context("Failed to decode Steam API response")?;

    let mut map = HashMap::new();
    for item in resp.response.publishedfiledetails {
        if let Some(id) = item.publishedfileid {
            // preview_url is not always present in ISteamRemoteStorage/GetPublishedFileDetails.
            let resolved_preview_url = item
                .preview_url
                .filter(|s| !s.is_empty()) // treat empty string as absent
                .or_else(|| {
                    item.hcontent_preview
                        .filter(|s| !s.is_empty())
                        .map(|h| format!("https://steamuserimages-a.akamaihd.net/ugc/{}/", h))
                });
            map.insert(
                id,
                WorkshopMetadata {
                    title: item.title,
                    time_updated: item.time_updated,
                    incompatible: item.incompatible.unwrap_or(false),
                    tags: item
                        .tags
                        .unwrap_or_default()
                        .into_iter()
                        .map(|t| t.tag)
                        .collect(),
                    preview_url: resolved_preview_url,
                },
            );
        }
    }

    Ok(map)
}

pub async fn open_uri(uri: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        let uri_w: Vec<u16> = uri.encode_utf16().chain(Some(0)).collect();

        let result = unsafe {
            ShellExecuteW(
                0,
                verb.as_ptr(),
                uri_w.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };

        if result <= 32 {
            return Err(format!("ShellExecuteW failed: {}", result));
        }

        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        open::that_detached(uri).map_err(|e| e.to_string())
    }
}

pub async fn try_fetch_image(client: &reqwest::Client, url: &str) -> Option<widget::image::Handle> {
    let resp = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .ok()?;
    let bytes = resp.bytes().await.ok()?;
    Some(widget::image::Handle::from_bytes(bytes.to_vec()))
}
