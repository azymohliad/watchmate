use std::sync::Arc;
use tokio::runtime::Runtime;

mod bt;
mod ui;
mod firmware_download;
mod media_player;

fn main() {
    env_logger::Builder::new()
        .format_timestamp(None)
        .filter_module("watchmate", log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let runtime = Runtime::new().expect("Failed to initialize tokio runtime");
    match runtime.block_on(bt::init_adapter()) {
        Ok(adapter) => ui::run(Arc::new(adapter)),
        Err(_) => log::error!("Failed to initialize bluetooth adapter"),
    }
}
