use std::sync::Arc;
use gtk::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use relm4::{gtk, ComponentParts, ComponentSender, Component, WidgetPlus, JoinHandle};
use mpris2_zbus::media_player::MediaPlayer;

use crate::{bt, media_player as mp};

#[derive(Debug)]
pub enum Input {
    DeviceConnection(Option<Arc<bt::InfiniTime>>),
    PlayersListRequest,
    ControlSessionStart,
    ControlSessionEnded,
}

#[derive(Debug)]
pub enum Output {
}

#[derive(Debug)]
pub enum CommandOutput {
    PlayersListResponse(Option<Vec<MediaPlayer>>),
}

#[derive(Default)]
pub struct Model {
    player_handles: Option<Vec<Arc<MediaPlayer>>>,
    player_names: Option<gtk::StringList>,
    control_task: Option<JoinHandle<()>>,
    infinitime: Option<Arc<bt::InfiniTime>>,
    dropdown: gtk::DropDown,
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
            set_orientation: gtk::Orientation::Horizontal,
            set_margin_all: 12,
            set_spacing: 10,

            if model.player_handles.is_some() {
                #[local]
                dropdown -> gtk::DropDown {
                    set_hexpand: true,
                    #[watch]
                    set_model: model.player_names.as_ref(),
                    connect_selected_notify[sender] => move |_| {
                        sender.input(Input::ControlSessionStart);
                    }
                }
            } else {
                gtk::Label {
                    set_hexpand: true,
                    set_label: "No media players detected",
                }
            },

            gtk::Button {
                set_tooltip_text: Some("Refresh releases list"),
                set_icon_name: "view-refresh-symbolic",
                connect_clicked[sender] => move |_| {
                    sender.input(Input::PlayersListRequest);
                },
            }
        }
    }

    fn init(_: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let dropdown = gtk::DropDown::default();
        let model = Self { dropdown: dropdown.clone(), ..Default::default() };
        let widgets = view_output!();
        sender.input(Input::PlayersListRequest);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            Input::DeviceConnection(infinitime) => {
                self.infinitime = infinitime;
                if self.infinitime.is_some() {
                    sender.input(Input::ControlSessionStart);
                }
            }
            Input::PlayersListRequest => {
                // Stop current media player control sesssion
                self.control_task.take().map(|h| h.abort());

                sender.oneshot_command(async move {
                    // TODO: Save and reuse D-Bus connection
                    match zbus::Connection::session().await {
                        Ok(connection) => match mp::get_players(&connection).await {
                            Ok(players) => if players.len() > 0 {
                                return CommandOutput::PlayersListResponse(Some(players));
                            }
                            Err(error) => {
                                log::error!("Failed to obtain MPRIS players list: {error}");
                            }
                        }
                        Err(error) => {
                            log::error!("Failed to establish D-Bus session connection: {error}")
                        }
                    }
                    CommandOutput::PlayersListResponse(None)
                })
            }
            Input::ControlSessionStart => {
                if let (Some(players), Some(infinitime)) = (&self.player_handles, &self.infinitime) {
                    let index = self.dropdown.selected();
                    if index != gtk::INVALID_LIST_POSITION {
                        // Stop current media player control sesssion
                        self.control_task.take().map(|h| h.abort());
                        // Start new media player control sesssion
                        let player = players[index as usize].clone();
                        let infinitime = infinitime.clone();
                        let task_handle = relm4::spawn(async move {
                            match mp::run_session(&player, &infinitime).await {
                                Ok(()) => log::warn!("Media player control session ended unexpectedly"),
                                Err(error) => log::error!("Media player control session error: {error}"),
                            }
                            sender.input(Input::ControlSessionEnded)
                        });
                        self.control_task = Some(task_handle);
                    }
                }
            }
            Input::ControlSessionEnded => {
                self.player_handles = None;
                self.player_names = None;
                self.control_task = None;
                sender.input(Input::PlayersListRequest);
            }
        }
    }

    fn update_cmd(&mut self, msg: Self::CommandOutput, _sender: ComponentSender<Self>) {
        match msg {
            CommandOutput::PlayersListResponse(players) => {
                if let Some(players) = players {
                    let names = gtk::StringList::new(&[]);
                    for player in &players {
                        if let Ok(Some(name)) = player.cached_identity() {
                            names.append(&name);
                        } else {
                            log::error!("Failed to obtain cached player identity");
                            return;
                        }
                    }
                    self.player_names = Some(names);
                    self.player_handles = Some(players.into_iter().map(Arc::new).collect());
                } else {
                    self.player_names = None;
                    self.player_handles = None;
                }
            }
        }
    }
}

