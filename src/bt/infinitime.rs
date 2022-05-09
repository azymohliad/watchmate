use std::{collections::HashMap, sync::Arc};
use tokio::{runtime, sync::Notify, task::JoinHandle};
use futures::{pin_mut, StreamExt};
use bluer::{gatt::remote::Characteristic, Adapter, Address, Device};
use uuid::Uuid;
use anyhow::{anyhow, Result};

use super::uuids;

pub enum Notification {
    HeartRate(u8),
}


pub struct InfiniTime {
    device: Device,
    alias: String,
    characteristics: HashMap<Uuid, Characteristic>,
    notification_stopper: Arc<Notify>,
    notification_handle: Option<JoinHandle<()>>,
}

impl InfiniTime {
    pub async fn new(device: Device) -> Result<Self> {
        let alias = device.alias().await?;
        let characteristics = super::read_characteristics_map(&device).await?;
        Ok(Self {
            device,
            alias,
            characteristics,
            notification_stopper: Arc::new(Notify::new()),
            notification_handle: None,
        })
    }

    pub fn get_alias(&self) -> &str {
        self.alias.as_str()
    }

    pub fn get_address(&self) -> Address {
        self.device.address()
    }

    pub async fn read_battery_level(&self) -> Result<u8> {
        Ok(self.read_characteristic(&uuids::CHR_BATTERY_LEVEL).await?[0])
    }

    pub async fn read_firmware_version(&self) -> Result<String> {
        Ok(String::from_utf8(self.read_characteristic(&uuids::CHR_FIRMWARE_REVISION).await?)?)
    }

    pub async fn read_heart_rate(&self) -> Result<u8> {
        // TODO: Parse properly according to 3.106 Heart Rate Measurement
        // from https://www.bluetooth.org/docman/handlers/DownloadDoc.ashx?doc_id=539729
        Ok(self.read_characteristic(&uuids::CHR_HEART_RATE).await?[1])
    }

    pub fn start_notification_session<F>(&mut self, runtime: runtime::Handle, callback: F)
        where F: Fn(Notification) + Send + 'static
    {
        let heart_rate_chr = self.characteristics.get(&uuids::CHR_HEART_RATE).unwrap().clone();
        let stopper = self.notification_stopper.clone();

        self.notification_handle = Some(runtime.spawn(async move {
            let heart_rate_stream = heart_rate_chr.notify().await.unwrap();
            pin_mut!(heart_rate_stream);

            loop {
                tokio::select! {
                    Some(value) = heart_rate_stream.next() => {
                        callback(Notification::HeartRate(value[1]));
                    }
                    _ = stopper.notified() => break,
                    else => break,
                }
            }
        }));
    }

    pub fn stop_notification_session(&mut self) {
        if let Some(_handle) = self.notification_handle.take() {
            self.notification_stopper.notify_one();
            // TODO: Would it be useful to await on handle?
        }
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

    pub async fn list_known_devices(adapter: &Adapter) -> Result<Vec<Device>> {
        let mut result = Vec::new();
        for address in adapter.device_addresses().await? {
            let device = adapter.device(address)?;
            if Self::check_device(&device).await {
                result.push(device);
            }
        }
        Ok(result)
    }
}
