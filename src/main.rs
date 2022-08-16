use std::sync::Arc;
use tokio::runtime::Runtime;

mod bt;
mod ui;
mod firmware_download;
mod media_player;

fn main() {
    let runtime = Runtime::new().unwrap();
    let adapter = Arc::new(runtime.block_on(bt::init_adapter()).unwrap());
    ui::run(adapter);
}
