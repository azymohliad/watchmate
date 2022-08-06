use std::{sync::Arc, collections::HashMap};
use futures::{pin_mut, StreamExt};
use bluer::{Adapter, AdapterEvent, Device, Result, Session, gatt::remote::Characteristic};
use uuid::Uuid;

mod infinitime;
mod uuids;
pub mod gatt_server;

pub use infinitime::{InfiniTime, Notification, FwUpdNotification};

pub async fn init_adapter() -> Result<Adapter> {
    let session = Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;
    Ok(adapter)
}


async fn read_characteristics_map(device: &Device) -> Result<HashMap<Uuid, Characteristic>> {
    let mut result = HashMap::new();
    for service in device.services().await? {
        // println!("Service {}", service.uuid().await?.to_string());
        for characteristic in service.characteristics().await? {
            let uuid = characteristic.uuid().await?;
            // println!("    - Characteristic {}", uuid.to_string());
            result.insert(uuid, characteristic);
        }
    }
    Ok(result)
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