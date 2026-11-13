use crate::ui;
use gtk::{
    gio, prelude::{
        GtkApplicationExt, WidgetExt, SettingsExt, EditableExt
    }
};
use adw::prelude::{AdwDialogExt, PreferencesPageExt, PreferencesGroupExt, PreferencesRowExt, ActionRowExt, PreferencesDialogExt, ComboRowExt};
use relm4::{adw, gtk, ComponentParts, ComponentSender, Component};
use ashpd::{desktop::background::Background, WindowIdentifier, Error};


#[derive(Debug)]
pub enum Input {
    RunInBackgroundRequest(bool),
    RunInBackgroundResponse(bool),
    AutoStartRequest(bool),
    AutoStartResponse(bool),
    CitySearch(String),
    CitySearchResult(String, Vec<crate::weather::GeocodingResult>),
    CitySelected(u32),
}


pub struct Model {
    background_row: adw::SwitchRow,
    autostart_row: adw::SwitchRow,
    city_row: adw::ComboRow,
    city_results: Vec<crate::weather::GeocodingResult>,
    city_query: String,
    city_search_task: Option<gtk::glib::JoinHandle<()>>,
    settings: gio::Settings,
}

impl Model {
    fn background_portal_request<F>(&self, autostart: bool, handler: F)
        where F: Fn(Result<Background, Error>) + 'static
    {
        let native_window = relm4::main_application()
            .active_window()
            .and_then(|w| w.native())
            .unwrap();
        relm4::spawn_local(async move {
            let identifier = WindowIdentifier::from_native(&native_window).await;
            let request = Background::request()
                .identifier(identifier)
                .auto_start(autostart)
                .command(["watchmate", "--background"])
                .reason("Keep the watch connected, forward notifications, control media player");
            let response = request.send().await.and_then(|r| r.response());
            handler(response);
        });
    }
}


#[relm4::component(pub)]
impl Component for Model {
    type CommandOutput = ();
    type Init = gio::Settings;
    type Input = Input;
    type Output = ();
    type Widgets = Widgets;

    view! {
        #[root]
        adw::PreferencesDialog {
            set_title: "Settings",

            add = &adw::PreferencesPage {
                add = &adw::PreferencesGroup {
                    #[local_ref]
                    background_row -> adw::SwitchRow {
                        set_title: "Run in Background",
                        set_subtitle: "When closed",
                        set_active: model.settings.boolean(super::SETTING_BACKGROUND),
                        connect_active_notify[sender] => move |row| {
                            sender.input(Input::RunInBackgroundRequest(row.is_active()));
                        }
                    },
                    #[local_ref]
                    autostart_row -> adw::SwitchRow {
                        set_title: "Auto-Start",
                        set_subtitle: "In background at login",
                        set_active: model.settings.boolean(super::SETTING_AUTO_START),
                        connect_active_notify[sender] => move |row| {
                            sender.input(Input::AutoStartRequest(row.is_active()));
                        }
                    },
                },
                add = &adw::PreferencesGroup {
                    set_title: "Weather (Open-Meteo)",
                    add = &adw::SwitchRow {
                        set_title: "Send Weather to PineTime",
                        set_subtitle: "Takes effect after reconnecting the watch",
                        set_active: model.settings.boolean("weather-enabled"),
                        connect_active_notify[settings = model.settings.clone()] => move |row| {
                            _ = settings.set_boolean("weather-enabled", row.is_active());
                        }
                    },
                    add = &adw::EntryRow {
                        set_title: "Search City",
                        connect_changed[sender] => move |row| {
                            sender.input(Input::CitySearch(row.text().to_string()));
                        }
                    },
                    #[local_ref]
                    city_row -> adw::ComboRow {
                        set_title: "Search Results",
                        set_visible: false,
                        connect_selected_notify[sender] => move |row| {
                            sender.input(Input::CitySelected(row.selected()));
                        }
                    },
                    add = &adw::EntryRow {
                        set_title: "Latitude (e.g. 52.15)",
                        set_text: &model.settings.string("weather-latitude"),
                        connect_changed[settings = model.settings.clone()] => move |row| {
                            _ = settings.set_string("weather-latitude", &row.text());
                        }
                    },
                    add = &adw::EntryRow {
                        set_title: "Longitude (e.g. 10.33)",
                        set_text: &model.settings.string("weather-longitude"),
                        connect_changed[settings = model.settings.clone()] => move |row| {
                            _ = settings.set_string("weather-longitude", &row.text());
                        }
                    },
                    add = &adw::EntryRow {
                        set_title: "Location Name",
                        set_text: &model.settings.string("weather-location"),
                        connect_changed[settings = model.settings.clone()] => move |row| {
                            _ = settings.set_string("weather-location", &row.text());
                        }
                    },
                }
            }
        }
    }

