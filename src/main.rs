mod db;
use iced::widget::{
    button, column, container, mouse_area, pick_list, row, stack, svg, text, text_input, Button,
    Space,
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

// ─────────────────────────────────────────────
// State
// ─────────────────────────────────────────────
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

    // Экран завершения
    game_result: Option<db::GameResult>,
    user_stats: Option<db::UserStats>,
    show_stats: bool, // показывать ли оверлей статистики

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
            board_cells: vec![],
            inventory: vec![],
            game_menu_open: false,
            game_result: None,
            user_stats: None,
            show_stats: false,
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
    ContinueGame, // загрузить последнюю игру напрямую

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
        iced::event::listen_with(|event, _status, _id| match event {
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::WindowResized(size.width, size.height))
            }
            _ => None,
        })
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
                    Task::batch([
                        Task::perform(
                            async move { db::list_users(&pool).await },
                            Message::UsersLoaded,
                        ),
                        Task::perform(
                            async move { db::get_active_game_for_user(&pool2, user_id).await },
                            Message::ActiveGameChecked,
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
            Message::RegisterDone(Ok(_)) => {
                self.status = "Аккаунт создан!".to_string();
                self.screen = Screen::Login;
                self.input_password = String::new();
                self.input_password2 = String::new();
                self.form_error = String::new();
                Task::none()
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
            Message::GameScreenLoaded(Ok((rules, state, cells, inventory))) => {
                self.game_rules = Some(rules);
                self.player_state = Some(state);
                self.board_cells = cells;
                self.inventory = inventory;
                self.screen = Screen::Game;
                self.game_menu_open = false;
                self.status = "Игра загружена".to_string();
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
        }
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

        if self.show_stats {
            let overlay = self.view_stats_overlay();
            stack![with_game_menu, overlay].into()
        } else {
            with_game_menu
        }
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

        container(
            container(menu)
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
        let sc = self.scale();

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
            Space::with_height(self.s(8.0)),
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
        let mut col = column![text("Загрузить игру").size(self.ts(40))]
            .spacing(8)
            .align_x(Alignment::Center);

        if self.user_games.is_empty() {
            col = col.push(Space::with_height(12));
            col = col.push(text("Игр пока нет").size(self.ts(18)));
        } else {
            col = col.push(Space::with_height(12));
            col = col.push(
                row![
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
                    Space::with_width(Length::Fixed(110.0)),
                ]
                .spacing(8),
            );
            col = col.push(Space::with_height(4));

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
                    Space::with_width(Length::Fixed(110.0)).into()
                } else {
                    button(text("Загрузить").size(14))
                        .on_press(Message::LoadGame(*game_id))
                        .width(Length::Fixed(self.s(110.0)))
                        .padding(6)
                        .into()
                };

                col = col.push(
                    row![
                        text(name).size(14).width(Length::Fixed(180.0)),
                        text(status_ru).size(14).width(Length::Fixed(130.0)),
                        text(format!("{}", balance))
                            .size(14)
                            .width(Length::Fixed(80.0)),
                        text(format!("{}", moves))
                            .size(14)
                            .width(Length::Fixed(60.0)),
                        load_cell,
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                );
            }
        }

        col = col
            .push(Space::with_height(16))
            .push(menu_button("Назад", Message::BackToMenu, sc));
        col.into()
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

        let turns_left = rules.max_turns - state.moves_made;

        let slot_size = self.s(56.0);
        let mut inv_row = row![].spacing(self.s(3.0) as u16);
        for slot in 0..5usize {
            let cell: Element<'_, Message> = if slot < self.inventory.len() {
                let item = &self.inventory[slot];
                container(text(item.name.chars().take(8).collect::<String>()).size(self.ts(10)))
                    .width(Length::Fixed(slot_size))
                    .height(Length::Fixed(slot_size))
                    .padding(self.s(3.0) as u16)
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.2, 0.2, 0.3, 0.88))),
                        border: iced::Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.25),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            } else {
                container(text("—").size(self.ts(13)))
                    .width(Length::Fixed(slot_size))
                    .height(Length::Fixed(slot_size))
                    .padding(self.s(3.0) as u16)
                    .style(|_theme| container::Style {
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

        let stats_panel = container(
            column![
                text(format!("Баланс:   {}", state.balance)).size(self.ts(14)),
                text(format!("Цель:     {}", rules.target_balance)).size(self.ts(14)),
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

        let cell_size = self.s(54.0);
        let panel_offset = cell_size + self.s(6.0);

        let panel_overlay: Element<'_, Message> = container(container(stats_panel).padding(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: panel_offset,
                right: 0.0,
                bottom: 0.0,
                left: panel_offset,
            })
            .align_x(iced::alignment::Horizontal::Left)
            .align_y(iced::alignment::Vertical::Top)
            .into();

        let board = self.view_board(state.position);

        stack![board, panel_overlay]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_board(&self, player_pos: i32) -> Element<'_, Message> {
        const GRID: usize = 11;
        let mut grid: Vec<Vec<Option<i32>>> = vec![vec![None; GRID]; GRID];
        for i in 0..10usize {
            grid[10][10 - i] = Some(i as i32);
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

        let cell_map: std::collections::HashMap<i32, &db::BoardCell> =
            self.board_cells.iter().map(|c| (c.cell_index, c)).collect();

        let available_h = self.window_height - self.s(40.0) - self.s(28.0);
        let available_w = self.window_width;
        let cell_size = (available_h / 11.0).min(available_w / 11.0).floor();

        let mut board_col = column![].spacing(1);
        for row_i in 0..GRID {
            let mut board_row = row![].spacing(1);
            for col_i in 0..GRID {
                let cell_elem: Element<'_, Message> = match grid[row_i][col_i] {
                    None => Space::new(Length::Fixed(cell_size), Length::Fixed(cell_size)).into(),
                    Some(idx) => match cell_map.get(&idx) {
                        Some(cell) => self.view_board_cell(cell, player_pos == idx, cell_size),
                        None => {
                            Space::new(Length::Fixed(cell_size), Length::Fixed(cell_size)).into()
                        }
                    },
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

    fn view_board_cell(
        &self,
        cell: &db::BoardCell,
        is_player: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let fs = (size / 54.0 * 10.0).max(7.0);
        let fs_sm = (size / 54.0 * 9.0).max(6.0);

        let marker = if is_player {
            text("[ ]").size(fs)
        } else {
            text(" ").size(fs)
        };
        let (label, sublabel) = match cell.cell_type.as_str() {
            "start" => ("СТАРТ".to_string(), String::new()),
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
        };
        container(
            column![
                marker,
                text(label).size(fs),
                text(sublabel).size(fs_sm),
                if cell.owner_user_id.is_some() {
                    text("*").size(fs_sm)
                } else {
                    text(" ").size(fs_sm)
                },
            ]
            .align_x(Alignment::Center)
            .spacing(1)
            .padding(2),
        )
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
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
                .spacing(16)
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
            Space::with_height(24),
            text("Режим окна: ").size(16),
            Space::with_height(4),
            pick_list(WINDOW_MODES, Some(self.window_mode), Message::SetWindowMode)
                .width(Length::Fixed(280.0))
                .padding(8),
            Space::with_height(24),
            menu_button(back_label, Message::BackToMenu, sc),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
        .padding(20)
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
