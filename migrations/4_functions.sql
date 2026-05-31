-- =============================================================
-- DreamBreaker — все функции базы данных.
-- Вся бизнес-логика инкапсулирована здесь.
-- Rust вызывает эти функции, не пишет прямые INSERT/SELECT.
-- =============================================================
-- -------------------------------------------------------------------
-- БЛОК 1: Пользователи (Игроки)
-- -------------------------------------------------------------------
-- Создать пользователя. Принимает уже хешированный пароль (хеш делается в Rust).
-- Возвращает UUID нового пользователя.
DROP FUNCTION IF EXISTS register_new_user(TEXT, TEXT, TEXT);

CREATE
OR REPLACE FUNCTION register_new_user(
    p_username TEXT,
    p_type TEXT,
    p_password_hash TEXT
) RETURNS UUID AS $$ DECLARE new_id UUID;

BEGIN
INSERT INTO
    users (username, type, password_hash)
VALUES
    (p_username, p_type, p_password_hash) RETURNING id INTO new_id;

RETURN new_id;

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Получить credentials пользователя по имени для проверки пароля в Rust.
-- Возвращает одну строку или пусто (если пользователь не найден).
DROP FUNCTION IF EXISTS get_user_credentials(TEXT);

CREATE
OR REPLACE FUNCTION get_user_credentials(p_username TEXT) RETURNS TABLE (user_id UUID, pass_hash TEXT, active BOOLEAN) AS $$ BEGIN RETURN QUERY
SELECT
    u.id,
    u.password_hash :: TEXT,
    u.is_active
FROM
    users u
WHERE
    u.username = p_username;

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Получить полные данные пользователя по его ID.
-- Используется после успешной проверки пароля.
DROP FUNCTION IF EXISTS get_user_by_id(UUID);

CREATE
OR REPLACE FUNCTION get_user_by_id(p_user_id UUID) RETURNS TABLE (
    id UUID,
    username TEXT,
    type TEXT,
    password_hash TEXT,
    created_at TIMESTAMPTZ,
    is_active BOOLEAN
) AS $$ BEGIN RETURN QUERY
SELECT
    u.id,
    u.username :: TEXT,
    u.type :: TEXT,
    u.password_hash :: TEXT,
    u.created_at,
    u.is_active
FROM
    users u
WHERE
    u.id = p_user_id;

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Получить список всех пользователей (для меню выбора профиля).
DROP FUNCTION IF EXISTS list_users();

CREATE
OR REPLACE FUNCTION list_users() RETURNS TABLE (
    id UUID,
    username TEXT,
    type TEXT,
    created_at TIMESTAMPTZ,
    is_active BOOLEAN
) AS $$ BEGIN RETURN QUERY
SELECT
    u.id,
    u.username :: TEXT,
    u.type :: TEXT,
    u.created_at,
    u.is_active
FROM
    users u
WHERE
    u.is_active = TRUE
    AND u.type = 'human'
ORDER BY
    u.created_at;

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- -------------------------------------------------------------------
-- БЛОК 2: Игры
-- -------------------------------------------------------------------
-- Создать игру вместе с правилами (связь 1:1) в одной транзакции.
-- Возвращает UUID новой игры.
DROP FUNCTION IF EXISTS create_game_with_rules(BIGINT, BIGINT, INT, BIGINT);

CREATE
OR REPLACE FUNCTION create_game_with_rules(
    p_seed BIGINT,
    p_starting_balance BIGINT,
    p_max_turns INT,
    p_target_balance BIGINT
) RETURNS UUID AS $$ DECLARE new_game_id UUID;

BEGIN
INSERT INTO
    games (seed)
VALUES
    (p_seed) RETURNING id INTO new_game_id;

INSERT INTO
    game_rules (
        game_id,
        starting_balance,
        max_turns,
        target_balance
    )
VALUES
    (
        new_game_id,
        p_starting_balance,
        p_max_turns,
        p_target_balance
    );

RETURN new_game_id;

