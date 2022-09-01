mod ui;
mod infinitime;

use infinitime as inft;

fn main() {
    env_logger::Builder::new()
        .format_timestamp(None)
        .filter_module("watchmate", log::LevelFilter::Info)
        .parse_default_env()
        .init();

    ui::run();
}
