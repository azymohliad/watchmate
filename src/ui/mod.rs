use crate::inft::bt;
use std::{sync::Arc, path::PathBuf};
use bluer::gatt::local::ApplicationHandle;
use gtk::prelude::{BoxExt, GtkWindowExt};
use relm4::{
    adw, gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmApp
};

mod dashboard;
mod devices;
mod firmware_update;
mod firmware_panel;
mod media_player;

#[derive(Debug)]
enum Input {
    SetView(View),
    DeviceConnected(Arc<bluer::Device>),
    DeviceDisconnected(Arc<bluer::Device>),
    DeviceReady(Arc<bt::InfiniTime>),
    FirmwareUpdateFromFile(PathBuf),
    FirmwareUpdateFromUrl(String),
    Notification(&'static str),
}

#[derive(Debug)]
enum CommandOutput {
    GattServicesResult(bluer::Result<ApplicationHandle>),
}

struct Model {
    // UI state
    active_view: View,
    is_connected: bool,
    // Components
    dashboard: Controller<dashboard::Model>,
    devices: Controller<devices::Model>,
    fwupd: Controller<firmware_update::Model>,
    // Other
    infinitime: Option<Arc<bt::InfiniTime>>,
    gatt_server: Option<ApplicationHandle>,
    toast_overlay: adw::ToastOverlay,
}

impl Model {
    fn notify(&self, message: &str) {
        self.toast_overlay.add_toast(&adw::Toast::new(message));
    }
}

#[relm4::component]
impl Component for Model {
    type CommandOutput = CommandOutput;
    type Init = Arc<bluer::Adapter>;
    type Input = Input;
    type Output = ();
    type Widgets = Widgets;

    view! {
        adw::ApplicationWindow {
            set_default_width: 600,
            set_default_height: 600,

            #[local]
            toast_overlay -> adw::ToastOverlay {
                // TODO: Use Relm 0.5 conditional widgets here (automatic stack)
                // I can't make it work here for some reason for now.
                #[wrap(Some)]
                set_child = &gtk::Stack {
                    add_named[Some("dashboard_view")] = &gtk::Box {
                        // set_visible: watch!(components.dashboard.model.device.is_some()),
                        append: model.dashboard.widget(),
                    },
                    add_named[Some("devices_view")] = &gtk::Box {
                        append: model.devices.widget(),
                    },
                    add_named[Some("fwupd_view")] = &gtk::Box {
                        append: model.fwupd.widget(),
                    },
                    #[watch]
                    set_visible_child_name: match model.active_view {
                        View::Dashboard => "dashboard_view",
                        View::Devices => "devices_view",
                        View::FirmwareUpdate => "fwupd_view",
                    },
                },
            },
        }
    }

    fn init(adapter: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        // Components
        let dashboard = dashboard::Model::builder()
            .launch(root.clone())
            .forward(&sender.input, |message| match message {
                dashboard::Output::FirmwareUpdateFromFile(file) => Input::FirmwareUpdateFromFile(file),
                dashboard::Output::FirmwareUpdateFromUrl(url) => Input::FirmwareUpdateFromUrl(url),
                dashboard::Output::Notification(text) => Input::Notification(text),
                dashboard::Output::SetView(view) => Input::SetView(view),
            });

        let devices = devices::Model::builder()
            .launch(adapter.clone())
            .forward(&sender.input, |message| match message {
                devices::Output::DeviceConnected(device) => Input::DeviceConnected(device),
                devices::Output::DeviceDisconnected(device) => Input::DeviceDisconnected(device),
                devices::Output::SetView(view) => Input::SetView(view),
            });

        let fwupd = firmware_update::Model::builder()
            .launch(())
            .forward(&sender.input, |message| match message {
                firmware_update::Output::SetView(view) => Input::SetView(view),
            });

        let toast_overlay = adw::ToastOverlay::new();

        let model = Model {
            // UI state
            active_view: View::Devices,
            is_connected: false,
            // Components
            dashboard,
            devices,
            fwupd,
            // Other
            infinitime: None,
            gatt_server: None,
            toast_overlay: toast_overlay.clone(),
        };

        let widgets = view_output!();

        sender.oneshot_command(async move {
            CommandOutput::GattServicesResult(bt::start_gatt_services(&adapter).await)
        });

        ComponentParts { model, widgets }
    }


    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Input::SetView(view) => {
                self.active_view = view;
            }
            Input::DeviceConnected(device) => {
                self.is_connected = true;
                relm4::spawn(async move {
                    match bt::InfiniTime::new(device).await {
                        Ok(infinitime) => {
                            sender.input(Input::DeviceReady(Arc::new(infinitime)));
                        }
                        Err(error) => {
                            log::error!("Device is rejected: {}", error);
                            sender.input(Input::Notification("Device is rejected by the app"));
                        }
                    }
                });
            }
            Input::DeviceDisconnected(device) => {
                if self.infinitime.as_ref().map_or(false, |i| i.device().address() == device.address()) {
                // Use this once is_some_and is stabilized:
                // if self.infinitime.is_some_and(|i| i.device().address() == device.address()) {
                    self.infinitime = None;
                }
                self.dashboard.emit(dashboard::Input::Disconnected);
                self.fwupd.emit(firmware_update::Input::Disconnected);
            }
            Input::DeviceReady(infinitime) => {
                self.infinitime = Some(infinitime.clone());
                self.active_view = View::Dashboard;
                self.dashboard.emit(dashboard::Input::Connected(infinitime.clone()));
                self.fwupd.emit(firmware_update::Input::Connected(infinitime));
            }
            Input::FirmwareUpdateFromFile(filepath) => {
                self.fwupd.emit(firmware_update::Input::FirmwareUpdateFromFile(filepath));
                sender.input(Input::SetView(View::FirmwareUpdate));
            }
            Input::FirmwareUpdateFromUrl(url) => {
                self.fwupd.emit(firmware_update::Input::FirmwareUpdateFromUrl(url));
                sender.input(Input::SetView(View::FirmwareUpdate));
            }
            Input::Notification(message) => {
                self.notify(message);
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, _sender: ComponentSender<Self>) {
        match msg {
            CommandOutput::GattServicesResult(Ok(handle)) => {
                self.gatt_server = Some(handle);
            }
            CommandOutput::GattServicesResult(Err(error)) => {
                log::error!("Failed to start GATT server: {error}");
                self.notify("Failed to start GATT server");
            }
        }
    }
}



#[derive(Debug, PartialEq)]
pub enum View {
    Dashboard,
    Devices,
    FirmwareUpdate,
}


pub fn run(adapter: Arc<bluer::Adapter>) {
    // Init GTK before libadwaita (ToastOverlay)
    gtk::init().unwrap();

    // Run app
    let app = RelmApp::new("io.gitlab.azymohliad.WatchMate");
    app.run::<Model>(adapter);
}
