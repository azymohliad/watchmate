use crate::inft::bt;
use std::sync::Arc;
use futures::{pin_mut, StreamExt};
use bluer::{gatt::local::ApplicationHandle, Adapter, Result, Session};
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use relm4::{
    adw, gtk, factory::{FactoryComponent, FactorySender, FactoryVecDeque, DynamicIndex},
    ComponentParts, ComponentSender, Component, JoinHandle, RelmWidgetExt,
};




#[derive(Debug)]
pub enum Input {
    InitAdapter,
    StartDiscovery,
    StopDiscovery,
    DiscoveryFailed,
    DeviceInfoReady(DeviceInfo),
    DeviceAdded(bluer::Address),
    DeviceRemoved(bluer::Address),
    DeviceDisconnected(bluer::Address),
    DeviceSelected(i32),
    DeviceManuallyConnected(Arc<bluer::Device>),
    DeviceManuallyDisconnected(Arc<bluer::Device>),
    DeviceConnectionFailed,
}

#[derive(Debug)]
pub enum Output {
    DeviceConnected(Arc<bluer::Device>),
    Toast(&'static str),
    SetView(super::View),
}

#[derive(Debug)]
pub enum CommandOutput {
    InitAdapterResult(bluer::Result<bluer::Adapter>),
    GattServicesResult(bluer::Result<ApplicationHandle>),
    KnownDevices(Vec<DeviceInfo>),
}

pub struct Model {
    devices: FactoryVecDeque<DeviceInfo>,
    adapter: Option<Arc<bluer::Adapter>>,
    gatt_server: Option<ApplicationHandle>,
    discovery_task: Option<JoinHandle<()>>,
}

impl Model {
    async fn init_adapter() -> Result<Adapter> {
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;
        Ok(adapter)
    }

    async fn run_discovery(adapter: Arc<Adapter>, sender: ComponentSender<Self>) {
        match adapter.discover_devices().await {
            Ok(stream) => {
                pin_mut!(stream);
                loop {
                    match stream.next().await {
                        Some(bluer::AdapterEvent::DeviceAdded(address)) => {
                            sender.input(Input::DeviceAdded(address));
                        }
                        Some(bluer::AdapterEvent::DeviceRemoved(address)) => {
                            sender.input(Input::DeviceRemoved(address));
                        }
                        _ => (),
                    }
                }
            }
            Err(_) => sender.input(Input::DiscoveryFailed),
        }
    }
}


#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = CommandOutput;
    type Init = ();
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    view! {
        gtk::Box {
            set_hexpand: true,
            set_orientation: gtk::Orientation::Vertical,

            adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,

                    gtk::Label {
                        set_label: "Devices",
                    },

                    gtk::Spinner {
                        #[watch]
                        set_visible: model.discovery_task.is_some(),
                        set_spinning: true,
                    }
                },

                pack_start = &gtk::Button {
                    set_tooltip_text: Some("Back"),
                    set_icon_name: "go-previous-symbolic",
                    connect_clicked[sender] => move |_| {
                        sender.output(Output::SetView(super::View::Dashboard)).unwrap();
                    },
                },
            },

