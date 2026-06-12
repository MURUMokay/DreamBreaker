#![allow(warnings)]
#![allow(clippy::all)]
mod db;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{
    button, column, container, mouse_area, pick_list, row, scrollable, stack, svg, text,
    text_input, Button, Space,
};
use iced::{window, Alignment, Background, Color, Element, Length, Subscription, Task};
use sqlx::types::Uuid;
use sqlx::PgPool;

fn main() -> iced::Result {
    let _ = dotenvy::dotenv();
    iced::application("DreamBreaker", DreamBreaker::update, DreamBreaker::view)
        .subscription(DreamBreaker::subscription)
        .run_with(DreamBreaker::new)
}

// ─────────────────────────────────────────────
// Режим окна
// ─────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowMode {
    Window,
    Fullscreen,
    Borderless,
}

impl std::fmt::Display for WindowMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowMode::Window => write!(f, "Окно"),
            WindowMode::Fullscreen => write!(f, "Полноэкранный"),
            WindowMode::Borderless => write!(f, "Полноэкранный в окне"),
        }
    }
}

const WINDOW_MODES: &[WindowMode] = &[
    WindowMode::Window,
    WindowMode::Fullscreen,
    WindowMode::Borderless,
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnPhase {
    WaitingRoll,   // ждём броска кубика
    Rolled,        // кубик брошен, игрок переместился — можно завершить ход
    WaitingAction, // ждём действия на клетке (купить/пропустить/аренда)
}
#[derive(Debug, Clone)]
enum CellAction {
    CanBuy {
        cell_index: i32,
        cost: i64,
        name: String,
    },
    MustPayRent {
        cell_index: i32,
        rent: i64,
        owner: String,
    },
    Tax, // <- добавить
    Info(String),
    Shop {
        shop_id: Uuid,
    },
}
// ─────────────────────────────────────────────
// State
// ─────────────────────────────────────────────
#[derive(Debug, Clone)]
struct TooltipItem {
    name: String,
    description: String,
    effect_type: String,
    effect_value: i64,
    buy_cost: Option<i64>,  // None если из инвентаря
    sell_cost: Option<i64>, // None если из магазина (ещё не куплен)
}

struct DreamBreaker {
    screen: Screen,
    pool: Option<PgPool>,
    status: String,
    current_user: Option<(Uuid, String)>,
    profile_menu_open: bool,
    users: Vec<(Uuid, String)>,

    input_username: String,
    input_password: String,
    input_password2: String,
    form_error: String,

    games: Vec<db::Game>,
    has_active_game: Option<bool>,
    user_games: Vec<(Uuid, String, i64, i32, chrono::DateTime<chrono::Utc>)>,

    // Создание игры
    create_game_balance: i64,
    create_game_max_turns: i32,
    create_game_target_balance: i64,

    // Активная игра
    active_game_id: Option<Uuid>,
    game_rules: Option<db::GameRules>,
    player_state: Option<db::ParticipantState>,
    board_cells: Vec<db::BoardCell>,
    inventory: Vec<db::InventoryItem>,
    game_menu_open: bool,
    dice_roll: Option<(u8, u8)>,
    local_player_pos: i32,
    turn_phase: TurnPhase,           // фаза хода
    cell_action: Option<CellAction>, // текущее действие на клетке
    shop_slots: Vec<db::ShopSlot>,   // текущие слоты открытого магазина

    // Экран завершения
    game_result: Option<db::GameResult>,
    user_stats: Option<db::UserStats>,
    show_stats: bool,

    // Управление собственностью
    show_property_mgmt: bool,
    player_properties: Vec<db::PlayerProperty>,
    selected_property: Option<usize>,

    // Боты
    bot_participants: Vec<db::BotParticipant>,
    bot_turn_queue: Vec<Uuid>,
    bot_turn_names: Vec<String>,
    bot_turn_log: Vec<String>,

    // Тултип усиления
    tooltip_item: Option<TooltipItem>,
    tooltip_hover_start: Option<std::time::Instant>,
    tooltip_visible: bool,
    tooltip_locked: bool,

    // Zoom/pan игрового поля
    board_zoom: f32,
    board_pan_x: f32,
    board_pan_y: f32,
    board_dragging: bool,
    board_mouse_pos: (f32, f32),

    // Лог пополнений баланса игрока
    income_log: Vec<String>,
    start_log: Vec<String>,
    rent_received_log: Vec<String>,

    // Наведение на клетку
    hovered_cell: Option<i32>,

    // Настройки
    window_mode: WindowMode,

    settings_from_game: bool,
    window_width: f32,
    window_height: f32,
}

impl Default for DreamBreaker {
    fn default() -> Self {
        Self {
            screen: Screen::Menu,
            pool: None,
            status: String::new(),
            current_user: None,
            profile_menu_open: false,
            users: vec![],
            input_username: String::new(),
            input_password: String::new(),
            input_password2: String::new(),
            form_error: String::new(),
            games: vec![],
            has_active_game: None,
            user_games: vec![],
            create_game_balance: 1000,
            create_game_max_turns: 50,
            create_game_target_balance: 2500,
            active_game_id: None,
            game_rules: None,
            player_state: None,
            cell_action: None,
            shop_slots: Vec::new(),
            local_player_pos: 0,
            board_cells: vec![],
            inventory: vec![],
            game_menu_open: false,
            dice_roll: None,
            turn_phase: TurnPhase::WaitingRoll,
            game_result: None,
            user_stats: None,
            show_stats: false,
            show_property_mgmt: false,
            player_properties: vec![],
            selected_property: None,
            bot_participants: vec![],
            bot_turn_queue: vec![],
            bot_turn_names: vec![],
            bot_turn_log: vec![],
            tooltip_item: None,
            tooltip_hover_start: None,
            tooltip_visible: false,
            tooltip_locked: false,
            board_zoom: 1.0,
            board_pan_x: 0.0,
            board_pan_y: 0.0,
            board_dragging: false,
            board_mouse_pos: (0.0, 0.0),
            income_log: vec![],
            start_log: vec![],
            rent_received_log: vec![],
            hovered_cell: None,
            window_mode: WindowMode::Window,
            settings_from_game: false,
            window_width: 1024.0,
            window_height: 768.0,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    #[default]
    Menu,
    Login,
    Register,
    LoadGame,
    CreateGame,
    Game,
    GameOver, // экран завершения
    Settings,
}

// ─────────────────────────────────────────────
// Messages
// ─────────────────────────────────────────────
#[derive(Debug, Clone)]
enum Message {
    // Навигация
    OpenLogin,
    StaleGamesReset,
    OpenRegister,
    OpenLoadGame,
    OpenCreateGame,
    OpenSettings,
    BackToMenu,
    Logout,
    Exit,
    // Профиль
    ToggleProfileMenu,
    SelectUser(Uuid, String),
    OpenStats,
    ToggleStats, // открыть/закрыть оверлей статистики

    // Ввод
    UsernameChanged(String),
    PasswordChanged(String),
    Password2Changed(String),

    // Формы
    SubmitLogin,
    SubmitRegister,

    // Создание игры
    GameBalanceIncrease,
    GameBalanceDecrease,
    GameMaxTurnsIncrease,
    GameMaxTurnsDecrease,
    GameTargetBalanceIncrease,
    GameTargetBalanceDecrease,
    CreateGameSubmit,

    // Игровое меню
    ToggleGameMenu,
    SaveAndExit,
    Surrender,
    OpenSettingsFromGame,
    RollDice,
    EndTurn,
    TurnSaved(Result<db::ParticipantState, db::DbError>),
    ContinueGame, // загрузить последнюю игру напрямую
    BalancesLoaded(Result<Vec<(Uuid, i64, String)>, db::DbError>),
    BuyProperty,
    SkipAction,
    PropertyBought(Result<db::ParticipantState, db::DbError>),
    RentPaid(Result<db::ParticipantState, db::DbError>),
    BoardReloaded(Result<Vec<db::BoardCell>, db::DbError>),
    OpenShop(Uuid),
    TooltipClose,
    ShopSlotsLoaded(Result<Vec<db::ShopSlot>, db::DbError>),
    BuyShopSlot(Uuid),
    ShopSlotBought(Result<db::ParticipantState, db::DbError>),
    RerollShop(Uuid),
    ShopRerolled(Result<db::ParticipantState, db::DbError>),
    TaxPaid(Result<db::ParticipantState, db::DbError>),
    InventoryReloaded(Result<Vec<db::InventoryItem>, db::DbError>),
    SellPowerUp(Uuid),
    PowerUpSold(Result<db::ParticipantState, db::DbError>),

    // Управление собственностью
    TogglePropertyMgmt,
    SelectProperty(usize),
    PropertiesLoaded(Result<Vec<db::PlayerProperty>, db::DbError>),
    InstallUpgrade {
        property_id: Uuid,
        power_up_id: Uuid,
    },
    UninstallUpgrade {
        property_id: Uuid,
        power_up_id: Uuid,
    },
    UpgradeInstalled(Result<String, db::DbError>),
    UpgradeUninstalled(Result<String, db::DbError>),

    // Боты
    BotsLoaded(Result<Vec<db::BotParticipant>, db::DbError>),
    BotsRefreshed(Result<Vec<db::BotParticipant>, db::DbError>), // только обновить позиции, без запуска ходов
    PlayerStateRefreshed(Result<Option<db::ParticipantState>, db::DbError>),
    BotTurnDone(Result<db::BotTurnResult, db::DbError>),
    TooltipHoverStart(TooltipItem),
    TooltipHoverEnd,
    TooltipTick,
    // Zoom/pan поля
    BoardZoom(f32),
    BoardScrollTo(iced::widget::scrollable::AbsoluteOffset),
    BoardScrolled(iced::widget::scrollable::Viewport),
    BoardDragStart,
    BoardDragEnd,
    BoardMouseMove(f32, f32),
    // Наведение на клетку доски
    CellHovered(Option<i32>),

    // Настройки
    SetWindowMode(WindowMode),
    WindowResized(f32, f32),

    // БД
    DbConnected(Result<PgPool, db::DbError>),
    MigrationsApplied(Result<(), db::DbError>),
    UsersLoaded(Result<Vec<(Uuid, String)>, db::DbError>),
    LoginDone(Result<db::User, db::DbError>),
    RegisterDone(Result<Uuid, db::DbError>),
    GamesLoaded(Result<Vec<db::Game>, db::DbError>),
    ActiveGameChecked(Result<Option<Uuid>, db::DbError>),
    UserGamesLoaded(
        Result<Vec<(Uuid, String, i64, i32, chrono::DateTime<chrono::Utc>)>, db::DbError>,
    ),
    GameCreated(Result<Uuid, db::DbError>),
    LoadGame(Uuid),
    GameScreenLoaded(
        Result<
            (
                db::GameRules,
                db::ParticipantState,
                Vec<db::BoardCell>,
                Vec<db::InventoryItem>,
                Vec<db::PlayerProperty>,
            ),
            db::DbError,
        >,
    ),
    GamePaused(Result<(), db::DbError>),
    GameSurrendered(Result<(), db::DbError>),
    StatsLoaded(Result<db::UserStats, db::DbError>),
    GameResultLoaded(Result<Option<db::GameResult>, db::DbError>),
}

// ─────────────────────────────────────────────
// Update
// ─────────────────────────────────────────────
impl DreamBreaker {
    fn new() -> (Self, Task<Message>) {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(url) => url,
            Err(_) => {
                return (
                    Self {
                        status: "Ошибка: DATABASE_URL не задана.".to_string(),
                        ..Default::default()
                    },
                    Task::none(),
                )
            }
        };
        let state = Self {
            status: "Подключение к БД...".to_string(),
            ..Default::default()
        };
        let task = Task::perform(
            async move { db::connect(&database_url).await },
            Message::DbConnected,
        );
        (state, task)
    }

    fn subscription(&self) -> Subscription<Message> {
        let window_sub = iced::event::listen_with(|event, _status, _id| match event {
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::WindowResized(size.width, size.height))
            }
            _ => None,
        });

        window_sub
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // ── Навигация ──────────────────────────────────
            Message::OpenLogin => {
                self.screen = Screen::Login;
                self.profile_menu_open = false;
                self.clear_form();
                Task::none()
            }
            Message::OpenRegister => {
                self.screen = Screen::Register;
                self.profile_menu_open = false;
                self.clear_form();
                Task::none()
            }
            Message::OpenLoadGame => {
                self.screen = Screen::LoadGame;
                self.profile_menu_open = false;
                if let Some((user_id, _)) = self.current_user.clone() {
                    if let Some(pool) = self.pool.clone() {
                        return Task::perform(
                            async move { db::get_user_games(&pool, user_id).await },
                            Message::UserGamesLoaded,
                        );
                    }
                }
                Task::none()
            }
            Message::OpenCreateGame => {
                self.screen = Screen::CreateGame;
                self.profile_menu_open = false;
                self.create_game_balance = 1000;
                self.create_game_max_turns = 50;
                self.create_game_target_balance = 2500;
                Task::none()
            }
            Message::OpenSettings => {
                self.settings_from_game = false;
                self.screen = Screen::Settings;
                self.profile_menu_open = false;
                Task::none()
            }
            Message::OpenSettingsFromGame => {
                self.settings_from_game = true;
                self.screen = Screen::Settings;
                self.game_menu_open = false;
                Task::none()
            }
            Message::BackToMenu => {
                if self.settings_from_game {
                    self.settings_from_game = false;
                    self.screen = Screen::Game;
                } else {
                    self.screen = Screen::Menu;
                    self.profile_menu_open = false;
                    self.clear_form();
                    self.local_player_pos = 0;
                    self.player_state = None;
                    self.game_rules = None;
                    self.board_cells.clear();
                    self.active_game_id = None;
                    self.dice_roll = None;
                    self.turn_phase = TurnPhase::WaitingRoll;
                    self.cell_action = None;
                }
                Task::none()
            }
            Message::Logout => {
                self.current_user = None;
                self.has_active_game = None;
                self.active_game_id = None;
                self.game_rules = None;
                self.player_state = None;
                self.board_cells.clear();
                self.inventory.clear();
                self.game_result = None;
                self.user_stats = None;
                self.show_stats = false;
                self.screen = Screen::Menu;
                self.profile_menu_open = false;
                self.status = "Вы вышли из аккаунта".to_string();
                Task::none()
            }
            Message::Exit => std::process::exit(0),

            // ── Профиль ────────────────────────────────────
            Message::ToggleProfileMenu => {
                self.profile_menu_open = !self.profile_menu_open;
                Task::none()
            }
            Message::SelectUser(_id, username) => {
                self.profile_menu_open = false;
                self.screen = Screen::Login;
                self.input_username = username;
                self.input_password = String::new();
                self.form_error = String::new();
                Task::none()
            }
            Message::ToggleStats => {
                self.show_stats = !self.show_stats;
                Task::none()
            }
            Message::OpenStats => {
                self.profile_menu_open = false;
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::get_user_stats(&pool, user_id).await },
                        Message::StatsLoaded,
                    )
                } else {
                    Task::none()
                }
            }

            // ── Ввод ──────────────────────────────────────
            Message::UsernameChanged(v) => {
                self.input_username = v;
                Task::none()
            }
            Message::PasswordChanged(v) => {
                self.input_password = v;
                Task::none()
            }
            Message::Password2Changed(v) => {
                self.input_password2 = v;
                Task::none()
            }

            // ── Вход ───────────────────────────────────────
            Message::SubmitLogin => {
                let username = self.input_username.trim().to_string();
                let password = self.input_password.clone();
                if username.is_empty() || password.is_empty() {
                    self.form_error = "Заполни все поля".to_string();
                    return Task::none();
                }
                self.form_error = String::new();
                self.status = "Проверка пароля...".to_string();
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::authenticate_user(&pool, &username, &password).await },
                        Message::LoginDone,
                    )
                } else {
                    self.form_error = "БД не подключена".to_string();
                    Task::none()
                }
            }
            Message::LoginDone(Ok(user)) => {
                let user_id = user.id;
                self.current_user = Some((user.id, user.username.clone()));
                self.status = format!("Добро пожаловать, {}!", user.username);
                self.screen = Screen::Menu;
                self.clear_form();
                self.has_active_game = None;
                if let Some(pool) = self.pool.clone() {
                    let pool2 = pool.clone();
                    let pool3 = pool.clone();
                    Task::batch([
                        Task::perform(
                            async move { db::list_users(&pool).await },
                            Message::UsersLoaded,
                        ),
                        Task::perform(
                            async move { db::get_active_game_for_user(&pool2, user_id).await },
                            Message::ActiveGameChecked,
                        ),
                        Task::perform(
                            async move {
                                let _ = db::reset_stale_active_games(&pool3, user_id).await;
                            },
                            |_| Message::StaleGamesReset,
                        ),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::LoginDone(Err(e)) => {
                self.form_error = e.0.clone();
                self.status = "Ошибка входа".to_string();
                Task::none()
            }

            // ── Регистрация ────────────────────────────────
            Message::SubmitRegister => {
                let username = self.input_username.trim().to_string();
                let password = self.input_password.clone();
                let password2 = self.input_password2.clone();
                if username.is_empty() || password.is_empty() {
                    self.form_error = "Заполни все поля".to_string();
                    return Task::none();
                }
                if username.len() < 3 {
                    self.form_error = "Имя: минимум 3 символа".to_string();
                    return Task::none();
                }
                if password.len() < 4 {
                    self.form_error = "Пароль: минимум 4 символа".to_string();
                    return Task::none();
                }
                if password != password2 {
                    self.form_error = "Пароли не совпадают".to_string();
                    return Task::none();
                }
                self.form_error = String::new();
                self.status = "Создание аккаунта...".to_string();
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::register_user(&pool, &username, &password).await },
                        Message::RegisterDone,
                    )
                } else {
                    self.form_error = "БД не подключена".to_string();
                    Task::none()
                }
            }
            Message::RegisterDone(Ok(new_user_id)) => {
                self.status = "Аккаунт создан!".to_string();
                self.clear_form();
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::get_user_by_id(&pool, new_user_id).await },
                        |res| match res {
                            Ok(Some(user)) => Message::LoginDone(Ok(user)),
                            Ok(None) => Message::LoginDone(Err(db::DbError(
                                "Пользователь не найден".to_string(),
                            ))),
                            Err(e) => Message::LoginDone(Err(e)),
                        },
                    )
                } else {
                    self.screen = Screen::Login;
                    Task::none()
                }
            }
            Message::RegisterDone(Err(e)) => {
                if e.0.contains("duplicate") || e.0.contains("unique") {
                    self.form_error = "Имя уже занято".to_string();
                } else {
                    self.form_error = e.0.clone();
                }
                self.status = "Ошибка регистрации".to_string();
                Task::none()
            }

            // ── Параметры создания игры ────────────────────
            Message::GameBalanceIncrease => {
                self.create_game_balance += 100;
                if self.create_game_target_balance <= self.create_game_balance {
                    self.create_game_target_balance = self.create_game_balance + 100;
                }
                Task::none()
            }
            Message::GameBalanceDecrease => {
                if self.create_game_balance > 100 {
                    self.create_game_balance -= 100;
                }
                Task::none()
            }
            Message::GameMaxTurnsIncrease => {
                self.create_game_max_turns += 2;
                Task::none()
            }
            Message::GameMaxTurnsDecrease => {
                if self.create_game_max_turns > 2 {
                    self.create_game_max_turns -= 2;
                }
                Task::none()
            }
            Message::GameTargetBalanceIncrease => {
                self.create_game_target_balance += 100;
                Task::none()
            }
            Message::GameTargetBalanceDecrease => {
                let min = self.create_game_balance + 100;
                if self.create_game_target_balance > min {
                    self.create_game_target_balance -= 100;
                }
                Task::none()
            }
            Message::CreateGameSubmit => {
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => {
                        self.status = "Ты не залогинен".to_string();
                        return Task::none();
                    }
                };
                let seed = chrono::Utc::now().timestamp();
                if let Some(pool) = self.pool.clone() {
                    let (balance, turns, target) = (
                        self.create_game_balance,
                        self.create_game_max_turns,
                        self.create_game_target_balance,
                    );
                    self.status = "Создаю игру...".to_string();
                    Task::perform(
                        async move {
                            db::create_game(&pool, seed, balance, turns, target, user_id).await
                        },
                        Message::GameCreated,
                    )
                } else {
                    self.status = "БД не подключена".to_string();
                    Task::none()
                }
            }
            Message::GameCreated(Ok(game_id)) => {
                self.status = "Игра создана, загружаю...".to_string();
                self.active_game_id = Some(game_id);
                self.has_active_game = Some(true);
                if let Some((user_id, _)) = self.current_user.clone() {
                    if let Some(pool) = self.pool.clone() {
                        return Task::perform(
                            async move { db::load_game_screen(&pool, game_id, user_id).await },
                            Message::GameScreenLoaded,
                        );
                    }
                }
                Task::none()
            }
            Message::GameCreated(Err(e)) => {
                self.status = format!("Ошибка создания игры: {}", e.0);
                Task::none()
            }

            // ── Продолжить последнюю игру ──────────────────
            Message::ContinueGame => {
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => {
                        self.status = "Ты не залогинен".to_string();
                        return Task::none();
                    }
                };
                self.status = "Загружаю последнюю игру...".to_string();
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::get_latest_user_game(&pool, user_id).await },
                        |result| match result {
                            Ok(Some(game_id)) => Message::LoadGame(game_id),
                            Ok(None) => Message::BackToMenu,
                            Err(e) => Message::UserGamesLoaded(Err(e)),
                        },
                    )
                } else {
                    Task::none()
                }
            }

            // ── Загрузка игры  ──────────────────────────────
            Message::LoadGame(game_id) => {
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => {
                        self.status = "Ты не залогинен".to_string();
                        return Task::none();
                    }
                };
                self.active_game_id = Some(game_id);
                self.status = "Загружаю игру...".to_string();
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::load_game_screen(&pool, game_id, user_id).await },
                        Message::GameScreenLoaded,
                    )
                } else {
                    Task::none()
                }
            }
            Message::GameScreenLoaded(Ok((rules, state, cells, inventory, properties))) => {
                self.local_player_pos = state.position;
                self.turn_phase = TurnPhase::WaitingRoll;
                self.dice_roll = None;
                self.game_rules = Some(rules);
                self.player_state = Some(state);
                self.board_cells = cells;
                self.inventory = inventory;
                self.player_properties = properties;
                self.screen = Screen::Game;
                self.game_menu_open = false;
                self.status = "Игра загружена".to_string();
                self.bot_turn_queue.clear();
                self.bot_turn_names.clear();
                self.bot_turn_log.clear();
                self.bot_participants.clear();
                self.show_property_mgmt = false;
                self.selected_property = None;
                self.board_zoom = 1.0;
                self.board_pan_x = 0.0;
                self.board_pan_y = 0.0;
                Task::none()
            }
            Message::GameScreenLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки игры: {}", e.0);
                Task::none()
            }

            // ── Игровое меню ───────────────────────────────
            Message::ToggleGameMenu => {
                self.game_menu_open = !self.game_menu_open;
                Task::none()
            }
            Message::SaveAndExit => {
                self.game_menu_open = false;
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => {
                        self.screen = Screen::Menu;
                        return Task::none();
                    }
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => {
                        self.screen = Screen::Menu;
                        return Task::none();
                    }
                };
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::pause_game(&pool, game_id, user_id).await },
                        Message::GamePaused,
                    )
                } else {
                    self.screen = Screen::Menu;
                    Task::none()
                }
            }
            Message::GamePaused(Ok(())) => {
                self.screen = Screen::Menu;
                self.has_active_game = Some(true);
                self.status = "Игра сохранена".to_string();
                // Сбрасываем игровой стейт чтобы он не "протекал" в следующую сессию
                self.local_player_pos = 0;
                self.player_state = None;
                self.game_rules = None;
                self.board_cells.clear();
                self.inventory.clear();
                self.player_properties.clear();
                self.dice_roll = None;
                self.turn_phase = TurnPhase::WaitingRoll;
                self.cell_action = None;
                self.shop_slots.clear();
                self.tooltip_item = None;
                self.tooltip_visible = false;
                self.tooltip_locked = false;
                self.bot_participants.clear();
                self.bot_turn_queue.clear();
                self.bot_turn_names.clear();
                self.bot_turn_log.clear();
                self.show_property_mgmt = false;
                self.selected_property = None;
                self.game_menu_open = false;
                Task::none()
            }
            Message::GamePaused(Err(e)) => {
                self.status = format!("Ошибка сохранения: {}", e.0);
                Task::none()
            }
            Message::Surrender => {
                self.game_menu_open = false;
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => {
                        self.screen = Screen::Menu;
                        return Task::none();
                    }
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => {
                        self.screen = Screen::Menu;
                        return Task::none();
                    }
                };
                if let Some(pool) = self.pool.clone() {
                    let pool2 = pool.clone();
                    Task::perform(
                        async move {
                            db::surrender_game(&pool, game_id, user_id).await?;
                            db::get_game_result(&pool2, game_id, user_id).await
                        },
                        Message::GameResultLoaded,
                    )
                } else {
                    self.screen = Screen::Menu;
                    Task::none()
                }
            }
            Message::GameSurrendered(Ok(())) => Task::none(),
            Message::GameSurrendered(Err(e)) => {
                self.status = format!("Ошибка: {}", e.0);
                Task::none()
            }
            Message::GameResultLoaded(Ok(result)) => {
                self.game_result = result;
                if let Some((user_id, _)) = self.current_user.clone() {
                    if let Some(pool) = self.pool.clone() {
                        let _ = Task::perform(
                            async move { db::get_active_game_for_user(&pool, user_id).await },
                            Message::ActiveGameChecked,
                        );
                    }
                }
                self.screen = Screen::GameOver;
                Task::none()
            }
            Message::GameResultLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки результата: {}", e.0);
                self.screen = Screen::Menu;
                Task::none()
            }
            Message::TooltipHoverStart(item) => {
                self.tooltip_item = Some(item);
                self.tooltip_visible = true;
                Task::none()
            }
            Message::TooltipHoverEnd => {
                self.tooltip_item = None;
                self.tooltip_visible = false;
                Task::none()
            }
            Message::TooltipTick => Task::none(),
            Message::TooltipClose => {
                self.tooltip_item = None;
                self.tooltip_visible = false;
                Task::none()
            }
            Message::BoardZoom(delta) => {
                self.board_zoom = (self.board_zoom + delta * 0.001).clamp(0.3, 4.0);
                Task::none()
            }
            Message::BoardScrollTo(_) => Task::none(),
            Message::BoardScrolled(_) => Task::none(),
            Message::BoardDragStart => {
                self.board_dragging = true;
                Task::none()
            }
            Message::BoardDragEnd => {
                self.board_dragging = false;
                Task::none()
            }
            Message::BoardMouseMove(x, y) => {
                let (ox, oy) = self.board_mouse_pos;
                if self.board_dragging {
                    self.board_pan_x += ox - x;
                    self.board_pan_y += oy - y;
                }
                self.board_mouse_pos = (x, y);
                Task::none()
            }
            Message::CellHovered(idx) => {
                self.hovered_cell = idx;
                Task::none()
            }

            // ── Настройки ──────────────────────────────────
            Message::SetWindowMode(mode) => {
                self.window_mode = mode;
                let wmode = match mode {
                    WindowMode::Window => window::Mode::Windowed,
                    WindowMode::Fullscreen => window::Mode::Fullscreen,
                    WindowMode::Borderless => window::Mode::Fullscreen,
                };
                window::get_latest().then(move |id| {
                    if let Some(id) = id {
                        window::change_mode(id, wmode)
                    } else {
                        Task::none()
                    }
                })
            }

            // ── БД ───────────────────────────────────────
            Message::DbConnected(Ok(pool)) => {
                self.status = "Применяю миграции...".to_string();
                let pool_clone = pool.clone();
                self.pool = Some(pool);
                Task::perform(
                    async move { db::run_migrations(&pool_clone).await },
                    Message::MigrationsApplied,
                )
            }
            Message::DbConnected(Err(e)) => {
                self.status = format!("Ошибка подключения: {}", e.0);
                Task::none()
            }
            Message::MigrationsApplied(Ok(())) => {
                self.status = "Готово".to_string();
                if let Some(pool) = self.pool.clone() {
                    Task::perform(
                        async move { db::list_users(&pool).await },
                        Message::UsersLoaded,
                    )
                } else {
                    Task::none()
                }
            }
            Message::MigrationsApplied(Err(e)) => {
                self.status = format!("Ошибка миграций: {}", e.0);
                Task::none()
            }
            Message::UsersLoaded(Ok(users)) => {
                self.users = users;
                Task::none()
            }
            Message::UsersLoaded(Err(_)) => Task::none(),
            Message::GamesLoaded(Ok(games)) => {
                self.games = games;
                Task::none()
            }
            Message::GamesLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки игр: {}", e.0);
                Task::none()
            }
            Message::ActiveGameChecked(Ok(maybe_id)) => {
                self.has_active_game = Some(maybe_id.is_some());
                Task::none()
            }
            Message::ActiveGameChecked(Err(_)) => {
                self.has_active_game = Some(false);
                Task::none()
            }
            Message::UserGamesLoaded(Ok(games)) => {
                self.user_games = games;
                Task::none()
            }
            Message::UserGamesLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки игр: {}", e.0);
                Task::none()
            }
            Message::StatsLoaded(Ok(stats)) => {
                self.user_stats = Some(stats);
                self.show_stats = true;
                Task::none()
            }
            Message::StatsLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки статистики: {}", e.0);
                Task::none()
            }

            Message::WindowResized(w, h) => {
                self.window_width = w;
                self.window_height = h;
                Task::none()
            }
            Message::RollDice => {
                if self.turn_phase != TurnPhase::WaitingRoll {
                    return Task::none();
                }
                self.income_log.clear();
                self.start_log.clear();
                self.rent_received_log.clear();
                if let (Some(state), Some(rules)) = (&self.player_state, &self.game_rules) {
                    if state.moves_made >= rules.max_turns {
                        return Task::none();
                    }
                }
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos();
                let d1 = ((seed % 6) + 1) as u8;
                let d2 = (((seed / 7) % 6) + 1) as u8;
                self.dice_roll = Some((d1, d2));
                self.local_player_pos = (self.local_player_pos + (d1 + d2) as i32) % 40;
                self.turn_phase = TurnPhase::Rolled;
                self.shop_slots = Vec::new();

                // Определяем действие на новой клетке
                let new_pos = self.local_player_pos;
                let user_id = self.current_user.as_ref().map(|(id, _)| *id);
                let cell = self
                    .board_cells
                    .iter()
                    .find(|c| c.cell_index == new_pos)
                    .cloned();

                self.cell_action = match &cell {
                    Some(c) if c.cell_type == "property" => {
                        let cost = c.purchase_cost.unwrap_or(0);
                        let rent = c.rent_cost.unwrap_or(0);
                        if c.owner_user_id.is_none() {
                            Some(CellAction::CanBuy {
                                cell_index: new_pos,
                                cost,
                                name: c.prop_name.clone().unwrap_or_default(),
                            })
                        } else if c.owner_user_id == user_id {
                            Some(CellAction::Info("Ваша собственность".to_string()))
                        } else {
                            let owner = c.prop_name.clone().unwrap_or_default();
                            Some(CellAction::MustPayRent {
                                cell_index: new_pos,
                                rent,
                                owner,
                            })
                        }
                    }
                    Some(c) if c.cell_type == "tax" => Some(CellAction::Tax),
                    Some(c) if c.cell_type == "shop" => {
                        if let Some(shop_id) = c.shop_id {
                            Some(CellAction::Shop { shop_id })
                        } else {
                            Some(CellAction::Info("Магазин".to_string()))
                        }
                    }
                    _ => None,
                };

                // Если магазин — сразу загружаем слоты
                if let Some(CellAction::Shop { shop_id }) = &self.cell_action {
                    let shop_id = *shop_id;
                    let game_id = match self.active_game_id {
                        Some(id) => id,
                        None => return Task::none(),
                    };
                    let uid = match user_id {
                        Some(id) => id,
                        None => return Task::none(),
                    };
                    if let Some(pool) = self.pool.clone() {
                        return Task::perform(
                            async move { db::get_shop_slots(&pool, shop_id, game_id, uid).await },
                            Message::ShopSlotsLoaded,
                        );
                    }
                }

                Task::none()
            }
            Message::OpenShop(shop_id) => {
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::get_shop_slots(&pool, shop_id, game_id, user_id).await },
                    Message::ShopSlotsLoaded,
                )
            }
            Message::ShopSlotsLoaded(Ok(slots)) => {
                self.shop_slots = slots;
                Task::none()
            }
            Message::ShopSlotsLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки магазина: {}", e.0);
                Task::none()
            }
            Message::BuyShopSlot(slot_id) => {
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::buy_shop_slot(&pool, slot_id, game_id, user_id).await },
                    Message::ShopSlotBought,
                )
            }
            Message::ShopSlotBought(Ok(new_state)) => {
                self.player_state = Some(new_state);
                self.status = "Усиление куплено".to_string();
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                let shop_id = match &self.cell_action {
                    Some(CellAction::Shop { shop_id }) => *shop_id,
                    _ => return Task::none(),
                };
                let pool2 = pool.clone();
                Task::batch([
                    Task::perform(
                        async move { db::get_shop_slots(&pool, shop_id, game_id, user_id).await },
                        Message::ShopSlotsLoaded,
                    ),
                    Task::perform(
                        async move { db::get_player_inventory(&pool2, game_id, user_id).await },
                        Message::InventoryReloaded,
                    ),
                ])
            }
            Message::ShopSlotBought(Err(e)) => {
                if Self::is_game_over_error(&e) {
                    return self.force_exit_to_menu("Игра уже завершена");
                }
                self.status = format!("Ошибка покупки: {}", e.0);
                Task::none()
            }
            Message::InventoryReloaded(Ok(items)) => {
                self.inventory = items;
                Task::none()
            }
            Message::InventoryReloaded(Err(e)) => {
                self.status = format!("Ошибка обновления инвентаря: {}", e.0);
                Task::none()
            }
            Message::SellPowerUp(power_up_id) => {
                self.tooltip_visible = false;
                self.tooltip_locked = false;
                self.tooltip_item = None;
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::sell_power_up(&pool, game_id, user_id, power_up_id).await },
                    Message::PowerUpSold,
                )
            }
            Message::PowerUpSold(Ok(new_state)) => {
                self.player_state = Some(new_state);
                self.status = "Усиление продано".to_string();
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::get_player_inventory(&pool, game_id, user_id).await },
                    Message::InventoryReloaded,
                )
            }
            Message::PowerUpSold(Err(e)) => {
                if Self::is_game_over_error(&e) {
                    return self.force_exit_to_menu("Игра уже завершена");
                }
                self.status = format!("Ошибка продажи: {}", e.0);
                Task::none()
            }

            // ── Управление собственностью ───────────────────────
            Message::TogglePropertyMgmt => {
                self.show_property_mgmt = !self.show_property_mgmt;
                if !self.show_property_mgmt {
                    return Task::none();
                }
                // При открытии — загружаем свежий список собственностей
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::get_player_properties(&pool, game_id, user_id).await },
                    Message::PropertiesLoaded,
                )
            }
            Message::SelectProperty(idx) => {
                self.selected_property = Some(idx);
                Task::none()
            }
            Message::PropertiesLoaded(Ok(props)) => {
                self.player_properties = props;
                if self
                    .selected_property
                    .map(|i| i >= self.player_properties.len())
                    .unwrap_or(false)
                {
                    self.selected_property = None;
                }
                Task::none()
            }
            Message::PropertiesLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки собственностей: {}", e.0);
                Task::none()
            }
            Message::InstallUpgrade {
                property_id,
                power_up_id,
            } => {
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move {
                        db::install_upgrade(&pool, game_id, user_id, property_id, power_up_id).await
                    },
                    Message::UpgradeInstalled,
                )
            }
            Message::UpgradeInstalled(Ok(result)) => {
                if result == "ok" {
                    self.status = "Усиление установлено".to_string();
                } else {
                    self.status = format!("Нельзя установить: {}", result);
                }
                // Перезагружаем собственности и инвентарь
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                let pool2 = pool.clone();
                Task::batch([
                    Task::perform(
                        async move { db::get_player_properties(&pool, game_id, user_id).await },
                        Message::PropertiesLoaded,
                    ),
                    Task::perform(
                        async move { db::get_player_inventory(&pool2, game_id, user_id).await },
                        Message::InventoryReloaded,
                    ),
                ])
            }
            Message::UpgradeInstalled(Err(e)) => {
                self.status = format!("Ошибка установки: {}", e.0);
                Task::none()
            }
            Message::UninstallUpgrade {
                property_id,
                power_up_id,
            } => {
                self.tooltip_visible = false;
                self.tooltip_locked = false;
                self.tooltip_item = None;
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move {
                        db::uninstall_upgrade(&pool, game_id, user_id, property_id, power_up_id)
                            .await
                    },
                    Message::UpgradeUninstalled,
                )
            }
            Message::UpgradeUninstalled(Ok(result)) => {
                if result == "ok" {
                    self.status = "Усиление возвращено в инвентарь".to_string();
                } else {
                    self.status = format!("Нельзя извлечь: {}", result);
                }
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                let pool2 = pool.clone();
                Task::batch([
                    Task::perform(
                        async move { db::get_player_properties(&pool, game_id, user_id).await },
                        Message::PropertiesLoaded,
                    ),
                    Task::perform(
                        async move { db::get_player_inventory(&pool2, game_id, user_id).await },
                        Message::InventoryReloaded,
                    ),
                ])
            }
            Message::UpgradeUninstalled(Err(e)) => {
                self.status = format!("Ошибка извлечения: {}", e.0);
                Task::none()
            }
            Message::RerollShop(shop_id) => {
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::reroll_shop(&pool, shop_id, game_id, user_id).await },
                    Message::ShopRerolled,
                )
            }
            Message::ShopRerolled(Ok(new_state)) => {
                self.player_state = Some(new_state);
                self.status = "Магазин обновлён".to_string();
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                let shop_id = match &self.cell_action {
                    Some(CellAction::Shop { shop_id }) => *shop_id,
                    _ => return Task::none(),
                };
                Task::perform(
                    async move { db::get_shop_slots(&pool, shop_id, game_id, user_id).await },
                    Message::ShopSlotsLoaded,
                )
            }
            Message::ShopRerolled(Err(e)) => {
                if Self::is_game_over_error(&e) {
                    return self.force_exit_to_menu("Игра уже завершена");
                }
                self.status = format!("Ошибка реролла: {}", e.0);
                Task::none()
            }
            Message::EndTurn => {
                if self.turn_phase != TurnPhase::Rolled {
                    return Task::none();
                }
                self.shop_slots = Vec::new();
                self.tooltip_item = None;
                self.tooltip_visible = false;
                self.tooltip_locked = false;

                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };

                // Оплата аренды
                if let Some(CellAction::MustPayRent { cell_index, .. }) = &self.cell_action.clone()
                {
                    let ci = *cell_index;
                    self.cell_action = None;
                    return Task::perform(
                        async move { db::pay_rent(&pool, game_id, user_id, ci).await },
                        Message::RentPaid,
                    );
                }

                // Оплата налога
                if let Some(CellAction::Tax) = &self.cell_action {
                    self.cell_action = None;
                    return Task::perform(
                        async move { db::pay_tax(&pool, game_id, user_id).await },
                        Message::TaxPaid,
                    );
                }

                self.cell_action = None;
                let new_pos = self.local_player_pos;
                Task::perform(
                    async move { db::commit_player_move(&pool, game_id, user_id, new_pos).await },
                    Message::TurnSaved,
                )
            }
            Message::TurnSaved(Ok(new_state)) => {
                let old_balance = self.player_state.as_ref().map(|s| s.balance).unwrap_or(0);
                self.player_state = Some(new_state.clone());
                self.turn_phase = TurnPhase::WaitingRoll;
                self.dice_roll = None;

                let balance_diff = new_state.balance - old_balance;
                self.status = if balance_diff >= 400 {
                    "Ход завершён. Вы встали на СТАРТ! +400".to_string()
                } else if balance_diff >= 200 {
                    "Ход завершён. Вы прошли СТАРТ! +200".to_string()
                } else {
                    "Ход завершён. Боты ходят...".to_string()
                };

                self.bot_turn_log.clear();
                self.start_log.clear();
                self.rent_received_log.clear();
                // income_log не чистим — там уже лежит аренда/налог этого хода
                // Бонус СТАРТ фиксированный: 400 за приземление, 200 за прохождение.
                // balance_diff ненадёжен если до этого была аренда/налог — проверяем позицию.
                let start_bonus = if new_state.position == 0 && old_balance > 0 {
                    400i64
                } else if old_balance > 0 && new_state.position < self.local_player_pos {
                    200i64
                } else {
                    0i64
                };
                if start_bonus == 400 {
                    self.start_log
                        .push(format!("Встали на СТАРТ: +{}", start_bonus));
                } else if start_bonus == 200 {
                    self.start_log
                        .push(format!("Прошли СТАРТ: +{}", start_bonus));
                }

                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };

                // Загружаем список живых ботов и запускаем их ходы
                if let Some(pool) = self.pool.clone() {
                    return Task::perform(
                        async move { db::get_bot_participants(&pool, game_id).await },
                        Message::BotsLoaded,
                    );
                }
                Task::none()
            }

            // ── Ходы ботов ─────────────────────────────────
            Message::BotsLoaded(Ok(bots)) => {
                self.bot_participants = bots.clone();
                // Заполняем очереди: только живые боты по turn_order
                self.bot_turn_queue = bots.iter().map(|b| b.user_id).collect();
                self.bot_turn_names = bots.iter().map(|b| b.username.clone()).collect();

                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };

                // Запускаем ход первого бота из очереди
                if !self.bot_turn_queue.is_empty() {
                    let bot_id = self.bot_turn_queue.remove(0);
                    let _bot_name = if !self.bot_turn_names.is_empty() {
                        self.bot_turn_names.remove(0)
                    } else {
                        "Бот".to_string()
                    };
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos();
                    let d1 = ((seed % 6) + 1) as i32;
                    let d2 = (((seed / 7) % 6) + 1) as i32;
                    let dice = d1 + d2;
                    // Сохраняем имя текущего бота для лога
                    self.status = format!("Ходит {}... (кубик: {}+{}={})", _bot_name, d1, d2, dice);
                    // Временно сохраняем имя в очередь имён для BotTurnDone (как первый элемент)
                    self.bot_turn_names.insert(0, _bot_name);
                    return Task::perform(
                        async move { db::do_bot_turn(&pool, game_id, bot_id, dice).await },
                        Message::BotTurnDone,
                    );
                }

                // Ботов нет — сразу проверяем балансы
                let pool2 = pool.clone();
                Task::batch([
                    Task::perform(
                        async move { db::get_all_balances(&pool, game_id).await },
                        Message::BalancesLoaded,
                    ),
                    Task::perform(
                        async move { db::get_bot_participants(&pool2, game_id).await },
                        Message::BotsRefreshed,
                    ),
                ])
            }
            Message::BotsLoaded(Err(e)) => {
                self.status = format!("Ошибка загрузки ботов: {}", e.0);
                // Всё равно проверяем балансы
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                if let Some(pool) = self.pool.clone() {
                    return Task::perform(
                        async move { db::get_all_balances(&pool, game_id).await },
                        Message::BalancesLoaded,
                    );
                }
                Task::none()
            }
            Message::BotsRefreshed(Ok(bots)) => {
                self.bot_participants = bots;
                Task::none()
            }
            Message::BotsRefreshed(Err(_)) => Task::none(),
            Message::PlayerStateRefreshed(Ok(Some(state))) => {
                self.player_state = Some(state);
                Task::none()
            }
            Message::PlayerStateRefreshed(Ok(None)) => Task::none(),
            Message::PlayerStateRefreshed(Err(_)) => Task::none(),
            Message::BotTurnDone(Ok(result)) => {
                // Извлекаем имя текущего бота из головы очереди имён
                let bot_name = if !self.bot_turn_names.is_empty() {
                    self.bot_turn_names.remove(0)
                } else {
                    "Бот".to_string()
                };

                let action_str = match result.action.as_str() {
                    "bought" => format!("купил «{}»", result.action_detail),
                    "rent_paid" => {
                        let parts: Vec<&str> = result.action_detail.splitn(2, ':').collect();
                        let cell_name = parts.first().copied().unwrap_or("?");
                        let amount = parts.get(1).copied().unwrap_or("?");
                        // Если владелец клетки — игрок, фиксируем доход
                        if let Some((user_id, _)) = &self.current_user {
                            let is_player_owner = self.board_cells.iter().any(|c| {
                                c.prop_name.as_deref() == Some(cell_name)
                                    && c.owner_user_id == Some(*user_id)
                            });
                            if is_player_owner {
                                self.rent_received_log
                                    .push(format!("Аренда от {}: +{}", bot_name, amount));
                            }
                        }
                        format!("заплатил аренду {} за «{}»", amount, cell_name)
                    }
                    "tax_paid" => format!("заплатил налог {}", result.action_detail),
                    "bankrupt" => format!("💀 БАНКРОТ ({})", result.action_detail),
                    "start_bonus" => format!("прошёл СТАРТ +{}", result.action_detail),
                    _ => "пропустил ход".to_string(),
                };

                let log_line = format!("{}: {}", bot_name, action_str);
                self.bot_turn_log.push(log_line);
                if self.bot_turn_log.len() > 10 {
                    self.bot_turn_log.remove(0);
                }

                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };

                // Следующий бот в очереди?
                if !self.bot_turn_queue.is_empty() {
                    let bot_id = self.bot_turn_queue.remove(0);
                    let next_name = if !self.bot_turn_names.is_empty() {
                        self.bot_turn_names.remove(0)
                    } else {
                        "Бот".to_string()
                    };
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos();
                    let d1 = ((seed % 6) + 1) as i32;
                    let d2 = (((seed / 7) % 6) + 1) as i32;
                    let dice = d1 + d2;
                    self.status = format!("Ходит {}... ({}+{}={})", next_name, d1, d2, dice);
                    self.bot_turn_names.insert(0, next_name);
                    return Task::perform(
                        async move { db::do_bot_turn(&pool, game_id, bot_id, dice).await },
                        Message::BotTurnDone,
                    );
                }

                // Все боты походили — обновляем поле, позиции ботов и проверяем балансы
                self.status = "Ходы ботов завершены".to_string();
                let pool3 = pool.clone();
                let pool4 = pool.clone();
                let uid_for_refresh = self.current_user.as_ref().map(|(id, _)| *id);
                Task::batch([
                    Task::perform(
                        {
                            let pool2 = pool.clone();
                            async move { db::get_all_balances(&pool2, game_id).await }
                        },
                        Message::BalancesLoaded,
                    ),
                    Task::perform(
                        async move { db::get_board_cells(&pool, game_id).await },
                        Message::BoardReloaded,
                    ),
                    Task::perform(
                        async move { db::get_bot_participants(&pool3, game_id).await },
                        Message::BotsRefreshed,
                    ),
                    Task::perform(
                        async move {
                            if let Some(uid) = uid_for_refresh {
                                db::get_participant_state(&pool4, game_id, uid).await
                            } else {
                                Ok(None)
                            }
                        },
                        Message::PlayerStateRefreshed,
                    ),
                ])
            }
            Message::BotTurnDone(Err(e)) => {
                self.status = format!("Ошибка хода бота: {}", e.0);
                // Пропускаем имя текущего бота
                if !self.bot_turn_names.is_empty() {
                    self.bot_turn_names.remove(0);
                }
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                if !self.bot_turn_queue.is_empty() {
                    let bot_id = self.bot_turn_queue.remove(0);
                    let next_name = if !self.bot_turn_names.is_empty() {
                        self.bot_turn_names.remove(0)
                    } else {
                        "Бот".to_string()
                    };
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos()
                        .wrapping_add(13);
                    let d1 = ((seed % 6) + 1) as i32;
                    let d2 = (((seed / 7) % 6) + 1) as i32;
                    let dice = d1 + d2;
                    self.bot_turn_names.insert(0, next_name);
                    return Task::perform(
                        async move { db::do_bot_turn(&pool, game_id, bot_id, dice).await },
                        Message::BotTurnDone,
                    );
                }
                Task::perform(
                    async move { db::get_all_balances(&pool, game_id).await },
                    Message::BalancesLoaded,
                )
            }
            Message::TurnSaved(Err(e)) => {
                if Self::is_game_over_error(&e) {
                    return self.force_exit_to_menu("Игра уже завершена");
                }
                self.status = format!("Ошибка сохранения хода: {}", e.0);
                Task::none()
            }
            Message::BalancesLoaded(Ok(balances)) => {
                let rules = match &self.game_rules {
                    Some(r) => r.clone(),
                    None => return Task::none(),
                };
                let state = match &self.player_state {
                    Some(s) => s.clone(),
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };

                let moves_left = (rules.max_turns - state.moves_made).max(0);

                // Считаем не-банкротов среди ботов
                let bots_alive = balances
                    .iter()
                    .filter(|(id, balance, utype)| *id != user_id && utype == "bot" && *balance > 0)
                    .count();

                let player_bankrupt = state.balance <= 0;

                // Победа 1: баланс достиг цели
                // Победа 2: все боты банкроты, игрок жив
                let victory =
                    state.balance >= rules.target_balance || (!player_bankrupt && bots_alive == 0);

                // Поражение 1: ходы кончились, цель не достигнута
                // Поражение 2: игрок банкрот
                let defeat = (!victory && moves_left <= 0) || player_bankrupt;

                if victory || defeat {
                    let status_str = if victory {
                        "Выиграна"
                    } else {
                        "Проиграна"
                    };
                    self.status = status_str.to_string();

                    if let (Some(game_id), Some(pool)) = (self.active_game_id, self.pool.clone()) {
                        return Task::perform(
                            async move {
                                db::set_game_status(&pool, game_id, "finished").await?;
                                db::get_game_result(&pool, game_id, user_id).await
                            },
                            Message::GameResultLoaded,
                        );
                    }
                }

                Task::none()
            }
            Message::BalancesLoaded(Err(e)) => {
                self.status = format!("Ошибка проверки балансов: {}", e.0);
                Task::none()
            }
            Message::BuyProperty => {
                let action = match &self.cell_action {
                    Some(CellAction::CanBuy { cell_index, .. }) => *cell_index,
                    _ => return Task::none(),
                };
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                self.cell_action = None;
                Task::perform(
                    async move { db::buy_property(&pool, game_id, user_id, action).await },
                    Message::PropertyBought,
                )
            }
            Message::SkipAction => {
                // Если это аренда которую нельзя пропустить — всё равно платим при EndTurn
                // Если можно пропустить (не купить) — просто сбрасываем
                self.cell_action = None;
                self.shop_slots.clear();
                self.tooltip_item = None;
                self.tooltip_visible = false;
                self.tooltip_locked = false;
                Task::none()
            }
            Message::PropertyBought(Ok(new_state)) => {
                self.player_state = Some(new_state);
                self.status = "Собственность куплена".to_string();
                // Перезагружаем клетки чтобы обновить владельца
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::get_board_cells(&pool, game_id).await },
                    Message::BoardReloaded,
                )
            }
            Message::PropertyBought(Err(e)) => {
                if Self::is_game_over_error(&e) {
                    return self.force_exit_to_menu("Игра уже завершена");
                }
                self.status = format!("Ошибка покупки: {}", e.0);
                Task::none()
            }
            Message::RentPaid(Ok(new_state)) => {
                if let (Some(old),) = (self.player_state.as_ref(),) {
                    let diff = old.balance - new_state.balance;
                    if diff > 0 {
                        self.income_log.push(format!("Аренда: -{}", diff));
                    }
                }
                self.player_state = Some(new_state);
                self.status = "Аренда уплачена, завершаю ход...".to_string();
                self.cell_action = None;
                // Сразу фиксируем ход — не требуем повторного нажатия
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                let new_pos = self.local_player_pos;
                Task::perform(
                    async move { db::commit_player_move(&pool, game_id, user_id, new_pos).await },
                    Message::TurnSaved,
                )
            }
            Message::RentPaid(Err(e)) => {
                if Self::is_game_over_error(&e) {
                    return self.force_exit_to_menu("Игра уже завершена");
                }
                self.status = format!("Ошибка аренды: {}", e.0);
                Task::none()
            }
            Message::TaxPaid(Ok(new_state)) => {
                if let Some(old) = self.player_state.as_ref() {
                    let diff = old.balance - new_state.balance;
                    if diff > 0 {
                        self.income_log.push(format!("Налог: -{}", diff));
                    }
                }
                self.player_state = Some(new_state);
                self.status = "Налог уплачен, завершаю ход...".to_string();
                self.cell_action = None;
                // Сразу фиксируем ход — не требуем повторного нажатия
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                let new_pos = self.local_player_pos;
                Task::perform(
                    async move { db::commit_player_move(&pool, game_id, user_id, new_pos).await },
                    Message::TurnSaved,
                )
            }
            Message::TaxPaid(Err(e)) => {
                if Self::is_game_over_error(&e) {
                    return self.force_exit_to_menu("Игра уже завершена");
                }
                self.status = format!("Ошибка налога: {}", e.0);
                Task::none()
            }
            Message::BoardReloaded(Ok(cells)) => {
                self.board_cells = cells;
                // Перезагружаем собственности, чтобы SVG-уровень отражал актуальные усиления
                let game_id = match self.active_game_id {
                    Some(id) => id,
                    None => return Task::none(),
                };
                let (user_id, _) = match &self.current_user {
                    Some(u) => u.clone(),
                    None => return Task::none(),
                };
                let pool = match self.pool.clone() {
                    Some(p) => p,
                    None => return Task::none(),
                };
                Task::perform(
                    async move { db::get_player_properties(&pool, game_id, user_id).await },
                    Message::PropertiesLoaded,
                )
            }
            Message::BoardReloaded(Err(_)) => Task::none(),
            Message::StaleGamesReset => Task::none(),
        }
    }

    fn is_game_over_error(e: &db::DbError) -> bool {
        e.0.contains("GAME_OVER")
    }

    fn force_exit_to_menu(&mut self, reason: &str) -> Task<Message> {
        self.status = reason.to_string();
        self.screen = Screen::Menu;
        self.active_game_id = None;
        self.player_state = None;
        self.game_rules = None;
        self.board_cells.clear();
        self.inventory.clear();
        self.player_properties.clear();
        self.dice_roll = None;
        self.turn_phase = TurnPhase::WaitingRoll;
        self.cell_action = None;
        self.shop_slots.clear();
        self.tooltip_item = None;
        self.tooltip_visible = false;
        self.tooltip_locked = false;
        self.bot_participants.clear();
        self.bot_turn_queue.clear();
        self.bot_turn_names.clear();
        self.bot_turn_log.clear();
        self.income_log.clear();
        self.start_log.clear();
        self.rent_received_log.clear();
        self.show_property_mgmt = false;
        self.selected_property = None;
        self.game_menu_open = false;
        Task::none()
    }

    fn clear_form(&mut self) {
        self.input_username = String::new();
        self.input_password = String::new();
        self.input_password2 = String::new();
        self.form_error = String::new();
    }
}

