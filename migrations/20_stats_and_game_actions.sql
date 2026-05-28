-- =============================================================
-- DreamBreaker — функции статистики профиля, сдачи, сохранения.
-- =============================================================


-- -------------------------------------------------------------------
-- Сохранить и выйти: перевести игру в paused.
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS pause_game(UUID, UUID);
CREATE OR REPLACE FUNCTION pause_game(p_game_id UUID, p_user_id UUID)
RETURNS VOID AS $$
BEGIN
    -- Проверяем что игрок участник этой игры
    IF NOT EXISTS (
        SELECT 1 FROM game_participants
        WHERE game_id = p_game_id AND user_id = p_user_id
    ) THEN
        RAISE EXCEPTION 'Игрок не является участником игры';
    END IF;

    UPDATE games SET status = 'paused' WHERE id = p_game_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Сдаться: перевести игру в surrender.
-- Поражение засчитывается по завершении — статус surrender.
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS surrender_game(UUID, UUID);
CREATE OR REPLACE FUNCTION surrender_game(p_game_id UUID, p_user_id UUID)
RETURNS VOID AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM game_participants
        WHERE game_id = p_game_id AND user_id = p_user_id
    ) THEN
        RAISE EXCEPTION 'Игрок не является участником игры';
    END IF;

    UPDATE games SET status = 'surrender' WHERE id = p_game_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Статистика профиля (динамически по завершённым играм).
-- Считает только игры со статусом finished (победа) или surrender (поражение).
--
-- Победа: баланс >= target_balance на момент завершения.
-- Поражение: статус surrender.
-- Текущая серия побед: количество последних подряд идущих finished-игр
--   (сортировка по last_saved_at DESC, до первого не-finished).
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS get_user_stats(UUID);
CREATE OR REPLACE FUNCTION get_user_stats(p_user_id UUID)
RETURNS TABLE (
    total_games       INT,
    total_wins        INT,
    current_win_streak INT,
    total_moves       BIGINT,
    total_earned      BIGINT,
    total_spent       BIGINT,
    properties_bought BIGINT,
    power_ups_bought  BIGINT
) AS $$
DECLARE
    v_streak       INT := 0;
    v_game_status  TEXT;
    v_game_balance BIGINT;
    v_game_target  BIGINT;
BEGIN
    -- Текущая серия побед: идём по играм от новых к старым,
    -- считаем пока идут подряд finished с balance >= target
    FOR v_game_status, v_game_balance, v_game_target IN
        SELECT g.status, gp.balance, gr.target_balance
        FROM games g
        JOIN game_participants gp ON g.id = gp.game_id
        JOIN game_rules gr ON g.id = gr.game_id
        WHERE gp.user_id = p_user_id
          AND g.status IN ('finished', 'surrender')
        ORDER BY g.last_saved_at DESC
    LOOP
        IF v_game_status = 'finished' AND v_game_balance >= v_game_target THEN
            v_streak := v_streak + 1;
        ELSE
            EXIT; -- первое поражение/сдача — серия прерывается
        END IF;
    END LOOP;

    RETURN QUERY
    SELECT
        -- Всего завершённых игр (finished + surrender)
        COUNT(DISTINCT g.id)::INT,

        -- Побед: finished И баланс >= target
        COUNT(DISTINCT CASE
            WHEN g.status = 'finished' AND gp.balance >= gr.target_balance
            THEN g.id END)::INT,

        -- Серия (вычислена выше)
        v_streak,

        -- Всего ходов
        COALESCE(SUM(gp.moves_made), 0)::BIGINT,

        -- Всего заработано
        COALESCE(SUM(gp.total_earned), 0)::BIGINT,

        -- Всего потрачено
        COALESCE(SUM(gp.total_spent), 0)::BIGINT,

        -- Куплено собственности: считаем property клетки где owner = этот игрок
        -- в завершённых играх
        (SELECT COUNT(*)
         FROM properties p
         JOIN game_cells gc ON gc.property_id = p.id
         JOIN games g2 ON gc.game_id = g2.id
         WHERE p.owner_user_id = p_user_id
           AND g2.status IN ('finished', 'surrender'))::BIGINT,

        -- Куплено усилений: суммируем quantity по инвентарю завершённых игр
        (SELECT COALESCE(SUM(pi.quantity), 0)
         FROM player_inventory pi
         JOIN games g2 ON pi.game_id = g2.id
         WHERE pi.user_id = p_user_id
           AND g2.status IN ('finished', 'surrender'))::BIGINT

    FROM games g
    JOIN game_participants gp ON g.id = gp.game_id
    JOIN game_rules gr ON g.id = gr.game_id
    WHERE gp.user_id = p_user_id
      AND g.status IN ('finished', 'surrender');
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Получить итоги завершённой игры для экрана завершения.
-- Возвращает статистику конкретной партии + способ завершения.
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS get_game_result(UUID, UUID);
CREATE OR REPLACE FUNCTION get_game_result(p_game_id UUID, p_user_id UUID)
RETURNS TABLE (
    game_status    TEXT,
    balance        BIGINT,
    target_balance BIGINT,
    moves_made     INT,
    max_turns      INT,
    total_earned   BIGINT,
    total_spent    BIGINT,
    is_victory     BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        g.status::TEXT,
        gp.balance,
        gr.target_balance,
        gp.moves_made,
        gr.max_turns,
        gp.total_earned,
        gp.total_spent,
        (g.status = 'finished' AND gp.balance >= gr.target_balance)
    FROM games g
    JOIN game_participants gp ON g.id = gp.game_id
    JOIN game_rules gr ON g.id = gr.game_id
    WHERE g.id = p_game_id AND gp.user_id = p_user_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;