use std::sync::Arc;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{
    adw, gtk, factory::{FactoryComponent, FactoryVecDeque, DynamicIndex},
    ComponentParts, ComponentSender, Sender, WidgetPlus, Component
};

use crate::bt;


#[derive(Debug)]
pub enum Input {
    ScanToggled,
    DeviceSelected(i32),
}

#[derive(Debug)]
pub enum Output {
    DeviceConnected(bluer::Device),
    DeviceDisconnected(bluer::Device),
    Notification(String),
    SetView(super::View),
}

#[derive(Debug)]
pub enum CommandOutput {
    KnownDevices(Vec<DeviceInfo>),
    DeviceInfoReady(DeviceInfo),
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceConnected(bluer::Device),
    DeviceDisconnected(bluer::Device),
}

pub struct Model {
    // UI State
    is_scanning: bool,
    devices: FactoryVecDeque<gtk::ListBox, DeviceInfo, Input>,
    // Non-UI
    adapter: Arc<bluer::Adapter>,
    scanner: bt::Scanner,
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = CommandOutput;
    type InitParams = Arc<bluer::Adapter>;
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    view! {
        gtk::Box {
            set_hexpand: true,
            set_orientation: gtk::Orientation::Vertical,

            adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &gtk::Label {
                    set_label: "Devices",
                },

                pack_start = &gtk::Button {
                    set_tooltip_text: Some("Back"),
                    set_icon_name: "go-previous-symbolic",
                    connect_clicked[sender] => move |_| {
                        sender.output(Output::SetView(super::View::Dashboard));
                    },
                },
            },

            adw::Clamp {
                set_maximum_size: 400,
                set_vexpand: true,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 12,
                    set_spacing: 10,


                    gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_vexpand: true,

                        #[local_ref]
                        factory_widget -> gtk::ListBox {
                            // set_margin_all: 5,
                            set_valign: gtk::Align::Start,
                            add_css_class: "boxed-list",
                            connect_row_activated[sender] => move |_, row| {
                                sender.input(Input::DeviceSelected(row.index()))
                            }
                        },
                    },

                    gtk::Spinner {
                        #[watch]
                        set_visible: model.is_scanning,
                        set_spinning: true,
                    },

                    gtk::Button {
                        #[watch]
                        set_label: if model.is_scanning {
                            "Stop Scanning"
                        } else {
                            "Start Scanning"
                        },
                        set_valign: gtk::Align::End,
                        connect_clicked[sender] => move |_| {
                            sender.input(Input::ScanToggled);
                        },
                    },
                }
            }

        }
    }

    fn init(adapter: Self::InitParams, root: &Self::Root, sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            is_scanning: false,
            devices: FactoryVecDeque::new(gtk::ListBox::new(), &sender.input),
            adapter,
            scanner: bt::Scanner::new(),
        };

        let factory_widget = model.devices.widget();
        let widgets = view_output!();

        // Read known devices list
        let adapter = model.adapter.clone();
        sender.command(move |out, shutdown| {
            // TODO: Remove this extra clone once ComponentSender::command
            // is patched to accept FnOnce instead of Fn
            let adapter = adapter.clone();
            let task = async move {
                let mut devices = Vec::new();
                for device in bt::InfiniTime::list_known_devices(&adapter).await.unwrap() {
                    devices.push(DeviceInfo::new(&device).await.unwrap())
                }
                out.send(CommandOutput::KnownDevices(devices));
            };
            shutdown.register(task).drop_on_shutdown()
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        let mut devices_guard = self.devices.guard();
        match msg {
            Input::ScanToggled => {
                if self.is_scanning {
                    self.is_scanning = false;
                    self.scanner.stop();
                } else {
                    self.is_scanning = true;
                    devices_guard.clear();
                    let adapter = self.adapter.clone();
                    let scanner = self.scanner.clone();
                    sender.command(move |out, shutdown| {
                        // TODO: Remove these extra clones once ComponentSender::command
                        // is patched to accept FnOnce instead of Fn
                        let adapter = adapter.clone();
                        let scanner = scanner.clone();
                        shutdown.register(scanner.run(adapter, move |event| {
                            match event {
                                bluer::AdapterEvent::DeviceAdded(address) => {
                                    out.send(CommandOutput::DeviceAdded(address));
                                }
                                bluer::AdapterEvent::DeviceRemoved(address) => {
                                    out.send(CommandOutput::DeviceRemoved(address));
                                }
                                _ => (),
                            }
                        })).drop_on_shutdown()
                    });
                }
            }

            Input::DeviceSelected(index) => {
                if let Some(info) = devices_guard.get(index as usize) {
                    self.scanner.stop();
                    self.is_scanning = false;
                    match self.adapter.device(info.address) {
                        Ok(device) => {
                            let connected = info.connected;
                            sender.command(move |out, shutdown| {
                                // TODO: Remove this extra clone once ComponentSender::command
                                // is patched to accept FnOnce instead of Fn
                                let device = device.clone();
                                shutdown.register(async move {
                                    if !connected {
                                        match device.connect().await {
                                            Ok(()) => out.send(CommandOutput::DeviceConnected(device)),
                                            Err(error) => eprintln!("Connection failure: {}", error),
                                        }
                                    } else {
                                        match device.disconnect().await {
                                            Ok(()) => out.send(CommandOutput::DeviceDisconnected(device)),
                                            Err(error) => eprintln!("Connection failure: {}", error),
                                        }
                                    }
                                }).drop_on_shutdown()
                            });
                        }
                        Err(error) => {
                            eprintln!("Connection failure: {}", error);
                            sender.output(Output::Notification(String::from("Connection failure")));
                        }
                    }
                }
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, sender: &ComponentSender<Self>) {
        let mut devices_guard = self.devices.guard();
        match msg {
            CommandOutput::KnownDevices(devices) => {
                let connected = devices.iter().find(|d| d.connected).map(|d| d.address);

                for device in devices {
                    devices_guard.push_back(device);
                }
                // Automatic device selection logic
                if let Some(address) = connected {
                    // If suitable device is already connected - just report it as connected
                    if let Ok(device) = self.adapter.device(address) {
                        sender.output(Output::DeviceConnected(device));
                        println!("InfiniTime ({}) is already connected", address.to_string());
                    }
                } else {
                    if devices_guard.is_empty() {
                        // If no suitable devices are known - start scanning automatically
                        sender.input(Input::ScanToggled);
                        println!("No InfiniTime devices are known. Scanning...");
                    } else if devices_guard.len() == 1 {
                        // If only one suitable device is known - try to connect to it automatically
                        sender.input(Input::DeviceSelected(0));
                        println!("Trying to connect to InfiniTime ({})", devices_guard[0].address.to_string());
                    } else {
                        println!("Multiple InfiniTime devices are known. Waiting for the user to select");
                    }
                }
            }
            CommandOutput::DeviceInfoReady(info) => {
                devices_guard.push_front(info);
            }
            CommandOutput::DeviceAdded(address) => {
                if let Ok(device) = self.adapter.device(address) {
                    sender.command(move |out, shutdown| {
                        // TODO: Remove this extra clone once ComponentSender::command
                        // is patched to accept FnOnce instead of Fn
                        let device = device.clone();
                        let task = async move {
                            if bt::InfiniTime::check_device(&device).await {
                                match DeviceInfo::new(&device).await {
                                    Ok(info) => out.send(CommandOutput::DeviceInfoReady(info)),
                                    Err(error) => eprintln!("Failed to read device info: {}", error),
                                }
                            }
                        };
                        shutdown.register(task).drop_on_shutdown()
                    });
                }
            }
            CommandOutput::DeviceRemoved(address) => {
                for i in (0..devices_guard.len()).rev() {
                    if let Some(device) = devices_guard.get(i) {
                        if device.address == address {
                            devices_guard.remove(i);
                        }
                    }
                }
            }
            CommandOutput::DeviceConnected(device) => {
                for i in (0..devices_guard.len()).rev() {
                    if let Some(info) = devices_guard.get(i) {
                        if info.address == device.address() {
                            devices_guard.get_mut(i).unwrap().connected = true;
                        }
                    }
                }
                sender.output(Output::DeviceConnected(device));
            }
            CommandOutput::DeviceDisconnected(device) => {
                for i in (0..devices_guard.len()).rev() {
                    if let Some(info) = devices_guard.get(i) {
                        if info.address == device.address() {
                            devices_guard.get_mut(i).unwrap().connected = false;
                        }
                    }
                }
                sender.output(Output::DeviceDisconnected(device));
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
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,

                    gtk::Label {
                        set_margin_all: 5,
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_label: &self.alias,
                    },

                    gtk::Image {
                        set_margin_all: 5,
                        set_halign: gtk::Align::End,
                        set_hexpand: true,
                        set_icon_name: Some("bluetooth-symbolic"),
                        #[watch]
                        set_visible: self.connected,
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 5,
                    set_margin_all: 5,

                    gtk::Label {
                        set_margin_all: 5,
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_label: &self.address.to_string(),
                        add_css_class: "dim-label",
                    },

                    gtk::Label {
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


