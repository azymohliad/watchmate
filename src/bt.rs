use std::sync::Arc;
use futures::{pin_mut, StreamExt};
use tokio::{sync::Notify, runtime::Runtime, task::JoinHandle};
use bluer::{Adapter, Address, AdapterEvent, Device, Result, Session};


pub struct Host {
    adapter: Adapter,
    scan_stopper: Arc<Notify>,
    scan_handle: Option<JoinHandle<()>>
}

impl Host {
    pub async fn new() -> Result<Self> {
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;
        let scan_stopper = Arc::new(Notify::new());
        let scan_handle = None;

        adapter.set_powered(true).await?;

        Ok(Self { adapter, scan_stopper, scan_handle })
    }

    pub fn device(&self, address: Address) -> Result<Device> {
        self.adapter.device(address)
    }

    pub fn scan_start<F>(&mut self, rt: &Runtime, callback: F)
        where F: Fn(AdapterEvent) + Send + 'static
    {
        let adapter = self.adapter.clone();
        let stopper = self.scan_stopper.clone();
        let join_handle = rt.spawn(async {
            Self::scan(adapter, stopper, callback).await;
        });
        self.scan_handle = Some(join_handle);
    }

    pub fn scan_stop(&mut self) {
        if let Some(_handle) = self.scan_handle.take() {
            self.scan_stopper.notify_one();
            // TODO: Would it be useful to await on handle?
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.scan_handle.is_some()
    }

    async fn scan(adapter: Adapter, stopper: Arc<Notify>, callback: impl Fn(AdapterEvent)) {
        match adapter.discover_devices().await {
            Ok(events) => {
                pin_mut!(events);

                loop {
                    tokio::select! {
                        Some(event) = events.next() => callback(event),
                        _ = stopper.notified() => break,
                        else => break,
                    }
                }
            },
            Err(error) => {
                eprintln!("Error: {}", error);
            }
        }
    }
}
