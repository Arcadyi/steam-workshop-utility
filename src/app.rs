use crate::config::Config;
use crate::steam::api::{fetch_workshop_metadata_batch, open_uri, try_fetch_image};
use crate::steam::library::{
    enrich_workshop_items_for_game, find_acf_path, get_games, get_workshop_entries,
    zero_acf_entries,
};
use crate::types::types::{GameEntry, ItemStatus, WorkshopItem};
use crate::utils::general::{format_size, format_timestamp};
use crate::utils::ui::colored_icon;
use anyhow::Context;
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::{Background, Border, Color, Length, Subscription};
use cosmic::prelude::*;
use cosmic::widget::{self, about::About, container, icon, menu};
use icon::from_svg_bytes;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use crate::icons::{APP_ICON, ICON_CHECK, ICON_CROSS, ICON_FOLDER, ICON_GAME, ICON_QUESTION, ICON_STEAM};
use crate::steam::collection::collection::CollectionCode;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// Tracks the async scan lifecycle
#[derive(Debug, Default)]
pub enum AppState {
    #[default]
    Loading,
    Loaded {
        // Map from appid to that game's workshop items
        items: HashMap<String, Vec<WorkshopItem>>,
        games: HashMap<String, GameEntry>,
        // Ordered list of appids for stable display order
        game_order: Vec<String>,
    },
    Error(String),
}

pub struct AppModel {
    core: cosmic::Core,
    context_page: ContextPage,
    about: About,
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    config: Config,
    state: AppState,
    selected_game: Option<String>,
    polling: Option<PollingState>,
    confirming_redownload: bool,
    redownload_in_progress: bool,
    redownload_complete: bool,
    search_query: String,
    sort_order: SortOrder,
    thumbnail_cache: HashMap<String, widget::image::Handle>,
    game_banner_cache: HashMap<String, widget::image::Handle>,
    compact_mode: bool,

    generated_code: Option<String>,
    import_code_input: String,
    pending_import: Option<CollectionCode>,
    import_error: bool,
    import_item_names: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SortOrder {
    #[default]
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
    StatusFirst,
}

pub struct PollingState {
    appid: String,
    elapsed_secs: u32,
    pending_item_ids: HashSet<String>,
    initial_item_count: usize,
}

const POLL_INTERVAL_SECS: u32 = 1;
const POLL_TIMEOUT_SECS: u32 = 300;

#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    ScanComplete(Vec<(GameEntry, Vec<WorkshopItem>)>),
    ScanFailed(String),
    SelectGame(String),
    ToggleItem {
        appid: String,
        item_id: String,
    },
    ToggleAllItems(String),
    ForceRedownload,
    RedownloadComplete(Result<(), String>),
    OpenFolder(PathBuf),
    OpenSteam {
        appid: String,
        item_id: String,
    },
    RefreshGame(String),
    RefreshComplete {
        appid: String,
        data: Vec<(GameEntry, Vec<WorkshopItem>)>,
    },
    Tick,
    StopPolling,
    ConfirmRedownload,
    CancelRedownload,
    DismissComplete,
    SearchChanged(String),
    SortChanged(SortOrder),
    ThumbnailsLoaded(HashMap<String, widget::image::Handle>),
    GameBannerLoaded {
        appid: String,
        handle: widget::image::Handle,
    },
    ToggleCompactMode,
    SelectOutOfDate,

    // Collection
    GenerateCollectionCode,
    CollectionCodeGenerated(String),
    CopyCodeToClipboard(String),

    ToggleImportPanel,
    ImportCodeChanged(String),
    ImportCode,
    ImportResolved(CollectionCode),
    ClearImport,
    ImportMetadataLoaded {
        collection: CollectionCode,
        names: HashMap<String, String>, // item_id -> name
    },

    Noop,
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.arcadyi.steam_workshop_utility";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let about = About::default()
            .name("Steam Workshop Utility")
            .icon(from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            .author("Created by Arcady - Denizhan De Asis")
            .comments(
                "A Steam Workshop utility for managing and force-redownloading workshop items.",
            )
            .links([("Repository", REPOSITORY)])
            .license(env!("CARGO_PKG_LICENSE"));

        let app = AppModel {
            core,
            context_page: ContextPage::default(),
            about,
            key_binds: HashMap::new(),
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| {
                    Config::get_entry(&context).unwrap_or_else(|(_errors, config)| config)
                })
                .unwrap_or_default(),
            state: AppState::Loading,
            selected_game: None,
            polling: None,
            confirming_redownload: false,
            redownload_in_progress: false,
            redownload_complete: false,
            search_query: String::new(),
            sort_order: SortOrder::default(),
            thumbnail_cache: HashMap::new(),
            game_banner_cache: HashMap::new(),
            compact_mode: false,
            generated_code: None,
            import_code_input: "".to_string(),
            pending_import: None,
            import_error: false,
            import_item_names: HashMap::new(),
        };

        let scan_task = Task::perform(
            async {
                let client = reqwest::Client::new();
                let games = get_games().map_err(|e| e.to_string())?;

                let mut results = Vec::new();
                for game in games {
                    let items = get_workshop_entries(&game).map_err(|e| e.to_string())?;
                    let items = enrich_workshop_items_for_game(&client, items)
                        .await
                        .map_err(|e| e.to_string())?;
                    results.push((game, items));
                }

                Ok::<_, String>(results)
            },
            |result| match result {
                Ok(data) => cosmic::Action::App(Message::ScanComplete(data)),
                Err(e) => cosmic::Action::App(Message::ScanFailed(e)),
            },
        );