END;

$$ LANGUAGE plpgsql;

-- Получить список всех игр, отсортированный по дате последнего сохранения.
DROP FUNCTION IF EXISTS list_games();

CREATE
OR REPLACE FUNCTION list_games() RETURNS TABLE (
    id UUID,
    status TEXT,
    seed BIGINT,
    created_at TIMESTAMPTZ,
    last_saved_at TIMESTAMPTZ
) AS $$ BEGIN RETURN QUERY
SELECT
    g.id,
    g.status :: TEXT,
    g.seed,
    g.created_at,
    g.last_saved_at
FROM
    games g
ORDER BY
    g.last_saved_at DESC;

END;

$$ LANGUAGE plpgsql;

-- Получить правила конкретной игры (связь 1:1).
DROP FUNCTION IF EXISTS get_game_rules(UUID);

CREATE
OR REPLACE FUNCTION get_game_rules(p_game_id UUID) RETURNS TABLE (
    game_id UUID,
    starting_balance BIGINT,
    max_turns INT,
    target_balance BIGINT
) AS $$ BEGIN RETURN QUERY
SELECT
    gr.game_id,
    gr.starting_balance,
    gr.max_turns,
    gr.target_balance
FROM
    game_rules gr
WHERE
    gr.game_id = p_game_id;

END;

$$ LANGUAGE plpgsql;

-- Получить активную игру пользователя.
-- Возвращает UUID самой новой активной/ожидающей игры или NULL.
DROP FUNCTION IF EXISTS get_active_game(UUID);

CREATE
OR REPLACE FUNCTION get_active_game(p_user_id UUID) RETURNS UUID AS $$ DECLARE found_game_id UUID;

BEGIN
SELECT
    g.id INTO found_game_id
FROM
    games g
    JOIN game_participants gp ON g.id = gp.game_id
WHERE
    gp.user_id = p_user_id
    AND g.status IN ('active', 'pending')
ORDER BY
    g.last_saved_at DESC
LIMIT
    1;

RETURN found_game_id;

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Совместимость со старым именем.
DROP FUNCTION IF EXISTS get_active_game_for_user(UUID);

CREATE
OR REPLACE FUNCTION get_active_game_for_user(p_user_id UUID) RETURNS UUID AS $$ BEGIN RETURN get_active_game(p_user_id);

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Проверить, есть ли у пользователя активная игра.
DROP FUNCTION IF EXISTS user_has_active_game(UUID);

CREATE
OR REPLACE FUNCTION user_has_active_game(p_user_id UUID) RETURNS BOOLEAN AS $$ BEGIN RETURN EXISTS (
    SELECT
        1
    FROM
        games g
        JOIN game_participants gp ON g.id = gp.game_id
    WHERE
        gp.user_id = p_user_id
        AND g.status IN ('active', 'pending')
);

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Получить все игры пользователя со статистикой.
-- Возвращает все игры включая завершённые (для экрана загрузки).
-- Время создания в UTC+7.
DROP FUNCTION IF EXISTS get_user_games(UUID);

CREATE
OR REPLACE FUNCTION get_user_games(p_user_id UUID) RETURNS TABLE (
    game_id UUID,
    status TEXT,
    balance BIGINT,
    moves_made INT,
    created_at TIMESTAMPTZ
) AS $$ BEGIN RETURN QUERY
SELECT
    g.id,
    g.status :: TEXT,
    gp.balance,
    gp.moves_made,
    (g.created_at AT TIME ZONE 'Asia/Krasnoyarsk') :: TIMESTAMPTZ
FROM
    games g
    JOIN game_participants gp ON g.id = gp.game_id
WHERE
    gp.user_id = p_user_id
ORDER BY
    g.last_saved_at DESC;

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Получить последнюю активную/приостановленную игру пользователя.
-- Используется для кнопки "Продолжить игру" — загружает сразу.
DROP FUNCTION IF EXISTS get_latest_user_game(UUID);