// ─────────────────────────────────────────────
// View
// ─────────────────────────────────────────────
impl DreamBreaker {
    fn scale(&self) -> f32 {
        let sx = self.window_width / 1024.0;
        let sy = self.window_height / 768.0;
        sx.min(sy).max(0.5)
    }

    fn s(&self, base: f32) -> f32 {
        base * self.scale()
    }

    fn ts(&self, base: u16) -> f32 {
        (base as f32 * self.scale()).max(8.0)
    }

    fn view(&self) -> Element<'_, Message> {
        let in_game = self.screen == Screen::Game;
        let body = match self.screen {
            Screen::Menu => self.view_menu(),
            Screen::Login => self.view_login(),
            Screen::Register => self.view_register(),
            Screen::LoadGame => self.view_load_game(),
            Screen::CreateGame => self.view_create_game(),
            Screen::Game => self.view_game(),
            Screen::GameOver => self.view_game_over(),
            Screen::Settings => self.view_settings(),
        };

        let centered: Element<'_, Message> = if in_game {
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.screen == Screen::LoadGame {
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .align_y(iced::alignment::Vertical::Top)
                .into()
        } else {
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        };

        let status_bar = container(text(format!("Статус: {}", self.status)).size(self.ts(13)))
            .padding(self.s(4.0) as u16)
            .width(Length::Fill);

