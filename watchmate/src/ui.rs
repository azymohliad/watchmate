use infinitime::{bluer, bt};
use std::{sync::Arc, path::PathBuf};
use futures::{pin_mut, StreamExt};
use gtk::{gio, glib, prelude::{ApplicationExt, BoxExt, GtkWindowExt}};
use relm4::{
    adw, gtk, actions::{AccelsPlus, RelmAction, RelmActionGroup},
    Component, ComponentController, ComponentParts,
    ComponentSender, Controller, RelmApp, MessageBroker
};


mod dashboard_page;
mod devices_page;
mod fwupd_page;
mod settings_page;

use fwupd_page::AssetType;

static APP_ID: &'static str = "io.gitlab.azymohliad.WatchMate";
static BROKER: relm4::MessageBroker<Input> = MessageBroker::new();


relm4::new_action_group!(ViewActionGroup, "view");
relm4::new_stateless_action!(DashboardViewAction, ViewActionGroup, "dashboard");
relm4::new_stateless_action!(DevicesViewAction, ViewActionGroup, "devices");
relm4::new_stateless_action!(SettingsViewAction, ViewActionGroup, "settings");
relm4::new_stateless_action!(AboutAction, ViewActionGroup, "about");
relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateless_action!(CloseAction, WindowActionGroup, "close");
relm4::new_stateless_action!(QuitAction, WindowActionGroup, "quit");


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
    About,
    Close,
    Quit,
    None,
}

