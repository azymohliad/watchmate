use crate::inft::bt;
use super::{media_player, firmware_panel, notifications, AssetType};
use std::{sync::Arc, path::PathBuf};
use futures::{pin_mut, StreamExt};
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use adw::prelude::{PreferencesRowExt, ExpanderRowExt};
use relm4::{adw, gtk, ComponentController, ComponentParts, ComponentSender, Component, Controller, Sender, RelmWidgetExt};
use anyhow::{Result, anyhow};
use version_compare::Version;


#[derive(Debug)]
pub enum Input {
    Connected(Arc<bt::InfiniTime>),
    Disconnected,
    LatestFirmwareVersion(Option<String>),
    FlashAssetFromFile(PathBuf, AssetType),
    FlashAssetFromUrl(String, AssetType),
    Toast(&'static str),
}

#[derive(Debug)]
pub enum Output {
    FlashAssetFromFile(PathBuf, AssetType),
    FlashAssetFromUrl(String, AssetType),
    Toast(&'static str),
    SetView(super::View),
}

#[derive(Debug)]
pub enum CommandOutput {
    BatteryLevel(u8),
    HeartRate(u8),
    StepCount(u32),
    Alias(String),
    Address(String),
    FirmwareVersion(String),
    Toast(&'static str),
}

pub struct Model {
    // UI state
    // - InfiniTime data
    battery_level: Option<u8>,
    heart_rate: Option<u8>,
    step_count: Option<u32>,
    alias: Option<String>,
    address: Option<String>,
    fw_version: Option<String>,
    fw_latest: Option<String>,
    fw_update_available: bool,
    // Components
    player_panel: Controller<media_player::Model>,
    notifications_panel: Controller<notifications::Model>,
    firmware_panel: Controller<firmware_panel::Model>,
    // Other
    infinitime: Option<Arc<bt::InfiniTime>>,
}

impl Model {
    async fn read_info(infinitime: Arc<bt::InfiniTime>, sender: Sender<CommandOutput>) -> Result<()> {
        sender.send(CommandOutput::Address(infinitime.device().address().to_string()))
            .map_err(|_| anyhow!("Relm4 message failure"))?;
        sender.send(CommandOutput::BatteryLevel(infinitime.read_battery_level().await?))
            .map_err(|_| anyhow!("Relm4 message failure"))?;
        sender.send(CommandOutput::HeartRate(infinitime.read_heart_rate().await?))
            .map_err(|_| anyhow!("Relm4 message failure"))?;
        sender.send(CommandOutput::StepCount(infinitime.read_step_count().await?))
            .map_err(|_| anyhow!("Relm4 message failure"))?;
        sender.send(CommandOutput::Alias(infinitime.device().alias().await?))
            .map_err(|_| anyhow!("Relm4 message failure"))?;
        sender.send(CommandOutput::FirmwareVersion(infinitime.read_firmware_version().await?))
            .map_err(|_| anyhow!("Relm4 message failure"))?;
        Ok(())
    }

    fn check_fw_update_available(&mut self) {
        let latest = self.fw_latest.as_ref()
            .and_then(|v| Version::from(v));
        let current = self.fw_version.as_ref()
            .and_then(|v| Version::from(v));
        if let (Some(latest), Some(current)) = (latest, current) {
            self.fw_update_available = latest > current;
        }
    }
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = CommandOutput;
    type Init = adw::ApplicationWindow;
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
                        sender.output(Output::SetView(super::View::Devices)).unwrap();
                    },
                },
            },

            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vexpand: true,

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

