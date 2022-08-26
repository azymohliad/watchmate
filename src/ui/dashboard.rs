use crate::inft::{bt, gh};
use super::media_player;
use std::{sync::Arc, path::PathBuf};
use futures::{pin_mut, StreamExt};
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, ListBoxRowExt, WidgetExt};
use adw::prelude::{PreferencesRowExt, ExpanderRowExt};
use relm4::{adw, gtk, ComponentController, ComponentParts, ComponentSender, Component, Controller, Sender, WidgetPlus};
use anyhow::Result;
use version_compare::Version;



#[derive(Debug)]
pub enum Input {
    Connected(Arc<bt::InfiniTime>),
    Disconnected,
    FirmwareReleasesRequest,
    FirmwareReleaseNotes(u32),
    FirmwareDownload(u32),
    FirmwareUpdate(u32),
}

#[derive(Debug)]
pub enum Output {
    FirmwareUpdateFromFile,
    FirmwareUpdateFromUrl(String),
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
    FirmwareReleases(Result<Vec<gh::ReleaseInfo>>),
    FirmwareDownloaded(PathBuf),
    Notification(String),
}

#[derive(Debug, Default, PartialEq)]
pub enum FirmwareReleasesState {
    #[default]
    None,
    Requested,
    Some(Vec<gh::ReleaseInfo>),
    Error,
}

impl FirmwareReleasesState {
    pub fn as_option(&self) -> Option<&Vec<gh::ReleaseInfo>> {
        match &self {
            FirmwareReleasesState::Some(r) => Some(r),
            _ => None,
        }
    }

    pub fn _is_none(&self) -> bool {
        self == &FirmwareReleasesState::None
    }

    pub fn is_requested(&self) -> bool {
        self == &FirmwareReleasesState::Requested
    }

    pub fn is_some(&self) -> bool {
        self.as_option().is_some()
    }

    pub fn _is_error(&self) -> bool {
        self == &FirmwareReleasesState::Error
    }
}

pub struct Model {
    // UI state
    // - InfiniTime data
    battery_level: Option<u8>,
    heart_rate: Option<u8>,
    alias: Option<String>,
    address: Option<String>,
    fw_version: Option<String>,
    // - Firmware releases
    fw_update_available: bool,
    fw_downloading: bool,
    fw_releases: FirmwareReleasesState,
    fw_tags: Option<gtk::StringList>,
    // - Media Players
    // Components
    media_player_controller: Controller<media_player::Model>,
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

    fn check_fw_update_available(&mut self) {
        let latest = self.fw_releases.as_option()
            .and_then(|l| l.first())
            .and_then(|r| Version::from(&r.tag));
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
    type Init = ();
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
                            },

