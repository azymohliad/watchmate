mod msg;

use msg::Response;
use super::InfiniTime;
use chrono::Utc;
use futures::{pin_mut, StreamExt};
use anyhow::{anyhow, Result};


const CHUNK_SIZE: u32 = 200;


impl InfiniTime {
    pub async fn read_fs_version(&self) -> Result<u16> {
        let data = self.chr_fs_version.read().await?;
        Ok(u16::from_le_bytes(data.as_slice().try_into()?))
    }

    pub async fn read_file(&self, path: &str, position: u32) -> Result<Vec<u8>> {
        let response_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(response_stream);

        let mut offset = position;

        // Init
        log::trace!("Sending file read header");
        let header = [
            [msg::Command::ReadInit as u8, 0x00].as_slice(),
            &(path.as_bytes().len() as u16).to_le_bytes(),
            &offset.to_le_bytes(),
            &CHUNK_SIZE.to_le_bytes(),
            path.as_bytes()
        ].concat();
        self.chr_fs_transfer.write(&header).await?;

        let data = response_stream.next().await.ok_or(anyhow!("No response"))?;
        let response = msg::ReadResponse::try_from(data.as_slice())?;
        response.check()?;

        let mut content = Vec::with_capacity(response.total_size as usize);
        
        log::trace!("Read chunk: {} - {}", offset, offset + response.chunk_size);
        content.extend_from_slice(response.data);
        offset += response.chunk_size;

        // Read content
        while content.len() < response.total_size as usize {
            let packet = [
                [msg::Command::ReadContinue as u8, msg::Status::Ok  as u8, 0x00, 0x00],
                offset.to_le_bytes(),
                CHUNK_SIZE.to_le_bytes(),
            ].concat();
            self.chr_fs_transfer.write(&packet).await?;

            let data = response_stream.next().await.ok_or(anyhow!("No response"))?;
            let response = msg::ReadResponse::try_from(data.as_slice())?;
            response.check()?;

            log::trace!("Read chunk: {} - {}", offset, offset + response.chunk_size);
            content.extend_from_slice(response.data);
            offset += response.chunk_size;
        }

        Ok(content)
    }

    pub async fn write_file(&self, path: &str, content: &[u8], position: u32) -> Result<()> {
        let response_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(response_stream);

        // Init
        log::trace!("Sending file write header");
        let header = [
            [msg::Command::WriteInit as u8, 0x00].as_slice(),
            &(path.as_bytes().len() as u16).to_le_bytes(),
            &position.to_le_bytes(),
            &(Utc::now().timestamp_nanos() as u64).to_le_bytes(),
            &(content.len() as u32).to_le_bytes(),
            path.as_bytes()
        ].concat();
        self.chr_fs_transfer.write(&header).await?;

        // Process init resonse
        let data = response_stream.next().await.ok_or(anyhow!("No response"))?;
        let response = msg::WriteResponse::try_from(data.as_slice())?;
        response.check()?;

        // Write content
        let mut offset = 0u32;
        for chunk in content.chunks(CHUNK_SIZE as usize) {
            log::trace!("Sending file chunk: {} - {}", offset, chunk.len() as u32 + offset);
            // Write chunk
            let packet = [
                [msg::Command::WriteContinue as u8, msg::Status::Ok as u8, 0x00, 0x00].as_slice(),
                &offset.to_le_bytes(),
                &(chunk.len() as u32).to_le_bytes(),
                chunk
            ].concat();
            offset += chunk.len() as u32;
            self.chr_fs_transfer.write(&packet).await?;

            // Process response
            let data = response_stream.next().await.ok_or(anyhow!("No response"))?;
            let response = msg::WriteResponse::try_from(data.as_slice())?;
            response.check()?;
        }

        log::debug!("File written to PineTime: {} ({} B)", path, content.len());
        Ok(())
    }

    pub async fn delete_file(&self, path: &str) -> Result<()> {
        Ok(())
    }

    pub async fn move_file(&self, old_path: &str, new_path: &str) -> Result<()> {
        Ok(())
    }

    pub async fn make_dir(&self, path: &str) -> Result<()> {
        Ok(())
    }

    pub async fn list_dir(&self, path: &str) -> Result<()> {
        Ok(())
    }
}
