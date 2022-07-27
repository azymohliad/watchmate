use std::{sync::Arc, path::PathBuf};
use tokio::runtime::Runtime;
use gtk::prelude::{ButtonExt, GtkWindowExt, OrientableExt, WidgetExt, FileChooserExt, FileExt};
use relm4::{adw, gtk,
    Component, ComponentController, ComponentParts, ComponentSender,
    Controller, RelmApp, SimpleComponent
};

mod watch;
mod scanner;

#[derive(Debug)]
enum Input {
    SetView(View),
    DeviceSelected(bluer::Address),
    DeviceConnected(bluer::Address),
    FirmwareUpdate(PathBuf),
    Notification(String),
}

struct Model {
    // UI state
    active_view: View,
    is_connected: bool,
    // Non-UI state
    runtime: Runtime,
    adapter: Arc<bluer::Adapter>,
    toast_overlay: adw::ToastOverlay,
    // Components
    watch: Controller<watch::Model>,
    scanner: Controller<scanner::Model>,
}

impl Model {
    fn notify(&self, message: &str) {
        self.toast_overlay.add_toast(&adw::Toast::new(message));
    }
}

#[relm4::component]
impl SimpleComponent for Model {
    type InitParams = (Runtime, Arc<bluer::Adapter>);
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
                            View::Main => "WatchMate",
                            View::Scan => "Devices",
                            View::FileChooser => "Choose DFU file",
                            View::FirmwareUpdate => "Firmware Upgrade",
                        },
                    },

                    pack_start = &gtk::Button {
                        set_label: "Back",
                        set_icon_name: "go-previous-symbolic",
                        #[watch]
                        set_visible: model.active_view != View::Main,
                        connect_clicked[sender] => move |_| {
                            sender.input(Input::SetView(View::Main));
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
                        set_visible: model.active_view == View::Main,
                        connect_clicked[sender] => move |_| {
                            sender.input(Input::SetView(View::Scan));
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
                    #[wrap(Some)]
                    set_child = &gtk::Stack {
                        add_named[Some("main_view")] = &adw::Clamp {
                            set_maximum_size: 400,
                            // set_visible: watch!(components.watch.model.device.is_some()),
                            set_child: Some(model.watch.widget()),
                        },
                        add_named[Some("scan_view")] = &adw::Clamp {
                            set_maximum_size: 400,
                            set_child: Some(model.scanner.widget()),
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
                        },
                        #[watch]
                        set_visible_child_name: match model.active_view {
                            View::Main => "main_view",
                            View::Scan => "scan_view",
                            View::FileChooser => "file_view",
                            View::FirmwareUpdate => "fwupd_view",
                        },
                    },
                },
            },
        }
    }

    fn init(params: Self::InitParams, root: &Self::Root, sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        // Init params
        let runtime = params.0;
        let adapter = params.1;

        // Components
        let watch = watch::Model::builder()
            .launch(runtime.handle().clone())
            .forward(&sender.input, |message| match message {
                watch::Output::OpenFileDialog => Input::SetView(View::FileChooser),
            });

        let scanner = scanner::Model::builder()
            .launch((runtime.handle().clone(), adapter.clone()))
            .forward(&sender.input, |message| match message {
                scanner::Output::DeviceConnected(address) => Input::DeviceConnected(address),
                scanner::Output::DeviceSelected(address) => Input::DeviceSelected(address),
            });

        let toast_overlay = adw::ToastOverlay::new();

        let model = Model {
            // UI state
            active_view: View::Scan,
            is_connected: false,
            // System
            runtime,
            adapter,
            // Widget handles
            toast_overlay: toast_overlay.clone(),
            // Components
            watch,
            scanner,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }


    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        match msg {
            Input::SetView(view) => {
                self.active_view = view;
            }
            Input::DeviceSelected(address) => {
                match self.adapter.device(address) {
                    Ok(device) => {
                        let send = sender.clone();
                        self.runtime.spawn(async move {
                            match device.connect().await {
                                Ok(()) => send.input(Input::DeviceConnected(address)),
                                Err(error) => eprintln!("Error: {}", error),
                            }
                        });
                    }
                    Err(error) => self.notify(&format!("Error: {}", error)),
                }
            }
            Input::DeviceConnected(address) => {
                println!("Connected: {}", address.to_string());
                self.is_connected = true;
                self.active_view = View::Main;
                match self.adapter.device(address) {
                    Ok(device) => self.watch.emit(watch::Input::Connected(device)),
                    Err(error) => self.notify(&format!("Error: {}", error)),
                }
            }
            Input::FirmwareUpdate(filename) => {
                self.watch.emit(watch::Input::FirmwareUpdate(filename));
                sender.input(Input::SetView(View::FirmwareUpdate));
            }
            Input::Notification(message) => {
                self.notify(&message);
            }
        }
    }
}



#[derive(Debug, PartialEq)]
enum View {
    Main,
    Scan,
    FileChooser,
    FirmwareUpdate,
}


pub fn run(runtime: Runtime, adapter: Arc<bluer::Adapter>) {
    // Init GTK before libadwaita (ToastOverlay)
    gtk::init().unwrap();

    // Run app
    let app = RelmApp::new("io.gitlab.azymohliad.WatchMate");
    app.run::<Model>((runtime, adapter));
}
