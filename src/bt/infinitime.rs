use std::collections::HashMap;
use bluer::{Address, Device, gatt::remote::Characteristic};
use uuid::{uuid, Uuid};
use anyhow::{anyhow, Result};

const CHR_BATTERY_LEVEL: Uuid = uuid!("00002a19-0000-1000-8000-00805f9b34fb");
const CHR_FIRMWARE_REVISION: Uuid = uuid!("00002a26-0000-1000-8000-00805f9b34fb");
const CHR_HEART_RATE: Uuid = uuid!("00002a37-0000-1000-8000-00805f9b34fb");


pub struct InfiniTime {
    device: Device,
    alias: String,
    characteristics: HashMap<Uuid, Characteristic>,
}

impl InfiniTime {
    pub async fn new(device: Device) -> Result<Self> {
        let alias = device.alias().await?;
        let characteristics = super::read_characteristics_map(&device).await?;
        Ok(Self { device, alias, characteristics })
    }

    pub fn get_alias(&self) -> &str {
        self.alias.as_str()
    }

    pub fn get_address(&self) -> Address {
        self.device.address()
    }

    pub async fn read_battery_level(&self) -> Result<u8> {
        Ok(self.read_characteristic(&CHR_BATTERY_LEVEL).await?[0])
    }

    pub async fn read_firmware_version(&self) -> Result<String> {
        Ok(String::from_utf8(self.read_characteristic(&CHR_FIRMWARE_REVISION).await?)?)
    }

    pub async fn read_heart_rate(&self) -> Result<u8> {
        // TODO: Parse properly according to 3.106 Heart Rate Measurement
        // from https://www.bluetooth.org/docman/handlers/DownloadDoc.ashx?doc_id=539729
        Ok(self.read_characteristic(&CHR_HEART_RATE).await?[1])
    }

    async fn read_characteristic(&self, uuid: &Uuid) -> Result<Vec<u8>> {
        match self.characteristics.get(uuid) {
            Some(c) => Ok(c.read().await?),
            None => Err(anyhow!("Characteristic {} not found", uuid.to_string())),
        }
    }

    pub async fn check_device(device: &Device) -> bool {
        match device.name().await {
            Ok(Some(name)) => name.as_str() == "InfiniTime",
            _ => false,
        }
    }
}
