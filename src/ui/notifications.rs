use crate::inft::{bt, fdo::notifications};
use std::sync::Arc;
use gtk::prelude::{BoxExt, OrientableExt, WidgetExt};
use relm4::{gtk, ComponentParts, ComponentSender, Component, JoinHandle, RelmWidgetExt};


#[derive(Debug)]
pub enum Input {
    Device(Option<Arc<bt::InfiniTime>>),
    SwitchProxy(bool),
    SessionEnded,
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

impl Model {
    fn start_proxy_task(&mut self, sender: ComponentSender<Self>) {
        if let Some(infinitime) = self.infinitime.clone() {
            self.stop_proxy_task();
            log::info!("Notification session started");
            let infinitime = infinitime.clone();
            self.task = Some(relm4::spawn(async move {
                if let Err(error) = notifications::run_notification_session(&infinitime).await {
                    log::error!("Notifications session error: {error}");
                }
                sender.input(Input::SessionEnded);
            }));
        }
    }

    fn stop_proxy_task(&mut self) {
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
                set_state: model.is_enabled,
                set_halign: gtk::Align::End,
                set_hexpand: true,
                connect_active_notify[sender] => move |switch| {
                    let state = switch.is_active();
                    sender.input(Input::SwitchProxy(state));
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
                match self.infinitime {
                    Some(_) if self.is_enabled => self.start_proxy_task(sender),
                    Some(_) => {},
                    None => self.stop_proxy_task(),
                }
            }
            Input::SwitchProxy(state) => {
                self.is_enabled = state;
                match state {
                    true => self.start_proxy_task(sender),
                    false => self.stop_proxy_task(),
                }
            }
            Input::SessionEnded => {
                self.task = None;
            }
        }
    }
}

