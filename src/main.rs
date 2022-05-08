use tokio::runtime::Runtime;
use adw::prelude::AdwApplicationWindowExt;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{send, adw, Sender, Widgets, WidgetPlus, Model, AppUpdate, RelmApp, factory::{FactoryPrototype, FactoryVecDeque, DynamicIndex}};
use anyhow::Result;

mod bt;

#[derive(Debug)]
enum AppMsg {
    SetView(AppView),
    ScanToggled,
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceSelected(i32),
    DeviceConnected(bluer::Address),
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
    type Msg = AppMsg;

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
enum AppView {
    Main,
    Scan,
}

struct AppModel {
    active_view: AppView,
    rt: Runtime,
    adapter: bluer::Adapter,
    scanner: bt::Scanner,
    devices: FactoryVecDeque<DeviceInfo>,
    infinitime: Option<bt::InfiniTime>,
    battery_level: u8,
    heart_rate: u8,
    firmware_version: String,
    toast_overlay: adw::ToastOverlay,
}

impl AppModel {
    fn notify(&self, message: &str) {
        self.toast_overlay.add_toast(&adw::Toast::new(message));
    }
}

impl Model for AppModel {
    type Msg = AppMsg;
    type Widgets = AppWidgets;
    type Components = ();
}

impl AppUpdate for AppModel {
    fn update(&mut self, msg: AppMsg, _components: &(), sender: Sender<AppMsg>) -> bool {
        match msg {
            AppMsg::SetView(view) => {
                self.active_view = view;
            }
            AppMsg::ScanToggled => {
                if !self.scanner.is_scanning() {
                    self.devices.clear();
                    self.scanner.start(self.adapter.clone(), &self.rt, move |event| {
                        match event {
                            bluer::AdapterEvent::DeviceAdded(address) => {
                                sender.send(AppMsg::DeviceAdded(address)).unwrap();
                            }
                            bluer::AdapterEvent::DeviceRemoved(address) => {
                                sender.send(AppMsg::DeviceRemoved(address)).unwrap();
                            }
                            _ => (),
                        }
                    });
                } else {
                    self.scanner.stop();
                }
            }
            AppMsg::DeviceAdded(address) => {
                if let Ok(device) = self.adapter.device(address) {
                    if let Some(info) = self.rt.block_on(DeviceInfo::new_filtered(&device)) {
                        self.devices.push_front(info);
                    }
                }
            }
            AppMsg::DeviceRemoved(address) => {
                if let Some((index, _)) = self.devices.iter().enumerate().find(|(_, d)| d.address == address) {
                    self.devices.remove(index);
                }
            }
            AppMsg::DeviceSelected(index) => {
                if let Some(info) = self.devices.get(index as usize) {
                    println!("Selected device: {:?}", info);
                    match self.adapter.device(info.address) {
                        Ok(device) => {
                            let address = info.address;
                            self.rt.spawn(async move {
                                match device.connect().await {
                                    Ok(()) => sender.send(AppMsg::DeviceConnected(address)).unwrap(),
                                    Err(error) => eprintln!("Error: {}", error),
                                }
                            });
                        }
                        Err(error) => self.notify(&format!("Error: {}", error)),
                    }
                }
            }
            AppMsg::DeviceConnected(address) => {
                println!("Connected: {}", address.to_string());
                self.active_view = AppView::Main;
                match self.adapter.device(address) {
                    Ok(device) => match self.rt.block_on(bt::InfiniTime::new(device)) {
                        Ok(infinitime) => {
                            self.notify("Connected");
                            self.battery_level = self.rt.block_on(infinitime.read_battery_level()).unwrap();
                            self.firmware_version = self.rt.block_on(infinitime.read_firmware_version()).unwrap();
                            self.heart_rate = self.rt.block_on(infinitime.read_heart_rate()).unwrap();
                            self.infinitime = Some(infinitime);
                        }
                        Err(error) => {
                            self.notify(&format!("Error: {}", error));
                        }
                    }
                    Err(error) => {
                        self.notify(&format!("Error: {}", error));
                    }
                }
            }
        }
        true
    }
}

#[relm4::widget]
impl Widgets<AppModel, ()> for AppWidgets {
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
                            set_label: watch!(if let Some(infinitime) = &model.infinitime {
                                infinitime.get_alias()
                            } else {
                                "WatchMate"
                            }),
                        },
                        append = &gtk::Label {
                            set_label: watch!(if let Some(_infinitime) = &model.infinitime {
                                "Connected" // TODO: Print bluetooth address instead
                            } else {
                                "Not connected"
                            }),
                            add_css_class: "dim-label",
                        },
                    },
                    pack_start = &gtk::Button {
                        set_label: "Back",
                        set_icon_name: "go-previous-symbolic",
                        set_visible: watch!(model.active_view != AppView::Main),
                        connect_clicked(sender) => move |_| {
                            send!(sender, AppMsg::SetView(AppView::Main));
                        },
                    },
                    pack_start = &gtk::Button {
                        set_label: "Devices",
                        set_icon_name: watch!(if model.infinitime.is_some() {
                            "bluetooth-symbolic"
                        } else {
                            "bluetooth-disconnected-symbolic"
                        }),
                        set_visible: watch!(model.active_view != AppView::Scan),
                        connect_clicked(sender) => move |_| {
                            send!(sender, AppMsg::SetView(AppView::Scan));
                        },
                    },
                },
                append = &Clone::clone(&model.toast_overlay) -> adw::ToastOverlay {
                    set_child = Some(&gtk::Stack) {
                        add_named(Some("main_view")) = &adw::Clamp {
                            set_maximum_size: 400,
                            set_visible: watch!(model.infinitime.is_some()),
                            set_child = Some(&gtk::Box) {
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
                                                set_label: watch!(&format!("{}%", model.battery_level)),
                                                add_css_class: "dim-label",
                                            },
                                            append = &gtk::LevelBar {
                                                set_min_value: 0.0,
                                                set_max_value: 100.0,
                                                set_value: watch!(model.battery_level as f64),
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
                                                set_label: watch!(&format!("{} BPM", model.heart_rate)),
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
                                                set_label: watch!(&model.firmware_version),
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
                                                set_label: watch!(&model.infinitime.as_ref().map(|d| d.get_address().to_string()).unwrap_or("".to_string())),
                                                add_css_class: "dim-label",
                                                set_hexpand: true,
                                                set_halign: gtk::Align::End,
                                            },
                                        },
                                    },
                                },
                            },
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
                                        send!(sender, AppMsg::ScanToggled);
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
                                            send!(sender, AppMsg::DeviceSelected(row.index()))
                                        }
                                    },
                                },
                            },
                        },
                        set_visible_child_name: watch!(match model.active_view {
                            AppView::Main => "main_view",
                            AppView::Scan => "scan_view",
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
            send!(sender, AppMsg::DeviceConnected(device.address));
            println!("InfiniTime ({}) is already connected", device.address.to_string());
        } else {
            if model.devices.is_empty() {
                // If no suitable devices are known - start scanning automatically
                send!(sender, AppMsg::ScanToggled);
                println!("No InfiniTime devices are known. Scanning...");
            } else if model.devices.len() == 1 {
                // If only one suitable device is known - try to connect to it automatically
                send!(sender, AppMsg::DeviceSelected(0));
                let address = model.devices.get(0).unwrap().address;
                println!("Trying to connect to InfiniTime ({})", address.to_string());
            } else {
                println!("Multiple InfiniTime devices are known. Waiting for the user to select");
            }
        }
    }
}

fn main() {
    gtk::init().unwrap();
    let rt = Runtime::new().unwrap();
    let adapter = rt.block_on(bt::init_adapter()).unwrap();
    let known_devices = rt.block_on(async {
        let mut result = FactoryVecDeque::new();
        for address in adapter.device_addresses().await? {
            let device = adapter.device(address)?;
            if let Some(info) = DeviceInfo::new_filtered(&device).await {
                result.push_back(info);
            }
        }
        Ok(result) as Result<FactoryVecDeque<DeviceInfo>>
    }).unwrap();

    let scanner = bt::Scanner::new();
    let model = AppModel {
        // Main UI model
        active_view: AppView::Scan,
        // Async runtime
        rt,
        // Bluetooth
        adapter,
        scanner,
        devices: known_devices,
        infinitime: None,
        battery_level: 0,
        heart_rate: 0,
        firmware_version: String::new(),
        // Widget handles
        toast_overlay: adw::ToastOverlay::new(),
    };
    let app = RelmApp::new(model);
    app.run();
}
