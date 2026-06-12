#![allow(warnings)]
#![allow(clippy::all)]

use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::types::Uuid;
use std::time::Duration;

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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PlayerProperty {
    pub property_id: Uuid,
    pub cell_index: i32,
    pub prop_name: String,
    pub purchase_cost: i64,
    pub rent_cost: i64,
    pub upgrades: serde_json::Value,
    pub upgrades_count: i32,
    pub max_upgrades: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ParticipantState {
    pub position: i32,
    pub balance: i64,
    pub moves_made: i32,
    pub total_spent: i64,
    pub total_earned: i64,
}

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
pub async fn get_user_by_id(pool: &PgPool, user_id: Uuid) -> Result<Option<User>, DbError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, type, password_hash, created_at, is_active
         FROM get_user_by_id($1)",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn list_users(pool: &PgPool) -> Result<Vec<(Uuid, String)>, DbError> {
    let users: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, username FROM list_users()")
        .fetch_all(pool)
        .await?;
    Ok(users)
}

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

pub async fn set_game_status(pool: &PgPool, game_id: Uuid, status: &str) -> Result<(), DbError> {
    sqlx::query("SELECT set_game_status($1, $2)")
        .bind(game_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

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
        Vec<PlayerProperty>,
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
    let properties = get_player_properties(pool, game_id, user_id).await?;

    Ok((rules, state, cells, inventory, properties))
}

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

pub async fn pause_game(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> Result<(), DbError> {
    sqlx::query("SELECT pause_game($1, $2)")
        .bind(game_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn surrender_game(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> Result<(), DbError> {
    sqlx::query("SELECT surrender_game($1, $2)")
        .bind(game_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

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

pub async fn get_all_balances(
    pool: &PgPool,
    game_id: Uuid,
) -> Result<Vec<(Uuid, i64, String)>, DbError> {
    let rows: Vec<(Uuid, i64, String)> =
        sqlx::query_as("SELECT user_id, balance, user_type FROM get_all_balances($1)")
            .bind(game_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

pub async fn buy_property(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
    cell_index: i32,
) -> Result<ParticipantState, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM buy_property($1, $2, $3)",
    )
    .bind(game_id)
    .bind(user_id)
    .bind(cell_index)
    .fetch_one(pool)
    .await?;
    Ok(state)
}

pub async fn pay_rent(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
    cell_index: i32,
) -> Result<ParticipantState, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM pay_rent($1, $2, $3)",
    )
    .bind(game_id)
    .bind(user_id)
    .bind(cell_index)
    .fetch_one(pool)
    .await?;
    Ok(state)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShopSlot {
    pub slot_index: i32,
    pub slot_id: Uuid,
    pub power_up_id: Uuid,
    pub name: String,
    pub description: String,
    pub cost: i64,
    pub status: String,
    pub already_own: bool,
    pub reroll_count: i32,
}

pub async fn get_shop_slots(
    pool: &PgPool,
    shop_id: Uuid,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<ShopSlot>, DbError> {
    let slots = sqlx::query_as::<_, ShopSlot>(
        "SELECT slot_index, slot_id, power_up_id, name, description,
                cost, status, already_own, reroll_count
         FROM get_shop_slots($1, $2, $3)",
    )
    .bind(shop_id)
    .bind(game_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(slots)
}

pub async fn buy_shop_slot(
    pool: &PgPool,
    slot_id: Uuid,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<ParticipantState, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM buy_shop_slot($1, $2, $3)",
    )
    .bind(slot_id)
    .bind(game_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(state)
}

pub async fn reroll_shop(
    pool: &PgPool,
    shop_id: Uuid,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<ParticipantState, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM reroll_shop($1, $2, $3)",
    )
    .bind(shop_id)
    .bind(game_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(state)
}

pub async fn get_player_properties(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<PlayerProperty>, DbError> {
    let props = sqlx::query_as::<_, PlayerProperty>(
        "SELECT property_id, cell_index, prop_name, purchase_cost, rent_cost,
                upgrades, upgrades_count, max_upgrades
         FROM get_player_properties($1, $2)",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(props)
}

pub async fn install_upgrade(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
    property_id: Uuid,
    power_up_id: Uuid,
) -> Result<String, DbError> {
    let row: (String,) = sqlx::query_as("SELECT install_upgrade($1, $2, $3, $4)")
        .bind(game_id)
        .bind(user_id)
        .bind(property_id)
        .bind(power_up_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn uninstall_upgrade(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
    property_id: Uuid,
    power_up_id: Uuid,
) -> Result<String, DbError> {
    let row: (String,) = sqlx::query_as("SELECT uninstall_upgrade($1, $2, $3, $4)")
        .bind(game_id)
        .bind(user_id)
        .bind(property_id)
        .bind(power_up_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotParticipant {
    pub user_id: Uuid,
    pub username: String,
    pub position: i32,
    pub balance: i64,
    pub turn_order: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BotTurnResult {
    pub new_position: i32,
    pub new_balance: i64,
    pub action: String,
    pub action_detail: String,
}

pub async fn get_bot_participants(
    pool: &PgPool,
    game_id: Uuid,
) -> Result<Vec<BotParticipant>, DbError> {
    let bots = sqlx::query_as::<_, BotParticipant>(
        r#"SELECT user_id, username, "position", balance, turn_order
           FROM get_bot_participants($1)"#,
    )
    .bind(game_id)
    .fetch_all(pool)
    .await?;
    Ok(bots)
}

pub async fn do_bot_turn(
    pool: &PgPool,
    game_id: Uuid,
    bot_id: Uuid,
    dice: i32,
) -> Result<BotTurnResult, DbError> {
    let result = sqlx::query_as::<_, BotTurnResult>(
        r#"SELECT new_position, new_balance, action, action_detail
           FROM do_bot_turn($1, $2, $3)"#,
    )
    .bind(game_id)
    .bind(bot_id)
    .bind(dice)
    .fetch_one(pool)
    .await?;
    Ok(result)
}

pub async fn pay_tax(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
) -> Result<ParticipantState, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM pay_tax($1, $2)",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(state)
}

pub async fn sell_power_up(
    pool: &PgPool,
    game_id: Uuid,
    user_id: Uuid,
    power_up_id: Uuid,
) -> Result<ParticipantState, DbError> {
    let state = sqlx::query_as::<_, ParticipantState>(
        "SELECT \"position\", balance, moves_made, total_spent, total_earned
         FROM sell_power_up($1, $2, $3)",
    )
    .bind(game_id)
    .bind(user_id)
    .bind(power_up_id)
    .fetch_one(pool)
    .await?;
    Ok(state)
}
pub async fn reset_stale_active_games(pool: &PgPool, user_id: Uuid) -> Result<(), DbError> {
    sqlx::query("SELECT reset_stale_active_games($1)")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
