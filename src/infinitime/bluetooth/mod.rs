use bluer::{Adapter, AdapterEvent, Result, Session};
use futures::{pin_mut, StreamExt};
use std::sync::Arc;

mod device;
mod services;
mod uuids;

pub use device::{
    media_player::MediaPlayerEvent, notification::Notification,
    InfiniTime, ProgressEvent, ProgressRx, ProgressTx,
    progress_channel,
};
pub use services::start_gatt_services;

pub async fn init_adapter() -> Result<Adapter> {
    let session = Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;
    Ok(adapter)
}

pub async fn scan(adapter: Arc<Adapter>, callback: impl Fn(AdapterEvent)) {
    match adapter.discover_devices().await {
        Ok(events) => {
            pin_mut!(events);

            loop {
                tokio::select! {
                    Some(event) = events.next() => callback(event),
                    else => break,
                }
            }
        }
        Err(error) => {
            log::error!("Scanning failure: {}", error);
        }
    }
}
