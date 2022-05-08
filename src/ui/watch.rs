use tokio::runtime;
use gtk::prelude::{BoxExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{ComponentUpdate, Sender, WidgetPlus};
use anyhow::Result;

use crate::bt;


pub enum Message {
    Connected(bluer::Device),

}

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


pub struct Model {
    runtime: runtime::Handle,
    watch: Option<Watch>,
}

impl relm4::Model for Model {
    type Msg = Message;
    type Widgets = Widgets;
    type Components = ();
}

impl ComponentUpdate<super::Model> for Model {
    fn init_model(parent: &super::Model) -> Self {
        Self {
            runtime: parent.runtime.handle().clone(),
            watch: None,
        }
    }

    fn update(&mut self, msg: Message, _components: &(), _sender: Sender<Message>, _parent_sender: Sender<super::Message>) {
        match msg {
            Message::Connected(device) => {
                self.watch = self.runtime.block_on(Watch::new(device)).ok();
            }
        }
    }
}


#[relm4::widget(pub)]
impl relm4::Widgets<Model, super::Model> for Widgets {
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
                    set_child = Some(&gtk::Box) {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Battery",
                        },
                        append = &gtk::Label {
                            set_label: watch!(match &model.watch {
                                Some(watch) => format!("{}%", watch.battery_level),
                                None => String::from("Unavailable"),
                            }.as_str()),
                            add_css_class: "dim-label",
                        },
                        append = &gtk::LevelBar {
                            set_min_value: 0.0,
                            set_max_value: 100.0,
                            set_value: watch!(match &model.watch {
                                Some(watch) => watch.battery_level as f64,
                                None => 0.0,
                            }),
                            set_visible: watch!(model.watch.is_some()),
                            set_hexpand: true,
                            set_valign: gtk::Align::Center,
                        },
                    },
                },
                append = &gtk::ListBoxRow {
                    set_selectable: false,
                    set_child = Some(&gtk::Box) {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Heart Rate",
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                        },
                        append = &gtk::Label {
                            set_label: watch!(match &model.watch {
                                Some(watch) => format!("{} BPM", watch.heart_rate),
                                None => String::from("Unavailable"),
                            }.as_str()),
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
                    set_child = Some(&gtk::Box) {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Firmware Version",
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                        },
                        append = &gtk::Label {
                            set_label: watch!(match &model.watch {
                                Some(watch) => watch.firmware_version.as_str(),
                                None => "Unavailable",
                            }),
                            add_css_class: "dim-label",
                            set_hexpand: true,
                            set_halign: gtk::Align::End,
                        },
                    },
                },
                append = &gtk::ListBoxRow {
                    set_selectable: false,
                    set_child = Some(&gtk::Box) {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 12,
                        set_spacing: 10,
                        append = &gtk::Label {
                            set_label: "Address",
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                        },
                        append = &gtk::Label {
                            set_label: watch!(match &model.watch {
                                Some(watch) => watch.device.get_address().to_string(),
                                None => String::from("Unavailable"),
                            }.as_str()),
                            add_css_class: "dim-label",
                            set_hexpand: true,
                            set_halign: gtk::Align::End,
                        },
                    },
                },
            },
        }
    }
}
