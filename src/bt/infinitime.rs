use std::io;
use bluer::{Address, Device, Result};
use uuid::{uuid, Uuid};


pub struct InfiniTime {
    device: Device,
    name: Option<String>,
}

impl InfiniTime {
    pub async fn new(device: Device) -> Result<Self> {
        let name = device.name().await?;
        Ok(Self { device, name })
    }

    pub fn get_name(&self) -> Option<&str> {
        self.name.as_ref().map(String::as_str)
    }

    pub fn get_address(&self) -> Address {
        self.device.address()
    }

    pub async fn read_battery_level(&self) -> Result<u8> {
        const SERVICE_UUID: Uuid = uuid!("0000180F-0000-1000-8000-00805f9b34fb");
        const CHARACTERISTIC_UUID: Uuid = uuid!("00002A19-0000-1000-8000-00805f9b34fb");

        for service in self.device.services().await? {
            let uuid = service.uuid().await?;
            if uuid == SERVICE_UUID {
                for characteristic in service.characteristics().await? {
                    let uuid = characteristic.uuid().await?;
                    if uuid == CHARACTERISTIC_UUID {
                        return Ok(characteristic.read().await?[0]);
                    }
                }
            }
        }

        Err(io::Error::new(io::ErrorKind::NotFound, "Service or characteristic not found").into())
    }
}
