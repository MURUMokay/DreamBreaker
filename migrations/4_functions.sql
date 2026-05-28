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
CREATE OR REPLACE FUNCTION register_new_user(
    p_username TEXT,
    p_type TEXT,
    p_password_hash TEXT
) RETURNS UUID AS $$
DECLARE
    new_id UUID;
BEGIN
    INSERT INTO users (username, type, password_hash)
    VALUES (p_username, p_type, p_password_hash)
    RETURNING id INTO new_id;
    RETURN new_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Получить credentials пользователя по имени для проверки пароля в Rust.
-- Возвращает одну строку или пусто (если пользователь не найден).
DROP FUNCTION IF EXISTS get_user_credentials(TEXT);
CREATE OR REPLACE FUNCTION get_user_credentials(p_username TEXT)
RETURNS TABLE (user_id UUID, pass_hash TEXT, active BOOLEAN) AS $$
BEGIN
    RETURN QUERY
    SELECT u.id, u.password_hash::TEXT, u.is_active
    FROM users u
    WHERE u.username = p_username;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Получить полные данные пользователя по его ID.
-- Используется после успешной проверки пароля.
DROP FUNCTION IF EXISTS get_user_by_id(UUID);
CREATE OR REPLACE FUNCTION get_user_by_id(p_user_id UUID)
RETURNS TABLE (
    id UUID,
    username TEXT,
    type TEXT,
    password_hash TEXT,
    created_at TIMESTAMPTZ,
    is_active BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT u.id,
           u.username::TEXT,
           u.type::TEXT,
           u.password_hash::TEXT,
           u.created_at,
           u.is_active
    FROM users u
    WHERE u.id = p_user_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Получить список всех пользователей (для меню выбора профиля).
DROP FUNCTION IF EXISTS list_users();
CREATE OR REPLACE FUNCTION list_users()
RETURNS TABLE (
    id UUID,
    username TEXT,
    type TEXT,
    created_at TIMESTAMPTZ,
    is_active BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT u.id,
           u.username::TEXT,
           u.type::TEXT,
           u.created_at,
           u.is_active
    FROM users u
    WHERE u.is_active = TRUE
      AND u.type = 'human'
    ORDER BY u.created_at;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- БЛОК 2: Игры
-- -------------------------------------------------------------------

-- Создать игру вместе с правилами (связь 1:1) в одной транзакции.
-- Возвращает UUID новой игры.
DROP FUNCTION IF EXISTS create_game_with_rules(BIGINT, BIGINT, INT, BIGINT);
CREATE OR REPLACE FUNCTION create_game_with_rules(
    p_seed BIGINT,
    p_starting_balance BIGINT,
    p_max_turns INT,
    p_target_balance BIGINT
) RETURNS UUID AS $$
DECLARE
    new_game_id UUID;
BEGIN
    INSERT INTO games (seed)
    VALUES (p_seed)
    RETURNING id INTO new_game_id;

    INSERT INTO game_rules (game_id, starting_balance, max_turns, target_balance)
    VALUES (new_game_id, p_starting_balance, p_max_turns, p_target_balance);

    RETURN new_game_id;
END;
$$ LANGUAGE plpgsql;


-- Получить список всех игр, отсортированный по дате последнего сохранения.
DROP FUNCTION IF EXISTS list_games();
CREATE OR REPLACE FUNCTION list_games()
RETURNS TABLE (
    id UUID,
    status TEXT,
    seed BIGINT,
    created_at TIMESTAMPTZ,
    last_saved_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT g.id,
           g.status::TEXT,
           g.seed,
           g.created_at,
           g.last_saved_at
    FROM games g
    ORDER BY g.last_saved_at DESC;
END;
$$ LANGUAGE plpgsql;


-- Получить правила конкретной игры (связь 1:1).
DROP FUNCTION IF EXISTS get_game_rules(UUID);
CREATE OR REPLACE FUNCTION get_game_rules(p_game_id UUID)
RETURNS TABLE (
    game_id UUID,
    starting_balance BIGINT,
    max_turns INT,
    target_balance BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT gr.game_id, gr.starting_balance, gr.max_turns, gr.target_balance
    FROM game_rules gr
    WHERE gr.game_id = p_game_id;
END;
$$ LANGUAGE plpgsql;


-- Получить активную игру пользователя.
-- Возвращает UUID самой новой активной/ожидающей игры или NULL.
DROP FUNCTION IF EXISTS get_active_game(UUID);
CREATE OR REPLACE FUNCTION get_active_game(p_user_id UUID)
RETURNS UUID AS $$
DECLARE
    found_game_id UUID;
BEGIN
    SELECT g.id INTO found_game_id
    FROM games g
    JOIN game_participants gp ON g.id = gp.game_id
    WHERE gp.user_id = p_user_id
      AND g.status IN ('active', 'pending')
    ORDER BY g.last_saved_at DESC
    LIMIT 1;
    RETURN found_game_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Совместимость со старым именем.
DROP FUNCTION IF EXISTS get_active_game_for_user(UUID);
CREATE OR REPLACE FUNCTION get_active_game_for_user(p_user_id UUID)
RETURNS UUID AS $$
BEGIN
    RETURN get_active_game(p_user_id);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Проверить, есть ли у пользователя активная игра.
DROP FUNCTION IF EXISTS user_has_active_game(UUID);
CREATE OR REPLACE FUNCTION user_has_active_game(p_user_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM games g
        JOIN game_participants gp ON g.id = gp.game_id
        WHERE gp.user_id = p_user_id
          AND g.status IN ('active', 'pending')
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Получить все игры пользователя со статистикой.
-- Возвращает все игры включая завершённые (для экрана загрузки).
-- Время создания в UTC+7.
DROP FUNCTION IF EXISTS get_user_games(UUID);
CREATE OR REPLACE FUNCTION get_user_games(p_user_id UUID)
RETURNS TABLE (
    game_id    UUID,
    status     TEXT,
    balance    BIGINT,
    moves_made INT,
    created_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT g.id,
           g.status::TEXT,
           gp.balance,
           gp.moves_made,
           (g.created_at AT TIME ZONE 'Asia/Krasnoyarsk')::TIMESTAMPTZ
    FROM games g
    JOIN game_participants gp ON g.id = gp.game_id
    WHERE gp.user_id = p_user_id
    ORDER BY g.last_saved_at DESC;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Получить последнюю активную/приостановленную игру пользователя.
-- Используется для кнопки "Продолжить игру" — загружает сразу.
DROP FUNCTION IF EXISTS get_latest_user_game(UUID);
CREATE OR REPLACE FUNCTION get_latest_user_game(p_user_id UUID)
RETURNS TABLE (
    game_id    UUID,
    status     TEXT,
    balance    BIGINT,
    moves_made INT,
    created_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT g.id,
           g.status::TEXT,
           gp.balance,
           gp.moves_made,
           (g.created_at AT TIME ZONE 'Asia/Krasnoyarsk')::TIMESTAMPTZ
    FROM games g
    JOIN game_participants gp ON g.id = gp.game_id
    WHERE gp.user_id = p_user_id
      AND g.status IN ('active', 'pending', 'paused')
    ORDER BY g.last_saved_at DESC
    LIMIT 1;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- Изменить статус игры.
DROP FUNCTION IF EXISTS set_game_status(UUID, TEXT);
CREATE OR REPLACE FUNCTION set_game_status(p_game_id UUID, p_status TEXT)
RETURNS VOID AS $$
BEGIN
    UPDATE games SET status = p_status WHERE id = p_game_id;
END;
$$ LANGUAGE plpgsql;


-- -------------------------------------------------------------------
-- БЛОК 3: Участники игры
-- -------------------------------------------------------------------

-- Добавить участника в игру.
-- Стартовый баланс берётся автоматически из game_rules этой игры.
DROP FUNCTION IF EXISTS add_game_participant(UUID, UUID, INT);
CREATE OR REPLACE FUNCTION add_game_participant(
    p_game_id UUID,
    p_user_id UUID,
    p_turn_order INT
) RETURNS VOID AS $$
BEGIN
    INSERT INTO game_participants (game_id, user_id, turn_order, balance)
    VALUES (
        p_game_id,
        p_user_id,
        p_turn_order,
        (SELECT starting_balance FROM game_rules WHERE game_id = p_game_id)
    );
END;
$$ LANGUAGE plpgsql;


-- Получить список участников игры в порядке хода.
DROP FUNCTION IF EXISTS list_game_participants(UUID);
CREATE OR REPLACE FUNCTION list_game_participants(p_game_id UUID)
RETURNS TABLE (
    game_id UUID,
    user_id UUID,
    "position" INT,
    balance BIGINT,
    moves_made INT,
    total_spent BIGINT,
    total_earned BIGINT,
    turn_order INT
) AS $$
BEGIN
    RETURN QUERY
    SELECT gp.game_id, gp.user_id, gp.position, gp.balance,
           gp.moves_made, gp.total_spent, gp.total_earned, gp.turn_order
    FROM game_participants gp
    WHERE gp.game_id = p_game_id
    ORDER BY gp.turn_order;
END;
$$ LANGUAGE plpgsql;


-- -------------------------------------------------------------------
-- БЛОК 4: Усиления и инвентарь
-- -------------------------------------------------------------------

-- Получить все доступные усиления (справочник).
DROP FUNCTION IF EXISTS list_power_ups();
CREATE OR REPLACE FUNCTION list_power_ups()
RETURNS TABLE (
    id UUID,
    name TEXT,
    description TEXT,
    cost BIGINT,
    effect JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT p.id,
           p.name::TEXT,
           p.description::TEXT,
           p.cost,
           p.effect
    FROM power_ups p
    ORDER BY p.cost;
END;
$$ LANGUAGE plpgsql;


-- Добавить усиление в инвентарь игрока (или увеличить количество).
DROP FUNCTION IF EXISTS add_to_inventory(UUID, UUID, UUID, INT);
CREATE OR REPLACE FUNCTION add_to_inventory(
    p_game_id UUID,
    p_user_id UUID,
    p_power_up_id UUID,
    p_quantity INT
) RETURNS VOID AS $$
BEGIN
    INSERT INTO player_inventory (game_id, user_id, power_up_id, quantity)
    VALUES (p_game_id, p_user_id, p_power_up_id, p_quantity)
    ON CONFLICT (game_id, user_id, power_up_id)
    DO UPDATE SET quantity = player_inventory.quantity + EXCLUDED.quantity;
END;
$$ LANGUAGE plpgsql;


-- Получить инвентарь игрока в конкретной игре.
DROP FUNCTION IF EXISTS get_player_inventory(UUID, UUID);
CREATE OR REPLACE FUNCTION get_player_inventory(p_game_id UUID, p_user_id UUID)
RETURNS TABLE (
    power_up_id UUID,
    name TEXT,
    quantity INT,
    effect JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT pi.power_up_id,
           pu.name::TEXT,
           pi.quantity,
           pu.effect
    FROM player_inventory pi
    JOIN power_ups pu ON pu.id = pi.power_up_id
    WHERE pi.game_id = p_game_id AND pi.user_id = p_user_id
    ORDER BY pu.name;
END;
$$ LANGUAGE plpgsql;