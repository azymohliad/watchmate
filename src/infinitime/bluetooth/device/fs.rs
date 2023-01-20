use super::{InfiniTime, ProgressEvent, ProgressTx, report_progress};
use msg::{Response, Status};
use chrono::Utc;
use futures::{pin_mut, StreamExt};
use anyhow::{anyhow, Result};

mod msg;

const CHUNK_SIZE: u32 = 200;

#[derive(Debug)]
pub struct DirEntry {
    pub path: String,
    pub size: u32,
    pub is_dir: bool,
    pub timestamp: u64,
    pub entry_idx: u32,
    pub entries_total: u32,
}

impl<'s> From<&msg::ListDirResponse<'s>> for DirEntry {
    fn from(resp: &msg::ListDirResponse) -> Self {
        Self {
            path: String::from(resp.path),
            size: resp.size,
            is_dir: resp.flags & 0x1 != 0,
            timestamp: resp.timestamp,
            entry_idx: resp.entry_idx,
            entries_total: resp.entries_total,
        }
    }
}

pub fn parent(path: &str) -> Option<&str> {
    let (parent, _) = path.rsplit_once('/')?;
    if parent.is_empty() {
        None
    } else {
        Some(parent)
    }
}

pub fn ancestors(path: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut child = path;
    while let Some(parent) = parent(child) {
        result.push(parent);
        child = parent;
    }
    result
}

pub fn ancestors_union<'s>(paths: impl Iterator<Item=&'s str>) -> Vec<&'s str> {
    let mut result = Vec::new();
    for path in paths {
        result.append(&mut ancestors(path))
    }
    result.sort();
    result.dedup();
    result
}


impl InfiniTime {
    pub async fn read_fs_version(&self) -> Result<u16> {
        let data = self.chr_fs_version.read().await?;
        Ok(u16::from_le_bytes(data.as_slice().try_into()?))
    }

    pub async fn read_file(
        &self, path: &str, position: u32, progress_sender: Option<ProgressTx>
    ) -> Result<Vec<u8>> {
        log::info!("Reading file: {}", path);
        let resp_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(resp_stream);

        // Init
        let req = msg::read_init_req(path, position, CHUNK_SIZE);
        self.chr_fs_transfer.write(&req).await?;
        let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
        let parsed = msg::ReadResponse::deserialize_check(resp.as_slice())?;

        let total_size = parsed.total_size - position;
        let mut offset = position;
        let mut content = Vec::with_capacity(total_size as usize);
        content.extend_from_slice(parsed.data);
        offset += parsed.chunk_size;
        report_progress(&progress_sender, ProgressEvent::Progress {
            current: content.len() as u32, total: total_size
        }).await;

        // Read content
        while content.len() < total_size as usize {
            let req = msg::read_chunk_req(offset, CHUNK_SIZE);
            self.chr_fs_transfer.write(&req).await?;
            let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
            let parsed = msg::ReadResponse::deserialize_check(resp.as_slice())?;

            content.extend_from_slice(parsed.data);
            offset += parsed.chunk_size;
            report_progress(&progress_sender, ProgressEvent::Progress {
                current: content.len() as u32, total: total_size
            }).await;
        }

        Ok(content)
    }

    pub async fn write_file(
        &self, path: &str, content: &[u8], position: u32, progress_sender: Option<ProgressTx>
    ) -> Result<()> {
        log::info!("Writing file: {}", path);
        let resp_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(resp_stream);

        // Init
        let timestamp = Utc::now().timestamp_nanos() as u64;
        let req = msg::write_init_req(path, position, content.len() as u32, timestamp);
        self.chr_fs_transfer.write(&req).await?;
        let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
        msg::WriteResponse::deserialize_check(resp.as_slice())?;

        // Write content
        let mut offset = position;
        for chunk in content.chunks(CHUNK_SIZE as usize) {
            log::trace!("Sending file chunk: {} - {}", offset, offset + chunk.len() as u32);
            // Write chunk
            let req = msg::write_chunk_req(offset, chunk);
            self.chr_fs_transfer.write(&req).await?;
            let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
            msg::WriteResponse::deserialize_check(resp.as_slice())?;
            offset += chunk.len() as u32;
            report_progress(&progress_sender, ProgressEvent::Progress {
                current: offset - position, total: content.len() as u32
            }).await;
        }

        Ok(())
    }

    pub async fn delete_file(&self, path: &str) -> Result<()> {
        log::info!("Deleting file: {}", path);
        let resp_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(resp_stream);

        let req = msg::delete_req(path);
        self.chr_fs_transfer.write(&req).await?;
        let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
        msg::DeleteResponse::deserialize_check(resp.as_slice())?;
        Ok(())
    }

    pub async fn make_dir(&self, path: &str) -> Result<()> {
        log::info!("Making dir: {}", path);
        let resp_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(resp_stream);

        let timestamp = Utc::now().timestamp_nanos() as u64;
        let req = msg::make_dir_req(path, timestamp);
        self.chr_fs_transfer.write(&req).await?;
        let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
        let parsed = msg::MakeDirResponse::deserialize(resp.as_slice())?;
        if parsed.status != Status::Ok && parsed.status != Status::Exists {
            Err(anyhow!("LittleFS error: {:?}", parsed.status))
        } else {
            Ok(())
        }
    }

    pub async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        log::info!("Listing dir: {}", path);
        let resp_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(resp_stream);

        let req = msg::list_dir_req(path);
        self.chr_fs_transfer.write(&req).await?;

        let mut output = Vec::new();
        while let Some(resp) = resp_stream.next().await {
            let parsed = msg::ListDirResponse::deserialize_check(resp.as_slice())?;
            output.push(DirEntry::from(&parsed));
            if parsed.entry_idx >= parsed.entries_total - 1 {
                break;
            }
        }
        Ok(output)
    }

    pub async fn move_file(&self, old_path: &str, new_path: &str) -> Result<()> {
        log::info!("Move file or directory: {} -> {}", old_path, new_path);
        let resp_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(resp_stream);

        let req = msg::move_req(old_path, new_path);
        self.chr_fs_transfer.write(&req).await?;
        let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
        msg::MoveResp::deserialize_check(resp.as_slice())?;
        Ok(())
    }

    pub async fn make_dirs(&self, path: &str) -> Result<()> {
        for p in ancestors(path).iter().rev() {
            self.make_dir(p).await?;
        }
        Ok(())
    }
}
