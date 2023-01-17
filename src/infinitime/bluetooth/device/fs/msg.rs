use crate::inft::utils::value_enum;
use anyhow::{anyhow, Result};

// -- Commands and statuses --

value_enum! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Command::<u8> {
        ReadInit = 0x10,
        ReadResp = 0x11,
        ReadChunk = 0x12,
        WriteInit = 0x20,
        WriteResp = 0x21,
        WriteChunk = 0x22,
        Delete = 0x30,
        DeleteResp = 0x31,
        MakeDir = 0x40,
        MakeDirResp = 0x41,
        ListDir = 0x50,
        ListDirResp = 0x51,
        Move = 0x60,
        MoveResp = 0x61
    }
}

value_enum! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    #[allow(unused)]
    pub enum Status::<i8> {
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
        NameTooLong = -36
    }
}

impl Status {
    pub fn into_result(self) -> Result<()> {
        match self {
            Status::Ok => Ok(()),
            error => Err(anyhow!("LittleFS error: {:?}", error))
        }
    }
}


// -- Traits --

pub trait Response<'s>: Sized {
    fn status(&self) -> Status;

    fn deserialize(data: &'s [u8]) -> Result<Self>;

    fn check(&self) -> Result<()> {
        self.status().into_result()
    }

    fn deserialize_check(data: &'s [u8]) -> Result<Self> {
        let response = Self::deserialize(data)?;
        response.check()?;
        Ok(response)
    }
}


// -- Requests --

// TODO: Return Concat<u8> instead of Vec<u8> (when stable)?
// Does it copy its input data? If not, how can it be coerced to &[u8]?

pub fn read_init_req(path: &str, offset: u32, chunk_size: u32) -> Vec<u8> {
    let path = path.as_bytes();
    [
        [Command::ReadInit as u8, 0x00].as_slice(),
        &(path.len() as u16).to_le_bytes(),
        &offset.to_le_bytes(),
        &chunk_size.to_le_bytes(),
        path,
    ].concat().into()
}

pub fn read_chunk_req(offset: u32, chunk_size: u32) -> Vec<u8> {
    [
        [Command::ReadChunk as u8, Status::Ok as u8, 0x00, 0x00].as_slice(),
        &offset.to_le_bytes(),
        &chunk_size.to_le_bytes(),
    ].concat().into()
}

pub fn write_init_req(path: &str, position: u32, length: u32, timestamp: u64) -> Vec<u8> {
    let path = path.as_bytes();
    [
        [Command::WriteInit as u8, 0x00].as_slice(),
        &(path.len() as u16).to_le_bytes(),
        &position.to_le_bytes(),
        &timestamp.to_le_bytes(),
        &length.to_le_bytes(),
        path,
    ].concat().into()
}

pub fn write_chunk_req(offset: u32, chunk: &[u8]) -> Vec<u8> {
    // TODO: Avoid copying chunk
    [
        [Command::WriteChunk as u8, Status::Ok as u8, 0x00, 0x00].as_slice(),
        &offset.to_le_bytes(),
        &(chunk.len() as u32).to_le_bytes(),
        chunk,
    ].concat().into()
}

pub fn delete_req(path: &str) -> Vec<u8> {
    let path = path.as_bytes();
    [
        [Command::Delete as u8, 0x00].as_slice(),
        &(path.len() as u16).to_le_bytes(),
        path,
    ].concat().into()
}

pub fn make_dir_req(path: &str, timestamp: u64) -> Vec<u8> {
    let path = path.as_bytes();
    [
        [Command::MakeDir as u8, 0x00].as_slice(),
        &(path.len() as u16).to_le_bytes(),
        [0x00; 4].as_slice(),
        &timestamp.to_le_bytes(),
        path,
    ].concat().into()
}

pub fn list_dir_req(path: &str) -> Vec<u8> {
    let path = path.as_bytes();
    [
        [Command::ListDir as u8, 0x00].as_slice(),
        &(path.len() as u16).to_le_bytes(),
        path,
    ].concat().into()
}

pub fn move_req(old_path: &str, new_path: &str) -> Vec<u8> {
    let old_path = old_path.as_bytes();
    let new_path = new_path.as_bytes();
    [
        [Command::Move as u8, 0x00].as_slice(),
        &(old_path.len() as u16).to_le_bytes(),
        &(new_path.len() as u16).to_le_bytes(),
        old_path,
        [0x00].as_slice(),
        new_path,
    ].concat().into()
}


