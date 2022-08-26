use std::sync::Arc;
use tokio::runtime::Runtime;

mod ui;
mod infinitime;

use infinitime as inft;

fn main() {
    env_logger::Builder::new()
        .format_timestamp(None)
        .filter_module("watchmate", log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let runtime = Runtime::new().expect("Failed to initialize tokio runtime");
    match runtime.block_on(inft::bt::init_adapter()) {
        Ok(adapter) => ui::run(Arc::new(adapter)),
        Err(_) => log::error!("Failed to initialize bluetooth adapter"),
    }
}
