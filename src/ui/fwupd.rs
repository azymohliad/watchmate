use std::{sync::Arc, path::PathBuf};
use gtk::prelude::{BoxExt, OrientableExt, WidgetExt};
use relm4::{gtk, ComponentParts, ComponentSender, Component, WidgetPlus};

use crate::bt;

#[derive(Debug)]
pub enum Input {
    FirmwareUpdate(PathBuf, Arc<bt::InfiniTime>),
}

#[derive(Debug)]
pub enum Output {
}

#[derive(Debug)]
pub enum CommandOutput {
    UpdateSuccessful,
    UpdateFailed,
    Message(String),
    Progress(u32, u32),
}

pub struct Model {
    message: String,
    sent: u32,
    total: u32,
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = CommandOutput;
    type InitParams = ();
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_margin_all: 12,
            set_spacing: 10,

            gtk::LevelBar {
                set_min_value: 0.0,
                #[watch]
                set_max_value: model.total as f64,
                #[watch]
                set_value: model.sent as f64,
            },

            gtk::Label {
                #[watch]
                set_label: &model.message,
                set_halign: gtk::Align::Start,
                set_margin_top: 20,
            },
        }
    }

    fn init(_: Self::InitParams, root: &Self::Root, _sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self { message: String::new(), sent: 0, total: 0 };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        match msg {
            Input::FirmwareUpdate(filename, infinitime) => {
                sender.command(move |out, shutdown| {
                    // TODO: Remove these extra clones once ComponentSender::command
                    // is patched to accept FnOnce instead of Fn
                    let infinitime = infinitime.clone();
                    let filename = filename.clone();
                    let sender = out.clone();
                    let callback = move |notification| match notification {
                        bt::FwUpdNotification::Extracted => {
                            sender.send(CommandOutput::Message(format!("Firmware files extracted")));
                        }
                        bt::FwUpdNotification::Initiated => {
                            sender.send(CommandOutput::Message(format!("Firmware update initiated")));
                        }
                        bt::FwUpdNotification::BytesSent(sent, total) => {
                            sender.send(CommandOutput::Progress(sent, total));
                        }
                    };
                    let task = async move {
                        match infinitime.firmware_upgrade(filename.as_path(), callback).await {
                            Ok(()) => out.send(CommandOutput::UpdateSuccessful),
                            Err(_error) => out.send(CommandOutput::UpdateFailed),
                        }
                    };
                    shutdown.register(task).drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, _sender: &ComponentSender<Self>) {
        match msg {
            CommandOutput::UpdateSuccessful => {
                self.message = format!("Update suffessful :)");
            }
            CommandOutput::UpdateFailed => {
                self.message = format!("Update failed :(");
            }
            CommandOutput::Message(text) => {
                self.message = text;
            }
            CommandOutput::Progress(sent, total) => {
                self.message = format!("Sending firmware to InfiniTime");
                self.sent = sent;
                self.total = total;
            }
        }
    }
}

