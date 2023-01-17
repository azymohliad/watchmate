use super::uuids;
use anyhow::Result;
use bluer::{gatt::remote::Characteristic, Adapter, Device};
use futures::{Stream, StreamExt};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub mod fs;
pub mod fwupd;
pub mod notification;
pub mod media_player;


#[derive(Debug)]
pub struct InfiniTime {
    device: Arc<Device>,
    is_upgrading_firmware: AtomicBool,
    // BLE Characteristics
    chr_battery_level: Characteristic,
    chr_firmware_revision: Characteristic,
    chr_heart_rate: Characteristic,
    chr_new_alert: Characteristic,
    chr_notification_event: Characteristic,
    chr_fs_version: Characteristic,
    chr_fs_transfer: Characteristic,
    chr_fwupd_control_point: Characteristic,
    chr_fwupd_packet: Characteristic,
    chr_mp_events: Characteristic,
    chr_mp_status: Characteristic,
    chr_mp_artist: Characteristic,
    chr_mp_track: Characteristic,
    chr_mp_album: Characteristic,
    chr_mp_position: Characteristic,
    chr_mp_duration: Characteristic,
    chr_mp_speed: Characteristic,
    chr_mp_repeat: Characteristic,
    chr_mp_shuffle: Characteristic,
}

impl InfiniTime {
    pub async fn new(device: Arc<Device>) -> Result<Self> {
        let mut characteristics = super::CharacteristicsMap::read(&device).await?;
        log::debug!("Characteristics: {:#?}", characteristics.0.keys());
        Ok(Self {
            device,
            is_upgrading_firmware: AtomicBool::new(false),
            chr_battery_level: characteristics.take(&uuids::CHR_BATTERY_LEVEL)?,
            chr_firmware_revision: characteristics.take(&uuids::CHR_FIRMWARE_REVISION)?,
            chr_heart_rate: characteristics.take(&uuids::CHR_HEART_RATE)?,
            chr_new_alert: characteristics.take(&uuids::CHR_NEW_ALERT)?,
            chr_notification_event: characteristics.take(&uuids::CHR_NOTIFICATION_EVENT)?,
            chr_fs_version: characteristics.take(&uuids::CHR_FS_VERSION)?,
            chr_fs_transfer: characteristics.take(&uuids::CHR_FS_TRANSFER)?,
            chr_fwupd_control_point: characteristics.take(&uuids::CHR_FWUPD_CONTROL_POINT)?,
            chr_fwupd_packet: characteristics.take(&uuids::CHR_FWUPD_PACKET)?,
            chr_mp_events: characteristics.take(&uuids::CHR_MP_EVENTS)?,
            chr_mp_status: characteristics.take(&uuids::CHR_MP_STATUS)?,
            chr_mp_artist: characteristics.take(&uuids::CHR_MP_ARTIST)?,
            chr_mp_track: characteristics.take(&uuids::CHR_MP_TRACK)?,
            chr_mp_album: characteristics.take(&uuids::CHR_MP_ALBUM)?,
            chr_mp_position: characteristics.take(&uuids::CHR_MP_POSITION)?,
            chr_mp_duration: characteristics.take(&uuids::CHR_MP_DURATION)?,
            chr_mp_speed: characteristics.take(&uuids::CHR_MP_SPEED)?,
            chr_mp_repeat: characteristics.take(&uuids::CHR_MP_REPEAT)?,
            chr_mp_shuffle: characteristics.take(&uuids::CHR_MP_SHUFFLE)?,
        })
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    // -- Basic getters --

    pub async fn read_battery_level(&self) -> Result<u8> {
        Ok(self.chr_battery_level.read().await?[0])
    }

    pub async fn read_firmware_version(&self) -> Result<String> {
        Ok(String::from_utf8(self.chr_firmware_revision.read().await?)?)
    }

    pub async fn read_heart_rate(&self) -> Result<u8> {
        // TODO: Parse properly according to 3.106 Heart Rate Measurement
        // from https://www.bluetooth.org/docman/handlers/DownloadDoc.ashx?doc_id=539729
        Ok(self.chr_heart_rate.read().await?[1])
    }

    // -- Media player control --

    // -- Event streams --

    pub async fn get_heart_rate_stream(&self) -> Result<impl Stream<Item = u8>> {
        let stream = self.chr_heart_rate.notify().await?;
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
}
