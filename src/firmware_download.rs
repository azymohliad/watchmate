use std::{env, path::{Path, PathBuf}};
use tokio::{fs::File, io::AsyncWriteExt};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use reqwest::IntoUrl;

#[derive(Deserialize, Debug)]
pub struct ReleaseInfo {
    pub name: String,
    #[serde(rename = "tag_name")]
    pub tag: String,
    #[serde(rename = "html_url")]
    pub url: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Debug)]
pub struct Asset {
    pub name: String,
    pub url: String,
    #[serde(rename = "browser_download_url")]
    pub direct_url: String,
    pub content_type: String,
    pub size: u32,
}

impl ReleaseInfo {
    pub fn get_dfu_asset(&self) -> Option<&Asset> {
        self.assets.iter().find(|a| {
            a.name.starts_with("pinetime-mcuboot-app-dfu") &&
            a.name.ends_with(".zip")
        })
    }
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

pub async fn download_dfu_content(url: impl IntoUrl) -> Result<Vec<u8>>
{
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", "WatchMate")
        .send().await?;

    let status = response.status();
    if status.is_success() {
        let content = response.bytes().await?;
        Ok(content.to_vec())
    } else {
        let text = response.text().await?;
        eprintln!("Request failed: {}\n{}", status, text);
        Err(anyhow!("Request failed: {}", status))
    }
}

pub async fn download_dfu_file(url: impl IntoUrl, filepath: impl AsRef<Path>) -> Result<()> {
    let content = download_dfu_content(url).await?;
    let mut file = File::create(&filepath).await?;
    file.write_all(&content).await?;
    Ok(())
}


pub fn get_download_dir() -> Result<PathBuf> {
    match env::var("XDG_DOWNLOAD_DIR") {
        Ok(value) => Ok(PathBuf::from(value)),
        Err(_) => Ok(Path::new(&env::var("HOME")?).join("Downloads")),
    }
}

pub fn get_download_filepath(filename: impl AsRef<Path>) -> Result<PathBuf> {
    Ok(get_download_dir()?.join(&filename))
}