CREATE
OR REPLACE FUNCTION get_latest_user_game(p_user_id UUID) RETURNS TABLE (
    game_id UUID,
    status TEXT,
    balance BIGINT,
    moves_made INT,
    created_at TIMESTAMPTZ
) AS $$ BEGIN RETURN QUERY
SELECT
    g.id,
    g.status :: TEXT,
    gp.balance,
    gp.moves_made,
    (g.created_at AT TIME ZONE 'Asia/Krasnoyarsk') :: TIMESTAMPTZ
FROM
    games g
    JOIN game_participants gp ON g.id = gp.game_id
WHERE
    gp.user_id = p_user_id
    AND g.status IN ('active', 'pending', 'paused')
ORDER BY
    g.last_saved_at DESC
LIMIT
    1;

END;

$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Изменить статус игры.
DROP FUNCTION IF EXISTS set_game_status(UUID, TEXT);

CREATE
OR REPLACE FUNCTION set_game_status(p_game_id UUID, p_status TEXT) RETURNS VOID AS $$ BEGIN
UPDATE
    games
SET
    status = p_status
WHERE
    id = p_game_id;

END;

$$ LANGUAGE plpgsql;

-- -------------------------------------------------------------------
-- БЛОК 3: Участники игры
-- -------------------------------------------------------------------
-- Добавить участника в игру.
-- Стартовый баланс берётся автоматически из game_rules этой игры.
DROP FUNCTION IF EXISTS add_game_participant(UUID, UUID, INT);

CREATE
OR REPLACE FUNCTION add_game_participant(
    p_game_id UUID,
    p_user_id UUID,
    p_turn_order INT
) RETURNS VOID AS $$ BEGIN
INSERT INTO
    game_participants (game_id, user_id, turn_order, balance)
VALUES
    (
        p_game_id,
        p_user_id,
        p_turn_order,
        (
            SELECT
                starting_balance
            FROM
                game_rules
            WHERE
                game_id = p_game_id
        )
    );

END;

$$ LANGUAGE plpgsql;

-- Получить список участников игры в порядке хода.
DROP FUNCTION IF EXISTS list_game_participants(UUID);

CREATE
OR REPLACE FUNCTION list_game_participants(p_game_id UUID) RETURNS TABLE (
    game_id UUID,
    user_id UUID,
    "position" INT,
    balance BIGINT,
    moves_made INT,
    total_spent BIGINT,
    total_earned BIGINT,
    turn_order INT
) AS $$ BEGIN RETURN QUERY
SELECT
    gp.game_id,
    gp.user_id,
    gp.position,
    gp.balance,
    gp.moves_made,
    gp.total_spent,
    gp.total_earned,
    gp.turn_order
FROM
    game_participants gp
WHERE
    gp.game_id = p_game_id
ORDER BY
    gp.turn_order;

END;

$$ LANGUAGE plpgsql;

-- -------------------------------------------------------------------
-- БЛОК 4: Усиления и инвентарь
-- -------------------------------------------------------------------
-- Получить все доступные усиления (справочник).
DROP FUNCTION IF EXISTS list_power_ups();

CREATE
OR REPLACE FUNCTION list_power_ups() RETURNS TABLE (
    id UUID,
    name TEXT,
    description TEXT,
    cost BIGINT,
    effect JSONB
) AS $$ BEGIN RETURN QUERY
SELECT
    p.id,
    p.name :: TEXT,
    p.description :: TEXT,
    p.cost,
    p.effect
FROM
    power_ups p
ORDER BY
    p.cost;

END;

$$ LANGUAGE plpgsql;

-- Добавить усиление в инвентарь игрока (или увеличить количество).
DROP FUNCTION IF EXISTS add_to_inventory(UUID, UUID, UUID, INT);

CREATE
OR REPLACE FUNCTION add_to_inventory(
    p_game_id UUID,
    p_user_id UUID,
    p_power_up_id UUID,
    p_quantity INT
) RETURNS VOID AS $$ BEGIN
INSERT INTO
    player_inventory (game_id, user_id, power_up_id, quantity)
