use std::{sync::Arc, path::PathBuf};
use tokio::{fs::File, io::AsyncReadExt};
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component, WidgetPlus, JoinHandle};
use crate::{bt, firmware_download as fw};

#[derive(Debug)]
pub enum Input {
    Connected(Arc<bt::InfiniTime>),
    Disconnected,

    FirmwareUpdateFromFile(PathBuf),
    FirmwareUpdateFromUrl(String),

    FirmwareContentReady(Vec<u8>),
    FirmwareUpdateFinished,
    FirmwareUpdateFailed(&'static str),

    StatusMessage(&'static str),
    FlashProgress { flashed: u32, total: u32 },

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

#[derive(Default)]
pub struct Model {
    status_message: String,
    progress: f64,
    state: State,
    dfu_content: Option<Arc<Vec<u8>>>,
    dfu_source: Option<Source>,

    infinitime: Option<Arc<bt::InfiniTime>>,
    task_handle: Option<JoinHandle<()>>,
}

impl Model {
    fn download_dfu(url: Arc<String>, sender: ComponentSender<Self>) -> JoinHandle<()> {
        relm4::spawn(async move {
            match fw::download_dfu_content(url.as_str()).await {
                Ok(content) => sender.input(Input::FirmwareContentReady(content)),
                Err(_) => sender.input(Input::FirmwareUpdateFailed("Failed to download DFU file")),
            }
        })
    }

    fn read_dfu_file(filepath: Arc<PathBuf>, sender: ComponentSender<Self>) -> JoinHandle<()> {
        relm4::spawn(async move {
            match File::open(filepath.as_path()).await {
                Ok(mut file) => {
                    let mut content = Vec::new();
                    match file.read_to_end(&mut content).await {
                        Ok(_) => sender.input(Input::FirmwareContentReady(content)),
                        Err(_) => sender.input(Input::FirmwareUpdateFailed("Failed to open DFU file")),
                    }
                }
                Err(_) => {
                    sender.input(Input::FirmwareUpdateFailed("Failed to read DFU file"));
                }
            }
        })
    }

    fn flash(infinitime: Arc<bt::InfiniTime>, dfu: Arc<Vec<u8>>, sender: ComponentSender<Self>) -> JoinHandle<()> {
        relm4::spawn(async move {
            let sender_ = sender.clone();
            let callback = move |notification| match notification {
                bt::FwUpdNotification::Message(text) => {
                    sender_.input(Input::StatusMessage(text));
                }
                bt::FwUpdNotification::BytesSent(flashed, total) => {
                    sender_.input(Input::FlashProgress { flashed, total });
                }
            };
            match infinitime.firmware_upgrade(dfu.as_slice(), callback).await {
                Ok(()) => sender.input(Input::FirmwareUpdateFinished),
                Err(_) => sender.input(Input::FirmwareUpdateFailed("Failed to flash firmware")),
            };
        })
    }
}

#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
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
                    set_label: "Firmware Update",
                },

                pack_start = &gtk::Button {
                    set_tooltip_text: Some("Back"),
                    set_icon_name: "go-previous-symbolic",
                    #[watch]
                    set_visible: model.state != State::InProgress,
                    connect_clicked[sender] => move |_| {
                        sender.output(Output::SetView(super::View::Dashboard));
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
                        set_label: &model.status_message,
                        set_halign: gtk::Align::Center,
                        set_margin_top: 20,
                    },

                    gtk::LevelBar {
                        set_min_value: 0.0,
                        #[watch]
                        set_max_value: 1.0,
                        #[watch]
                        set_value: model.progress,
                        #[watch]
                        set_visible: model.state == State::InProgress && model.progress > 0.0,
                    },

                    gtk::Spinner {
                        #[watch]
                        set_visible: model.state == State::InProgress && model.progress == 0.0,
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
                                sender.output(Output::SetView(super::View::Dashboard));
                            },
                        },
                    }
                },
            },
        }
    }

    fn init(_: Self::InitParams, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self::default();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Input::Connected(infinitime) => {
                self.infinitime = Some(infinitime);
            }
            Input::Disconnected => {
                self.infinitime = None;
            }
            Input::FirmwareUpdateFromFile(filepath) => {
                let filepath = Arc::new(filepath);
                self.status_message = format!("Reading firmware file");
                self.state = State::InProgress;
                self.dfu_source = Some(Source::File(filepath.clone()));
                self.task_handle = Some(Self::read_dfu_file(filepath.clone(), sender));
            }
            Input::FirmwareUpdateFromUrl(url) => {
                let url = Arc::new(url);
                self.status_message = format!("Downloading firmware");
                self.state = State::InProgress;
                self.dfu_source = Some(Source::Url(url.clone()));
                self.task_handle = Some(Self::download_dfu(url.clone(), sender));
            }
            Input::FirmwareContentReady(content) => {
                if let Some(infinitime) = &self.infinitime {
                    let content = Arc::new(content);
                    self.dfu_source = None;
                    self.dfu_content = Some(content.clone());
                    self.task_handle = Some(Self::flash(infinitime.clone(), content, sender));
                }
            }
            Input::FirmwareUpdateFinished => {
                self.status_message = format!("Firmware update complete :)");
                self.state = State::Finished;
                self.task_handle = None;
                self.dfu_content = None;
            }
            Input::FirmwareUpdateFailed(message) => {
                self.status_message = format!("Firmware update error: {message}");
                self.state = State::Aborted;
                self.task_handle = None;
            }
            Input::StatusMessage(text) => {
                self.status_message = text.to_string();
            }
            Input::FlashProgress { flashed, total } => {
                let flashed = flashed as f64 / 1024.0;
                let total = total as f64 / 1024.0;
                self.status_message = format!("Flashing firmware: {flashed:.01}/{total:.01} kB");
                self.progress = (flashed / total) as f64;
            }
            Input::Retry => {
                if let Some(content) = &self.dfu_content {
                    if let Some(infinitime) = &self.infinitime {
                        self.task_handle = Some(Self::flash(infinitime.clone(), content.clone(), sender));
                    }
                } else {
                    match &self.dfu_source {
                        Some(Source::File(filepath)) => {
                            self.task_handle = Some(Self::read_dfu_file(filepath.clone(), sender));
                        }
                        Some(Source::Url(url)) => {
                            self.task_handle = Some(Self::download_dfu(url.clone(), sender));
                        }
                        None => {}
                    }
                }
            }
            Input::Abort => {
                if let Some(handle) = self.task_handle.take() {
                    handle.abort();
                    self.status_message = format!("Firmware update aborted");
                    self.state = State::Aborted;
                }
            }
        }
    }
}
