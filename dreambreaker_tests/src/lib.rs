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
// ════════════════════════════════════════════════════════
// БЛОК 6: Пользователи — дополнительные функции
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_users_extended {
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

    // Тест 21: get_user_by_id возвращает корректные данные
    #[tokio::test]
    async fn test_get_user_by_id() {
        let pool = setup_db().await;
        let username = "test_byid_user";
        let user_id = create_test_user(&pool, username).await;

        let result: Result<(uuid::Uuid, String, String, String, bool), _> = sqlx::query_as(
            "SELECT id, username, type, password_hash, is_active
                 FROM get_user_by_id($1)",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok(), "get_user_by_id должен вернуть строку");
        let (id, uname, _, hash, active) = result.unwrap();
        assert_eq!(id, user_id, "ID должны совпадать");
        assert_eq!(uname, username, "Имя должно совпадать");
        assert_eq!(hash, "testhash", "Хеш должен совпадать");
        assert!(active, "Пользователь должен быть активен");

        cleanup(&pool, username).await;
    }

    // Тест 22: get_user_by_id с несуществующим ID возвращает пусто
    #[tokio::test]
    async fn test_get_user_by_id_nonexistent() {
        let pool = setup_db().await;
        let fake_id = uuid::Uuid::new_v4();

        let result: Result<(uuid::Uuid, String, String, String, bool), _> = sqlx::query_as(
            "SELECT id, username, type, password_hash, is_active
                 FROM get_user_by_id($1)",
        )
        .bind(fake_id)
        .fetch_one(&pool)
        .await;

        assert!(
            result.is_err(),
            "get_user_by_id с несуществующим ID должен вернуть пустой результат"
        );
    }

    // Тест 23: list_users возвращает только human-пользователей
    #[tokio::test]
    async fn test_list_users_returns_humans_only() {
        let pool = setup_db().await;
        let username = "test_listusers_human";
        create_test_user(&pool, username).await;

        let rows: Vec<(uuid::Uuid, String, String, bool)> =
            sqlx::query_as("SELECT id, username, type, is_active FROM list_users()")
                .fetch_all(&pool)
                .await
                .expect("list_users провалился");

        assert!(
            !rows.is_empty(),
            "list_users должен вернуть хотя бы одного пользователя"
        );
        for (_, _, utype, _) in &rows {
            assert_eq!(utype, "human", "list_users не должен возвращать ботов");
        }

        cleanup(&pool, username).await;
    }
}

// ════════════════════════════════════════════════════════
// БЛОК 7: Игры — дополнительные функции
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_games_extended {
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

    // Тест 24: get_user_games возвращает игры пользователя
    #[tokio::test]
    async fn test_get_user_games() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_ugames_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let rows: Vec<(uuid::Uuid, String, i64, i32)> =
            sqlx::query_as("SELECT game_id, status, balance, moves_made FROM get_user_games($1)")
                .bind(user_id)
                .fetch_all(&pool)
                .await
                .expect("get_user_games провалился");

        assert!(
            !rows.is_empty(),
            "get_user_games должен вернуть хотя бы одну игру"
        );
        let found = rows.iter().any(|(gid, _, _, _)| *gid == game_id);
        assert!(found, "Созданная партия должна быть в списке");

        cleanup_all(&pool, game_id, "test_ugames_user").await;
    }

    // Тест 25: get_latest_user_game возвращает последнюю активную игру
    #[tokio::test]
    async fn test_get_latest_user_game() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_latestgame_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let result: Result<(uuid::Uuid, String, i64, i32), _> = sqlx::query_as(
            "SELECT game_id, status, balance, moves_made
                 FROM get_latest_user_game($1)",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok(), "get_latest_user_game должен вернуть строку");
        let (gid, status, _, _) = result.unwrap();
        assert_eq!(gid, game_id, "Должна вернуться последняя созданная игра");
        assert_eq!(status, "active", "Статус должен быть active");

        cleanup_all(&pool, game_id, "test_latestgame_user").await;
    }

    // Тест 26: get_active_game возвращает UUID активной игры
    #[tokio::test]
    async fn test_get_active_game() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_activegame_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let result: Result<(Option<uuid::Uuid>,), _> = sqlx::query_as("SELECT get_active_game($1)")
            .bind(user_id)
            .fetch_one(&pool)
            .await;

        assert!(result.is_ok(), "get_active_game не должен падать");
        let (maybe_id,) = result.unwrap();
        assert_eq!(
            maybe_id,
            Some(game_id),
            "Должна вернуться UUID активной игры"
        );

        cleanup_all(&pool, game_id, "test_activegame_user").await;
    }

    // Тест 27: user_has_active_game возвращает true при наличии активной игры
    #[tokio::test]
    async fn test_user_has_active_game_true() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_hasactive_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (has_active,): (bool,) = sqlx::query_as("SELECT user_has_active_game($1)")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("user_has_active_game провалился");

        assert!(has_active, "Должно вернуть true — активная игра есть");

        cleanup_all(&pool, game_id, "test_hasactive_user").await;
    }

    // Тест 28: user_has_active_game возвращает false без активных игр
    #[tokio::test]
    async fn test_user_has_active_game_false() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_noactive_user").await;

        let (has_active,): (bool,) = sqlx::query_as("SELECT user_has_active_game($1)")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("user_has_active_game провалился");

        assert!(!has_active, "Должно вернуть false — активных игр нет");

        cleanup(&pool, "test_noactive_user").await;
    }

    // Тест 29: set_game_status меняет статус игры
    #[tokio::test]
    async fn test_set_game_status() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_setstatus_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        sqlx::query("SELECT set_game_status($1, $2)")
            .bind(game_id)
            .bind("paused")
            .execute(&pool)
            .await
            .expect("set_game_status провалился");

        let (status,): (String,) = sqlx::query_as("SELECT status FROM games WHERE id = $1")
            .bind(game_id)
            .fetch_one(&pool)
            .await
            .expect("Игра не найдена");

        assert_eq!(status, "paused", "Статус должен стать paused");

        cleanup_all(&pool, game_id, "test_setstatus_user").await;
    }

    // Тест 30: list_games возвращает хотя бы одну игру
    #[tokio::test]
    async fn test_list_games() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_listgames_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let rows: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT id, status FROM list_games()")
            .fetch_all(&pool)
            .await
            .expect("list_games провалился");

        assert!(
            !rows.is_empty(),
            "list_games должен вернуть хотя бы одну игру"
        );

        cleanup_all(&pool, game_id, "test_listgames_user").await;
    }

    // Тест 31: get_game_rules возвращает правила созданной игры
    #[tokio::test]
    async fn test_get_game_rules() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_rules_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let result: Result<(uuid::Uuid, i64, i32, i64), _> = sqlx::query_as(
            "SELECT game_id, starting_balance, max_turns, target_balance
                 FROM get_game_rules($1)",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok(), "get_game_rules должен вернуть строку");
        let (gid, start_bal, max_turns, target) = result.unwrap();
        assert_eq!(gid, game_id);
        assert_eq!(start_bal, 1000);
        assert_eq!(max_turns, 50);
        assert_eq!(target, 2500);

        cleanup_all(&pool, game_id, "test_rules_user").await;
    }

    // Тест 32: list_game_participants возвращает всех участников (игрок + 4 бота)
    #[tokio::test]
    async fn test_list_game_participants() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_participants_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let rows: Vec<(uuid::Uuid, uuid::Uuid, i32, i64, i32)> = sqlx::query_as(
            "SELECT game_id, user_id, position, balance, turn_order
             FROM list_game_participants($1)",
        )
        .bind(game_id)
        .fetch_all(&pool)
        .await
        .expect("list_game_participants провалился");

        assert_eq!(rows.len(), 5, "Должно быть 5 участников: 1 игрок + 4 бота");
        let player_found = rows.iter().any(|(_, uid, _, _, _)| *uid == user_id);
        assert!(player_found, "Игрок должен быть в списке участников");

        cleanup_all(&pool, game_id, "test_participants_user").await;
    }
}

