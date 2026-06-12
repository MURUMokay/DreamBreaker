-- =============================================================
-- DreamBreaker — логика ходов ботов.
-- =============================================================

-- -------------------------------------------------------------------
-- Получить список живых ботов игры (balance > 0) по порядку хода.
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS get_bot_participants(UUID);
CREATE OR REPLACE FUNCTION get_bot_participants(p_game_id UUID)
RETURNS TABLE (
    user_id    UUID,
    username   TEXT,
    "position" INT,
    balance    BIGINT,
    turn_order INT
) AS $$
BEGIN
    RETURN QUERY
    SELECT gp.user_id, u.username::TEXT, gp."position", gp.balance, gp.turn_order
    FROM game_participants gp
    JOIN users u ON u.id = gp.user_id
    WHERE gp.game_id = p_game_id
      AND u.type = 'bot'
      AND gp.balance > 0
    ORDER BY gp.turn_order;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Выполнить один ход бота.
--
-- Алгоритм:
--   1. Вычислить новую позицию.
--   2. Вызвать commit_player_move — обновляет позицию, moves_made,
--      начисляет бонус за СТАРТ (та же логика, что и для игрока).
--   3. Определить тип клетки.
--   4. property + свободная + хватает баланса → buy_property.
--   5. property + чужая → проверить, хватает ли баланса на аренду.
--        Хватает  → pay_rent.
--        Не хватит → банкротство: balance = -1, освободить собственность.
--   6. tax → проверить, хватает ли баланса.
--        Хватает  → pay_tax.
--        Не хватит → банкротство.
--   7. Остальные типы (start, shop, своя собственность) — ничего.
--
-- Возвращает одну строку:
--   new_position, new_balance,
--   action        : 'bought' | 'rent_paid' | 'tax_paid' |
--                   'bankrupt' | 'passed' | 'start_bonus'
--   action_detail : пояснение (название собственности, сумма и т.п.)
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS do_bot_turn(UUID, UUID, INT);
CREATE OR REPLACE FUNCTION do_bot_turn(
    p_game_id  UUID,
    p_bot_id   UUID,
    p_dice     INT        -- сумма двух кубиков (2..12)
)
RETURNS TABLE (
    new_position  INT,
    new_balance   BIGINT,
    action        TEXT,
    action_detail TEXT
) AS $$
DECLARE
    v_old_pos    INT;
    v_new_pos    INT;
    v_balance    BIGINT;
    v_cell_type  TEXT;
    v_prop_id    UUID;
    v_cell_index INT;
    v_owner_id   UUID;
    v_buy_cost   BIGINT;
    v_rent       BIGINT;
    v_tax        BIGINT;
    v_prop_name  TEXT;
    v_action     TEXT    := 'passed';
    v_detail     TEXT    := '';
    v_bonus      BIGINT;    -- бонус, начисленный commit_player_move
BEGIN
    -- Читаем текущую позицию
    SELECT gp."position"
    INTO v_old_pos
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_bot_id;

    v_new_pos := (v_old_pos + p_dice) % 40;

    -- 1. Зафиксировать перемещение (бонус за СТАРТ — внутри функции)
    --    Возвращаемый balance уже включает возможный бонус.
    SELECT cpm.balance INTO v_balance
    FROM commit_player_move(p_game_id, p_bot_id, v_new_pos) cpm;

    -- Определить, был ли начислен бонус за старт
    -- (balance после move - balance до move, без учёта трат)
    -- Нужно лишь для action_detail — логируем отдельно если встали/прошли СТАРТ.
    IF v_new_pos = 0 THEN
        v_action := 'start_bonus';
        v_detail := '400';
        RETURN QUERY SELECT v_new_pos, v_balance, v_action, v_detail;
        RETURN;
    ELSIF v_old_pos > v_new_pos THEN
        -- Прошли СТАРТ — но продолжаем обрабатывать клетку
        v_action := 'start_bonus';
        v_detail := '200';
    END IF;

    -- 2. Определить тип клетки
    SELECT gc.cell_type, gc.property_id, gc.cell_index
    INTO v_cell_type, v_prop_id, v_cell_index
    FROM game_cells gc
    WHERE gc.game_id = p_game_id AND gc.cell_index = v_new_pos;

    -- 3. Обработать клетку
    IF v_cell_type = 'property' THEN

        SELECT p.owner_user_id, p.purchase_cost, p.name
        INTO v_owner_id, v_buy_cost, v_prop_name
        FROM properties p WHERE p.id = v_prop_id;

        IF v_owner_id IS NULL THEN
            -- Свободная — купить, если хватает
            IF v_balance >= v_buy_cost THEN
                -- Используем существующую buy_property
                PERFORM buy_property(p_game_id, p_bot_id, v_cell_index);
                v_action := 'bought';
                v_detail := v_prop_name;
            END IF;
            -- Иначе: action остаётся 'passed' или 'start_bonus'

        ELSIF v_owner_id != p_bot_id THEN
            -- Чужая — нужно платить аренду
            -- pay_rent использует p.rent_cost (без усилений) — то же поведение, что у игрока
            v_rent := calc_rent(p_game_id, v_prop_id);

IF v_balance >= v_rent THEN
    PERFORM pay_rent(p_game_id, p_bot_id, v_cell_index);
                v_action := 'rent_paid';
                v_detail := v_prop_name || ':' || v_rent::TEXT;
            ELSE
                -- Банкротство
                UPDATE game_participants
                SET balance = -1
                WHERE game_id = p_game_id AND user_id = p_bot_id;

                -- Освобождаем всю собственность бота в этой игре
                UPDATE properties p2
                SET owner_game_id = NULL,
                    owner_user_id = NULL
                FROM game_cells gc
                WHERE gc.id = p2.cell_id
                  AND gc.game_id = p_game_id
                  AND p2.owner_user_id = p_bot_id
                  AND p2.owner_game_id = p_game_id;

                v_action := 'bankrupt';
                v_detail := v_prop_name;
                RETURN QUERY SELECT v_new_pos, -1::BIGINT, v_action, v_detail;
                RETURN;
            END IF;
        END IF;
        -- Своя собственность — ничего не делаем, action уже 'passed'/'start_bonus'

    ELSIF v_cell_type = 'tax' THEN

        v_tax := 100 + CEIL(v_balance * 0.05);

        IF v_balance >= v_tax THEN
            PERFORM pay_tax(p_game_id, p_bot_id);
            v_action := 'tax_paid';
            v_detail := v_tax::TEXT;
        ELSE
            -- Банкротство
            UPDATE game_participants
            SET balance = -1
            WHERE game_id = p_game_id AND user_id = p_bot_id;

            UPDATE properties p2
            SET owner_game_id = NULL,
                owner_user_id = NULL
            FROM game_cells gc
            WHERE gc.id = p2.cell_id
              AND gc.game_id = p_game_id
              AND p2.owner_user_id = p_bot_id
              AND p2.owner_game_id = p_game_id;

            v_action := 'bankrupt';
            v_detail := 'налог ' || v_tax::TEXT;
            RETURN QUERY SELECT v_new_pos, -1::BIGINT, v_action, v_detail;
            RETURN;
        END IF;

    END IF;
    -- shop / start / угол — ничего дополнительного не делаем

    -- Перечитываем финальный баланс после всех операций
    SELECT gp.balance INTO v_balance
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_bot_id;

    RETURN QUERY SELECT v_new_pos, v_balance, v_action, v_detail;
END;
$$ LANGUAGE plpgsql;