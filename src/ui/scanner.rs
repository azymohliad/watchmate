use std::sync::Arc;
use tokio::runtime;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{
    adw, gtk,
    factory::{FactoryComponent, FactoryVecDeque, DynamicIndex},
    ComponentParts, ComponentSender, Sender, WidgetPlus, SimpleComponent
};

use crate::bt;


#[derive(Debug)]
pub enum Input {
    ScanToggled,
    KnownDevices(Vec<DeviceInfo>),
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceSelected(i32),
}

#[derive(Debug)]
pub enum Output {
    DeviceSelected(bluer::Address),
    DeviceConnected(bluer::Address),
}

pub struct Model {
    // UI State
    devices: FactoryVecDeque<gtk::ListBox, DeviceInfo, Input>,
    // Non-UI
    runtime: runtime::Handle,
    adapter: Arc<bluer::Adapter>,
    scanner: bt::Scanner,
}

#[relm4::component(pub)]
impl SimpleComponent for Model {
    type InitParams = (runtime::Handle, Arc<bluer::Adapter>);
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_margin_all: 12,
            set_spacing: 10,
            append = &gtk::Button {
                #[wrap(Some)]
                set_child = &adw::ButtonContent {
                    #[watch]
                    set_label: if model.scanner.is_scanning() {
                        "Scanning..."
                    } else {
                        "Scan"
                    },
                    #[watch]
                    set_icon_name: if model.scanner.is_scanning() {
                        "bluetooth-acquiring-symbolic"
                    } else {
                        "bluetooth-symbolic"
                    },
                },
                connect_clicked[sender] => move |_| {
                    sender.input(Input::ScanToggled);
                },
            },
            append = &gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vexpand: true,
                #[wrap(Some)]
                #[local_ref]
                set_child = factory_widget -> gtk::ListBox {
                    // set_margin_all: 5,
                    set_valign: gtk::Align::Start,
                    add_css_class: "boxed-list",
                    connect_row_activated[sender] => move |_, row| {
                        sender.input(Input::DeviceSelected(row.index()))
                    }
                },
            },
        }
    }

    fn init(params: Self::InitParams, root: &Self::Root, sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            devices: FactoryVecDeque::new(gtk::ListBox::new(), &sender.input),
            runtime: params.0,
            adapter: params.1,
            scanner: bt::Scanner::new(),
        };

        let factory_widget = model.devices.widget();
        let widgets = view_output!();

        // Read known devices list
        let adapter = model.adapter.clone();
        let send = sender.clone();
        model.runtime.spawn(async move {
            let mut devices = Vec::new();
            for device in bt::InfiniTime::list_known_devices(&adapter).await.unwrap() {
                devices.push(DeviceInfo::new(&device).await.unwrap())
            }
            send.input(Input::KnownDevices(devices));
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        let mut devices_guard = self.devices.guard();
        match msg {
            Input::ScanToggled => {
                if !self.scanner.is_scanning() {
                    devices_guard.clear();
                    let send = sender.clone();
                    self.scanner.start(self.adapter.clone(), self.runtime.clone(), move |event| {
                        match event {
                            bluer::AdapterEvent::DeviceAdded(address) => {
                                send.input(Input::DeviceAdded(address));
                            }
                            bluer::AdapterEvent::DeviceRemoved(address) => {
                                send.input(Input::DeviceRemoved(address));
                            }
                            _ => (),
                        }
                    });
                } else {
                    self.scanner.stop();
                }
            }
            Input::KnownDevices(devices) => {
                let connected_address = devices.iter().find(|d| d.connected).map(|d| d.address);

                for device in devices {
                    devices_guard.push_back(device);
                }
                // Automatic device selection logic
                if let Some(address) = connected_address {
                    // If suitable device is already connected - just report it as connected
                    sender.output(Output::DeviceConnected(address));
                    println!("InfiniTime ({}) is already connected", address.to_string());
                } else {
                    if devices_guard.is_empty() {
                        // If no suitable devices are known - start scanning automatically
                        sender.input(Input::ScanToggled);
                        println!("No InfiniTime devices are known. Scanning...");
                    } else if devices_guard.len() == 1 {
                        // If only one suitable device is known - try to connect to it automatically
                        let address = devices_guard.get(0).unwrap().address;
                        sender.output(Output::DeviceSelected(address));
                        println!("Trying to connect to InfiniTime ({})", address.to_string());
                    } else {
                        println!("Multiple InfiniTime devices are known. Waiting for the user to select");
                    }
                }
            }
            Input::DeviceAdded(address) => {
                if let Ok(device) = self.adapter.device(address) {
                    if let Some(info) = self.runtime.block_on(async {
                        if bt::InfiniTime::check_device(&device).await {
                            DeviceInfo::new(&device).await.ok()
                        } else {
                            None
                        }
                    }) {
                        devices_guard.push_front(info);
                    }
                }
            }
            Input::DeviceRemoved(address) => {
                // if let Some((index, _)) = devices_guard.iter().enumerate().find(|(_, d)| d.address == address) {
                //     devices_guard.remove(index);
                // }
                for i in (0..devices_guard.len()).rev() {
                    if let Some(device) = devices_guard.get(i) {
                        if device.address == address {
                            devices_guard.remove(i);
                        }
                    }
                }
            }
            Input::DeviceSelected(index) => {
                if let Some(info) = devices_guard.get(index as usize) {
                    self.scanner.stop();
                    if !info.connected {
                        println!("Selected device: {:?}", info);
                        sender.output(Output::DeviceSelected(info.address));
                    } else {
                        self.runtime.block_on(self.adapter.device(info.address).unwrap().disconnect()).unwrap();
                    }
                }
            }
        }
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
#[relm4::factory(pub)]
impl FactoryComponent<gtk::ListBox, Input> for DeviceInfo {
    type Command = ();
    type CommandOutput = ();
    type InitParams = Self;
    type Input = ();
    type Output = ();
    type Widgets = DeviceInfoWidgets;

    view! {
        #[root]
        gtk::ListBoxRow {
            #[wrap(Some)]
            set_child = &gtk::Box {
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
                        #[watch]
                        set_visible: self.connected,
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

    fn init_model(
        model: Self,
        _index: &DynamicIndex,
        _input: &Sender<Self::Input>,
        _output: &Sender<Self::Output>,
    ) -> Self {
        model
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: &Self::Root,
        _returned_widget: &gtk::ListBoxRow,
        _input: &Sender<Self::Input>,
        _output: &Sender<Self::Output>,
    ) -> Self::Widgets {
        let widgets = view_output!();
        widgets
    }
}


