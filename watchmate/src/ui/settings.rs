use crate::ui;
use gtk::{gio, prelude::{OrientableExt, WidgetExt, ButtonExt, SettingsExtManual}};
use adw::prelude::{PreferencesPageExt, PreferencesGroupExt, PreferencesRowExt, ActionRowExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component};


#[derive(Debug)]
pub enum Output {
    SetAutoReconnect(bool),
}

pub struct Model {
}


#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
    type Init = gio::Settings;
    type Input = ();
    type Output = Output;
    type Widgets = Widgets;

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
            },

            adw::PreferencesPage {
                add = &adw::PreferencesGroup {
                    #[name = "autoreconnect_switch"]
                    add = &adw::SwitchRow {
                        set_title: "Automatically reconnect",
                        set_subtitle: "When BLE connection is lost",
                        connect_active_notify[sender] => move |wgt| {
                            _ = sender.output(Output::SetAutoReconnect(wgt.is_active()));
                        }
                    }
                }
            }
        }
    }

    fn init(persistent_settings: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {};
        let widgets = view_output!();
        persistent_settings.bind("auto-reconnect-enabled", &widgets.autoreconnect_switch, "active").build();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>, _root: &Self::Root) {
    }
}

