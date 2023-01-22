use super::uuids;
use uuid::Uuid;
use anyhow::{anyhow, Result};
use bluer::{gatt::remote::Characteristic, Adapter, Device};
use futures::{Stream, StreamExt};
use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, collections::HashMap};
use tokio::sync::mpsc;

pub mod fs;
pub mod fwupd;
pub mod notification;
pub mod media_player;
pub mod resources;


#[derive(Debug)]
pub struct InfiniTime {
    device: Arc<Device>,
    characteristics: HashMap<Uuid, Characteristic>,
    is_upgrading_firmware: AtomicBool,
}

impl InfiniTime {
    pub async fn new(device: Arc<Device>) -> Result<Self> {
        let characteristics = Self::read_characteristics_map(&device).await?;
        log::debug!("Characteristics: {:#?}", characteristics.keys());
        Ok(Self {
            device,
            characteristics,
            is_upgrading_firmware: AtomicBool::new(false),
        })
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    // -- Basic getters --

    pub async fn read_battery_level(&self) -> Result<u8> {
        Ok(self.chr(&uuids::CHR_BATTERY_LEVEL)?.read().await?[0])
    }

    pub async fn read_firmware_version(&self) -> Result<String> {
        let bytes = self.chr(&uuids::CHR_FIRMWARE_REVISION)?.read().await?;
        Ok(String::from_utf8(bytes)?)
    }

    pub async fn read_heart_rate(&self) -> Result<u8> {
        // TODO: Parse properly according to 3.106 Heart Rate Measurement
        // from https://www.bluetooth.org/docman/handlers/DownloadDoc.ashx?doc_id=539729
        Ok(self.chr(&uuids::CHR_HEART_RATE)?.read().await?[1])
    }

    // -- Media player control --

    // -- Event streams --

    pub async fn get_heart_rate_stream(&self) -> Result<impl Stream<Item = u8>> {
        let stream = self.chr(&uuids::CHR_HEART_RATE)?.notify().await?;
        Ok(stream.filter_map(|v| async move { v.get(1).cloned() }))
    }

    // -- Firmware upgrade --

    pub fn is_upgrading_firmware(&self) -> bool {
        self.is_upgrading_firmware.load(Ordering::SeqCst)
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

    fn chr<'s>(&'s self, uuid: &Uuid) -> Result<&'s Characteristic> {
        self.characteristics.get(uuid)
            .ok_or(anyhow!("Characteristic not found by UUID: {}", uuid.to_string()))
    }

    async fn read_characteristics_map(device: &Device) -> Result<HashMap<Uuid, Characteristic>> {
        let mut map = HashMap::new();
        for service in device.services().await? {
            for characteristic in service.characteristics().await? {
                let uuid = characteristic.uuid().await?;
                map.insert(uuid, characteristic);
            }
        }
        Ok(map)
    }
}


#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Message(String),
    Numbers { current: u32, total: u32 },
}

pub type ProgressRx = mpsc::Receiver<ProgressEvent>;
pub type ProgressTx = mpsc::Sender<ProgressEvent>;

pub fn progress_channel(capacity: usize) -> (ProgressTx, ProgressRx) {
    mpsc::channel(capacity)
}

// Private helper

struct ProgressTxWrapper(Option<ProgressTx>);

impl ProgressTxWrapper {
    async fn report(&self, event: ProgressEvent) {
        if let Some(tx) = &self.0 {
            if let Err(err) = tx.send(event).await {
                log::error!("Failed to send progress event: {}", err);
            }
        }
    }

    async fn report_msg<T: Into<String>>(&self, msg: T) {
        self.report(ProgressEvent::Message(msg.into())).await;
    }

    async fn report_num(&self, current: u32, total: u32) {
        self.report(ProgressEvent::Numbers { current, total }).await;
    }
}