// -- Responses --

/// File read response
#[derive(Debug)]
pub struct ReadResponse<'s> {
    pub status: Status,
    pub offset: u32,
    pub total_size: u32,
    pub chunk_size: u32,
    pub data: &'s [u8],
}

impl<'s> Response<'s> for ReadResponse<'s> {
    fn status(&self) -> Status { self.status }

    fn deserialize(data: &'s [u8]) -> Result<Self> {
        response_data_check(data, 16, Command::ReadResp)?;
        Ok(Self {
            status: (data[1] as i8).try_into()?,
            offset: u32::from_le_bytes(data[4..8].try_into()?),
            total_size: u32::from_le_bytes(data[8..12].try_into()?),
            chunk_size: u32::from_le_bytes(data[12..16].try_into()?),
            data: &data[16..],
        })
    }
}

/// File write response
#[derive(Debug)]
pub struct WriteResponse {
    pub status: Status,
    pub offset: u32,
    pub timestamp: u64,
    pub remained: u32,
}

impl<'s> Response<'s> for WriteResponse {
    fn status(&self) -> Status { self.status }

    fn deserialize(data: &[u8]) -> Result<Self> {
        response_data_check(data, 20, Command::WriteResp)?;
        Ok(Self {
            status: (data[1] as i8).try_into()?,
            offset: u32::from_le_bytes(data[4..8].try_into()?),
            timestamp: u64::from_le_bytes(data[8..16].try_into()?),
            remained: u32::from_le_bytes(data[16..20].try_into()?),
        })
    }
}

/// Delete file response
#[derive(Debug)]
pub struct DeleteResponse(Status);

impl<'s> Response<'s> for DeleteResponse {
    fn status(&self) -> Status { self.0 }

    fn deserialize(data: &[u8]) -> Result<Self> {
        response_data_check(data, 2, Command::DeleteResp)?;
        Ok(Self((data[1] as i8).try_into()?))
    }
}

/// Make directory response
#[derive(Debug)]
pub struct MakeDirResponse {
    pub status: Status,
    pub timestamp: u64,
}

impl<'s> Response<'s> for MakeDirResponse {
    fn status(&self) -> Status { self.status }

    fn deserialize(data: &[u8]) -> Result<Self> {
        response_data_check(data, 16, Command::MakeDirResp)?;
        Ok(Self {
            status: (data[1] as i8).try_into()?,
            timestamp: u64::from_le_bytes(data[8..16].try_into()?),
        })
    }
}


/// List directory response
#[derive(Debug)]
pub struct ListDirResponse<'s> {
    pub status: Status,
    pub entry_idx: u32,
    pub entries_total: u32,
    pub flags: u32,
    pub timestamp: u64,
    pub size: u32,
    pub path: &'s str,
}

impl<'s> Response<'s> for ListDirResponse<'s> {
    fn status(&self) -> Status { self.status }

    fn deserialize(data: &'s [u8]) -> Result<Self> {
        response_data_check(data, 20, Command::ListDirResp)?;
        let path_length = u16::from_le_bytes(data[2..4].try_into()?) as usize;
        Ok(Self {
            status: (data[1] as i8).try_into()?,
            entry_idx: u32::from_le_bytes(data[4..8].try_into()?),
            entries_total: u32::from_le_bytes(data[8..12].try_into()?),
            flags: u32::from_le_bytes(data[12..16].try_into()?),
            timestamp: u64::from_le_bytes(data[16..24].try_into()?),
            size: u32::from_le_bytes(data[24..28].try_into()?),
            path: std::str::from_utf8(&data[28..(28 + path_length)])?,
        })
    }
}

/// Move file/dir response
#[derive(Debug)]
pub struct MoveResp(Status);

impl<'s> Response<'s> for MoveResp {
    fn status(&self) -> Status { self.0 }

    fn deserialize(data: &[u8]) -> Result<Self> {
        response_data_check(data, 2, Command::MoveResp)?;
        Ok(Self((data[1] as i8).try_into()?))
    }
}


fn response_data_check(data: &[u8], min_size: usize, exp_cmd: Command) -> Result<()> {
    if data.len() < min_size {
        Err(anyhow!("Unexpected response length: {} < {}", data.len(), min_size))
    } else if data[0] != exp_cmd as u8 {
        Err(anyhow!("Unexpected command: {:02x} != {:?}", data[0], exp_cmd))
    } else {
        Ok(())
    }
}