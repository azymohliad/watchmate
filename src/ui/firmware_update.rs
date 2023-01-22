use crate::inft::{bt::{self, ProgressEvent, InfiniTime}, gh};
use std::{sync::Arc, path::PathBuf};
use tokio::{fs::File, io::AsyncReadExt};
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component, JoinHandle, RelmWidgetExt};

#[derive(Debug)]
pub enum Input {
    Connected(Arc<bt::InfiniTime>),
    Disconnected,

    FlashAssetFromFile(PathBuf, AssetType),
    FlashAssetFromUrl(String, AssetType),

    ContentReady(Vec<u8>),

    OtaProgress(ProgressEvent),
    OtaFinished,
    OtaFailed(String),

    Retry,
    Abort,
}

#[derive(Debug)]
pub enum Output {
    SetView(super::View),
}

pub enum Source {
    File(Arc<PathBuf>),
    Url(Arc<String>),
}

#[derive(PartialEq, Default)]
pub enum State {
    InProgress,
    Aborted,
    #[default]
    Finished,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum AssetType {
    #[default]
    Firmware,
    Resources,
}

impl AssetType {
    fn name(&self) -> &'static str {
        match self {
            AssetType::Firmware => "Firmware",
            AssetType::Resources => "Resources",
        }
    }
}

#[derive(Default)]
pub struct Model {
    progress_status: String,
    progress_current: u32,
    progress_total: u32,
    state: State,
    asset_type: AssetType,
    asset_content: Option<Arc<Vec<u8>>>,
    asset_source: Option<Source>,

    infinitime: Option<Arc<bt::InfiniTime>>,
    task_handle: Option<JoinHandle<()>>,
}

impl Model {
    fn download_asset(url: Arc<String>, sender: ComponentSender<Self>) -> JoinHandle<()> {
        relm4::spawn(async move {
            match gh::download_content(url.as_str()).await {
                Ok(content) => sender.input(Input::ContentReady(content)),
                Err(_) => sender.input(Input::OtaFailed("Downloading failed".to_string())),
            }
        })
    }

    fn read_asset_file(filepath: Arc<PathBuf>, sender: ComponentSender<Self>) -> JoinHandle<()> {
        relm4::spawn(async move {
            match File::open(filepath.as_path()).await {
                Ok(mut file) => {
                    let mut content = Vec::new();
                    match file.read_to_end(&mut content).await {
                        Ok(_) => sender.input(Input::ContentReady(content)),
                        Err(_) => sender.input(Input::OtaFailed("Failed to open file".to_string())),
                    }
                }
                Err(err) => {
                    sender.input(Input::OtaFailed("Failed to read file".to_string()));
                    log::error!("Failed to read file '{:?}': {}", &filepath, err)
                }
            }
        })
    }

