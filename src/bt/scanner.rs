use std::sync::Arc;
use futures::{pin_mut, StreamExt};
use tokio::{sync::Notify, runtime::Runtime, task::JoinHandle};
use bluer::{Adapter, AdapterEvent};


pub struct Scanner {
    stopper: Arc<Notify>,
    handle: Option<JoinHandle<()>>
}

impl Scanner {
    pub fn new() -> Self {
        let stopper = Arc::new(Notify::new());
        let handle = None;

        Self { stopper, handle }
    }

    pub fn start<F>(&mut self, adapter: Adapter, rt: &Runtime, callback: F)
        where F: Fn(AdapterEvent) + Send + 'static
    {
        let stopper = self.stopper.clone();
        let join_handle = rt.spawn(async {
            Self::scan(adapter, stopper, callback).await;
        });
        self.handle = Some(join_handle);
    }

    pub fn stop(&mut self) {
        if let Some(_handle) = self.handle.take() {
            self.stopper.notify_one();
            // TODO: Would it be useful to await on handle?
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.handle.is_some()
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
