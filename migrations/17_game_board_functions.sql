-- =============================================================
-- DreamBreaker — функции инициализации поля и игровой логики.
-- =============================================================

-- -------------------------------------------------------------------
-- Шаблон поля (40 клеток, 4 стороны по 10):
--
-- Каждая сторона:
--   [угол]  [p][p][p] [shop] [tax] [p][p][p][p]
--
-- Угол стороны 0 = start (index 0)
-- Угол стороны 1 = shop  (index 10)
-- Угол стороны 2 = shop  (index 20)
-- Угол стороны 3 = shop  (index 30)
--
-- Цвета собственностей (по 7 на цвет, 8 цветов):
--   Цвет 1: indices 1,2,3,6,7,8,9        (сторона 0)
--   Цвет 2: indices 11,12,13,16,17,18,19 (сторона 1)
--   Цвет 3: indices 21,22,23,26,27,28,29 (сторона 2)
--   Цвет 4: indices 31,32,33,36,37,38,39 (сторона 3)
--   + 4 "тёмных" цвета на обратных позициях каждой стороны
--     (фактически те же индексы, но другой color_group)
--
-- Точная раскладка одной стороны (начиная с угла):
--   0: угол
--   1,2,3: property (цвет A)
--   4: shop
--   5: tax
--   6,7,8,9: property (цвет B)
-- -------------------------------------------------------------------

-- Названия собственностей (28 фантастических, по 7 на цвет × 4 стороны)
-- Используется массив, индекс выбирается по номеру property на поле.
-- -------------------------------------------------------------------

-- Инициализировать поле игры: создать клетки, собственности, магазины.
-- Вызывается сразу после create_game_with_rules().
-- p_seed используется для перемешивания усилений в магазинах.
DROP FUNCTION IF EXISTS init_game_board(UUID, BIGINT);
CREATE OR REPLACE FUNCTION init_game_board(p_game_id UUID, p_seed BIGINT)
RETURNS VOID AS $$
DECLARE
    -- Названия собственностей: 8 групп × 7 = 56 (берём первые 28 нужных)
    v_names TEXT[] := ARRAY[
        -- Цвет 1: Маримор
        'Парк Маримор', 'Центр Маримор', 'Рынок Маримор',
        'Район Маримор', 'Сад Маримор', 'Площадь Маримор', 'Башня Маримор',
        -- Цвет 2: Эльдор
        'Парк Эльдор', 'Центр Эльдор', 'Рынок Эльдор',
        'Район Эльдор', 'Сад Эльдор', 'Площадь Эльдор', 'Башня Эльдор',
        -- Цвет 3: Кварин
        'Парк Кварин', 'Центр Кварин', 'Рынок Кварин',
        'Район Кварин', 'Сад Кварин', 'Площадь Кварин', 'Башня Кварин',
        -- Цвет 4: Зорфис
        'Парк Зорфис', 'Центр Зорфис', 'Рынок Зорфис',
        'Район Зорфис', 'Сад Зорфис', 'Площадь Зорфис', 'Башня Зорфис',
        -- Цвет 5: Нелвар
        'Парк Нелвар', 'Центр Нелвар', 'Рынок Нелвар',
        'Район Нелвар', 'Сад Нелвар', 'Площадь Нелвар', 'Башня Нелвар',
        -- Цвет 6: Ауриен
        'Парк Ауриен', 'Центр Ауриен', 'Рынок Ауриен',
        'Район Ауриен', 'Сад Ауриен', 'Площадь Ауриен', 'Башня Ауриен',
        -- Цвет 7: Тирмас
        'Парк Тирмас', 'Центр Тирмас', 'Рынок Тирмас',
        'Район Тирмас', 'Сад Тирмас', 'Площадь Тирмас', 'Башня Тирмас',
        -- Цвет 8: Солвэн
        'Парк Солвэн', 'Центр Солвэн', 'Рынок Солвэн',
        'Район Солвэн', 'Сад Солвэн', 'Площадь Солвэн', 'Башня Солвэн'
    ];

    -- Стоимости покупки: 28 значений от 100 до 600, линейно
    -- 28 собственностей: шаг = (600 - 100) / 27 ≈ 18.5, округляем
    v_purchase_costs BIGINT[] := ARRAY[
        100, 119, 137, 156, 174, 193, 211,   -- цвет 1 (indices 0-6)
        230, 248, 267, 285, 304, 322, 341,   -- цвет 2
        359, 378, 396, 415, 433, 452, 470,   -- цвет 3
        489, 507, 526, 544, 563, 581, 600    -- цвет 4 (нет, 8 цветов)
    ];

    v_side INT;
    v_cell_id UUID;
    v_prop_id UUID;
    v_shop_id UUID;
    v_cell_index INT;
    v_prop_counter INT := 0;  -- счётчик собственностей (0..27)
    v_shop_counter INT := 0;  -- счётчик магазинов (0..4)
    v_color_group INT;
    v_purchase_cost BIGINT;
    v_rent_cost BIGINT;

    -- Усиления для магазинов (перемешиваем по seed)
    v_power_up_ids UUID[];
    v_shuffled_ids UUID[];
    v_shop_ids UUID[];
    v_slot INT;
    v_pu_index INT;
