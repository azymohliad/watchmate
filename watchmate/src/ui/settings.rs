use crate::ui;
use gtk::{gio, prelude::{OrientableExt, WidgetExt, ButtonExt, SettingsExt, SettingsExtManual}};
use adw::prelude::{PreferencesPageExt, PreferencesGroupExt, PreferencesRowExt, ActionRowExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component};


#[derive(Debug)]
pub enum Message {
    AutoReconnect(bool),
    RunInBackground(bool),
}

pub struct Model {
    settings: gio::Settings,
}


#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
    type Init = gio::Settings;
    type Input = Message;
    type Output = Message;
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
                    #[name = "autoreconnect_switch"]
                    add = &adw::SwitchRow {
                        set_title: "Automatically reconnect",
                        set_subtitle: "When BLE connection is lost",
                        connect_active_notify[sender] => move |wgt| {
                            _ = sender.output(Message::AutoReconnect(wgt.is_active()));
                        }
                    },
                    #[name = "background_switch"]
                    add = &adw::SwitchRow {
                        set_title: "Run in background",
                        set_subtitle: "When closed",
                        connect_active_notify[sender] => move |wgt| {
                            _ = sender.output(Message::RunInBackground(wgt.is_active()));
                        }
                    },
                }
            }
        }
    }

    fn init(settings: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self { settings };
        let widgets = view_output!();
        model.settings.bind("auto-reconnect-enabled", &widgets.autoreconnect_switch, "active").build();
        model.settings.bind("run-in-background-enabled", &widgets.background_switch, "active").build();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>, _root: &Self::Root) {
        let result = match msg {
            Message::AutoReconnect(value) => {
                self.settings.set_boolean("auto-reconnect-enabled", value)
            }
            Message::RunInBackground(value) => {
                self.settings.set_boolean("run-in-background-enabled", value)
            }
        };
        if let Err(error) = result {
            log::error!("Failed to update setting: {}", error);
        }
    }
}

