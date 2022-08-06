use std::sync::Arc;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use adw::prelude::{PreferencesRowExt, ExpanderRowExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component, Sender, WidgetPlus};
use anyhow::Result;
use version_compare::Version;

use crate::{bt, firmware_download as fw};

#[derive(Debug)]
pub enum Input {
    Connected(Arc<bt::InfiniTime>),
    Disconnected,
    FirmwareReleasesRequest,
    FirmwareReleaseNotes(u32),
}

#[derive(Debug)]
pub enum Output {
    DfuOpenRequest,
    Notification(String),
    SetView(super::View),
}

#[derive(Debug)]
pub enum CommandOutput {
    BatteryLevel(u8),
    HeartRate(u8),
    Alias(String),
    Address(String),
    FirmwareVersion(String),
    FirmwareReleases(Vec<fw::ReleaseInfo>),
}

#[derive(Default)]
pub struct Model {
    // UI state
    // - InfiniTime data
    battery_level: Option<u8>,
    heart_rate: Option<u8>,
    alias: Option<String>,
    address: Option<String>,
    firmware_version: Option<String>,
    // - Firmware releases
    firmware_update_available: bool,
    firmware_releases: Option<Vec<fw::ReleaseInfo>>,
    firmware_tags: Option<gtk::StringList>,
    // Other
    infinitime: Option<Arc<bt::InfiniTime>>,
}

impl Model {
    async fn read_info(infinitime: Arc<bt::InfiniTime>, sender: Sender<CommandOutput>) -> Result<()> {
        sender.send(CommandOutput::Address(infinitime.device().address().to_string()));
        sender.send(CommandOutput::BatteryLevel(infinitime.read_battery_level().await?));
        sender.send(CommandOutput::HeartRate(infinitime.read_heart_rate().await?));
        sender.send(CommandOutput::Alias(infinitime.device().alias().await?));
        sender.send(CommandOutput::FirmwareVersion(infinitime.read_firmware_version().await?));
        Ok(())
    }

    fn check_firmware_latest(&mut self) {
        let latest = self.firmware_releases.as_ref()
            .map(|rs| rs.first()).flatten()
            .map(|r| Version::from(&r.tag)).flatten();
        let current = self.firmware_version.as_ref()
            .map(|v| Version::from(v)).flatten();
        if let (Some(latest), Some(current)) = (latest, current) {
            self.firmware_update_available = latest > current;
        }
    }
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = CommandOutput;
    type InitParams = ();
    type Input = Input;
    type Output = Output;
    type Widgets = Widgets;