        let header_layer = if in_game {
            self.view_game_header()
        } else {
            self.view_header()
        };

        let base: Element<'_, Message> = column![
            container(Space::new(Length::Fill, Length::Fixed(self.s(40.0)))).width(Length::Fill),
            centered,
            status_bar,
        ]
        .into();

        let with_header: Element<'_, Message> = stack![base, header_layer].into();

        let with_dropdown: Element<'_, Message> = if self.profile_menu_open && !in_game {
            let dropdown = self.view_profile_dropdown();
            stack![with_header, dropdown].into()
        } else {
            with_header
        };

        let with_game_menu: Element<'_, Message> = if self.game_menu_open && in_game {
            let overlay = self.view_game_menu_overlay();
            stack![with_dropdown, overlay].into()
        } else {
            with_dropdown
        };

        let with_stats = if self.show_stats {
            let overlay = self.view_stats_overlay();
            stack![with_game_menu, overlay].into()
        } else {
            with_game_menu
        };

        let shop_open = matches!(&self.cell_action, Some(CellAction::Shop { .. }))
            && !self.shop_slots.is_empty();

        let with_shop = if shop_open {
            let overlay = self.view_shop_overlay();
            stack![with_stats, overlay].into()
        } else {
            with_stats
        };

        with_shop
    }

    fn view_header(&self) -> Element<'_, Message> {
        let label = match &self.current_user {
            Some((_, name)) => name.clone(),
            None => "Гость".to_string(),
        };

        let profile_btn = button(text(label.clone()).size(self.ts(16)))
            .on_press(Message::ToggleProfileMenu)
            .padding(self.s(8.0) as u16);

        // Кнопка «Статистика» — всегда справа от профиля, активна только для залогиненных
        let stats_btn: Element<'_, Message> = if self.current_user.is_some() {
            button(text("Статистика").size(self.ts(16)))
                .on_press(Message::OpenStats)
                .padding(self.s(8.0) as u16)
                .into()
        } else {
            button(text("Статистика").size(self.ts(16)))
                .padding(self.s(8.0) as u16)
                .into()
        };

        container(row![profile_btn, stats_btn].spacing(self.s(4.0) as u16))
            .padding(self.s(8.0) as u16)
            .width(Length::Fill)
            .into()
    }

    fn view_profile_dropdown(&self) -> Element<'_, Message> {
        let sc = self.scale();
        let mut menu = column![]
            .spacing(self.s(4.0) as u16)
            .padding(self.s(4.0) as u16);

        if self.current_user.is_some() {
            // Залогинен: показываем других пользователей и кнопку «Выйти»
            for (id, name) in &self.users {
                if self.current_user.as_ref().map(|(uid, _)| uid) != Some(id) {
                    menu = menu.push(profile_dropdown_btn(
                        name,
                        Message::SelectUser(*id, name.clone()),
                        sc,
                    ));
                }
            }
            menu = menu.push(profile_dropdown_btn("Выйти", Message::Logout, sc));
        } else {
            // Не залогинен: показываем доступные аккаунты (если есть)
            for (id, name) in &self.users {
                menu = menu.push(profile_dropdown_btn(
                    name,
                    Message::SelectUser(*id, name.clone()),
                    sc,
                ));
            }
            menu = menu.push(profile_dropdown_btn(
                "+ Создать аккаунт",
                Message::OpenRegister,
                sc,
            ));
        }

        // Высота одной кнопки ~(15*sc + 8*sc*2 + spacing) ≈ 47px при sc=1
        // Показываем максимум 5 пунктов без прокрутки
        let btn_h = self.s(15.0) + self.s(8.0) * 2.0 + self.s(4.0); // текст + padding + spacing
        let visible_items = 5usize;
        let total_items = self.users.len() + 1; // +1 за «Выйти» / «Создать»
        let max_h = btn_h * visible_items as f32 + self.s(8.0);
        let needs_scroll = total_items > visible_items;

        let menu_inner: Element<'_, Message> = if needs_scroll {
            scrollable(menu).height(Length::Fixed(max_h)).into()
        } else {
            menu.into()
        };

        container(
            container(menu_inner)
                .style(|_theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.08, 0.08, 0.12, 0.97))),
                    border: iced::Border {
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .padding(self.s(4.0) as u16),
        )
        .padding(iced::Padding {
            top: self.s(44.0),
            right: 0.0,
            bottom: 0.0,
            left: self.s(8.0),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_stats_overlay(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = if let Some(stats) = &self.user_stats {
            column![
                text("Статистика профиля").size(self.ts(20)),
                Space::with_height(self.s(8.0)),
                text(format!("Игр сыграно:            {}", stats.total_games)).size(self.ts(15)),
                text(format!("Побед:                  {}", stats.total_wins)).size(self.ts(15)),
                text(format!(
                    "Серия побед:            {}",
                    stats.current_win_streak
                ))
                .size(self.ts(15)),
                text(format!("Всего ходов:            {}", stats.total_moves)).size(self.ts(15)),
                text(format!("Всего заработано:       {}", stats.total_earned)).size(self.ts(15)),
                text(format!("Всего потрачено:        {}", stats.total_spent)).size(self.ts(15)),
                text(format!(
                    "Куплено собственностей: {}",
                    stats.properties_bought
                ))
                .size(self.ts(15)),
                text(format!(
                    "Куплено усилений:       {}",
                    stats.power_ups_bought
                ))
                .size(self.ts(15)),
                Space::with_height(self.s(12.0)),
                button(text("Закрыть").size(self.ts(16)))
                    .on_press(Message::ToggleStats)
                    .padding(self.s(8.0) as u16),
            ]
            .spacing(self.s(4.0) as u16)
            .padding(self.s(20.0) as u16)
            .into()
        } else {
            column![
                text("Загрузка статистики...").size(self.ts(18)),
                Space::with_height(self.s(12.0)),
                button(text("Закрыть").size(self.ts(16)))
                    .on_press(Message::ToggleStats)
                    .padding(self.s(8.0) as u16),
            ]
            .spacing(self.s(4.0) as u16)
            .padding(self.s(20.0) as u16)
            .into()
        };

        let card = container(content)
            .width(Length::Fixed(self.s(380.0)))
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.08, 0.08, 0.12, 0.97))),
                border: iced::Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.18),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

        let blocker = mouse_area(
            container(
                container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.62))),
                ..Default::default()
            }),
        )
        .on_press(Message::ToggleStats);

        blocker.into()
    }

    fn view_game_header(&self) -> Element<'_, Message> {
        container(
            button(text("Меню").size(self.ts(16)))
                .on_press(Message::ToggleGameMenu)
                .padding(self.s(8.0) as u16),
        )
        .padding(self.s(8.0) as u16)
        .width(Length::Fill)
        .into()
    }

    fn view_game_menu_overlay(&self) -> Element<'_, Message> {
        let sc = self.scale();

        let menu_items = column![
            menu_button("Сохранить и выйти", Message::SaveAndExit, sc),
            menu_button("Настройки", Message::OpenSettingsFromGame, sc),
            menu_button("Сдаться", Message::Surrender, sc),
        ]
        .spacing(self.s(8.0) as u16)
        .padding(self.s(16.0) as u16)
        .align_x(Alignment::Center);

        let card = container(menu_items).style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.08, 0.08, 0.12, 0.97))),
            border: iced::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.18),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        });

        mouse_area(
            container(
                container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.60))),
                ..Default::default()
            }),
        )
        .on_press(Message::ToggleGameMenu)
        .into()
    }
    fn view_shop_overlay(&self) -> Element<'_, Message> {
        let (shop_id, reroll_count) = match &self.cell_action {
            Some(CellAction::Shop { shop_id }) => {
                let rc = self.shop_slots.first().map(|s| s.reroll_count).unwrap_or(0);
                (*shop_id, rc)
            }
            _ => return Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into(),
        };
        let reroll_cost = 50 + 15 * reroll_count as i64;
        let sc = self.scale();

        // Сетка слотов 2×2
        let mut slots_grid = column![].spacing(self.s(8.0) as u16);
        let visible: Vec<&db::ShopSlot> = self
            .shop_slots
            .iter()
            .filter(|s| s.status != "rerolled")
            .collect();

        for chunk in visible.chunks(2) {
            let mut slot_row = row![].spacing(self.s(8.0) as u16);
            for slot in chunk {
                let slot_id = slot.slot_id;
                let inv_full = self.inventory.iter().map(|i| i.quantity).sum::<i32>() >= 5;

                let buy_btn: Element<'_, Message> = if slot.status == "sold" {
                    container(text("Куплено").size(self.ts(12)))
                        .width(Length::Fixed(self.s(160.0)))
                        .height(Length::Fixed(self.s(28.0)))
                        .align_x(iced::alignment::Horizontal::Center)
                        .align_y(iced::alignment::Vertical::Center)
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                0.2, 0.2, 0.2, 0.6,
                            ))),
                            ..Default::default()
                        })
                        .into()
                } else if slot.already_own {
                    container(text("Уже есть").size(self.ts(12)))
                        .width(Length::Fixed(self.s(160.0)))
                        .height(Length::Fixed(self.s(28.0)))
                        .align_x(iced::alignment::Horizontal::Center)
                        .align_y(iced::alignment::Vertical::Center)
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                0.2, 0.2, 0.2, 0.6,
                            ))),
                            ..Default::default()
                        })
                        .into()
                } else if inv_full {
                    container(text("Нет места").size(self.ts(12)))
                        .width(Length::Fixed(self.s(160.0)))
                        .height(Length::Fixed(self.s(28.0)))
                        .align_x(iced::alignment::Horizontal::Center)
                        .align_y(iced::alignment::Vertical::Center)
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                0.2, 0.2, 0.2, 0.6,
                            ))),
                            ..Default::default()
                        })
                        .into()
                } else {
                    button(text(format!("Купить {}", slot.cost)).size(self.ts(13)))
                        .on_press(Message::BuyShopSlot(slot_id))
                        .width(Length::Fixed(self.s(160.0)))
                        .padding(self.s(5.0) as u16)
                        .into()
                };

                let slot_card: Element<'_, Message> = container(
                    column![
                        text(slot.name.clone()).size(self.ts(13)),
                        Space::with_height(self.s(6.0)),
                        buy_btn,
                    ]
                    .spacing(self.s(2.0) as u16)
                    .padding(self.s(10.0) as u16)
                    .align_x(Alignment::Center)
                    .width(Length::Fixed(self.s(180.0))),
                )
                .width(Length::Fixed(self.s(180.0)))
                .height(Length::Fixed(self.s(100.0)))
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.12, 0.12, 0.22, 1.0))),
                    border: iced::Border {
                        color: Color::from_rgba(0.4, 0.4, 0.8, 0.7),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                })
                .into();
                let tip_item = TooltipItem {
                    name: slot.name.clone(),
                    description: slot.description.clone(),
                    effect_type: String::new(),
                    effect_value: 0,
                    buy_cost: Some(slot.cost),
                    sell_cost: Some(slot.cost / 2),
                };
                let tip_item_end = tip_item.clone();
                let slot_with_hover: Element<'_, Message> = mouse_area(slot_card)
                    .on_enter(Message::TooltipHoverStart(tip_item))
                    .on_exit(Message::TooltipHoverEnd)
                    .into();
                slot_row = slot_row.push(slot_with_hover);
            }
            slots_grid = slots_grid.push(slot_row);
        }

        let content = column![
            text("Магазин усилений").size(self.ts(20)),
            Space::with_height(self.s(12.0)),
            slots_grid,
            Space::with_height(self.s(12.0)),
            button(text(format!("Обновить ассортимент ({})", reroll_cost)).size(self.ts(13)))
                .on_press(Message::RerollShop(shop_id))
                .padding(self.s(8.0) as u16)
                .width(Length::Fixed(self.s(380.0))),
            Space::with_height(self.s(8.0)),
            button(text("Выйти из магазина").size(self.ts(13)))
                .on_press(Message::SkipAction)
                .padding(self.s(8.0) as u16)
                .width(Length::Fixed(self.s(380.0))),
        ]
        .spacing(self.s(4.0) as u16)
        .padding(self.s(20.0) as u16)
        .align_x(Alignment::Center);

        let card = container(content)
            .width(Length::Fixed(self.s(420.0)))
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.05, 0.05, 0.15, 0.98))),
                border: iced::Border {
                    color: Color::from_rgba(0.4, 0.4, 0.9, 0.8),
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            });

        let left_w = self.s(310.0) + self.s(16.0); // ширина левой панели с padding
        let right_w = self.s(200.0) + self.s(16.0); // ширина правой панели с padding

        container(
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            left: left_w,
            right: right_w,
            top: 0.0,
            bottom: 0.0,
        })
        .style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
            ..Default::default()
        })
        .into()
    }
    fn view_tooltip_overlay(&self) -> Element<'_, Message> {
        let item = match &self.tooltip_item {
            Some(i) => i,
            None => return Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into(),
        };

        let mut col = column![
            text(&item.name).size(self.ts(16)),
            Space::with_height(self.s(6.0)),
            text(&item.description).size(self.ts(12)),
        ]
        .spacing(self.s(2.0) as u16)
        .padding(self.s(14.0) as u16);

        if !item.effect_type.is_empty() {
            let effect_str = match item.effect_type.as_str() {
                "flat_base" => format!("+{} к базовой аренде", item.effect_value),
                "percent_bonus" => format!("+{}% к аренде", item.effect_value),
                "flat_final" => format!("+{} к итоговой аренде", item.effect_value),
                other => other.to_string(),
            };
            col = col.push(Space::with_height(self.s(4.0)));
            col = col.push(text(format!("Эффект: {}", effect_str)).size(self.ts(12)));
        }

        if let Some(buy) = item.buy_cost {
            col = col.push(text(format!("Цена покупки: {}", buy)).size(self.ts(12)));
        }
        if let Some(sell) = item.sell_cost {
            col = col.push(text(format!("Цена продажи: {}", sell)).size(self.ts(12)));
        }

        let card = container(col)
            .width(Length::Fixed(self.s(280.0)))
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.05, 0.05, 0.18, 0.93))),
                border: iced::Border {
                    color: Color::from_rgba(0.5, 0.5, 1.0, 0.7),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

        // Позиционируем по центру экрана, чуть выше середины
        mouse_area(
            container(
                container(card)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.0))),
                ..Default::default()
            }),
        )
        .on_exit(Message::TooltipClose)
        .into()
    }
    fn view_menu(&self) -> Element<'_, Message> {
        let sc = self.scale();
        let has_games = matches!(self.has_active_game, Some(true));

        let continue_btn: Element<'_, Message> = if has_games {
            icon_button(Icon::Play, "Продолжить игру", Message::ContinueGame, sc).into()
        } else {
            button(
                row![
                    icon(Icon::Play, 24.0 * sc),
                    text("Продолжить игру").size(self.ts(20))
                ]
                .spacing(self.s(10.0) as u16)
                .align_y(Alignment::Center),
            )
            .width(Length::Fixed(self.s(280.0)))
            .padding(self.s(12.0) as u16)
            .into()
        };

        let mut menu_col = column![
            text("DreamBreaker").size(self.ts(64)),
            Space::with_height(self.s(8.0)),
            continue_btn,
            icon_button(Icon::Play, "Начать игру", Message::OpenCreateGame, sc),
            Space::with_height(self.s(16.0)),
            icon_button(Icon::Save, "Загрузить игру", Message::OpenLoadGame, sc),
            icon_button(Icon::Settings, "Настройки", Message::OpenSettings, sc),
            icon_button(Icon::Close, "Выход", Message::Exit, sc),
        ]
        .spacing(self.s(8.0) as u16)
        .align_x(Alignment::Center);

        menu_col.into()
    }

    fn view_login(&self) -> Element<'_, Message> {
        let sc = self.scale();
        let mut col = column![
            text("Вход в аккаунт").size(self.ts(36)),
            Space::with_height(self.s(12.0)),
            text("Имя пользователя").size(self.ts(16)),
            text_input("Имя...", &self.input_username)
                .on_input(Message::UsernameChanged)
                .padding(self.s(10.0) as u16)
                .size(self.ts(18))
                .width(Length::Fixed(self.s(320.0))),
            Space::with_height(self.s(8.0)),
            text("Пароль").size(self.ts(16)),
            text_input("Пароль...", &self.input_password)
                .on_input(Message::PasswordChanged)
                .secure(true)
                .padding(self.s(10.0) as u16)
                .size(self.ts(18))
                .width(Length::Fixed(self.s(320.0))),
            Space::with_height(self.s(12.0)),
            menu_button("Войти", Message::SubmitLogin, sc),
            Space::with_height(4),
            menu_button("Назад", Message::BackToMenu, sc),
        ]
        .spacing(4)
        .align_x(Alignment::Center);
        if !self.form_error.is_empty() {
            col = col
                .push(Space::with_height(self.s(8.0)))
                .push(text(&self.form_error).size(self.ts(16)));
        }
        col.into()
    }

    fn view_register(&self) -> Element<'_, Message> {
        let sc = self.scale();
        let mut col = column![
            text("Создать аккаунт").size(self.ts(36)),
            Space::with_height(self.s(12.0)),
            text("Имя пользователя (минимум 3 символа)").size(self.ts(16)),
            text_input("Имя...", &self.input_username)
                .on_input(Message::UsernameChanged)
                .padding(self.s(10.0) as u16)
                .size(self.ts(18))
                .width(Length::Fixed(self.s(320.0))),
            Space::with_height(self.s(8.0)),
            text("Пароль (минимум 4 символа)").size(self.ts(16)),
            text_input("Пароль...", &self.input_password)
                .on_input(Message::PasswordChanged)
                .secure(true)
                .padding(self.s(10.0) as u16)
                .size(self.ts(18))
                .width(Length::Fixed(self.s(320.0))),
            Space::with_height(self.s(8.0)),
            text("Подтверди пароль").size(self.ts(16)),
            text_input("Повтори пароль...", &self.input_password2)
                .on_input(Message::Password2Changed)
                .secure(true)
                .padding(self.s(10.0) as u16)
                .size(self.ts(18))
                .width(Length::Fixed(self.s(320.0))),
            Space::with_height(self.s(12.0)),
            menu_button("Создать аккаунт", Message::SubmitRegister, sc),
            Space::with_height(4),
            menu_button("Назад", Message::BackToMenu, sc),
        ]
        .spacing(4)
        .align_x(Alignment::Center);
        if !self.form_error.is_empty() {
            col = col
                .push(Space::with_height(self.s(8.0)))
                .push(text(&self.form_error).size(self.ts(16)));
        }
        col.into()
    }

    fn view_load_game(&self) -> Element<'_, Message> {
        let sc = self.scale();

        // Фиксированные высоты элементов вне списка:
        // заголовок + шапка таблицы + кнопка Назад + отступы ≈ s(180)
        // Резерв: заголовок + шапка таблицы + кнопка Назад + отступы + статусбар
        let chrome_h = self.s(40.0)   // хедер (резервируется снаружи)
    + self.s(40.0)             // заголовок "Загрузить игру"
    + self.s(20.0)             // шапка таблицы
    + self.s(12.0)             // Space перед шапкой
    + self.s(4.0)              // Space после шапки
    + self.s(44.0)             // кнопка Назад (padding*2 + текст)
    + self.s(12.0)             // Space перед кнопкой
    + self.s(28.0)             // статусная строка снизу
    + self.s(24.0); // запас
        let list_max_h = (self.window_height - chrome_h).max(self.s(60.0));

        let list_area: Element<'_, Message> = if self.user_games.is_empty() {
            text("Игр пока нет").size(self.ts(18)).into()
        } else {
            let header = row![
                text("Название")
                    .size(self.ts(15))
                    .width(Length::Fixed(self.s(180.0))),
                text("Статус")
                    .size(self.ts(15))
                    .width(Length::Fixed(self.s(130.0))),
                text("Деньги")
                    .size(self.ts(15))
                    .width(Length::Fixed(self.s(80.0))),
                text("Ходы")
                    .size(self.ts(15))
                    .width(Length::Fixed(self.s(60.0))),
                Space::with_width(Length::Fixed(self.s(110.0))),
            ]
            .spacing(self.s(8.0) as u16);

            let mut games_col = column![].spacing(0);
            for (game_id, status, balance, moves, created_at) in &self.user_games {
                let ts = created_at.format("%d.%m.%y %H:%M").to_string();
                let name = format!("Игра-{}", ts);

                let status_ru = match status.as_str() {
                    "active" => "активна",
                    "paused" => "приостановлена",
                    "pending" => "ожидание",
                    "finished" => "завершена",
                    "surrender" => "сдался",
                    other => other,
                };

                let is_done = status == "finished" || status == "surrender";
                let load_cell: Element<'_, Message> = if is_done {
                    Space::with_width(Length::Fixed(self.s(110.0))).into()
                } else {
                    button(text("Загрузить").size(self.ts(14)))
                        .on_press(Message::LoadGame(*game_id))
                        .width(Length::Fixed(self.s(110.0)))
                        .padding(self.s(6.0) as u16)
                        .into()
                };

                let row_widget = container(
                    row![
                        text(name)
                            .size(self.ts(14))
                            .width(Length::Fixed(self.s(180.0))),
                        text(status_ru)
                            .size(self.ts(14))
                            .width(Length::Fixed(self.s(130.0))),
                        text(format!("{}", balance))
                            .size(self.ts(14))
                            .width(Length::Fixed(self.s(80.0))),
                        text(format!("{}", moves))
                            .size(self.ts(14))
                            .width(Length::Fixed(self.s(60.0))),
                        load_cell,
                    ]
                    .spacing(self.s(8.0) as u16)
                    .align_y(Alignment::Center),
                )
                .padding(iced::Padding {
                    top: self.s(4.0),
                    bottom: self.s(4.0),
                    left: self.s(8.0),
                    right: self.s(8.0),
                })
                .width(Length::Shrink)
                .style(|_| container::Style {
                    border: iced::Border {
                        color: Color::from_rgba(0.4, 0.4, 0.6, 0.5),
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                });
                games_col = games_col.push(row_widget);
            }

            let table_border_style = |_: &iced::Theme| container::Style {
                border: iced::Border {
                    color: Color::from_rgba(0.5, 0.5, 0.8, 0.7),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            };

            column![
                header,
                Space::with_height(self.s(4.0)),
                container(scrollable(games_col).height(Length::Fixed(list_max_h)))
                    .style(table_border_style)
                    .width(Length::Shrink),
            ]
            .spacing(0)
            .into()
        };

        column![
            text("Загрузить игру").size(self.ts(40)),
            Space::with_height(self.s(12.0)),
            list_area,
            Space::with_height(self.s(12.0)),
            menu_button("Назад", Message::BackToMenu, sc),
        ]
        .spacing(0)
        .align_x(Alignment::Center)
        .into()
    }

    fn view_create_game(&self) -> Element<'_, Message> {
        let sc = self.scale();
        column![
            text("Создать новую игру").size(self.ts(40)),
            Space::with_height(24),
            row![
                text("Стартовый баланс: ")
                    .size(18)
                    .width(Length::Fixed(200.0)),
                button(text(" < ").size(20))
                    .on_press(Message::GameBalanceDecrease)
                    .width(Length::Fixed(40.0))
                    .padding(8),
                text(format!("{}", self.create_game_balance))
                    .size(18)
                    .width(Length::Fixed(80.0)),
                button(text(" > ").size(20))
                    .on_press(Message::GameBalanceIncrease)
                    .width(Length::Fixed(40.0))
                    .padding(8),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            Space::with_height(12),
            row![
                text("Максимум ходов: ")
                    .size(18)
                    .width(Length::Fixed(200.0)),
                button(text(" < ").size(20))
                    .on_press(Message::GameMaxTurnsDecrease)
                    .width(Length::Fixed(40.0))
                    .padding(8),
                text(format!("{}", self.create_game_max_turns))
                    .size(18)
                    .width(Length::Fixed(80.0)),
                button(text(" > ").size(20))
                    .on_press(Message::GameMaxTurnsIncrease)
                    .width(Length::Fixed(40.0))
                    .padding(8),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            Space::with_height(12),
            row![
                text("Целевой баланс: ")
                    .size(18)
                    .width(Length::Fixed(200.0)),
                button(text(" < ").size(20))
                    .on_press(Message::GameTargetBalanceDecrease)
                    .width(Length::Fixed(40.0))
                    .padding(8),
                text(format!("{}", self.create_game_target_balance))
                    .size(18)
                    .width(Length::Fixed(80.0)),
                button(text(" > ").size(20))
                    .on_press(Message::GameTargetBalanceIncrease)
                    .width(Length::Fixed(40.0))
                    .padding(8),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            Space::with_height(8),
            text(format!(
                "Целевой баланс должен быть выше стартового (минимум {}) ",
                self.create_game_balance + 100
            ))
            .size(13),
            Space::with_height(16),
            menu_button("Создать игру", Message::CreateGameSubmit, sc),
            Space::with_height(8),
            menu_button("Назад", Message::BackToMenu, sc),
        ]
        .spacing(8)
        .align_x(Alignment::Center)
        .padding(20)
        .into()
    }

    fn view_game(&self) -> Element<'_, Message> {
        let rules = match &self.game_rules {
            Some(r) => r.clone(),
            None => return text("Загрузка...").size(24).into(),
        };
        let state = match &self.player_state {
            Some(s) => s.clone(),
            None => return text("Загрузка...").size(24).into(),
        };

        let turns_left = (rules.max_turns - state.moves_made).max(0);

        let slot_size = self.s(56.0);
        let btn_size = self.s(56.0);
        let fs_inv = self.ts(10);

        let short_label = |item: &db::InventoryItem| -> String {
            let val = item
                .effect
                .get("value")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            match item
                .effect
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
            {
                "flat_base" => format!("+{}\nБаза", val),
                "percent_bonus" => format!("+{}%\nАренда", val),
                "flat_final" => format!("+{}\nИтог", val),
                _ => item.name.chars().take(8).collect(),
            }
        };

        let mut inv_row = row![].spacing(self.s(3.0) as u16);
        for slot in 0..5usize {
            let cell: Element<'_, Message> = if slot < self.inventory.len() {
                let item = &self.inventory[slot];
                let label = short_label(item);
                let pu_id = item.power_up_id;
                let tip_item = TooltipItem {
                    name: item.name.clone(),
                    description: String::new(),
                    effect_type: item
                        .effect
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    effect_value: item
                        .effect
                        .get("value")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    buy_cost: None,
                    sell_cost: None,
                };
                let cell_inner: Element<'_, Message> = column![
                    container(text(label).size(fs_inv))
                        .width(Length::Fixed(slot_size))
                        .height(Length::Fixed(slot_size))
                        .padding(self.s(3.0) as u16)
                        .align_x(iced::alignment::Horizontal::Center)
                        .align_y(iced::alignment::Vertical::Center)
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                0.15, 0.2, 0.4, 0.9
                            ))),
                            border: iced::Border {
                                color: Color::from_rgba(0.4, 0.6, 1.0, 0.5),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..Default::default()
                        }),
                    button(text("Продать").size(self.ts(9)))
                        .on_press(Message::SellPowerUp(pu_id))
                        .width(Length::Fixed(btn_size))
                        .padding(self.s(2.0) as u16),
                ]
                .align_x(Alignment::Center)
                .spacing(self.s(2.0) as u16)
                .into();
                mouse_area(cell_inner)
                    .on_enter(Message::TooltipHoverStart(tip_item))
                    .on_exit(Message::TooltipHoverEnd)
                    .into()
            } else {
                container(text("— пусто —").size(self.ts(9)))
                    .width(Length::Fixed(self.s(56.0)))
                    .height(Length::Fixed(self.s(40.0)))
                    .padding(self.s(3.0) as u16)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center)
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.1, 0.1, 0.15, 0.72))),
                        border: iced::Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            };
            inv_row = inv_row.push(cell);
        }

        let icon_size = self.ts(14) as f32 * 0.75;

        let prop_btn_label = if self.show_property_mgmt {
            "▲ Мои объекты"
        } else {
            "▼ Мои объекты"
        };
        let prop_btn: Element<'_, Message> = button(text(prop_btn_label).size(self.ts(11)))
            .on_press(Message::TogglePropertyMgmt)
            .padding(self.s(5.0) as u16)
            .width(Length::Fill)
            .into();

        let stats_panel = container(
            column![
                row![
                    svg(std::path::Path::new("assets/money_icon.svg"))
                        .width(Length::Fixed(icon_size))
                        .height(Length::Fixed(icon_size)),
                    text(format!("Баланс:   {}", state.balance)).size(self.ts(14)),
                ]
                .spacing(self.s(4.0) as u16)
                .align_y(Alignment::Center),
                row![
                    svg(std::path::Path::new("assets/money_icon.svg"))
                        .width(Length::Fixed(icon_size))
                        .height(Length::Fixed(icon_size)),
                    text(format!("Цель:     {}", rules.target_balance)).size(self.ts(14)),
                ]
                .spacing(self.s(4.0) as u16)
                .align_y(Alignment::Center),
                text(format!(
                    "Ходов:    {}/{}",
                    state.moves_made, rules.max_turns
                ))
                .size(self.ts(14)),
                text(format!("Осталось: {}", turns_left)).size(self.ts(14)),
                Space::with_height(self.s(6.0)),
                text("Инвентарь").size(self.ts(12)),
                Space::with_height(self.s(3.0)),
                inv_row,
                Space::with_height(self.s(6.0)),
                prop_btn,
            ]
            .spacing(self.s(3.0) as u16)
            .padding(self.s(10.0) as u16),
        )
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.05, 0.05, 0.10, 0.84))),
            border: iced::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.13),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

        // SVG-спрайты кубиков
        let dice_svg_row: Element<'_, Message> = {
            let sz = self.s(36.0);
            let inner: Element<'_, Message> = match self.dice_roll {
                Some((d1, d2)) => {
                    let path1 = format!("assets/dice_{}.svg", d1);
                    let path2 = format!("assets/dice_{}.svg", d2);
                    row![
                        svg(iced::widget::svg::Handle::from_path(&path1))
                            .width(Length::Fixed(sz))
                            .height(Length::Fixed(sz)),
                        text(" + ").size(self.ts(16)),
                        svg(iced::widget::svg::Handle::from_path(&path2))
                            .width(Length::Fixed(sz))
                            .height(Length::Fixed(sz)),
                        text(format!(" = {}", d1 + d2)).size(self.ts(16)),
                    ]
                    .align_y(Alignment::Center)
                    .into()
                }
                None => {
                    Space::new(Length::Fixed(sz * 2.0 + self.s(60.0)), Length::Fixed(sz)).into()
                }
            };
            container(inner)
                .width(Length::Fixed(self.s(240.0)))
                .height(Length::Fixed(self.s(44.0)))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into()
        };

        // Одна кнопка: "Бросить кубик" / "Завершить ход"
        let action_btn: Element<'_, Message> = match self.turn_phase {
            TurnPhase::WaitingRoll => button(text("Бросить кубик").size(self.ts(15)))
                .on_press(Message::RollDice)
                .padding(self.s(10.0) as u16)
                .width(Length::Fixed(self.s(180.0)))
                .into(),
            TurnPhase::Rolled => button(text("Завершить ход").size(self.ts(15)))
                .on_press(Message::EndTurn)
                .padding(self.s(10.0) as u16)
                .width(Length::Fixed(self.s(180.0)))
                .into(),
            TurnPhase::WaitingAction => button(text("Завершить ход").size(self.ts(15)))
                .padding(self.s(10.0) as u16)
                .width(Length::Fixed(self.s(180.0)))
                .into(),
        };

        let dice_block: Element<'_, Message> = column![dice_svg_row, action_btn]
            .spacing(self.s(6.0) as u16)
            .align_x(Alignment::Center)
            .into();

        // Лог дохода — отдельный оверлей, прибитый к низу центра, не сдвигает кнопку
        // Вспомогательное замыкание для одного badge-сообщения
        let make_badge = |msg: &str,
                          bg: (f32, f32, f32),
                          border: (f32, f32, f32)|
         -> Element<'_, Message> {
            container(text(msg.to_string()).size(self.ts(13)))
                .padding(iced::Padding {
                    top: self.s(4.0),
                    bottom: self.s(4.0),
                    left: self.s(10.0),
                    right: self.s(10.0),
                })
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(bg.0, bg.1, bg.2, 0.9))),
                    border: iced::Border {
                        color: Color::from_rgba(border.0, border.1, border.2, 0.7),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        };

        // Единый блок событий: СТАРТ → списания (налог/аренда) → доходы от аренды.
        // Все сообщения одного типа объединяются в стопку, типы идут друг за другом —
        // каждый следующий автоматически смещается ниже предыдущего.
        let events_overlay: Element<'_, Message> = {
            let has_any = !self.start_log.is_empty()
                || !self.income_log.is_empty()
                || !self.rent_received_log.is_empty();

            if !has_any {
                Space::new(Length::Shrink, Length::Shrink).into()
            } else {
                let mut col = column![]
                    .spacing(self.s(5.0) as u16)
                    .align_x(Alignment::Center)
                    .max_width(self.s(320.0));

                // 1. СТАРТ (зелёный)
                for msg in &self.start_log {
                    col = col.push(make_badge(msg, (0.1, 0.35, 0.1), (0.3, 0.8, 0.3)));
                }

                // 2. Списания: налог / чужая аренда (красный)
                for msg in &self.income_log {
                    col = col.push(make_badge(msg, (0.25, 0.1, 0.1), (0.8, 0.3, 0.3)));
                }

                // 3. Доходы от аренды ботов (зелёный)
                for msg in &self.rent_received_log {
                    col = col.push(make_badge(msg, (0.05, 0.3, 0.05), (0.2, 0.8, 0.2)));
                }

                container(col)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Top)
                    .padding(iced::Padding {
                        top: self.s(120.0),
                        bottom: 0.0,
                        left: 0.0,
                        right: 0.0,
                    })
                    .into()
            }
        };
        // Блок действия на клетке
        let action_block: Option<Element<'_, Message>> = match &self.cell_action {
            Some(CellAction::CanBuy { cost, name, .. }) => Some(
                container(
                    column![
                        text(name.clone()).size(self.ts(15)),
                        text(format!("Свободная собственность! Купить за {}?", cost))
                            .size(self.ts(14)),
                        Space::with_height(self.s(6.0)),
                        button(text("Купить").size(self.ts(14)))
                            .on_press(Message::BuyProperty)
                            .padding(self.s(8.0) as u16)
                            .width(Length::Fixed(self.s(120.0))),
                    ]
                    .align_x(Alignment::Center)
                    .spacing(self.s(4.0) as u16)
                    .padding(self.s(10.0) as u16),
                )
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.1, 0.3, 0.1, 0.9))),
                    border: iced::Border {
                        color: Color::from_rgba(0.3, 0.8, 0.3, 0.8),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                })
                .into(),
            ),
            Some(CellAction::Shop { .. }) => None,
            Some(CellAction::MustPayRent { rent, owner, .. }) => Some(
                container(
                    column![
                        text(format!("Чужая собственность: {}", owner)).size(self.ts(14)),
                        text(format!(
                            "Аренда будет списана при завершении хода: {}",
                            rent
                        ))
                        .size(self.ts(13)),
                    ]
                    .align_x(Alignment::Center)
                    .spacing(self.s(4.0) as u16)
                    .padding(self.s(10.0) as u16),
                )
                .style(|_theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.3, 0.1, 0.1, 0.9))),
                    border: iced::Border {
                        color: Color::from_rgba(0.8, 0.3, 0.3, 0.8),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                })
                .into(),
            ),
            Some(CellAction::Info(msg)) => Some(
                container(text(msg.clone()).size(self.ts(14)))
                    .padding(self.s(10.0) as u16)
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.1, 0.1, 0.3, 0.9))),
                        border: iced::Border {
                            color: Color::from_rgba(0.3, 0.3, 0.8, 0.8),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    })
                    .into(),
            ),
            Some(CellAction::Tax) => {
                let tax = if let Some(s) = &self.player_state {
                    100 + (s.balance as f64 * 0.05).ceil() as i64
                } else {
                    100
                };
                Some(
                    container(
                        text(format!("Налог! При завершении хода спишется: {}", tax))
                            .size(self.ts(14)),
                    )
                    .padding(self.s(10.0) as u16)
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.3, 0.1, 0.1, 0.9))),
                        border: iced::Border {
                            color: Color::from_rgba(0.8, 0.3, 0.3, 0.8),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    })
                    .into(),
                )
            }
            None => None,
        };

        // ── Левая панель: статистика + инвентарь + управление объектами ──
        let mut left_col = column![stats_panel,]
            .spacing(self.s(6.0) as u16)
            .width(Length::Fixed(self.s(310.0)));

        if self.show_property_mgmt {
            left_col = left_col.push(self.view_property_mgmt());
        }

        let left_panel: Element<'_, Message> = container(left_col)
            .padding(self.s(8.0))
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.05, 0.7))),
                ..Default::default()
            })
            .into();

        // ── Центр: игровое поле + оверлей кубика ─────────────────────────
        let board = self.view_board(self.local_player_pos);

        // Кнопка — всегда строго по центру
        let dice_overlay: Element<'_, Message> = container(dice_block)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into();

        // action_block — отдельный слой выше кубиков, не смещает кнопку
        let action_overlay: Element<'_, Message> = if let Some(action) = action_block {
            container(action)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Bottom)
                .padding(iced::Padding {
                    bottom: self.s(180.0),
                    top: 0.0,
                    left: 0.0,
                    right: 0.0,
                })
                .into()
        } else {
            Space::new(Length::Shrink, Length::Shrink).into()
        };

        let center_overlay: Element<'_, Message> =
            stack![dice_overlay, action_overlay, events_overlay]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();

        let board_with_overlay: Element<'_, Message> = stack![board, center_overlay]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        // ── Правая панель: статус ботов + лог ────────────────────────────
        let mut right_col = column![]
            .spacing(self.s(8.0) as u16)
            .align_x(Alignment::Center);

        // Панель статусов ботов
        if !self.bot_participants.is_empty() {
            let mut bots_col = column![
                text("Боты:").size(self.ts(11)),
                Space::with_height(self.s(2.0)),
            ]
            .spacing(self.s(3.0) as u16);

            for (i, bot) in self.bot_participants.iter().enumerate() {
                let color = match i % 4 {
                    0 => Color::from_rgb(1.0, 0.3, 0.3),
                    1 => Color::from_rgb(0.3, 1.0, 0.4),
                    2 => Color::from_rgb(0.3, 0.7, 1.0),
                    _ => Color::from_rgb(1.0, 0.85, 0.1),
                };
                // Количество собственностей бота из board_cells
                let bot_props = self
                    .board_cells
                    .iter()
                    .filter(|c| c.cell_type == "property" && c.owner_user_id == Some(bot.user_id))
                    .count();

                let name_short: String = bot.username.chars().take(10).collect();
                let money_icon_sz = self.s(10.0);
                let bot_row: Element<'_, Message> = container(
                    row![
                        // Цветной квадрат — цвет бота
                        container(Space::new(
                            Length::Fixed(self.s(6.0)),
                            Length::Fixed(self.s(6.0)),
                        ))
                        .style(move |_| container::Style {
                            background: Some(Background::Color(color)),
                            border: iced::Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        text(name_short).size(self.ts(10)),
                        svg(std::path::Path::new("assets/money_icon.svg"))
                            .width(Length::Fixed(money_icon_sz))
                            .height(Length::Fixed(money_icon_sz)),
                        text(format!("{}", bot.balance)).size(self.ts(10)),
                        text(format!("| {} Н.", bot_props)).size(self.ts(10)),
                    ]
                    .spacing(self.s(3.0) as u16)
                    .align_y(Alignment::Center),
                )
                .padding(iced::Padding {
                    top: self.s(2.0),
                    bottom: self.s(2.0),
                    left: self.s(4.0),
                    right: self.s(4.0),
                })
                .into();
                bots_col = bots_col.push(bot_row);
            }

            right_col = right_col.push(
                container(bots_col)
                    .padding(self.s(6.0))
                    .width(Length::Fixed(self.s(200.0)))
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
                        border: iced::Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
                            width: 1.0,
                            radius: iced::border::Radius::from(4.0),
                        },
                        ..Default::default()
                    }),
            );
        }

        // Лог ходов ботов
        if !self.bot_turn_log.is_empty() {
            let mut log_col = column![text("Ходы ботов:").size(self.ts(11))].spacing(2);
            for line in self.bot_turn_log.iter().rev().take(6) {
                log_col = log_col.push(text(line).size(self.ts(10)));
            }
            right_col = right_col.push(
                container(log_col)
                    .padding(self.s(6.0))
                    .width(Length::Fixed(self.s(200.0)))
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
                        border: iced::Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
                            width: 1.0,
                            radius: iced::border::Radius::from(4.0),
                        },
                        ..Default::default()
                    }),
            );
        }

        // ── Правая нижняя: карточка собственности + тултип усиления ──────
        let mut info_col = column![].spacing(self.s(6.0) as u16);

        // Карточка усиления при наведении
        if self.tooltip_visible {
            if let Some(item) = &self.tooltip_item {
                let mut tip_col = column![
                    text(&item.name).size(self.ts(13)),
                    Space::with_height(self.s(4.0)),
                    text(&item.description).size(self.ts(11)),
                ]
                .spacing(self.s(2.0) as u16);

                if !item.effect_type.is_empty() {
                    let effect_str = match item.effect_type.as_str() {
                        "flat_base" => format!("+{} к базовой аренде", item.effect_value),
                        "percent_bonus" => format!("+{}% к аренде", item.effect_value),
                        "flat_final" => format!("+{} к итоговой аренде", item.effect_value),
                        other => other.to_string(),
                    };
                    tip_col =
                        tip_col.push(text(format!("Эффект: {}", effect_str)).size(self.ts(11)));
                }
                if let Some(buy) = item.buy_cost {
                    tip_col = tip_col.push(text(format!("Цена: {}", buy)).size(self.ts(11)));
                }
                if let Some(sell) = item.sell_cost {
                    tip_col = tip_col.push(text(format!("Продажа: {}", sell)).size(self.ts(11)));
                }

                info_col = info_col.push(
                    container(tip_col)
                        .padding(self.s(8.0))
                        .width(Length::Fixed(self.s(184.0)))
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                0.05, 0.18, 0.05, 0.96,
                            ))),
                            border: iced::Border {
                                color: Color::from_rgba(0.3, 0.8, 0.3, 0.7),
                                width: 1.0,
                                radius: 6.0.into(),
                            },
                            ..Default::default()
                        }),
                );
            }
        }

        // Карточка собственности при наведении
        if let Some(idx) = self.hovered_cell {
            if let Some(cell) = self.board_cells.iter().find(|c| c.cell_index == idx) {
                let upg = self
                    .player_properties
                    .iter()
                    .find(|p| p.cell_index == idx)
                    .map(|p| p.upgrades_count)
                    .unwrap_or(0);

                let owner_label = if let Some(owner_id) = cell.owner_user_id {
                    if self.current_user.as_ref().map(|(id, _)| *id) == Some(owner_id) {
                        "Ваша".to_string()
                    } else {
                        self.bot_participants
                            .iter()
                            .find(|b| b.user_id == owner_id)
                            .map(|b| b.username.clone())
                            .unwrap_or_else(|| "Чужая".to_string())
                    }
                } else {
                    "Свободна".to_string()
                };

                let card_col = column![
                    text(cell.prop_name.as_deref().unwrap_or("?")).size(self.ts(13)),
                    Space::with_height(self.s(4.0)),
                    text(format!("Владелец: {}", owner_label)).size(self.ts(11)),
                    text(format!("Цена: {}", cell.purchase_cost.unwrap_or(0))).size(self.ts(11)),
                    text(format!("Аренда: {}", cell.rent_cost.unwrap_or(0))).size(self.ts(11)),
                    text(format!("Улучшений: {}", upg)).size(self.ts(11)),
                ]
                .spacing(self.s(2.0) as u16);

                info_col = info_col.push(
                    container(card_col)
                        .padding(self.s(8.0))
                        .width(Length::Fixed(self.s(184.0)))
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                0.05, 0.05, 0.18, 0.96,
                            ))),
                            border: iced::Border {
                                color: Color::from_rgba(0.5, 0.5, 1.0, 0.7),
                                width: 1.0,
                                radius: 6.0.into(),
                            },
                            ..Default::default()
                        }),
                );
            }
        }

        let right_panel: Element<'_, Message> = container(
            column![
                // Верхняя половина — боты и лог
                container(right_col)
                    .width(Length::Fill)
                    .height(Length::FillPortion(1)),
                // Нижняя половина — карточки
                container(info_col)
                    .width(Length::Fill)
                    .height(Length::FillPortion(1))
                    .align_y(iced::alignment::Vertical::Bottom),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .padding(self.s(8.0))
        .width(Length::Fixed(self.s(200.0)))
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.05, 0.7))),
            ..Default::default()
        })
        .into();

        // ── Финальная сборка ─────────────────────────────────────────────
        row![left_panel, board_with_overlay, right_panel]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_board(&self, player_pos: i32) -> Element<'_, Message> {
        const GRID: usize = 11;

        let mut grid: Vec<Vec<Option<i32>>> = vec![vec![None; GRID]; GRID];
        for i in 0..10usize {
            grid[10][i] = Some(i as i32);
        }
        for i in 0..10usize {
            grid[10 - i][10] = Some((10 + i) as i32);
        }
        for i in 0..10usize {
            grid[0][10 - i] = Some((20 + i) as i32);
        }
        for i in 0..10usize {
            grid[i][0] = Some((30 + i) as i32);
        }

        let side_of = |idx: i32| -> &'static str {
            match idx {
                0..=9 => "bottom",
                10..=19 => "right",
                20..=29 => "top",
                _ => "left",
            }
        };

        let cell_map: std::collections::HashMap<i32, &db::BoardCell> =
            self.board_cells.iter().map(|c| (c.cell_index, c)).collect();

        let available_h = self.window_height - self.s(40.0) - self.s(28.0);
        let left_panel_w = self.s(310.0) + self.s(16.0);
        let right_panel_w = self.s(200.0) + self.s(16.0);
        let available_w = (self.window_width - left_panel_w - right_panel_w).max(100.0);
        let cell_size = (available_h / 11.0).min(available_w / 11.0).floor();

        let fs = (cell_size / 54.0 * 10.0).max(7.0);
        let fs_sm = (cell_size / 54.0 * 9.0).max(6.0);

        // Размер иконки: 4 штуки должны поместиться в ширину клетки
        let icon_sz = (cell_size / 4.0 - 1.0).max(6.0);

        // ── Цвет каждого участника ────────────────────────────────
        // Игрок — белый; боты — яркие цвета по порядку turn_order
        let player_id: Option<Uuid> = self.current_user.as_ref().map(|(id, _)| *id);
        let bot_colors: Vec<(Uuid, Color)> = self
            .bot_participants
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let c = match i % 4 {
                    0 => Color::from_rgb(1.0, 0.3, 0.3),  // красный
                    1 => Color::from_rgb(0.3, 1.0, 0.4),  // зелёный
                    2 => Color::from_rgb(0.3, 0.7, 1.0),  // голубой
                    _ => Color::from_rgb(1.0, 0.85, 0.1), // жёлтый
                };
                (b.user_id, c)
            })
            .collect();

        // Вспомогательная: цвет иконки по user_id
        let participant_color = |uid: Uuid| -> Color {
            if Some(uid) == player_id {
                Color::WHITE
            } else {
                bot_colors
                    .iter()
                    .find(|(id, _)| *id == uid)
                    .map(|(_, c)| *c)
                    .unwrap_or(Color::from_rgba(0.7, 0.7, 0.7, 0.6))
            }
        };

        // Позиции всех участников на клетках
        // Игрок: player_pos (локальный, до commit)
        // Боты: из bot_participants
        // Структура: cell_index -> Vec<(user_id, color)>
        let mut occupants: std::collections::HashMap<i32, Vec<(Uuid, Color)>> =
            std::collections::HashMap::new();

        if let Some(pid) = player_id {
            occupants
                .entry(player_pos)
                .or_default()
                .push((pid, Color::WHITE));
        }
        for bot in &self.bot_participants {
            let c = participant_color(bot.user_id);
            occupants
                .entry(bot.position)
                .or_default()
                .push((bot.user_id, c));
        }

        // cell_index → upgrades_count для собственностей игрока
        // Используется для выбора SVG property_0..3 без обращения к БД
        let prop_upgrades_map: std::collections::HashMap<i32, usize> = self
            .player_properties
            .iter()
            .map(|p| (p.cell_index, p.upgrades_count.min(3) as usize))
            .collect();

        let mut board_col = column![].spacing(1);
        for row_i in 0..GRID {
            let mut board_row = row![].spacing(1);
            for col_i in 0..GRID {
                let cell_elem: Element<'_, Message> = match grid[row_i][col_i] {
                    None => {
                        // Пустая внутренняя клетка: рисуем здесь иконки игроков,
                        // стоящих на соседних клетках периметра (вынос наружу)
                        let mut ext_colors: Vec<Color> = vec![];
                        // Запоминаем направление первого найденного соседа с периметра:
                        // (dr, dc) — смещение от пустой клетки к соседу периметра
                        let neighbors: [(i32, i32); 4] = [
                            (row_i as i32 - 1, col_i as i32),
                            (row_i as i32 + 1, col_i as i32),
                            (row_i as i32, col_i as i32 - 1),
                            (row_i as i32, col_i as i32 + 1),
                        ];
                        // dr/dc: направление от пустой клетки К клетке периметра
                        let mut border_dir: (i32, i32) = (0, 0);
                        for (r, c) in neighbors {
                            if r >= 0 && r < GRID as i32 && c >= 0 && c < GRID as i32 {
                                if let Some(nidx) = grid[r as usize][c as usize] {
                                    if let Some(occ) = occupants.get(&nidx) {
                                        for (_, color) in occ {
                                            ext_colors.push(*color);
                                        }
                                        if border_dir == (0, 0) {
                                            border_dir = (r - row_i as i32, c - col_i as i32);
                                        }
                                    }
                                }
                            }
                        }

                        if ext_colors.is_empty() {
                            Space::new(Length::Fixed(cell_size), Length::Fixed(cell_size)).into()
                        } else {
                            let mut icons_row = row![].spacing(1);
                            for c in ext_colors.iter().take(4) {
                                let cc = *c;
                                let ico: Element<'_, Message> =
                                    svg(std::path::Path::new("assets/player_icon.svg"))
                                        .width(Length::Fixed(icon_sz))
                                        .height(Length::Fixed(icon_sz))
                                        .style(move |_theme, _status| iced::widget::svg::Style {
                                            color: Some(cc),
                                        })
                                        .into();
                                icons_row = icons_row.push(ico);
                            }
                            // Прижимаем иконки к стороне, обращённой к клетке периметра,
                            // с отступом ICON_PAD пикселей — иконки будут у самого края,
                            // визуально "снаружи" клетки периметра.
                            const ICON_PAD: f32 = 10.0;
                            // Прижимаем к стороне, ПРОТИВОПОЛОЖНОЙ периметру — иконки снаружи поля
                            let align_x = match border_dir.1 {
                                d if d > 0 => iced::alignment::Horizontal::Right,
                                d if d < 0 => iced::alignment::Horizontal::Left,
                                _ => iced::alignment::Horizontal::Center,
                            };
                            let align_y = match border_dir.0 {
                                d if d > 0 => iced::alignment::Vertical::Bottom,
                                d if d < 0 => iced::alignment::Vertical::Top,
                                _ => iced::alignment::Vertical::Center,
                            };
                            let pad = iced::Padding {
                                top: if border_dir.0 < 0 { ICON_PAD } else { 0.0 },
                                bottom: if border_dir.0 > 0 { ICON_PAD } else { 0.0 },
                                left: if border_dir.1 < 0 { ICON_PAD } else { 0.0 },
                                right: if border_dir.1 > 0 { ICON_PAD } else { 0.0 },
                            };
                            container(icons_row)
                                .width(Length::Fixed(cell_size))
                                .height(Length::Fixed(cell_size))
                                .align_x(align_x)
                                .align_y(align_y)
                                .padding(pad)
                                .into()
                        }
                    }
                    Some(idx) => {
                        let side = side_of(idx);

                        // ── Текстовые метки ──────────────────────────────
                        let (label, sublabel) = if idx == 0 {
                            ("СТАРТ".to_string(), String::new())
                        } else {
                            match cell_map.get(&idx) {
                                Some(cell) => match cell.cell_type.as_str() {
                                    "tax" => ("НАЛОГ".to_string(), "100+5%".to_string()),
                                    "shop" => ("МАГ".to_string(), String::new()),
                                    "empty" => (String::new(), String::new()),
                                    "property" => {
                                        let short = cell
                                            .prop_name
                                            .as_deref()
                                            .unwrap_or("?")
                                            .split_whitespace()
                                            .last()
                                            .unwrap_or("?");
                                        (
                                            short.chars().take(7).collect(),
                                            format!("{}", cell.purchase_cost.unwrap_or(0)),
                                        )
                                    }
                                    _ => (String::new(), String::new()),
                                },
                                None => (String::new(), String::new()),
                            }
                        };

                        // ── Цветная полоска группы собственности ─────────
                        let color_stripe: Option<Color> =
                            if cell_map.get(&idx).map(|c| c.cell_type.as_str()) == Some("property")
                            {
                                let pos_on_side = idx % 10;
                                let side_num = idx / 10;
                                let group = if pos_on_side <= 3 {
                                    side_num * 2
                                } else {
                                    side_num * 2 + 1
                                };
                                Some(match group {
                                    0 => Color::from_rgb(0.53, 0.81, 0.98),
                                    1 => Color::from_rgb(0.58, 0.0, 0.83),
                                    2 => Color::from_rgb(1.0, 0.6, 0.2),
                                    3 => Color::from_rgb(1.0, 0.0, 0.0),
                                    4 => Color::from_rgb(1.0, 1.0, 0.0),
                                    5 => Color::from_rgb(0.13, 0.55, 0.13),
                                    6 => Color::from_rgb(0.0, 0.0, 0.8),
                                    _ => Color::from_rgb(0.55, 0.27, 0.07),
                                })
                            } else {
                                None
                            };

                        let stripe_size = (cell_size * 0.22).max(4.0);

                        // ── Сборка содержимого клетки ─────────────────────
                        // Структура зависит от стороны:
                        //   bottom/top: вертикальный layout (column)
                        //     bottom: [stripe_top] | [text] [owner_bar] [icons_bottom]
                        //     top:    [icons_top] [owner_bar] [text] | [stripe_bottom]
                        //   left/right: горизонтальный layout (row)
                        //     right: [stripe_left] | [icons_right] [owner_bar] [text]
                        //     left:  [text] [owner_bar] [icons_left] | [stripe_right]

                        // ── Текстовые метки / SVG ──────────────────────────
                        // property-клетки: SVG property_N с названием и ценой
                        // ostальные: текст как прежде
                        let text_core: Element<'_, Message> = {
                            let is_prop = cell_map
                                .get(&idx)
                                .map(|c| c.cell_type.as_str() == "property")
                                .unwrap_or(false);

                            if is_prop {
                                if let Some(cell) = cell_map.get(&idx) {
                                    // Уровень усилений: 0 если клетка не принадлежит игроку
                                    // (боты/свободные), иначе — из player_properties
                                    let upg = prop_upgrades_map.get(&idx).copied().unwrap_or(0);
                                    let prop_svg_size = cell_size;

                                    let short_name = cell
                                        .prop_name
                                        .as_deref()
                                        .unwrap_or("?")
                                        .split_whitespace()
                                        .last()
                                        .unwrap_or("?");

                                    let value_label = if cell.owner_user_id.is_some() {
                                        format!("А:{}", cell.rent_cost.unwrap_or(0))
                                    } else {
                                        format!("{}", cell.purchase_cost.unwrap_or(0))
                                    };

                                    let hdr_hex = match color_stripe {
                                        Some(c) => format!(
                                            "#{:02x}{:02x}{:02x}",
                                            (c.r * 255.0) as u8,
                                            (c.g * 255.0) as u8,
                                            (c.b * 255.0) as u8,
                                        ),
                                        None => "#cccccc".to_string(),
                                    };
                                    let owner_hex: Option<String> = cell.owner_user_id.map(|oid| {
                                        let c = participant_color(oid);
                                        format!(
                                            "#{:02x}{:02x}{:02x}",
                                            (c.r * 255.0) as u8,
                                            (c.g * 255.0) as u8,
                                            (c.b * 255.0) as u8,
                                        )
                                    });
                                    let handle = property_svg_handle(
                                        upg,
                                        short_name,
                                        &value_label,
                                        &hdr_hex,
                                        owner_hex.as_deref(),
                                    );
                                    svg(handle)
                                        .width(Length::Fixed(prop_svg_size))
                                        .height(Length::Fixed(prop_svg_size))
                                        .into()
                                } else {
                                    column![text(label).size(fs), text(sublabel).size(fs_sm)]
                                        .align_x(Alignment::Center)
                                        .spacing(1)
                                        .padding(1)
                                        .into()
                                }
                            } else {
                                column![text(label).size(fs), text(sublabel).size(fs_sm)]
                                    .align_x(Alignment::Center)
                                    .spacing(1)
                                    .padding(1)
                                    .into()
                            }
                        };

                        // SVG занимает всю клетку
                        let inner_el: Element<'_, Message> = container(text_core)
                            .width(Length::Fixed(cell_size))
                            .height(Length::Fixed(cell_size))
                            .into();

                        // Фон клетки (выделение если игрок стоит здесь)
                        let is_player = player_pos == idx;
                        let bg = if is_player {
                            Some(Background::Color(Color::from_rgba(0.8, 0.7, 0.0, 0.35)))
                        } else {
                            None
                        };

                        // SVG — базовый слой на всю клетку
                        let base_cell: Element<'_, Message> = container(inner_el)
                            .width(Length::Fixed(cell_size))
                            .height(Length::Fixed(cell_size))
                            .style(move |_| container::Style {
                                background: bg,
                                ..Default::default()
                            })
                            .into();

                        let has_owner = cell_map
                            .get(&idx)
                            .map(|c| c.cell_type == "property" && c.owner_user_id.is_some())
                            .unwrap_or(false);

                        let cell_body: Element<'_, Message> = if color_stripe.is_some() || has_owner
                        {
                            // Слой с двумя полосками: снаружи — цвет группы, изнутри — цвет владельца.
                            // Реализуем как column/row из двух тонких container'ов внутри stack.
                            let overlay: Element<'_, Message> = {
                                let bar_thick = (cell_size * 0.18).max(5.0);

                                // Полоска группы (внешняя сторона клетки)
                                let group_bar: Element<'_, Message> = if let Some(gc) = color_stripe
                                {
                                    container(Space::new(Length::Fill, Length::Fill))
                                        .width(match side {
                                            "left" | "right" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        })
                                        .height(match side {
                                            "top" | "bottom" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        })
                                        .style(move |_| container::Style {
                                            background: Some(Background::Color(gc)),
                                            ..Default::default()
                                        })
                                        .into()
                                } else {
                                    Space::new(
                                        match side {
                                            "left" | "right" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        },
                                        match side {
                                            "top" | "bottom" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        },
                                    )
                                    .into()
                                };

                                // Полоска владельца (внутренняя сторона клетки)
                                let owner_strip: Element<'_, Message> = if let Some(oc) =
                                    cell_map.get(&idx).and_then(|c| {
                                        if c.cell_type == "property" {
                                            c.owner_user_id
                                        } else {
                                            None
                                        }
                                    }) {
                                    let owner_color = participant_color(oc);
                                    container(Space::new(Length::Fill, Length::Fill))
                                        .width(match side {
                                            "left" | "right" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        })
                                        .height(match side {
                                            "top" | "bottom" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        })
                                        .style(move |_| container::Style {
                                            background: Some(Background::Color(owner_color)),
                                            ..Default::default()
                                        })
                                        .into()
                                } else {
                                    Space::new(
                                        match side {
                                            "left" | "right" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        },
                                        match side {
                                            "top" | "bottom" => Length::Fixed(bar_thick),
                                            _ => Length::Fill,
                                        },
                                    )
                                    .into()
                                };

                                // Собираем: снаружи group_bar, изнутри owner_strip,
                                // между ними пустое пространство
                                let bars: Element<'_, Message> = match side {
                                    "bottom" => column![
                                        group_bar,
                                        Space::new(Length::Fill, Length::Fill),
                                        owner_strip,
                                    ]
                                    .spacing(0)
                                    .into(),
                                    "top" => column![
                                        owner_strip,
                                        Space::new(Length::Fill, Length::Fill),
                                        group_bar,
                                    ]
                                    .spacing(0)
                                    .into(),
                                    "right" => row![
                                        group_bar,
                                        Space::new(Length::Fill, Length::Fill),
                                        owner_strip,
                                    ]
                                    .spacing(0)
                                    .into(),
                                    _ => row![
                                        owner_strip,
                                        Space::new(Length::Fill, Length::Fill),
                                        group_bar,
                                    ]
                                    .spacing(0)
                                    .into(),
                                };

                                container(bars)
                                    .width(Length::Fixed(cell_size))
                                    .height(Length::Fixed(cell_size))
                                    .into()
                            };

                            stack![overlay, base_cell].into()
                        } else {
                            base_cell
                        };

                        // Для property-клеток — mouse_area для карточки наведения
                        let is_prop = cell_map
                            .get(&idx)
                            .map(|c| c.cell_type.as_str() == "property")
                            .unwrap_or(false);

                        if is_prop {
                            mouse_area(cell_body)
                                .on_enter(Message::CellHovered(Some(idx)))
                                .on_exit(Message::CellHovered(None))
                                .into()
                        } else {
                            cell_body
                        }
                    }
                };
                board_row = board_row.push(cell_elem);
            }
            board_col = board_col.push(board_row);
        }

        container(board_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn view_property_mgmt(&self) -> Element<'_, Message> {
        let panel_w = self.s(260.0);

        if self.player_properties.is_empty() {
            return container(text("У вас нет собственностей").size(self.ts(11)))
                .padding(self.s(8.0))
                .width(Length::Fixed(panel_w))
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.05, 0.05, 0.15, 0.92))),
                    border: iced::Border {
                        color: Color::from_rgba(0.4, 0.4, 0.7, 0.5),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                })
                .into();
        }

        // Список собственностей — горизонтальная строка карточек
        let mut prop_list = row![].spacing(self.s(4.0) as u16);
        for (i, prop) in self.player_properties.iter().enumerate() {
            let is_selected = self.selected_property == Some(i);
            let upg = prop.upgrades_count.min(3) as usize;

            // В панели управления: снизу показываем аренду
            let short_name = prop.prop_name.split_whitespace().last().unwrap_or("?");
            let value_label = format!("А:{}", prop.rent_cost);
            // Цвет группы по cell_index собственности
            let prop_hdr_hex = {
                let idx = prop.cell_index;
                let pos_on_side = idx % 10;
                let side_num = idx / 10;
                let group = if pos_on_side <= 3 {
                    side_num * 2
                } else {
                    side_num * 2 + 1
                };
                match group {
                    0 => "#88ccff",
                    1 => "#9400d3",
                    2 => "#ff9933",
                    3 => "#ff0000",
                    4 => "#ffff00",
                    5 => "#228b22",
                    6 => "#0000cc",
                    _ => "#8b4513",
                }
            };
            let handle =
                property_svg_handle(upg, short_name, &value_label, prop_hdr_hex, Some("#ffffff"));
            let card_size = self.s(48.0);

            let prop_btn: Element<'_, Message> = button(
                svg(handle)
                    .width(Length::Fixed(card_size))
                    .height(Length::Fixed(card_size)),
            )
            .on_press(Message::SelectProperty(i))
            .padding(self.s(3.0) as u16)
            .style(move |theme, status| {
                let base = button::primary(theme, status);
                if is_selected {
                    button::Style {
                        background: Some(Background::Color(Color::from_rgba(0.3, 0.5, 0.9, 0.8))),
                        border: iced::Border {
                            color: Color::from_rgba(0.5, 0.7, 1.0, 1.0),
                            width: 2.0,
                            radius: 4.0.into(),
                        },
                        ..base
                    }
                } else {
                    button::Style {
                        background: Some(Background::Color(Color::from_rgba(0.1, 0.1, 0.25, 0.85))),
                        ..base
                    }
                }
            })
            .into();
            prop_list = prop_list.push(prop_btn);
        }

        let prop_scroll = scrollable(prop_list)
            .direction(Direction::Horizontal(
                Scrollbar::new()
                    .width(self.s(4.0) as u16)
                    .scroller_width(self.s(4.0) as u16),
            ))
            .width(Length::Fixed(panel_w));

        let mut panel_col = column![
            text("Управление объектами").size(self.ts(11)),
            Space::with_height(self.s(4.0)),
            prop_scroll,
        ]
        .spacing(self.s(4.0) as u16);

        // Детали выбранной собственности
        if let Some(sel) = self.selected_property {
            if let Some(prop) = self.player_properties.get(sel) {
                panel_col = panel_col.push(Space::with_height(self.s(6.0)));

                // Заголовок и аренда
                panel_col = panel_col.push(
                    text(format!("{} | Аренда: {}", prop.prop_name, prop.rent_cost))
                        .size(self.ts(10)),
                );
                panel_col = panel_col.push(Space::with_height(self.s(4.0)));

                // Слоты усилений (3 штуки)
                let installed: Vec<(String, Uuid)> = if let Some(arr) = prop.upgrades.as_array() {
                    arr.iter()
                        .filter_map(|v| {
                            let name = v.get("name")?.as_str()?.to_string();
                            let id_str = v.get("power_up_id")?.as_str()?;
                            let id = Uuid::parse_str(id_str).ok()?;
                            Some((name, id))
                        })
                        .collect()
                } else {
                    vec![]
                };

                let max_slots = prop.max_upgrades.max(1) as usize;

                panel_col = panel_col.push(text("Слоты усилений:").size(self.ts(10)));
                panel_col = panel_col.push(Space::with_height(self.s(2.0)));

                // Слоты по 3 в строку — каждый слот занимает равную долю ширины панели
                let slots_per_row = 3usize;
                let slot_gap = self.s(4.0);
                // ширина одной плитки с учётом отступов между ними
                let slot_w = ((panel_w - slot_gap * (slots_per_row as f32 - 1.0))
                    / slots_per_row as f32)
                    .floor();
                let slot_h = self.s(52.0);

                for chunk_start in (0..max_slots).step_by(slots_per_row) {
                    let mut slots_row = row![].spacing(slot_gap as u16);
                    for slot_i in chunk_start..(chunk_start + slots_per_row).min(max_slots) {
                        let slot_el: Element<'_, Message> =
                            if let Some((name, pu_id)) = installed.get(slot_i) {
                                let pu_id = *pu_id;
                                let prop_id = prop.property_id;
                                let tip = TooltipItem {
                                    name: name.clone(),
                                    description: String::new(),
                                    effect_type: String::new(),
                                    effect_value: 0,
                                    buy_cost: None,
                                    sell_cost: None,
                                };
                                let btn: Element<'_, Message> = button(
                                    column![
                                        text(name.clone()).size(self.ts(9)),
                                        text("× извлечь").size(self.ts(8)),
                                    ]
                                    .align_x(Alignment::Center)
                                    .spacing(1),
                                )
                                .on_press(Message::UninstallUpgrade {
                                    property_id: prop_id,
                                    power_up_id: pu_id,
                                })
                                .padding(self.s(4.0) as u16)
                                .width(Length::Fixed(slot_w))
                                .height(Length::Fixed(slot_h))
                                .style(|theme, status| {
                                    let base = button::primary(theme, status);
                                    button::Style {
                                        background: Some(Background::Color(Color::from_rgba(
                                            0.2, 0.4, 0.2, 0.9,
                                        ))),
                                        ..base
                                    }
                                })
                                .into();
                                mouse_area(btn)
                                    .on_enter(Message::TooltipHoverStart(tip))
                                    .on_exit(Message::TooltipHoverEnd)
                                    .into()
                            } else {
                                container(text("— пусто —").size(self.ts(9)))
                                    .width(Length::Fixed(slot_w))
                                    .height(Length::Fixed(slot_h))
                                    .align_x(iced::alignment::Horizontal::Center)
                                    .align_y(iced::alignment::Vertical::Center)
                                    .style(|_| container::Style {
                                        background: Some(Background::Color(Color::from_rgba(
                                            0.1, 0.1, 0.15, 0.6,
                                        ))),
                                        border: iced::Border {
                                            color: Color::from_rgba(0.4, 0.4, 0.5, 0.4),
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    })
                                    .into()
                            };
                        slots_row = slots_row.push(slot_el);
                    }
                    panel_col = panel_col.push(slots_row);
                    panel_col = panel_col.push(Space::with_height(self.s(2.0)));
                }

                // Инвентарь для установки (только если есть свободный слот)
                if installed.len() < max_slots && !self.inventory.is_empty() {
                    panel_col = panel_col.push(Space::with_height(self.s(6.0)));
                    panel_col = panel_col.push(text("Установить из инвентаря:").size(self.ts(10)));
                    panel_col = panel_col.push(Space::with_height(self.s(2.0)));

                    let prop_id = prop.property_id;
                    let items_per_row = 3usize;
                    let inv_items: Vec<_> = self.inventory.iter().collect();
                    for chunk_start in (0..inv_items.len()).step_by(items_per_row) {
                        let mut inv_row = row![].spacing(self.s(3.0) as u16);
                        for item in &inv_items
                            [chunk_start..(chunk_start + items_per_row).min(inv_items.len())]
                        {
                            let pu_id = item.power_up_id;
                            let already = installed.iter().any(|(_, id)| *id == pu_id);
                            let short = item
                                .name
                                .split(':')
                                .last()
                                .unwrap_or(&item.name)
                                .trim()
                                .chars()
                                .take(10)
                                .collect::<String>();
                            let inv_btn: Element<'_, Message> = if already {
                                container(text(short).size(self.ts(9)))
                                    .width(Length::Fixed(self.s(80.0)))
                                    .height(Length::Fixed(self.s(36.0)))
                                    .align_x(iced::alignment::Horizontal::Center)
                                    .align_y(iced::alignment::Vertical::Center)
                                    .style(|_| container::Style {
                                        background: Some(Background::Color(Color::from_rgba(
                                            0.2, 0.2, 0.2, 0.5,
                                        ))),
                                        ..Default::default()
                                    })
                                    .into()
                            } else {
                                button(
                                    column![
                                        text(short).size(self.ts(9)),
                                        text(format!("×{}", item.quantity)).size(self.ts(8)),
                                    ]
                                    .align_x(Alignment::Center)
                                    .spacing(1),
                                )
                                .on_press(Message::InstallUpgrade {
                                    property_id: prop_id,
                                    power_up_id: pu_id,
                                })
                                .padding(self.s(4.0) as u16)
                                .width(Length::Fixed(self.s(80.0)))
                                .into()
                            };
                            inv_row = inv_row.push(inv_btn);
                        }
                        panel_col = panel_col.push(inv_row);
                        panel_col = panel_col.push(Space::with_height(self.s(2.0)));
                    }
                }
            }
        }

        container(panel_col.padding(self.s(10.0) as u16))
            .width(Length::Fixed(panel_w))
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.05, 0.05, 0.15, 0.93))),
                border: iced::Border {
                    color: Color::from_rgba(0.4, 0.4, 0.8, 0.6),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_game_over(&self) -> Element<'_, Message> {
        let sc = self.scale();
        let result = match &self.game_result {
            Some(r) => r.clone(),
            None => {
                return column![
                    text("Игра завершена").size(self.ts(40)),
                    menu_button("В главное меню", Message::BackToMenu, sc),
                ]
                .spacing(16.0)
                .align_x(Alignment::Center)
                .into()
            }
        };

        let outcome_label: &'static str = if result.is_victory {
            "ПОБЕДА"
        } else {
            "ПОРАЖЕНИЕ"
        };
        let outcome_detail: String = if result.is_victory {
            "Достигнут целевой баланс".to_string()
        } else {
            match result.game_status.as_str() {
                "surrender" => "Вы сдались".to_string(),
                "finished" => "Закончились ходы".to_string(),
                other => other.to_string(),
            }
        };

        column![
            text(outcome_label).size(self.ts(56)),
            text(outcome_detail).size(self.ts(22)),
            Space::with_height(24),
            text("Итоги партии").size(18),
            Space::with_height(8),
            text(format!(
                "Баланс:          {} / {}",
                result.balance, result.target_balance
            ))
            .size(16),
            text(format!(
                "Ходов сыграно:   {} / {}",
                result.moves_made, result.max_turns
            ))
            .size(16),
            text(format!("Заработано:      {}", result.total_earned)).size(16),
            text(format!("Потрачено:       {}", result.total_spent)).size(16),
            Space::with_height(24),
            menu_button("В главное меню", Message::BackToMenu, sc),
        ]
        .spacing(8)
        .align_x(Alignment::Center)
        .padding(20)
        .into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let sc = self.scale();
        let back_label = if self.settings_from_game {
            "Назад к игре"
        } else {
            "Назад"
        };

        column![
            text("Настройки").size(self.ts(40)),
            Space::with_height(self.s(24.0)),
            text("Режим окна: ").size(self.ts(18)),
            Space::with_height(self.s(4.0)),
            pick_list(WINDOW_MODES, Some(self.window_mode), Message::SetWindowMode)
                .width(Length::Fixed(self.s(280.0)))
                .padding(self.s(8.0) as u16),
            Space::with_height(self.s(24.0)),
            menu_button(back_label, Message::BackToMenu, sc),
        ]
        .spacing(self.s(4.0) as u16)
        .align_x(Alignment::Center)
        .padding(self.s(20.0) as u16)
        .into()
    }
}

