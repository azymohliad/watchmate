use super::InfiniTime;
use chrono::Utc;
use futures::{pin_mut, StreamExt};
use anyhow::{anyhow, Result};


const CHUNK_SIZE: u32 = 200;


enum Command {
    ReadInit = 0x10,
    ReadResp = 0x11,
    ReadContinue = 0x12,
    WriteInit = 0x20,
    WriteResp = 0x21,
    WriteContinue = 0x22,
    Delete = 0x30,
    DeleteResp = 0x31,
    MakeDir = 0x40,
    MakeDirResp = 0x41,
    ListDir = 0x50,
    ListDirResp = 0x51,
    MoveFile = 0x60,
    MoveFileResp = 0x61,
}

#[allow(unused)]
enum Status {
    Ok = 1,
    IoError = -5,
    Corrupted = -84,
    NoDirectoryEntry = -2,
    Exists = -17,
    NotDir = -20,
    IsDir = -21,
    NotEmpty = -39,
    BadNumber = -9,
    FileTooLarge = -27,
    InvalidParam = -22,
    NoSpaceLeft = -28,
    NoMemory = -12,
    NoAttribute = -61,
    NameTooLong = -36,
}


#[derive(Debug)]
struct WriteResponse {
    command: u8,
    status: i8,
    offset: u32,
    timestamp: u64,
    remained: u32,
}

impl WriteResponse {
    fn check(&self) -> Result<()> {
        if self.command != Command::WriteResp as u8 {
            Err(anyhow!("Unexpected command: {:02x}", self.command))
        } else if self.status != Status::Ok as i8 {
            Err(anyhow!("LittleFS error: {}", self.status))
        } else {
            Ok(())
        }
    }
}

impl TryFrom<&[u8]> for WriteResponse {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() != 20 {
            Err(anyhow!("Array must be 20 bytes long"))
        } else {
            Ok(Self {
                command: value[0],
                status: value[1] as i8,
                offset: u32::from_le_bytes(value[4..8].try_into()?),
                timestamp: u64::from_le_bytes(value[8..16].try_into()?),
                remained: u32::from_le_bytes(value[16..20].try_into()?),
            })
        }
    }
}


#[derive(Debug)]
struct ReadResponse<'s> {
    command: u8,
    status: i8,
    offset: u32,
    total_size: u32,
    chunk_size: u32,
    data: &'s [u8],
}

impl<'s> ReadResponse<'s> {
    fn check(&self) -> Result<()> {
        if self.command != Command::ReadResp as u8 {
            Err(anyhow!("Unexpected command: {:02x}", self.command))
        } else if self.status != Status::Ok as i8 {
            Err(anyhow!("LittleFS error: {}", self.status))
        } else {
            Ok(())
        }
    }
}

impl<'s> TryFrom<&'s [u8]> for ReadResponse<'s> {
    type Error = anyhow::Error;

    fn try_from(value: &'s [u8]) -> Result<Self> {
        if value.len() < 16 {
            Err(anyhow!("ReadResponse must be at least 16 bytes long"))
        } else {
            Ok(Self {
                command: value[0],
                status: value[1] as i8,
                offset: u32::from_le_bytes(value[4..8].try_into()?),
                total_size: u32::from_le_bytes(value[8..12].try_into()?),
                chunk_size: u32::from_le_bytes(value[12..16].try_into()?),
                data: &value[16..],
            })
        }
    }
}


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
            [Command::ReadInit as u8, 0x00].as_slice(),
            &(path.as_bytes().len() as u16).to_le_bytes(),
            &offset.to_le_bytes(),
            &CHUNK_SIZE.to_le_bytes(),
            path.as_bytes()
        ].concat();
        self.chr_fs_transfer.write(&header).await?;

        let resp_data = response_stream.next().await
            .ok_or(anyhow!("Response stream ended"))?;
        let resp = ReadResponse::try_from(resp_data.as_slice())?;
        resp.check()?;

        let mut content = Vec::with_capacity(resp.total_size as usize);
        log::trace!("Read chunk: {} - {}", offset, offset + resp.chunk_size);
        content.extend_from_slice(resp.data);
        offset += resp.chunk_size;

        // Read content
        while content.len() < resp.total_size as usize {
            let packet = [
                [Command::ReadContinue as u8, Status::Ok  as u8, 0x00, 0x00],
                offset.to_le_bytes(),
                CHUNK_SIZE.to_le_bytes(),
            ].concat();
            self.chr_fs_transfer.write(&packet).await?;

            let resp_data = response_stream.next().await
                .ok_or(anyhow!("Response stream ended"))?;
            let resp = ReadResponse::try_from(resp_data.as_slice())?;
            resp.check()?;

            log::trace!("Read chunk: {} - {}", offset, offset + resp.chunk_size);
            content.extend_from_slice(resp.data);
            offset += resp.chunk_size;
        }

        Ok(content)
    }

    pub async fn write_file(&self, path: &str, content: &[u8], position: u32) -> Result<()> {
        let response_stream = self.chr_fs_transfer.notify().await?;
        pin_mut!(response_stream);

        // Init
        log::trace!("Sending file write header");
        let header = [
            [Command::WriteInit as u8, 0x00].as_slice(),
            &(path.as_bytes().len() as u16).to_le_bytes(),
            &position.to_le_bytes(),
            &(Utc::now().timestamp_nanos() as u64).to_le_bytes(),
            &(content.len() as u32).to_le_bytes(),
            path.as_bytes()
        ].concat();
        self.chr_fs_transfer.write(&header).await?;

        // Process init resonse
        let resp_data = response_stream.next().await
            .ok_or(anyhow!("Response stream ended"))?;
        let mut resp = WriteResponse::try_from(resp_data.as_slice())?;
        resp.check()?;


        // Write content
        let mut offset = 0u32;
        for chunk in content.chunks(CHUNK_SIZE as usize) {
            log::trace!("Sending file chunk: {} - {}", offset, chunk.len() as u32 + offset);
            // Write chunk
            let packet = [
                [Command::WriteContinue as u8, Status::Ok as u8, 0x00, 0x00].as_slice(),
                &offset.to_le_bytes(),
                &(chunk.len() as u32).to_le_bytes(),
                chunk
            ].concat();
            offset += chunk.len() as u32;
            self.chr_fs_transfer.write(&packet).await?;

            // Process response
            let resp_data = response_stream.next().await
                .ok_or(anyhow!("Response stream ended"))?;
            resp = WriteResponse::try_from(resp_data.as_slice())?;
            resp.check()?;
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