VALUES
    (p_game_id, p_user_id, p_power_up_id, p_quantity) ON CONFLICT (game_id, user_id, power_up_id) DO
UPDATE
SET
    quantity = player_inventory.quantity + EXCLUDED.quantity;

END;

$$ LANGUAGE plpgsql;

-- Получить инвентарь игрока в конкретной игре.
DROP FUNCTION IF EXISTS get_player_inventory(UUID, UUID);

CREATE
OR REPLACE FUNCTION get_player_inventory(p_game_id UUID, p_user_id UUID) RETURNS TABLE (
    power_up_id UUID,
    name TEXT,
    quantity INT,
    effect JSONB
) AS $$ BEGIN RETURN QUERY
SELECT
    pi.power_up_id,
    pu.name :: TEXT,
    pi.quantity,
    pu.effect
FROM
    player_inventory pi
    JOIN power_ups pu ON pu.id = pi.power_up_id
WHERE
    pi.game_id = p_game_id
    AND pi.user_id = p_user_id
ORDER BY
    pu.name;

END;

$$ LANGUAGE plpgsql;
-- -------------------------------------------------------------------
-- БЛОК 5: Ходы
-- -------------------------------------------------------------------

-- Зафиксировать ход игрока: обновить позицию и счётчик ходов.
-- Возвращает обновлённое состояние участника.
-- Алиасы out_* чтобы избежать неоднозначности с колонками таблицы.
DROP FUNCTION IF EXISTS commit_player_move(UUID, UUID, INT);
CREATE OR REPLACE FUNCTION commit_player_move(
    p_game_id      UUID,
    p_user_id      UUID,
    p_new_position INT
)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
DECLARE
    v_old_pos   INT;
    v_old_moves INT;
    v_bonus     BIGINT := 0;
    v_pos       INT;
    v_bal       BIGINT;
    v_moves     INT;
    v_spent     BIGINT;
    v_earned    BIGINT;
BEGIN
    SELECT gp.position, gp.moves_made INTO v_old_pos, v_old_moves
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_user_id;

    -- Бонус за старт (не начислять на первом ходу)
    IF v_old_moves > 0 THEN
        IF p_new_position = 0 THEN
            v_bonus := 400;
        ELSIF v_old_pos > p_new_position THEN
            v_bonus := 200;
        END IF;
    END IF;

    UPDATE game_participants gp3
    SET position     = p_new_position,
        moves_made   = v_old_moves + 1,
        balance      = gp3.balance + v_bonus,
        total_earned = gp3.total_earned + v_bonus
    WHERE gp3.game_id = p_game_id AND gp3.user_id = p_user_id;

    SELECT gp2.position, gp2.balance, gp2.moves_made, gp2.total_spent, gp2.total_earned
    INTO v_pos, v_bal, v_moves, v_spent, v_earned
    FROM game_participants gp2
    WHERE gp2.game_id = p_game_id AND gp2.user_id = p_user_id;

    RETURN QUERY SELECT v_pos, v_bal, v_moves, v_spent, v_earned;
END;
$$ LANGUAGE plpgsql;
-- -------------------------------------------------------------------
-- БЛОК 6: Покупка собственности и аренда
-- -------------------------------------------------------------------

DROP FUNCTION IF EXISTS buy_property(UUID, UUID, INT);
CREATE OR REPLACE FUNCTION buy_property(
    p_game_id    UUID,
    p_user_id    UUID,
    p_cell_index INT
)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
DECLARE
    v_prop_id  UUID;
    v_cost     BIGINT;
    v_pos      INT;
    v_bal      BIGINT;
    v_moves    INT;
    v_spent    BIGINT;
    v_earned   BIGINT;
