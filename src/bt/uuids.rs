use uuid::{uuid, Uuid};

pub const SRV_CURRENT_TIME: Uuid = uuid!("00001805-0000-1000-8000-00805f9b34fb");

pub const CHR_CURRENT_TIME: Uuid = uuid!("00002a2b-0000-1000-8000-00805f9b34fb");

pub const CHR_BATTERY_LEVEL: Uuid = uuid!("00002a19-0000-1000-8000-00805f9b34fb");
pub const CHR_FIRMWARE_REVISION: Uuid = uuid!("00002a26-0000-1000-8000-00805f9b34fb");
pub const CHR_HEART_RATE: Uuid = uuid!("00002a37-0000-1000-8000-00805f9b34fb");

pub const CHR_FWUPD_CONTROL_POINT: Uuid = uuid!("00001531-1212-efde-1523-785feabcd123");
pub const CHR_FWUPD_PACKET: Uuid = uuid!("00001532-1212-efde-1523-785feabcd123");
