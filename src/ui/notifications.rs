use crate::inft::{bt, fdo::notifications};
use std::sync::Arc;
use gtk::prelude::{BoxExt, OrientableExt, WidgetExt};
use relm4::{gtk, ComponentParts, ComponentSender, Component, JoinHandle, RelmWidgetExt};


#[derive(Debug)]
pub enum Input {
    Device(Option<Arc<bt::InfiniTime>>),
    NotificationSessionStart,
    NotificationSessionStop,
    NotificationSessionEnded,
}

#[derive(Debug)]
pub enum Output {
}

#[derive(Default)]
pub struct Model {
    infinitime: Option<Arc<bt::InfiniTime>>,
    is_enabled: bool,
    task: Option<JoinHandle<()>>,
}


#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
    type Init = ();
    type Input = Input;
    type Output = Output;
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

            gtk::Switch {
                #[watch]
                set_state: model.is_enabled,
                set_halign: gtk::Align::End,
                set_hexpand: true,
                connect_active_notify[sender] => move |switch| {
                    if switch.is_active() {
                        sender.input(Input::NotificationSessionStart);
                    } else {
                        sender.input(Input::NotificationSessionStop);
                    }
                }
            }
        }
    }

    fn init(_: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self { is_enabled: true, ..Default::default() };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::Device(infinitime) => {
                self.infinitime = infinitime;
                if self.infinitime.is_some() && self.is_enabled {
                    sender.input(Input::NotificationSessionStart);
                }
            }
            Input::NotificationSessionStart => {
                if self.task.is_some() {
                    log::warn!("Notification session is already running");
                } else if let Some(infinitime) = &self.infinitime {
                    log::info!("Notification session starting...");
                    let infinitime = infinitime.clone();
                    self.task = Some(relm4::spawn(async move {
                        if let Err(error) = notifications::run_notification_session(&infinitime).await {
                            log::error!("Notifications session error: {error}");
                        }
                        sender.input(Input::NotificationSessionEnded);
                    }));
                    self.is_enabled = true;
                }
            }
            Input::NotificationSessionStop => {
                // TODO: Is it safe to abort, or does it makes sense to
                // hook up a message channel to finish gracefully?
                if self.task.take().map(|h| h.abort()).is_some() {
                    log::info!("Notification session stopped");
                }
                self.is_enabled = false;
            }
            Input::NotificationSessionEnded => {
                self.task = None;
                self.is_enabled = false;
            }
        }
    }
}

