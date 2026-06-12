DROP FUNCTION IF EXISTS pause_game(UUID, UUID);
CREATE OR REPLACE FUNCTION pause_game(p_game_id UUID, p_user_id UUID)
RETURNS VOID AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM game_participants
        WHERE game_id = p_game_id AND user_id = p_user_id
    ) THEN
        RAISE EXCEPTION 'Игрок не является участником игры';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM games
        WHERE id = p_game_id AND status IN ('active', 'pending')
    ) THEN
        RAISE EXCEPTION 'GAME_OVER: игра уже завершена или сдана';
    END IF;

    UPDATE games SET status = 'paused' WHERE id = p_game_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

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

    IF NOT EXISTS (
        SELECT 1 FROM games
        WHERE id = p_game_id AND status IN ('active', 'pending', 'paused')
    ) THEN
        RAISE EXCEPTION 'GAME_OVER: игра уже завершена или сдана';
    END IF;

    UPDATE games SET status = 'surrender' WHERE id = p_game_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

DROP FUNCTION IF EXISTS get_user_stats(UUID);
CREATE OR REPLACE FUNCTION get_user_stats(p_user_id UUID)
RETURNS TABLE (
    total_games        INT,
    total_wins         INT,
    current_win_streak INT,
    total_moves        BIGINT,
    total_earned       BIGINT,
    total_spent        BIGINT,
    properties_bought  BIGINT,
    power_ups_bought   BIGINT
) AS $$
DECLARE
    v_streak       INT := 0;
    v_game_status  TEXT;
    v_game_balance BIGINT;
    v_game_target  BIGINT;
BEGIN
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
            EXIT;
        END IF;
    END LOOP;

    RETURN QUERY
    SELECT
        COUNT(DISTINCT g.id)::INT,
        COUNT(DISTINCT CASE
            WHEN g.status = 'finished' AND gp.balance >= gr.target_balance
            THEN g.id END)::INT,
        v_streak,
        COALESCE(SUM(gp.moves_made), 0)::BIGINT,
        COALESCE(SUM(gp.total_earned), 0)::BIGINT,
        COALESCE(SUM(gp.total_spent), 0)::BIGINT,
        (SELECT COUNT(*)
         FROM properties p
         JOIN game_cells gc ON gc.property_id = p.id
         JOIN games g2 ON gc.game_id = g2.id
         WHERE p.owner_user_id = p_user_id
           AND g2.status IN ('finished', 'surrender'))::BIGINT,
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
