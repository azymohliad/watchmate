use super::uuids;
use futures::FutureExt;
use bluer::{
    gatt::local::{
        Application, ApplicationHandle, Characteristic, CharacteristicRead, Service,
    },
    Adapter, Result,
};
use chrono::{Local, Datelike, Timelike};

pub async fn start_gatt_services(adapter: &Adapter) -> Result<ApplicationHandle> {
    let app = Application {
        services: vec![
            current_time_service()
        ],
        ..Default::default()
    };

    adapter.serve_gatt_application(app).await
}


fn current_time_service() -> Service {
    Service {
        uuid: uuids::SRV_CURRENT_TIME,
        primary: true,
        characteristics: vec![Characteristic {
            uuid: uuids::CHR_CURRENT_TIME,
            read: Some(CharacteristicRead {
                read: true,
                fun: Box::new(move |req| {
                    async move {
                        log::debug!("{:?}", &req);
                        let now = Local::now();
                        let year = (now.year() as u16).to_le_bytes();
                        Ok(vec![
                            year[0],
                            year[1],
                            now.month() as u8,
                            now.day() as u8,
                            now.hour() as u8,
                            now.minute() as u8,
                            now.second() as u8,
                            now.weekday().number_from_monday() as u8,
                            0x00,   // Fractions256
                            0x00,   // Adjust reason
                        ])
                    }.boxed()
                }),
                ..Default::default()
            }),
            write: None,
            notify: None,
            ..Default::default()
        }],
        ..Default::default()
    }
}