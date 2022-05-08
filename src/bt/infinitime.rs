use std::io;
use bluer::{Address, Device};
use uuid::{uuid, Uuid};
use anyhow::{anyhow, Result};


pub struct InfiniTime {
    device: Device,
    alias: String,
}

impl InfiniTime {
    pub async fn new(device: Device) -> Result<Self> {
        let alias = device.alias().await?;
        Ok(Self { device, alias })
    }

    pub fn get_alias(&self) -> &str {
        self.alias.as_str()
    }

    pub fn get_address(&self) -> Address {
        self.device.address()
    }

    pub async fn read_battery_level(&self) -> Result<u8> {
        const SERVICE_UUID: Uuid = uuid!("0000180f-0000-1000-8000-00805f9b34fb");
        const CHARACTERISTIC_UUID: Uuid = uuid!("00002a19-0000-1000-8000-00805f9b34fb");

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

        Err(anyhow!("Service or characteristic not found"))
    }

    pub async fn read_firmware_version(&self) -> Result<String> {
        const SERVICE_UUID: Uuid = uuid!("0000180a-0000-1000-8000-00805f9b34fb");
        const CHARACTERISTIC_UUID: Uuid = uuid!("00002a26-0000-1000-8000-00805f9b34fb");

        for service in self.device.services().await? {
            let uuid = service.uuid().await?;
            if uuid == SERVICE_UUID {
                for characteristic in service.characteristics().await? {
                    let uuid = characteristic.uuid().await?;
                    if uuid == CHARACTERISTIC_UUID {
                        let version = String::from_utf8(characteristic.read().await?)?;
                        return Ok(version);
                    }
                }
            }
        }

        Err(anyhow!("Service or characteristic not found"))
    }

    pub async fn check_device(device: &Device) -> bool {
        match device.name().await {
            Ok(Some(name)) => name.as_str() == "InfiniTime",
            _ => false,
        }
    }
}
