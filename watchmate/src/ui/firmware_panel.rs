use crate::ui;
use super::AssetType;
use infinitime::gh;

use std::path::PathBuf;
use relm4::{
    adw, gtk,
    gtk::prelude::*,
    ComponentController, ComponentParts, ComponentSender, Component, Controller, JoinHandle, RelmWidgetExt
};
use relm4_components::{open_dialog::*, save_dialog::*, alert::*};
use anyhow::Result;
use version_compare::Version;


const MIN_FW_VERSION: &'static str = "1.7.0";

#[derive(Debug)]
pub enum Input {
    None,
    RequestReleases,
    SelectedRelease(u32),
    ReleaseNotes,

    // Firmware & Resources Download
    DownloadFirmware,
    DownloadResources,
    DownloadAsset(gh::Asset),
    CancelDownloading,
    FinishedDownloading(Result<Vec<u8>>),
    SaveFile(PathBuf),

    // Firmware & Resources Update
    OpenFirmwareFileDialog,
    FlashFirmwareFromReleaseClicked,
    FlashFirmwareFromRelease,
    FlashFirmwareFromFile(PathBuf),
    OpenResourcesFileDialog,
    FlashResourcesFromRelease,
    FlashResourcesFromFile(PathBuf),
}

#[derive(Debug)]
pub enum Output {
    FlashAssetFromFile(PathBuf, AssetType),
    FlashAssetFromUrl(String, AssetType),
    LatestFirmwareVersion(Option<String>),
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
    selected_index: u32,
    // Firmware download state
    download_task: Option<JoinHandle<()>>,
    download_content: Option<Vec<u8>>,
    download_filepath: Option<PathBuf>,
    // Components
    dfu_open_dialog: Controller<OpenDialog>,
    res_open_dialog: Controller<OpenDialog>,
    save_dialog: Controller<SaveDialog>,
    confirm_dialog: Controller<Alert>,
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

