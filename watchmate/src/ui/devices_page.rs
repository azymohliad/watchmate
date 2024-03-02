use crate::ui;
use std::{str::FromStr, time::Duration};
use infinitime::{ bluer, bt };
use std::sync::Arc;
use futures::{pin_mut, StreamExt};
use gtk::{gio, prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt, SettingsExt}};
use relm4::{
    adw, gtk,
    factory::{FactoryComponent, FactorySender, FactoryVecDeque, DynamicIndex},
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
    DeviceSelected(i32),
    DeviceConnected(Arc<bluer::Device>),
    DeviceDisconnected(Arc<bluer::Device>),
    DeviceDisconnecting(Arc<bluer::Device>),
    DeviceConnectionFailed,
    DeviceConnectionLost(bluer::Address),
    SaveAddress(Option<bluer::Address>),
}

#[derive(Debug)]
pub enum Output {
    DeviceConnected(Arc<bluer::Device>),
}

#[derive(Debug)]
pub enum CommandOutput {
    InitAdapterResult(bluer::Result<bluer::Adapter>),
    GattServicesResult(bluer::Result<bluer::gatt::local::ApplicationHandle>),
    KnownDevices(Vec<DeviceInfo>),
}

pub struct Model {
    settings: gio::Settings,
    devices: FactoryVecDeque<DeviceInfo>,
    adapter: Option<Arc<bluer::Adapter>>,
    gatt_server: Option<bluer::gatt::local::ApplicationHandle>,
    discovery_task: Option<JoinHandle<()>>,

    saved_address: Option<bluer::Address>,
    autoconnect_address: Option<bluer::Address>,
    disconnecting_address: Option<bluer::Address>,
}

impl Model {
    async fn init_adapter() -> bluer::Result<bluer::Adapter> {
        let session = bluer::Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;
        Ok(adapter)
    }

