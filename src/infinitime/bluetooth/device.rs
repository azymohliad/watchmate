use super::uuids;
use anyhow::{anyhow, ensure, Result};
use bluer::{gatt::remote::Characteristic, Adapter, Device};
use futures::{pin_mut, Stream, StreamExt};
use std::{
    io::{Cursor, Read},
    sync::Arc,
};

#[derive(Debug)]
pub enum FwUpdNotification {
    Message(&'static str),
    BytesSent(u32, u32),
}

#[derive(Debug)]
pub enum MediaPlayerEvent {
    AppOpenned,
    Play,
    Pause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
}

impl MediaPlayerEvent {
    fn from_raw(v: u8) -> Option<Self> {
        match v {
            0xe0 => Some(MediaPlayerEvent::AppOpenned),
            0x00 => Some(MediaPlayerEvent::Play),
            0x01 => Some(MediaPlayerEvent::Pause),
            0x03 => Some(MediaPlayerEvent::Next),
            0x04 => Some(MediaPlayerEvent::Previous),
            0x05 => Some(MediaPlayerEvent::VolumeUp),
            0x06 => Some(MediaPlayerEvent::VolumeDown),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct InfiniTime {
    device: Arc<Device>,
    // BLE Characteristics
    chr_battery_level: Characteristic,
    chr_firmware_revision: Characteristic,
    chr_heart_rate: Characteristic,
    chr_new_alert: Characteristic,
    chr_notification_event: Characteristic,
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
            chr_battery_level: characteristics.take(&uuids::CHR_BATTERY_LEVEL)?,
            chr_firmware_revision: characteristics.take(&uuids::CHR_FIRMWARE_REVISION)?,
            chr_heart_rate: characteristics.take(&uuids::CHR_HEART_RATE)?,
            chr_new_alert: characteristics.take(&uuids::CHR_NEW_ALERT)?,
            chr_notification_event: characteristics.take(&uuids::CHR_NOTIFICATION_EVENT)?,
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

    pub async fn write_notification(
        &self,
        title: &str,
        content: &str,
    ) -> Result<()> {
        // InfiniTime defines 10 categories, but at the time of writing only 2 of them
        // are implemented in the firmware: simple alert and call. It's not clear
        // whether others are intended to be implemented there later, so for now
        // we explicitly don't support them for the sake of simplicity
        let category = 0;   // Simple alert
        let count = 1;      // Notifications count
        let header = &[category, count];
        let message = [header, title.as_bytes(), content.as_bytes()].join(&0);
        Ok(self.chr_new_alert.write(&message).await?)
    }

    pub async fn write_mp_artist(&self, artist: &str) -> Result<()> {
        Ok(self.chr_mp_artist.write(artist.as_ref()).await?)
    }

    pub async fn write_mp_album(&self, album: &str) -> Result<()> {
        Ok(self.chr_mp_album.write(album.as_ref()).await?)
    }

    pub async fn write_mp_track(&self, track: &str) -> Result<()> {
        Ok(self.chr_mp_track.write(track.as_ref()).await?)
    }

    pub async fn write_mp_playback_status(&self, playing: bool) -> Result<()> {
        Ok(self.chr_mp_status.write(&[u8::from(playing)]).await?)
    }

    pub async fn write_mp_position(&self, position: u32) -> Result<()> {
        Ok(self.chr_mp_position.write(&position.to_be_bytes()).await?)
    }

    pub async fn write_mp_duration(&self, duration: u32) -> Result<()> {
        Ok(self.chr_mp_duration.write(&duration.to_be_bytes()).await?)
    }

    pub async fn write_mp_playback_speed(&self, speed: f32) -> Result<()> {
        let percentage = (speed * 100.0) as u32;
        Ok(self.chr_mp_speed.write(&percentage.to_be_bytes()).await?)
    }

    pub async fn write_mp_repeat(&self, repeat: bool) -> Result<()> {
        Ok(self.chr_mp_repeat.write(&[u8::from(repeat)]).await?)
    }

    pub async fn write_mp_shuffle(&self, shuffle: bool) -> Result<()> {
        Ok(self.chr_mp_shuffle.write(&[u8::from(shuffle)]).await?)
    }

    pub async fn get_heart_rate_stream(&self) -> Result<impl Stream<Item = u8>> {
        let stream = self.chr_heart_rate.notify().await?;
        Ok(stream.filter_map(|v| async move { v.get(1).cloned() }))
    }

    pub async fn get_media_player_events_stream(&self) -> Result<impl Stream<Item = MediaPlayerEvent>> {
        let stream = self.chr_mp_events.notify().await?;
        Ok(stream.filter_map(|v| async move { MediaPlayerEvent::from_raw(v[0]) }))
    }

    pub async fn firmware_upgrade<F>(&self, dfu_content: &[u8], callback: F) -> Result<()>
    where
        F: Fn(FwUpdNotification) + Send + 'static,
    {
        callback(FwUpdNotification::Message("Extracting firmware files..."));

        let mut zip = zip::ZipArchive::new(Cursor::new(dfu_content))?;

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
        let control_point_stream = self.chr_fwupd_control_point.notify().await?;
        pin_mut!(control_point_stream);

        // Step 1
        callback(FwUpdNotification::Message("Initiating firmware upgrade..."));
        self.chr_fwupd_control_point.write(&[0x01, 0x04]).await?;

        // Step 2
        let mut size_packet = vec![0; 8];
        let firmware_size = firmware_buffer.len() as u32;
        size_packet.extend_from_slice(&firmware_size.to_le_bytes());
        self.chr_fwupd_packet.write(&size_packet).await?;

        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x01, 0x01]);

        // Step 3
        callback(FwUpdNotification::Message("Sending DFU init packet..."));
        self.chr_fwupd_control_point.write(&[0x02, 0x00]).await?;

        // Step 4
        self.chr_fwupd_packet.write(&init_packet).await?;
        self.chr_fwupd_control_point.write(&[0x02, 0x01]).await?;

        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x02, 0x01]);

        // Step 5
        callback(FwUpdNotification::Message("Configuring receipt interval..."));
        let receipt_interval = 100;
        self.chr_fwupd_control_point.write(&[0x08, receipt_interval]).await?;

        // Step 6
        self.chr_fwupd_control_point.write(&[0x03]).await?;

        // Step 7
        callback(FwUpdNotification::Message("Sending firmware..."));
        let mut bytes_sent = 0;
        for (idx, packet) in firmware_buffer.chunks(20).enumerate() {
            self.chr_fwupd_packet.write(&packet).await?;
            bytes_sent += packet.len() as u32;
            if (idx + 1) % receipt_interval as usize == 0 {
                let receipt = control_point_stream.next().await
                    .ok_or(anyhow!("Control point notification stream ended"))?;
                let bytes_received = u32::from_le_bytes(receipt[1..5].try_into()?);
                ensure!(bytes_sent == bytes_received);
                callback(FwUpdNotification::BytesSent(bytes_sent, firmware_size))
            }
        }

        // Step 8
        callback(FwUpdNotification::Message("Waiting for firmware receipt..."));
        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x03, 0x01]);
        self.chr_fwupd_control_point.write(&[0x04]).await?;

        // Step 9
        callback(FwUpdNotification::Message("Waiting for firmware validation..."));
        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x04, 0x01]);
        self.chr_fwupd_control_point.write(&[0x05]).await?;

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
}
