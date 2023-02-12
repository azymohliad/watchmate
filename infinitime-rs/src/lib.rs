pub mod bluetooth;
pub use bluetooth as bt;

#[cfg(feature = "freedesktop")]
pub mod freedesktop;
#[cfg(feature = "freedesktop")]
pub use freedesktop as fdo;

#[cfg(feature = "github")]
pub mod github;
#[cfg(feature = "github")]
pub use github as gh;

mod utils;


// Dependency reexports
pub use bluer;
pub use tokio;
#[cfg(feature = "freedesktop")]
pub use zbus;
