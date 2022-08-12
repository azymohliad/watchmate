use std::{sync::Arc, collections::HashMap};
use futures::{pin_mut, StreamExt};
use bluer::{Adapter, AdapterEvent, Device, Result, Session, gatt::remote::Characteristic};
use uuid::Uuid;

mod infinitime;
mod uuids;
pub mod gatt_server;

pub use infinitime::{InfiniTime, FwUpdNotification, MediaPlayerEvent};

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
        },
        Err(error) => {
            eprintln!("Scanning failure: {}", error);
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
            })
        }
    }
}