// ─────────────────────────────────────────────
// Вспомогательные UI функции
// ─────────────────────────────────────────────
#[derive(Debug, Clone, Copy)]
enum Icon {
    Play,
    Save,
    Settings,
    Close,
}

impl Icon {
    fn path(self) -> &'static str {
        match self {
            Icon::Play => "assets/play_btn.svg",
            Icon::Save => "assets/save_btn.svg",
            Icon::Settings => "assets/settings_btn.svg",
            Icon::Close => "assets/close_btn.svg",
        }
    }
}

fn icon(which: Icon, size: f32) -> iced::widget::Svg<'static> {
    svg(which.path())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

fn icon_button<'a>(which: Icon, label: &'a str, msg: Message, sc: f32) -> Button<'a, Message> {
    button(
        row![
            icon(which, 24.0 * sc),
            text(label).size((20.0 * sc).max(12.0))
        ]
        .spacing((10.0 * sc) as u16)
        .align_y(Alignment::Center),
    )
    .on_press(msg)
    .width(Length::Fixed(280.0 * sc))
    .padding((12.0 * sc) as u16)
}

fn menu_button<'a>(label: &'a str, msg: Message, sc: f32) -> Button<'a, Message> {
    button(text(label).size((20.0 * sc).max(12.0)))
        .on_press(msg)
        .width(Length::Fixed(280.0 * sc))
        .padding((12.0 * sc) as u16)
}