    async fn run_discovery(adapter: Arc<bluer::Adapter>, sender: ComponentSender<Self>) {
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
    type Init = gio::Settings;
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    menu! {
        main_menu: {
            "Back to Dashboard" => super::DashboardViewAction,
            "Settings" => super::SettingsViewAction,
            section! {
                "About" => super::AboutAction,
            },
            section! {
                "Quit" => super::QuitAction,
            }
        }
    }

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
                    connect_clicked => |_| {
                        ui::BROKER.send(ui::Input::SetView(super::View::Dashboard));
                    },
                },
                pack_end = &gtk::MenuButton {
                    set_icon_name: "open-menu-symbolic",
                    #[wrap(Some)]
                    set_popover = &gtk::PopoverMenu::from_model(Some(&main_menu)) {}
                }
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
                            connect_clicked => Input::InitAdapter,
                        },
                    }
                }
            }
        }
    }

    fn init(settings: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let saved_address = match settings.string(super::SETTING_DEVICE_ADDRESS).as_str() {
            "" => None,
            address => bluer::Address::from_str(address).ok()
        };

        let devices = FactoryVecDeque::builder()
            .launch(gtk::ListBox::new())
            .forward(sender.input_sender(), |output| match output {
                DeviceOutput::Connected(device) => Input::DeviceConnected(device),
                DeviceOutput::Disconnected(device) => Input::DeviceDisconnected(device),
                DeviceOutput::Disconnecting(device) => Input::DeviceDisconnecting(device),
                DeviceOutput::ConnectionFailed => Input::DeviceConnectionFailed,
                DeviceOutput::SaveAddress(address) => Input::SaveAddress(address),
            });

        let model = Self {
            settings,
            devices,
            adapter: None,
            gatt_server: None,
            discovery_task: None,
            autoconnect_address: saved_address.clone(),
            saved_address,
            disconnecting_address: None,
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
                if self.discovery_task.is_none() {
                    if let Some(adapter) = self.adapter.clone() {
                        self.devices.guard().clear();
                        self.discovery_task = Some(relm4::spawn(Self::run_discovery(adapter, sender)));
                        log::info!("Device discovery started");
                    }
                }
            }

            Input::StopDiscovery => {
                if let Some(handle) = self.discovery_task.take() {
                    handle.abort();
                    log::info!("Device discovery stopped");
                }
            }

            Input::DiscoveryFailed => {
                log::error!("Device discovery failed");
                self.discovery_task = None;
                // Usually this may happen when waking up from suspend. Retry
                relm4::spawn(async move {
                    relm4::tokio::time::sleep(Duration::from_secs(1)).await;
                    sender.input(Input::StartDiscovery);
                });
            }

            Input::DeviceInfoReady(info) => {
                let address = info.address;
                let mut devices = self.devices.guard();
                devices.push_front(info);
                if Some(address) == self.autoconnect_address {
                    log::debug!("Detected lost device: {}. Trying to reconnect...", address);
                    sender.input(Input::StopDiscovery);
                    devices.send(0, DeviceInput::Connect);
                }
            }

            Input::DeviceAdded(address) => {
                if let Some(adapter) = &self.adapter {
                    if let Ok(device) = adapter.device(address) {
                        let device = Arc::new(device);
                        let saved = Some(address) == self.saved_address;
                        relm4::spawn(async move {
                            if bt::InfiniTime::check_device(&device).await {
                                log::debug!("Device discovered: {}", address);
                                match DeviceInfo::new(device, saved).await {
                                    Ok(info) => sender.input(Input::DeviceInfoReady(info)),
                                    Err(error) => log::error!("Failed to read device info: {}", error),
                                }
                            }
                        });
                    }
                }
            }

            Input::DeviceRemoved(address) => {
                let mut devices_guard = self.devices.guard();
                for i in (0..devices_guard.len()).rev() {
                    if let Some(device) = devices_guard.get(i) {
                        if device.address == address {
                            log::debug!("Device lost: {}", address);
                            devices_guard.remove(i);
                        }
                    }
                }
            }

            Input::DeviceSelected(index) => {
                log::debug!("Device selected: {}", index);
                sender.input(Input::StopDiscovery);
                self.autoconnect_address = None;
                if let Some(device) = self.devices.get(index as usize) {
                    if device.state != DeviceState::Transitioning {
                        self.devices.send(index as usize, DeviceInput::Connect);
                    }
                }
            }

            Input::DeviceConnected(device) => {
                log::debug!("Device connected successfully: {}", device.address());
                self.autoconnect_address = None;
                _ = self.settings.set_string(super::SETTING_DEVICE_ADDRESS, &device.address().to_string());
                sender.input(Input::SaveAddress(Some(device.address())));
                sender.output(Output::DeviceConnected(device)).unwrap();
            }

            Input::DeviceDisconnected(device) => {
                log::debug!("Device disconnected successfully: {}", device.address());
                if Some(device.address()) == self.autoconnect_address {
                    self.autoconnect_address = None;
                }
                self.disconnecting_address = None;
                // Repopulate known devices
                sender.input(Input::StopDiscovery);
                sender.input(Input::StartDiscovery);
            }

            Input::DeviceDisconnecting(device) => {
                self.disconnecting_address = Some(device.address());
            }

            Input::DeviceConnectionFailed => {
                log::debug!("Device connection failed");
                sender.input(Input::StartDiscovery);
            }

            Input::DeviceConnectionLost(address) => {
                log::debug!("Device connection lost: {}", address);

                let devices = self.devices.guard();
                let result = devices.iter().enumerate().find(|(_, d)| d.address == address);
                if let Some((idx, _)) = result {
                    devices.send(idx, DeviceInput::StateUpdated(DeviceState::Disconnected));
                }
                if Some(address) != self.disconnecting_address && Some(address) == self.saved_address {
                    self.autoconnect_address = Some(address);
                    sender.input(Input::StartDiscovery);
                }
            }

            Input::SaveAddress(address) => {
                self.saved_address = address;
                let address_str = address.map(|a| a.to_string()).unwrap_or_default();
                _ = self.settings.set_string(super::SETTING_DEVICE_ADDRESS, &address_str);
                self.devices.broadcast(DeviceInput::SavedAddress(address));
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
                    let saved_address = self.saved_address.clone();
                    sender.oneshot_command(async move {
                        let mut devices = Vec::new();
                        for device in bt::InfiniTime::list_known_devices(&adapter).await.unwrap() {
                            let saved = Some(device.address()) == saved_address;
                            devices.push(DeviceInfo::new(Arc::new(device), saved).await.unwrap())
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
                    ui::BROKER.send(ui::Input::ToastStatic("Failed to start GATT server"));
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
                            self.autoconnect_address = None;
                            log::info!("InfiniTime ({}) is already connected", address.to_string());
                        }
                    }
                } else {
                    if let Some((i, d)) = devices_guard.iter().enumerate().find(
                        |(_, d)| Some(d.address) == self.autoconnect_address
                    ) {
                        log::info!("Trying to connect to InfiniTime ({})", d.address.to_string());
                        devices_guard.send(i, DeviceInput::Connect);
                    } else {
                        // Otherwise, start discovery
                        sender.input(Input::StartDiscovery);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    address: bluer::Address,
    alias: String,
    rssi: Option<i16>,
    state: DeviceState,
    device: Arc<bluer::Device>,
    saved: bool,
}

impl DeviceInfo {
    async fn new(device: Arc<bluer::Device>, saved: bool) -> bluer::Result<Self> {
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
            saved,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DeviceState {
    Disconnected,
    Transitioning,
    Connected,
}

#[derive(Clone, Debug)]
pub enum DeviceInput {
    Connect,
    Disconnect,
    StateUpdated(DeviceState),
    SavedToggle,
    SavedAddress(Option<bluer::Address>),
}

#[derive(Debug)]
pub enum DeviceOutput {
    Connected(Arc<bluer::Device>),
    Disconnected(Arc<bluer::Device>),
    Disconnecting(Arc<bluer::Device>),
    ConnectionFailed,
    SaveAddress(Option<bluer::Address>),
}

// Factory for device list
#[relm4::factory(pub)]
impl FactoryComponent for DeviceInfo {
    type ParentWidget = gtk::ListBox;
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
                set_spacing: 5,
                set_hexpand: true,

                gtk::Button {
                    #[watch]
                    set_tooltip_text: match self.saved {
                        true => Some("Disable automatic re-connection"),
                        false => Some("Enable automatic re-connection"),
                    },
                    #[watch]
                    set_icon_name: match self.saved {
                        true => "heart-filled-symbolic",
                        false => "heart-outline-thin-symbolic",
                    },
                    add_css_class: "flat",
                    connect_clicked => DeviceInput::SavedToggle,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,

                    gtk::Label {
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        set_label: &self.alias,
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 0,
                        set_spacing: 10,

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            set_label: &self.address.to_string(),
                            add_css_class: "dim-label",
                        },

                        gtk::Label {
                            set_halign: gtk::Align::Start,
                            set_hexpand: true,
                            set_label: &match self.rssi {
                                Some(rssi) => format!("RSSI: {}", rssi),
                                None => String::from(""),
                            },
                            add_css_class: "dim-label",
                        },
                    },
                },

                gtk::Button {
                    set_tooltip_text: Some("Click to disconnect"),
                    set_icon_name: "cross-symbolic",
                    add_css_class: "flat",
                    #[watch]
                    set_visible: self.state == DeviceState::Connected,
                    connect_clicked => DeviceInput::Disconnect,
                },

                gtk::Spinner {
                    #[watch]
                    set_visible: self.state == DeviceState::Transitioning,
                    set_spinning: true,
                },
            },
        }
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
        root: Self::Root,
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
                            _ = sender.output(DeviceOutput::Connected(device));
                        }
                        Err(error) => {
                            sender.input(DeviceInput::StateUpdated(DeviceState::Disconnected));
                            _ = sender.output(DeviceOutput::ConnectionFailed);
                            log::error!("Connection failure: {}", error);
                        }
                    }
                });
            }

            DeviceInput::Disconnect => {
                self.state = DeviceState::Transitioning;
                let device = self.device.clone();
                _ = sender.output(DeviceOutput::Disconnecting(device.clone()));
                relm4::spawn(async move {
                    match device.disconnect().await {
                        Ok(()) => {
                            // self.state cannot be updated via the message here, because
                            // bluer removes the device immediately after disconnection
                            _ = sender.output(DeviceOutput::Disconnected(device));
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

            DeviceInput::SavedToggle => {
                let address = match self.saved {
                    true => None,
                    false => Some(self.address),
                };
                _ = sender.output(DeviceOutput::SaveAddress(address))
            }

            DeviceInput::SavedAddress(address) => {
                self.saved = Some(self.address) == address;
            }
        }
    }
}
