use super::{uuids, InfiniTime};
use anyhow::Result;


pub enum Notification<'s> {
    // InfiniTime defines 10 categories, but at the time of writing only 2 of them
    // are implemented in the firmware: simple alert and call. It's not clear
    // whether others are intended to be implemented there later, so for now
    // we explicitly don't support them for the sake of simplicity
    Alert { title: &'s str, content: &'s str },
    Call { title: &'s str },
}

impl<'s> Notification<'s> {
    pub fn category(&self) -> u8 {
        match &self {
            Self::Alert { title: _, content: _ } => 0,
            Self::Call { title: _ } => 3,
        }
    }
}


impl InfiniTime {
    pub async fn write_notification<'s>(&self, notification: Notification<'s>) -> Result<()> {
        let header = &[notification.category(), 1];
        let message = match notification {
            Notification::Alert { title, content } => {
                [header, title.as_bytes(), content.as_bytes()].join(&0)
            }
            Notification::Call { title } => {
                [header, title.as_bytes()].join(&0)
            }
        };
        let characteristic = self.chr(&uuids::CHR_NEW_ALERT)?;
        Ok(characteristic.write(&message).await?)
    }
}