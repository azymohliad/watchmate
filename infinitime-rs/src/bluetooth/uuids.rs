use uuid::{uuid, Uuid};

pub const SRV_CURRENT_TIME: Uuid = uuid!("00001805-0000-1000-8000-00805f9b34fb");

pub const CHR_CURRENT_TIME: Uuid = uuid!("00002a2b-0000-1000-8000-00805f9b34fb");

pub const CHR_BATTERY_LEVEL: Uuid = uuid!("00002a19-0000-1000-8000-00805f9b34fb");
pub const CHR_FIRMWARE_REVISION: Uuid = uuid!("00002a26-0000-1000-8000-00805f9b34fb");
pub const CHR_HEART_RATE: Uuid = uuid!("00002a37-0000-1000-8000-00805f9b34fb");

pub const CHR_NEW_ALERT: Uuid = uuid!("00002a46-0000-1000-8000-00805f9b34fb");
pub const CHR_NOTIFICATION_EVENT: Uuid = uuid!("00020001-78fc-48fe-8e23-433b3a1942d0");

pub const CHR_FS_VERSION: Uuid = uuid!("adaf0100-4669-6c65-5472-616e73666572");
pub const CHR_FS_TRANSFER: Uuid = uuid!("adaf0200-4669-6c65-5472-616e73666572");

pub const CHR_FWUPD_CONTROL_POINT: Uuid = uuid!("00001531-1212-efde-1523-785feabcd123");
pub const CHR_FWUPD_PACKET: Uuid = uuid!("00001532-1212-efde-1523-785feabcd123");

pub const CHR_MP_EVENTS: Uuid = uuid!("00000001-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_STATUS: Uuid = uuid!("00000002-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_ARTIST: Uuid = uuid!("00000003-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_TRACK: Uuid = uuid!("00000004-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_ALBUM: Uuid = uuid!("00000005-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_POSITION: Uuid = uuid!("00000006-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_DURATION: Uuid = uuid!("00000007-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_SPEED: Uuid = uuid!("0000000a-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_REPEAT: Uuid = uuid!("0000000b-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MP_SHUFFLE: Uuid = uuid!("0000000c-78fc-48fe-8e23-433b3a1942d0");

pub const CHR_STEP_COUNT: Uuid = uuid!("00030001-78fc-48fe-8e23-433b3a1942d0");
pub const CHR_MOTION: Uuid = uuid!("00030002-78fc-48fe-8e23-433b3a1942d0");