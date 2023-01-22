use super::{uuids, InfiniTime, ProgressTx, ProgressTxWrapper};
use crate::inft::utils;
use anyhow::{anyhow, ensure, Result};
use futures::{pin_mut, StreamExt};
use serde::Deserialize;
use std::{
    io::{Cursor, Read},
    sync::atomic::Ordering,
};


pub const MAX_FIRMWARE_SIZE: usize = 512 * 1024;


#[derive(Deserialize, Debug)]
struct Manifest {
    manifest: ManifestInner,
}

#[derive(Deserialize, Debug)]
struct ManifestInner {
    application: Application,
    // dfu_version: String,
}

#[derive(Deserialize, Debug)]
struct Application {
    bin_file: String,
    dat_file: String,
    // init_packet_data: InitPacketData,
}

// #[derive(Deserialize, Debug)]
// struct InitPacketData {
//     application_version: u32,
//     device_revision: u16,
//     device_type: u16,
//     firmware_crc16: u16,
//     softdevice_req: Vec<u16>,
// }


impl InfiniTime {
    pub async fn firmware_upgrade(&self, dfu_content: &[u8], progress_sender: Option<ProgressTx>) -> Result<()> {
        let chr_ctrl = self.chr(&uuids::CHR_FWUPD_CONTROL_POINT)?;
        let chr_packet = self.chr(&uuids::CHR_FWUPD_PACKET)?;

        let progress = ProgressTxWrapper(progress_sender);

        self.is_upgrading_firmware.store(true, Ordering::SeqCst);

        // Set is_upgrading_firmware back to false automatically when function returns
        let _guard = utils::ScopeGuard::new(|| self.is_upgrading_firmware.store(false, Ordering::SeqCst));

        progress.report_msg("Extracting firmware files...").await;

        // Parse manifest from the archive
        let mut zip = zip::ZipArchive::new(Cursor::new(dfu_content))?;
        let mut json = String::new();
        zip.by_name("manifest.json")?.read_to_string(&mut json)?;
        let manifest = serde_json::from_str::<Manifest>(&json)
            .map_err(|_| anyhow!("Invalid manifest.json"))?.manifest;


        // Read DFU data
        let mut init_packet = Vec::new();
        zip.by_name(&manifest.application.dat_file)?.read_to_end(&mut init_packet)?;

        let mut firmware_buffer = Vec::new();
        {
            // file is not Send, so it needs to go out of scope before the next await
            let mut file = zip.by_name(&manifest.application.bin_file)?;
            ensure!(file.size() < MAX_FIRMWARE_SIZE as u64, "Firmware cannot be that large");
            file.read_to_end(&mut firmware_buffer)?;
        }

        // Obtain characteristics
        let control_point_stream = chr_ctrl.notify().await?;
        pin_mut!(control_point_stream);

        // Step 1
        progress.report_msg("Initiating firmware upgrade...").await;
        chr_ctrl.write(&[0x01, 0x04]).await?;

        // Step 2
        let mut size_packet = vec![0; 8];
        let firmware_size = firmware_buffer.len() as u32;
        size_packet.extend_from_slice(&firmware_size.to_le_bytes());
        chr_packet.write(&size_packet).await?;

        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x01, 0x01]);

        // Step 3
        progress.report_msg("Sending DFU init packet...").await;
        chr_ctrl.write(&[0x02, 0x00]).await?;

        // Step 4
        chr_packet.write(&init_packet).await?;
        chr_ctrl.write(&[0x02, 0x01]).await?;

        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x02, 0x01]);

        // Step 5
        progress.report_msg("Configuring receipt interval...").await;
        let receipt_interval = 100;
        chr_ctrl.write(&[0x08, receipt_interval]).await?;

        // Step 6
        chr_ctrl.write(&[0x03]).await?;

        // Step 7
        progress.report_msg("Sending firmware...").await;
        let mut bytes_sent = 0;
        for (idx, packet) in firmware_buffer.chunks(20).enumerate() {
            chr_packet.write(&packet).await?;
            bytes_sent += packet.len() as u32;
            if (idx + 1) % receipt_interval as usize == 0 {
                let receipt = control_point_stream.next().await
                    .ok_or(anyhow!("Control point notification stream ended"))?;
                let bytes_received = u32::from_le_bytes(receipt[1..5].try_into()?);
                ensure!(bytes_sent == bytes_received);
                progress.report_num(bytes_sent, firmware_size).await;
            }
        }

        // Step 8
        progress.report_msg("Waiting for firmware receipt...").await;
        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x03, 0x01]);
        chr_ctrl.write(&[0x04]).await?;

        // Step 9
        progress.report_msg("Waiting for firmware validation...").await;
        let receipt = control_point_stream.next().await
            .ok_or(anyhow!("Control point notification stream ended"))?;
        ensure!(receipt == &[0x10, 0x04, 0x01]);
        chr_ctrl.write(&[0x05]).await?;

        progress.report_msg("Done!").await;

        Ok(())
    }
}