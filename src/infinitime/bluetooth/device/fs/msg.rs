use crate::inft::utils::value_enum;
use anyhow::{anyhow, Result};

// -- Commands and statuses --

value_enum! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Command::<u8> {
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
        MoveFileResp = 0x61
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


// -- Requests --



// -- Responses --

pub trait Response {
    fn status(&self) -> Status;
    
    fn check(&self) -> Result<()> {
        self.status().into_result()
    }
}

/// File read response
#[derive(Debug)]
pub struct ReadResponse<'s> {
    pub status: Status,
    pub offset: u32,
    pub total_size: u32,
    pub chunk_size: u32,
    pub data: &'s [u8],
}

impl<'s> Response for ReadResponse<'s> {
    fn status(&self) -> Status { self.status }
}

impl<'s> TryFrom<&'s [u8]> for ReadResponse<'s> {
    type Error = anyhow::Error;

    fn try_from(value: &'s [u8]) -> Result<Self> {
        if value.len() < 16 {
            Err(anyhow!("ReadResponse must be at least 16 bytes long"))
        } else if value[0] != Command::ReadResp as u8 {
            Err(anyhow!("Unexpected command: {:02x}", value[0] as u8))
        } else {            
            Ok(Self {
                status: (value[1] as i8).try_into()?,
                offset: u32::from_le_bytes(value[4..8].try_into()?),
                total_size: u32::from_le_bytes(value[8..12].try_into()?),
                chunk_size: u32::from_le_bytes(value[12..16].try_into()?),
                data: &value[16..],
            })
        }
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

impl Response for WriteResponse {
    fn status(&self) -> Status { self.status }
}

impl TryFrom<&[u8]> for WriteResponse {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() != 20 {
            Err(anyhow!("Array must be 20 bytes long"))
        } else if value[0] != Command::WriteResp as u8 {
            Err(anyhow!("Unexpected command: {:02x}", value[0]))
        } else {            
            Ok(Self {
                status: (value[1] as i8).try_into()?,
                offset: u32::from_le_bytes(value[4..8].try_into()?),
                timestamp: u64::from_le_bytes(value[8..16].try_into()?),
                remained: u32::from_le_bytes(value[16..20].try_into()?),
            })
        }
    }
}

/// Delete file response
#[derive(Debug)]
pub struct DeleteResponse(Status);

impl Response for DeleteResponse {
    fn status(&self) -> Status { self.0 }
}

impl TryFrom<&[u8]> for DeleteResponse {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() != 2 {
            Err(anyhow!("Array must be 20 bytes long"))
        } else if value[0] != Command::DeleteResp as u8 {
            Err(anyhow!("Unexpected command: {:02x}", value[0]))
        } else {            
            Ok(Self((value[1] as i8).try_into()?))
        }
    }
}