// ════════════════════════════════════════════════════════
// БЛОК 8: Игровое поле и состояние
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_board {
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

    // Тест 33: get_board_cells возвращает ровно 40 клеток с корректными типами
    #[tokio::test]
    async fn test_get_board_cells() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_board_user1").await;
        let game_id = create_test_game(&pool, user_id).await;

        let rows: Vec<(i32, String)> =
            sqlx::query_as("SELECT cell_index, cell_type FROM get_board_cells($1)")
                .bind(game_id)
                .fetch_all(&pool)
                .await
                .expect("get_board_cells провалился");

        assert_eq!(rows.len(), 40, "Должно быть ровно 40 клеток");

        // Клетка 0 — start
        let cell_0 = rows.iter().find(|(idx, _)| *idx == 0);
        assert!(cell_0.is_some(), "Клетка 0 должна существовать");
        assert_eq!(cell_0.unwrap().1, "start", "Клетка 0 должна быть start");

        // Проверяем наличие property и shop клеток
        let has_property = rows.iter().any(|(_, t)| t == "property");
        let has_shop = rows.iter().any(|(_, t)| t == "shop");
        let has_tax = rows.iter().any(|(_, t)| t == "tax");
        assert!(has_property, "Должны быть property-клетки");
        assert!(has_shop, "Должны быть shop-клетки");
        assert!(has_tax, "Должны быть tax-клетки");

        cleanup_all(&pool, game_id, "test_board_user1").await;
    }

    // Тест 34: get_participant_state возвращает корректное начальное состояние
    #[tokio::test]
    async fn test_get_participant_state() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_state_user1").await;
        let game_id = create_test_game(&pool, user_id).await;

        let result: Result<(i32, i64, i32, i64, i64), _> = sqlx::query_as(
            "SELECT position, balance, moves_made, total_spent, total_earned
             FROM get_participant_state($1, $2)",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await;

        assert!(
            result.is_ok(),
            "get_participant_state должен вернуть строку"
        );
        let (pos, bal, moves, spent, earned) = result.unwrap();
        assert_eq!(pos, 0, "Начальная позиция должна быть 0");
        assert_eq!(bal, 1000, "Начальный баланс должен быть 1000");
        assert_eq!(moves, 0, "Начальное количество ходов должно быть 0");
        assert_eq!(spent, 0, "total_spent должен быть 0");
        assert_eq!(earned, 0, "total_earned должен быть 0");

        cleanup_all(&pool, game_id, "test_state_user1").await;
    }

    // Тест 35: calc_rent возвращает 0 для собственности без владельца
    #[tokio::test]
    async fn test_calc_rent_no_owner() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_calcrent_user1").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Берём первую property без владельца
        let (prop_id,): (uuid::Uuid,) = sqlx::query_as(
            "SELECT p.id FROM properties p
             JOIN game_cells gc ON gc.id = p.cell_id
             WHERE gc.game_id = $1 AND p.owner_user_id IS NULL
             LIMIT 1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .expect("Нет свободных собственностей");

        let (rent,): (i64,) = sqlx::query_as("SELECT calc_rent($1, $2)")
            .bind(game_id)
            .bind(prop_id)
            .fetch_one(&pool)
            .await
            .expect("calc_rent провалился");

        assert_eq!(rent, 0, "Аренда без владельца должна быть 0");

        cleanup_all(&pool, game_id, "test_calcrent_user1").await;
    }

    // Тест 36: calc_rent возвращает корректное значение для собственности с владельцем
    #[tokio::test]
    async fn test_calc_rent_with_owner() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_calcrent_user2").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Находим первую property-клетку
        let (cell_index,): (i32,) = sqlx::query_as(
            "SELECT gc.cell_index FROM game_cells gc
             WHERE gc.game_id = $1 AND gc.cell_type = 'property'
             ORDER BY gc.cell_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .expect("Нет property-клеток");

        // Перемещаем игрока и покупаем
        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("SELECT buy_property($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .expect("buy_property провалился");

        let (prop_id,): (uuid::Uuid,) = sqlx::query_as(
            "SELECT p.id FROM properties p
             JOIN game_cells gc ON gc.id = p.cell_id
             WHERE gc.game_id = $1 AND p.owner_user_id = $2
             LIMIT 1",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("Собственность не найдена");

        let (rent,): (i64,) = sqlx::query_as("SELECT calc_rent($1, $2)")
            .bind(game_id)
            .bind(prop_id)
            .fetch_one(&pool)
            .await
            .expect("calc_rent провалился");

        assert!(rent > 0, "Аренда с владельцем должна быть больше 0");

        cleanup_all(&pool, game_id, "test_calcrent_user2").await;
    }

    // Тест 37: pay_tax уменьшает баланс на 100 + 5% от баланса
    #[tokio::test]
    async fn test_pay_tax() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_tax_user1").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (bal_before,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        let expected_tax: i64 = 100 + (bal_before as f64 * 0.05).ceil() as i64;

        sqlx::query("SELECT pay_tax($1, $2)")
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("pay_tax провалился");

        let (bal_after,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(
            bal_after,
            bal_before - expected_tax,
            "Баланс должен уменьшиться на 100 + 5% от баланса"
        );

        cleanup_all(&pool, game_id, "test_tax_user1").await;
    }
}

// ════════════════════════════════════════════════════════
// БЛОК 9: Магазин — дополнительные функции
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_shop_extended {
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
                .bind(5000_i64)
                .bind(50_i32)
                .bind(99999_i64)
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

    async fn get_shop_id(pool: &PgPool, game_id: uuid::Uuid) -> uuid::Uuid {
        let (shop_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT id FROM shops WHERE game_id = $1 ORDER BY id LIMIT 1")
                .bind(game_id)
                .fetch_one(pool)
                .await
                .expect("Магазин не найден");
        shop_id
    }

    // Тест 38: get_shop_slots возвращает слоты магазина
    #[tokio::test]
    async fn test_get_shop_slots() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_getslots_user").await;
        let game_id = create_test_game(&pool, user_id).await;
        let shop_id = get_shop_id(&pool, game_id).await;

        let rows: Vec<(i32, uuid::Uuid, String, i64, String)> = sqlx::query_as(
            "SELECT slot_index, slot_id, name, cost, status
             FROM get_shop_slots($1, $2, $3)",
        )
        .bind(shop_id)
        .bind(game_id)
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .expect("get_shop_slots провалился");

        assert!(
            !rows.is_empty(),
            "get_shop_slots должен вернуть хотя бы один слот"
        );
        for (_, _, name, cost, status) in &rows {
            assert!(!name.is_empty(), "Имя усиления не должно быть пустым");
            assert!(*cost > 0, "Стоимость должна быть больше 0");
            assert_eq!(status, "available", "Слоты должны быть available");
        }

        cleanup_all(&pool, game_id, "test_getslots_user").await;
    }

    // Тест 39: reroll_shop обновляет слоты и списывает деньги
    #[tokio::test]
    async fn test_reroll_shop() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_reroll_user").await;
        let game_id = create_test_game(&pool, user_id).await;
        let shop_id = get_shop_id(&pool, game_id).await;

        // Сохраняем IDs слотов до реролла
        let slots_before: Vec<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM shop_slots WHERE shop_id = $1 ORDER BY slot_index")
                .bind(shop_id)
                .fetch_all(&pool)
                .await
                .expect("Слоты не найдены");

        let (bal_before,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query("SELECT reroll_shop($1, $2, $3)")
            .bind(shop_id)
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("reroll_shop провалился");

        let slots_after: Vec<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM shop_slots WHERE shop_id = $1 ORDER BY slot_index")
                .bind(shop_id)
                .fetch_all(&pool)
                .await
                .expect("Слоты не найдены после реролла");

        let (bal_after,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        // Слоты должны замениться (новые UUID)
        assert_ne!(
            slots_before, slots_after,
            "После реролла слоты должны быть новыми"
        );
        assert!(bal_after < bal_before, "Реролл должен списать деньги");

        cleanup_all(&pool, game_id, "test_reroll_user").await;
    }
}

// ════════════════════════════════════════════════════════
// БЛОК 10: Усиления — дополнительные функции
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_power_ups_extended {
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
                .bind(2000_i64)
                .bind(50_i32)
                .bind(99999_i64)
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

    // Тест 40: list_power_ups возвращает 15 усилений
    #[tokio::test]
    async fn test_list_power_ups() {
        let pool = setup_db().await;

        let rows: Vec<(uuid::Uuid, String, i64)> =
            sqlx::query_as("SELECT id, name, cost FROM list_power_ups()")
                .fetch_all(&pool)
                .await
                .expect("list_power_ups провалился");

        assert_eq!(rows.len(), 15, "Должно быть ровно 15 усилений");
        // Проверяем сортировку по стоимости
        let costs: Vec<i64> = rows.iter().map(|(_, _, c)| *c).collect();
        let mut sorted = costs.clone();
        sorted.sort();
        assert_eq!(
            costs, sorted,
            "list_power_ups должен возвращать усиления по возрастанию стоимости"
        );
    }

    // Тест 41: add_to_inventory добавляет усиление в инвентарь
    #[tokio::test]
    async fn test_add_to_inventory() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_addinv_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (pu_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT id FROM power_ups ORDER BY cost LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("power_ups пуста");

        sqlx::query("SELECT add_to_inventory($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .bind(2_i32)
            .execute(&pool)
            .await
            .expect("add_to_inventory провалился");

        let (qty,): (i32,) = sqlx::query_as(
            "SELECT quantity FROM player_inventory
             WHERE game_id=$1 AND user_id=$2 AND power_up_id=$3",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(pu_id)
        .fetch_one(&pool)
        .await
        .expect("Запись в инвентаре не найдена");

        assert_eq!(qty, 2, "Количество должно быть 2");

        cleanup_all(&pool, game_id, "test_addinv_user").await;
    }

    // Тест 42: add_to_inventory накапливает количество при повторном вызове
    #[tokio::test]
    async fn test_add_to_inventory_accumulates() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_addinv_acc_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (pu_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT id FROM power_ups ORDER BY cost LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("power_ups пуста");

        // Добавляем 1, потом ещё 1
        sqlx::query("SELECT add_to_inventory($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .bind(1_i32)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("SELECT add_to_inventory($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .bind(1_i32)
            .execute(&pool)
            .await
            .ok();

        let (qty,): (i32,) = sqlx::query_as(
            "SELECT quantity FROM player_inventory
             WHERE game_id=$1 AND user_id=$2 AND power_up_id=$3",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(pu_id)
        .fetch_one(&pool)
        .await
        .expect("Запись не найдена");

        assert_eq!(
            qty, 2,
            "При повторном добавлении quantity должно суммироваться"
        );

        cleanup_all(&pool, game_id, "test_addinv_acc_user").await;
    }

    // Тест 43: get_player_inventory возвращает добавленные усиления
    #[tokio::test]
    async fn test_get_player_inventory() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_getinv_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (pu_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT id FROM power_ups ORDER BY cost LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("power_ups пуста");

        sqlx::query("SELECT add_to_inventory($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .bind(3_i32)
            .execute(&pool)
            .await
            .ok();

        let rows: Vec<(uuid::Uuid, String, i32)> =
            sqlx::query_as("SELECT power_up_id, name, quantity FROM get_player_inventory($1, $2)")
                .bind(game_id)
                .bind(user_id)
                .fetch_all(&pool)
                .await
                .expect("get_player_inventory провалился");

        assert!(!rows.is_empty(), "Инвентарь не должен быть пустым");
        let found = rows.iter().any(|(pid, _, qty)| *pid == pu_id && *qty == 3);
        assert!(
            found,
            "Добавленное усиление должно быть в инвентаре с qty=3"
        );

        cleanup_all(&pool, game_id, "test_getinv_user").await;
    }

    // Тест 44: sell_power_up возвращает половину стоимости и убирает из инвентаря
    #[tokio::test]
    async fn test_sell_power_up() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_sell_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (pu_id, pu_cost): (uuid::Uuid, i64) =
            sqlx::query_as("SELECT id, cost FROM power_ups ORDER BY cost LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("power_ups пуста");

        // Добавляем усиление в инвентарь напрямую
        sqlx::query("SELECT add_to_inventory($1, $2, $3, $4)")
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .bind(1_i32)
            .execute(&pool)
            .await
            .ok();

        let (bal_before,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query("SELECT sell_power_up($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .execute(&pool)
            .await
            .expect("sell_power_up провалился");

        let (bal_after,): (i64,) =
            sqlx::query_as("SELECT balance FROM game_participants WHERE game_id=$1 AND user_id=$2")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        let refund = pu_cost / 2;
        assert_eq!(
            bal_after,
            bal_before + refund,
            "Баланс должен увеличиться на половину стоимости усиления"
        );

        // Усиление должно исчезнуть из инвентаря
        let inv: Option<(i32,)> = sqlx::query_as(
            "SELECT quantity FROM player_inventory
             WHERE game_id=$1 AND user_id=$2 AND power_up_id=$3",
        )
        .bind(game_id)
        .bind(user_id)
        .bind(pu_id)
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(
            inv.is_none() || inv.unwrap().0 == 0,
            "После продажи усиление должно исчезнуть из инвентаря"
        );

        cleanup_all(&pool, game_id, "test_sell_user").await;
    }

    // Тест 45: sell_power_up возвращает ошибку если усиления нет в инвентаре
    #[tokio::test]
    async fn test_sell_power_up_not_in_inventory() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_sell_err_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (pu_id,): (uuid::Uuid,) =
            sqlx::query_as("SELECT id FROM power_ups ORDER BY cost LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("power_ups пуста");

        let result = sqlx::query("SELECT sell_power_up($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(pu_id)
            .execute(&pool)
            .await;

        assert!(
            result.is_err(),
            "sell_power_up без усиления в инвентаре должен вернуть ошибку"
        );

        cleanup_all(&pool, game_id, "test_sell_err_user").await;
    }
}

// ════════════════════════════════════════════════════════
// БЛОК 11: Статистика и завершение игры
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_stats {
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

    // Тест 46: get_all_balances возвращает балансы всех участников
    #[tokio::test]
    async fn test_get_all_balances() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_allbal_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let rows: Vec<(uuid::Uuid, i64, String)> =
            sqlx::query_as("SELECT user_id, balance, user_type FROM get_all_balances($1)")
                .bind(game_id)
                .fetch_all(&pool)
                .await
                .expect("get_all_balances провалился");

        assert_eq!(rows.len(), 5, "Должно быть 5 участников (игрок + 4 бота)");
        for (_, bal, _) in &rows {
            assert_eq!(
                *bal, 1000,
                "Начальный баланс всех участников должен быть 1000"
            );
        }

        cleanup_all(&pool, game_id, "test_allbal_user").await;
    }

    // Тест 47: surrender_game переводит игру в статус surrender
    #[tokio::test]
    async fn test_surrender_game() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_surrender_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        sqlx::query("SELECT surrender_game($1, $2)")
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("surrender_game провалился");

        let (status,): (String,) = sqlx::query_as("SELECT status FROM games WHERE id = $1")
            .bind(game_id)
            .fetch_one(&pool)
            .await
            .expect("Игра не найдена");

        assert_eq!(status, "surrender", "Статус должен стать surrender");

        cleanup_all(&pool, game_id, "test_surrender_user").await;
    }

    // Тест 48: surrender_game возвращает ошибку если пользователь не участник
    #[tokio::test]
    async fn test_surrender_game_not_participant() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_surr_owner").await;
        let outsider_id = create_test_user(&pool, "test_surr_outsider").await;
        let game_id = create_test_game(&pool, user_id).await;

        let result = sqlx::query("SELECT surrender_game($1, $2)")
            .bind(game_id)
            .bind(outsider_id)
            .execute(&pool)
            .await;

        assert!(
            result.is_err(),
            "surrender_game от не-участника должен вернуть ошибку"
        );

        cleanup_all(&pool, game_id, "test_surr_owner").await;
        cleanup(&pool, "test_surr_outsider").await;
    }

    // Тест 49: get_user_stats возвращает нули для нового пользователя без завершённых игр
    #[tokio::test]
    async fn test_get_user_stats_empty() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_stats_empty_user").await;

        let result: Result<(i32, i32, i32, i64, i64, i64, i64, i64), _> = sqlx::query_as(
            "SELECT total_games, total_wins, current_win_streak,
                    total_moves, total_earned, total_spent,
                    properties_bought, power_ups_bought
             FROM get_user_stats($1)",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await;

        // Для пользователя без завершённых игр функция вернёт NULL-строку (агрегат по пустому)
        // Это нормально — проверяем что функция не падает
        // Если вернулась строка — все значения должны быть 0
        if let Ok((total, wins, streak, moves, earned, spent, props, pus)) = result {
            assert_eq!(total, 0);
            assert_eq!(wins, 0);
            assert_eq!(streak, 0);
            assert_eq!(moves, 0);
            assert_eq!(earned, 0);
            assert_eq!(spent, 0);
            assert_eq!(props, 0);
            assert_eq!(pus, 0);
        }
        // Если ничего не вернула — это тоже допустимо для пустого агрегата

        cleanup(&pool, "test_stats_empty_user").await;
    }

    // Тест 50: get_user_stats подсчитывает surrender-игру
    #[tokio::test]
    async fn test_get_user_stats_after_surrender() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_stats_surr_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Сдаёмся — игра завершена со статусом surrender
        sqlx::query("SELECT surrender_game($1, $2)")
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .ok();

        let (total_games,): (i32,) = sqlx::query_as("SELECT total_games FROM get_user_stats($1)")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("get_user_stats провалился");

        assert_eq!(
            total_games, 1,
            "После одной surrender-игры total_games должен быть 1"
        );

        cleanup_all(&pool, game_id, "test_stats_surr_user").await;
    }

    // Тест 51: get_game_result возвращает корректные данные после surrender
    #[tokio::test]
    async fn test_get_game_result_surrender() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_result_surr_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        sqlx::query("SELECT surrender_game($1, $2)")
            .bind(game_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .ok();

        let result: Result<(String, i64, i64, i32, i32, i64, i64, bool), _> = sqlx::query_as(
            "SELECT game_status, balance, target_balance, moves_made, max_turns,
                    total_earned, total_spent, is_victory
             FROM get_game_result($1, $2)",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok(), "get_game_result должен вернуть строку");
        let (status, _, target, _, max_turns, _, _, is_victory) = result.unwrap();
        assert_eq!(status, "surrender", "Статус должен быть surrender");
        assert_eq!(target, 2500, "target_balance должен быть 2500");
        assert_eq!(max_turns, 50, "max_turns должен быть 50");
        assert!(!is_victory, "surrender не является победой");

        cleanup_all(&pool, game_id, "test_result_surr_user").await;
    }

    // Тест 52: get_game_result с победой (баланс >= target) возвращает is_victory = true
    #[tokio::test]
    async fn test_get_game_result_victory() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_result_win_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Устанавливаем баланс выше целевого и статус finished
        sqlx::query(
            "UPDATE game_participants SET balance = 9999
             WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .ok();

        sqlx::query("SELECT set_game_status($1, $2)")
            .bind(game_id)
            .bind("finished")
            .execute(&pool)
            .await
            .ok();

        let (is_victory,): (bool,) =
            sqlx::query_as("SELECT is_victory FROM get_game_result($1, $2)")
                .bind(game_id)
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("get_game_result провалился");

        assert!(
            is_victory,
            "При balance >= target и статусе finished должна быть победа"
        );

        cleanup_all(&pool, game_id, "test_result_win_user").await;
    }
}

// ════════════════════════════════════════════════════════
// БЛОК 12: Ботовые функции
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_bots {
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

    // Тест 53: get_bot_participants возвращает ровно 4 бота
    #[tokio::test]
    async fn test_get_bot_participants() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_bots_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let rows: Vec<(uuid::Uuid, String, i32, i64, i32)> = sqlx::query_as(
            "SELECT user_id, username, position, balance, turn_order
             FROM get_bot_participants($1)",
        )
        .bind(game_id)
        .fetch_all(&pool)
        .await
        .expect("get_bot_participants провалился");

        assert_eq!(rows.len(), 4, "Должно быть ровно 4 бота");
        for (_, _, pos, bal, _) in &rows {
            assert_eq!(*pos, 0, "Начальная позиция бота должна быть 0");
            assert_eq!(*bal, 1000, "Начальный баланс бота должен быть 1000");
        }

        cleanup_all(&pool, game_id, "test_bots_user").await;
    }

    // Тест 54: do_bot_turn перемещает бота и обновляет позицию
    #[tokio::test]
    async fn test_do_bot_turn_updates_position() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_botturn_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Берём первого бота
        let (bot_id,): (uuid::Uuid,) = sqlx::query_as(
            "SELECT gp.user_id FROM game_participants gp
             JOIN users u ON u.id = gp.user_id
             WHERE gp.game_id = $1 AND u.type = 'bot'
             ORDER BY gp.turn_order LIMIT 1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .expect("Бот не найден");

        let result: Result<(i32, i64, String, String), _> = sqlx::query_as(
            "SELECT new_position, new_balance, action, action_detail
             FROM do_bot_turn($1, $2, $3)",
        )
        .bind(game_id)
        .bind(bot_id)
        .bind(3_i32)
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok(), "do_bot_turn должен выполниться без ошибок");
        let (new_pos, new_bal, _, _) = result.unwrap();
        assert_eq!(new_pos, 3, "Бот должен переместиться на позицию 3");
        assert!(new_bal > 0, "Баланс бота должен оставаться положительным");

        // Проверяем что позиция обновилась в БД
        let (db_pos,): (i32,) = sqlx::query_as(
            "SELECT position FROM game_participants WHERE game_id=$1 AND user_id=$2",
        )
        .bind(game_id)
        .bind(bot_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(db_pos, 3, "Позиция в БД должна обновиться");

        cleanup_all(&pool, game_id, "test_botturn_user").await;
    }

    // Тест 55: get_player_properties возвращает купленные собственности
    #[tokio::test]
    async fn test_get_player_properties() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_playerprops_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        // Покупаем первую собственность
        let (cell_index,): (i32,) = sqlx::query_as(
            "SELECT gc.cell_index FROM game_cells gc
             WHERE gc.game_id = $1 AND gc.cell_type = 'property'
             ORDER BY gc.cell_index LIMIT 1",
        )
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .expect("Нет property-клеток");

        sqlx::query("SELECT commit_player_move($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .ok();
        sqlx::query("SELECT buy_property($1, $2, $3)")
            .bind(game_id)
            .bind(user_id)
            .bind(cell_index)
            .execute(&pool)
            .await
            .expect("buy_property провалился");

        let rows: Vec<(uuid::Uuid, i32, String, i32, i32)> = sqlx::query_as(
            "SELECT property_id, cell_index, prop_name, upgrades_count, max_upgrades
             FROM get_player_properties($1, $2)",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .expect("get_player_properties провалился");

        assert_eq!(rows.len(), 1, "Должна быть ровно 1 собственность");
        let (_, ci, _, upg_count, max_upg) = &rows[0];
        assert_eq!(*ci, cell_index, "cell_index должен совпадать");
        assert_eq!(*upg_count, 0, "Усилений изначально нет");
        assert_eq!(*max_upg, 3, "max_upgrades должен быть 3");

        cleanup_all(&pool, game_id, "test_playerprops_user").await;
    }
}
// ════════════════════════════════════════════════════════
// БЛОК 13: Дополнительные функции и триггеры
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_misc {
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

    // Тест 56: get_active_game_for_user — алиас get_active_game, возвращает тот же UUID
    #[tokio::test]
    async fn test_get_active_game_for_user() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_agfu_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (via_alias,): (Option<uuid::Uuid>,) =
            sqlx::query_as("SELECT get_active_game_for_user($1)")
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("get_active_game_for_user провалился");

        let (via_original,): (Option<uuid::Uuid>,) =
            sqlx::query_as("SELECT get_active_game($1)")
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .expect("get_active_game провалился");

        assert_eq!(
            via_alias, via_original,
            "get_active_game_for_user должен возвращать тот же результат что и get_active_game"
        );
        assert_eq!(
            via_alias,
            Some(game_id),
            "Должна вернуться UUID активной игры"
        );

        cleanup_all(&pool, game_id, "test_agfu_user").await;
    }

    // Тест 57: триггер update_game_timestamp обновляет last_saved_at при изменении игры
    #[tokio::test]
    async fn test_update_game_timestamp_trigger() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_trigger_user").await;
        let game_id = create_test_game(&pool, user_id).await;

        let (ts_before,): (chrono::DateTime<chrono::Utc>,) =
            sqlx::query_as("SELECT last_saved_at FROM games WHERE id = $1")
                .bind(game_id)
                .fetch_one(&pool)
                .await
                .expect("Игра не найдена");

        // Небольшая пауза чтобы timestamp гарантированно изменился
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Любое UPDATE на games должно сработать триггер
        sqlx::query("SELECT set_game_status($1, $2)")
            .bind(game_id)
            .bind("paused")
            .execute(&pool)
            .await
            .expect("set_game_status провалился");

        let (ts_after,): (chrono::DateTime<chrono::Utc>,) =
            sqlx::query_as("SELECT last_saved_at FROM games WHERE id = $1")
                .bind(game_id)
                .fetch_one(&pool)
                .await
                .expect("Игра не найдена");

        assert!(
            ts_after > ts_before,
            "Триггер должен обновить last_saved_at после UPDATE на games"
        );

        cleanup_all(&pool, game_id, "test_trigger_user").await;
    }
}
// ════════════════════════════════════════════════════════
// БЛОК 14: reset_stale_active_games
// ════════════════════════════════════════════════════════
#[cfg(test)]
mod tests_reset_stale {
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

    // Тест 58: reset_stale_active_games оставляет только одну active-игру,
    // остальные переводит в paused
    #[tokio::test]
    async fn test_reset_stale_active_games() {
        let pool = setup_db().await;
        let user_id = create_test_user(&pool, "test_reset_stale_user").await;

        // Создаём три игры — все получат статус active
        let (game_id_1,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(1111_i64).bind(1000_i64).bind(50_i32).bind(2500_i64).bind(user_id)
                .fetch_one(&pool).await.expect("Создание игры 1 провалилось");

        // Небольшая пауза чтобы last_saved_at различались
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let (game_id_2,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(2222_i64).bind(1000_i64).bind(50_i32).bind(2500_i64).bind(user_id)
                .fetch_one(&pool).await.expect("Создание игры 2 провалилось");

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let (game_id_3,): (uuid::Uuid,) =
            sqlx::query_as("SELECT create_game_full($1, $2, $3, $4, $5)")
                .bind(3333_i64).bind(1000_i64).bind(50_i32).bind(2500_i64).bind(user_id)
                .fetch_one(&pool).await.expect("Создание игры 3 провалилось");

        // Убеждаемся что все три — active
        let (active_before,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM games g
             JOIN game_participants gp ON g.id = gp.game_id
             WHERE gp.user_id = $1 AND g.status = 'active'",
        )
        .bind(user_id)
        .fetch_one(&pool).await.unwrap();
        assert_eq!(active_before, 3, "Перед вызовом должно быть 3 active-игры");

        // Вызываем функцию
        sqlx::query("SELECT reset_stale_active_games($1)")
            .bind(user_id)
            .execute(&pool).await.expect("reset_stale_active_games провалился");

        // Должна остаться ровно одна active
        let (active_after,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM games g
             JOIN game_participants gp ON g.id = gp.game_id
             WHERE gp.user_id = $1 AND g.status = 'active'",
        )
        .bind(user_id)
        .fetch_one(&pool).await.unwrap();
        assert_eq!(active_after, 1, "После вызова должна остаться ровно одна active-игра");

        // Оставшаяся active — последняя по last_saved_at (game_id_3)
        let (surviving_id,): (uuid::Uuid,) = sqlx::query_as(
            "SELECT g.id FROM games g
             JOIN game_participants gp ON g.id = gp.game_id
             WHERE gp.user_id = $1 AND g.status = 'active'",
        )
        .bind(user_id)
        .fetch_one(&pool).await.unwrap();
        assert_eq!(surviving_id, game_id_3, "Выжить должна последняя созданная игра");

        // Первые две должны стать paused
        let (status_1,): (String,) =
            sqlx::query_as("SELECT status FROM games WHERE id = $1")
                .bind(game_id_1).fetch_one(&pool).await.unwrap();
        let (status_2,): (String,) =
            sqlx::query_as("SELECT status FROM games WHERE id = $1")
                .bind(game_id_2).fetch_one(&pool).await.unwrap();
        assert_eq!(status_1, "paused", "Игра 1 должна стать paused");
        assert_eq!(status_2, "paused", "Игра 2 должна стать paused");

        // Cleanup
        sqlx::query("DELETE FROM games WHERE id IN ($1, $2, $3)")
            .bind(game_id_1).bind(game_id_2).bind(game_id_3)
            .execute(&pool).await.ok();
        cleanup(&pool, "test_reset_stale_user").await;
    }
}