BEGIN
    -- Получаем property_id через game_cells
    SELECT gc.property_id INTO v_prop_id
    FROM game_cells gc
    WHERE gc.game_id = p_game_id AND gc.cell_index = p_cell_index;

    IF v_prop_id IS NULL THEN
        RAISE EXCEPTION 'Собственность не найдена на клетке %', p_cell_index;
    END IF;

    SELECT p.purchase_cost INTO v_cost
    FROM properties p WHERE p.id = v_prop_id;

    -- Проверка баланса
    SELECT gp.balance INTO v_bal
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_user_id;

    IF v_bal < v_cost THEN
        RAISE EXCEPTION 'Недостаточно средств: нужно %, есть %', v_cost, v_bal;
    END IF;

    -- Списать деньги
    UPDATE game_participants gp2
    SET balance     = gp2.balance - v_cost,
        total_spent = gp2.total_spent + v_cost
    WHERE gp2.game_id = p_game_id AND gp2.user_id = p_user_id;

    -- Записать владельца
    UPDATE properties
    SET owner_game_id = p_game_id,
        owner_user_id = p_user_id
    WHERE id = v_prop_id;

    -- Вернуть обновлённое состояние через переменные
    SELECT gp3.position, gp3.balance, gp3.moves_made, gp3.total_spent, gp3.total_earned
    INTO v_pos, v_bal, v_moves, v_spent, v_earned
    FROM game_participants gp3
    WHERE gp3.game_id = p_game_id AND gp3.user_id = p_user_id;

    RETURN QUERY SELECT v_pos, v_bal, v_moves, v_spent, v_earned;
END;
$$ LANGUAGE plpgsql;


DROP FUNCTION IF EXISTS pay_rent(UUID, UUID, INT);
CREATE OR REPLACE FUNCTION pay_rent(
    p_game_id    UUID,
    p_user_id    UUID,
    p_cell_index INT
)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
DECLARE
    v_prop_id  UUID;
    v_owner_id UUID;
    v_rent     BIGINT;
    v_pos      INT;
    v_bal      BIGINT;
    v_moves    INT;
    v_spent    BIGINT;
    v_earned   BIGINT;
BEGIN
    SELECT gc.property_id INTO v_prop_id
    FROM game_cells gc
    WHERE gc.game_id = p_game_id AND gc.cell_index = p_cell_index;

    IF v_prop_id IS NULL THEN
        RAISE EXCEPTION 'Собственность не найдена на клетке %', p_cell_index;
    END IF;

    SELECT p.rent_cost, p.owner_user_id INTO v_rent, v_owner_id
    FROM properties p WHERE p.id = v_prop_id;

    -- Списать аренду у арендатора
    UPDATE game_participants gp2
    SET balance     = gp2.balance - v_rent,
        total_spent = gp2.total_spent + v_rent
    WHERE gp2.game_id = p_game_id AND gp2.user_id = p_user_id;

    -- Начислить владельцу
    UPDATE game_participants gp3
    SET balance      = gp3.balance + v_rent,
        total_earned = gp3.total_earned + v_rent
    WHERE gp3.game_id = p_game_id AND gp3.user_id = v_owner_id;

    -- Вернуть обновлённое состояние арендатора через переменные
    SELECT gp4.position, gp4.balance, gp4.moves_made, gp4.total_spent, gp4.total_earned
    INTO v_pos, v_bal, v_moves, v_spent, v_earned
    FROM game_participants gp4
    WHERE gp4.game_id = p_game_id AND gp4.user_id = p_user_id;

    RETURN QUERY SELECT v_pos, v_bal, v_moves, v_spent, v_earned;
END;
$$ LANGUAGE plpgsql;

-- -------------------------------------------------------------------
-- БЛОК 7: Магазин усилений
-- -------------------------------------------------------------------

-- Получить слоты магазина по shop_id.
-- Возвращает 4 слота с усилениями (или NULL если слот пуст/продан).
DROP FUNCTION IF EXISTS get_shop_slots(UUID, UUID, UUID);
CREATE OR REPLACE FUNCTION get_shop_slots(
    p_shop_id  UUID,
    p_game_id  UUID,
    p_user_id  UUID
)
RETURNS TABLE (
    slot_index    INT,
    slot_id       UUID,
    power_up_id   UUID,
    name          TEXT,
    description   TEXT,
    cost          BIGINT,
    status        TEXT,
    already_own   BOOLEAN,
    reroll_count  INT
) AS $$
DECLARE
    v_offset INT;
