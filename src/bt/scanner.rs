use std::sync::Arc;
use futures::{pin_mut, StreamExt};
use tokio::sync::Notify;
use bluer::{Adapter, AdapterEvent};


#[derive(Clone)]
pub struct Scanner(Arc<Notify>);

impl Scanner {
    pub fn new() -> Self {
        Self(Arc::new(Notify::new()))
    }

    pub fn stop(&mut self) {
        self.0.notify_one();
    }

    pub async fn run(self, adapter: Arc<Adapter>, callback: impl Fn(AdapterEvent)) {
        match adapter.discover_devices().await {
            Ok(events) => {
                pin_mut!(events);

                loop {
                    tokio::select! {
                        Some(event) = events.next() => callback(event),
                        _ = self.0.notified() => break,
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
