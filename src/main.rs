use std::sync::Arc;
use tokio::runtime::Runtime;

mod bt;
mod ui;
mod firmware_download;

fn main() {
    let runtime = Runtime::new().unwrap();
    let adapter = Arc::new(runtime.block_on(bt::init_adapter()).unwrap());
    let _gatt_app = runtime.block_on(bt::gatt_server::start(&adapter)).unwrap();
    ui::run(adapter);
}