struct Model {
    // UI state
    active_view: View,
    is_connected: bool,
    // Components
    dashboard_page: Controller<dashboard_page::Model>,
    devices_page: Controller<devices_page::Model>,
    fwupd_page: Controller<fwupd_page::Model>,
    settings_page: Controller<settings_page::Model>,
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
        #[name = "main_window"]
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
                        append: model.dashboard_page.widget(),
                    },
                    add_named[Some("devices_view")] = &gtk::Box {
                        append: model.devices_page.widget(),
                    },
                    add_named[Some("fwupd_view")] = &gtk::Box {
                        append: model.fwupd_page.widget(),
                    },
                    add_named[Some("settings_view")] = &gtk::Box {
                        append: model.settings_page.widget(),
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
        let settings = gio::Settings::new(APP_ID);

        // Components
        let dashboard_page = dashboard_page::Model::builder()
            .launch((root.clone(), settings.clone()))
            .forward(&sender.input_sender(), |message| match message {
                dashboard_page::Output::FlashAssetFromFile(file, atype) => Input::FlashAssetFromFile(file, atype),
                dashboard_page::Output::FlashAssetFromUrl(url, atype) => Input::FlashAssetFromUrl(url, atype),
            });

        let devices_page = devices_page::Model::builder()
            .launch(())
            .forward(&sender.input_sender(), |message| match message {
                devices_page::Output::DeviceConnected(device) => Input::DeviceConnected(device),
            });

        let fwupd_page = fwupd_page::Model::builder()
            .launch(())
            .detach();

        let settings_page = settings_page::Model::builder()
            .launch(settings.clone())
            .forward(&sender.input_sender(), |message| match message {
                settings_page::Output::RunInBackground(on) => Input::SetRunInBackground(on),
                settings_page::Output::AutoReconnect(on) => Input::SetAutoReconnect(on),
                settings_page::Output::AutoStart(_) => Input::None,
            });

        // Initialize model
        let model = Model {
            // UI state
            active_view: View::Devices,
            is_connected: false,
            // Components
            dashboard_page,
            devices_page,
            fwupd_page,
            settings_page,
            // Other
            infinitime: None,
            toast_overlay: adw::ToastOverlay::new(),
        };

        // Widgets
        let toast_overlay = model.toast_overlay.clone();
        let widgets = view_output!();

        // Actions
        let app = relm4::main_application();
        app.set_accelerators_for_action::<CloseAction>(&["<primary>W"]);
        app.set_accelerators_for_action::<QuitAction>(&["<primary>Q"]);

        let mut view_group = RelmActionGroup::<ViewActionGroup>::new();
        view_group.add_action(RelmAction::<DashboardViewAction>::new_stateless(
            glib::clone!(@strong sender => move |_| {
                sender.input(Input::SetView(View::Dashboard));
            }
        )));
        view_group.add_action(RelmAction::<DevicesViewAction>::new_stateless(
            glib::clone!(@strong sender => move |_| {
                sender.input(Input::SetView(View::Devices));
            }
        )));
        view_group.add_action(RelmAction::<SettingsViewAction>::new_stateless(
            glib::clone!(@strong sender => move |_| {
                sender.input(Input::SetView(View::Settings));
            }
        )));
        view_group.add_action(RelmAction::<AboutAction>::new_stateless(
            glib::clone!(@strong sender => move |_| {
                sender.input(Input::About);
            }
        )));
        view_group.register_for_widget(&widgets.main_window);

        let mut global_group = RelmActionGroup::<WindowActionGroup>::new();
        global_group.add_action(RelmAction::<QuitAction>::new_stateless(
            glib::clone!(@strong sender => move |_| {
                sender.input(Input::Quit);
            }
        )));
        global_group.add_action(RelmAction::<CloseAction>::new_stateless(
            glib::clone!(@strong sender => move |_| {
                sender.input(Input::Close);
            }
        )));
        global_group.register_for_widget(&widgets.main_window);

        ComponentParts { model, widgets }
    }


    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            Input::SetView(view) => {
                if self.active_view != view {
                    if self.active_view == View::Devices {
                        self.devices_page.emit(devices_page::Input::StopDiscovery);
                    }
                    if view == View::Devices {
                        self.devices_page.emit(devices_page::Input::StartDiscovery);
                    }
                    self.active_view = view;
                }
            }
            Input::SetAutoReconnect(enabled) => {
                self.devices_page.emit(devices_page::Input::SetAutoReconnect(enabled));
            }
            Input::SetRunInBackground(enabled) => {
                root.set_hide_on_close(enabled);
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
                    self.devices_page.emit(devices_page::Input::DeviceConnectionLost(infinitime.device().address()));
                }
                self.dashboard_page.emit(dashboard_page::Input::Disconnected);
                self.fwupd_page.emit(fwupd_page::Input::Disconnected);
                sender.input(Input::SetView(View::Devices));
            }
            Input::DeviceReady(infinitime) => {
                log::info!("PineTime recognized");
                self.infinitime = Some(infinitime.clone());
                if self.active_view == View::Devices {
                    self.active_view = View::Dashboard;
                }
                self.dashboard_page.emit(dashboard_page::Input::Connected(infinitime.clone()));
                self.fwupd_page.emit(fwupd_page::Input::Connected(infinitime.clone()));
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
                self.devices_page.emit(devices_page::Input::StartDiscovery);
            }
            Input::FlashAssetFromFile(file, atype) => {
                self.fwupd_page.emit(fwupd_page::Input::FlashAssetFromFile(file, atype));
                sender.input(Input::SetView(View::FirmwareUpdate));
            }
            Input::FlashAssetFromUrl(url, atype) => {
                self.fwupd_page.emit(fwupd_page::Input::FlashAssetFromUrl(url, atype));
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
            Input::About => {
                adw::AboutWindow::builder()
                    .transient_for(root)
                    .application_icon(APP_ID)
                    .application_name("WatchMate")
                    .version("v0.4.6")
                    .website("https://github.com/azymohliad/watchmate")
                    .issue_url("https://github.com/azymohliad/watchmate/issues")
                    .license_type(gtk::License::Gpl30)
                    .build()
                    .present();
            }
            Input::Close => {
                root.close();
            }
            Input::Quit => {
                root.application().unwrap().quit();
            }
            Input::None => {}
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