            adw::Clamp {
                set_maximum_size: 400,
                set_vexpand: true,

                if model.adapter.is_some() {
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
                    }
                } else {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_margin_all: 12,
                        set_spacing: 10,
                        set_valign: gtk::Align::Center,

                        gtk::Label {
                            set_label: "Bluetooth adapter not found!",
                        },

                        gtk::Button {
                            set_label: "Retry",
                            set_halign: gtk::Align::Center,
                            connect_clicked[sender] => move |_| {
                                sender.input(Input::InitAdapter)
                            }
                        },
                    }
                }
            }
        }
    }

    fn init(_: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            devices: FactoryVecDeque::new(gtk::ListBox::new(), &sender.input_sender()),
            adapter: None,
            gatt_server: None,
            discovery_task: None,
        };

        let factory_widget = model.devices.widget();
        let widgets = view_output!();

        sender.input(Input::InitAdapter);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::InitAdapter => {
                sender.oneshot_command(async move {
                    CommandOutput::InitAdapterResult(Self::init_adapter().await)
                });
            }

            Input::StartDiscovery => {
                if let Some(adapter) = self.adapter.clone() {
                    self.devices.guard().clear();
                    self.discovery_task = Some(relm4::spawn(Self::run_discovery(adapter, sender)));
                    log::info!("Discovery started");
                }
            }

            Input::StopDiscovery => {
                if let Some(handle) = self.discovery_task.take() {
                    handle.abort();
                    log::info!("Discovery stopped");
                }
            }

            Input::DiscoveryFailed => {
                self.discovery_task = None;
            }

            Input::DeviceInfoReady(info) => {
                self.devices.guard().push_front(info);
            }

            Input::DeviceAdded(address) => {
                log::debug!("Device added: {}", address);
                if let Some(adapter) = &self.adapter {
                    if let Ok(device) = adapter.device(address) {
                        let device = Arc::new(device);
                        relm4::spawn(async move {
                            if bt::InfiniTime::check_device(&device).await {
                                match DeviceInfo::new(device).await {
                                    Ok(info) => sender.input(Input::DeviceInfoReady(info)),
                                    Err(error) => log::error!("Failed to read device info: {}", error),
                                }
                            }
                        });
                    }
                }
            }

            Input::DeviceRemoved(address) => {
                log::debug!("Device removed: {}", address);
                let mut devices_guard = self.devices.guard();
                for i in (0..devices_guard.len()).rev() {
                    if let Some(device) = devices_guard.get(i) {
                        if device.address == address {
                            // Temporary hack to prevent removing devices
                            // while they're being manually disconnected.
                            // TODO: Ask bluer maintainers if the adapter is supposed to
                            // sends AdapterEvent::DeviceRemoved event when disconnecting,
                            // or if it is a bug
                            if device.state == DeviceState::Transitioning { continue; }

                            devices_guard.remove(i);
                        }
                    }
                }
            }

            Input::DeviceDisconnected(address) => {
                let device = self.devices.iter().enumerate().find(|(_,d)| d.address == address);
                if let Some((idx, _)) = device {
                    self.devices.send(idx, DeviceInput::StateUpdated(DeviceState::Disconnected));
                }
            }

            Input::DeviceSelected(index) => {
                sender.input(Input::StopDiscovery);
                self.devices.send(index as usize, DeviceInput::Connect);
            }

            Input::DeviceManuallyConnected(device) => {
                sender.output(Output::DeviceConnected(device)).unwrap();
            }

            Input::DeviceManuallyDisconnected(_) => {}

            Input::DeviceConnectionFailed => {
                sender.input(Input::StartDiscovery);
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            CommandOutput::InitAdapterResult(result) => match result {
                Ok(adapter) => {
                    let adapter = Arc::new(adapter);
                    self.adapter = Some(adapter.clone());

                    // Start GATT serices
                    let adapter_ = adapter.clone();
                    sender.oneshot_command(async move {
                        CommandOutput::GattServicesResult(bt::start_gatt_services(&adapter_).await)
                    });

                    // Read known devices list
                    sender.oneshot_command(async move {
                        let mut devices = Vec::new();
                        for device in bt::InfiniTime::list_known_devices(&adapter).await.unwrap() {
                            devices.push(DeviceInfo::new(Arc::new(device)).await.unwrap())
                        }
                        CommandOutput::KnownDevices(devices)
                    });
                }
                Err(error) => {
                    log::error!("Failed to initialize bluetooth adapter: {error}");
                }
            }
            CommandOutput::GattServicesResult(result) => match result {
                Ok(handle) => {
                    self.gatt_server = Some(handle);
                }
                Err(error) => {
                    log::error!("Failed to start GATT server: {error}");
                    sender.output(Output::Toast("Failed to start GATT server")).unwrap();
                }
            }

            CommandOutput::KnownDevices(devices) => {
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
                    if let Some(adapter) = &self.adapter {
                        if let Ok(device) = adapter.device(address) {
                            let device = Arc::new(device);
                            sender.output(Output::DeviceConnected(device)).unwrap();
                            log::info!("InfiniTime ({}) is already connected", address.to_string());
                        }
                    }
                } else {
                    if devices_guard.len() == 1 {
                        // If only one suitable device is known - try to connect to it automatically
                        sender.input(Input::DeviceSelected(0));
                        log::info!("Trying to connect to InfiniTime ({})", devices_guard[0].address.to_string());
                    } else {
                        // Otherwise, start discovery
                        sender.input(Input::StartDiscovery);
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
    ConnectionFailed,
}

// Factory for device list
#[relm4::factory(pub)]
impl FactoryComponent for DeviceInfo {
    type ParentWidget = gtk::ListBox;
    type ParentInput = Input;
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

    fn output_to_parent_input(output: Self::Output) -> Option<Input> {
        Some(match output {
            DeviceOutput::Connected(device) => Input::DeviceManuallyConnected(device),
            DeviceOutput::Disconnected(device) => Input::DeviceManuallyDisconnected(device),
            DeviceOutput::ConnectionFailed => Input::DeviceConnectionFailed,
        })
    }

    fn init_model(
        model: Self,
        _index: &DynamicIndex,
        _sender: FactorySender<Self>,
    ) -> Self {
        model
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: &Self::Root,
        _returned_widget: &gtk::ListBoxRow,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();
        widgets
    }

    fn update(
        &mut self,
        msg: Self::Input,
        sender: FactorySender<Self>,
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
                            sender.output(DeviceOutput::ConnectionFailed);
                            log::error!("Connection failure: {}", error);
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
                            log::error!("Disconnection failure: {}", error);
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
