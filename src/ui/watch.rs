use std::path::PathBuf;
use tokio::runtime;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use adw::prelude::{PreferencesRowExt, ExpanderRowExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, SimpleComponent, WidgetPlus};
use anyhow::Result;

use crate::bt;

struct Watch {
    device: bt::InfiniTime,
    battery_level: u8,
    heart_rate: u8,
    firmware_version: String,
}

impl Watch {
    async fn new(device: bluer::Device) -> Result<Self> {
        let device = bt::InfiniTime::new(device).await?;
        let battery_level = device.read_battery_level().await?;
        let heart_rate = device.read_heart_rate().await?;
        let firmware_version = device.read_firmware_version().await?;
        Ok(Self {
            device,
            battery_level,
            heart_rate,
            firmware_version,
        })
    }
}

#[derive(Debug)]
pub enum Input {
    Connected(bluer::Device),
    HeartRateUpdate(u8),
    FirmwareUpdate(PathBuf),
}

#[derive(Debug)]
pub enum Output {
    OpenFileDialog,
    Notification(String),
}

pub struct Model {
    runtime: runtime::Handle,
    watch: Option<Watch>,
}

#[relm4::component(pub)]
impl SimpleComponent for Model {
    type InitParams = runtime::Handle;
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
                            set_label: match &model.watch {
                                Some(watch) => format!("{}%", watch.battery_level),
                                None => String::from("Unavailable"),
                            }.as_str(),
                            add_css_class: "dim-label",
                        },
                        append = &gtk::LevelBar {
                            set_min_value: 0.0,
                            set_max_value: 100.0,
                            #[watch]
                            set_value: match &model.watch {
                                Some(watch) => watch.battery_level as f64,
                                None => 0.0,
                            },
                            #[watch]
                            set_visible: model.watch.is_some(),
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
                            set_label: match &model.watch {
                                Some(watch) => format!("{} BPM", watch.heart_rate),
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
                            set_label: match &model.watch {
                                Some(watch) => watch.device.get_alias(),
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
                            set_label: match &model.watch {
                                Some(watch) => watch.device.get_address().to_string(),
                                None => String::from("Unavailable"),
                            }.as_str(),
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
                        set_label: match &model.watch {
                            Some(watch) => watch.firmware_version.as_str(),
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
                                    // let filepath = PathBuf::from("/home/azymohliad/Downloads/OS/pinetime-mcuboot-app-dfu-1.10.0.zip");
                                    // sender.input(Input::FirmwareUpdate(filepath));
                                    sender.output(Output::OpenFileDialog);
                                },
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(runtime: Self::InitParams, root: &Self::Root, sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self { runtime, watch: None };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        match msg {
            Input::Connected(device) => {
                self.watch = self.runtime.block_on(Watch::new(device)).ok();

                if let Some(watch) = self.watch.as_mut() {
                    let send = sender.clone();
                    watch.device.start_notification_session(self.runtime.clone(), move |notification| {
                        match notification {
                            bt::Notification::HeartRate(value) => send.input(Input::HeartRateUpdate(value)),
                        }
                    })
                }
            }
            Input::HeartRateUpdate(value) => {
                if let Some(watch) = self.watch.as_mut() {
                    watch.heart_rate = value;
                }
            }
            Input::FirmwareUpdate(filename) => {
                if let Some(watch) = &self.watch {
                    let res = self.runtime.block_on(watch.device.firmware_upgrade(filename.as_path()));
                    if let Err(error) = res {
                        sender.output(Output::Notification(String::from("Firmware update failed...")));
                        eprintln!("Firmware update failed: {}", error);
                    }
                }
            }
        }
    }
}

