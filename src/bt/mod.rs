mod scanner;
mod infinitime;

pub use scanner::Scanner;
pub use infinitime::InfiniTime;

pub async fn init_adapter() -> bluer::Result<bluer::Adapter> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    adapter.set_powered(true).await?;
    Ok(adapter)
}