fn profile_dropdown_btn<'a>(label: &'a str, msg: Message, sc: f32) -> Button<'a, Message> {
    button(text(label).size((15.0 * sc).max(10.0)))
        .on_press(msg)
        .width(Length::Fixed(220.0 * sc))
        .padding((8.0 * sc) as u16)
}

/// Собирает SVG карточки собственности с вписанным названием и ценой.
///
/// Читает шаблон `assets/property_{upgrades}.svg` (0–3) и инжектирует два
/// `<text>` элемента прямо перед `</svg>`:
///   • название — верхняя зона (y≈280, над разделительной линией y≈296)
///   • value_label — нижняя зона (y≈507, под разделительной линией y≈496)
///
/// viewBox карточек: "256 256 256 256" → центр X = 256 + 128 = 384.
fn property_svg_handle(
    upgrades: usize,
    name: &str,
    value_label: &str,
    header_color: &str,        // hex цвет группы, например "#88ccff"
    owner_color: Option<&str>, // hex цвет владельца, None если свободна
) -> iced::widget::svg::Handle {
    let path = format!("assets/property_{}.svg", upgrades.min(3));
    let template = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"256 256 256 256\">\
         <rect x=\"256\" y=\"256\" width=\"254\" height=\"254\" fill=\"#d8d8d8\" stroke=\"#000\"/>\
         </svg>"
            .to_string()
    });

    let name_short: String = name.chars().take(8).collect();
    let value_short: String = value_label.chars().take(8).collect();

    // Верхняя зона (заголовок): y=256..296, высота=40
    // Нижняя зона владельца: от линии под ценой (y≈496) до края (512)
    let owner_rect = match owner_color {
        Some(oc) => format!(
            "<rect x=\"256\" y=\"496\" width=\"256\" height=\"16\" fill=\"{}\" opacity=\"0.9\"/>",
            oc
        ),
        None => String::new(),
    };

    let injection = format!(
        "<rect x=\"256\" y=\"256\" width=\"256\" height=\"40\" fill=\"{header_color}\" opacity=\"0.85\"/>\
         {owner_rect}\
         <text x=\"384\" y=\"276\" \
              text-anchor=\"middle\" dominant-baseline=\"middle\" \
              font-family=\"sans-serif\" font-size=\"22\" fill=\"#111\">{name}</text>\
         <text x=\"340\" y=\"490\" \
              text-anchor=\"middle\" dominant-baseline=\"middle\" \
              font-family=\"sans-serif\" font-size=\"20\" fill=\"#333\">{value}</text>",
        header_color = header_color,
        owner_rect = owner_rect,
        name = xml_escape(&name_short),
        value = xml_escape(&value_short),
    );

    let svg_with_text = template.replace("</svg>", &format!("{}</svg>", injection));
    iced::widget::svg::Handle::from_memory(svg_with_text.into_bytes())
}

/// Экранирует спецсимволы XML для безопасной вставки в SVG text.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
