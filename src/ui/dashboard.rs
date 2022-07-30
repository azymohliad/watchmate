use std::sync::Arc;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use adw::prelude::{PreferencesRowExt, ExpanderRowExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component, Sender, WidgetPlus};
use anyhow::Result;
use crate::bt;

#[derive(Debug)]
pub enum Input {
    Connected(Arc<bt::InfiniTime>),
}

#[derive(Debug)]
pub enum Output {
    OpenFileDialog,
    Notification(String),
}

#[derive(Debug)]
pub enum CommandOutput {
    BatteryLevel(u8),
    HeartRate(u8),
    Alias(String),
    Address(String),
    FirmwareVersion(String),
}

#[derive(Default)]
pub struct Model {
    // UI state
    battery_level: Option<u8>,
    heart_rate: Option<u8>,
    alias: Option<String>,
    address: Option<String>,
    firmware_version: Option<String>,
    // Other
    infinitime: Option<Arc<bt::InfiniTime>>,
}

impl Model {
    async fn read_info(infinitime: Arc<bt::InfiniTime>, sender: Sender<CommandOutput>) -> Result<()> {
        sender.send(CommandOutput::Address(infinitime.device().address().to_string()));
        sender.send(CommandOutput::BatteryLevel(infinitime.read_battery_level().await?));
        sender.send(CommandOutput::HeartRate(infinitime.read_heart_rate().await?));
        sender.send(CommandOutput::Alias(infinitime.device().alias().await?));
        sender.send(CommandOutput::FirmwareVersion(infinitime.read_firmware_version().await?));
        Ok(())
    }
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
            append = &gtk::ListBox {
                set_valign: gtk::Align::Start,
                add_css_class: "boxed-list",
                append = &gtk::ListBoxRow {
                    set_selectable: false,
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Battery",
                        },
                        append = &gtk::Label {
                            #[watch]
                            set_label: match model.battery_level {
                                Some(soc) => format!("{}%", soc),
                                None => String::from("Unavailable"),
                            }.as_str(),
                            add_css_class: "dim-label",
                        },
                        append = &gtk::LevelBar {
                            set_min_value: 0.0,
                            set_max_value: 100.0,
                            #[watch]
                            set_value: match model.battery_level {
                                Some(soc) => soc as f64,
                                None => 0.0,
                            },
                            #[watch]
                            set_visible: model.battery_level.is_some(),
                            set_hexpand: true,
                            set_valign: gtk::Align::Center,
                        },
                    },
                },
                append = &gtk::ListBoxRow {
                    set_selectable: false,
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Heart Rate",
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                        },
                        append = &gtk::Label {
                            #[watch]
                            set_label: match model.heart_rate {
                                Some(rate) => format!("{} BPM", rate),
                                None => String::from("Unavailable"),
                            }.as_str(),
                            add_css_class: "dim-label",
                            set_hexpand: true,
                            set_halign: gtk::Align::End,
                        },
                    },
                },
            },
            append = &gtk::Label {
                set_label: "System Info",
                set_halign: gtk::Align::Start,
                set_margin_top: 20,
            },
            append = &gtk::ListBox {
                set_valign: gtk::Align::Start,
                add_css_class: "boxed-list",
                append = &gtk::ListBoxRow {
                    set_selectable: false,
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Name",
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                        },
                        append = &gtk::Label {
                            #[watch]
                            set_label: match &model.alias {
                                Some(alias) => alias,
                                None => "Unavailable",
                            },
                            add_css_class: "dim-label",
                            set_hexpand: true,
                            set_halign: gtk::Align::End,
                        },
                    },
                },
                append = &gtk::ListBoxRow {
                    set_selectable: false,
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Address",
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                        },
                        append = &gtk::Label {
                            #[watch]
                            set_label: match &model.address {
                                Some(address) => address,
                                None => "Unavailable",
                            },
                            add_css_class: "dim-label",
                            set_hexpand: true,
                            set_halign: gtk::Align::End,
                        },
                    },
                },
                append = &adw::ExpanderRow {
                    set_title: "Firmware Version",
                    add_action = &gtk::Label {
                        #[watch]
                        set_label: match &model.firmware_version {
                            Some(version) => version,
                            None => "Unavailable",
                        },
                        add_css_class: "dim-label",
                    },
                    add_row = &gtk::ListBoxRow {
                        set_selectable: false,
                        #[wrap(Some)]
                        set_child = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_margin_all: 12,
                            set_spacing: 10,
                            append = &gtk::Button {
                                set_label: "Update",
                                connect_clicked[sender] => move |_| {
                                    sender.output(Output::OpenFileDialog);
                                },
                            },
                        },
                    },
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
            Input::Connected(infinitime) => {
                self.infinitime = Some(infinitime.clone());
                sender.command(move |out, shutdown| {
                    // TODO: Remove this extra clone once ComponentSender::command
                    // is patched to accept FnOnce instead of Fn
                    let infinitime = infinitime.clone();
                    shutdown.register(async move {
                        // Read inital data
                        if let Err(error) = Self::read_info(infinitime.clone(), out.clone()).await {
                            eprintln!("Failed to read info: {}", error);
                        }
                        // Run data update session
                        infinitime.run_notification_session(move |notification| {
                            match notification {
                                bt::Notification::HeartRate(value) => out.send(CommandOutput::HeartRate(value)),
                            }
                        }).await;
                    }).drop_on_shutdown()
                });
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, _sender: &ComponentSender<Self>) {
        match msg {
            CommandOutput::BatteryLevel(soc) => self.battery_level = Some(soc),
            CommandOutput::HeartRate(rate) => self.heart_rate = Some(rate),
            CommandOutput::Alias(alias) => self.alias = Some(alias),
            CommandOutput::Address(address) => self.address = Some(address),
            CommandOutput::FirmwareVersion(version) => self.firmware_version = Some(version),
        }
    }
}

