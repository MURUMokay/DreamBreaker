use sqlx::postgres::PgPool;

// ── Подключение к тестовой БД ────────────────────────────
pub async fn setup_db() -> PgPool {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL не задана");
    PgPool::connect(&url)
        .await
        .expect("Не удалось подключиться к БД")
}

// ── Очистка тестовых данных после каждого теста ──────────
pub async fn cleanup(pool: &PgPool, username: &str) {
    sqlx::query("DELETE FROM users WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await
        .ok();
}

// ════════════════════════════════════════════════════════
// БЛОК 1: Авторизация и регистрация
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_auth {
    use super::*;

    // Тест 1: регистрация нового пользователя
    #[tokio::test]
    async fn test_register_new_user() {
        let pool = setup_db().await;
        let username = "test_user_reg";
        cleanup(&pool, username).await;

        let result: Result<(uuid::Uuid,), _> =
            sqlx::query_as("SELECT register_new_user($1, $2, $3)")
                .bind(username)
                .bind("human")
                .bind("hash_placeholder")
                .fetch_one(&pool)
                .await;

        assert!(
            result.is_ok(),
            "Регистрация нового пользователя провалилась"
        );
        cleanup(&pool, username).await;
    }

    // Тест 2: повторная регистрация с тем же именем
    #[tokio::test]
    async fn test_register_duplicate_user() {
        let pool = setup_db().await;
        let username = "test_user_dup";
        cleanup(&pool, username).await;

        // Первая регистрация
        sqlx::query("SELECT register_new_user($1, $2, $3)")
            .bind(username)
            .bind("human")
            .bind("hash1")
            .execute(&pool)
            .await
            .ok();

        // Вторая — должна упасть
        let result: Result<(uuid::Uuid,), _> =
            sqlx::query_as("SELECT register_new_user($1, $2, $3)")
                .bind(username)
                .bind("human")
                .bind("hash2")
                .fetch_one(&pool)
                .await;

        assert!(
            result.is_err(),
            "Повторная регистрация должна возвращать ошибку"
        );
        cleanup(&pool, username).await;
    }

    // Тест 3: получение credentials существующего пользователя
    #[tokio::test]
    async fn test_get_credentials_existing() {
        let pool = setup_db().await;
        let username = "test_user_creds";
        cleanup(&pool, username).await;

        sqlx::query("SELECT register_new_user($1, $2, $3)")
            .bind(username)
            .bind("human")
            .bind("testhash")
            .execute(&pool)
            .await
            .ok();

        let result: Result<(uuid::Uuid, String, bool), _> =
            sqlx::query_as("SELECT user_id, pass_hash, active FROM get_user_credentials($1)")
                .bind(username)
                .fetch_one(&pool)
                .await;

        assert!(result.is_ok(), "Credentials должны находиться");
        let (_, hash, active) = result.unwrap();
        assert_eq!(hash, "testhash");
        assert!(active, "Пользователь должен быть активен");
        cleanup(&pool, username).await;
    }

    // Тест 4: credentials несуществующего пользователя
    #[tokio::test]
    async fn test_get_credentials_nonexistent() {
        let pool = setup_db().await;

        let result: Result<(uuid::Uuid, String, bool), _> =
            sqlx::query_as("SELECT user_id, pass_hash, active FROM get_user_credentials($1)")
                .bind("nobody_xyz_12345")
                .fetch_one(&pool)
                .await;

        assert!(
            result.is_err(),
            "Несуществующий пользователь должен давать пустой результат"
        );
    }
}
// ════════════════════════════════════════════════════════
// БЛОК 2: Управление партиями
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_games {
    use super::*;

    async fn create_test_user(pool: &PgPool, username: &str) -> uuid::Uuid {
        cleanup(pool, username).await;
        let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT register_new_user($1, $2, $3)")
            .bind(username)
            .bind("human")
            .bind("testhash")
            .fetch_one(pool)
            .await
            .expect("Не удалось создать тестового пользователя");
        id
    }

    async fn cleanup_game(pool: &PgPool, game_id: uuid::Uuid) {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(game_id)
            .execute(pool)
            .await
            .ok();
    }

    // Тест 5: создание партии с параметрами по умолчанию
    #[tokio::test]
    async fn test_create_game_default() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_game_user1").await;

        let result: Result<(uuid::Uuid,), _> =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(12345_i64) // seed
                .bind(1000_i64) // starting_balance
                .bind(50_i32) // max_turns
                .bind(2500_i64) // target_balance
                .bind(user_id)
                .fetch_one(&pool)
                .await;

        assert!(result.is_ok(), "Создание партии должно проходить успешно");
        let (game_id,) = result.unwrap();

        // Проверяем что игрок добавлен с правильным балансом
        let balance: (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("Участник не найден");

        assert_eq!(balance.0, 1000, "Стартовый баланс должен быть 1000");

        cleanup_game(&pool, game_id).await;
        cleanup(&pool, "test_game_user1").await;
    }

    // Тест 6: создание партии со своими параметрами
    #[tokio::test]
    async fn test_create_game_custom_params() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_game_user2").await;

        let (game_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(99999_i64)
                .bind(500_i64)
                .bind(30_i32)
                .bind(5000_i64)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("Создание партии провалилось");

        // Проверяем правила игры
        let rules: (i64, i32, i64) = sqlx::query_as(
            "SELECT starting_balance, max_turns, target_balance
                 FROM game_rules WHERE game_id = $1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .expect("Правила игры не найдены");

        assert_eq!(rules.0, 500, "starting_balance должен быть 500");
        assert_eq!(rules.1, 30, "max_turns должен быть 30");
        assert_eq!(rules.2, 5000, "target_balance должен быть 5000");

        cleanup_game(&pool, game_id).await;
        cleanup(&pool, "test_game_user2").await;
    }

    // Тест 7: поле инициализировано (40 клеток)
    #[tokio::test]
    async fn test_board_has_40_cells() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_game_user3").await;

        let (game_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(77777_i64)
                .bind(1000_i64)
                .bind(50_i32)
                .bind(2500_i64)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("Создание партии провалилось");

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM game_cells WHERE game_id = $1")
            .bind(game_id)
            .fetch_one(&pool)
            .await
            .expect("Ошибка подсчёта клеток");

        assert_eq!(count, 40, "Игровое поле должно содержать ровно 40 клеток");

        cleanup_game(&pool, game_id).await;
        cleanup(&pool, "test_game_user3").await;
    }

    // Тест 8: загрузка сохранённой партии (статус paused)
    #[tokio::test]
    async fn test_load_paused_game() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_game_user4").await;

        let (game_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(11111_i64)
                .bind(1000_i64)
                .bind(50_i32)
                .bind(2500_i64)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("Создание партии провалилось");

        // Ставим статус paused
        sqlx::query("SELECT pause_game($1, $2)")
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("pause_game провалился");

        // Проверяем статус
        let (status,): (String,) = sqlx::query_as("SELECT status FROM games WHERE id = $1")
            .bind(game_id)
            .fetch_one(&pool)
            .await
            .expect("Игра не найдена");

        assert_eq!(status, "paused", "Статус должен быть paused");

        cleanup_game(&pool, game_id).await;
        cleanup(&pool, "test_game_user4").await;
    }
}
// ════════════════════════════════════════════════════════
// БЛОК 3: Игровой ход
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_gameplay {
    use super::*;

    async fn create_test_user(pool: &PgPool, username: &str) -> uuid::Uuid {
        cleanup(pool, username).await;
        let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT register_new_user($1, $2, $3)")
            .bind(username)
            .bind("human")
            .bind("testhash")
            .fetch_one(pool)
            .await
            .expect("Не удалось создать пользователя");
        id
    }

    async fn create_test_game(pool: &PgPool, user_id: uuid::Uuid) -> uuid::Uuid {
        let (game_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(42_i64)
                .bind(1000_i64)
                .bind(50_i32)
                .bind(2500_i64)
                .bind(user_id)
                .fetch_one(pool)
                .await
                .expect("Не удалось создать партию");
        game_id
    }

    async fn cleanup_all(pool: &PgPool, game_id: uuid::Uuid, username: &str) {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(game_id)
            .execute(pool)
            .await
            .ok();
        cleanup(pool, username).await;
    }

    // Тест 9: перемещение игрока обновляет позицию
    #[tokio::test]
    async fn test_commit_player_move_updates_position() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_move_user1").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Перемещаем на позицию 5
        let result: Result<(i32, i64, i32), _> = sqlx::query_as(
            "SELECT position, balance, moves_made
                 FROM commit_player_move($1, $2, $3)",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(5_i32)
        .fetch_one(&pool)
        .await;

        assert!(
            result.is_ok(),
            "commit_player_move должен выполняться без ошибок"
        );
        let (pos, _, moves) = result.unwrap();
        assert_eq!(pos, 5, "Позиция должна быть 5");
        assert_eq!(moves, 1, "moves_made должен стать 1");

        cleanup_all(&pool, game_id, "test_move_user1").await;
    }

    // Тест 10: бонус при прохождении стартовой клетки
    #[tokio::test]
    async fn test_start_bonus_on_pass() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_move_user2").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Сначала ставим игрока на позицию 38
        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(38_i32)
            .execute(&pool)
            .await
            .ok();

        let (bal_before,): (i64,) = sqlx::query_as(
            "SELECT balance FROM game_participants
                 WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("Участник не найден");

        // Перемещаем на позицию 2 (прошли старт)
        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(2_i32)
            .execute(&pool)
            .await
            .ok();

        let (bal_after,): (i64,) = sqlx::query_as(
            "SELECT balance FROM game_participants
                 WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("Участник не найден");

        assert!(
            bal_after > bal_before,
            "Баланс должен вырасти после прохождения старта"
        );

        cleanup_all(&pool, game_id, "test_move_user2").await;
    }

    // Тест 11: покупка свободной собственности уменьшает баланс
    #[tokio::test]
    async fn test_buy_property_decreases_balance() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_move_user3").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Находим первую клетку-собственность
        let prop: Option<(i32, i64)> = sqlx::query_as(
            "SELECT gc.cell_index, p.purchase_cost
                 FROM game_cells gc
                 JOIN properties p ON p.id = gc.property_id
                 WHERE gc.game_id = $1
                   AND gc.cell_type = 'property'
                   AND p.owner_user_id IS NULL
                 ORDER BY gc.cell_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_optional(&pool)
        .await
        .expect("Ошибка поиска собственности");

        let (cell_index, cost) = prop.expect("Нет свободных собственностей");

        // Перемещаем игрока на эту клетку
        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .ok();

        let (bal_before,): (i64,) = sqlx::query_as(
            "SELECT balance FROM game_participants
                 WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        // Покупаем
        sqlx::query("SELECT buy_property($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .expect("buy_property провалился");

        let (bal_after,): (i64,) = sqlx::query_as(
            "SELECT balance FROM game_participants
                 WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            bal_after,
            bal_before - cost,
            "Баланс должен уменьшиться на стоимость покупки"
        );

        cleanup_all(&pool, game_id, "test_move_user3").await;
    }

    // Тест 12: выплата ренты списывает баланс и зачисляет владельцу
    #[tokio::test]
    async fn test_pay_rent_transfers_balance() {
        let pool = setup_db().await;
        let owner_id = create_test_user(&pool, "test_rent_owner").await;
        let renter_id = create_test_user(&pool, "test_rent_renter").await;
        let game_id = create_test_game(&pool, owner_id).await;

        // Добавляем арендатора в игру
        sqlx::query(
            "INSERT INTO game_participants
             (game_id, user_id, balance, moves_made, total_spent, total_earned, turn_order)
             VALUES ($1, $2, 1000, 0, 0, 0, 5)",
        )
        .bind(game_id)
        .bind(renter_id)
        .execute(&pool)
        .await
        .expect("Не удалось добавить арендатора");

        // Владелец покупает первую собственность
        let (cell_index,): (i32,) = sqlx::query_as(
            "SELECT gc.cell_index FROM game_cells gc
                 JOIN properties p ON p.id = gc.property_id
                 WHERE gc.game_id=$1 AND gc.cell_type='property'
                   AND p.owner_user_id IS NULL
                 ORDER BY gc.cell_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(owner_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("SELECT buy_property($1, $2, $3)")
            .bind(game_id)
            .bind(owner_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .ok();

        // Арендатор попадает на ту же клетку
        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(renter_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .ok();

        let (owner_before,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(owner_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        let (renter_before,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(renter_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        // Платим ренту
        sqlx::query("SELECT pay_rent($1, $2, $3)")
            .bind(game_id)
            .bind(renter_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .expect("pay_rent провалился");

        let (owner_after,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(owner_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        let (renter_after,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(renter_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert!(
            renter_after < renter_before,
            "Баланс арендатора должен уменьшиться"
        );
        assert!(
            owner_after > owner_before,
            "Баланс владельца должен увеличиться"
        );

        cleanup_all(&pool, game_id, "test_rent_owner").await;
        cleanup(&pool, "test_rent_renter").await;
    }
}
// ════════════════════════════════════════════════════════
// БЛОК 4: Магазин усилений
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_shop {
    use super::*;

    async fn create_test_user(pool: &PgPool, username: &str) -> uuid::Uuid {
        cleanup(pool, username).await;
        let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT register_new_user($1, $2, $3)")
            .bind(username)
            .bind("human")
            .bind("testhash")
            .fetch_one(pool)
            .await
            .expect("Не удалось создать пользователя");
        id
    }

    async fn create_test_game(pool: &PgPool, user_id: uuid::Uuid) -> uuid::Uuid {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM power_ups")
            .fetch_one(pool)
            .await
            .unwrap();
        assert!(count > 0, "power_ups пуста – проверь миграции тестовой БД");
        let (game_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(42_i64)
                .bind(1000_i64)
                .bind(50_i32)
                .bind(2500_i64)
                .bind(user_id)
                .fetch_one(pool)
                .await
                .expect("Не удалось создать партию");
        game_id
    }

    async fn cleanup_all(pool: &PgPool, game_id: uuid::Uuid, username: &str) {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(game_id)
            .execute(pool)
            .await
            .ok();
        cleanup(pool, username).await;
    }

    async fn get_first_valid_slot(pool: &PgPool, game_id: uuid::Uuid) -> (uuid::Uuid, i64) {
        // Сначала выводим все слоты для диагностики
        let all: Vec<(i32, Option<uuid::Uuid>, i64, String)> = sqlx::query_as(
            "SELECT ss.slot_index, ss.power_up_id, ss.cost, ss.status
                 FROM shop_slots ss
                 JOIN shops s ON s.id = ss.shop_id
                 WHERE s.game_id = $1
                 ORDER BY ss.slot_index",
        )
        .bind(game_id)
        .fetch_all(pool)
        .await
        .unwrap();
        println!("Слоты партии {}: {:?}", game_id, all);

        // Берём слот с ненулевым power_up_id
        let slot: Option<(uuid::Uuid, i64)> = sqlx::query_as(
            "SELECT ss.id, ss.cost
                 FROM shop_slots ss
                 JOIN shops s ON s.id = ss.shop_id
                 WHERE s.game_id = $1
                   AND ss.status = 'available'
                   AND ss.power_up_id IS NOT NULL
                 ORDER BY ss.slot_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_optional(pool)
        .await
        .expect("Ошибка поиска слота");

        slot.expect("Нет доступных слотов с ненулевым power_up_id")
    }

    // Тест 13: магазин содержит слоты после инициализации поля
    #[tokio::test]
    async fn test_shop_has_slots() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_shop_user1").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM shop_slots ss
                 JOIN shops s ON s.id = ss.shop_id
                 WHERE s.game_id = $1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .expect("Ошибка подсчёта слотов");

        assert!(
            count > 0,
            "Магазин должен содержать слоты после создания партии"
        );

        cleanup_all(&pool, game_id, "test_shop_user1").await;
    }

    // Тест 14: покупка усиления из магазина уменьшает баланс
    #[tokio::test(flavor = "current_thread")]
    async fn test_buy_power_up_decreases_balance() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_shop_user2").await;
        let game_id = create_test_game(&pool, user_id).await;

        let slot: Option<(uuid::Uuid, i64)> = sqlx::query_as(
            "SELECT ss.id, ss.cost
             FROM shop_slots ss
             JOIN shops s ON s.id = ss.shop_id
             WHERE s.game_id = $1
               AND ss.status = 'available'
               AND ss.power_up_id IS NOT NULL
             ORDER BY ss.slot_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_optional(&pool)
        .await
        .expect("Ошибка поиска слота");

        let (slot_id, cost) = slot.expect("Нет доступных слотов");

        let (bal_before,): (i64,) = sqlx::query_as(
            "SELECT balance FROM game_participants
             WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query("SELECT buy_shop_slot($1, $2, $3)")
            .bind(slot_id)
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("buy_shop_slot провалился");

        let (bal_after,): (i64,) = sqlx::query_as(
            "SELECT balance FROM game_participants
             WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            bal_after,
            bal_before - cost,
            "Баланс должен уменьшиться на стоимость усиления"
        );

        cleanup_all(&pool, game_id, "test_shop_user2").await;
    }

    // Тест 15: купленное усиление появляется в инвентаре
    #[tokio::test(flavor = "current_thread")]
    async fn test_buy_power_up_adds_to_inventory() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_shop_user3").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Берём slot_id напрямую из БД по game_id этой партии
        let slot: Option<(uuid::Uuid, i64)> = sqlx::query_as(
            "SELECT ss.id, ss.cost
             FROM shop_slots ss
             JOIN shops s ON s.id = ss.shop_id
             WHERE s.game_id = $1
               AND ss.status = 'available'
               AND ss.power_up_id IS NOT NULL
             ORDER BY ss.slot_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_optional(&pool)
        .await
        .expect("Ошибка поиска слота");

        let (slot_id, _cost) = slot.expect("Нет доступных слотов с ненулевым power_up_id");

        println!("Тест 15: game_id={}, slot_id={}", game_id, slot_id);

        sqlx::query("SELECT buy_shop_slot($1, $2, $3)")
            .bind(slot_id)
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("buy_shop_slot провалился");

        let (inv_count,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(quantity), 0)
             FROM player_inventory
             WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            inv_count > 0,
            "После покупки инвентарь не должен быть пустым"
        );

        cleanup_all(&pool, game_id, "test_shop_user3").await;
    }

    // Тест 16: покупка при нехватке баланса возвращает ошибку
    #[tokio::test]
    async fn test_buy_power_up_insufficient_balance() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_shop_user4").await;
        let game_id = create_test_game(&pool, user_id).await;

        sqlx::query(
            "UPDATE game_participants SET balance = 0
             WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .ok();

        let slot: Option<(uuid::Uuid,)> = sqlx::query_as(
            "SELECT ss.id FROM shop_slots ss
                 JOIN shops s ON s.id = ss.shop_id
                 WHERE s.game_id=$1
                   AND ss.status='available'
                   AND ss.power_up_id IS NOT NULL
                 ORDER BY ss.slot_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_optional(&pool)
        .await
        .unwrap();

        let (slot_id,) = slot.expect("Нет доступных слотов");

        let result = sqlx::query("SELECT buy_shop_slot($1, $2, $3)")
            .bind(slot_id)
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await;

        assert!(
            result.is_err(),
            "Покупка при нулевом балансе должна возвращать ошибку"
        );

        cleanup_all(&pool, game_id, "test_shop_user4").await;
    }
}
// ════════════════════════════════════════════════════════
// БЛОК 5: Управление собственностью и усилениями
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_upgrades {
    use super::*;

    async fn create_test_user(pool: &PgPool, username: &str) -> uuid::Uuid {
        cleanup(pool, username).await;
        let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT register_new_user($1, $2, $3)")
            .bind(username)
            .bind("human")
            .bind("testhash")
            .fetch_one(pool)
            .await
            .expect("Не удалось создать пользователя");
        id
    }

    async fn create_test_game(pool: &PgPool, user_id: uuid::Uuid) -> uuid::Uuid {
        let (game_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(42_i64)
                .bind(1000_i64)
                .bind(50_i32)
                .bind(2500_i64)
                .bind(user_id)
                .fetch_one(pool)
                .await
                .expect("Не удалось создать партию");
        game_id
    }

    async fn cleanup_all(pool: &PgPool, game_id: uuid::Uuid, username: &str) {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(game_id)
            .execute(pool)
            .await
            .ok();
        cleanup(pool, username).await;
    }

    // Покупаем первое доступное усиление из магазина
    async fn buy_first_power_up(
        pool: &PgPool,
        game_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> uuid::Uuid {
        let (slot_id,): (uuid::Uuid,) = sqlx::query_as(
            "SELECT ss.id FROM shop_slots ss
                 JOIN shops s ON s.id = ss.shop_id
                 WHERE s.game_id = $1
                   AND ss.status = 'available'
                   AND ss.power_up_id IS NOT NULL
                 ORDER BY ss.slot_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_one(pool)
        .await
        .expect("Нет доступных слотов");

        sqlx::query("SELECT buy_shop_slot($1, $2, $3)")
            .bind(slot_id)
            .bind(game_id)
            .bind(user_id)
            .execute(pool)
            .await
            .expect("buy_shop_slot провалился");

        // Возвращаем power_up_id из инвентаря
        let (pu_id,): (uuid::Uuid,) = sqlx::query_as(
            "SELECT power_up_id FROM player_inventory
                 WHERE game_id=$1 AND user_id=$2
                 LIMIT 1",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("Усиление не найдено в инвентаре");

        pu_id
    }

    // Покупаем первую свободную собственность
    async fn buy_first_property(
        pool: &PgPool,
        game_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> (uuid::Uuid, i32) {
        let (cell_index,): (i32,) = sqlx::query_as(
            "SELECT gc.cell_index FROM game_cells gc
                 JOIN properties p ON p.id = gc.property_id
                 WHERE gc.game_id = $1
                   AND gc.cell_type = 'property'
                   AND p.owner_user_id IS NULL
                 ORDER BY gc.cell_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_one(pool)
        .await
        .expect("Нет свободных собственностей");

        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(pool)
            .await
            .ok();

        sqlx::query("SELECT buy_property($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(pool)
            .await
            .expect("buy_property провалился");

        let (prop_id,): (uuid::Uuid,) = sqlx::query_as(
            "SELECT p.id FROM properties p
                 JOIN game_cells gc ON gc.property_id = p.id
                 WHERE gc.game_id = $1 AND p.owner_user_id = $2
                 LIMIT 1",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("Собственность не найдена");

        (prop_id, cell_index)
    }

    // Тест 17: установка усиления на собственность
    #[tokio::test]
    async fn test_install_upgrade() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_upg_user1").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (prop_id, _) = buy_first_property(&pool, game_id, user_id).await;
        let pu_id = buy_first_power_up(&pool, game_id, user_id).await;

        let result: (String,) = sqlx::query_as("SELECT install_upgrade($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(prop_id)
            .bind(pu_id)
            .fetch_one(&pool)
            .await
            .expect("install_upgrade провалился");

        assert_eq!(result.0, "ok", "install_upgrade должен вернуть 'ok'");

        // Проверяем что усиление появилось в upgrades собственности
        let (upg_count,): (i32,) = sqlx::query_as(
            "SELECT jsonb_array_length(upgrades)
                 FROM properties WHERE id = $1",
        )
        .bind(prop_id)
        .fetch_one(&pool)
        .await
        .expect("Ошибка чтения upgrades");

        assert_eq!(upg_count, 1, "В собственности должно быть 1 усиление");

        cleanup_all(&pool, game_id, "test_upg_user1").await;
    }

    // Тест 18: усиление списывается из инвентаря после установки
    #[tokio::test]
    async fn test_install_upgrade_removes_from_inventory() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_upg_user2").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (prop_id, _) = buy_first_property(&pool, game_id, user_id).await;
        let pu_id = buy_first_power_up(&pool, game_id, user_id).await;

        let (qty_before,): (i32,) = sqlx::query_as(
            "SELECT COALESCE(quantity, 0) FROM player_inventory
                 WHERE game_id=$1 AND user_id=$2 AND power_up_id=$3",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(pu_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        sqlx::query("SELECT install_upgrade($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(prop_id)
            .bind(pu_id)
            .execute(&pool)
            .await
            .expect("install_upgrade провалился");

        let qty_after: Option<(i32,)> = sqlx::query_as(
            "SELECT quantity FROM player_inventory
                 WHERE game_id=$1 AND user_id=$2 AND power_up_id=$3",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(pu_id)
        .fetch_optional(&pool)
        .await
        .unwrap();

        let qty_after_val = qty_after.map(|(q,)| q).unwrap_or(0);
        assert!(
            qty_after_val < qty_before,
            "Количество усилений в инвентаре должно уменьшиться после установки"
        );

        cleanup_all(&pool, game_id, "test_upg_user2").await;
    }

    // Тест 19: снятие усиления возвращает его в инвентарь
    #[tokio::test]
    async fn test_uninstall_upgrade_returns_to_inventory() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_upg_user3").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (prop_id, _) = buy_first_property(&pool, game_id, user_id).await;
        let pu_id = buy_first_power_up(&pool, game_id, user_id).await;

        // Устанавливаем
        sqlx::query("SELECT install_upgrade($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(prop_id)
            .bind(pu_id)
            .execute(&pool)
            .await
            .expect("install_upgrade провалился");

        // Снимаем
        let result: (String,) = sqlx::query_as("SELECT uninstall_upgrade($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(prop_id)
            .bind(pu_id)
            .fetch_one(&pool)
            .await
            .expect("uninstall_upgrade провалился");

        assert_eq!(result.0, "ok", "uninstall_upgrade должен вернуть 'ok'");

        // Усиление должно вернуться в инвентарь
        let (qty,): (i32,) = sqlx::query_as(
            "SELECT COALESCE(quantity, 0) FROM player_inventory
                 WHERE game_id=$1 AND user_id=$2 AND power_up_id=$3",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(pu_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        assert!(
            qty >= 1,
            "После снятия усиление должно вернуться в инвентарь"
        );

        // upgrades собственности должен быть пуст
        let (upg_count,): (i32,) =
            sqlx::query_as("SELECT jsonb_array_length(upgrades) FROM properties WHERE id = $1")
                .bind(prop_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(upg_count, 0, "После снятия upgrades должен быть пуст");

        cleanup_all(&pool, game_id, "test_upg_user3").await;
    }

    // Тест 20: превышение лимита слотов (максимум 3)
    #[tokio::test]
    async fn test_install_upgrade_slot_limit() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_upg_user4").await;
        // Даём большой баланс чтобы хватило на 4 покупки
        let (game_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(42_i64)
                .bind(5000_i64)
                .bind(50_i32)
                .bind(99999_i64)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("Не удалось создать партию");

        let (prop_id, _) = buy_first_property(&pool, game_id, user_id).await;

        // Покупаем и устанавливаем 3 разных усиления
        let pu_ids: Vec<uuid::Uuid> =
            sqlx::query_as("SELECT id FROM power_ups ORDER BY cost LIMIT 4")
                .fetch_all(&pool)
                .await
                .unwrap()
                .into_iter()
                .map(|(id,)| id)
                .collect();

        // Добавляем 3 усиления в инвентарь напрямую
        for pu_id in pu_ids.iter().take(3) {
            sqlx::query(
                "INSERT INTO player_inventory (game_id, user_id, power_up_id, quantity)
                 VALUES ($1, $2, $3, 1)
                 ON CONFLICT (game_id, user_id, power_up_id)
                 DO UPDATE SET quantity = player_inventory.quantity + 1",
            )
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .execute(&pool)
            .await
            .ok();

            sqlx::query("SELECT install_upgrade($1, $2, $3, $4)")
                .bind(game_id)
                .bind(user_id)
                .bind(prop_id)
                .bind(pu_id)
                .execute(&pool)
                .await
                .expect("install_upgrade провалился");
        }

        // Пытаемся установить 4-е усиление
        sqlx::query(
            "INSERT INTO player_inventory (game_id, user_id, power_up_id, quantity)
             VALUES ($1, $2, $3, 1)
             ON CONFLICT (game_id, user_id, power_up_id)
             DO UPDATE SET quantity = player_inventory.quantity + 1",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(pu_ids[3])
        .execute(&pool)
        .await
        .ok();

        let result: (String,) = sqlx::query_as("SELECT install_upgrade($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(prop_id)
            .bind(pu_ids[3])
            .fetch_one(&pool)
            .await
            .expect("install_upgrade не вернул результат");

        assert_ne!(
            result.0, "ok",
            "Установка 4-го усиления должна быть отклонена"
        );
        assert!(
            result.0.contains("слот") || result.0.contains("заняты"),
            "Сообщение об ошибке должно содержать 'слот' или 'заняты': {}",
            result.0
        );

        cleanup_all(&pool, game_id, "test_upg_user4").await;
    }
}
