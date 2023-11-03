use crate::ui;
use gtk::{
    gio, glib::Propagation, prelude::{
        GtkApplicationExt, OrientableExt, WidgetExt, ButtonExt, SettingsExt,
        SettingsExtManual
    }
};
use adw::prelude::{PreferencesPageExt, PreferencesGroupExt, PreferencesRowExt, ActionRowExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component};
use ashpd::{desktop::background::Background, WindowIdentifier, Error};


#[derive(Debug)]
pub enum Input {
    RunInBackgroundRequest(bool),
    RunInBackgroundResponse(bool),
    AutoStartRequest(bool),
    AutoStartResponse(bool),
}

#[derive(Debug)]
pub enum Output {
    RunInBackground(bool),
    AutoStart(bool),
    AutoReconnect(bool),
}

pub struct Model {
    background_switch: gtk::Switch,
    autostart_switch: gtk::Switch,
    settings: gio::Settings,
}

impl Model {
    fn background_portal_request<F>(&self, autostart: bool, handler: F)
        where F: Fn(Result<Background, Error>) + 'static
    {
        let native_window = relm4::main_application()
            .active_window()
            .and_then(|w| w.native())
            .unwrap();
        relm4::spawn_local(async move {
            let identifier = WindowIdentifier::from_native(&native_window).await;
            let request = Background::request()
                .identifier(identifier)
                .auto_start(autostart)
                .command(["watchmate"])
                .reason("Keep the watch connected, forward notifications, control media player");
            let response = request.send().await.and_then(|r| r.response());
            handler(response);
        });
    }
}


#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
    type Init = gio::Settings;
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    menu! {
        main_menu: {
            "Back to Dashboard" => super::DashboardViewAction,
            "Devices" => super::DevicesViewAction,
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
                set_title_widget = &gtk::Label {
                    set_label: "Settings",
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

            adw::PreferencesPage {
                add = &adw::PreferencesGroup {
                    add = &adw::ActionRow {
                        set_title: "Run in background",
                        set_subtitle: "When closed",
                        #[local]
                        add_suffix = &background_switch -> gtk::Switch {
                            set_active: model.settings.boolean("run-in-background-enabled"),
                            set_valign: gtk::Align::Center,
                            connect_state_set[sender] => move |_, state| {
                                sender.input(Input::RunInBackgroundRequest(state));
                                Propagation::Stop
                            }
                        }
                    },
                    add = &adw::ActionRow {
                        set_title: "Autostart",
                        set_subtitle: "At login",
                        #[local]
                        add_suffix = &autostart_switch -> gtk::Switch {
                            set_active: model.settings.boolean("auto-start-enabled"),
                            set_valign: gtk::Align::Center,
                            connect_state_set[sender] => move |_, state| {
                                sender.input(Input::AutoStartRequest(state));
                                Propagation::Stop
                            }
                        }
                    },
                    #[name = "autoreconnect_switch"]
                    add = &adw::SwitchRow {
                        set_title: "Automatically reconnect",
                        set_subtitle: "When BLE connection is lost",
                        connect_active_notify[sender] => move |wgt| {
                            _ = sender.output(Output::AutoReconnect(wgt.is_active()));
                        }
                    },
                }
            }
        }
    }

    fn init(settings: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            background_switch: gtk::Switch::new(),
            autostart_switch: gtk::Switch::new(),
            settings
        };

        let background_switch = model.background_switch.clone();
        let autostart_switch = model.autostart_switch.clone();
        let widgets = view_output!();
        // Bind simple settings
        model.settings.bind("auto-reconnect-enabled", &widgets.autoreconnect_switch, "active").build();
        // Signal start-up settings
        _ = sender.output(Output::RunInBackground(model.settings.boolean("run-in-background-enabled")));
        _ = sender.output(Output::AutoStart(model.settings.boolean("auto-start-enabled")));
        _ = sender.output(Output::AutoReconnect(model.settings.boolean("auto-reconnect-enabled")));
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::RunInBackgroundRequest(enabled) => {
                if self.background_switch.state() == self.background_switch.is_active() {
                    // Switch state was reverted, do nothing
                } else if enabled {
                    let autostart = self.settings.boolean("auto-start-enabled");
                    self.background_portal_request(autostart, move |r| match r {
                        Ok(response ) => {
                            sender.input(Input::RunInBackgroundResponse(response.run_in_background()));
                        }
                        Err(error) => {
                            sender.input(Input::RunInBackgroundResponse(false));
                            log::error!("Background portal request failed: {error}");
                        }
                    });
                } else {
                    sender.input(Input::RunInBackgroundResponse(false));
                }
            }
            Input::AutoStartRequest(enabled) => {
                if self.autostart_switch.state() == self.autostart_switch.is_active() {
                    // Switch state was reverted, do nothing
                } else {
                    let old_state = self.autostart_switch.state();
                    self.background_portal_request(enabled, move |r| match r {
                        Ok(response) => {
                            sender.input(Input::AutoStartResponse(response.auto_start()));
                        }
                        Err(error) => {
                            sender.input(Input::AutoStartResponse(old_state));
                            log::error!("Background portal request failed: {error}");
                        }
                    });
                }
            }
            Input::RunInBackgroundResponse(enabled) => {
                self.background_switch.set_state(enabled);
                self.background_switch.set_active(enabled);
                _ = self.settings.set_boolean("run-in-background-enabled", enabled);
                _ = sender.output(Output::RunInBackground(enabled));
            }
            Input::AutoStartResponse(enabled) => {
                self.autostart_switch.set_state(enabled);
                self.autostart_switch.set_active(enabled);
                _ = self.settings.set_boolean("auto-start-enabled", enabled);
                _ = sender.output(Output::AutoStart(enabled));
            }
        };
    }
}