BEGIN
    -- ---------------------------------------------------------------
    -- 1. Создаём 40 клеток поля
    --    Раскладка на каждой стороне (10 клеток):
    --    [угол] [p][p][p] [shop] [tax] [p][p][p][p]
    -- ---------------------------------------------------------------
    FOR v_side IN 0..3 LOOP
        FOR v_slot IN 0..9 LOOP
            v_cell_index := v_side * 10 + v_slot;

            IF v_slot = 0 THEN
                -- Угловая клетка
                IF v_side = 0 THEN
                    INSERT INTO game_cells (game_id, cell_index, cell_type)
                    VALUES (p_game_id, v_cell_index, 'start')
                    RETURNING id INTO v_cell_id;
                ELSE
                    INSERT INTO game_cells (game_id, cell_index, cell_type)
                    VALUES (p_game_id, v_cell_index, 'shop')
                    RETURNING id INTO v_cell_id;

                    -- Создаём магазин для угловой клетки
                    INSERT INTO shops (cell_id, game_id, offset_value)
                    VALUES (v_cell_id, p_game_id, 0)
                    RETURNING id INTO v_shop_id;

                    UPDATE game_cells SET shop_id = v_shop_id WHERE id = v_cell_id;
                    v_shop_counter := v_shop_counter + 1;
                END IF;

            ELSIF v_slot IN (1, 2, 3) THEN
                -- Первые 3 property на стороне
                v_color_group := v_side * 2 + 1;  -- нечётный цвет стороны
                v_purchase_cost := v_purchase_costs[v_prop_counter + 1];
                v_rent_cost := CEIL(v_purchase_cost * 0.11);

                INSERT INTO game_cells (game_id, cell_index, cell_type)
                VALUES (p_game_id, v_cell_index, 'property')
                RETURNING id INTO v_cell_id;

                INSERT INTO properties (cell_id, name, purchase_cost, rent_cost)
                VALUES (
                    v_cell_id,
                    v_names[v_prop_counter + 1],
                    v_purchase_cost,
                    v_rent_cost
                )
                RETURNING id INTO v_prop_id;

                UPDATE game_cells SET property_id = v_prop_id WHERE id = v_cell_id;
                v_prop_counter := v_prop_counter + 1;

            ELSIF v_slot = 4 THEN
                -- Магазин внутри стороны
                INSERT INTO game_cells (game_id, cell_index, cell_type)
                VALUES (p_game_id, v_cell_index, 'shop')
                RETURNING id INTO v_cell_id;

                INSERT INTO shops (cell_id, game_id, offset_value)
                VALUES (v_cell_id, p_game_id, 0)
                RETURNING id INTO v_shop_id;

                UPDATE game_cells SET shop_id = v_shop_id WHERE id = v_cell_id;
                v_shop_counter := v_shop_counter + 1;

            ELSIF v_slot = 5 THEN
                -- Налог
                INSERT INTO game_cells (game_id, cell_index, cell_type, tax_amount)
                VALUES (p_game_id, v_cell_index, 'tax', 0);
                -- tax_amount считается динамически при попадании (100 + 5% баланса)

            ELSIF v_slot IN (6, 7, 8, 9) THEN
                -- Последние 4 property на стороне
                v_color_group := v_side * 2 + 2;  -- чётный цвет стороны
                v_purchase_cost := v_purchase_costs[v_prop_counter + 1];
                v_rent_cost := CEIL(v_purchase_cost * 0.11);

                INSERT INTO game_cells (game_id, cell_index, cell_type)
                VALUES (p_game_id, v_cell_index, 'property')
                RETURNING id INTO v_cell_id;

                INSERT INTO properties (cell_id, name, purchase_cost, rent_cost)
                VALUES (
                    v_cell_id,
                    v_names[v_prop_counter + 1],
                    v_purchase_cost,
                    v_rent_cost
                )
                RETURNING id INTO v_prop_id;

                UPDATE game_cells SET property_id = v_prop_id WHERE id = v_cell_id;
                v_prop_counter := v_prop_counter + 1;
            END IF;
        END LOOP;
    END LOOP;

    -- ---------------------------------------------------------------
    -- 2. Наполняем магазины усилениями (перемешиваем по seed)
    --    Каждый магазин получает 3 слота из перемешанного списка.
    -- ---------------------------------------------------------------

    -- Собираем все ID усилений
    SELECT ARRAY(SELECT id FROM power_ups ORDER BY cost) INTO v_power_up_ids;

    -- Псевдослучайное перемешивание через seed (Fisher-Yates на SQL)
    SELECT ARRAY(
        SELECT id FROM power_ups
        ORDER BY md5((p_seed + ROW_NUMBER() OVER (ORDER BY cost))::TEXT)
    ) INTO v_shuffled_ids;

    -- Собираем ID всех магазинов этой игры в порядке offset
    SELECT ARRAY(SELECT id FROM shops WHERE game_id = p_game_id ORDER BY id)
    INTO v_shop_ids;

    -- Заполняем каждый магазин 3 слотами (циклически по shuffled list)
    FOR v_shop_counter IN 1..array_length(v_shop_ids, 1) LOOP
        FOR v_slot IN 0..3 LOOP
            v_pu_index := ((v_shop_counter - 1) * 4 + v_slot)
                          % array_length(v_shuffled_ids, 1) + 1;

            INSERT INTO shop_slots (shop_id, power_up_id, slot_index, status, cost)
            VALUES (
                v_shop_ids[v_shop_counter],
                v_shuffled_ids[v_pu_index],
                v_slot,
                'available',
                (SELECT cost FROM power_ups WHERE id = v_shuffled_ids[v_pu_index])
            );
        END LOOP;
    END LOOP;