BEGIN
    SELECT s.offset_value INTO v_offset
    FROM shops s WHERE s.id = p_shop_id;

    RETURN QUERY
    SELECT
        ss.slot_index,
        ss.id,
        ss.power_up_id,
        pu.name::TEXT,
        pu.description::TEXT,
        ss.cost,
        ss.status::TEXT,
        EXISTS (
            SELECT 1 FROM player_inventory pi
            WHERE pi.game_id = p_game_id
              AND pi.user_id = p_user_id
              AND pi.power_up_id = ss.power_up_id
        ),
        v_offset
    FROM shop_slots ss
    JOIN power_ups pu ON pu.id = ss.power_up_id
    WHERE ss.shop_id = p_shop_id
    ORDER BY ss.slot_index;
END;
$$ LANGUAGE plpgsql;


-- Купить усиление из магазина.
-- Списывает деньги, добавляет в инвентарь, помечает слот как sold.
DROP FUNCTION IF EXISTS buy_shop_slot(UUID, UUID, UUID);
CREATE OR REPLACE FUNCTION buy_shop_slot(
    p_slot_id  UUID,
    p_game_id  UUID,
    p_user_id  UUID
)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
DECLARE
    v_cost       BIGINT;
    v_pu_id      UUID;
    v_status     TEXT;
    v_pos        INT;
    v_bal        BIGINT;
    v_moves      INT;
    v_spent      BIGINT;
    v_earned     BIGINT;
BEGIN
    SELECT ss.cost, ss.power_up_id, ss.status
    INTO v_cost, v_pu_id, v_status
    FROM shop_slots ss WHERE ss.id = p_slot_id;

    IF v_status != 'available' THEN
        RAISE EXCEPTION 'Слот недоступен';
    END IF;

    SELECT gp.balance INTO v_bal
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_user_id;

    IF v_bal < v_cost THEN
        RAISE EXCEPTION 'Недостаточно средств: нужно %, есть %', v_cost, v_bal;
    END IF;

    -- Списать деньги
    UPDATE game_participants gp2
    SET balance     = gp2.balance - v_cost,
        total_spent = gp2.total_spent + v_cost
    WHERE gp2.game_id = p_game_id AND gp2.user_id = p_user_id;

    -- Добавить в инвентарь
    INSERT INTO player_inventory (game_id, user_id, power_up_id, quantity)
    VALUES (p_game_id, p_user_id, v_pu_id, 1)
    ON CONFLICT (game_id, user_id, power_up_id)
    DO UPDATE SET quantity = player_inventory.quantity + 1;

    -- Пометить слот как проданный
    UPDATE shop_slots SET status = 'sold' WHERE id = p_slot_id;

    SELECT gp3.position, gp3.balance, gp3.moves_made, gp3.total_spent, gp3.total_earned
    INTO v_pos, v_bal, v_moves, v_spent, v_earned
    FROM game_participants gp3
    WHERE gp3.game_id = p_game_id AND gp3.user_id = p_user_id;

    RETURN QUERY SELECT v_pos, v_bal, v_moves, v_spent, v_earned;
END;
$$ LANGUAGE plpgsql;


-- Реролл магазина: заменить все слоты новой четвёркой усилений.
-- Не выдаёт усиления из инвентаря игрока и не дублирует в рамках четвёрки.
-- Стоимость реролла: 50 + 15 * кол-во_реролов_этого_магазина.
DROP FUNCTION IF EXISTS reroll_shop(UUID, UUID, UUID);
CREATE OR REPLACE FUNCTION reroll_shop(
    p_shop_id  UUID,
    p_game_id  UUID,
    p_user_id  UUID
)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
DECLARE
    v_reroll_count INT;
    v_cost         BIGINT;
    v_bal          BIGINT;
    v_game_seed    BIGINT;
    v_available    UUID[];
    v_chosen       UUID[] := '{}';
    v_pu_id        UUID;
    v_i            INT;
    v_hash         TEXT;
    v_idx          INT;
    v_pos          INT;
    v_moves        INT;
    v_spent        BIGINT;
    v_earned       BIGINT;
