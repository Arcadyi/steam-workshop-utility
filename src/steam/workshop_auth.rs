use anyhow::{Context, Result};
use reqwest::Client;
use crate::steam::cookies::SteamCookies;

pub async fn subscribe(
    client: &Client,
    appid: &str,
    item_id: &str,
    cookies: &SteamCookies,
) -> Result<()> {
    let resp = client
        .post("https://steamcommunity.com/sharedfiles/subscribe")
        .header(
            "Cookie",
            format!(
                "steamLoginSecure={}; sessionid={}",
                cookies.login_secure, cookies.session_id
            ),
        )
        .header(
            "Referer",
            format!(
                "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
                item_id
            ),
        )
        .header("Origin", "https://steamcommunity.com")
        .form(&[
            ("id", item_id),
            ("appid", appid),
            ("sessionid", cookies.session_id.as_str()),
        ])
        .send()
        .await
        .context("Subscribe request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Subscribe failed with status: {}", resp.status());
    }

    Ok(())
}

pub async fn unsubscribe(
    client: &Client,
    appid: &str,
    item_id: &str,
    cookies: &SteamCookies,
) -> Result<()> {
    let resp = client
        .post("https://steamcommunity.com/sharedfiles/unsubscribe")
        .header(
            "Cookie",
            format!(
                "steamLoginSecure={}; sessionid={}",
                cookies.login_secure, cookies.session_id
            ),
        )
        .header(
            "Referer",
            format!(
                "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
                item_id
            ),
        )
        .header("Origin", "https://steamcommunity.com")
        .form(&[
            ("id", item_id),
            ("appid", appid),
            ("sessionid", cookies.session_id.as_str()),
        ])
        .send()
        .await
        .context("Unsubscribe request failed")?;

    if !resp.status().is_success() {
        anyhow::bail!("Unsubscribe failed with status: {}", resp.status());
    }

    Ok(())
}