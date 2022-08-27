use crate::inft::gh;
use std::path::PathBuf;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use relm4::{adw, gtk, ComponentController, ComponentParts, ComponentSender, Component, Controller, WidgetPlus, JoinHandle};
use relm4_components::{open_dialog::*, save_dialog::*};
use anyhow::Result;

#[derive(Debug)]
pub enum Input {
    None,
    FirmwareReleasesRequest,
    FirmwareReleaseNotes(u32),
    
    // Firmware Download
    FirmwareDownload(u32),
    FirmwareDownloadCancel,
    FirmwareDownloadFinished(Result<Vec<u8>>),
    FirmwareSave(PathBuf),

    // Firmware Update
    FirmwareUpdateOpenDialog,
    FirmwareUpdateFromUrl(u32),
    FirmwareUpdateFromFile(PathBuf),
}

#[derive(Debug)]
pub enum Output {
    FirmwareUpdateFromFile(PathBuf),
    FirmwareUpdateFromUrl(String),
    FirmwareVersionLatest(Option<String>),
    Notification(&'static str),
}

#[derive(Debug)]
pub enum CommandOutput {
    FirmwareReleasesResponse(Result<Vec<gh::ReleaseInfo>>),
    FirmwareSaveResponse(Result<()>),
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
    open_file_dialog: Controller<OpenDialog>,
    save_file_dialog: Controller<SaveDialog>,
}

impl Model {
    fn save_firmware_file(&mut self, sender: ComponentSender<Self>) {
        if self.download_content.is_some() && self.download_filepath.is_some() {
            let content = self.download_content.take().unwrap();
            let filepath = self.download_filepath.take().unwrap();
            sender.oneshot_command(async move {
                CommandOutput::FirmwareSaveResponse(
                    gh::save_dfu_file(&content, filepath).await
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
                        sender.input(Input::FirmwareUpdateFromUrl(releases_dropdown.selected()));
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
                    sender.input(Input::FirmwareUpdateOpenDialog);
                },
            },
        }                                
    }

    fn init(main_window: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let file_filter = gtk::FileFilter::new();
        file_filter.add_pattern("*.zip");
        let open_file_dialog = OpenDialog::builder()
            .transient_for_native(&main_window)
            .launch(OpenDialogSettings {
                create_folders: false,
                filters: vec![file_filter],
                ..Default::default()
            })
            .forward(&sender.input, |message| match message {
                OpenDialogResponse::Accept(path) => Input::FirmwareUpdateFromFile(path),
                OpenDialogResponse::Cancel => Input::None,
            });

        let save_file_dialog = SaveDialog::builder()
            .transient_for_native(&main_window)
            .launch(SaveDialogSettings::default())
            .forward(&sender.input, |message| match message {
                SaveDialogResponse::Accept(path) => Input::FirmwareSave(path),
                SaveDialogResponse::Cancel => Input::FirmwareDownloadCancel,
            });

        let model = Model {
            releases: FirmwareReleasesState::default(),
            tags: None,
            download_task: None,
            download_content: None,
            download_filepath: None,
            open_file_dialog,
            save_file_dialog,
        };

        let widgets = view_output!();
        sender.input(Input::FirmwareReleasesRequest);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Input::None => {}
            Input::FirmwareReleasesRequest => {
                self.releases = FirmwareReleasesState::Requested;
                sender.oneshot_command(async move {
                    CommandOutput::FirmwareReleasesResponse(gh::list_releases().await)
                });
            }
            Input::FirmwareReleaseNotes(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.releases {
                    gtk::show_uri(None as Option<&adw::ApplicationWindow>, &releases[index as usize].url, 0);
                }
            }
            Input::FirmwareDownload(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.releases {
                    match releases[index as usize].get_dfu_asset() {
                        Some(asset) => {
                            let url = asset.url.clone();
                            let filename = asset.name.clone();
                            let task = relm4::spawn(async move {
                                sender.input(Input::FirmwareDownloadFinished(
                                    gh::download_dfu_content(url.as_str()).await
                                ))
                            });
                            self.download_task = Some(task);
                            self.save_file_dialog.emit(SaveDialogMsg::SaveAs(filename));
                        }
                        None => {
                            sender.output(Output::Notification("DFU file not found"));
                        }
                    }
                }                
            }
            Input::FirmwareDownloadCancel => {
                self.download_task.take().map(|h| h.abort());
                self.download_content = None;
                self.download_filepath = None;
            }
            Input::FirmwareDownloadFinished(result) => {
                self.download_task.take().map(|h| h.abort());
                match result {
                    Ok(content) => {
                        self.download_content = Some(content);
                        self.save_firmware_file(sender);
                    }
                    Err(error) => {
                        self.download_content = None;
                        log::error!("Failed to download DFU file: {}", error);
                        sender.output(Output::Notification("Failed to download DFU file"));
                    }
                }
            }
            Input::FirmwareSave(filepath) => {
                self.download_filepath = Some(filepath);
                self.save_firmware_file(sender);
            }
            Input::FirmwareUpdateOpenDialog => {
                self.open_file_dialog.emit(OpenDialogMsg::Open);
            }
            Input::FirmwareUpdateFromUrl(index) => {
                if let FirmwareReleasesState::Some(releases) = &self.releases {
                    match releases[index as usize].get_dfu_asset() {
                        Some(asset) => {
                            sender.output(Output::FirmwareUpdateFromUrl(asset.url.clone()));
                        }
                        None => {
                            sender.output(Output::Notification("DFU file not found"));
                        }
                    }
                }
            }
            Input::FirmwareUpdateFromFile(filepath) => {
                sender.output(Output::FirmwareUpdateFromFile(filepath));
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, sender: ComponentSender<Self>) {
        match msg {
            CommandOutput::FirmwareReleasesResponse(response) => match response {
                Ok(releases) => {
                    let tags = releases.iter().map(|r| r.tag.as_str()).collect::<Vec<&str>>();
                    let latest = tags.first().map(|t| t.to_string());
                    self.tags = Some(gtk::StringList::new(&tags));
                    self.releases = FirmwareReleasesState::Some(releases);
                    sender.output(Output::FirmwareVersionLatest(latest));
                }
                Err(error) => {
                    self.tags = None;
                    self.releases = FirmwareReleasesState::Error;
                    sender.output(Output::FirmwareVersionLatest(None));
                    log::error!("Failed to fetch firmware releases: {error}");
                }
            }
            CommandOutput::FirmwareSaveResponse(response) => match response {
                Ok(()) => {
                    sender.output(Output::Notification("Firmware downloaded"));
                }
                Err(error) => {
                    log::error!("Failed to save firmware file: {error}");
                    sender.output(Output::Notification("Failed to save DFU file"));
                }
            }
        }
    }
}

