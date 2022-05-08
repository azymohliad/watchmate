use tokio::runtime::Runtime;
use adw::prelude::AdwApplicationWindowExt;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{send, adw, Sender, WidgetPlus, AppUpdate, RelmApp, RelmComponent, factory::{FactoryPrototype, FactoryVecDeque, DynamicIndex}};
use anyhow::Result;

use crate::bt;

mod watch;

#[derive(Debug)]
enum Message {
    SetView(View),
    ScanToggled,
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceSelected(i32),
    DeviceConnected(bluer::Address),
    Notification(String)
}

#[derive(Debug)]
struct DeviceInfo {
    address: bluer::Address,
    alias: String,
    rssi: Option<i16>,
    connected: bool,
}

impl DeviceInfo {
    async fn new(device: &bluer::Device) -> Result<Self> {
        Ok(Self {
            address: device.address(),
            alias: device.alias().await?,
            rssi: device.rssi().await?,
            connected: device.is_connected().await?,
        })
    }

    async fn new_filtered(device: &bluer::Device) -> Option<Self> {
        if bt::InfiniTime::check_device(device).await {
            Self::new(device).await.ok()
        } else {
            None
        }
    }
}

#[relm4::factory_prototype]
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

#[derive(Debug, PartialEq)]
enum View {
    Main,
    Scan,
}

struct Components {
    watch: RelmComponent<watch::Model, Model>,
}

impl relm4::Components<Model> for Components {
    fn init_components(parent_model: &Model, parent_sender: Sender<Message>) -> Self {
        Self {
            watch: RelmComponent::new(parent_model, parent_sender),
        }
    }

    fn connect_parent(&mut self, parent_widgets: &Widgets) {
        self.watch.connect_parent(parent_widgets);
    }
}

struct Model {
    // UI state
    active_view: View,
    devices: FactoryVecDeque<DeviceInfo>,
    watch: Option<String>,
    // Non-UI state
    runtime: Runtime,
    adapter: bluer::Adapter,
    scanner: bt::Scanner,
    toast_overlay: adw::ToastOverlay,
}

impl Model {
    fn notify(&self, message: &str) {
        self.toast_overlay.add_toast(&adw::Toast::new(message));
    }
}

impl relm4::Model for Model {
    type Msg = Message;
    type Widgets = Widgets;
    type Components = Components;
}