    fn flash_asset(infinitime: Arc<InfiniTime>, content: Arc<Vec<u8>>, asset_type: AssetType, sender: ComponentSender<Self>) -> JoinHandle<()> {
        let (progress_tx, mut progress_rx) = bt::progress_channel(32);

        let sender_ = sender.clone();
        let progress_updater = async move {
            while let Some(event) = progress_rx.recv().await {
                sender_.input(Input::OtaProgress(event));
            }
        };

        let flasher = async move {
            match asset_type {
                AssetType::Firmware => {
                    infinitime.firmware_upgrade(&content, Some(progress_tx)).await
                }
                AssetType::Resources => {
                    infinitime.upload_resources(&content, Some(progress_tx)).await
                }
            }
        };

        relm4::spawn(async move {
            let (_, result) = tokio::join!(progress_updater, flasher);
            match result {
                Ok(()) => sender.input(Input::OtaFinished),
                Err(err) => sender.input(Input::OtaFailed(err.to_string())),
            }
        })
    }
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
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
                    set_label: "Firmware Update",
                },

                pack_start = &gtk::Button {
                    set_tooltip_text: Some("Back"),
                    set_icon_name: "go-previous-symbolic",
                    #[watch]
                    set_visible: model.state != State::InProgress,
                    connect_clicked[sender] => move |_| {
                        sender.output(Output::SetView(super::View::Dashboard)).unwrap();
                    },
                },
            },

            adw::Clamp {
                set_maximum_size: 400,
                set_vexpand: true,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 12,
                    set_spacing: 10,
                    set_valign: gtk::Align::Center,

                    gtk::Label {
                        #[watch]
                        set_label: &model.progress_status,
                        set_halign: gtk::Align::Center,
                        set_margin_top: 20,
                    },

                    gtk::LevelBar {
                        set_min_value: 0.0,
                        #[watch]
                        set_max_value: model.progress_total as f64,
                        #[watch]
                        set_value: model.progress_current as f64,
                        #[watch]
                        set_visible: model.state == State::InProgress && model.progress_current > 0,
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &format!("{:.1} KB / {:.1} KB", model.progress_current as f32 / 1024.0, model.progress_total as f32 / 1024.0),
                        #[watch]
                        set_visible: model.state == State::InProgress && model.progress_current > 0,
                    },

                    gtk::Spinner {
                        #[watch]
                        set_visible: model.state == State::InProgress && model.progress_current == 0,
                        set_spinning: true,
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 10,
                        set_halign: gtk::Align::Center,

                        gtk::Button {
                            set_label: "Abort",
                            add_css_class:"destructive-action",
                            #[watch]
                            set_visible: model.state == State::InProgress,
                            connect_clicked[sender] => move |_| sender.input(Input::Abort),
                        },

                        gtk::Button {
                            set_label: "Retry",
                            #[watch]
                            set_visible: model.state == State::Aborted,
                            connect_clicked[sender] => move |_| sender.input(Input::Retry),
                        },

                        gtk::Button {
                            set_label: "Back",
                            #[watch]
                            set_visible: model.state != State::InProgress,
                            connect_clicked[sender] => move |_| {
                                sender.output(Output::SetView(super::View::Dashboard)).unwrap();
                            },
                        },
                    }
                },
            },
        }
    }

    fn init(_: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self::default();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::Connected(infinitime) => {
                self.infinitime = Some(infinitime);
            }
            Input::Disconnected => {
                self.infinitime = None;
            }
            Input::FlashAssetFromFile(filepath, asset_type) => {
                let filepath = Arc::new(filepath);
                self.progress_status = format!("Reading {} file", asset_type.name().to_lowercase());
                self.progress_current = 0;
                self.progress_total = 0;
                self.state = State::InProgress;
                self.asset_type = asset_type;
                self.asset_source = Some(Source::File(filepath.clone()));
                self.task_handle = Some(Self::read_asset_file(filepath.clone(), sender));
            }
            Input::FlashAssetFromUrl(url, asset_type) => {
                let url = Arc::new(url);
                self.progress_status = format!("Downloading {}", asset_type.name().to_lowercase());
                self.progress_current = 0;
                self.progress_total = 0;
                self.state = State::InProgress;
                self.asset_type = asset_type;
                self.asset_source = Some(Source::Url(url.clone()));
                self.task_handle = Some(Self::download_asset(url.clone(), sender));
            }
            Input::ContentReady(content) => {
                if let Some(infinitime) = self.infinitime.clone() {
                    let content = Arc::new(content);
                    self.asset_source = None;
                    self.asset_content = Some(content.clone());
                    self.task_handle = Some(Self::flash_asset(infinitime, content, self.asset_type, sender));
                }
            }
            Input::OtaFinished => {
                self.progress_status = format!("{} update complete :)", self.asset_type.name());
                self.state = State::Finished;
                self.task_handle = None;
                self.asset_content = None;
            }
            Input::OtaFailed(message) => {
                self.progress_status = format!("{} update failed: {}", self.asset_type.name(), message);
                self.state = State::Aborted;
                self.task_handle = None;
            }
            Input::OtaProgress(event) => {
                match event {
                    ProgressEvent::Message(text) => {
                        self.progress_status = text;
                    }
                    ProgressEvent::Numbers { current, total } => {
                        self.progress_current = current;
                        self.progress_total = total;
                    }
                }
            }
            Input::Retry => {
                self.progress_current = 0;
                self.progress_total = 0;
                if let Some(content) = self.asset_content.clone() {
                    if let Some(infinitime) = self.infinitime.clone() {
                        self.state = State::InProgress;
                        self.task_handle = Some(Self::flash_asset(infinitime, content, self.asset_type, sender));
                    }
                } else {
                    match &self.asset_source {
                        Some(Source::File(filepath)) => {
                            self.task_handle = Some(Self::read_asset_file(filepath.clone(), sender));
                        }
                        Some(Source::Url(url)) => {
                            self.task_handle = Some(Self::download_asset(url.clone(), sender));
                        }
                        None => {}
                    }
                }
            }
            Input::Abort => {
                if let Some(handle) = self.task_handle.take() {
                    handle.abort();
                    self.progress_status = format!("{} update aborted", self.asset_type.name());
                    self.state = State::Aborted;
                }
            }
        }
    }
}
