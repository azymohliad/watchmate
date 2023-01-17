mod msg;

use msg::Response;
use super::InfiniTime;
use chrono::Utc;
use futures::{pin_mut, StreamExt};
use anyhow::{anyhow, Result};


const CHUNK_SIZE: u32 = 50;

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


impl InfiniTime {
    pub async fn read_fs_version(&self) -> Result<u16> {
        let data = self.chr_fs_version.read().await?;
        Ok(u16::from_le_bytes(data.as_slice().try_into()?))
    }

    pub async fn read_file(&self, path: &str, position: u32) -> Result<Vec<u8>> {
        log::info!("Reading file: {}", path);
        let resp_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(resp_stream);

        let mut offset = position;

        // Init
        let req = msg::read_init_req(path, offset, CHUNK_SIZE);
        self.chr_fs_transfer.write(&req).await?;
        let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
        let parsed = msg::ReadResponse::deserialize_check(resp.as_slice())?;

        let mut content = Vec::with_capacity(parsed.total_size as usize);
        content.extend_from_slice(parsed.data);
        offset += parsed.chunk_size;

        // Read content
        while content.len() < parsed.total_size as usize {
            let req = msg::read_chunk_req(offset, CHUNK_SIZE);
            self.chr_fs_transfer.write(&req).await?;
            let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
            let parsed = msg::ReadResponse::deserialize_check(resp.as_slice())?;

            content.extend_from_slice(parsed.data);
            offset += parsed.chunk_size;
        }

        Ok(content)
    }

    pub async fn write_file(&self, path: &str, content: &[u8], position: u32) -> Result<()> {
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
        let mut offset = 0u32;
        for chunk in content.chunks(CHUNK_SIZE as usize) {
            log::trace!("Sending file chunk: {} - {}", offset, chunk.len() as u32 + offset);
            // Write chunk
            let req = msg::write_chunk_req(offset, chunk);
            self.chr_fs_transfer.write(&req).await?;
            let resp = resp_stream.next().await.ok_or(anyhow!("No response"))?;
            msg::WriteResponse::deserialize_check(resp.as_slice())?;
            offset += chunk.len() as u32;
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
        msg::MakeDirResponse::deserialize_check(resp.as_slice())?;
        Ok(())
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
}