impl AppUpdate for Model {
    fn update(&mut self, msg: Message, components: &Components, sender: Sender<Message>) -> bool {
        match msg {
            Message::SetView(view) => {
                self.active_view = view;
            }
            Message::ScanToggled => {
                if !self.scanner.is_scanning() {
                    self.devices.clear();
                    self.scanner.start(self.adapter.clone(), &self.runtime, move |event| {
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
            Message::DeviceAdded(address) => {
                if let Ok(device) = self.adapter.device(address) {
                    if let Some(info) = self.runtime.block_on(DeviceInfo::new_filtered(&device)) {
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
                    match self.adapter.device(info.address) {
                        Ok(device) => {
                            let address = info.address;
                            self.runtime.spawn(async move {
                                match device.connect().await {
                                    Ok(()) => sender.send(Message::DeviceConnected(address)).unwrap(),
                                    Err(error) => eprintln!("Error: {}", error),
                                }
                            });
                        }
                        Err(error) => self.notify(&format!("Error: {}", error)),
                    }
                }
            }
            Message::DeviceConnected(address) => {
                println!("Connected: {}", address.to_string());
                self.active_view = View::Main;
                match self.adapter.device(address) {
                    Ok(device) => components.watch.send(watch::Message::Connected(device)).unwrap(),
                    Err(error) => self.notify(&format!("Error: {}", error)),
                }
            }
            Message::Notification(message) => {
                self.notify(&message);
            }
        }
        true
    }
}

#[relm4::widget]
impl relm4::Widgets<Model, ()> for Widgets {
    view! {
        adw::ApplicationWindow {
            set_default_width: 480,
            set_default_height: 720,
            set_content = Some(&gtk::Box) {
                set_orientation: gtk::Orientation::Vertical,
                append = &adw::HeaderBar {
                    set_title_widget = Some(&gtk::Box) {
                        set_margin_all: 5,
                        set_orientation: gtk::Orientation::Vertical,
                        append = &gtk::Label {
                            set_label: watch!(match &model.watch {
                                Some(alias) => &alias,
                                None => "WatchMate",
                            }),
                        },
                        append = &gtk::Label {
                            set_label: watch!(if model.watch.is_some() {
                                "Connected"
                            } else {
                                "Not connected"
                            }),
                            add_css_class: "dim-label",
                        },
                    },
                    pack_start = &gtk::Button {
                        set_label: "Back",
                        set_icon_name: "go-previous-symbolic",
                        set_visible: watch!(model.active_view != View::Main),
                        connect_clicked(sender) => move |_| {
                            send!(sender, Message::SetView(View::Main));
                        },
                    },
                    pack_start = &gtk::Button {
                        set_label: "Devices",
                        set_icon_name: watch!(if model.watch.is_some() {
                            "bluetooth-symbolic"
                        } else {
                            "bluetooth-disconnected-symbolic"
                        }),
                        set_visible: watch!(model.active_view != View::Scan),
                        connect_clicked(sender) => move |_| {
                            send!(sender, Message::SetView(View::Scan));
                        },
                    },
                },
                append = &Clone::clone(&model.toast_overlay) -> adw::ToastOverlay {
                    set_child = Some(&gtk::Stack) {
                        add_named(Some("main_view")) = &adw::Clamp {
                            set_maximum_size: 400,
                            // set_visible: watch!(components.watch.model.device.is_some()),
                            set_child: Some(components.watch.root_widget()),
                        },
                        add_named(Some("scan_view")) = &adw::Clamp {
                            set_maximum_size: 400,
                            set_child = Some(&gtk::Box) {
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
                            },
                        },
                        set_visible_child_name: watch!(match model.active_view {
                            View::Main => "main_view",
                            View::Scan => "scan_view",
                        }),
                    },
                },
            },
        }
    }

    fn post_init() {
        // Automatic device selection logic
        if let Some(device) = model.devices.iter().find(|d| d.connected) {
            // If suitable device is already connected - just report it as connected
            send!(sender, Message::DeviceConnected(device.address));
            println!("InfiniTime ({}) is already connected", device.address.to_string());
        } else {
            if model.devices.is_empty() {
                // If no suitable devices are known - start scanning automatically
                send!(sender, Message::ScanToggled);
                println!("No InfiniTime devices are known. Scanning...");
            } else if model.devices.len() == 1 {
                // If only one suitable device is known - try to connect to it automatically
                send!(sender, Message::DeviceSelected(0));
                let address = model.devices.get(0).unwrap().address;
                println!("Trying to connect to InfiniTime ({})", address.to_string());
            } else {
                println!("Multiple InfiniTime devices are known. Waiting for the user to select");
            }
        }
    }
}


pub fn run(runtime: Runtime, adapter: bluer::Adapter) {
    // Read saved bluetooth devices
    let known_devices = runtime.block_on(async {
        let mut result = FactoryVecDeque::new();
        for address in adapter.device_addresses().await? {
            let device = adapter.device(address)?;
            if let Some(info) = DeviceInfo::new_filtered(&device).await {
                result.push_back(info);
            }
        }
        Ok(result) as Result<FactoryVecDeque<DeviceInfo>>
    }).unwrap();

    // Init GTK before libadwaita (ToastOverlay)
    gtk::init().unwrap();

    // Init model
    let model = Model {
        // UI state
        active_view: View::Scan,
        devices: known_devices,
        watch: None,
        // System
        runtime,
        adapter,
        scanner: bt::Scanner::new(),
        // Widget handles
        toast_overlay: adw::ToastOverlay::new(),
    };

    // Run app
    let app = RelmApp::new(model);
    app.run();
}