                            gtk::Label {
                                set_label: "Media Player Control",
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
                                    set_child: Some(model.media_player_controller.widget()),
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

                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Vertical,
                                            set_margin_all: 12,
                                            set_spacing: 10,

                                            gtk::Label {
                                                set_label: "Update from GitHub release",
                                                set_halign: gtk::Align::Start,
                                            },

                                            gtk::Box {
                                                set_spacing: 10,

                                                #[name(releases_dropdown)]
                                                gtk::DropDown {
                                                    set_hexpand: true,
                                                    #[watch]
                                                    set_visible: model.fw_releases.is_some(),
                                                    #[watch]
                                                    set_model: model.fw_tags.as_ref(),
                                                },

                                                adw::SplitButton {
                                                    #[watch]
                                                    set_visible: model.fw_releases.is_some(),
                                                    #[watch]
                                                    set_sensitive: !model.fw_downloading,
                                                    set_label: "Flash",
                                                    connect_clicked[sender, releases_dropdown] => move |_| {
                                                        sender.input(Input::FirmwareUpdate(releases_dropdown.selected()));
                                                    },
                                                    #[wrap(Some)]
                                                    set_popover = &gtk::Popover {
                                                        gtk::Box {
                                                            set_spacing: 10,
                                                            set_orientation: gtk::Orientation::Vertical,

                                                            gtk::Button {
                                                                set_label: "Download Only",
                                                                connect_clicked[sender, releases_dropdown] => move |_| {
                                                                    sender.input(Input::FirmwareDownload(releases_dropdown.selected()));
                                                                },
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

                                                gtk::Label {
                                                    set_hexpand: true,
                                                    #[watch]
                                                    set_visible: !model.fw_releases.is_some(),
                                                    #[watch]
                                                    set_label: match &model.fw_releases {
                                                        FirmwareReleasesState::None => "Firmware releases are not loaded",
                                                        FirmwareReleasesState::Requested => "Getting firmware releases...",
                                                        FirmwareReleasesState::Error => "Failed to get firmware releases",
                                                        _ => "",
                                                    },
                                                },

                                                if model.fw_downloading || model.fw_releases.is_requested() {
                                                    gtk::Spinner {
                                                        set_spinning: true,
                                                    }
                                                } else {
                                                    gtk::Button {
                                                        set_tooltip_text: Some("Refresh releases list"),
                                                        set_icon_name: "view-refresh-symbolic",
                                                        connect_clicked[sender] => move |_| {
                                                            sender.input(Input::FirmwareReleasesRequest);
                                                        },
                                                    }
                                                }
                                            },

                                            gtk::Separator {
                                                set_orientation: gtk::Orientation::Horizontal,
                                            },

                                            gtk::Label {
                                                set_label: "Update from file",
                                                set_halign: gtk::Align::Start,
                                            },

                                            gtk::Button {
                                                set_label: "Select File",
                                                connect_clicked[sender] => move |_| {
                                                    sender.output(Output::FirmwareUpdateFromFile);
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
                }
            },
        }
    }

    fn init(_: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {

        let media_player_controller = media_player::Model::builder()
            .launch(())
            .detach();

        let model = Model {
            battery_level: None,
            heart_rate: None,
            alias: None,
            address: None,
            fw_version: None,
            fw_update_available: false,
            fw_downloading: false,
            fw_releases: FirmwareReleasesState::default(),
            fw_tags: None,
            media_player_controller,
            infinitime: None,
        };

        let widgets = view_output!();
        sender.input(Input::FirmwareReleasesRequest);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Input::Connected(infinitime) => {
                self.infinitime = Some(infinitime.clone());
                let infinitime_ = infinitime.clone();
                sender.command(move |out, shutdown| {
                    shutdown.register(async move {
                        // Read inital data
                        if let Err(error) = Self::read_info(infinitime_, out.clone()).await {
                            log::error!("Failed to read data: {}", error);
                            out.send(CommandOutput::Notification(format!("Failed to read data")));
                        }
                    }).drop_on_shutdown()
                });
                // Listed to data update notifications
                let infinitime_ = infinitime.clone();
                sender.command(move |out, shutdown| {
                    shutdown.register(async move {
                        if let Ok(hr_stream) = infinitime_.get_heart_rate_stream().await {
                            pin_mut!(hr_stream);
                            while let Some(hr) = hr_stream.next().await {
                                out.send(CommandOutput::HeartRate(hr));
                            }
                        }
                    }).drop_on_shutdown()
                });
                // Propagate to components
                self.media_player_controller.emit(
                    media_player::Input::Device(Some(infinitime.clone()))
                );
            }
            Input::Disconnected => {
                self.battery_level = None;
                self.heart_rate = None;
                self.alias = None;
                self.address = None;
                self.fw_version = None;
                self.infinitime = None;

                // Propagate to components
                self.media_player_controller.emit(media_player::Input::Device(None));
            }
            Input::FirmwareReleasesRequest => {
                self.fw_releases = FirmwareReleasesState::Requested;
                sender.oneshot_command(async move {
                    CommandOutput::FirmwareReleases(gh::list_releases().await)
                });
            }
            Input::FirmwareReleaseNotes(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.fw_releases {
                    gtk::show_uri(None as Option<&adw::ApplicationWindow>, &releases[index as usize].url, 0);
                }
            }
            Input::FirmwareDownload(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.fw_releases {
                    match releases[index as usize].get_dfu_asset() {
                        Some(asset) => {
                            self.fw_downloading = true;
                            let url = asset.url.clone();
                            let filepath = gh::get_download_filepath(&asset.name).unwrap();
                            sender.oneshot_command(async move {
                                match gh::download_dfu_file(url.as_str(), filepath.as_path()).await {
                                    Ok(()) => {
                                        CommandOutput::FirmwareDownloaded(filepath)
                                    }
                                    Err(error) => {
                                        log::error!("Failed to download of DFU file: {}", error);
                                        CommandOutput::Notification(format!("Failed to fetch firmware releases"))
                                    }
                                }
                            });
                        }
                        None => {
                            sender.output(Output::Notification(format!("DFU file not found")));
                        }
                    }
                }
            }
            Input::FirmwareUpdate(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.fw_releases {
                    match releases[index as usize].get_dfu_asset() {
                        Some(asset) => {
                            sender.output(Output::FirmwareUpdateFromUrl(asset.url.clone()));
                        }
                        None => {
                            sender.output(Output::Notification(format!("DFU file not found")));
                        }
                    }
                }
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, sender: ComponentSender<Self>) {
        match msg {
            CommandOutput::BatteryLevel(soc) => {
                self.battery_level = Some(soc);
            }
            CommandOutput::HeartRate(rate) => {
                self.heart_rate = Some(rate);
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
            CommandOutput::FirmwareReleases(response) => {
                match response {
                    Ok(releases) => {
                        let tags = releases.iter().map(|r| r.tag.as_str()).collect::<Vec<&str>>();
                        self.fw_tags = Some(gtk::StringList::new(&tags));
                        self.fw_releases = FirmwareReleasesState::Some(releases);
                        self.check_fw_update_available();
                    }
                    Err(_) => {
                        self.fw_tags = None;
                        self.fw_releases = FirmwareReleasesState::Error;
                        self.fw_update_available = false;
                    }
                }
            }
            CommandOutput::FirmwareDownloaded(_filepath) => {
                self.fw_downloading = false;
                sender.output(Output::Notification(format!("Firmware downloaded")));
            }
            CommandOutput::Notification(text) => {
                sender.output(Output::Notification(text));
            }
        }
    }
}