        (app, scan_task)
    }

    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::about(
                &self.about,
                |url| Message::LaunchUrl(url.to_string()),
                Message::ToggleContextPage(ContextPage::About),
            ),
            ContextPage::CollectionCode => context_drawer::context_drawer(
                self.view_collection_code_drawer(),
                Message::ToggleContextPage(ContextPage::CollectionCode),
            )
                .title("Share Collection"),

            ContextPage::ImportCode => context_drawer::context_drawer(
                self.view_import_drawer(),
                Message::ToggleContextPage(ContextPage::ImportCode),
            )
                .title("Import Collection"),
        })
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root("View").apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![menu::Item::Button("About", None, MenuAction::About)],
            ),
        )]);

        vec![menu_bar.into()]
    }

    // No nav_model — we handle game selection ourselves
    fn nav_model(&self) -> Option<&cosmic::widget::nav_bar::Model> {
        None
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs = vec![
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
        ];

        if self.polling.is_some() {
            subs.push(
                cosmic::iced::time::every(std::time::Duration::from_secs(
                    POLL_INTERVAL_SECS as u64,
                ))
                    .map(|_| Message::Tick),
            );
        }

        Subscription::batch(subs)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Noop => {}

            Message::ScanComplete(results) => {
                let mut items_map = HashMap::new();
                let mut games_map = HashMap::new();
                let mut game_order = Vec::new();

                for (game, mut items) in results {
                    if items.is_empty() {
                        continue;
                    }
                    for item in items.iter_mut() {
                        if matches!(item.status, ItemStatus::OutOfDate) {
                            item.selected = true;
                        }
                    }
                    let appid = game.appid.clone();
                    game_order.push(appid.clone());
                    games_map.insert(appid.clone(), game);
                    items_map.insert(appid, items);
                }

                // Select first game automatically
                let first_appid = game_order.first().cloned();
                self.selected_game = first_appid.clone();

                self.state = AppState::Loaded {
                    games: games_map,
                    items: items_map,
                    game_order,
                };

                let _ = self.update_title();
                let mut tasks = vec![self.load_thumbnails_for_active_game()];

                // Kick off banner loads for ALL games so the card list looks good
                if let AppState::Loaded { game_order, .. } = &self.state {
                    for appid in game_order.clone() {
                        tasks.push(self.load_banner_for_appid(appid));
                    }
                }

                return Task::batch(tasks);
            }

            Message::ScanFailed(err) => {
                self.state = AppState::Error(err);
            }

            Message::SelectGame(appid) => {
                self.selected_game = Some(appid.clone());
                self.confirming_redownload = false;
                self.redownload_complete = false;
                let _ = self.update_title();
                return Task::batch([
                    self.load_thumbnails_for_active_game(),
                    self.load_banner_for_appid(appid),
                ]);
            }

            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }

            Message::UpdateConfig(config) => {
                self.config = config;
            }

            Message::LaunchUrl(url) => {
                if let Err(err) = open::that_detached(&url) {
                    eprintln!("failed to open {url:?}: {err}");
                }
            }

            Message::ToggleItem { appid, item_id } => {
                if let AppState::Loaded { items, .. } = &mut self.state {
                    if let Some(game_items) = items.get_mut(&appid) {
                        if let Some(item) = game_items.iter_mut().find(|i| i.item_id == item_id) {
                            item.selected = !item.selected;
                        }
                    }
                }
            }

            Message::ToggleAllItems(appid) => {
                if let AppState::Loaded { items, .. } = &mut self.state {
                    if let Some(game_items) = items.get_mut(&appid) {
                        let all_selected = game_items.iter().all(|i| i.selected);
                        for item in game_items.iter_mut() {
                            item.selected = !all_selected;
                        }
                    }
                }
            }

            Message::ForceRedownload => {
                self.confirming_redownload = false;
                self.redownload_in_progress = true;
                self.redownload_complete = false;

                let AppState::Loaded { games, items, .. } = &self.state else {
                    return Task::none();
                };

                let Some(ref active_appid) = self.selected_game else {
                    return Task::none();
                };

                let Some(game_entry) = games.get(active_appid) else {
                    return Task::none();
                };
                let Some(game_items) = items.get(active_appid) else {
                    return Task::none();
                };

                let selected: Vec<WorkshopItem> =
                    game_items.iter().filter(|i| i.selected).cloned().collect();
                if selected.is_empty() {
                    return Task::none();
                }

                let appid = game_entry.appid.clone();
                let item_ids: Vec<String> = selected.iter().map(|i| i.item_id.clone()).collect();
                let paths: Vec<PathBuf> = selected.iter().map(|i| i.path.clone()).collect();

                let acf_path = match find_acf_path(game_entry) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        eprintln!("[ForceRedownload] No ACF: {}", e);
                        None
                    }
                };

                return Task::perform(
                    async move {
                        for path in &paths {
                            std::fs::remove_dir_all(path)
                                .with_context(|| format!("Could not delete {}", path.display()))
                                .map_err(|e| e.to_string())?;
                        }

                        if let Some(ref acf) = acf_path {
                            zero_acf_entries(acf, &item_ids).map_err(|e| e.to_string())?;
                        }

                        for id in &item_ids {
                            let uri = format!("steam://workshop_download_item/{}/{}", appid, id);
                            open_uri(&uri).await?;

                            // Windows may need a small delay
                            #[cfg(target_os = "windows")]
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }

                        let uri = format!("steam://validate/{}", appid);
                        open_uri(&uri).await?;

                        Ok::<(), String>(())
                    },
                    |result| cosmic::Action::App(Message::RedownloadComplete(result)),
                );
            }

            Message::RedownloadComplete(result) => {
                self.redownload_in_progress = false;
                if let Err(e) = result {
                    eprintln!("Force redownload failed: {e}");
                    return Task::none();
                }

                let Some(ref active_appid) = self.selected_game.clone() else {
                    return Task::none();
                };
                let appid = active_appid.clone();

                let pending: HashSet<String> = if let AppState::Loaded { items, .. } = &self.state {
                    items
                        .get(&appid)
                        .map(|game_items| {
                            game_items
                                .iter()
                                .filter(|i| i.selected)
                                .map(|i| i.item_id.clone())
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    HashSet::new()
                };

                if let AppState::Loaded { items, .. } = &mut self.state {
                    for game_items in items.values_mut() {
                        for item in game_items.iter_mut() {
                            item.selected = false;
                        }
                    }
                }

                self.polling = Some(PollingState {
                    appid: appid.clone(),
                    elapsed_secs: 0,
                    initial_item_count: pending.len(),
                    pending_item_ids: pending,
                });

                return self.update(Message::RefreshGame(appid));
            }

            Message::OpenFolder(path) => {
                if let Err(e) = open::that_detached(&path) {
                    eprintln!("Failed to open folder {}: {e}", path.display());
                }
            }

            Message::OpenSteam { appid, item_id } => {
                let url = format!("steam://url/CommunityFilePage/{}", item_id);
                return Task::perform(
                    async move { open_uri(&url).await },
                    |_| cosmic::Action::App(Message::Noop),
                );
            }

            Message::RefreshGame(appid) => {
                let appid_clone = appid.clone();
                return Task::perform(
                    async move {
                        let client = reqwest::Client::new();
                        let games = get_games().map_err(|e| e.to_string())?;

                        let game = games
                            .into_iter()
                            .find(|g| g.appid == appid_clone)
                            .ok_or_else(|| format!("Game '{}' not found", appid_clone))?;

                        let items = get_workshop_entries(&game).map_err(|e| e.to_string())?;
                        let items = enrich_workshop_items_for_game(&client, items)
                            .await
                            .map_err(|e| e.to_string())?;

                        Ok::<_, String>(vec![(game, items)])
                    },
                    move |result| match result {
                        Ok(data) => cosmic::Action::App(Message::RefreshComplete { appid, data }),
                        Err(e) => cosmic::Action::App(Message::ScanFailed(e)),
                    },
                );
            }

            Message::RefreshComplete { appid, data } => {
                if let AppState::Loaded { games, items, .. } = &mut self.state {
                    for (game, mut workshop_items) in data {
                        for item in workshop_items.iter_mut() {
                            if matches!(item.status, ItemStatus::OutOfDate) {
                                item.selected = true;
                            }
                        }

                        if let Some(ref mut poll) = self.polling {
                            if poll.appid == appid {
                                poll.pending_item_ids.retain(|id| {
                                    match workshop_items.iter().find(|i| &i.item_id == id) {
                                        Some(item) => !matches!(item.status, ItemStatus::UpToDate),
                                        None => true,
                                    }
                                });

                                if poll.pending_item_ids.is_empty() {
                                    self.polling = None;
                                    self.redownload_complete = true;
                                }
                            }
                        }

                        games.insert(appid.clone(), game);
                        items.insert(appid.clone(), workshop_items);
                        return self.load_thumbnails_for_active_game();
                    }
                }
                return self.load_thumbnails_for_active_game();
            }

            Message::Tick => {
                let Some(ref mut poll) = self.polling else {
                    return Task::none();
                };

                poll.elapsed_secs += POLL_INTERVAL_SECS;

                if poll.elapsed_secs >= POLL_TIMEOUT_SECS {
                    self.polling = None;
                    return Task::none();
                }

                let appid = poll.appid.clone();
                return self.update(Message::RefreshGame(appid));
            }

            Message::StopPolling => {
                self.polling = None;
            }

            Message::ConfirmRedownload => {
                self.confirming_redownload = true;
            }

            Message::CancelRedownload => {
                self.confirming_redownload = false;
            }

            Message::DismissComplete => {
                self.redownload_complete = false;
            }

            Message::SearchChanged(query) => {
                self.search_query = query;
            }

            Message::SortChanged(order) => {
                self.sort_order = order;
            }

            Message::ThumbnailsLoaded(new_handles) => {
                self.thumbnail_cache.extend(new_handles);
            }

            Message::GameBannerLoaded { appid, handle } => {
                if !appid.is_empty() {
                    self.game_banner_cache.insert(appid, handle);
                }
            }

            Message::ToggleCompactMode => {
                self.compact_mode = !self.compact_mode;
            }

            Message::SelectOutOfDate => {
                if let AppState::Loaded { items, .. } = &mut self.state {
                    if let Some(ref appid) = self.selected_game.clone() {
                        if let Some(game_items) = items.get_mut(appid) {
                            for item in game_items.iter_mut() {
                                if matches!(item.status, ItemStatus::OutOfDate) {
                                    item.selected = true;
                                }
                            }
                        }
                    }
                }
            }

            Message::GenerateCollectionCode => {
                if let (Some(appid), AppState::Loaded { items, games, .. }) =
                    (&self.selected_game, &self.state)
                {
                    let selected: Vec<String> = items[appid]
                        .iter()
                        .filter(|i| i.selected)
                        .map(|i| i.item_id.clone())
                        .collect();

                    let name = games[appid].name.clone().unwrap_or_default();
                    let code = CollectionCode { appid: appid.clone(), name, items: selected }.encode();
                    self.generated_code = Some(code);

                    self.context_page = ContextPage::CollectionCode;
                    self.core.window.show_context = true;
                }
            }

            Message::CollectionCodeGenerated(code) => {
                self.generated_code = Some(code);
                self.context_page = ContextPage::CollectionCode;
                return self.update(Message::ToggleContextPage(ContextPage::CollectionCode));
            }

            Message::CopyCodeToClipboard(code) => {
                return cosmic::iced::clipboard::write(code);
            }

            Message::ToggleImportPanel => {
                self.context_page = ContextPage::ImportCode;
                return self.update(Message::ToggleContextPage(ContextPage::ImportCode));
            }

            Message::ImportCodeChanged(code) => {
                self.import_code_input = code;
            }

            Message::ImportCode => {
                if let Some(collection) = CollectionCode::decode(&self.import_code_input) {
                    self.import_error = false;

                    // Collect all IDs we need names for: collection items + installed items for this game
                    let collection_ids = collection.items.clone();
                    let installed_ids: Vec<String> = if let AppState::Loaded { items, .. } = &self.state {
                        items
                            .get(&collection.appid)
                            .map(|gi| gi.iter().map(|i| i.item_id.clone()).collect())
                            .unwrap_or_default()
                    } else {
                        vec![]
                    };

                    let all_ids: Vec<String> = collection_ids.iter()
                        .chain(installed_ids.iter())
                        .cloned()
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();

                    return Task::perform(
                        async move {
                            let client = reqwest::Client::new();
                            let metadata = fetch_workshop_metadata_batch(&client, &all_ids)
                                .await
                                .unwrap_or_default();
                            let names = metadata.into_iter()
                                .filter_map(|(id, meta)| meta.title.map(|t| (id, t)))
                                .collect();
                            (collection, names)
                        },
                        |(collection, names)| cosmic::Action::App(Message::ImportMetadataLoaded { collection, names }),
                    );
                } else {
                    self.import_error = true;
                    self.pending_import = None;
                }
            }

            Message::ImportMetadataLoaded { collection, names } => {
                self.import_item_names = names;
                self.pending_import = Some(collection);
            }

            Message::ImportResolved(collection) => {
                self.pending_import = Some(collection);
            }
            Message::ClearImport => {
                self.pending_import = None;
                self.import_error = false;
                self.import_code_input = String::new();
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        match &self.state {
            AppState::Loading => widget::container(
                widget::column::with_capacity(3)
                    .push(widget::icon(from_svg_bytes(APP_ICON)).size(64))
                    .push(widget::text::title3("Scanning Steam library..."))
                    .push(widget::text::body("Fetching workshop metadata"))
                    .spacing(cosmic::theme::spacing().space_s)
                    .align_x(cosmic::iced::Alignment::Center),
            )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .align_y(cosmic::iced::alignment::Vertical::Center)
                .into(),

            AppState::Error(msg) => {
                widget::container(widget::text::title3(format!("Error: {msg}")))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(cosmic::iced::alignment::Horizontal::Center)
                    .align_y(cosmic::iced::alignment::Vertical::Center)
                    .into()
            }

            AppState::Loaded { items, games, game_order } => {
                let active_appid = self.selected_game.as_deref().unwrap_or("");
                let active_game = games.get(active_appid);
                let workshop_items = items.get(active_appid);

                let selected_count = workshop_items
                    .map(|it| it.iter().filter(|i| i.selected).count())
                    .unwrap_or(0);

                let has_outdated = workshop_items
                    .map(|it| it.iter().any(|i| matches!(i.status, ItemStatus::OutOfDate)))
                    .unwrap_or(false);

                // Left panel: scrollable game card list
                let game_list = self.view_game_list(game_order, games, items, active_appid);

                // Right panel: workshop items for selected game
                let right_content: Element<'_, Message> = match workshop_items {
                    None => widget::container(
                        widget::text::body("Select a game from the left panel")
                    )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .align_y(cosmic::iced::alignment::Vertical::Center)
                        .into(),

                    Some(ws_items) => widget::column::with_capacity(3)
                        .push(self.view_status_bar())
                        .push(
                            self.view_workshop_items(
                                active_appid,
                                ws_items,
                                active_game,
                                &self.search_query,
                                &self.sort_order,
                            )
                        )
                        .push(self.view_bottom_bar(active_appid, selected_count, has_outdated))
                        .into(),
                };

                // Two-panel layout
                widget::row::with_capacity(2)
                    .push(game_list)
                    .push(right_content)
                    .into()
            }
        }
    }
}

