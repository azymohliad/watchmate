use bluer::{gatt::remote::Characteristic, Adapter, AdapterEvent, Device, Result, Session};
use futures::{pin_mut, StreamExt};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

mod device;
mod services;
mod uuids;

pub use device::{FwUpdNotification, InfiniTime, MediaPlayerEvent, Notification};
pub use services::start_gatt_services;

pub async fn init_adapter() -> Result<Adapter> {
    let session = Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;
    Ok(adapter)
}

pub async fn scan(adapter: Arc<Adapter>, callback: impl Fn(AdapterEvent)) {
    match adapter.discover_devices().await {
        Ok(events) => {
            pin_mut!(events);

            loop {
                tokio::select! {
                    Some(event) = events.next() => callback(event),
                    else => break,
                }
            }
        }
        Err(error) => {
            log::error!("Scanning failure: {}", error);
        }
    }
}

struct CharacteristicsMap(HashMap<Uuid, Characteristic>);

impl CharacteristicsMap {
    async fn read(device: &Device) -> Result<Self> {
        let mut map = HashMap::new();
        for service in device.services().await? {
            for characteristic in service.characteristics().await? {
                let uuid = characteristic.uuid().await?;
                map.insert(uuid, characteristic);
            }
        }
        Ok(Self(map))
    }

    fn take(&mut self, uuid: &Uuid) -> Result<Characteristic> {
        match self.0.remove(uuid) {
            Some(c) => Ok(c),
            None => Err(bluer::Error {
                kind: bluer::ErrorKind::NotFound,
                message: format!("Characteristic not found by UUID: {}", uuid.to_string()),
            }),
        }
    }
}