BEGIN
    -- Считаем сколько раз уже делали реролл через offset_value магазина
    SELECT COALESCE(s.offset_value, 0) INTO v_reroll_count
    FROM shops s WHERE s.id = p_shop_id;

    v_cost := 50 + 15 * v_reroll_count;

    SELECT gp.balance INTO v_bal
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_user_id;

    IF v_bal < v_cost THEN
        RAISE EXCEPTION 'Недостаточно средств для реролла: нужно %, есть %', v_cost, v_bal;
    END IF;

    -- Seed игры для детерминированной генерации
    SELECT g.seed INTO v_game_seed FROM games g WHERE g.id = p_game_id;

    -- Все усиления которых нет в инвентаре игрока
    SELECT ARRAY(
        SELECT pu.id FROM power_ups pu
        WHERE NOT EXISTS (
            SELECT 1 FROM player_inventory pi
            WHERE pi.game_id = p_game_id
              AND pi.user_id = p_user_id
              AND pi.power_up_id = pu.id
        )
        ORDER BY md5((v_game_seed::TEXT || p_shop_id::TEXT ||
                      v_reroll_count::TEXT || pu.id::TEXT))
    ) INTO v_available;

    -- Выбираем 4 уникальных усиления
    v_i := 1;
    FOREACH v_pu_id IN ARRAY v_available LOOP
        EXIT WHEN v_i > 4;
        -- Не дублируем в рамках четвёрки (v_available уже уникален по id)
        v_chosen := array_append(v_chosen, v_pu_id);
        v_i := v_i + 1;
    END LOOP;

    IF array_length(v_chosen, 1) < 1 THEN
        RAISE EXCEPTION 'Нет доступных усилений для реролла';
    END IF;

    -- Удаляем старые слоты (они заменяются новыми)
    DELETE FROM shop_slots WHERE shop_id = p_shop_id;

    -- Увеличиваем счётчик реролов
    UPDATE shops SET offset_value = offset_value + 1 WHERE id = p_shop_id;

    -- Создаём новые слоты
    FOR v_i IN 1..array_length(v_chosen, 1) LOOP
        INSERT INTO shop_slots (shop_id, power_up_id, slot_index, status, cost)
        VALUES (
            p_shop_id,
            v_chosen[v_i],
            v_i - 1,
            'available',
            (SELECT pu.cost FROM power_ups pu WHERE pu.id = v_chosen[v_i])
        );
    END LOOP;

    -- Списать стоимость реролла
    UPDATE game_participants gp2
    SET balance     = gp2.balance - v_cost,
        total_spent = gp2.total_spent + v_cost
    WHERE gp2.game_id = p_game_id AND gp2.user_id = p_user_id;

    SELECT gp3.position, gp3.balance, gp3.moves_made, gp3.total_spent, gp3.total_earned
    INTO v_pos, v_bal, v_moves, v_spent, v_earned
    FROM game_participants gp3
    WHERE gp3.game_id = p_game_id AND gp3.user_id = p_user_id;

    RETURN QUERY SELECT v_pos, v_bal, v_moves, v_spent, v_earned;
END;
$$ LANGUAGE plpgsql;

-- -------------------------------------------------------------------
-- БЛОК 8: Продажа усилений
-- -------------------------------------------------------------------

