use std::{sync::Arc, path::PathBuf};
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component, WidgetPlus, JoinHandle};

use crate::bt;

#[derive(Debug)]
pub enum Input {
    Init(PathBuf, Arc<bt::InfiniTime>),
    Start,
    Abort,
    Finished,
    Failed,
    Message(&'static str),
    Progress(u32, u32),
}

#[derive(Debug)]
pub enum Output {
    SetView(super::View),
}

#[derive(Default)]
pub struct Model {
    message: String,
    sent_size: u32,
    total_size: u32,
    state: State,

    context: Option<(Arc<PathBuf>, Arc<bt::InfiniTime>)>,
    handle: Option<JoinHandle<()>>,
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
    type InitParams = ();
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    view! {
        gtk::Box {
            set_hexpand: true,
            set_orientation: gtk::Orientation::Vertical,

            adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &gtk::Label {
                    set_label: "Firmware Update",
                },

                pack_start = &gtk::Button {
                    set_tooltip_text: Some("Back"),
                    set_icon_name: "go-previous-symbolic",
                    #[watch]
                    set_visible: model.state != State::InProgress,
                    connect_clicked[sender] => move |_| {
                        sender.output(Output::SetView(super::View::Dashboard));
                    },
                },
            },

            adw::Clamp {
                set_maximum_size: 400,

                gtk::CenterBox {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 12,
                    set_vexpand: true,

                    #[wrap(Some)]
                    set_center_widget = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 10,

                        gtk::Label {
                            #[watch]
                            set_label: &model.message,
                            set_halign: gtk::Align::Center,
                            set_margin_top: 20,
                        },

                        gtk::LevelBar {
                            set_min_value: 0.0,
                            #[watch]
                            set_max_value: model.total_size as f64,
                            #[watch]
                            set_value: model.sent_size as f64,
                            #[watch]
                            set_visible: model.state == State::InProgress,
                        },
                    },

                    #[wrap(Some)]
                    set_end_widget = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 10,
                        set_halign: gtk::Align::Center,

                        gtk::Button {
                            set_label: "Abort",
                            add_css_class:"destructive-action",
                            #[watch]
                            set_visible: model.state == State::InProgress,
                            connect_clicked[sender] => move |_| sender.input(Input::Abort),
                        },

                        gtk::Button {
                            set_label: "Retry",
                            #[watch]
                            set_visible: model.state == State::Aborted,
                            connect_clicked[sender] => move |_| sender.input(Input::Start),
                        },

                        gtk::Button {
                            set_label: "Back",
                            #[watch]
                            set_visible: model.state != State::InProgress,
                            connect_clicked[sender] => move |_| {
                                sender.output(Output::SetView(super::View::Dashboard));
                            },
                        },
                    }
                },
            },
        }
    }

    fn init(_: Self::InitParams, root: &Self::Root, sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self::default();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        match msg {
            Input::Init(filename, infinitime) => {
                self.context = Some((Arc::new(filename), infinitime.clone()));
                sender.input(Input::Start);
            }
            Input::Start => {
                self.state = State::InProgress;
                let sender = sender.clone();
                if let Some((filename, infinitime)) = &self.context {
                    let filename = filename.clone();
                    let infinitime = infinitime.clone();
                    self.handle = Some(relm4::spawn(async move {
                        let snd = sender.clone();
                        let callback = move |notification| match notification {
                            bt::FwUpdNotification::Message(text) => {
                                snd.input(Input::Message(text));
                            }
                            bt::FwUpdNotification::BytesSent(sent, total) => {
                                snd.input(Input::Progress(sent, total));
                            }
                        };
                        match infinitime.firmware_upgrade(filename.as_path(), callback).await {
                            Ok(()) => sender.input(Input::Finished),
                            Err(_error) => sender.input(Input::Failed),
                        };
                    }));
                }
            }
            Input::Abort => {
                if let Some(handle) = self.handle.take() {
                    handle.abort();
                    self.message = format!("Firmware update aborted");
                    self.state = State::Aborted;
                }
            }
            Input::Finished => {
                self.message = format!("Firmware update complete :)");
                self.state = State::Finished;
                self.handle = None;
                self.context = None;
            }
            Input::Failed => {
                self.message = format!("Firmware update failed :(");
                self.state = State::Aborted;
                self.handle = None;
            }
            Input::Message(text) => {
                self.message = text.to_string();
            }
            Input::Progress(sent, total) => {
                self.message = format!("Sending firmware: {:.01}/{:.01} kB", sent as f32 / 1024.0, total as f32 / 1024.0);
                self.sent_size = sent;
                self.total_size = total;
            }
        }
    }
}

#[derive(PartialEq, Default)]
pub enum State {
    #[default]
    Ready,
    InProgress,
    Finished,
    Aborted,
}
