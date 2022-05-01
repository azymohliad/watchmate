use tokio::runtime::Runtime;
use adw::prelude::AdwApplicationWindowExt;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{send, adw, Sender, Widgets, WidgetPlus, Model, AppUpdate, RelmApp, factory::{FactoryPrototype, FactoryVecDeque, DynamicIndex}};

mod bt;

#[derive(Debug)]
enum AppMsg {
    ScanToggled,
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceSelected(i32),
    DeviceConnected(bluer::Address),
}

#[derive(Debug)]
struct DeviceInfo {
    address: bluer::Address,
    name: Option<String>,
    rssi: Option<i16>,
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
                append = &gtk::Label {
                    set_margin_all: 5,
                    set_label: self.name.as_ref().unwrap_or(&"Unknown Device".to_string()),
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
                        set_margin_end: 5,
                        set_halign: gtk::Align::End,
                        set_hexpand: true,
                        set_label: &match self.rssi {
                            Some(rssi) => format!("RSSI: {}", rssi),
                            None => String::from("Unreachable"),
                        },
                        add_css_class: "dim-label",
                    }
                }
            }
        }
    }

    fn position(&self, _index: &DynamicIndex) {}
}

struct AppModel {
    rt: Runtime,
    bt: bt::Host,
    devices: FactoryVecDeque<DeviceInfo>
}

impl Model for AppModel {
    type Msg = AppMsg;
    type Widgets = AppWidgets;
    type Components = ();
}

impl AppUpdate for AppModel {
    fn update(&mut self, msg: AppMsg, _components: &(), sender: Sender<AppMsg>) -> bool {
        match msg {
            AppMsg::ScanToggled => {
                if !self.bt.is_scanning() {
                    self.devices.clear();
                    self.bt.scan_start(&self.rt, move |event| {
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
                    self.bt.scan_stop();
                }
            }
            AppMsg::DeviceAdded(address) => {
                if let Ok(device) = self.bt.device(address) {
                    let info = self.rt.block_on(async {
                        DeviceInfo {
                            address,
                            name: device.name().await.unwrap(),
                            rssi: device.rssi().await.unwrap(),
                        }
                    });
                    self.devices.push_front(info);
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
                    match self.bt.device(info.address) {
                        Ok(device) => {
                            let address = info.address;
                            self.rt.spawn(async move {
                                match device.connect().await {
                                    Ok(()) => sender.send(AppMsg::DeviceConnected(address)).unwrap(),
                                    Err(error) => eprintln!("Error: {}", error),
                                }
                            });
                        }
                        Err(error) => eprintln!("Error: {}", error),
                    }
                }
            }
            AppMsg::DeviceConnected(address) => {
                println!("Connected: {}", address.to_string());
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
                    set_title_widget = Some(&gtk::Label) {
                        set_label: "WatchMate",
                    }
                },
                append = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 5,
                    set_spacing: 5,

                    append = &gtk::Button {
                        set_label: watch!(if model.bt.is_scanning() { "Stop" } else { "Start" }),
                        connect_clicked(sender) => move |_| {
                            send!(sender, AppMsg::ScanToggled);
                        },
                    },

                    append = &gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_vexpand: true,
                        set_child = Some(&adw::Clamp) {
                            set_maximum_size: 400,
                            set_child = Some(&gtk::ListBox) {
                                set_margin_all: 5,
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
            },
        }
    }
}

fn main() {
    let rt = Runtime::new().unwrap();
    let bt = rt.block_on(async { bt::Host::new().await }).unwrap();
    let model = AppModel {
        rt,
        bt,
        devices: FactoryVecDeque::new(),
    };
    let app = RelmApp::new(model);
    app.run();
}
