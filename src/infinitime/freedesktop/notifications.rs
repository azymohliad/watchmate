use std::collections::HashMap;
use zbus::zvariant::{Type, Value};
use serde::Deserialize;
use anyhow::Result;
use futures::TryStreamExt;

use crate::inft::bt;


#[derive(Debug, Deserialize, Type)]
struct Notification<'s> {
    app_name: &'s str,
    replaces_id: u32,
    app_icon: &'s str,
    summary: &'s str,
    body: &'s str,
    actions: Vec<&'s str>,
    hints: HashMap<&'s str, Value<'s>>,
    expire_timeout: i32,
}

pub async fn run_notification_session(infinitime: &bt::InfiniTime) -> Result<()> {
    // Monitor requires a separate connection
    let connection = zbus::Connection::session().await?;
    let proxy = zbus::fdo::MonitoringProxy::builder(&connection)
        .destination("org.freedesktop.DBus")?
        .path("/org/freedesktop/DBus")?
        .build()
        .await?;

    let rules = "type='method_call',member='Notify',path='/org/freedesktop/Notifications',interface='org.freedesktop.Notifications'";
    proxy.become_monitor(&[rules], 0).await?;

    let mut stream = zbus::MessageStream::from(connection);
    while let Some(msg) = stream.try_next().await? {
        if msg.message_type() == zbus::MessageType::MethodCall {
            match msg.body::<Notification>() {
                Ok(notification) => {
                    _ = infinitime.write_notification(notification.summary, notification.body).await;
                }
                Err(error) => {
                    log::error!("Failed to parse notification: {error}");
                }
            }
        }
    }
    Ok(())
}