    fn init(settings: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = Self {
            background_row: adw::SwitchRow::new(),
            autostart_row: adw::SwitchRow::new(),
            city_row: adw::ComboRow::new(),
            city_results: Vec::new(),
            city_query: String::new(),
            city_search_task: None,
            settings,
        };

        let background_row = model.background_row.clone();
        let autostart_row = model.autostart_row.clone();
        let city_row = model.city_row.clone();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            Input::RunInBackgroundRequest(enabled) => {
                if enabled == self.settings.boolean(super::SETTING_BACKGROUND) {
                    // Already in this state, nothing to do
                } else if enabled {
                    let autostart = self.settings.boolean(super::SETTING_AUTO_START);
                    self.background_portal_request(autostart, move |r| match r {
                        Ok(response) => {
                            sender.input(Input::RunInBackgroundResponse(response.run_in_background()));
                            if !response.run_in_background() {
                                ui::BROKER.send(ui::Input::ToastStatic("Not allowed to run in background"));
                            }
                        }
                        Err(error) => {
                            // Enable the background mode anyway, it might not require the
                            // permission if the portal is not implemented
                            sender.input(Input::RunInBackgroundResponse(true));
                            log::error!("Background portal request failed: {error}");
                            ui::BROKER.send(ui::Input::ToastStatic("Background permission request failed"));
                        }
                    });
                } else {
                    sender.input(Input::RunInBackgroundResponse(false));
                }
            }
            Input::AutoStartRequest(enabled) => {
                if enabled == self.settings.boolean(super::SETTING_AUTO_START) {
                    // Already in this state, nothing to do
                } else {
                    let old_state = self.settings.boolean(super::SETTING_AUTO_START);
                    self.background_portal_request(enabled, move |r| match r {
                        Ok(response) => {
                            sender.input(Input::AutoStartResponse(response.auto_start()));
                            if response.auto_start() != enabled {
                                ui::BROKER.send(ui::Input::ToastStatic("Not allowed to change autostart setting"));
                            }
                        }
                        Err(error) => {
                            sender.input(Input::AutoStartResponse(old_state));
                            log::error!("Background portal request failed: {error}");
                            ui::BROKER.send(ui::Input::ToastStatic("Autostart request failed"));
                        }
                    });
                }
            }
            Input::RunInBackgroundResponse(enabled) => {
                self.background_row.set_active(enabled);
                _ = self.settings.set_boolean(super::SETTING_BACKGROUND, enabled);
            }
            Input::AutoStartResponse(enabled) => {
                self.autostart_row.set_active(enabled);
                _ = self.settings.set_boolean(super::SETTING_AUTO_START, enabled);
            }
            Input::CitySearch(query) => {
                let query = query.trim().to_string();
                if let Some(task) = self.city_search_task.take() {
                    task.abort();
                }
                if query.chars().count() < 2 {
                    self.city_query.clear();
                    self.city_row.set_visible(false);
                    return;
                }
                self.city_query = query.clone();
                self.city_search_task = Some(relm4::spawn_local(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    match crate::weather::search_city(&query).await {
                        Ok(results) => sender.input(Input::CitySearchResult(query, results)),
                        Err(err) => {
                            log::error!("City search failed: {err}");
                            ui::BROKER.send(ui::Input::ToastStatic("City search failed"));
                        }
                    }
                }));
            }
            Input::CitySearchResult(query, results) => {
                if query != self.city_query {
                    return;
                }
                if results.is_empty() {
                    ui::BROKER.send(ui::Input::ToastStatic("No city found"));
                    self.city_row.set_visible(false);
                    return;
                }
                let labels: Vec<String> = results.iter().map(|r| r.label()).collect();
                let model = gtk::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>());
                self.city_row.set_model(Some(&model));
                self.city_results = results;
                self.city_row.set_visible(true);
            }
            Input::CitySelected(index) => {
                if let Some(city) = self.city_results.get(index as usize) {
                    _ = self.settings.set_string("weather-latitude", &city.latitude.to_string());
                    _ = self.settings.set_string("weather-longitude", &city.longitude.to_string());
                    _ = self.settings.set_string("weather-location", &city.name);
                }
            }
        };
    }
}
