use std::collections::HashMap;
use bluer::{Adapter, Device, Result, Session, gatt::remote::Characteristic};
use uuid::Uuid;

mod scanner;
mod infinitime;
mod uuids;
pub mod gatt_server;

pub use scanner::Scanner;
pub use infinitime::{InfiniTime, Notification};

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