impl AppModel {
    /// Left panel: scrollable list of game cards
    fn view_game_list<'a>(
        &'a self,
        game_order: &'a [String],
        games: &'a HashMap<String, GameEntry>,
        items: &'a HashMap<String, Vec<WorkshopItem>>,
        active_appid: &'a str,
    ) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();

        let cards = game_order.iter().fold(
            widget::column::with_capacity(game_order.len()).spacing(spacing.space_xs),
            |col, appid| {
                let game = match games.get(appid) {
                    Some(g) => g,
                    None => return col,
                };
                let game_items = items.get(appid).map(|v| v.as_slice()).unwrap_or(&[]);
                let is_selected = appid == active_appid;
                let card = self.view_game_card(appid, game, game_items, is_selected);
                col.push(card)
            },
        );

        widget::container(
            widget::scrollable(
                widget::column::with_capacity(2)
                    .push(
                        widget::text::title4("Games")
                            .width(Length::Fill)
                    )
                    .push(cards)
                    .spacing(spacing.space_s)
                    .padding([spacing.space_s, spacing.space_s, spacing.space_m, spacing.space_s]),
            )
                .height(Length::Fill)
                .width(Length::Fill),
        )
            .width(280)
            .height(Length::Fill)
            .style(|theme| container::Style {
                background: Some(Background::Color(
                    theme.cosmic().palette.neutral_1.into(),
                )),
                ..Default::default()
            })
            .into()
    }

    /// A single game card in the left panel
    fn view_game_card<'a>(
        &'a self,
        appid: &'a str,
        game: &'a GameEntry,
        game_items: &'a [WorkshopItem],
        is_selected: bool,
    ) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();

        let total = game_items.len();
        let out_of_date = game_items
            .iter()
            .filter(|i| matches!(i.status, ItemStatus::OutOfDate))
            .count();
        let up_to_date = game_items
            .iter()
            .filter(|i| matches!(i.status, ItemStatus::UpToDate))
            .count();

        // Game banner / thumbnail — use cached banner if available, else placeholder
        let banner: Element<Message> = match self.game_banner_cache.get(appid) {
            Some(handle) => widget::image(handle.clone())
                .width(Length::Fill)
                .height(80)
                .border_radius([8.0, 8.0, 0.0, 0.0])
                .content_fit(cosmic::iced::ContentFit::Cover)
                .into(),
            None => widget::container(
                widget::icon(from_svg_bytes(ICON_GAME)).size(32)
            )
                .width(Length::Fill)
                .height(80)
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .align_y(cosmic::iced::alignment::Vertical::Center)
                .style(|_theme| container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.18))),
                    border: Border {
                        radius: [8.0, 8.0, 0.0, 0.0].into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                })
                .into(),
        };

        let name = game.name.as_deref().unwrap_or("Unknown Game");

        // Stats row
        let stats = widget::row::with_capacity(3)
            .push(
                widget::text(format!("{} items", total))
                    .size(11)
                    .class(cosmic::style::Text::Default)
            )
            .push(widget::Space::new().width(Length::Fill))
            .push(
                colored_icon(ICON_CHECK, 12, Color::from_rgb(0.35, 0.6, 0.3))
            )
            .push(
                widget::text(format!(" {}", up_to_date))
                    .size(11)
                    .class(cosmic::style::Text::Default)
            )
            .push(
                colored_icon(ICON_CROSS, 12, Color::from_rgb(0.7, 0.2, 0.2))
            )
            .push(
                widget::text(format!(" {}", out_of_date))
                    .size(11)
                    .class(cosmic::style::Text::Default)
            )
            .align_y(cosmic::iced::Alignment::Center)
            .spacing(2);

        // Out-of-date warning badge
        let warning: Option<Element<Message>> = if out_of_date > 0 {
            Some(
                widget::container(
                    widget::text(format!("{} outdated", out_of_date))
                        .size(10)
                )
                    .padding([2, 6])
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.8, 0.2, 0.0, 0.85))),
                        border: Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..Default::default()
                    })
                    .into()
            )
        } else {
            None
        };

        let mut info_col = widget::column::with_capacity(3)
            .push(widget::text(name).size(13).font(cosmic::font::bold()))
            .push(stats)
            .spacing(spacing.space_xxxs);

        if let Some(badge) = warning {
            info_col = info_col.push(badge);
        }

        let card_content = widget::column::with_capacity(2)
            .push(banner)
            .push(
                widget::container(info_col)
                    .padding([spacing.space_xs, spacing.space_s])
                    .width(Length::Fill)
            )
            .width(Length::Fill);

        let appid_owned = appid.to_string();

        widget::button::custom(card_content)
            .on_press(Message::SelectGame(appid_owned))
            .width(Length::Fill)
            .class(if is_selected {
                cosmic::style::Button::Custom {
                    active: Box::new(|_, _| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(0.2, 0.5, 0.9, 0.3)
                        )),
                        border_radius: 8.0.into(),
                        border_color: Color::from_rgba(0.3, 0.6, 1.0, 0.7),
                        border_width: 1.5,
                        ..Default::default()
                    }),
                    hovered: Box::new(|_, _| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(0.25, 0.55, 0.95, 0.4)
                        )),
                        border_radius: 8.0.into(),
                        border_color: Color::from_rgba(0.3, 0.6, 1.0, 0.85),
                        border_width: 1.5,
                        ..Default::default()
                    }),
                    pressed: Box::new(|_, _| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(0.15, 0.45, 0.85, 0.45)
                        )),
                        border_radius: 8.0.into(),
                        ..Default::default()
                    }),
                    disabled: Box::new(|_| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(0.2, 0.5, 0.9, 0.3)
                        )),
                        border_radius: 8.0.into(),
                        ..Default::default()
                    }),
                }
            } else {
                cosmic::style::Button::Custom {
                    active: Box::new(|_, _| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                        )),
                        border_radius: 8.0.into(),
                        border_color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                        border_width: 1.0,
                        ..Default::default()
                    }),
                    hovered: Box::new(|_, _| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                        )),
                        border_radius: 8.0.into(),
                        border_color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
                        border_width: 1.0,
                        ..Default::default()
                    }),
                    pressed: Box::new(|_, _| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                        )),
                        border_radius: 8.0.into(),
                        ..Default::default()
                    }),
                    disabled: Box::new(|_| widget::button::Style {
                        background: Some(Background::Color(
                            Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                        )),
                        border_radius: 8.0.into(),
                        ..Default::default()
                    }),
                }
            })
            .into()
    }

    fn view_workshop_items<'a>(
        &'a self,
        appid: &'a str,
        items: &'a [WorkshopItem],
        game: Option<&'a GameEntry>,
        search_query: &'a str,
        sort_order: &SortOrder,
    ) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();
        let all_selected = !items.is_empty() && items.iter().all(|i| i.selected);

        let query = search_query.to_lowercase();
        let mut filtered: Vec<&WorkshopItem> = items
            .iter()
            .filter(|i| {
                if query.is_empty() {
                    return true;
                }
                i.name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query)
                    || i.item_id.contains(&query)
            })
            .collect();

        match sort_order {
            SortOrder::NameAsc => filtered.sort_by(|a, b| {
                a.name
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.name.as_deref().unwrap_or(""))
            }),
            SortOrder::NameDesc => filtered.sort_by(|a, b| {
                b.name
                    .as_deref()
                    .unwrap_or("")
                    .cmp(a.name.as_deref().unwrap_or(""))
            }),
            SortOrder::SizeAsc => filtered.sort_by_key(|i| i.disk_size),
            SortOrder::SizeDesc => filtered.sort_by_key(|i| std::cmp::Reverse(i.disk_size)),
            SortOrder::StatusFirst => filtered.sort_by_key(|i| {
                if matches!(i.status, ItemStatus::OutOfDate) { 0 } else { 1 }
            }),
        }

        let sort_options = vec![
            ("Name A→Z", SortOrder::NameAsc),
            ("Name Z→A", SortOrder::NameDesc),
            ("Size ↑", SortOrder::SizeAsc),
            ("Size ↓", SortOrder::SizeDesc),
            ("Status", SortOrder::StatusFirst),
        ];

        let sort_menu = widget::dropdown(
            &["Name A→Z", "Name Z→A", "Size ↑", "Size ↓", "Status"],
            sort_options.iter().position(|(_, o)| o == sort_order),
            |idx| {
                let order = match idx {
                    0 => SortOrder::NameAsc,
                    1 => SortOrder::NameDesc,
                    2 => SortOrder::SizeAsc,
                    3 => SortOrder::SizeDesc,
                    4 => SortOrder::StatusFirst,
                    _ => SortOrder::NameAsc,
                };
                Message::SortChanged(order)
            },
        );

        let game_banner = {
            let banner: Element<Message> = match game.and_then(|g| self.game_banner_cache.get(&g.appid)) {
                Some(handle) => widget::image(handle.clone())
                    .width(Length::Fill)
                    .height(256)
                    .border_radius(12.0)
                    .content_fit(cosmic::iced::ContentFit::Cover)
                    .into(),
                None => widget::container(widget::icon(from_svg_bytes(ICON_GAME)).size(48))
                    .width(Length::Fill)
                    .height(256)
                    .align_x(cosmic::iced::alignment::Horizontal::Center)
                    .align_y(cosmic::iced::alignment::Vertical::Center)
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
                        border: Border {
                            radius: 12.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..Default::default()
                    })
                    .into(),
            };

            let name = game.and_then(|g| g.name.as_deref()).unwrap_or("Unknown Game");

            widget::container(
                widget::column::with_capacity(2)
                    .push(banner)
                    .push(widget::text::title3(name))
                    .spacing(spacing.space_s)
                    .align_x(cosmic::iced::Alignment::Center),
            )
                .width(Length::Fill)
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .padding([0, 0, spacing.space_s, 0])
        };

        let appid_owned = appid.to_string();
        let appid_owned2 = appid.to_string();

        let header = widget::row::with_capacity(5)
            .push(
                widget::checkbox(all_selected)
                    .size(24)
                    .on_toggle(move |_| Message::ToggleAllItems(appid_owned.clone())),
            )
            .push(widget::text::body(format!(
                "{} item(s){}",
                filtered.len(),
                if filtered.len() != items.len() {
                    format!(" (filtered from {})", items.len())
                } else {
                    String::new()
                }
            )))
            .push(
                widget::search_input("Search...", search_query)
                    .on_input(Message::SearchChanged)
                    .width(Length::Fill),
            )
            .padding(spacing.space_s)
            .push(
                widget::button::standard(if self.compact_mode { "Expanded" } else { "Compact" })
                    .on_press(Message::ToggleCompactMode),
            )
            .push(
                widget::button::standard("Share")
                    .on_press(Message::GenerateCollectionCode),
            )
            .push(
                widget::button::standard("Import")
                    .on_press(Message::ToggleImportPanel),
            )
            .push(sort_menu)
            .align_y(cosmic::iced::Alignment::Center)
            .spacing(spacing.space_s);

        let rows = filtered.iter().fold(
            widget::column::with_capacity(filtered.len()).spacing(spacing.space_xs),
            |col, item| {
                let item_id_for_steam = item.item_id.clone();
                let item_id_toggle = item.item_id.clone();
                let selected = item.selected;
                let path = item.path.clone();
                let appid_steam = appid_owned2.clone();
                let appid_toggle = appid_owned2.clone();

                let is_out_of_date = matches!(item.status, ItemStatus::OutOfDate);

                let status_icon = match item.status {
                    ItemStatus::UpToDate  => colored_icon(ICON_CHECK, 24, Color::from_rgb(0.35, 0.6, 0.3)),
                    ItemStatus::OutOfDate => colored_icon(ICON_CROSS, 24, Color::from_rgb(0.7, 0.2, 0.2)),
                    ItemStatus::Unknown   => colored_icon(ICON_QUESTION, 24, Color::from_rgb(0.75, 0.75, 0.75)),
                };

                let status_text = match item.status {
                    ItemStatus::UpToDate  => "Up to Date",
                    ItemStatus::OutOfDate => "Out of Date",
                    ItemStatus::Unknown   => "Unknown",
                };

                let action_buttons = widget::row::with_capacity(2)
                    .push(
                        widget::button::icon(from_svg_bytes(ICON_FOLDER))
                            .on_press(Message::OpenFolder(path))
                    )
                    .push(
                        widget::button::icon(from_svg_bytes(ICON_STEAM))
                            .on_press(Message::OpenSteam {
                                appid: appid_steam,
                                item_id: item_id_for_steam,
                            })
                    )
                    .spacing(spacing.space_xs);

                let thumb: Element<Message> = if let Some(handle) = self.thumbnail_cache.get(&item.item_id) {
                    widget::image(handle.clone())
                        .width(64)
                        .height(64)
                        .border_radius(12.0)
                        .content_fit(cosmic::iced::ContentFit::Cover)
                        .into()
                } else {
                    widget::container(
                        widget::icon(from_svg_bytes(ICON_GAME)).size(32)
                    )
                        .width(64)
                        .height(64)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .align_y(cosmic::iced::alignment::Vertical::Center)
                        .into()
                };

                let thumbnail = widget::container(
                    widget::container(thumb)
                        .width(64)
                        .height(64)
                        .style(|_theme| container::Style {
                            background: Some(Background::Color(Color::from_rgb(0.85, 0.85, 0.85))),
                            border: Border {
                                radius: 12.0.into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                            ..Default::default()
                        })
                )
                    .width(70)
                    .height(70)
                    .align_x(cosmic::iced::alignment::Horizontal::Center)
                    .align_y(cosmic::iced::alignment::Vertical::Center)
                    .style(|_theme| container::Style {
                        background: None,
                        border: Border {
                            radius: 14.0.into(),
                            width: 2.0,
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.75),
                        },
                        ..Default::default()
                    });

                let row = if self.compact_mode {
                    widget::row::with_capacity(2)
                        .push(
                            widget::row::with_capacity(2)
                                .push(
                                    widget::row::with_capacity(3)
                                        .push(
                                            widget::text(item.name.as_deref().unwrap_or(&item.item_id))
                                                .size(15)
                                                .font(cosmic::font::bold())
                                        )
                                        .push(widget::text(format!("•  {}", status_text)))
                                        .push(status_icon)
                                        .align_y(cosmic::iced::Alignment::Center)
                                        .width(Length::Fill)
                                        .spacing(spacing.space_xs)
                                )
                                .push(action_buttons)
                                .align_y(cosmic::iced::Alignment::Center)
                                .padding([spacing.space_xxxs, spacing.space_xxxs, spacing.space_xxxs, spacing.space_s])
                                .width(Length::Fill)
                        )
                        .align_y(cosmic::iced::Alignment::Start)
                        .width(Length::Fill)
                } else {
                    widget::row::with_capacity(2)
                        .push(
                            widget::container(thumbnail)
                                .align_y(cosmic::iced::alignment::Vertical::Center)
                                .padding([spacing.space_xs, spacing.space_xs, spacing.space_xs, spacing.space_xs])
                        )
                        .push(
                            widget::row::with_capacity(2)
                                .push(
                                    widget::column::with_capacity(3)
                                        .push(
                                            widget::row::with_capacity(3)
                                                .push(
                                                    widget::text(item.name.as_deref().unwrap_or(&item.item_id))
                                                        .size(18)
                                                        .font(cosmic::font::bold())
                                                )
                                                .push(widget::text(format!("•  {}", status_text)))
                                                .push(status_icon)
                                                .align_y(cosmic::iced::Alignment::Center)
                                                .spacing(spacing.space_xs)
                                        )
                                        .push(widget::divider::horizontal::default())
                                        .push(
                                            widget::text(format!(
                                                "ID: {}  •  Size: {}  •  Local: {}  •  Remote: {}",
                                                item.item_id,
                                                format_size(item.disk_size),
                                                format_timestamp(item.local_timestamp),
                                                format_timestamp(item.remote_timestamp),
                                            ))
                                                .size(13)
                                                .class(cosmic::style::Text::Default)
                                        )
                                        .spacing(spacing.space_xs)
                                        .width(Length::Fill)
                                )
                                .push(action_buttons)
                                .align_y(cosmic::iced::Alignment::Center)
                                .padding([spacing.space_xs, spacing.space_xs, spacing.space_xs, 0])
                                .width(Length::Fill)
                        )
                        .align_y(cosmic::iced::Alignment::Start)
                        .width(Length::Fill)
                };

                let card = widget::button::custom(row)
                    .on_press(Message::ToggleItem {
                        appid: appid_toggle,
                        item_id: item_id_toggle,
                    })
                    .width(Length::Fill)
                    .class(if selected {
                        cosmic::style::Button::Custom {
                            active: Box::new(|_, _| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgba(0.2, 0.5, 0.9, 0.35))),
                                border_radius: 4.0.into(),
                                border_color: Color::from_rgba(0.3, 0.6, 1.0, 0.7),
                                border_width: 1.5,
                                ..Default::default()
                            }),
                            hovered: Box::new(|_, _| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgba(0.25, 0.55, 0.95, 0.45))),
                                border_radius: 4.0.into(),
                                border_color: Color::from_rgba(0.3, 0.6, 1.0, 0.85),
                                border_width: 1.5,
                                ..Default::default()
                            }),
                            pressed: Box::new(|_, _| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgba(0.15, 0.45, 0.85, 0.5))),
                                border_radius: 4.0.into(),
                                ..Default::default()
                            }),
                            disabled: Box::new(|_| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgba(0.2, 0.5, 0.9, 0.35))),
                                border_radius: 4.0.into(),
                                ..Default::default()
                            }),
                        }
                    } else if is_out_of_date {
                        cosmic::style::Button::Custom {
                            active: Box::new(|_, _| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgb(0.8, 0.3, 0.0))),
                                border_radius: 4.0.into(),
                                ..Default::default()
                            }),
                            hovered: Box::new(|_, _| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgb(0.9, 0.4, 0.1))),
                                border_radius: 4.0.into(),
                                ..Default::default()
                            }),
                            pressed: Box::new(|_, _| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgb(0.7, 0.25, 0.0))),
                                border_radius: 4.0.into(),
                                ..Default::default()
                            }),
                            disabled: Box::new(|_| widget::button::Style {
                                background: Some(Background::Color(Color::from_rgb(0.8, 0.3, 0.0))),
                                border_radius: 4.0.into(),
                                ..Default::default()
                            }),
                        }
                    } else {
                        cosmic::style::Button::MenuItem
                    });

                col.push(card)
            },
        ).spacing(if self.compact_mode { spacing.space_xxxs } else { spacing.space_xs });

        widget::column::with_capacity(2)
            .push(
                widget::container(game_banner)
                    .padding([0, spacing.space_xs])
            )
            .push(widget::divider::horizontal::default())
            .push(header)
            .push(widget::divider::horizontal::default())
            .push(
                widget::scrollable(
                    widget::column::with_capacity(4)
                        .push(rows)
                        .spacing(spacing.space_m)
                        .padding([spacing.space_xs, spacing.space_m, spacing.space_xs, spacing.space_xs]),
                )
                    .height(Length::Fill)
                    .width(Length::Fill)
            )
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    fn view_bottom_bar(
        &self,
        appid: &str,
        selected_count: usize,
        has_outdated: bool,
    ) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let busy = self.polling.is_some() || self.redownload_in_progress;
        let appid = appid.to_string();

        if self.confirming_redownload {
            let label = widget::text::body(format!(
                "Delete and redownload {} item(s) from Steam? This cannot be undone.",
                selected_count
            ));

            let confirm = widget::button::destructive("Yes, redownload").on_press_maybe(
                if busy { None } else { Some(Message::ForceRedownload) }
            );
            let cancel = widget::button::standard("Cancel").on_press(Message::CancelRedownload);

            return widget::row::with_capacity(3)
                .push(label)
                .push(widget::Space::new().width(Length::Fill))
                .push(cancel)
                .push(confirm)
                .align_y(cosmic::iced::Alignment::Center)
                .padding(spacing.space_s)
                .spacing(spacing.space_s)
                .into();
        }

        let label = if selected_count == 0 {
            widget::text::body("No items selected")
        } else {
            widget::text::body(format!("{} item(s) selected", selected_count))
        };

        let refresh_button = widget::button::standard("Refresh").on_press_maybe(
            if busy { None } else { Some(Message::RefreshGame(appid.clone())) }
        );

        let select_outdated_button = widget::tooltip(
            widget::button::standard("Select Out of Date").on_press_maybe(
                if busy || !has_outdated { None } else { Some(Message::SelectOutOfDate) }
            ),
            widget::text::body("Select all out of date workshop items."),
            widget::tooltip::Position::Top,
        );

        let redownload_button = widget::tooltip(
            widget::button::destructive("Force Redownload").on_press_maybe(
                if busy || selected_count == 0 { None } else { Some(Message::ConfirmRedownload) }
            ),
            widget::text::body("Delete local files and re-queue download from Steam"),
            widget::tooltip::Position::Top,
        );

        widget::row::with_capacity(4)
            .push(label)
            .push(widget::Space::new().width(Length::Fill))
            .push(refresh_button)
            .push(select_outdated_button)
            .push(redownload_button)
            .align_y(cosmic::iced::Alignment::Center)
            .padding(spacing.space_s)
            .spacing(spacing.space_s)
            .into()
    }

    fn view_status_bar(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let inner: Element<'_, Message> = if self.redownload_complete {
            widget::row::with_capacity(2)
                .push(widget::text::body("✓ All items redownloaded successfully.").width(Length::Fill))
                .push(widget::button::standard("Dismiss").on_press(Message::DismissComplete))
                .align_y(cosmic::iced::Alignment::Center)
                .spacing(spacing.space_s)
                .into()
        } else if self.redownload_in_progress {
            widget::column::with_capacity(2)
                .push(
                    widget::text::body("Deleting local files and queuing Steam download...")
                        .width(Length::Fill),
                )
                .push(
                    cosmic::iced::widget::progress_bar(0.0..=100.0, 0.0)
                        .girth(15)
                        .length(Length::Fill),
                )
                .spacing(spacing.space_xs)
                .into()
        } else if let Some(ref poll) = self.polling {
            let pending_count = poll.pending_item_ids.len();
            let resolved_count = poll.initial_item_count.saturating_sub(pending_count);
            let elapsed = poll.elapsed_secs;

            let resolution_progress = if poll.initial_item_count > 0 {
                (resolved_count as f32 / poll.initial_item_count as f32) * 70.0
            } else {
                0.0
            };
            let time_progress = (elapsed as f32 / POLL_TIMEOUT_SECS as f32).min(1.0) * 30.0;
            let total_progress = (resolution_progress + time_progress).min(100.0);

            let status_text = if elapsed == 0 {
                format!("Waiting for Steam to start downloading {} item(s)...", pending_count)
            } else if pending_count == 0 {
                "All items downloaded — verifying...".to_string()
            } else {
                format!(
                    "Downloading — {} of {} item(s) resolved ({}s elapsed)",
                    resolved_count, poll.initial_item_count, elapsed,
                )
            };

            widget::column::with_capacity(2)
                .push(
                    widget::row::with_capacity(2)
                        .push(widget::text::body(status_text).width(Length::Fill))
                        .push(widget::button::standard("Stop watching").on_press(Message::StopPolling))
                        .align_y(cosmic::iced::Alignment::Center)
                        .spacing(spacing.space_s),
                )
                .push(
                    cosmic::iced::widget::progress_bar(0.0..=100.0, total_progress)
                        .girth(15)
                        .length(Length::Fill),
                )
                .spacing(spacing.space_xs)
                .into()
        } else {
            return widget::Space::new().height(0).into();
        };

        widget::container(inner)
            .padding([spacing.space_s, spacing.space_m])
            .width(Length::Fill)
            .into()
    }

    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = "Steam Workshop Utility".to_string();

        if let (Some(appid), AppState::Loaded { games, .. }) = (&self.selected_game, &self.state) {
            if let Some(game) = games.get(appid) {
                if let Some(name) = &game.name {
                    window_title.push_str(" — ");
                    window_title.push_str(name);
                }
            }
        }

        self.set_header_title(window_title.clone());

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }

    fn load_thumbnails_for_active_game(&self) -> Task<cosmic::Action<Message>> {
        let AppState::Loaded { items, .. } = &self.state else {
            return Task::none();
        };

        let Some(ref appid) = self.selected_game else {
            return Task::none();
        };

        let Some(game_items) = items.get(appid) else {
            return Task::none();
        };

        let to_fetch: Vec<(String, String)> = game_items
            .iter()
            .filter_map(|item| {
                let url = item.preview_url.clone()?;
                if self.thumbnail_cache.contains_key(&item.item_id) {
                    return None;
                }
                Some((item.item_id.clone(), url))
            })
            .collect();

        if to_fetch.is_empty() {
            return Task::none();
        }

        Task::perform(
            async move {
                let client = reqwest::Client::new();
                let mut cache = HashMap::new();

                for (item_id, url) in to_fetch {
                    let result = client
                        .get(&url)
                        .send()
                        .await
                        .and_then(|r| r.error_for_status())
                        .ok();

                    if let Some(resp) = result {
                        if let Ok(bytes) = resp.bytes().await {
                            let handle = widget::image::Handle::from_bytes(bytes.to_vec());
                            cache.insert(item_id, handle);
                        }
                    }
                }

                cache
            },
            |cache| cosmic::Action::App(Message::ThumbnailsLoaded(cache)),
        )
    }

    /// Load banner for a specific appid (not just the active game)
    fn load_banner_for_appid(&self, appid: String) -> Task<cosmic::Action<Message>> {
        if self.game_banner_cache.contains_key(&appid) {
            return Task::none();
        }

        Task::perform(
            async move {
                let client = reqwest::Client::new();
                let simple_urls = [
                    format!(
                        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_hero.jpg",
                        appid
                    ),
                    format!(
                        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg",
                        appid
                    ),
                ];
                for url in &simple_urls {
                    if let Some(handle) = try_fetch_image(&client, url).await {
                        return Some((appid, handle));
                    }
                }

                let api_url = format!(
                    "https://store.steampowered.com/api/appdetails?appids={}&filters=basic",
                    appid
                );
                if let Ok(resp) = client.get(&api_url).send().await {
                    if let Ok(text) = resp.text().await {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            let header_image =
                                json[&appid]["data"]["header_image"].as_str().unwrap_or("");
                            if !header_image.is_empty() {
                                if let Some(handle) = try_fetch_image(&client, header_image).await {
                                    return Some((appid, handle));
                                }
                            }
                        }
                    }
                }
                None
            },
            |result| match result {
                Some((appid, handle)) => cosmic::Action::App(Message::GameBannerLoaded { appid, handle }),
                None => cosmic::Action::App(Message::GameBannerLoaded {
                    appid: String::new(),
                    handle: widget::image::Handle::from_bytes(vec![]),
                }),
            },
        )
    }

    fn view_collection_code_drawer(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let Some(code) = &self.generated_code else {
            return widget::container(
                widget::text::body("No code generated yet. Select items and press Share.")
            )
                .padding(spacing.space_m)
                .into();
        };

        let decoded = CollectionCode::decode(code);
        let item_count = decoded.as_ref().map(|c| c.items.len()).unwrap_or(0);
        let game_name = decoded.map(|c| c.name).unwrap_or_default(); // owned String, no borrow

        widget::column::with_capacity(5)
            .push(
                widget::container(
                    widget::column::with_capacity(2)
                        .push(widget::text::title4(game_name))
                        .push(
                            widget::text::body(format!("{} workshop item(s)", item_count))
                                .class(cosmic::style::Text::Default),
                        )
                        .spacing(spacing.space_xxxs),
                )
                    .padding(spacing.space_m)
                    .width(Length::Fill)
                    .style(|theme| container::Style {
                        background: Some(Background::Color(
                            theme.cosmic().palette.neutral_2.into(),
                        )),
                        border: Border {
                            radius: 8.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..Default::default()
                    }),
            )
            .push(widget::divider::horizontal::default())
            .push(widget::text::body(
                "Share this code with others so they can subscribe to the same mods.",
            ))
            .push(
                widget::container(
                    widget::scrollable(
                        widget::container(
                            widget::text(code.as_str())
                                .size(11)
                                .font(cosmic::font::mono())
                                .width(Length::Fill),
                        ).padding([0,0,spacing.space_s,0]),
                    )
                        .width(Length::Fill).horizontal(),
                )
                    .padding(spacing.space_s)
                    .width(Length::Fill)
                    .style(|theme| container::Style {
                        background: Some(Background::Color(
                            theme.cosmic().palette.neutral_3.into(),
                        )),
                        border: Border {
                            radius: 6.0.into(),
                            width: 1.0,
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                        },
                        ..Default::default()
                    }),
            )
            .push(
                widget::button::suggested("Copy to Clipboard")
                    .on_press(Message::CopyCodeToClipboard(code.clone()))
                    .width(Length::Fill),
            )
            .spacing(spacing.space_m)
            .padding(spacing.space_m)
            .width(Length::Fill)
            .into()
    }

    fn view_import_drawer(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        if let Some(ref collection) = self.pending_import {
            let item_count = collection.items.len();
            let game_name = collection.name.clone();
            let appid = collection.appid.clone();

            let installed_ids: HashSet<String> = if let AppState::Loaded { items, .. } = &self.state {
                items
                    .get(&appid)
                    .map(|gi| gi.iter().map(|i| i.item_id.clone()).collect())
                    .unwrap_or_default()
            } else {
                HashSet::new()
            };

            let collection_item_ids: HashSet<String> = collection.items.iter().cloned().collect();

            let new_count = collection.items.iter().filter(|id| !installed_ids.contains(*id)).count();
            let mut extra_ids: Vec<String> = installed_ids
                .iter()
                .filter(|id| !collection_item_ids.contains(*id))
                .cloned()
                .collect();

            extra_ids.sort();
            let extra_count = extra_ids.len();

            let mut sorted_collection_items = collection.items.clone();
            sorted_collection_items.sort_by(|a, b| {
                let name_a = self.import_item_names.get(a).map(|s| s.as_str()).unwrap_or(a.as_str());
                let name_b = self.import_item_names.get(b).map(|s| s.as_str()).unwrap_or(b.as_str());
                name_a.cmp(name_b)
            });

            extra_ids.sort_by(|a, b| {
                let name_a = self.import_item_names.get(a).map(|s| s.as_str()).unwrap_or(a.as_str());
                let name_b = self.import_item_names.get(b).map(|s| s.as_str()).unwrap_or(b.as_str());
                name_a.cmp(name_b)
            });

            // Helper closure to build a clickable item row
            let make_item_row = |item_id: &String, is_installed: bool, appid: &String| {
                let name = self.import_item_names
                    .get(item_id)
                    .cloned()
                    .unwrap_or_else(|| item_id.clone());

                let status_badge = widget::container(
                    widget::text(if is_installed { "Installed" } else { "New" }).size(10),
                )
                    .padding([2, 6])
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(if is_installed {
                            Color::from_rgba(0.2, 0.6, 0.2, 0.4)
                        } else {
                            Color::from_rgba(0.2, 0.4, 0.8, 0.4)
                        })),
                        border: Border {
                            radius: 4.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..Default::default()
                    });

                widget::button::custom(
                    widget::row::with_capacity(3)
                        .push(widget::text(name).size(13).width(Length::Fill))
                        .push(status_badge)
                        .push(widget::icon(from_svg_bytes(ICON_STEAM)).size(14))
                        .align_y(cosmic::iced::Alignment::Center)
                        .spacing(spacing.space_xs)
                        .padding([spacing.space_xs, spacing.space_s]),
                )
                    .on_press(Message::OpenSteam {
                        appid: appid.clone(),
                        item_id: item_id.clone(),
                    })
                    .width(Length::Fill)
                    .class(cosmic::style::Button::MenuItem)
            };

            // Collection items list
            let collection_rows = sorted_collection_items.iter().fold(
                widget::column::with_capacity(sorted_collection_items.len()).spacing(spacing.space_xxxs),
                |col, item_id| {
                    let is_installed = installed_ids.contains(item_id);
                    col.push(make_item_row(item_id, is_installed, &appid))
                },
            );

            // Extra items list (installed but not in collection)
            let extra_rows = extra_ids.iter().fold(
                widget::column::with_capacity(extra_ids.len()).spacing(spacing.space_xxxs),
                |col, item_id| {
                    col.push(make_item_row(item_id, true, &appid))
                },
            );

            let extra_section: Element<Message> = if extra_count > 0 {
                widget::column::with_capacity(2)
                    .push(
                        widget::text::body(format!("Not in collection ({})", extra_count))
                            .class(cosmic::style::Text::Default),
                    )
                    .push(extra_rows)
                    .spacing(spacing.space_xs)
                    .into()
            } else {
                widget::Space::new().height(0).into()
            };

            return widget::column::with_capacity(5)
                .push(
                    widget::container(
                        widget::column::with_capacity(3)
                            .push(widget::text::title4(game_name))
                            .push(
                                widget::text::body(format!("{} item(s) in collection", item_count))
                                    .class(cosmic::style::Text::Default),
                            )
                            .push(
                                widget::text(format!("{} new  •  {} not in collection", new_count, extra_count))
                                    .size(11)
                                    .class(cosmic::style::Text::Default),
                            )
                            .spacing(spacing.space_xxxs),
                    )
                        .padding(spacing.space_m)
                        .width(Length::Fill)
                        .style(|theme| container::Style {
                            background: Some(Background::Color(
                                theme.cosmic().palette.neutral_2.into(),
                            )),
                            border: Border {
                                radius: 8.0.into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                            ..Default::default()
                        }),
                )
                .push(
                    widget::container(
                        widget::text("Steam does not expose public APIs for a lot of functions, so you'll have to click on each item to subscribe/unsubscribe manually.")
                            .class(cosmic::style::Text::Default)
                            .size(12),
                    )
                )
                .push(
                    widget::scrollable(
                        widget::column::with_capacity(4)
                            .push(
                                widget::text::body(format!("In collection ({})", item_count))
                                    .class(cosmic::style::Text::Default),
                            )
                            .push(collection_rows)
                            .push(widget::Space::new().height(spacing.space_s))
                            .push(extra_section)
                            .spacing(spacing.space_xs)
                            .padding([spacing.space_xs, 0]),
                    )
                        .height(Length::FillPortion(3)),
                )
                .push(widget::divider::horizontal::default())
                .push(
                    widget::button::standard("← Back")
                        .on_press(Message::ClearImport)
                        .width(Length::Fill)
                )
                .spacing(spacing.space_m)
                .padding(spacing.space_m)
                .width(Length::Fill)
                .into();
        }

        // Default: paste input step
        let error_row: Element<Message> = if self.import_error {
            widget::container(
                widget::text::body("Invalid code. Make sure you copied the full SWUC_ code.")
                    .class(cosmic::style::Text::Color(Color::from_rgb(0.9, 0.3, 0.2))),
            )
                .padding([spacing.space_xs, 0])
                .into()
        } else {
            widget::Space::new().height(0).into()
        };

        widget::column::with_capacity(4)
            .push(widget::text::body(
                "Paste a collection code shared by someone else to subscribe to their mod list.",
            ))
            .push(
                widget::text_input("Paste SWUC_ code here...", &self.import_code_input)
                    .on_input(Message::ImportCodeChanged)
                    .width(Length::Fill),
            )
            .push(error_row)
            .push(
                widget::button::suggested("Import")
                    .on_press_maybe(if self.import_code_input.trim().is_empty() {
                        None
                    } else {
                        Some(Message::ImportCode)
                    })
                    .width(Length::Fill),
            )
            .spacing(spacing.space_m)
            .padding(spacing.space_m)
            .width(Length::Fill)
            .into()
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
    CollectionCode,
    ImportCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
        }
    }
}