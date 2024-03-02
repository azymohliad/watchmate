use crate::ui;
use infinitime::{zbus, bt, fdo::notifications};
use std::sync::Arc;
use gtk::{gio, prelude::{BoxExt, OrientableExt, WidgetExt, SettingsExt, SettingsExtManual}};
use relm4::{gtk, ComponentParts, ComponentSender, Component, JoinHandle, RelmWidgetExt};


#[derive(Debug)]
pub enum Input {
    Device(Option<Arc<bt::InfiniTime>>),
    SetNotificationSession(bool),
    NotificationSessionEnded,
}

#[derive(Default)]
pub struct Model {
    infinitime: Option<Arc<bt::InfiniTime>>,
    is_enabled: bool,
    task: Option<JoinHandle<()>>,
}

impl Model {
    fn start_notifications_task(&mut self, sender: ComponentSender<Self>) {
        if let Some(infinitime) = self.infinitime.clone() {
            self.stop_notifications_task();
            log::info!("Notification session started");
            let infinitime = infinitime.clone();
            self.task = Some(relm4::spawn(async move {
                if let Err(error) = notifications::run_notification_session(&infinitime).await {
                    if let Some(zbus::fdo::Error::AccessDenied(_)) = error.downcast_ref() {
                        log::warn!(
                            "Notification session failed: the app doesn't have permissions to monitor \
                            D-Bus session bus. If you're running it from flatpak, you can grant access with \
                            command: `flatpak override --socket=session-bus io.gitlab.azymohliad.WatchMate`, \
                            or via Flatseal"
                        );
                        ui::BROKER.send(ui::Input::ToastWithLink {
                            message: "Session bus permission is needed here",
                            label: "Details",
                            url: "https://github.com/azymohliad/watchmate/issues/6",
                        });
                    } else {
                        log::warn!("Notifications session failed: {error}");
                        ui::BROKER.send(ui::Input::ToastStatic("Notification session failed"));
                    }
                }
                sender.input(Input::NotificationSessionEnded);
            }));
        }
    }

    fn stop_notifications_task(&mut self) {
        // TODO: Is it safe to abort, or does it makes sense to
        // hook up a message channel to finish gracefully?
        if self.task.take().map(|h| h.abort()).is_some() {
            log::info!("Notification session stopped");
        }
    }
}


#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
    type Init = gio::Settings;
    type Input = Input;
    type Output = ();
    type Widgets = Widgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_margin_all: 12,
            set_spacing: 10,

            gtk::Label {
                set_label: "Notifications",
                set_halign: gtk::Align::Start,
            },

            #[name = "switch"]
            gtk::Switch {
                #[watch]
                set_state: model.is_enabled && model.task.is_some(),
                set_halign: gtk::Align::End,
                set_hexpand: true,
                connect_active_notify[sender] => move |switch| {
                    sender.input(Input::SetNotificationSession(switch.is_active()));
                }
            }
        }
    }

    fn init(settings: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let is_enabled = settings.boolean(ui::SETTING_NOTIFICATIONS);
        let model = Self { is_enabled, ..Default::default() };
        let widgets = view_output!();
        settings.bind(ui::SETTING_NOTIFICATIONS, &widgets.switch, "active").build();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::Device(infinitime) => {
                self.infinitime = infinitime;
                match self.infinitime {
                    Some(_) if self.is_enabled => self.start_notifications_task(sender),
                    Some(_) => {},
                    None => self.stop_notifications_task(),
                }
            }
            Input::SetNotificationSession(state) => {
                self.is_enabled = state;
                match state {
                    true => self.start_notifications_task(sender),
                    false => self.stop_notifications_task(),
                }
            }
            Input::NotificationSessionEnded => {
                self.task = None;
            }
        }
    }
}

