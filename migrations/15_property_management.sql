-- =============================================================
-- DreamBreaker — управление собственностью игрока.
-- install_upgrade / uninstall_upgrade / get_player_properties
-- =============================================================

-- -------------------------------------------------------------------
-- Восстанавливаем get_board_cells в оригинальном виде (без upgrades_count).
-- upgrades_count не нужен в BoardCell — он вычисляется на стороне клиента
-- из get_player_properties.
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS get_board_cells(UUID);
CREATE OR REPLACE FUNCTION get_board_cells(p_game_id UUID)
RETURNS TABLE (
    cell_index    INT,
    cell_type     TEXT,
    tax_amount    BIGINT,
    prop_name     TEXT,
    purchase_cost BIGINT,
    rent_cost     BIGINT,
    owner_user_id UUID,
    shop_id       UUID,
    refresh_cost  BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        gc.cell_index,
        gc.cell_type::TEXT,
        gc.tax_amount,
        p.name::TEXT,
        p.purchase_cost,
        p.rent_cost,
        p.owner_user_id,
        s.id,
        NULL::BIGINT
    FROM game_cells gc
    LEFT JOIN properties p ON p.id = gc.property_id
    LEFT JOIN shops s ON s.id = gc.shop_id
    WHERE gc.game_id = p_game_id
    ORDER BY gc.cell_index;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Получить собственности игрока с деталями усилений.
-- Возвращает по одной строке на каждую собственность.
-- upgrades — JSONB-массив объектов {power_up_id, name, effect}
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS get_player_properties(UUID, UUID);
CREATE OR REPLACE FUNCTION get_player_properties(p_game_id UUID, p_user_id UUID)
RETURNS TABLE (
    property_id    UUID,
    cell_index     INT,
    prop_name      TEXT,
    purchase_cost  BIGINT,
    rent_cost      BIGINT,
    upgrades       JSONB,        -- текущие установленные усиления
    upgrades_count INT,
    max_upgrades   INT           -- максимум слотов (всегда 3)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        p.id,
        gc.cell_index,
        p.name::TEXT,
        p.purchase_cost,
        p.rent_cost,
        p.upgrades,
        COALESCE(jsonb_array_length(p.upgrades), 0),
        3
    FROM properties p
    JOIN game_cells gc ON gc.id = p.cell_id
    WHERE gc.game_id = p_game_id
      AND p.owner_user_id = p_user_id
    ORDER BY gc.cell_index;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Установить усиление в собственность.
--
-- Правила:
--   1. Собственность должна принадлежать игроку.
--   2. Усиление должно быть в инвентаре игрока (quantity >= 1).
--   3. В собственности не может быть более 3 усилений.
--   4. Одно и то же усиление нельзя установить дважды в одну собственность.
--
-- При успехе:
--   - убирает 1 единицу из инвентаря (если quantity=1 — удаляет строку)
--   - добавляет запись в properties.upgrades
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS install_upgrade(UUID, UUID, UUID, UUID);
CREATE OR REPLACE FUNCTION install_upgrade(
    p_game_id      UUID,
    p_user_id      UUID,
    p_property_id  UUID,
    p_power_up_id  UUID
)
RETURNS TEXT AS $$   -- 'ok' или текст ошибки
DECLARE
    v_owner_id     UUID;
    v_inv_qty      INT;
    v_upg_count    INT;
    v_already      BOOLEAN;
    v_pu_name      TEXT;
    v_pu_effect    JSONB;
BEGIN
    -- Проверяем владельца
    SELECT owner_user_id INTO v_owner_id
    FROM properties WHERE id = p_property_id;

    IF v_owner_id IS DISTINCT FROM p_user_id THEN
        RETURN 'Это не ваша собственность';
    END IF;

    -- Проверяем инвентарь
    SELECT quantity INTO v_inv_qty
    FROM player_inventory
    WHERE game_id = p_game_id AND user_id = p_user_id AND power_up_id = p_power_up_id;

    IF v_inv_qty IS NULL OR v_inv_qty < 1 THEN
        RETURN 'Усиление не найдено в инвентаре';
    END IF;

    -- Проверяем количество слотов
    SELECT COALESCE(jsonb_array_length(upgrades), 0) INTO v_upg_count
    FROM properties WHERE id = p_property_id;

    IF v_upg_count >= 3 THEN
        RETURN 'Все слоты заняты (максимум 3)';
    END IF;

    -- Проверяем дубли
    SELECT EXISTS (
        SELECT 1 FROM jsonb_array_elements(
            (SELECT upgrades FROM properties WHERE id = p_property_id)
        ) elem
        WHERE (elem->>'power_up_id')::UUID = p_power_up_id
    ) INTO v_already;

    IF v_already THEN
        RETURN 'Это усиление уже установлено в данной собственности';
    END IF;

    -- Получаем данные усиления
    SELECT name, effect INTO v_pu_name, v_pu_effect
    FROM power_ups WHERE id = p_power_up_id;

    -- Списываем из инвентаря
    IF v_inv_qty = 1 THEN
        DELETE FROM player_inventory
        WHERE game_id = p_game_id AND user_id = p_user_id AND power_up_id = p_power_up_id;
    ELSE
        UPDATE player_inventory
        SET quantity = quantity - 1
        WHERE game_id = p_game_id AND user_id = p_user_id AND power_up_id = p_power_up_id;
    END IF;

    -- Добавляем в upgrades собственности
    UPDATE properties
    SET upgrades = upgrades || jsonb_build_array(
        jsonb_build_object(
            'power_up_id', p_power_up_id::TEXT,
            'name',        v_pu_name,
            'effect',      v_pu_effect
        )
    )
    WHERE id = p_property_id;

    RETURN 'ok';
END;
$$ LANGUAGE plpgsql;


-- -------------------------------------------------------------------
-- Извлечь усиление из собственности обратно в инвентарь.
-- -------------------------------------------------------------------
-- Извлечь усиление из собственности обратно в инвентарь.
DROP FUNCTION IF EXISTS uninstall_upgrade(UUID, UUID, UUID, UUID);
CREATE OR REPLACE FUNCTION uninstall_upgrade(
p_game_id      UUID,
p_user_id      UUID,
p_property_id  UUID,
p_power_up_id  UUID
)
RETURNS TEXT AS $$   -- 'ok' или текст ошибки
DECLARE
v_owner_id  UUID;
v_found     BOOLEAN;
v_inv_total INT;
v_inv_cap   INT := 5;   -- максимум инвентаря
BEGIN
-- Проверяем владельца
SELECT owner_user_id INTO v_owner_id
FROM properties WHERE id = p_property_id;

IF v_owner_id IS DISTINCT FROM p_user_id THEN
    RETURN 'Это не ваша собственность';
END IF;

-- Проверяем наличие усиления в upgrades
SELECT EXISTS (
    SELECT 1 FROM jsonb_array_elements(
        (SELECT upgrades FROM properties WHERE id = p_property_id)
    ) elem
    WHERE (elem->>'power_up_id')::UUID = p_power_up_id
) INTO v_found;

IF NOT v_found THEN
    RETURN 'Усиление не найдено в данной собственности';
END IF;

-- Проверяем место в инвентаре
SELECT COALESCE(SUM(quantity), 0) INTO v_inv_total
FROM player_inventory
WHERE game_id = p_game_id AND user_id = p_user_id;

IF v_inv_total >= v_inv_cap THEN
    RETURN 'Инвентарь полон';
END IF;

-- Обновляем upgrades: убираем элемент с matching power_up_id.
-- Т.к. дубликаты запрещены при установке, достаточно простого фильтра.
-- Сохраняем порядок элементов (ORDER BY ord).
UPDATE properties
SET upgrades = (
    SELECT COALESCE(jsonb_agg(elem ORDER BY ord), '[]'::JSONB)
    FROM jsonb_array_elements(upgrades) WITH ORDINALITY AS t(elem, ord)
    WHERE (elem->>'power_up_id')::UUID != p_power_up_id
)
WHERE id = p_property_id;

-- Возвращаем в инвентарь
INSERT INTO player_inventory (game_id, user_id, power_up_id, quantity)
VALUES (p_game_id, p_user_id, p_power_up_id, 1)
ON CONFLICT (game_id, user_id, power_up_id)
DO UPDATE SET quantity = player_inventory.quantity + 1;

RETURN 'ok';
END;
$$ LANGUAGE plpgsql;