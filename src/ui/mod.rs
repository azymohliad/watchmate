use crate::inft::bt;
use std::{sync::Arc, path::PathBuf};
use gtk::prelude::{BoxExt, GtkWindowExt};
use relm4::{
    adw, gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmApp
};

mod dashboard;
mod devices;
mod firmware_update;
mod firmware_panel;
mod media_player;
mod notifications;

use firmware_update::AssetType;


#[derive(Debug)]
enum Input {
    SetView(View),
    DeviceConnected(Arc<bluer::Device>),
    DeviceDisconnected(Arc<bluer::Device>),
    DeviceReady(Arc<bt::InfiniTime>),
    FlashAssetFromFile(PathBuf, AssetType),
    FlashAssetFromUrl(String, AssetType),
    Toast(&'static str),
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
    toast_overlay: adw::ToastOverlay,
}

impl Model {
    fn notify(&self, message: &str) {
        self.toast_overlay.add_toast(&adw::Toast::new(message));
    }
}

#[relm4::component]
impl Component for Model {
    type CommandOutput = ();
    type Init = ();
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

    fn init(_: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        // Components
        let dashboard = dashboard::Model::builder()
            .launch(root.clone())
            .forward(&sender.input_sender(), |message| match message {
                dashboard::Output::FlashAssetFromFile(file, atype) => Input::FlashAssetFromFile(file, atype),
                dashboard::Output::FlashAssetFromUrl(url, atype) => Input::FlashAssetFromUrl(url, atype),
                dashboard::Output::Toast(text) => Input::Toast(text),
                dashboard::Output::SetView(view) => Input::SetView(view),
            });

        let devices = devices::Model::builder()
            .launch(())
            .forward(&sender.input_sender(), |message| match message {
                devices::Output::DeviceConnected(device) => Input::DeviceConnected(device),
                devices::Output::DeviceDisconnected(device) => Input::DeviceDisconnected(device),
                devices::Output::Toast(text) => Input::Toast(text),
                devices::Output::SetView(view) => Input::SetView(view),
            });

        let fwupd = firmware_update::Model::builder()
            .launch(())
            .forward(&sender.input_sender(), |message| match message {
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
            toast_overlay: toast_overlay.clone(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }


    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
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
                            sender.input(Input::Toast("Device is rejected by the app"));
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
                self.fwupd.emit(firmware_update::Input::Connected(infinitime.clone()));
            }
            Input::FlashAssetFromFile(file, atype) => {
                self.fwupd.emit(firmware_update::Input::FlashAssetFromFile(file, atype));
                sender.input(Input::SetView(View::FirmwareUpdate));
            }
            Input::FlashAssetFromUrl(url, atype) => {
                self.fwupd.emit(firmware_update::Input::FlashAssetFromUrl(url, atype));
                sender.input(Input::SetView(View::FirmwareUpdate));
            }
            Input::Toast(message) => {
                self.notify(message);
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


pub fn run() {
    // Init GTK before libadwaita (ToastOverlay)
    gtk::init().unwrap();

    // Run app
    let app = RelmApp::new("io.gitlab.azymohliad.WatchMate");
    app.run::<Model>(());
}
