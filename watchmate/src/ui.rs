use infinitime::{bluer, bt};
use std::{sync::Arc, path::PathBuf};
use futures::{pin_mut, StreamExt};
use gtk::{gio, prelude::{BoxExt, GtkWindowExt, SettingsExt, WidgetExt}};
use relm4::{
    adw, gtk, Component, ComponentController, ComponentParts,
    ComponentSender, Controller, RelmApp, MessageBroker
};
use ashpd::{desktop::background::Background, WindowIdentifier};


mod dashboard;
mod devices;
mod firmware_update;
mod firmware_panel;
mod media_player;
mod notifications;
mod settings;

use firmware_update::AssetType;

static APP_ID: &'static str = "io.gitlab.azymohliad.WatchMate";
static BROKER: relm4::MessageBroker<Input> = MessageBroker::new();

#[derive(Debug)]
enum Input {
    SetView(View),
    SetAutoReconnect(bool),
    SetRunInBackground(bool),
    DeviceConnected(Arc<bluer::Device>),
    DeviceDisconnected,
    DeviceReady(Arc<bt::InfiniTime>),
    DeviceRejected,
    FlashAssetFromFile(PathBuf, AssetType),
    FlashAssetFromUrl(String, AssetType),
    Toast(String),
    ToastStatic(&'static str),
    ToastWithLink {
        message: &'static str,
        label: &'static str,
        url: &'static str,
    },
}

struct Model {
    // UI state
    active_view: View,
    is_connected: bool,
    // Components
    dashboard: Controller<dashboard::Model>,
    devices: Controller<devices::Model>,
    fwupd: Controller<firmware_update::Model>,
    settings: Controller<settings::Model>,
    // Other
    infinitime: Option<Arc<bt::InfiniTime>>,
    toast_overlay: adw::ToastOverlay,
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
            set_default_width: 480,
            set_default_height: 720,

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
                    add_named[Some("settings_view")] = &gtk::Box {
                        append: model.settings.widget(),
                    },
                    #[watch]
                    set_visible_child_name: match model.active_view {
                        View::Dashboard => "dashboard_view",
                        View::Devices => "devices_view",
                        View::FirmwareUpdate => "fwupd_view",
                        View::Settings => "settings_view",
                    },
                },
            },
        }
    }

    fn init(_: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let persistent_settings = gio::Settings::new(APP_ID);
        // Components
        let dashboard = dashboard::Model::builder()
            .launch((root.clone(), persistent_settings.clone()))
            .forward(&sender.input_sender(), |message| match message {
                dashboard::Output::FlashAssetFromFile(file, atype) => Input::FlashAssetFromFile(file, atype),
                dashboard::Output::FlashAssetFromUrl(url, atype) => Input::FlashAssetFromUrl(url, atype),
            });

        let devices = devices::Model::builder()
            .launch(())
            .forward(&sender.input_sender(), |message| match message {
                devices::Output::DeviceConnected(device) => Input::DeviceConnected(device),
            });

        let fwupd = firmware_update::Model::builder()
            .launch(())
            .detach();

        let settings = settings::Model::builder()
            .launch(persistent_settings.clone())
            .forward(&sender.input_sender(), |message| match message {
                settings::Message::AutoReconnect(on) => Input::SetAutoReconnect(on),
                settings::Message::RunInBackground(on) => Input::SetRunInBackground(on),
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
            settings,
            // Other
            infinitime: None,
            toast_overlay: toast_overlay.clone(),
        };

        let widgets = view_output!();

        model.devices.emit(devices::Input::SetAutoReconnect(persistent_settings.boolean("auto-reconnect-enabled")));

        ComponentParts { model, widgets }
    }


    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            Input::SetView(view) => {
                if self.active_view != view {
                    if self.active_view == View::Devices {
                        self.devices.emit(devices::Input::StopDiscovery);
                    }
                    if view == View::Devices {
                        self.devices.emit(devices::Input::StartDiscovery);
                    }
                    self.active_view = view;
                }
            }
            Input::SetAutoReconnect(enabled) => {
                self.devices.emit(devices::Input::SetAutoReconnect(enabled));
            }
            Input::SetRunInBackground(enabled) => {
                root.set_hide_on_close(enabled);
                if enabled {
                    let native_root = root.native().unwrap();
                    let settings_sender = self.settings.sender().clone();
                    relm4::spawn_local(async move {
                        let identifier = WindowIdentifier::from_native(&native_root).await;
                        let request = Background::request()
                            .identifier(identifier)
                            .reason("WatchMate needs to run in the background to maintain the connection to your PineTime");
                        match request.send().await.and_then(|r| r.response()) {
                            Ok(_response) => {}
                            Err(err) => {
                                _ = settings_sender.send(settings::Message::RunInBackground(false));
                                sender.input(Input::SetRunInBackground(false));
                                sender.input(Input::ToastStatic("Not allowed to run in background"));
                                log::error!("Failed to request running in background: {err}");
                            }
                        }
                    });
                }
            }
            Input::DeviceConnected(device) => {
                log::info!("Device connected: {}", device.address());
                self.is_connected = true;
                relm4::spawn(async move {
                    match bt::InfiniTime::new(device).await {
                        Ok(infinitime) => {
                            sender.input(Input::DeviceReady(Arc::new(infinitime)));
                        }
                        Err(error) => {
                            sender.input(Input::DeviceRejected);
                            log::error!("Device is rejected: {}", error);
                            sender.input(Input::ToastStatic("Device is rejected by the app"));
                        }
                    }
                });
            }
            Input::DeviceDisconnected => {
                log::info!("PineTime disconnected");
                if let Some(infinitime) = self.infinitime.take() {
                    self.devices.emit(devices::Input::DeviceConnectionLost(infinitime.device().address()));
                }
                self.dashboard.emit(dashboard::Input::Disconnected);
                self.fwupd.emit(firmware_update::Input::Disconnected);
                sender.input(Input::SetView(View::Devices));
            }
            Input::DeviceReady(infinitime) => {
                log::info!("PineTime recognized");
                self.infinitime = Some(infinitime.clone());
                self.active_view = View::Dashboard;
                self.dashboard.emit(dashboard::Input::Connected(infinitime.clone()));
                self.fwupd.emit(firmware_update::Input::Connected(infinitime.clone()));
                // Handle disconnection
                relm4::spawn(async move {
                    match infinitime.get_property_stream().await {
                        Ok(stream) => {
                            pin_mut!(stream);
                            // Wait for the event stream to end
                            stream.count().await;
                        }
                        Err(error) => {
                            log::error!("Failed to get property stream: {}", error);
                        }
                    }
                    sender.input(Input::DeviceDisconnected);
                });
            }
            Input::DeviceRejected => {
                self.devices.emit(devices::Input::StartDiscovery);
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
                self.toast_overlay.add_toast(adw::Toast::new(&message));
            }
            Input::ToastStatic(message) => {
                self.toast_overlay.add_toast(adw::Toast::new(message));
            }
            Input::ToastWithLink { message, label, url } => {
                let toast = adw::Toast::new(message);
                let root = root.clone();
                toast.set_button_label(Some(label));
                toast.connect_button_clicked(move |_| {
                    gtk::UriLauncher::new(&url)
                        .launch(Some(&root), gio::Cancellable::NONE, |_| ());
                });
                self.toast_overlay.add_toast(toast);
            }
        }
    }
}



#[derive(Debug, PartialEq)]
pub enum View {
    Dashboard,
    Devices,
    FirmwareUpdate,
    Settings,
}


pub fn run() {
    // Init GTK before libadwaita (ToastOverlay)
    gtk::init().unwrap();

    // Init icons
    relm4_icons::initialize_icons();

    // Run app
    RelmApp::new(APP_ID)
        .with_broker(&BROKER)
        .run::<Model>(());
}
