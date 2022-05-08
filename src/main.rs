use tokio::runtime::Runtime;

mod bt;
mod ui;

fn main() {
    let runtime = Runtime::new().unwrap();
    let adapter = runtime.block_on(bt::init_adapter()).unwrap();
    ui::run(runtime, adapter);
}
