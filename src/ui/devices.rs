use std::sync::Arc;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{
    adw, gtk, factory::{FactoryComponent, FactoryComponentSender, FactoryVecDeque, DynamicIndex},
    ComponentParts, ComponentSender, WidgetPlus, Component, JoinHandle
};

use crate::bt;


#[derive(Debug)]
pub enum Input {
    ScanToggled,
    KnownDevicesReady(Vec<DeviceInfo>),
    DeviceInfoReady(DeviceInfo),
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceSelected(i32),
    DeviceConnected(Arc<bluer::Device>),
    DeviceDisconnected(Arc<bluer::Device>),
}

#[derive(Debug)]
pub enum Output {
    DeviceConnected(Arc<bluer::Device>),
    DeviceDisconnected(Arc<bluer::Device>),
    SetView(super::View),
}

#[derive(Debug)]
pub enum CommandOutput {
}

pub struct Model {
    devices: FactoryVecDeque<DeviceInfo>,
    adapter: Arc<bluer::Adapter>,
    scan_handle: Option<JoinHandle<()>>,
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = CommandOutput;
    type Init = Arc<bluer::Adapter>;
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
                        set_visible: model.scan_handle.is_some(),
                        set_spinning: true,
                    },

                    gtk::Button {
                        #[watch]
                        set_label: if model.scan_handle.is_some() {
                            "Stop Scanning"
                        } else {
                            "Start Scanning"
                        },
                        set_valign: gtk::Align::End,
                        set_halign: gtk::Align::Center,
                        connect_clicked[sender] => move |_| {
                            sender.input(Input::ScanToggled);
                        },
                    },
                }
            }

        }
    }

    fn init(adapter: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            devices: FactoryVecDeque::new(gtk::ListBox::new(), &sender.input),
            adapter,
            scan_handle: None,
        };

        let factory_widget = model.devices.widget();
        let widgets = view_output!();

        // Read known devices list
        let adapter = model.adapter.clone();
        relm4::spawn(async move {
            let mut devices = Vec::new();
            for device in bt::InfiniTime::list_known_devices(&adapter).await.unwrap() {
                devices.push(DeviceInfo::new(Arc::new(device)).await.unwrap())
            }
            sender.input(Input::KnownDevicesReady(devices));
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Input::ScanToggled => {
                if self.scan_handle.is_some() {
                    self.scan_handle.take().unwrap().abort();
                } else {
                    let adapter = self.adapter.clone();
                    let callback = move |event| {
                        match event {
                            bluer::AdapterEvent::DeviceAdded(address) => {
                                sender.input(Input::DeviceAdded(address));
                            }
                            bluer::AdapterEvent::DeviceRemoved(address) => {
                                sender.input(Input::DeviceRemoved(address));
                            }
                            _ => (),
                        }
                    };
                    self.devices.guard().clear();
                    self.scan_handle = Some(relm4::spawn(bt::scan(adapter, callback)));
                }
            }

            Input::KnownDevicesReady(devices) => {
                let connected = devices.iter()
                    .find(|d| d.state == DeviceState::Connected)
                    .map(|d| d.address);

                let mut devices_guard = self.devices.guard();
                for device in devices {
                    devices_guard.push_back(device);
                }

                // Automatic device selection logic
                if let Some(address) = connected {
                    // If suitable device is already connected - just report it as connected
                    if let Ok(device) = self.adapter.device(address) {
                        let device = Arc::new(device);
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

            Input::DeviceInfoReady(info) => {
                self.devices.guard().push_front(info);
            }

            Input::DeviceAdded(address) => {
                if let Ok(device) = self.adapter.device(address) {
                    let device = Arc::new(device);
                    relm4::spawn(async move {
                        if bt::InfiniTime::check_device(&device).await {
                            match DeviceInfo::new(device).await {
                                Ok(info) => sender.input(Input::DeviceInfoReady(info)),
                                Err(error) => eprintln!("Failed to read device info: {}", error),
                            }
                        }
                    });
                }
            }

            Input::DeviceRemoved(address) => {
                let mut devices_guard = self.devices.guard();
                for i in (0..devices_guard.len()).rev() {
                    if let Some(device) = devices_guard.get(i) {
                        if device.address == address {
                            devices_guard.remove(i);
                        }
                    }
                }
            }

            Input::DeviceSelected(index) => {
                self.scan_handle.take().map(|h| h.abort());
                self.devices.send(index as usize, DeviceInput::Connect);
            }

            Input::DeviceConnected(device) => {
                sender.output(Output::DeviceConnected(device));
            }

            Input::DeviceDisconnected(device) => {
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
    state: DeviceState,
    device: Arc<bluer::Device>,
}

impl DeviceInfo {
    async fn new(device: Arc<bluer::Device>) -> bluer::Result<Self> {
        let state = if device.is_connected().await? {
            DeviceState::Connected
        } else {
            DeviceState::Disconnected
        };
        Ok(Self {
            address: device.address(),
            alias: device.alias().await?,
            rssi: device.rssi().await?,
            state,
            device,
        })
    }
}

#[derive(PartialEq, Debug)]
pub enum DeviceState {
    Disconnected,
    Transitioning,
    Connected,
}

#[derive(Debug)]
pub enum DeviceInput {
    Connect,
    Disconnect,
    StateUpdated(DeviceState),
}

#[derive(Debug)]
pub enum DeviceOutput {
    Connected(Arc<bluer::Device>),
    Disconnected(Arc<bluer::Device>),
}

// Factory for device list
#[relm4::factory(pub)]
impl FactoryComponent for DeviceInfo {
    type ParentWidget = gtk::ListBox;
    type ParentMsg = Input;
    type CommandOutput = ();
    type Init = Self;
    type Input = DeviceInput;
    type Output = DeviceOutput;
    type Widgets = DeviceInfoWidgets;

    view! {
        #[root]
        gtk::ListBoxRow {
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_margin_all: 12,
                set_spacing: 10,
                set_hexpand: true,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,

                    gtk::Label {
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_label: &self.alias,
                    },

                    gtk::Label {
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_label: &self.address.to_string(),
                        add_css_class: "dim-label",
                    },
                },

                gtk::Label {
                    set_label: &match self.rssi {
                        Some(rssi) => format!("RSSI: {}", rssi),
                        None => String::from("Saved"),
                    },
                    add_css_class: "dim-label",
                },

                gtk::Button {
                    set_tooltip_text: Some("Click to disconnect"),
                    set_icon_name: "bluetooth-symbolic",
                    add_css_class: "flat",
                    #[watch]
                    set_visible: self.state == DeviceState::Connected,
                    connect_clicked[sender] => move |_| {
                        sender.input(DeviceInput::Disconnect);
                    }
                },

                gtk::Spinner {
                    #[watch]
                    set_visible: self.state == DeviceState::Transitioning,
                    set_spinning: true,
                },
            },
        }
    }

    fn output_to_parent_msg(output: Self::Output) -> Option<Input> {
        Some(match output {
            DeviceOutput::Connected(device) => Input::DeviceConnected(device),
            DeviceOutput::Disconnected(device) => Input::DeviceDisconnected(device),
        })
    }

    fn init_model(
        model: Self,
        _index: &DynamicIndex,
        _sender: FactoryComponentSender<Self>,
    ) -> Self {
        model
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: &Self::Root,
        _returned_widget: &gtk::ListBoxRow,
        sender: FactoryComponentSender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();
        widgets
    }

    fn update(
        &mut self,
        msg: Self::Input,
        sender: FactoryComponentSender<Self>,
    ) {
        match msg {
            DeviceInput::Connect => {
                self.state = DeviceState::Transitioning;
                let device = self.device.clone();
                relm4::spawn(async move {
                    match device.connect().await {
                        Ok(()) => {
                            sender.input(DeviceInput::StateUpdated(DeviceState::Connected));
                            sender.output(DeviceOutput::Connected(device));
                        }
                        Err(error) => {
                            sender.input(DeviceInput::StateUpdated(DeviceState::Disconnected));
                            eprintln!("Connection failure: {}", error);
                        }
                    }
                });
            }

            DeviceInput::Disconnect => {
                self.state = DeviceState::Transitioning;
                let device = self.device.clone();
                relm4::spawn(async move {
                    match device.disconnect().await {
                        Ok(()) => {
                            sender.input(DeviceInput::StateUpdated(DeviceState::Disconnected));
                            sender.output(DeviceOutput::Disconnected(device));
                        }
                        Err(error) => {
                            sender.input(DeviceInput::StateUpdated(DeviceState::Connected));
                            eprintln!("Disconnection failure: {}", error);
                        }
                    }
                });
            }

            DeviceInput::StateUpdated(state) => {
                self.state = state;
            }
        }
    }
}


