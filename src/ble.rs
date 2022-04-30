use std::sync::Arc;
use futures::{pin_mut, StreamExt};
use tokio::sync::Notify;
use relm4::Sender;


pub async fn scan(notifier: Arc<Notify>, sender: Sender<super::AppMsg>) -> bluer::Result<()>
{
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;

    println!("Discovering devices using Bluetooth adapater {}\n", adapter.name());
    adapter.set_powered(true).await?;

    let device_events = adapter.discover_devices().await?;
    pin_mut!(device_events);

    loop {
        tokio::select! {
            Some(device_event) = device_events.next() => {
                match device_event {
                    bluer::AdapterEvent::DeviceAdded(addr) => {
                        let device = adapter.device(addr)?;
                        let device_info = super::DeviceInfo {
                            address: addr,
                            name: device.name().await?,
                            rssi: device.rssi().await?,
                        };
                        sender.send(super::AppMsg::DeviceAdded(device_info)).unwrap();
                    }
                    bluer::AdapterEvent::DeviceRemoved(addr) => {
                        sender.send(super::AppMsg::DeviceRemoved(addr)).unwrap();
                    }
                    _ => (),
                }
            }
            _ = notifier.notified() => break,
            else => break
        }
    }

    Ok(())
}