    view! {
        gtk::Box {
            set_hexpand: true,
            set_orientation: gtk::Orientation::Vertical,

            adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &gtk::Label {
                    set_label: "WatchMate",
                },

                pack_start = &gtk::Button {
                    set_tooltip_text: Some("Devices"),
                    set_icon_name: "open-menu-symbolic",
                    connect_clicked[sender] => move |_| {
                        sender.output(Output::SetView(super::View::Devices));
                    },
                },
            },

            adw::Clamp {
                set_maximum_size: 400,
                set_vexpand: true,

                if model.infinitime.is_some() {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_margin_all: 12,
                        set_spacing: 10,

                        gtk::ListBox {
                            set_valign: gtk::Align::Start,
                            add_css_class: "boxed-list",

                            gtk::ListBoxRow {
                                set_selectable: false,
                                #[watch]
                                set_sensitive: model.battery_level.is_some(),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_margin_all: 12,
                                    set_spacing: 10,

                                    gtk::Label {
                                        set_label: "Battery",
                                    },

                                    gtk::LevelBar {
                                        set_min_value: 0.0,
                                        set_max_value: 100.0,
                                        #[watch]
                                        set_value: model.battery_level.unwrap_or(0) as f64,
                                        #[watch]
                                        set_visible: model.battery_level.is_some(),
                                        set_hexpand: true,
                                        set_valign: gtk::Align::Center,
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: match model.battery_level {
                                            Some(soc) => format!("{}%", soc),
                                            None => String::from("Loading..."),
                                        }.as_str(),
                                        add_css_class: "dim-label",
                                    },
                                },
                            },

                            gtk::ListBoxRow {
                                set_selectable: false,
                                #[watch]
                                set_sensitive: model.heart_rate.is_some(),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_margin_all: 12,
                                    set_spacing: 10,

                                    gtk::Label {
                                        set_label: "Heart Rate",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: match model.heart_rate {
                                            Some(rate) => format!("{} BPM", rate),
                                            None => String::from("Loading..."),
                                        }.as_str(),
                                        add_css_class: "dim-label",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::End,
                                    },
                                },
                            },
                        },

                        gtk::Label {
                            set_label: "System Info",
                            set_halign: gtk::Align::Start,
                            set_margin_top: 20,
                        },

                        gtk::ListBox {
                            set_valign: gtk::Align::Start,
                            add_css_class: "boxed-list",

                            gtk::ListBoxRow {
                                set_selectable: false,
                                #[watch]
                                set_sensitive: model.alias.is_some(),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_margin_all: 12,
                                    set_spacing: 10,

                                    gtk::Label {
                                        set_label: "Name",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: match &model.alias {
                                            Some(alias) => alias,
                                            None => "Loading...",
                                        },
                                        add_css_class: "dim-label",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::End,
                                    },
                                },
                            },

                            gtk::ListBoxRow {
                                set_selectable: false,
                                #[watch]
                                set_sensitive: model.address.is_some(),

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_margin_all: 12,
                                    set_spacing: 10,

                                    gtk::Label {
                                        set_label: "Address",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::Start,
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: match &model.address {
                                            Some(address) => address,
                                            None => "Loading...",
                                        },
                                        add_css_class: "dim-label",
                                        set_hexpand: true,
                                        set_halign: gtk::Align::End,
                                    },
                                },
                            },

