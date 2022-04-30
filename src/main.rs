use std::sync::Arc;
use tokio::{runtime::Runtime, sync::Notify};
use adw::prelude::AdwApplicationWindowExt;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{send, adw, Sender, Widgets, WidgetPlus, Model, AppUpdate, RelmApp, factory::{FactoryPrototype, FactoryVecDeque, DynamicIndex}};

mod ble;

#[derive(Debug)]
pub enum AppMsg {
    ScannerToggled,
    DeviceAdded(DeviceInfo),
    DeviceRemoved(bluer::Address),
    DeviceSelected(i32),
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub address: bluer::Address,
    pub name: Option<String>,
    pub rssi: Option<i16>,
}

#[relm4::factory_prototype(pub)]
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
    notifier: Arc<Notify>,
    is_discovering: bool,
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
            AppMsg::ScannerToggled => {
                self.is_discovering = !self.is_discovering;
                if self.is_discovering {
                    self.devices.clear();
                    let notifier = self.notifier.clone();
                    self.rt.spawn(async {
                        ble::scan(notifier, sender).await.unwrap();
                    });
                } else {
                    self.notifier.notify_one();
                }
            }
            AppMsg::DeviceAdded(info) => {
                self.devices.push_front(info);
            }
            AppMsg::DeviceRemoved(address) => {
                if let Some((index, _)) = self.devices.iter().enumerate().find(|(_, d)| d.address == address) {
                    self.devices.remove(index);
                }
            }
            AppMsg::DeviceSelected(index) => {
                let device = self.devices.get(index as usize);
                println!("Selected device: {:?}", device);

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
                        set_label: watch!(if model.is_discovering { "Stop" } else { "Start" }),
                        connect_clicked(sender) => move |_| {
                            send!(sender, AppMsg::ScannerToggled);
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
    let notifier = Arc::new(Notify::new());
    let model = AppModel {
        rt,
        notifier,
        devices: FactoryVecDeque::new(),
        is_discovering: false,
    };
    let app = RelmApp::new(model);
    app.run();
}
