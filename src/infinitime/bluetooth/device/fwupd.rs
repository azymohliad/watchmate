use super::InfiniTime;
use crate::inft::utils;
use anyhow::{anyhow, ensure, Result};
use futures::{pin_mut, StreamExt};
use std::{
    io::{Cursor, Read},
    sync::atomic::Ordering,
};

#[derive(Debug)]
pub enum DfuProgressMsg {
    Message(&'static str),
    BytesSent(u32, u32),
}


impl InfiniTime {
    pub async fn firmware_upgrade<F>(&self, dfu_content: &[u8], callback: F) -> Result<()>
    where
        F: Fn(DfuProgressMsg) + Send + 'static,
    {
        self.is_upgrading_firmware.store(true, Ordering::SeqCst);

        // Set is_upgrading_firmware back to false automatically when function returns
        let _guard = utils::ScopeGuard::new(|| self.is_upgrading_firmware.store(false, Ordering::SeqCst));

        callback(DfuProgressMsg::Message("Extracting firmware files..."));

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
        callback(DfuProgressMsg::Message("Initiating firmware upgrade..."));
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
        callback(DfuProgressMsg::Message("Sending DFU init packet..."));
        self.chr_fwupd_control_point.write(&[0x02, 0x00]).await?;

        // Step 4
        self.chr_fwupd_packet.write(&init_packet).await?;
        self.chr_fwupd_control_point.write(&[0x02, 0x01]).await?;

        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x02, 0x01]);

        // Step 5
        callback(DfuProgressMsg::Message("Configuring receipt interval..."));
        let receipt_interval = 100;
        self.chr_fwupd_control_point.write(&[0x08, receipt_interval]).await?;

        // Step 6
        self.chr_fwupd_control_point.write(&[0x03]).await?;

        // Step 7
        callback(DfuProgressMsg::Message("Sending firmware..."));
        let mut bytes_sent = 0;
        for (idx, packet) in firmware_buffer.chunks(20).enumerate() {
            self.chr_fwupd_packet.write(&packet).await?;
            bytes_sent += packet.len() as u32;
            if (idx + 1) % receipt_interval as usize == 0 {
                let receipt = control_point_stream.next().await
                    .ok_or(anyhow!("Control point notification stream ended"))?;
                let bytes_received = u32::from_le_bytes(receipt[1..5].try_into()?);
                ensure!(bytes_sent == bytes_received);
                callback(DfuProgressMsg::BytesSent(bytes_sent, firmware_size))
            }
        }

        // Step 8
        callback(DfuProgressMsg::Message("Waiting for firmware receipt..."));
        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x03, 0x01]);
        self.chr_fwupd_control_point.write(&[0x04]).await?;

        // Step 9
        callback(DfuProgressMsg::Message("Waiting for firmware validation..."));
        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x04, 0x01]);
        self.chr_fwupd_control_point.write(&[0x05]).await?;

        callback(DfuProgressMsg::Message("Done!"));

        Ok(())
    }
}