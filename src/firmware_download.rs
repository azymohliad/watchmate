use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ReleaseInfo {
    pub name: String,
    #[serde(rename = "tag_name")]
    pub tag: String,
}

pub async fn list_releases() -> Result<Vec<ReleaseInfo>> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/repos/InfiniTimeOrg/InfiniTime/releases")
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "WatchMate")
        .send().await?;

    let status = response.status();
    if status.is_success() {
        let releases = response.json().await?;
        // dbg!(&releases);
        Ok(releases)
    } else {
        let text = response.text().await?;
        eprintln!("Request failed: {}\n{}", status, text);
        Err(anyhow!("Request failed: {}", status))
    }
}