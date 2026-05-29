//! Работа с базой данных DreamBreaker.
//!
//! Rust не пишет прямые INSERT/SELECT — вся логика инкапсулирована
//! в функциях PostgreSQL (migrations/4_functions.sql).
//! Здесь только: подключение, миграции и вызовы этих функций.

use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::types::Uuid;
use std::time::Duration;

// ----- Ошибки -----

#[derive(Debug, Clone)]
pub struct DbError(pub String);

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError(e.to_string())
    }
}
impl From<sqlx::migrate::MigrateError> for DbError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        DbError(e.to_string())
    }
}
impl From<bcrypt::BcryptError> for DbError {
    fn from(e: bcrypt::BcryptError) -> Self {
        DbError(format!("Bcrypt: {}", e))
    }
}

// ----- Модели -----

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub r#type: String,
    pub password_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Game {
    pub id: Uuid,
    pub status: String,
    pub seed: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_saved_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GameRules {
    pub game_id: Uuid,
    pub starting_balance: i64,
    pub max_turns: i32,
    pub target_balance: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GameParticipant {
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub position: i32,
    pub balance: i64,
    pub moves_made: i32,
    pub total_spent: i64,
    pub total_earned: i64,
    pub turn_order: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryItem {
    pub power_up_id: Uuid,
    pub name: String,
    pub quantity: i32,
    pub effect: serde_json::Value,
}

/// Клетка игрового поля со всеми связанными данными.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BoardCell {
    pub cell_index: i32,
    pub cell_type: String,
    pub tax_amount: Option<i64>,
    pub prop_name: Option<String>,
    pub purchase_cost: Option<i64>,
    pub rent_cost: Option<i64>,
    pub owner_user_id: Option<Uuid>,
    pub shop_id: Option<Uuid>,
    pub refresh_cost: Option<i64>,
}

/// Состояние игрока на экране игры.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ParticipantState {
    pub position: i32,
    pub balance: i64,
    pub moves_made: i32,
    pub total_spent: i64,
    pub total_earned: i64,
}

// ----- Подключение и миграции -----

pub async fn connect(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

// ----- БЛОК 1: Пользователи -----

/// Вызывает функцию БД: register_new_user(username, type, password_hash).
pub async fn register_user(pool: &PgPool, username: &str, password: &str) -> Result<Uuid, DbError> {
    let password_hash = hash(password, DEFAULT_COST)?;
    let row: (Uuid,) = sqlx::query_as("SELECT register_new_user($1, $2, $3)")
        .bind(username)
        .bind("human")
        .bind(password_hash)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Вызывает функции БД: get_user_credentials(), get_user_by_id().
pub async fn authenticate_user(
    pool: &PgPool,
    username: &str,
    password: &str,
) -> Result<User, DbError> {
    let creds: Option<(Uuid, String, bool)> =
        sqlx::query_as("SELECT user_id, pass_hash, active FROM get_user_credentials($1)")
            .bind(username)
            .fetch_optional(pool)
            .await?;

    let (user_id, pass_hash, is_active) =
        creds.ok_or_else(|| DbError("Пользователь не найден".to_string()))?;

    if !is_active {
        return Err(DbError("Пользователь заблокирован".to_string()));
    }

    if !verify(password, &pass_hash)? {
        return Err(DbError("Неверный пароль".to_string()));
    }

    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, type, password_hash, created_at, is_active
         FROM get_user_by_id($1)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Вызывает функцию БД: list_users().
pub async fn list_users(pool: &PgPool) -> Result<Vec<(Uuid, String)>, DbError> {
    let users: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, username FROM list_users()")
        .fetch_all(pool)
        .await?;
    Ok(users)
}

// ----- БЛОК 2: Игры -----

/// Создать новую игру с полем, ботами и участником за одну транзакцию.
/// Вызывает функцию БД: create_game_full(seed, balance, turns, target, user_id).
pub async fn create_game(
    pool: &PgPool,
    seed: i64,
    starting_balance: i64,
    max_turns: i32,
    target_balance: i64,
    user_id: Uuid,
) -> Result<Uuid, DbError> {
    let row: (Uuid,) = sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
        .bind(seed)
        .bind(starting_balance)
        .bind(max_turns)
        .bind(target_balance)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Вызывает функцию БД: get_game_rules(game_id).
pub async fn get_game_rules(pool: &PgPool, game_id: Uuid) -> Result<Option<GameRules>, DbError> {
    let rules = sqlx::query_as::<_, GameRules>(
        "SELECT game_id, starting_balance, max_turns, target_balance
         FROM get_game_rules($1)",
    )
    .bind(game_id)
    .fetch_optional(pool)
    .await?;
    Ok(rules)
}

/// Вызывает функцию БД: get_active_game(user_id).
pub async fn get_active_game_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<Uuid>, DbError> {
    let row: Option<(Option<Uuid>,)> = sqlx::query_as("SELECT get_active_game($1)")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|(id,)| id))
}

/// Вызывает функцию БД: get_user_games(user_id).
pub async fn get_user_games(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<(Uuid, String, i64, i32, chrono::DateTime<chrono::Utc>)>, DbError> {
    let games: Vec<(Uuid, String, i64, i32, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT game_id, status, balance, moves_made, created_at FROM get_user_games($1)",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(games)
}

/// Вызывает функцию БД: set_game_status(game_id, status).
pub async fn set_game_status(pool: &PgPool, game_id: Uuid, status: &str) -> Result<(), DbError> {
    sqlx::query("SELECT set_game_status($1, $2)")
        .bind(game_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

// ----- БЛОК 3: Игровое поле -----

/// Вызывает функцию БД: get_board_cells(game_id).
pub async fn get_board_cells(pool: &PgPool, game_id: Uuid) -> Result<Vec<BoardCell>, DbError> {
    let cells = sqlx::query_as::<_, BoardCell>(
        "SELECT cell_index, cell_type, tax_amount,
                prop_name, purchase_cost, rent_cost, owner_user_id,
                shop_id, refresh_cost
         FROM get_board_cells($1)",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;
    Ok(cells)
}

/// Вызывает функцию БД: get_participant_state(game_id, user_id).
pub async fn get_participant_state(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<Option<ParticipantState>, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM get_participant_state($1, $2)",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(state)
}

/// Загрузить всё нужное для экрана игры за один вызов.
pub async fn load_game_screen(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<
    (
        GameRules,
        ParticipantState,
        Vec<BoardCell>,
        Vec<InventoryItem>,
    ),
    DbError,
> {
    let rules = get_game_rules(pool, game_id)
        .await?
        .ok_or_else(|| DbError("Правила игры не найдены".to_string()))?;

    let state = get_participant_state(pool, game_id, user_id)
        .await?
        .ok_or_else(|| DbError("Участник не найден".to_string()))?;

    let cells = get_board_cells(pool, game_id).await?;
    let inventory = get_player_inventory(pool, game_id, user_id).await?;

    Ok((rules, state, cells, inventory))
}

// ----- БЛОК 4: Участники -----

/// Вызывает функцию БД: list_game_participants(game_id).
pub async fn list_participants(
    pool: &PgPool,
    game_id: Uuid,
) -> Result<Vec<GameParticipant>, DbError> {
    let participants = sqlx::query_as::<_, GameParticipant>(
        "SELECT game_id, user_id, \"position\", balance, moves_made,
                total_spent, total_earned, turn_order
         FROM list_game_participants($1)",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;
    Ok(participants)
}

// ----- БЛОК 5: Усиления и инвентарь -----

/// Вызывает функцию БД: add_to_inventory(game_id, user_id, power_up_id, quantity).
pub async fn add_to_inventory(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
    power_up_id: Uuid,
    quantity: i32,
) -> Result<(), DbError> {
    sqlx::query("SELECT add_to_inventory($1, $2, $3, $4)")
        .bind(game_id)
        .bind(user_id)
        .bind(power_up_id)
        .bind(quantity)
        .execute(pool)
        .await?;
    Ok(())
}

/// Вызывает функцию БД: get_player_inventory(game_id, user_id).
pub async fn get_player_inventory(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<InventoryItem>, DbError> {
    let items = sqlx::query_as::<_, InventoryItem>(
        "SELECT power_up_id, name, quantity, effect
         FROM get_player_inventory($1, $2)",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(items)
}

// ----- БЛОК 6: Игровые действия -----

/// Сохранить и выйти: статус -> paused.
/// Вызывает функцию БД: pause_game(game_id, user_id).
pub async fn pause_game(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> Result<(), DbError> {
    sqlx::query("SELECT pause_game($1, $2)")
        .bind(game_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Сдаться: статус -> surrender.
/// Вызывает функцию БД: surrender_game(game_id, user_id).
pub async fn surrender_game(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> Result<(), DbError> {
    sqlx::query("SELECT surrender_game($1, $2)")
        .bind(game_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ----- БЛОК 7: Статистика -----

/// Статистика профиля по завершённым играм.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserStats {
    pub total_games: i32,
    pub total_wins: i32,
    pub current_win_streak: i32,
    pub total_moves: i64,
    pub total_earned: i64,
    pub total_spent: i64,
    pub properties_bought: i64,
    pub power_ups_bought: i64,
}

/// Вызывает функцию БД: get_user_stats(user_id).
pub async fn get_user_stats(pool: &PgPool, user_id: Uuid) -> Result<UserStats, DbError> {
    let stats = sqlx::query_as::<_, UserStats>(
        "SELECT total_games, total_wins, current_win_streak,
                total_moves, total_earned, total_spent,
                properties_bought, power_ups_bought
         FROM get_user_stats($1)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(stats)
}

// ----- БЛОК 8: Результат игры -----

/// Итоги завершённой игры для экрана завершения.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GameResult {
    pub game_status: String,
    pub balance: i64,
    pub target_balance: i64,
    pub moves_made: i32,
    pub max_turns: i32,
    pub total_earned: i64,
    pub total_spent: i64,
    pub is_victory: bool,
}

/// Вызывает функцию БД: get_game_result(game_id, user_id).
pub async fn get_game_result(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<Option<GameResult>, DbError> {
    let result = sqlx::query_as::<_, GameResult>(
        "SELECT game_status, balance, target_balance, moves_made,
                max_turns, total_earned, total_spent, is_victory
         FROM get_game_result($1, $2)",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(result)
}

// ----- БЛОК 9: Последняя игра (для кнопки "Продолжить") -----

/// Вызывает функцию БД: get_latest_user_game(user_id).
/// Возвращает последнюю активную/приостановленную игру или None.
pub async fn get_latest_user_game(pool: &PgPool, user_id: Uuid) -> Result<Option<Uuid>, DbError> {
    let row: Option<(Uuid, String, i64, i32, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT game_id, status, balance, moves_made, created_at
             FROM get_latest_user_game($1)",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id, _, _, _, _)| id))
}
pub async fn commit_player_move(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
    new_position: i32,
) -> Result<ParticipantState, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM commit_player_move($1, $2, $3)",
    )
    .bind(game_id)
    .bind(user_id)
    .bind(new_position)
    .fetch_one(pool)
    .await?;
    Ok(state)
}
// ----- БЛОК 11: Проверка банкротства -----

/// Получить балансы всех участников игры (для проверки банкротства).
/// Возвращает (user_id, balance, user_type).
pub async fn get_all_balances(
    pool: &PgPool,
    game_id: Uuid,
) -> Result<Vec<(Uuid, i64, String)>, DbError> {
    let rows: Vec<(Uuid, i64, String)> = sqlx::query_as(
        "SELECT gp.user_id, gp.balance, u.type
         FROM game_participants gp
         JOIN users u ON u.id = gp.user_id
         WHERE gp.game_id = $1",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
