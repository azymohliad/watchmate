use crate::inft::gh;
use std::path::PathBuf;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use relm4::{adw, gtk, ComponentController, ComponentParts, ComponentSender, Component, Controller, JoinHandle, RelmWidgetExt};
use relm4_components::{open_dialog::*, save_dialog::*};
use anyhow::Result;

#[derive(Debug)]
pub enum Input {
    None,
    RequestReleases,
    ReleaseNotes(u32),

    // Firmware Download
    DownloadFirmware(u32),
    DownloadResources(u32),
    DownloadAsset(gh::Asset),
    CancelDownloading,
    FinishedDownloading(Result<Vec<u8>>),
    SaveFile(PathBuf),

    // Firmware Update
    FirmwareUpdateOpenDialog,
    FirmwareUpdateFromReleaseIndex(u32),
    FirmwareUpdateFromFile(PathBuf),
}

#[derive(Debug)]
pub enum Output {
    FirmwareUpdateFromFile(PathBuf),
    FirmwareUpdateFromUrl(String),
    FirmwareVersionLatest(Option<String>),
    Toast(&'static str),
}

#[derive(Debug)]
pub enum CommandOutput {
    FirmwareReleasesResponse(Result<Vec<gh::ReleaseInfo>>),
    SaveFileResponse(Result<()>),
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
    releases: FirmwareReleasesState,
    tags: Option<gtk::StringList>,
    // Firmware download state
    download_task: Option<JoinHandle<()>>,
    download_content: Option<Vec<u8>>,
    download_filepath: Option<PathBuf>,
    // Components
    dfu_open_dialog: Controller<OpenDialog>,
    save_dialog: Controller<SaveDialog>,
}

impl Model {
    fn save_downloaded_file(&mut self, sender: ComponentSender<Self>) {
        if self.download_content.is_some() && self.download_filepath.is_some() {
            let content = self.download_content.take().unwrap();
            let filepath = self.download_filepath.take().unwrap();
            sender.oneshot_command(async move {
                CommandOutput::SaveFileResponse(
                    gh::save_file(&content, filepath).await
                )
            });
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
                    set_visible: model.releases.is_some(),
                    #[watch]
                    set_model: model.tags.as_ref(),
                },

                adw::SplitButton {
                    #[watch]
                    set_visible: model.releases.is_some(),
                    #[watch]
                    set_sensitive: !model.download_task.is_some(),
                    set_label: "Flash",
                    connect_clicked[sender, releases_dropdown] => move |_| {
                        sender.input(Input::FirmwareUpdateFromReleaseIndex(releases_dropdown.selected()));
                    },
                    #[wrap(Some)]
                    set_popover = &gtk::Popover {
                        gtk::Box {
                            set_spacing: 10,
                            set_orientation: gtk::Orientation::Vertical,

                            gtk::Button {
                                set_label: "Flash Resources",
                            },

                            gtk::Button {
                                set_label: "Download",
                                connect_clicked[sender, releases_dropdown] => move |_| {
                                    sender.input(Input::DownloadFirmware(releases_dropdown.selected()));
                                },
                            },

                            gtk::Button {
                                set_label: "Download Resources",
                                connect_clicked[sender, releases_dropdown] => move |_| {
                                    sender.input(Input::DownloadResources(releases_dropdown.selected()));
                                },
                            },

                            gtk::Button {
                                set_label: "Release Notes",
                                connect_clicked[sender, releases_dropdown] => move |_| {
                                    sender.input(Input::ReleaseNotes(releases_dropdown.selected()));
                                },
                            },
                        },
                    },
                },

                gtk::Label {
                    set_hexpand: true,
                    #[watch]
                    set_visible: !model.releases.is_some(),
                    #[watch]
                    set_label: match &model.releases {
                        FirmwareReleasesState::None => "Firmware releases are not loaded",
                        FirmwareReleasesState::Requested => "Getting firmware releases...",
                        FirmwareReleasesState::Error => "Failed to get firmware releases",
                        _ => "",
                    },
                },

                if model.download_task.is_some() || model.releases.is_requested() {
                    gtk::Spinner {
                        set_spinning: true,
                    }
                } else {
                    gtk::Button {
                        set_tooltip_text: Some("Refresh releases list"),
                        set_icon_name: "view-refresh-symbolic",
                        connect_clicked[sender] => move |_| {
                            sender.input(Input::RequestReleases);
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

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

                gtk::Button {
                    set_label: "Firmware...",
                    connect_clicked[sender] => move |_| {
                        sender.input(Input::FirmwareUpdateOpenDialog);
                    },
                },

                gtk::Button {
                    set_label: "Resources...",
                    connect_clicked[sender] => move |_| {
                        // sender.input(Input::FirmwareUpdateOpenDialog);
                    },
                },
            }
        }
    }

    fn init(main_window: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let file_filter = gtk::FileFilter::new();
        file_filter.add_pattern("*.zip");
        let dfu_open_dialog = OpenDialog::builder()
            .transient_for_native(&main_window)
            .launch(OpenDialogSettings {
                create_folders: false,
                filters: vec![file_filter],
                ..Default::default()
            })
            .forward(&sender.input_sender(), |message| match message {
                OpenDialogResponse::Accept(path) => Input::FirmwareUpdateFromFile(path),
                OpenDialogResponse::Cancel => Input::None,
            });

        let save_dialog = SaveDialog::builder()
            .transient_for_native(&main_window)
            .launch(SaveDialogSettings::default())
            .forward(&sender.input_sender(), |message| match message {
                SaveDialogResponse::Accept(path) => Input::SaveFile(path),
                SaveDialogResponse::Cancel => Input::CancelDownloading,
            });

        let model = Model {
            releases: FirmwareReleasesState::default(),
            tags: None,
            download_task: None,
            download_content: None,
            download_filepath: None,
            dfu_open_dialog,
            save_dialog,
        };

        let widgets = view_output!();
        sender.input(Input::RequestReleases);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::None => {}
            Input::RequestReleases => {
                self.releases = FirmwareReleasesState::Requested;
                sender.oneshot_command(async move {
                    CommandOutput::FirmwareReleasesResponse(gh::list_releases().await)
                });
            }
            Input::ReleaseNotes(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.releases {
                    gtk::show_uri(None as Option<&adw::ApplicationWindow>, &releases[index as usize].url, 0);
                }
            }
            Input::DownloadFirmware(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.releases {
                    match releases[index as usize].get_dfu_asset() {
                        Some(asset) => {
                            sender.input(Input::DownloadAsset(asset.clone()));
                        }
                        None => {
                            sender.output(Output::Toast("DFU file not found")).unwrap();
                        }
                    }
                }
            }
            Input::DownloadResources(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.releases {
                    match releases[index as usize].get_resources_asset() {
                        Some(asset) => {
                            sender.input(Input::DownloadAsset(asset.clone()));
                        }
                        None => {
                            sender.output(Output::Toast("Resources file not found")).unwrap();
                        }
                    }
                }
            }
            Input::DownloadAsset(asset) => {
                    let url = asset.url;
                    let filename = asset.name;
                    let task = relm4::spawn(async move {
                        sender.input(Input::FinishedDownloading(
                            gh::download_content(url.as_str()).await
                        ))
                    });
                    self.download_task = Some(task);
                    self.save_dialog.emit(SaveDialogMsg::SaveAs(filename));
            }
            Input::CancelDownloading => {
                self.download_task.take().map(|h| h.abort());
                self.download_content = None;
                self.download_filepath = None;
            }
            Input::FinishedDownloading(result) => {
                self.download_task.take().map(|h| h.abort());
                match result {
                    Ok(content) => {
                        self.download_content = Some(content);
                        self.save_downloaded_file(sender);
                    }
                    Err(error) => {
                        self.download_content = None;
                        log::error!("Failed to download DFU file: {}", error);
                        sender.output(Output::Toast("Failed to download DFU file")).unwrap();
                    }
                }
            }
            Input::SaveFile(filepath) => {
                self.download_filepath = Some(filepath);
                self.save_downloaded_file(sender);
            }
            Input::FirmwareUpdateOpenDialog => {
                self.dfu_open_dialog.emit(OpenDialogMsg::Open);
            }
            Input::FirmwareUpdateFromReleaseIndex(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.releases {
                    match releases[index as usize].get_dfu_asset() {
                        Some(asset) => {
                            sender.output(Output::FirmwareUpdateFromUrl(asset.url.clone())).unwrap();
                        }
                        None => {
                            sender.output(Output::Toast("DFU file not found")).unwrap();
                        }
                    }
                }
            }
            Input::FirmwareUpdateFromFile(filepath) => {
                sender.output(Output::FirmwareUpdateFromFile(filepath)).unwrap();
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            CommandOutput::FirmwareReleasesResponse(response) => match response {
                Ok(releases) => {
                    let tags = releases.iter().map(|r| r.tag.as_str()).collect::<Vec<&str>>();
                    let latest = tags.first().map(|t| t.to_string());
                    self.tags = Some(gtk::StringList::new(&tags));
                    self.releases = FirmwareReleasesState::Some(releases);
                    sender.output(Output::FirmwareVersionLatest(latest)).unwrap();
                }
                Err(error) => {
                    self.tags = None;
                    self.releases = FirmwareReleasesState::Error;
                    sender.output(Output::FirmwareVersionLatest(None)).unwrap();
                    log::error!("Failed to fetch firmware releases: {error}");
                }
            }
            CommandOutput::SaveFileResponse(response) => match response {
                Ok(()) => {
                    sender.output(Output::Toast("Firmware downloaded")).unwrap();
                }
                Err(error) => {
                    log::error!("Failed to save firmware file: {error}");
                    sender.output(Output::Toast("Failed to save DFU file")).unwrap();
                }
            }
        }
    }
}