-- Продать усиление из инвентаря: вернуть половину стоимости, удалить из инвентаря.
DROP FUNCTION IF EXISTS sell_power_up(UUID, UUID, UUID);
CREATE OR REPLACE FUNCTION sell_power_up(
    p_game_id    UUID,
    p_user_id    UUID,
    p_power_up_id UUID
)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
DECLARE
    v_cost   BIGINT;
    v_refund BIGINT;
    v_qty    INT;
    v_pos    INT;
    v_bal    BIGINT;
    v_moves  INT;
    v_spent  BIGINT;
    v_earned BIGINT;
BEGIN
    SELECT pu.cost INTO v_cost
    FROM power_ups pu WHERE pu.id = p_power_up_id;

    v_refund := v_cost / 2;

    SELECT pi.quantity INTO v_qty
    FROM player_inventory pi
    WHERE pi.game_id = p_game_id
      AND pi.user_id = p_user_id
      AND pi.power_up_id = p_power_up_id;

    IF v_qty IS NULL OR v_qty < 1 THEN
        RAISE EXCEPTION 'Усиление не найдено в инвентаре';
    END IF;

    -- Удаляем одну единицу или весь стак
    IF v_qty <= 1 THEN
        DELETE FROM player_inventory
        WHERE game_id = p_game_id
          AND user_id = p_user_id
          AND power_up_id = p_power_up_id;
    ELSE
        UPDATE player_inventory
        SET quantity = quantity - 1
        WHERE game_id = p_game_id
          AND user_id = p_user_id
          AND power_up_id = p_power_up_id;
    END IF;

    -- Возвращаем деньги
    UPDATE game_participants gp2
    SET balance      = gp2.balance + v_refund,
        total_earned = gp2.total_earned + v_refund
    WHERE gp2.game_id = p_game_id AND gp2.user_id = p_user_id;

    SELECT gp3.position, gp3.balance, gp3.moves_made, gp3.total_spent, gp3.total_earned
    INTO v_pos, v_bal, v_moves, v_spent, v_earned
    FROM game_participants gp3
    WHERE gp3.game_id = p_game_id AND gp3.user_id = p_user_id;

    RETURN QUERY SELECT v_pos, v_bal, v_moves, v_spent, v_earned;
END;
$$ LANGUAGE plpgsql;

-- -------------------------------------------------------------------
-- БЛОК 9: Проверка банкротства
-- -------------------------------------------------------------------

DROP FUNCTION IF EXISTS get_all_balances(UUID);
CREATE OR REPLACE FUNCTION get_all_balances(p_game_id UUID)
RETURNS TABLE (
    user_id   UUID,
    balance   BIGINT,
    user_type TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT gp.user_id, gp.balance, u.type::TEXT
    FROM game_participants gp
    JOIN users u ON u.id = gp.user_id
    WHERE gp.game_id = p_game_id;
END;
$$ LANGUAGE plpgsql;

-- -------------------------------------------------------------------
-- БЛОК 10: Налог
-- -------------------------------------------------------------------

DROP FUNCTION IF EXISTS pay_tax(UUID, UUID);
CREATE OR REPLACE FUNCTION pay_tax(
    p_game_id UUID,
    p_user_id UUID
)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
DECLARE
    v_bal    BIGINT;
    v_tax    BIGINT;
    v_pos    INT;
    v_moves  INT;
    v_spent  BIGINT;
    v_earned BIGINT;
BEGIN
    SELECT gp.balance INTO v_bal
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_user_id;

    v_tax := 100 + CEIL(v_bal * 0.05);

    UPDATE game_participants gp2
    SET balance     = gp2.balance - v_tax,
        total_spent = gp2.total_spent + v_tax
    WHERE gp2.game_id = p_game_id AND gp2.user_id = p_user_id;

    SELECT gp3.position, gp3.balance, gp3.moves_made, gp3.total_spent, gp3.total_earned
    INTO v_pos, v_bal, v_moves, v_spent, v_earned
    FROM game_participants gp3
    WHERE gp3.game_id = p_game_id AND gp3.user_id = p_user_id;

    RETURN QUERY SELECT v_pos, v_bal, v_moves, v_spent, v_earned;
END;
$$ LANGUAGE plpgsql;