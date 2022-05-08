use std::{collections::VecDeque, sync::Arc};
use tokio::runtime;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{
    send, adw, factory::{FactoryPrototype, FactoryVecDeque, DynamicIndex},
    ComponentUpdate, Sender, WidgetPlus
};

use crate::bt;


pub enum Message {
    ScanToggled,
    KnownDevices(VecDeque<DeviceInfo>),
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceSelected(i32),
}

pub struct Model {
    // UI State
    devices: FactoryVecDeque<DeviceInfo>,
    // Non-UI
    runtime: runtime::Handle,
    adapter: Arc<bluer::Adapter>,
    scanner: bt::Scanner,
}

impl relm4::Model for Model {
    type Msg = Message;
    type Widgets = Widgets;
    type Components = ();
}

impl ComponentUpdate<super::Model> for Model {
    fn init_model(parent: &super::Model) -> Self {
        Self {
            devices: FactoryVecDeque::new(),
            runtime: parent.runtime.handle().clone(),
            adapter: parent.adapter.clone(),
            scanner: bt::Scanner::new(),
        }
    }

    fn update(&mut self, msg: Message, _components: &(), sender: Sender<Message>, parent_sender: Sender<super::Message>) {
        match msg {
            Message::ScanToggled => {
                if !self.scanner.is_scanning() {
                    self.devices.clear();
                    self.scanner.start(self.adapter.clone(), self.runtime.clone(), move |event| {
                        match event {
                            bluer::AdapterEvent::DeviceAdded(address) => {
                                sender.send(Message::DeviceAdded(address)).unwrap();
                            }
                            bluer::AdapterEvent::DeviceRemoved(address) => {
                                sender.send(Message::DeviceRemoved(address)).unwrap();
                            }
                            _ => (),
                        }
                    });
                } else {
                    self.scanner.stop();
                }
            }
            Message::KnownDevices(devices) => {
                self.devices = FactoryVecDeque::from_vec_deque(devices);
                // Automatic device selection logic
                if let Some(device) = self.devices.iter().find(|d| d.connected) {
                    // If suitable device is already connected - just report it as connected
                    parent_sender.send(super::Message::DeviceConnected(device.address)).unwrap();
                    println!("InfiniTime ({}) is already connected", device.address.to_string());
                } else {
                    if self.devices.is_empty() {
                        // If no suitable devices are known - start scanning automatically
                        sender.send(Message::ScanToggled).unwrap();
                        println!("No InfiniTime devices are known. Scanning...");
                    } else if self.devices.len() == 1 {
                        // If only one suitable device is known - try to connect to it automatically
                        let address = self.devices.get(0).unwrap().address;
                        parent_sender.send(super::Message::DeviceSelected(address)).unwrap();
                        println!("Trying to connect to InfiniTime ({})", address.to_string());
                    } else {
                        println!("Multiple InfiniTime devices are known. Waiting for the user to select");
                    }
                }
            }
            Message::DeviceAdded(address) => {
                if let Ok(device) = self.adapter.device(address) {
                    if let Some(info) = self.runtime.block_on(async {
                        if bt::InfiniTime::check_device(&device).await {
                            DeviceInfo::new(&device).await.ok()
                        } else {
                            None
                        }
                    }) {
                        self.devices.push_front(info);
                    }
                }
            }
            Message::DeviceRemoved(address) => {
                if let Some((index, _)) = self.devices.iter().enumerate().find(|(_, d)| d.address == address) {
                    self.devices.remove(index);
                }
            }
            Message::DeviceSelected(index) => {
                if let Some(info) = self.devices.get(index as usize) {
                    println!("Selected device: {:?}", info);
                    parent_sender.send(super::Message::DeviceSelected(info.address)).unwrap();
                }
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
            append = &gtk::Button {
                set_child = Some(&adw::ButtonContent) {
                    set_label: watch!(if model.scanner.is_scanning() {
                        "Scanning..."
                    } else {
                        "Scan"
                    }),
                    set_icon_name: watch!(if model.scanner.is_scanning() {
                        "bluetooth-acquiring-symbolic"
                    } else {
                        "bluetooth-symbolic"
                    }),
                },
                connect_clicked(sender) => move |_| {
                    send!(sender, Message::ScanToggled);
                },
            },
            append = &gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vexpand: true,
                set_child = Some(&gtk::ListBox) {
                    // set_margin_all: 5,
                    set_valign: gtk::Align::Start,
                    add_css_class: "boxed-list",
                    factory!(model.devices),
                    connect_row_activated(sender) => move |_, row| {
                        send!(sender, Message::DeviceSelected(row.index()))
                    }
                },
            },
        }
    }

    fn post_init() {
        // Read known devices list
        let adapter = model.adapter.clone();
        model.runtime.spawn(async move {
            let mut devices = VecDeque::new();
            for device in bt::InfiniTime::list_known_devices(&adapter).await.unwrap() {
                devices.push_back(DeviceInfo::new(&device).await.unwrap())
            }
            send!(sender, Message::KnownDevices(devices));
        });
    }
}

#[derive(Debug)]
pub struct DeviceInfo {
    address: bluer::Address,
    alias: String,
    rssi: Option<i16>,
    connected: bool,
}

impl DeviceInfo {
    async fn new(device: &bluer::Device) -> bluer::Result<Self> {
        Ok(Self {
            address: device.address(),
            alias: device.alias().await?,
            rssi: device.rssi().await?,
            connected: device.is_connected().await?,
        })
    }
}

// Factory for device list
#[relm4::factory_prototype(pub)]
impl FactoryPrototype for DeviceInfo {
    type Factory = FactoryVecDeque<Self>;
    type Widgets = DeviceInfoWidgets;
    type View = gtk::ListBox;
    type Msg = Message;

    view! {
        gtk::ListBoxRow {
            set_child = Some(&gtk::Box) {
                set_orientation: gtk::Orientation::Vertical,
                append = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    append = &gtk::Label {
                        set_margin_all: 5,
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_label: &self.alias,
                    },
                    append = &gtk::Image {
                        set_margin_all: 5,
                        set_halign: gtk::Align::End,
                        set_hexpand: true,
                        set_icon_name: Some("emblem-default-symbolic"),
                        set_visible: watch!(self.connected),
                    }
                },
                append = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 5,
                    set_margin_all: 5,
                    append = &gtk::Label {
                        set_margin_all: 5,
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_label: &self.address.to_string(),
                        add_css_class: "dim-label",
                    },
                    append = &gtk::Label {
                        set_margin_all: 5,
                        set_halign: gtk::Align::End,
                        set_hexpand: true,
                        set_label: &match self.rssi {
                            Some(rssi) => format!("RSSI: {}", rssi),
                            None => String::from("Saved"),
                        },
                        add_css_class: "dim-label",
                    },
                },
            },
        }
    }

    fn position(&self, _index: &DynamicIndex) {}
}


