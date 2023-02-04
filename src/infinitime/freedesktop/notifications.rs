use std::collections::HashMap;
use zbus::zvariant::{Type, Value};
use serde::Deserialize;
use anyhow::Result;
use futures::TryStreamExt;

use crate::inft::bt;

#[allow(unused)]
#[derive(Debug, Deserialize, Type)]
struct DesktopNotification<'s> {
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

    let rules = "type='method_call',member='Notify',path='/org/freedesktop/Notifications',interface='org.freedesktop.Notifications',eavesdrop=true";
    proxy.become_monitor(&[rules], 0).await?;

    let mut stream = zbus::MessageStream::from(&connection);
    while let Some(msg) = stream.try_next().await? {
        match msg.body::<DesktopNotification>() {
            Ok(notification) => {
                // Dirty hack to avoid duplicated notifications:
                // For some reason, every notification produces two identical calls on DBus,
                // except one has hints["sender-pid"] as U32 and another one as I64, so we
                // can deduplicate them by filtering out one of these types.
                // TODO: Find proper solution.
                if let Some(Value::U32(_)) = notification.hints.get("sender-pid") {
                    continue;
                }

                if infinitime.is_upgrading_firmware() {
                    continue;
                }

                log::debug!("Forwarding notification: {notification:?}");
                let alert = bt::Notification::Alert {
                    title: &format!("{}: {}", notification.app_name, notification.summary),
                    content: notification.body,
                };
                _ = infinitime.write_notification(alert).await;
            }
            Err(error) => {
                log::error!("Failed to parse notification: {error}");
            }
        }
    }
    Ok(())
}