                                gtk::ListBoxRow {
                                    set_selectable: false,
                                    #[watch]
                                    set_sensitive: model.step_count.is_some(),

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_margin_all: 12,
                                        set_spacing: 10,

                                        gtk::Label {
                                            set_label: "Step Count",
                                            set_hexpand: true,
                                            set_halign: gtk::Align::Start,
                                        },

                                        gtk::Label {
                                            #[watch]
                                            set_label: match model.step_count {
                                                Some(rate) => format!("{}", rate),
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
                                set_label: "Companion Integration",
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
                                    set_child: Some(model.player_panel.widget()),
                                },

                                gtk::ListBoxRow {
                                    set_selectable: false,
                                    #[watch]
                                    set_sensitive: model.alias.is_some(),
                                    set_child: Some(model.notifications_panel.widget()),
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
                                    set_sensitive: model.fw_version.is_some(),

                                    add_action = &gtk::Box {
                                        set_spacing: 10,

                                        gtk::Label {
                                            #[watch]
                                            set_label: match &model.fw_version {
                                                Some(version) => version,
                                                None => "Loading...",
                                            },
                                            add_css_class: "dim-label",
                                        },

                                        gtk::Image {
                                            #[watch]
                                            set_visible: model.fw_update_available,
                                            set_tooltip_text: Some("Firmware update available"),
                                            set_icon_name: Some("software-update-available-symbolic"),
                                        },
                                    },

                                    add_row = &gtk::ListBoxRow {
                                        set_selectable: false,
                                        #[watch]
                                        set_child: Some(model.firmware_panel.widget()),
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
                                    sender.output(Output::SetView(super::View::Devices)).unwrap();
                                },
                            },
                        }
                    }
                }
            },
        }
    }

    fn init(main_window: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {

        let player_panel = media_player::Model::builder()
            .launch(())
            .detach();

        let notifications_panel = notifications::Model::builder()
            .launch(())
            .detach();

        let firmware_panel = firmware_panel::Model::builder()
            .launch(main_window)
            .forward(&sender.input_sender(), |message| match message {
                firmware_panel::Output::LatestFirmwareVersion(f) => Input::LatestFirmwareVersion(f),
                firmware_panel::Output::FlashAssetFromFile(f, t) => Input::FlashAssetFromFile(f, t),
                firmware_panel::Output::FlashAssetFromUrl(u, t) => Input::FlashAssetFromUrl(u, t),
                firmware_panel::Output::Toast(n) => Input::Toast(n),
            });

        let model = Model {
            battery_level: None,
            heart_rate: None,
            step_count: None,
            alias: None,
            address: None,
            fw_version: None,
            fw_latest: None,
            fw_update_available: false,
            player_panel,
            notifications_panel,
            firmware_panel,
            infinitime: None,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::Connected(infinitime) => {
                self.infinitime = Some(infinitime.clone());
                let infinitime_ = infinitime.clone();
                sender.command(move |out, shutdown| {
                    shutdown.register(async move {
                        // Read inital data
                        if let Err(error) = Self::read_info(infinitime_, out.clone()).await {
                            log::error!("Failed to read data: {}", error);
                            out.send(CommandOutput::Toast("Failed to read data")).unwrap();
                        }
                    }).drop_on_shutdown()
                });
                // Listed to data update notifications
                // TODO:
                //  - Abort streams upon disconnect
                //  - Merge together with tokio::select!
                let infinitime_ = infinitime.clone();
                sender.command(move |out, shutdown| {
                    shutdown.register(async move {
                        if let Ok(hr_stream) = infinitime_.get_heart_rate_stream().await {
                            pin_mut!(hr_stream);
                            while let Some(hr) = hr_stream.next().await {
                                out.send(CommandOutput::HeartRate(hr)).unwrap();
                            }
                        }
                    }).drop_on_shutdown()
                });
                let infinitime_ = infinitime.clone();
                sender.command(move |out, shutdown| {
                    shutdown.register(async move {
                        if let Ok(sc_stream) = infinitime_.get_step_count_stream().await {
                            pin_mut!(sc_stream);
                            while let Some(sc) = sc_stream.next().await {
                                out.send(CommandOutput::StepCount(sc)).unwrap();
                            }
                        }
                    }).drop_on_shutdown()
                });
                // Propagate to components
                self.player_panel.emit(
                    media_player::Input::Device(Some(infinitime.clone()))
                );
                self.notifications_panel.emit(
                    notifications::Input::Device(Some(infinitime.clone()))
                );
            }
            Input::Disconnected => {
                self.battery_level = None;
                self.heart_rate = None;
                self.alias = None;
                self.address = None;
                self.fw_version = None;
                self.fw_update_available = false;
                self.infinitime = None;

                // Propagate to components
                self.player_panel.emit(media_player::Input::Device(None));
                self.notifications_panel.emit(notifications::Input::Device(None));
            }
            Input::LatestFirmwareVersion(latest) => {
                self.fw_latest = latest;
                self.check_fw_update_available();
            }
            Input::FlashAssetFromFile(f, t) => {
                sender.output(Output::FlashAssetFromFile(f, t)).unwrap();
            }
            Input::FlashAssetFromUrl(u, t) => {
                sender.output(Output::FlashAssetFromUrl(u, t)).unwrap();
            }
            Input::Toast(n) => {
                sender.output(Output::Toast(n)).unwrap();
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            CommandOutput::BatteryLevel(soc) => {
                self.battery_level = Some(soc);
            }
            CommandOutput::HeartRate(rate) => {
                self.heart_rate = Some(rate);
            }
            CommandOutput::StepCount(count) => {
                self.step_count = Some(count);
            }
            CommandOutput::Alias(alias) => {
                self.alias = Some(alias);
            }
            CommandOutput::Address(address) => {
                self.address = Some(address);
            }
            CommandOutput::FirmwareVersion(version) => {
                self.fw_version = Some(version);
                self.check_fw_update_available();
            }
            CommandOutput::Toast(text) => {
                sender.output(Output::Toast(text)).unwrap();
            }
        }
    }
}

