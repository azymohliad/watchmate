use std::{fs::File, collections::HashMap, path::Path, sync::Arc, io::Read};
use futures::{pin_mut, StreamExt};
use bluer::{gatt::remote::Characteristic, Adapter, Device};
use uuid::Uuid;
use anyhow::{anyhow, ensure, Result};

use super::uuids;

pub enum Notification {
    HeartRate(u8),
}

#[derive(Debug)]
pub enum FwUpdNotification {
    Message(&'static str),
    BytesSent(u32, u32),
}

#[derive(Debug)]
pub struct InfiniTime {
    device: Arc<Device>,
    characteristics: HashMap<Uuid, Characteristic>,
}

impl InfiniTime {
    pub async fn new(device: Arc<Device>) -> Result<Self> {
        let characteristics = super::read_characteristics_map(&device).await?;
        Ok(Self { device, characteristics })
    }

    pub fn device(&self) -> &bluer::Device {
        &self.device
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

    pub async fn run_notification_session<F>(&self, callback: F)
        where F: Fn(Notification) + Send + 'static
    {
        let heart_rate_chr = self.characteristics.get(&uuids::CHR_HEART_RATE).unwrap();
        let heart_rate_stream = heart_rate_chr.notify().await.unwrap();
        pin_mut!(heart_rate_stream);

        loop {
            tokio::select! {
                Some(value) = heart_rate_stream.next() => {
                    callback(Notification::HeartRate(value[1]));
                }
                else => break,
            }
        }
    }

    pub async fn firmware_upgrade<F>(&self, filepath: &Path, callback: F) -> Result<()>
        where F: Fn(FwUpdNotification) + Send + 'static
    {
        callback(FwUpdNotification::Message("Extracting firmware files..."));

        let file = File::open(filepath)?;
        let mut zip = zip::ZipArchive::new(file)?;

        // Find filenames
        let mut dfu_bin = None;
        let mut dfu_dat = None;
        for filename in zip.file_names() {
            if filename.ends_with(".bin") {
                if dfu_bin.replace(filename).is_some() {
                    return Err(anyhow!("DFU archive contains multiple .bin files"));
                }
            }
            if filename.ends_with(".dat") {
                if dfu_dat.replace(filename).is_some() {
                    return Err(anyhow!("DFU archive contains multiple .dat files"));
                }
            }
        }
        if dfu_bin.is_none() || dfu_dat.is_none() {
            return Err(anyhow!("DFU archive is lacking .bin and/or .dat files"));
        }
        // Filenames need to be cloned to unborrow zip
        let dfu_bin = dfu_bin.unwrap().to_string();
        let dfu_dat = dfu_dat.unwrap().to_string();

        // Read DFU data
        let mut init_packet = Vec::new();
        zip.by_name(&dfu_dat).unwrap().read_to_end(&mut init_packet)?;
        let mut firmware_buffer = Vec::new();
        zip.by_name(&dfu_bin).unwrap().read_to_end(&mut firmware_buffer)?;


        // Obtain characteristics
        let ctl_char = self.characteristics.get(&uuids::CHR_FWUPD_CONTROL_POINT)
            .ok_or(anyhow!("Firmware update control point characteristic is not found"))?;
        let pkt_char = self.characteristics.get(&uuids::CHR_FWUPD_PACKET)
            .ok_or(anyhow!("Firmware update packet characteristic is not found"))?;
        let ctl_stream = ctl_char.notify().await?;
        pin_mut!(ctl_stream);

        // Step 1
        callback(FwUpdNotification::Message("Initiating firmware upgrade..."));
        ctl_char.write(&[0x01, 0x04]).await?;

        // Step 2
        let mut size_packet = vec![0; 8];
        let firmware_size = firmware_buffer.len() as u32;
        size_packet.extend_from_slice(&firmware_size.to_le_bytes());
        pkt_char.write(&size_packet).await?;

        let receipt = ctl_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x01, 0x01]);

        // Step 3
        callback(FwUpdNotification::Message("Sending DFU init packet..."));
        ctl_char.write(&[0x02, 0x00]).await?;

        // Step 4
        pkt_char.write(&init_packet).await?;
        ctl_char.write(&[0x02, 0x01]).await?;

        let receipt = ctl_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x02, 0x01]);

        // Step 5
        callback(FwUpdNotification::Message("Configuring receipt interval..."));
        let receipt_interval = 100;
        ctl_char.write(&[0x08, receipt_interval]).await?;

        // Step 6
        ctl_char.write(&[0x03]).await?;

        // Step 7
        callback(FwUpdNotification::Message("Sending firmware..."));
        let mut bytes_sent = 0;
        for (idx, packet) in firmware_buffer.chunks(20).enumerate() {
            pkt_char.write(&packet).await?;
            bytes_sent += packet.len() as u32;
            if (idx + 1) % receipt_interval as usize == 0 {
                let receipt = ctl_stream.next().await
                    .ok_or(anyhow!("Control point notification stream ended"))?;
                let bytes_received = u32::from_le_bytes(receipt[1..5].try_into()?);
                ensure!(bytes_sent == bytes_received);
                callback(FwUpdNotification::BytesSent(bytes_sent, firmware_size))
            }
        }

        // Step 8
        callback(FwUpdNotification::Message("Waiting for firmware receipt..."));
        let receipt = ctl_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x03, 0x01]);
        ctl_char.write(&[0x04]).await?;


        // Step 9
        callback(FwUpdNotification::Message("Waiting for firmware validation..."));
        let receipt = ctl_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x04, 0x01]);
        ctl_char.write(&[0x05]).await?;

        callback(FwUpdNotification::Message("Done!"));

        Ok(())
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

    async fn read_characteristic(&self, uuid: &Uuid) -> Result<Vec<u8>> {
        match self.characteristics.get(uuid) {
            Some(c) => Ok(c.read().await?),
            None => Err(anyhow!("Characteristic {} not found", uuid.to_string())),
        }
    }
}