END;
$$ LANGUAGE plpgsql;


-- -------------------------------------------------------------------
-- Получить все клетки поля игры с данными собственности/магазина.
-- Вызывается при загрузке экрана игры.
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS get_board_cells(UUID);
CREATE OR REPLACE FUNCTION get_board_cells(p_game_id UUID)
RETURNS TABLE (
    cell_index  INT,
    cell_type   TEXT,
    tax_amount  BIGINT,
    -- property fields (NULL если не property)
    prop_name        TEXT,
    purchase_cost    BIGINT,
    rent_cost        BIGINT,
    owner_user_id    UUID,
    -- shop fields (NULL если не shop)
    shop_id          UUID,
    refresh_cost     BIGINT
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
        NULL::BIGINT  -- refresh_cost считается в Rust (50 + 15 * N)
    FROM game_cells gc
    LEFT JOIN properties p ON p.id = gc.property_id
    LEFT JOIN shops s ON s.id = gc.shop_id
    WHERE gc.game_id = p_game_id
    ORDER BY gc.cell_index;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Получить состояние участника игры (для отображения на экране).
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS get_participant_state(UUID, UUID);
CREATE OR REPLACE FUNCTION get_participant_state(p_game_id UUID, p_user_id UUID)
RETURNS TABLE (
    "position"   INT,
    balance      BIGINT,
    moves_made   INT,
    total_spent  BIGINT,
    total_earned BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT gp."position", gp.balance, gp.moves_made, gp.total_spent, gp.total_earned
    FROM game_participants gp
    WHERE gp.game_id = p_game_id AND gp.user_id = p_user_id;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Вычислить итоговую аренду собственности с учётом усилений владельца.
-- Формула: CEIL((base_cost + flat_base) * (11% + percent_bonus)) + flat_final
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS calc_rent(UUID, UUID);
CREATE OR REPLACE FUNCTION calc_rent(p_game_id UUID, p_property_id UUID)
RETURNS BIGINT AS $$
DECLARE
    v_base_cost     BIGINT;
    v_owner_user_id UUID;
    v_flat_base     BIGINT := 0;
    v_pct_bonus     NUMERIC := 0;
    v_flat_final    BIGINT := 0;
    v_result        BIGINT;
BEGIN
    SELECT p.rent_cost, p.owner_user_id
    INTO v_base_cost, v_owner_user_id
    FROM properties p WHERE p.id = p_property_id;

    IF v_owner_user_id IS NULL THEN
        RETURN 0;
    END IF;

    -- Суммируем эффекты усилений, установленных в данную собственность
    SELECT
        COALESCE(SUM(CASE WHEN (elem->'effect'->>'type') = 'flat_base'
                     THEN (elem->'effect'->>'value')::BIGINT ELSE 0 END), 0),
        COALESCE(SUM(CASE WHEN (elem->'effect'->>'type') = 'percent_bonus'
                     THEN (elem->'effect'->>'value')::NUMERIC ELSE 0 END), 0),
        COALESCE(SUM(CASE WHEN (elem->'effect'->>'type') = 'flat_final'
                     THEN (elem->'effect'->>'value')::BIGINT ELSE 0 END), 0)
    INTO v_flat_base, v_pct_bonus, v_flat_final
    FROM properties p
    CROSS JOIN jsonb_array_elements(p.upgrades) AS elem
    WHERE p.id = p_property_id;

    -- Формула: CEIL((base + flat_base) * (0.11 + pct_bonus/100)) + flat_final
    v_result := CEIL((v_base_cost + v_flat_base) * (0.11 + v_pct_bonus / 100.0))
                + v_flat_final;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;


-- -------------------------------------------------------------------
-- Создать игру с полем за одну транзакцию.
-- Заменяет create_game_with_rules — теперь сразу инициализирует поле.
-- -------------------------------------------------------------------
DROP FUNCTION IF EXISTS create_game_full(BIGINT, BIGINT, INT, BIGINT, UUID);
CREATE OR REPLACE FUNCTION create_game_full(
    p_seed             BIGINT,
    p_starting_balance BIGINT,
    p_max_turns        INT,
    p_target_balance   BIGINT,
    p_user_id          UUID
) RETURNS UUID AS $$
DECLARE
    v_game_id UUID;
BEGIN
    -- Создаём игру и правила
    INSERT INTO games (seed) VALUES (p_seed) RETURNING id INTO v_game_id;

    INSERT INTO game_rules (game_id, starting_balance, max_turns, target_balance)
    VALUES (v_game_id, p_starting_balance, p_max_turns, p_target_balance);

    -- Добавляем игрока (ход 1)
    INSERT INTO game_participants (game_id, user_id, turn_order, balance)
    VALUES (v_game_id, p_user_id, 1, p_starting_balance);

    -- Добавляем 4 ботов (ходы 2-5)
    INSERT INTO game_participants (game_id, user_id, turn_order, balance)
    SELECT v_game_id, u.id, ROW_NUMBER() OVER () + 1, p_starting_balance
    FROM users u
    WHERE u.type = 'bot'
    LIMIT 4;

    -- Инициализируем поле
    PERFORM init_game_board(v_game_id, p_seed);

    -- Переводим игру в активный статус
    UPDATE games SET status = 'active' WHERE id = v_game_id;

    RETURN v_game_id;
END;
$$ LANGUAGE plpgsql;