                            adw::ExpanderRow {
                                set_title: "Firmware Version",
                                #[watch]
                                set_sensitive: model.firmware_version.is_some(),

                                add_action = &gtk::Box {
                                    set_spacing: 10,

                                    gtk::Label {
                                        #[watch]
                                        set_label: match &model.firmware_version {
                                            Some(version) => version,
                                            None => "Loading...",
                                        },
                                        add_css_class: "dim-label",
                                    },

                                    gtk::Image {
                                        #[watch]
                                        set_visible: model.firmware_update_available,
                                        set_tooltip_text: Some("Firmware update available"),
                                        set_icon_name: Some("software-update-available-symbolic"),
                                    },
                                },

                                add_row = &gtk::ListBoxRow {
                                    set_selectable: false,

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_margin_all: 12,
                                        set_spacing: 10,

                                        gtk::Label {
                                            set_label: "Update from file",
                                            set_halign: gtk::Align::Start,
                                        },

                                        gtk::Button {
                                            set_label: "Select File",
                                            connect_clicked[sender] => move |_| {
                                                sender.output(Output::DfuOpenRequest);
                                            },
                                        },

                                        gtk::Separator {
                                            set_orientation: gtk::Orientation::Horizontal,
                                        },

                                        gtk::Label {
                                            set_label: "Update from Github release",
                                            set_halign: gtk::Align::Start,
                                        },

                                        gtk::Box {
                                            set_spacing: 10,

                                            #[name(releases_dropdown)]
                                            gtk::DropDown {
                                                set_hexpand: true,
                                                #[watch]
                                                set_sensitive: model.firmware_tags.is_some(),
                                                #[watch]
                                                set_model: model.firmware_tags.as_ref(),
                                            },

                                            adw::SplitButton {
                                                #[watch]
                                                set_sensitive: model.firmware_tags.is_some(),
                                                set_label: "Update",
                                                connect_clicked[sender] => move |_| {},
                                                #[wrap(Some)]
                                                set_popover = &gtk::Popover {
                                                    gtk::Box {
                                                        set_spacing: 10,
                                                        set_orientation: gtk::Orientation::Vertical,

                                                        gtk::Button {
                                                            set_label: "Download Only",
                                                            connect_clicked[sender] => move |_| {},
                                                        },

                                                        gtk::Button {
                                                            set_label: "Release Notes",
                                                            connect_clicked[sender, releases_dropdown] => move |_| {
                                                                sender.input(Input::FirmwareReleaseNotes(releases_dropdown.selected()));
                                                            },
                                                        },
                                                    },
                                                },
                                            },

                                            gtk::Button {
                                                set_tooltip_text: Some("Refresh releases list"),
                                                set_icon_name: "view-refresh-symbolic",
                                                connect_clicked[sender] => move |_| {
                                                    sender.input(Input::FirmwareReleasesRequest);
                                                },
                                            },
                                        },
                                    },
                                },
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
                            set_label: "InfiniTime watch is not connected",
                        },

                        gtk::Button {
                            set_label: "Devices",
                            set_halign: gtk::Align::Center,

                            connect_clicked[sender] => move |_| {
                                sender.output(Output::SetView(super::View::Devices));
                            },
                        },
                    }
                }
            },
        }
    }

    fn init(_: Self::InitParams, root: &Self::Root, sender: &ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self::default();
        let widgets = view_output!();
        sender.input(Input::FirmwareReleasesRequest);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: &ComponentSender<Self>) {
        match msg {
            Input::Connected(infinitime) => {
                self.infinitime = Some(infinitime.clone());
                let sender_ = sender.clone();
                sender.command(move |out, shutdown| {
                    // TODO: Remove this extra clone once ComponentSender::command
                    // is patched to accept FnOnce instead of Fn
                    let infinitime = infinitime.clone();
                    let sender_ = sender_.clone();
                    shutdown.register(async move {
                        // Read inital data
                        if let Err(error) = Self::read_info(infinitime.clone(), out.clone()).await {
                            eprintln!("Failed to read data: {}", error);
                            sender_.output(Output::Notification(format!("Failed to read data")));
                        }
                        // Run data update session
                        infinitime.run_notification_session(move |notification| {
                            match notification {
                                bt::Notification::HeartRate(value) => out.send(CommandOutput::HeartRate(value)),
                            }
                        }).await;
                    }).drop_on_shutdown()
                });
            }
            Input::Disconnected => {
                self.battery_level = None;
                self.heart_rate = None;
                self.alias = None;
                self.address = None;
                self.firmware_version = None;
                self.infinitime = None;
            }
            Input::FirmwareReleasesRequest => {
                let sender_ = sender.clone();
                sender.command(move |out, shutdown| {
                    // TODO: Remove this extra clone once ComponentSender::command
                    // is patched to accept FnOnce instead of Fn
                    let sender_ = sender_.clone();
                    shutdown.register(async move {
                        match fw::list_releases().await {
                            Ok(releases) => {
                                out.send(CommandOutput::FirmwareReleases(releases));
                            }
                            Err(error) => {
                                eprintln!("Failed to fetch the list of firmware releases: {}", error);
                                sender_.output(Output::Notification(format!("Failed to fetch firmware releases")));
                            }
                        }
                    }).drop_on_shutdown()
                });
            }
            Input::FirmwareReleaseNotes(index) => {
                if let Some(releases) = &self.firmware_releases {
                    gtk::show_uri(None as Option<&adw::ApplicationWindow>, &releases[index as usize].url, 0);
                }
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, _sender: &ComponentSender<Self>) {
        match msg {
            CommandOutput::BatteryLevel(soc) => self.battery_level = Some(soc),
            CommandOutput::HeartRate(rate) => self.heart_rate = Some(rate),
            CommandOutput::Alias(alias) => self.alias = Some(alias),
            CommandOutput::Address(address) => self.address = Some(address),
            CommandOutput::FirmwareVersion(version) => {
                self.firmware_version = Some(version);
                self.check_firmware_latest();
            }
            CommandOutput::FirmwareReleases(releases) => {
                let tags = releases.iter().map(|r| r.tag.as_str()).collect::<Vec<&str>>();
                self.firmware_tags = Some(gtk::StringList::new(&tags));
                self.firmware_releases = Some(releases);
                self.check_firmware_latest();
            }
        }
    }
}