    fn selected_release_info(&self) -> Option<&gh::ReleaseInfo> {
        if let FirmwareReleasesState::Some(releases) = &self.releases {
            releases.get(self.selected_index as usize)
        } else {
            None
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

                gtk::DropDown {
                    set_hexpand: true,
                    #[watch]
                    set_visible: model.releases.is_some(),
                    #[watch]
                    set_model: model.tags.as_ref(),
                    #[wrap(Some)]
                    set_factory = &gtk::SignalListItemFactory {
                        connect_setup => |_, item| {
                            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                            let label = gtk::Label::new(None);
                            let scroll_view = gtk::ScrolledWindow::builder()
                                .vscrollbar_policy(gtk::PolicyType::Never)
                                .child(&label)
                                .build();
                            item.property_expression("item")
                                .chain_property::<gtk::StringObject>("string")
                                .bind(&label, "label", gtk::Widget::NONE);
                            item.set_child(Some(&scroll_view));
                        }
                    },
                    connect_selected_notify[sender] => move |wgt| {
                        sender.input(Input::SelectedRelease(wgt.selected()));
                    }
                },

                adw::SplitButton {
                    #[watch]
                    set_visible: model.releases.is_some(),
                    #[watch]
                    set_sensitive: !model.download_task.is_some(),
                    set_label: "Flash",
                    connect_clicked => Input::FlashFirmwareFromReleaseClicked,
                    #[wrap(Some)]
                    set_popover = &gtk::Popover {
                        gtk::Box {
                            set_spacing: 10,
                            set_orientation: gtk::Orientation::Vertical,

                            gtk::Button {
                                set_label: "Flash Resources",
                                connect_clicked => Input::FlashResourcesFromRelease,
                            },

                            gtk::Button {
                                set_label: "Download",
                                connect_clicked => Input::DownloadFirmware,
                            },

                            gtk::Button {
                                set_label: "Download Resources",
                                connect_clicked => Input::DownloadResources,
                            },

                            gtk::Button {
                                set_label: "Release Notes",
                                connect_clicked => Input::ReleaseNotes,
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
                        connect_clicked => Input::RequestReleases,
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
                set_spacing: 10,

                gtk::Button {
                    set_label: "Firmware",
                    set_hexpand: true,
                    connect_clicked => Input::OpenFirmwareFileDialog,
                },

                gtk::Button {
                    set_label: "Resources",
                    set_hexpand: true,
                    connect_clicked => Input::OpenResourcesFileDialog,
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
                filters: vec![file_filter.clone()],
                ..Default::default()
            })
            .forward(&sender.input_sender(), |message| match message {
                OpenDialogResponse::Accept(path) => Input::FlashFirmwareFromFile(path),
                OpenDialogResponse::Cancel => Input::None,
            });

        let res_open_dialog = OpenDialog::builder()
            .transient_for_native(&main_window)
            .launch(OpenDialogSettings {
                create_folders: false,
                filters: vec![file_filter],
                ..Default::default()
            })
            .forward(&sender.input_sender(), |message| match message {
                OpenDialogResponse::Accept(path) => Input::FlashResourcesFromFile(path),
                OpenDialogResponse::Cancel => Input::None,
            });

        let save_dialog = SaveDialog::builder()
            .transient_for_native(&main_window)
            .launch(SaveDialogSettings::default())
            .forward(&sender.input_sender(), |message| match message {
                SaveDialogResponse::Accept(path) => Input::SaveFile(path),
                SaveDialogResponse::Cancel => Input::CancelDownloading,
            });

        let confirm_dialog = Alert::builder()
            .transient_for(&main_window)
            .launch(AlertSettings {
                text: String::from("Warning: old firmware!"),
                secondary_text: Some(format!("WatchMate might not work with InfiniTime below v{}", MIN_FW_VERSION)),
                confirm_label: String::from("Proceed"),
                cancel_label: String::from("Cancel"),
                option_label: None,
                is_modal: true,
                destructive_accept: true,
            })
            .forward(sender.input_sender(), |message| match message {
                AlertResponse::Confirm => Input::FlashFirmwareFromRelease,
                AlertResponse::Cancel => Input::None,
                AlertResponse::Option => Input::None,
            });

        let model = Model {
            releases: FirmwareReleasesState::default(),
            tags: None,
            selected_index: 0,
            download_task: None,
            download_content: None,
            download_filepath: None,
            dfu_open_dialog,
            res_open_dialog,
            save_dialog,
            confirm_dialog,
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
            Input::SelectedRelease(index) => {
                self.selected_index = index;
            }
            Input::ReleaseNotes => {
                if let Some(release) = self.selected_release_info() {
                    gtk::show_uri(adw::ApplicationWindow::NONE, &release.url, 0);
                }
            }
            Input::DownloadFirmware => {
                if let Some(release) = self.selected_release_info() {
                    match release.get_dfu_asset() {
                        Some(asset) => {
                            sender.input(Input::DownloadAsset(asset.clone()));
                        }
                        None => {
                            ui::BROKER.send(ui::Input::Toast("DFU file not found"));
                        }
                    }
                }
            }
            Input::DownloadResources => {
                if let Some(release) = self.selected_release_info() {
                    match release.get_resources_asset() {
                        Some(asset) => {
                            sender.input(Input::DownloadAsset(asset.clone()));
                        }
                        None => {
                            ui::BROKER.send(ui::Input::Toast("Resources file not found"));
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
                        ui::BROKER.send(ui::Input::Toast("Failed to download DFU file"));
                    }
                }
            }
            Input::SaveFile(filepath) => {
                self.download_filepath = Some(filepath);
                self.save_downloaded_file(sender);
            }
            Input::OpenFirmwareFileDialog => {
                self.dfu_open_dialog.emit(OpenDialogMsg::Open);
            }
            Input::OpenResourcesFileDialog => {
                self.res_open_dialog.emit(OpenDialogMsg::Open);
            }
            Input::FlashFirmwareFromReleaseClicked => {
                if let Some(release) = self.selected_release_info() {
                    let selected = Version::from(&release.tag);
                    let minimum = Version::from(MIN_FW_VERSION);
                    if let (Some(selected), Some(minimum)) = (selected, minimum) {
                        if selected < minimum {
                            self.confirm_dialog.emit(AlertMsg::Show);
                        } else {
                            sender.input(Input::FlashFirmwareFromRelease);
                        }
                    } else {
                        sender.input(Input::FlashFirmwareFromRelease);
                    }
                }
            }
            Input::FlashFirmwareFromRelease => {
                if let Some(release) = self.selected_release_info() {
                    match release.get_dfu_asset() {
                        Some(asset) => {
                            let url = asset.url.clone();
                            let atype = AssetType::Firmware;
                            sender.output(Output::FlashAssetFromUrl(url, atype)).unwrap();
                        }
                        None => {
                            ui::BROKER.send(ui::Input::Toast("DFU file not found"));
                        }
                    }
                }
            }
            Input::FlashFirmwareFromFile(filepath) => {
                let atype = AssetType::Firmware;
                sender.output(Output::FlashAssetFromFile(filepath, atype)).unwrap();
            }
            Input::FlashResourcesFromRelease => {
                if let Some(release) = self.selected_release_info() {
                    match release.get_resources_asset() {
                        Some(asset) => {
                            let url = asset.url.clone();
                            let atype = AssetType::Resources;
                            sender.output(Output::FlashAssetFromUrl(url, atype)).unwrap();
                        }
                        None => {
                            ui::BROKER.send(ui::Input::Toast("DFU file not found"));
                        }
                    }
                }
            }
            Input::FlashResourcesFromFile(filepath) => {
                let atype = AssetType::Resources;
                sender.output(Output::FlashAssetFromFile(filepath, atype)).unwrap();
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
                    sender.output(Output::LatestFirmwareVersion(latest)).unwrap();
                }
                Err(error) => {
                    self.tags = None;
                    self.releases = FirmwareReleasesState::Error;
                    sender.output(Output::LatestFirmwareVersion(None)).unwrap();
                    log::error!("Failed to fetch firmware releases: {error}");
                }
            }
            CommandOutput::SaveFileResponse(response) => match response {
                Ok(()) => {
                    ui::BROKER.send(ui::Input::Toast("Firmware downloaded"));
                }
                Err(error) => {
                    log::error!("Failed to save firmware file: {error}");
                    ui::BROKER.send(ui::Input::Toast("Failed to save DFU file"));
                }
            }
        }
    }
}

