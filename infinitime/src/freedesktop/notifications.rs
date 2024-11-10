use anyhow::Result;
use futures::TryStreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use zbus::{
    match_rule::MatchRule,
    zvariant::{Type, Value},
};

use crate::bt;

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

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::MethodCall)
        .interface("org.freedesktop.Notifications")?
        .member("Notify")?
        .path("/org/freedesktop/Notifications")?
        .build();
    proxy.become_monitor(&[rule], 0).await?;

    let mut stream = zbus::MessageStream::from(&connection);
    while let Some(msg) = stream.try_next().await? {
        match msg.body().deserialize::<DesktopNotification>() {
            Ok(notification) => {
                // Dirty hack to avoid duplicated notifications:
                // For some reason, every notification produces two calls on DBus
                // with identical fields, except the second contains extra hints:
                // "x-shell-sender" and "x-shell-sender-pid".
                // TODO: Find proper solution.
                if notification.hints.contains_key("x-shell-sender") {
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
