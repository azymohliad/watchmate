use std::{sync::Arc, path::PathBuf};
use gtk::prelude::{ButtonExt, GtkWindowExt, OrientableExt, WidgetExt, FileChooserExt, FileExt};
use relm4::{
    adw, gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmApp
};
use crate::bt;

mod dashboard;
mod devices;
mod fwupd;

#[derive(Debug)]
enum Input {
    SetView(View),
    DeviceConnected(bluer::Device),
    FirmwareUpdate(PathBuf),
    Notification(String),
}

#[derive(Debug)]
enum CommandOutput {
    DeviceReady(Arc<bt::InfiniTime>),
}

struct Model {
    // UI state
    active_view: View,
    is_connected: bool,
    // Components
    dashboard: Controller<dashboard::Model>,
    devices: Controller<devices::Model>,
    fwupd: Controller<fwupd::Model>,
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
    type CommandOutput = CommandOutput;
    type InitParams = Arc<bluer::Adapter>;
    type Input = Input;
    type Output = ();
    type Widgets = Widgets;

    view! {
        adw::ApplicationWindow {
            set_default_width: 480,
            set_default_height: 640,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &gtk::Label {
                        #[watch]
                        set_label: match model.active_view {
                            View::Dashboard => "WatchMate",
                            View::Devices => "Devices",
                            View::FileChooser => "Choose DFU file",
                            View::FirmwareUpdate => "Firmware Upgrade",
                        },
                    },

                    pack_start = &gtk::Button {
                        set_label: "Back",
                        set_icon_name: "go-previous-symbolic",
                        #[watch]
                        set_visible: model.active_view != View::Dashboard,
                        connect_clicked[sender] => move |_| {
                            sender.input(Input::SetView(View::Dashboard));
                        },
                    },

                    pack_start = &gtk::Button {
                        set_label: "Devices",
                        #[watch]
                        set_icon_name: if model.is_connected {
                            "bluetooth-symbolic"
                        } else {
                            "bluetooth-disconnected-symbolic"
                        },
                        #[watch]
                        set_visible: model.active_view == View::Dashboard,
                        connect_clicked[sender] => move |_| {
                            sender.input(Input::SetView(View::Devices));
                        },
                    },

                    pack_start = &gtk::Button {
                        set_label: "Open",
                        set_icon_name: "document-send-symbolic",
                        // set_sensitive: watch!(file_chooser.file().is_some()),
                        #[watch]
                        set_visible: model.active_view == View::FileChooser,
                        connect_clicked[sender, file_chooser] => move |_| {
                            if let Some(file) = file_chooser.file() {
                                sender.input(Input::FirmwareUpdate(file.path().unwrap()));
                            }
                        },
                    }
                },

                #[local]
                toast_overlay -> adw::ToastOverlay {
                    // TODO: Use Relm 0.5 conditional widgets here (automatic stack)
                    // I can't make it work here for some reason for now.
                    #[wrap(Some)]
                    set_child = &gtk::Stack {
                        add_named[Some("dashboard_view")] = &adw::Clamp {
                            set_maximum_size: 400,
                            // set_visible: watch!(components.dashboard.model.device.is_some()),
                            set_child: Some(model.dashboard.widget()),
                        },
                        add_named[Some("devices_view")] = &adw::Clamp {
                            set_maximum_size: 400,
                            set_child: Some(model.devices.widget()),
                        },
                        #[name(file_chooser)]
                        add_named[Some("file_view")] = &gtk::FileChooserWidget {
                            set_action: gtk::FileChooserAction::Open,
                            set_filter = &gtk::FileFilter {
                                add_pattern: "*.zip"
                            },
                        },
                        add_named[Some("fwupd_view")] = &adw::Clamp {
                            set_maximum_size: 400,
                            set_child: Some(model.fwupd.widget()),
                        },
                        #[watch]
                        set_visible_child_name: match model.active_view {
                            View::Dashboard => "dashboard_view",
                            View::Devices => "devices_view",
                            View::FileChooser => "file_view",
                            View::FirmwareUpdate => "fwupd_view",
                        },
                    },
                },
            },
        }
    }

    fn init(adapter: Self::InitParams, root: &Self::Root, sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        // Components
        let dashboard = dashboard::Model::builder()
            .launch(())
            .forward(&sender.input, |message| match message {
                dashboard::Output::OpenFileDialog => Input::SetView(View::FileChooser),
                dashboard::Output::Notification(text) => Input::Notification(text),
            });

        let devices = devices::Model::builder()
            .launch(adapter)
            .forward(&sender.input, |message| match message {
                devices::Output::DeviceConnected(address) => Input::DeviceConnected(address),
                devices::Output::Notification(text) => Input::Notification(text),
            });

        let fwupd = fwupd::Model::builder().launch(()).detach();

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


    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        match msg {
            Input::SetView(view) => {
                self.active_view = view;
            }
            Input::DeviceConnected(device) => {
                self.is_connected = true;
                self.active_view = View::Dashboard;
                sender.command(move |out, shutdown| {
                    // TODO: Remove this extra clone once ComponentSender::command
                    // is patched to accept FnOnce instead of Fn
                    let device = device.clone();
                    let task = async move {
                        let infinitime = bt::InfiniTime::new(device).await.unwrap();
                        out.send(CommandOutput::DeviceReady(Arc::new(infinitime)));
                    };
                    shutdown.register(task).drop_on_shutdown()
                })
            }
            Input::FirmwareUpdate(filename) => {
                if let Some(infinitime) = self.infinitime.clone() {
                    self.fwupd.emit(fwupd::Input::FirmwareUpdate(filename, infinitime));
                    sender.input(Input::SetView(View::FirmwareUpdate));
                }
            }
            Input::Notification(message) => {
                self.notify(&message);
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, _sender: &ComponentSender<Self>) {
        match msg {
            CommandOutput::DeviceReady(infinitime) => {
                self.infinitime = Some(infinitime.clone());
                self.dashboard.emit(dashboard::Input::Connected(infinitime));
            }
        }
    }
}



#[derive(Debug, PartialEq)]
enum View {
    Dashboard,
    Devices,
    FileChooser,
    FirmwareUpdate,
}


pub fn run(adapter: Arc<bluer::Adapter>) {
    // Init GTK before libadwaita (ToastOverlay)
    gtk::init().unwrap();

    // Run app
    let app = RelmApp::new("io.gitlab.azymohliad.WatchMate");
    app.run::<Model>(adapter);
}
